//! The look read from the install, and the rows listed.

use super::*;

/// The MASTERING page as the game draws it: a weapon's views in the list
/// and the grid, a dropdown's line, the tooltip, each with what of it is
/// drawn, and the list's fill's picture.
pub(crate) struct MasteryLook {
    pub(crate) screen: MasteringScreen,
    pub(super) detailed_drawn: Drawn,
    pub(super) thumb_drawn: Drawn,
    pub(super) line_drawn: Drawn,
    pub(super) fill: Option<Picture>,
    pub(super) tooltip: Option<(ScenePanel, Drawn)>,
}

impl MasteryLook {
    /// Read from the SKILLS page as read (`SkillsLook::read`), its
    /// pictures through the reader.
    pub(crate) fn read(install: &trp_data::Install, page: &trp_data::interface::screen::SkillsScreen, file: &str, reader: &mut PictureReader, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Option<Self> {
        let mut screen = match MasteringScreen::of(page) {
            Ok(screen) => screen,
            Err(e) => {
                eprintln!("the game's MASTERING page could not be read; it is left dimmed: {e:#}");
                return None;
            }
        };
        if let Some(line) = screen.line.as_mut() {
            let texts: Vec<usize> = (0..line.nodes.len()).filter(|&index| line.nodes[index].text.is_some()).collect();
            inset_by_margins(line, &texts);
        }
        let view = |reader: &mut PictureReader, kept: &mut Vec<_>, panel: &ScenePanel, own: &str, scripted: &[String]| {
            let scripted: Vec<usize> = scripted.iter().filter_map(|path| panel.find(path)).collect();
            let mut drawn = Drawn { shown: shown_of(&panel.nodes, &scripted, &[], &scripted), pictures: Vec::new() };
            drawn.pictures = reader.panel(panel, &drawn.shown, own, kept);
            drawn
        };
        let in_list = [PICTURE, LOADER].map(|end| format!("Skill Icon/{end}"));
        let detailed_drawn = view(reader, kept, &screen.detailed, file, &in_list);
        let thumb_drawn = view(reader, kept, &screen.thumb, file, &[PICTURE.to_owned(), LOADER.to_owned()]);
        let line_drawn = screen.line.as_ref().map(|line| view(reader, kept, line, file, &[])).unwrap_or_default();
        let fill = screen.fill_sprite.as_ref().and_then(|(file, id)| reader.sprite(file, *id, kept));
        let tooltip = match trp_data::interface::screen::mastering_tooltip(install) {
            Ok(whole) => whole.find("Mastering Tooltip").map(|top| {
                let panel = whole.subtree(top);
                let own = panel.file.clone();
                let drawn = view(reader, kept, &panel, &own, &[]);
                (panel, drawn)
            }),
            Err(e) => {
                eprintln!("the game's mastering tooltip could not be read; the grid shows none: {e:#}");
                None
            }
        };
        Some(Self { screen, detailed_drawn, thumb_drawn, line_drawn, fill, tooltip })
    }

    /// Its panels, whose texts' fonts are read with the rest.
    pub(crate) fn panels(&self) -> Vec<&ScenePanel> {
        let mut panels = vec![&self.screen.detailed, &self.screen.thumb];
        panels.extend(self.screen.line.as_ref());
        panels.extend(self.tooltip.as_ref().map(|(panel, _)| panel));
        panels
    }
}

/// The objects of the page the scripts show and hide (`skills_view` reads
/// the page's pictures with them): the panel, the search field's two
/// words, each dropdown's list and arrows, the two views' tabs' states.
pub(crate) fn scripted_paths() -> Vec<String> {
    let mut paths: Vec<String> = vec![trp_data::interface::screen::MASTERING_PANEL.to_owned(), format!("{MASTERING_SEARCH}/Text Area/Placeholder"), format!("{MASTERING_SEARCH}/Text Area/Text")];
    for selector in MASTERING_DROPDOWNS {
        paths.extend(["Open Panel", "Button/Closed Image", "Button/Open Image"].map(|end| format!("{selector}/{end}")));
    }
    for tab in MASTERING_TABS {
        paths.extend([format!("{tab}/Normal"), format!("{tab}/Selected")]);
    }
    paths
}

/// The install's words the page says (`Words::load`).
pub fn word_keys() -> Vec<String> {
    ["Weapon name", "Show:", "All types", "Level "].into_iter().chain(SORTS).map(str::to_owned).collect()
}

/// A row of the page: a begun family's weapon, its names (`<id>
/// ShortName`, `<id> Name`), its type (`weapClass + " Mastering"`) and its
/// mastering as it shows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) template: String,
    pub(crate) short: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) shown: Shown,
}

/// The rows (`MasteringScreen._flatMastering`): each begun family in the
/// order begun, each of its weapons in the install's order, all on the
/// family's points; with `--mastering`, at the start of that level, as
/// every weapon reads it.
pub(crate) fn rows(mastery: &crate::player::skills::Mastery, texts: &trp_data::character::inventory::ItemTexts, db: &trp_data::Database) -> Vec<Row> {
    let mut rows = Vec::new();
    for (name, points) in mastery.levels.begun() {
        let Some(family) = mastery.families.by_name(name) else { continue };
        let points = mastery.run_level().map_or(*points, |level| family.points_at(level));
        let shown = family.shown(points);
        for weapon in &family.weapons {
            let Some(item) = db.items.get(weapon) else { continue };
            let class = item.props.get("weapClass").and_then(|class| class.as_str()).unwrap_or_default();
            let say = |key: String, otherwise: &str| texts.say(&key).filter(|said| !said.is_empty()).unwrap_or_else(|| otherwise.to_owned());
            rows.push(Row {
                template: weapon.clone(),
                short: say(format!("{weapon} ShortName"), weapon),
                name: say(format!("{weapon} Name"), weapon),
                kind: say(format!("{class} Mastering"), class),
                shown,
            });
        }
    }
    rows
}

/// The weapon types among the rows in the order first met, each with its
/// count (`MasteringScreen.ItemTemplates`, `GetAcceptableWeaponsCount`).
fn types(rows: &[Row]) -> Vec<(String, usize)> {
    let mut types: Vec<(String, usize)> = Vec::new();
    for row in rows {
        match types.iter_mut().find(|(kind, _)| *kind == row.kind) {
            Some((_, count)) => *count += 1,
            None => types.push((row.kind.clone(), 1)),
        }
    }
    types
}

/// The dropdowns' lines: the filter's ("All types (N)", then each type
/// with its count), the sort's.
pub(super) fn lines_of(said: &Words, rows: &[Row], role: usize) -> Vec<String> {
    match role {
        0 => std::iter::once(format!("{} ({})", said.word("All types", "All types"), rows.len())).chain(types(rows).into_iter().map(|(kind, count)| format!("{kind} ({count})"))).collect(),
        _ => SORTS.iter().map(|sort| said.word(sort, sort)).collect(),
    }
}

/// How many lines each dropdown has: the filter's, the sort's.
pub(crate) fn counts(said: &Words, rows: &[Row]) -> [usize; 2] {
    [lines_of(said, rows, 0).len(), SORTS.len()]
}

/// The rows listed (`MasteringContainer`): of the filter's type, whose
/// name holds what is typed whatever its case (`AcceptableFilter`); in the
/// rows' order, by level from the highest and then the share into it (a
/// stable sort, `MasteringLevelComparer`), or by when each last earned
/// (none earns here: the rows' order, `PROVISIONAL`).
pub(super) fn listed(rows: &[Row], choice: [usize; 2], search: &str) -> Vec<usize> {
    let types = types(rows);
    let kind = choice[0].checked_sub(1).and_then(|n| types.get(n)).map(|(kind, _)| kind.as_str());
    let needle = search.to_lowercase();
    let mut listed: Vec<usize> = (0..rows.len()).filter(|&n| kind.is_none_or(|kind| rows[n].kind == kind) && rows[n].name.to_lowercase().contains(&needle)).collect();
    if choice[1] == 1 {
        listed.sort_by(|&a, &b| {
            let (a, b) = (&rows[a].shown, &rows[b].shown);
            b.level.cmp(&a.level).then(b.share.partial_cmp(&a.share).unwrap_or(std::cmp::Ordering::Equal))
        });
    }
    listed
}

/// A row's level and points as the page writes them (`MasteringPanel`,
/// `MasteringIcon.UpdateProgress`): "Level: N" (the level and one), the
/// points into the level cut to a whole over what it takes, the mark's
/// figure.
pub(super) fn row_words(said: &Words, shown: &Shown) -> (String, String, String) {
    let level = shown.level + 1;
    (format!("{}{level}", said.word("Level ", "Level: ")), format!("<color=#c5c3b2><b>{}</b></color>/{}", shown.into as i32, shown.takes), level.to_string())
}
