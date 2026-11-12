//! The tag window's colours, places and drawing.

use super::*;

/// A tag's colour by its number (`GetColor`).
pub fn tag_colour(color: u32) -> Color {
    let [r, g, b] = TAG_COLOURS[if (color as usize) < TAG_COLORS { color as usize } else { 0 }];
    Color::srgb(r, g, b)
}

/// The most letters a tag has where the window's field could not be read
/// (as the install's had it when read).
pub const TAG_LIMIT: u32 = 16;

/// A colour cell's side in the grid (`GridLayoutGroup.m_CellSize`, as read).
pub(super) const TAG_CELL: f32 = 25.0;

const TAG_CAPTION: &str = "Inner/Caption Panel/Caption";

const TAG_CAPTION_PANEL: &str = "Inner/Caption Panel";

const TAG_CLOSE: &str = "Inner/Caption Panel/Close Button";

pub(super) const TAG_COLORS_PANEL: &str = "Inner/Contents/ColorsAndCount/Colors";

pub(super) const TAG_COUNT: &str = "Inner/Contents/ColorsAndCount/Text";

pub(super) const TAG_INPUT: &str = "Inner/Contents/TagInput";

pub(super) const TAG_TEXT: &str = "Inner/Contents/TagInput/Text Area/Text";

pub(super) const TAG_PLACEHOLDER: &str = "Inner/Contents/TagInput/Text Area/Placeholder";

const TAG_SAVE: &str = "Inner/Contents/SaveButton";

pub(super) const TAG_SAVE_SIZE: &str = "Inner/Contents/SaveButton/SizeLabel";

const TAG_SAVE_LABEL: &str = "Inner/Contents/SaveButton/SizeLabel/Label";

/// The prefab's `n`th colour cell.
pub(super) fn tag_cell(n: usize) -> String {
    if n == 0 { format!("{TAG_COLORS_PANEL}/ColorCell") } else { format!("{TAG_COLORS_PANEL}/ColorCell ({n})") }
}

/// A line's height as TextMeshPro asks it of a layout: the face's ascent
/// to its descent at that size.
pub(super) fn tmp_line(font: &trp_data::interface::screen::TmpFont, size: f32) -> f32 {
    (font.ascent - font.descent) * size / font.point_size.max(1.0) * font.scale
}

/// The window that edits a tag as the game lays it out (`TARKOV UI`): its
/// size, and where in it its caption bar and cross, its field, its colour
/// cells and its button lie, in units from its top left corner.
pub struct TagPlaces {
    pub size: [f32; 2],
    pub caption: [f32; 4],
    pub close: [f32; 4],
    pub input: [f32; 4],
    pub cells: Vec<[f32; 4]>,
    pub save: [f32; 4],
}

pub fn tag_places(look: &HealthLook) -> Option<TagPlaces> {
    let panel = look.tag_window.as_ref()?;
    let size = info_size(panel);
    let rect = |path: &str| panel.find(path).map(|index| panel.rect(index, size));
    let cells = (0..TAG_COLORS).filter_map(|n| rect(&tag_cell(n))).collect();
    Some(TagPlaces { size, caption: rect(TAG_CAPTION_PANEL)?, close: rect(TAG_CLOSE)?, input: rect(TAG_INPUT)?, cells, save: rect(TAG_SAVE)? })
}

/// What the window that edits a tag shows: its field (the name typed so
/// far, its caret and selection, whether it has the keys) and the colour
/// chosen.
pub struct TagWords<'a> {
    pub field: &'a crate::screens::text_field::TextField,
    pub color: u32,
}

/// What of the field's words, `said`, lies at a place of the window, in
/// units from its top left corner (`FieldPlace::spot`); none where the
/// game's window or its field is not had.
pub(crate) fn tag_spot(look: &HealthLook, said: &str, at: [f32; 2]) -> Option<crate::screens::text_field::Spot> {
    let (panel, field) = (look.tag_window.as_ref()?, look.tag_field?);
    Some(crate::screens::text_field::field_place(look, panel, field.text, said, Frame::Is(info_size(panel)), 1.0)?.spot(said, at))
}

/// The window that edits a tag (`EditTagWindow`), as the game draws it with
/// what its script writes: its caption in the install's words, the colours
/// with the chosen one's border, the count of letters against the most
/// (`TagTextChange`), the name or, with none, the placeholder (`container
/// tag`), and its button's `SAVE`; in a frame at its top left corner, `s`
/// pixels to a unit, the `index`th window open. While its field has the
/// keys, the field's selection or its caret under its words (inventory
/// spec, "A text field": `text_field::draw_field_marks`, by the field's
/// own figures from the install).
pub fn draw_tag_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, words: &TagWords, index: usize, s: f32) {
    let Some(panel) = &look.tag_window else { return };
    let size = info_size(panel);
    let name = words.field.text.as_str();
    let mut texts = HashMap::new();
    let mut set = HashMap::new();
    let say = |key: &str| said.word(key, key);
    for (path, now) in [
        (TAG_CAPTION, say("ADD/EDIT CONTAINER TAG")),
        (TAG_COUNT, format!("{}/{}", words.field.letters(), look.tag_limit)),
        (TAG_TEXT, name.to_owned()),
        (TAG_PLACEHOLDER, say("container tag")),
        (TAG_SAVE_LABEL, say("Save").to_uppercase()),
        (TAG_SAVE_SIZE, say("Save").to_uppercase()),
    ] {
        if let Some(index) = panel.find(path) {
            texts.insert(index, now);
        }
    }
    for n in 0..TAG_COLORS {
        if let Some(border) = panel.find(&format!("{}/Border", tag_cell(n))) {
            set.insert(border, n as u32 == words.color);
        }
    }
    let named = !name.is_empty();
    for (path, on) in [(TAG_TEXT, named), (TAG_PLACEHOLDER, !named)] {
        if let Some(index) = panel.find(path) {
            set.insert(index, on);
        }
    }
    let base = TAG_WINDOW + (index << 12);
    let marks = || Marks { words: Some(&texts), set: Some(&set), ..Marks::default() };
    // With the keys: all but the field's words, then its selection or its
    // caret, then its words over them.
    let field = look.tag_field.filter(|_| words.field.focused);
    let Some((field, place)) = field.and_then(|field| Some((field, crate::screens::text_field::field_place(look, panel, field.text, name, Frame::Is(size), s)?))) else {
        draw_nodes(column, look, panel, &look.tag_window_drawn, base, marks(), said, Frame::Is(size), [0.0, 0.0], s);
        return;
    };
    let text = [field.text];
    draw_nodes(column, look, panel, &look.tag_window_drawn, base, Marks { skip: &text, ..marks() }, said, Frame::Is(size), [0.0, 0.0], s);
    crate::screens::text_field::draw_field_marks(column, &place, &field, words.field, s);
    draw_nodes(column, look, panel, &look.tag_window_drawn, base, Marks { within: &text, ..marks() }, said, Frame::Is(size), [0.0, 0.0], s);
}
