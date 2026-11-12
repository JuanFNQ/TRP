//! The skills page's look read from the install, and effectiveness.

use super::*;

/// The SKILLS tab as the game draws it (`TARKOV UI`): its page, a skill's
/// views in the list and the grid, a dropdown's line, the skill tooltip,
/// and the pictures its scripts put on them: the skills' and the buffs'
/// icons (`StaticIcons`), the fills' (normal, elite; the tooltip's base,
/// normal, elite), a buff's grounds (normal, elite), the level's rank.
pub(crate) struct SkillsLook {
    pub(crate) page: SkillsScreen,
    pub(crate) page_drawn: Drawn,
    pub(super) detailed_drawn: Drawn,
    pub(super) thumb_drawn: Drawn,
    pub(super) line_drawn: Drawn,
    pub(super) tooltip: Option<(ScenePanel, Drawn)>,
    pub(super) skill_icons: HashMap<String, Picture>,
    buff_icons: HashMap<String, Picture>,
    pub(super) fills: [Option<Picture>; 2],
    pub(super) grounds: [Option<Picture>; 2],
    pub(super) tip_fills: [Option<Picture>; 3],
    pub(super) tip_grounds: [Option<Picture>; 2],
    /// The ranks' pictures by the level each names (`Level Icons/rank5` to
    /// `rank80`, and `rank85` where the install has it).
    pub(super) ranks: Vec<(u32, Picture)>,
    /// The MASTERING page, where it was read; its tab dimmed where not.
    pub(crate) mastery: Option<crate::screens::mastery_view::MasteryLook>,
}

impl SkillsLook {
    /// Reads it, its pictures through the reader (`HealthLook::read`).
    pub(crate) fn read(install: &trp_data::Install, file: &str, reader: &mut PictureReader, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Option<Self> {
        let mut page = match SkillsScreen::load(install, file) {
            Ok(page) => page,
            Err(e) => {
                eprintln!("the game's SKILLS tab could not be read; the app's own list is drawn: {e:#}");
                return None;
            }
        };
        // Each dropdown's list as high as it shows of its lines (at most
        // `_maxVisibleHeight`, `DropDownBox.Update`), its words within
        // their margins, its lines' too.
        let most_seen = page.dropdown_most_seen.unwrap_or(120.0);
        let line_height = page.line.as_ref().map_or(28.0, |line| line.asks(0, 1).0.max(1.0));
        for (selector, count) in SKILL_DROPDOWNS.iter().zip([CLASSES.len() + 1, SORTS.len()]) {
            let row = page.panel.find(&format!("{selector}/Open Panel/Scroll Area/Content")).and_then(|content| page.panel.nodes[content].group).unwrap_or_default();
            let whole = row.padding[2] + row.padding[3] + count as f32 * line_height + count.saturating_sub(1) as f32 * row.spacing;
            if let Some(area) = page.panel.find(&format!("{selector}/Open Panel/Scroll Area")) {
                let element = page.panel.nodes[area].element.get_or_insert(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
                element.min[1] = whole.min(most_seen);
            }
        }
        let boxes: Vec<usize> = SKILL_DROPDOWNS.iter().filter_map(|selector| page.panel.find(&format!("{selector}/Text"))).collect();
        inset_by_margins(&mut page.panel, &boxes);
        if let Some(line) = page.line.as_mut() {
            let texts: Vec<usize> = (0..line.nodes.len()).filter(|&index| line.nodes[index].text.is_some()).collect();
            inset_by_margins(line, &texts);
        }
        // The experience's bar at none (`TwoValueBar.SetBase(0)`,
        // `SetNew(0)`): its fills closed at its left end, the glow there.
        let bar = |page: &SkillsScreen, end: &str| page.panel.find(&format!("{EXPERIENCE}/Bar/{end}"));
        if let Some(base) = bar(&page, "Base Fill") {
            page.panel.nodes[base].layout.anchor_max[0] = 0.0;
        }
        if let Some(new) = bar(&page, "New Fill") {
            let layout = &mut page.panel.nodes[new].layout;
            (layout.anchor_min[0], layout.anchor_max[0], layout.offset[0], layout.size_delta[0]) = (0.0, 0.0, 0.0, 0.0);
        }
        if let Some(glow) = bar(&page, "New Glow") {
            let layout = &mut page.panel.nodes[glow].layout;
            (layout.anchor_min[0], layout.anchor_max[0]) = (0.0, 0.0);
        }
        // The MASTERING page's dropdowns' boxes within their margins too.
        let boxes: Vec<usize> = trp_data::interface::screen::MASTERING_DROPDOWNS.iter().filter_map(|selector| page.panel.find(&format!("{selector}/Text"))).collect();
        inset_by_margins(&mut page.panel, &boxes);
        // What the scripts show and hide: the caption's tabs, the views'
        // tabs, the dropdowns' lists and arrows, each page's panel (the
        // chosen tab's on), and the MASTERING page's own.
        let panel = &page.panel;
        let mut scripted_paths: Vec<String> = [format!("{CAPTION_SKILLS}/Normal"), format!("{CAPTION_SKILLS}/Selected"), format!("{CAPTION_MASTERING}/Normal"), format!("{CAPTION_MASTERING}/Selected"), SKILLS_PANEL.to_owned()].into();
        for tab in VIEW_TABS {
            scripted_paths.extend([format!("{tab}/Normal"), format!("{tab}/Selected")]);
        }
        for selector in SKILL_DROPDOWNS {
            scripted_paths.extend(["Open Panel", "Button/Closed Image", "Button/Open Image"].map(|end| format!("{selector}/{end}")));
        }
        scripted_paths.extend(crate::screens::mastery_view::scripted_paths());
        let scripted: Vec<usize> = scripted_paths.iter().filter_map(|path| panel.find(path)).collect();
        let turned = scripted.clone();
        let mut page_drawn = Drawn { shown: shown_of(&panel.nodes, &turned, &[], &scripted), pictures: Vec::new() };
        page_drawn.pictures = reader.panel(panel, &page_drawn.shown, file, kept);
        // A skill's views: the buff's icon's template, the elite's mark and
        // the level as the scripts have them.
        let view = |reader: &mut PictureReader, kept: &mut Vec<_>, panel: &ScenePanel, scripted: &[&str]| {
            let scripted: Vec<usize> = scripted.iter().filter_map(|path| panel.find(path)).collect();
            let mut drawn = Drawn { shown: shown_of(&panel.nodes, &scripted, &[], &scripted), pictures: Vec::new() };
            drawn.pictures = reader.panel(panel, &drawn.shown, file, kept);
            drawn
        };
        let detailed_drawn = view(reader, kept, &page.detailed, &["Buffs/Buff Icon", "Skill Icon/Level Panel/Level", "Skill Icon/Level Panel/Elite Icon", ARROWS[0], ARROWS[1]]);
        let thumb_drawn = view(reader, kept, &page.thumb, &["Level Panel/Level", "Level Panel/Elite Icon"]);
        let line_drawn = page.line.as_ref().map(|line| view(reader, kept, line, &[])).unwrap_or_default();
        let sprite = |reader: &mut PictureReader, kept: &mut Vec<_>, at: &Option<(String, i64)>| at.as_ref().and_then(|(file, id)| reader.sprite(file, *id, kept));
        let fills = [sprite(reader, kept, &page.fill_sprites[0]), sprite(reader, kept, &page.fill_sprites[1])];
        let grounds = [sprite(reader, kept, &page.buff_grounds[0]), sprite(reader, kept, &page.buff_grounds[1])];
        // The tooltip: its box, as a panel of its own.
        let (mut tooltip, mut tip_fills, mut tip_grounds) = (None, [None, None, None], [None, None]);
        match trp_data::interface::screen::skill_tooltip(install) {
            Ok(tip) => {
                if let Some(top) = tip.panel.find("Skill Tooltip") {
                    let panel = tip.panel.subtree(top);
                    let arrows = ARROWS.map(|end| format!("Progress Panel/{end}"));
                    let scripted: Vec<usize> = ["Buff Container/Buff Panel", "Header/Level Panel/Level", "Header/Level Panel/Elite Icon", arrows[0].as_str(), arrows[1].as_str()].iter().filter_map(|path| panel.find(path)).collect();
                    let mut drawn = Drawn { shown: shown_of(&panel.nodes, &scripted, &[], &scripted), pictures: Vec::new() };
                    let own = panel.file.clone();
                    drawn.pictures = reader.panel(&panel, &drawn.shown, &own, kept);
                    tooltip = Some((panel, drawn));
                }
                tip_fills = [0, 1, 2].map(|n| sprite(reader, kept, &tip.fill_sprites[n]));
                tip_grounds = [0, 1].map(|n| sprite(reader, kept, &tip.buff_grounds[n]));
            }
            Err(e) => eprintln!("the game's skill tooltip could not be read; the skills show none: {e:#}"),
        }
        let icons = |reader: &mut PictureReader, kept: &mut Vec<_>, table: anyhow::Result<Vec<(String, (String, i64))>>, what: &str| -> HashMap<String, Picture> {
            match table {
                Ok(table) => table.into_iter().filter_map(|(name, (file, id))| Some((name, reader.sprite(&file, id, kept)?))).collect(),
                Err(e) => {
                    eprintln!("the game's icons of the {what} could not be read; they are left out: {e:#}");
                    HashMap::new()
                }
            }
        };
        let skill_icons = icons(reader, kept, trp_data::interface::screen::skill_icons(install), "skills");
        let buff_icons = icons(reader, kept, trp_data::interface::screen::buff_icons(install), "skills' buffs");
        let ranks = (1..=17u32).filter_map(|n| trp_data::interface::screen::resource_sprite(install, &format!("Level Icons/rank{}", n * 5)).and_then(|(file, id)| reader.sprite(&file, id, kept)).map(|picture| (n * 5, picture))).collect();
        let mastery = crate::screens::mastery_view::MasteryLook::read(install, &page, file, reader, kept);
        Some(Self { page, page_drawn, detailed_drawn, thumb_drawn, line_drawn, tooltip, skill_icons, buff_icons, fills, grounds, tip_fills, tip_grounds, ranks, mastery })
    }

    /// Its panels, whose texts' fonts are read with the rest.
    pub(crate) fn panels(&self) -> Vec<&ScenePanel> {
        let mut panels = vec![&self.page.panel, &self.page.detailed, &self.page.thumb];
        panels.extend(self.page.line.as_ref());
        panels.extend(self.tooltip.as_ref().map(|(panel, _)| panel));
        panels.extend(self.mastery.iter().flat_map(|mastery| mastery.panels()));
        panels
    }

    /// A buff's icon by its name (`BuffIdSprites`, keyed by the names the
    /// game saves its buffs by: `JsonEnumName`, which differs for one).
    pub(super) fn buff_icon(&self, id: &str) -> Option<Picture> {
        let saved = match id {
            "CharismaFenceRepPenaltyReduction" => "CharismaFenceReducePenalty",
            other => other,
        };
        self.buff_icons.get(saved).copied()
    }
}

/// A skill's effectiveness with so many points earned in the raid
/// (`SkillManager.GetEffectiveness`, the install's figures): fresh
/// (`SkillFreshEffectiveness`, 1.3) below `SkillFreshPoints`, then 1 for
/// `SkillPointsBeforeFatigue` more, then falling by `SkillFatiguePerPoint`
/// a point to at least `SkillMinEffectiveness`. Nothing earns here, so a
/// skill keeps its effectiveness at none earned, as at a raid's start
/// (`Skill`'s constructor: `GetEffectiveness(0)`). None where the install
/// lacks a figure.
pub fn effectiveness(config: &serde_json::Value, earned: i32) -> Option<f32> {
    let figure = |key: &str| config.get(key)?.as_f64().map(|v| v as f32);
    let fresh = figure("SkillFreshPoints")? as i32;
    let rested = fresh + figure("SkillPointsBeforeFatigue")? as i32;
    if earned < fresh {
        return figure("SkillFreshEffectiveness");
    }
    if earned < rested {
        return Some(1.0);
    }
    Some(figure("SkillMinEffectiveness")?.max(figure("SkillFatiguePerPoint")?.powi(1 + earned - rested)))
}

/// The install's words the tab says (`Words::load`).
pub fn word_keys() -> Vec<String> {
    let mut keys: Vec<String> = ["Level ", ELITE_LEVEL, "maxed", "All types", "current:", "remaining:", "SKILLS_SPEED_UP", "SKILLS_SPEED_DOWN"].into_iter().chain(SORTS).chain(CLASSES).map(str::to_owned).collect();
    for skill in Skill::ALL {
        let name = skill.name();
        keys.extend([name.to_owned(), format!("{name}Description"), format!("{name}LevelingUpDescription")]);
        keys.extend(buffs_of(skill).iter().map(|buff| buff.id.to_owned()));
    }
    keys
}
