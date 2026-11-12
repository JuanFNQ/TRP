//! The OVERALL tab's texts and drawing.

use super::*;

/// Where the OVERALL tab's objects are numbered among the drawn pieces, and
/// its lists' lines, each line's past the one before it.
const OVERALL: usize = 17 << 20;

const OVERALL_LINES: usize = 18 << 20;

/// What the OVERALL tab shows as it stands (`overall_view`): each
/// dropdown's words and whether its box is lit (the mouse on it or on its
/// list), the list open, the model's picture, and whether a suit is being
/// read (the model's loading spinner, `PlayerModelView.Show`).
pub struct OverallShows<'a> {
    pub names: [&'a str; 2],
    /// The character's name (the options' `CHARACTER NAME`).
    pub name: &'a str,
    pub lit: [bool; 2],
    pub open: Option<OpenList<'a>>,
    pub figure: Option<Handle<Image>>,
    pub loading: bool,
    /// The character's level and experience (`PlayerLevelPanel.Set`, the
    /// window's experience label).
    pub level: u32,
    pub experience: i64,
}

/// A dropdown's list open: which dropdown's, its lines' words, how far it
/// is scrolled (units from its top) and its bar's handle: its place (left,
/// top, width, height in the screen's units) and its colour as tinted.
pub struct OpenList<'a> {
    pub role: usize,
    pub lines: &'a [&'a str],
    pub scroll: f32,
    pub handle: Option<([f32; 4], Color)>,
}

/// A text a script writes by `string.Format`, its values in its holes
/// (`{0}`, and `{1:00}` with its noughts). (Kept for the statistics once
/// they are counted; none is written with a figure now.)
#[cfg(test)]
pub(super) fn format_values(template: &str, values: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}').map(|at| open + at) else { break };
        let hole = &rest[open + 1..close];
        let (index, format) = hole.split_once(':').unwrap_or((hole, ""));
        match index.parse::<usize>().ok().and_then(|index| values.get(index)) {
            Some(value) => {
                let noughts = format.len();
                if noughts > 0 && format.chars().all(|c| c == '0') && value.len() < noughts {
                    out.push_str(&"0".repeat(noughts - value.len()));
                }
                out.push_str(value);
            }
            None => out.push_str(&rest[open..=close]),
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// A text with a figure's hole written without the figure: the hole and
/// the unit that follows it up to the next tag (`{0}%`, `{0}h`) give way
/// to `none`, so that a rate with no figure reads `S/R: -`, not `S/R: -%`.
pub(super) fn without_figure(template: &str, none: &str) -> String {
    let Some(open) = template.find('{') else { return template.to_owned() };
    let Some(close) = template[open..].find('}').map(|at| open + at + 1) else { return template.to_owned() };
    let end = template[close..].find('<').map_or(template.len(), |at| close + at);
    format!("{}{none}{}", &template[..open], &template[end..])
}

/// The words the OVERALL tab's scripts write on it for a character with no
/// statistics of its own (none is kept): the install's words for its
/// localized texts in their case; every figure of the short statistics
/// and the dashboard, the online time and the account's lifetime left
/// without a figure, the game's own `-` in its place (none is kept; the `-` is what
/// the scripts write for a figure not had, `OverallStatsPanel.FillStats`);
/// the character's level and experience (`PlayerLevelPanel.Set`: two
/// figures; the experience in groups of three); its name; each dropdown
/// its suit's name.
fn overall_texts(overall: &trp_data::interface::screen::OverallPanel, said: &Words, names: [&str; 2], nickname: &str, (character_level, experience): (u32, i64)) -> HashMap<usize, String> {
    let panel = &overall.panel;
    let mut texts: HashMap<usize, String> = overall.localized.iter().map(|(index, key, case)| (*index, trp_data::interface::screen::in_case(said.say(key, key), *case))).collect();
    // The objects a script's fields end with, in their order.
    let pointed = |path: &str, class: &str, count: usize| -> Vec<Option<usize>> {
        let Some(fields) = panel.find(path).and_then(|object| panel.script(object, class)) else { return vec![None; count] };
        let start = fields.len().saturating_sub(count * 12);
        (0..count).map(|n| panel.pointer_at(fields, start + n * 12)).collect()
    };
    fn put(texts: &mut HashMap<usize, String>, at: Option<usize>, words: String) {
        if let Some(at) = at {
            texts.insert(at, words);
        }
    }
    let none = said.say("-", "-");
    let short = pointed("LeftSide/CharacterPanel/Level Panel/Stats", "ShortStatsPanel", 6);
    for at in short.into_iter().flatten() {
        let template = texts.get(&at).cloned().unwrap_or_default();
        put(&mut texts, Some(at), without_figure(&template, &none));
    }
    // `_raidsLabel` to `_kdRatioLabel`, then the online time's and the
    // lifetime's.
    let dashboard = pointed("RightSide/Dashboard", "OverallStatsPanel", 14);
    for at in dashboard {
        put(&mut texts, at, none.clone());
    }
    let level = pointed("LeftSide/CharacterPanel/Level Panel", "PlayerLevelPanel", 2);
    put(&mut texts, level[0], format!("{character_level:02}"));
    // The character panel's: its nickname's and its experience's labels.
    let window = pointed("LeftSide", "InventoryPlayerModelWithStatsWindow", 15);
    put(&mut texts, window[0], nickname.to_owned());
    put(&mut texts, window[2], crate::screens::skills_view::thousands(experience.max(0) as u32));
    for (selector, name) in OVERALL_SELECTORS.iter().zip(names) {
        put(&mut texts, panel.find(&format!("{selector}/Text")), name.to_owned());
    }
    texts
}

/// Draws the game's OVERALL tab over the whole screen (`TARKOV UI`; screen
/// units, `s` pixels to one): its objects as `draw_nodes` draws them, the
/// model's picture in its view (drawn after what lies under the view and
/// before what lies over it), and the list open over all of it, as the
/// game puts it on its menus' layer (`DropDownBox.SetPanelState`): its
/// panel, its lines from the template in its scroll area's mask, and its
/// bar's handle.
pub fn draw_overall(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, screen: [f32; 2], shows: &OverallShows, s: f32) {
    let Some(overall) = &look.overall else {
        column.spawn(words("The game's OVERALL tab could not be read from the install.", 12.0, FAINT));
        return;
    };
    let texts = overall_texts(overall, said, shows.names, shows.name, (shows.level, shows.experience));
    // The name's and the experience's rows sized by their words
    // (TextMeshPro's preferred size, which the layout here does not ask of
    // a text itself), on a copy of the panel drawn this once.
    let mut fitted = overall.panel.clone();
    for path in [OVERALL_NICKNAME, OVERALL_EXPERIENCE] {
        let Some(at) = fitted.find(path) else { continue };
        let Some(text) = fitted.nodes[at].text.as_ref() else { continue };
        let Some(face) = fitted.file_of(text.font.0).and_then(|file| look.tmp_faces.get(&(file, text.font.1))) else { continue };
        let words = texts.get(&at).map_or("", String::as_str);
        let width = if words.is_empty() { 0.0 } else { face.font.width(words, text.size, text.spacing, text.style & 1 != 0) } + text.margin[0] + text.margin[2];
        let height = face.font.height(text.size) + text.margin[1] + text.margin[3];
        fitted.nodes[at].element = Some(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [width, height], flexible: [-1.0, -1.0] });
    }
    let panel = &fitted;
    let find = |path: &str| panel.find(path);
    let mut set: HashMap<usize, bool> = HashMap::new();
    let mut colors: HashMap<usize, Color> = HashMap::new();
    // The name in its member category's colour (`ChatSpecialIcon.Show`,
    // the name coloured).
    if let (Some(label), Some(c)) = (find(OVERALL_NICKNAME), look.member_colour) {
        colors.insert(label, Color::srgba(c[0], c[1], c[2], c[3]));
    }
    for (role, selector) in OVERALL_SELECTORS.iter().enumerate() {
        let open = shows.open.as_ref().is_some_and(|list| list.role == role);
        for (end, on) in [("Open Panel", open), ("Button/Closed Image", !open), ("Button/Open Image", open)] {
            if let Some(at) = find(&format!("{selector}/{end}")) {
                set.insert(at, on);
            }
        }
        // The box's ground under the mouse: its colour faded across by
        // the highlight (`Graphic.CrossFadeColor`, alpha with it), the
        // engine's multiply of the two.
        if let (true, Some(ground), Some(lit)) = (shows.lit[role], find(&format!("{selector}/Background")), overall.dropdown_highlight) {
            if let Some(image) = panel.nodes[ground].image {
                let c = image.color;
                colors.insert(ground, Color::srgba(c[0] * lit[0], c[1] * lit[1], c[2] * lit[2], c[3] * lit[3]));
            }
        }
    }
    if let Some(spinner) = find(&format!("{OVERALL_VIEW}/Progress Spinner")) {
        set.insert(spinner, shows.loading);
    }
    let mut sprites: HashMap<usize, Picture> = HashMap::new();
    if let (Some(icon), Some(rank)) = (find("LeftSide/CharacterPanel/Level Panel/Level Icon"), look.overall_pictures.get(rank_name(shows.level))) {
        sprites.insert(icon, *rank);
    }
    // The member category's mark (`IconSprite`) in the mark's place.
    if let (Some(mark), Some(picture)) = (find(OVERALL_MARK), look.overall_pictures.get("member")) {
        sprites.insert(mark, *picture);
    }
    // The side's and the prestige's at their own sizes (`SetNativeSize`).
    let mut pictures: HashMap<usize, Picture> = HashMap::new();
    for (path, name) in [(format!("{OVERALL_VIEW}/IconsContainer/SideImageHolder/SideImage"), "side"), (format!("{OVERALL_VIEW}/IconsContainer/PrestigeImageHolder/PrestigeImage"), "prestige")] {
        if let (Some(at), Some(picture)) = (find(&path), look.overall_pictures.get(name)) {
            pictures.insert(at, *picture);
        }
    }
    let view = find(OVERALL_VIEW);
    let clothing = find(OVERALL_CLOTHING);
    let right = find("RightSide");
    let open_panels: Vec<usize> = OVERALL_SELECTORS.iter().filter_map(|selector| find(&format!("{selector}/Open Panel"))).collect();
    // Under the view.
    let under: Vec<usize> = [view, clothing, right].into_iter().flatten().collect();
    let base = Marks { set: Some(&set), words: Some(&texts), colors: Some(&colors), sprites: Some(&sprites), pictures: Some(&pictures), ..Marks::default() };
    draw_nodes(column, look, panel, &look.overall_drawn, OVERALL, Marks { skip: &under, ..base }, said, Frame::In(screen), [0.0, 0.0], s);
    // The model, drawn into its view (`CameraImage`'s picture on its
    // `RawImage`, white).
    if let (Some(view), Some(figure)) = (view, shows.figure.clone()) {
        let [x, y, w, h] = panel.placed_in(view, screen).rect;
        let at = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
        column.spawn((Piece(OVERALL + panel.nodes.len()), at, EXACT, ImageNode::new(figure)));
    }
    // Over it, and the rest; the lists apart.
    let over: Vec<usize> = [view, clothing, right].into_iter().flatten().collect();
    let base = Marks { set: Some(&set), words: Some(&texts), colors: Some(&colors), sprites: Some(&sprites), pictures: Some(&pictures), ..Marks::default() };
    draw_nodes(column, look, panel, &look.overall_drawn, OVERALL, Marks { within: &over, skip: &open_panels, ..base }, said, Frame::In(screen), [0.0, 0.0], s);
    let Some(list) = &shows.open else { return };
    let selector = OVERALL_SELECTORS[list.role];
    let (Some(open), Some(area)) = (find(&format!("{selector}/Open Panel")), find(&format!("{selector}/Open Panel/Scroll Area"))) else { return };
    let handle = find(&format!("{selector}/Open Panel/Scrollbar/Sliding Area/Handle"));
    let skip: Vec<usize> = handle.into_iter().collect();
    let base = Marks { set: Some(&set), words: Some(&texts), ..Marks::default() };
    draw_nodes(column, look, panel, &look.overall_drawn, OVERALL, Marks { within: &[open], skip: &skip, ..base }, said, Frame::In(screen), [0.0, 0.0], s);
    // The lines in the scroll area's mask: laid out in its content's column
    // (`VerticalLayoutGroup`: its padding and spacing, each line as wide as
    // the column leaves and as high as it asks), from the scroll's place.
    let [ax, ay, aw, ah] = panel.placed_in(area, screen).rect;
    let clip = Node { position_type: PositionType::Absolute, left: Val::Px(ax * s), top: Val::Px(ay * s), width: Val::Px(aw * s), height: Val::Px(ah * s), overflow: Overflow::clip(), ..default() };
    let content = find(&format!("{selector}/Open Panel/Scroll Area/Content"));
    let row = content.and_then(|content| panel.nodes[content].group).unwrap_or_default();
    let [cx, cy, cw, _] = content.map_or([ax, ay, aw, ah], |content| panel.placed_in(content, screen).rect);
    let line_height = overall.line.as_ref().map_or(28.0, |line| line.asks(0, 1).0.max(1.0));
    let width = cw - row.padding[0] - row.padding[1];
    if let Some(line) = &overall.line {
        let text = (0..line.nodes.len()).find(|&index| line.nodes[index].text.is_some());
        column.spawn((clip, Visibility::Inherited)).with_children(|clip| {
            for (n, words) in list.lines.iter().enumerate() {
                let top = cy - ay + row.padding[2] + n as f32 * (line_height + row.spacing) - list.scroll;
                if top + line_height < 0.0 || top > ah {
                    continue;
                }
                let said_line: HashMap<usize, String> = text.map(|text| (text, (*words).to_owned())).into_iter().collect();
                let marks = Marks { words: Some(&said_line), ..Marks::default() };
                draw_nodes(clip, look, line, &look.line_drawn, OVERALL_LINES + (n << 8), marks, said, Frame::Is([width, line_height]), [cx - ax + row.padding[0], top], s);
            }
        });
    }
    if let Some(([x, y, w, h], color)) = list.handle {
        let at = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
        column.spawn((at, EXACT, BackgroundColor(color)));
    }
}
