//! Where a field's letters lie, what of them is under the mouse, and its selection and caret drawn.

use bevy::prelude::*;

use super::{words, Spot, TextField};
use crate::screens::health_view::{rgba, Frame, HealthLook, EXACT};
use crate::screens::split_view::{caret_width, field_box, SplitCaret};

/// Where a field's words lie as TextMeshPro lays them in its text's
/// rectangle, in the drawing's pixels: the gaps across (before each letter
/// and after the last: a letter's origin, the last one's advance), each
/// letter's drawn box across, the line's top and height (its ascender to
/// its descender), and the text's colour.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FieldPlace {
    pub(crate) gaps: Vec<f32>,
    pub(crate) boxes: Vec<[f32; 2]>,
    pub(crate) top: f32,
    pub(crate) height: f32,
    pub(crate) color: Color,
}

/// The place of a field's words, `said`, on its panel's text object:
/// `split_view::field_box` with every letter's place
/// (`TmpFace::letter_places`). `frame` is how the panel's top lies, as its
/// drawing has it; `s` the pixels to a unit.
pub(crate) fn field_place(look: &HealthLook, panel: &trp_data::interface::screen::ScenePanel, index: usize, said: &str, frame: Frame, s: f32) -> Option<FieldPlace> {
    let at = field_box(look, panel, index, said, frame, s)?;
    let text = panel.nodes[index].text.as_ref()?;
    let (asset, _) = look.tmp_of(panel, text)?;
    let (gaps, boxes) = look.tmp_faces.get(&asset)?.letter_places(said, text.size * s, text.spacing, text.style & 1 != 0, text.kerning);
    Some(FieldPlace { gaps: gaps.into_iter().map(|gap| at.left + gap).collect(), boxes: boxes.into_iter().map(|[left, right]| [at.left + left, at.left + right]).collect(), top: at.top, height: at.height, color: at.color })
}

impl FieldPlace {
    /// What of the field's words, `said`, lies at a place
    /// (`TMP_TextUtilities.GetCursorIndexFromPosition`, `FindIntersectingWord`).
    /// The nearest letter is the first whose drawn box the place is in
    /// across, else the one whose box's edge is nearest, the earlier of
    /// two as near; after the letters comes the box of no width that ends
    /// every field's line (its zero-width space). The gap is the one
    /// before that letter left of its box's middle, the one after from
    /// the middle on; a field that says nothing has the one gap. The word
    /// is the one the place is in, from its first letter's box to its
    /// last's and within the line's height.
    ///
    /// `PROVISIONAL`: the boxes go without the padding the text's material
    /// puts round a glyph, and the nearest is found across only (the
    /// component measures to the boxes' edges up and down as well).
    pub(crate) fn spot(&self, said: &str, [x, y]: [f32; 2]) -> Spot {
        let letters = self.boxes.len();
        let end = self.gaps.last().copied().unwrap_or_default();
        let away = |[left, right]: [f32; 2]| if x < left { left - x } else { (x - right).max(0.0) };
        let (mut letter, mut nearest) = (0, f32::MAX);
        for (index, ends) in self.boxes.iter().copied().chain([[end, end]]).enumerate() {
            let far = away(ends);
            if far < nearest {
                (letter, nearest) = (index, far);
            }
            if far <= 0.0 {
                break;
            }
        }
        let [left, right] = self.boxes.get(letter).copied().unwrap_or([end, end]);
        let gap = if letters == 0 { 0 } else if x < (left + right) / 2.0 { letter } else { (letter + 1).min(letters) };
        let on_line = y >= self.top && y <= self.top + self.height;
        let word = words(said).into_iter().find(|&(first, last)| on_line && last < letters && x >= self.boxes[first][0] && x <= self.boxes[last][1]).map(|(first, last)| (first, last + 1));
        Spot { gap, letter, word }
    }
}

/// A field's selection or its caret, drawn where its words lie (`place`)
/// with the field's own figures from the install (`TMP_InputField.
/// GenerateHightlight`, `GenerateCaret`). With something selected, a box
/// of the selection's colour from the selection's first gap to its last,
/// the line's height. Else the caret at its gap, as wide as the field has
/// it and never under a pixel (`split_view::caret_width`), the text's
/// colour unless the field has its own, blinking from when the field's
/// blink last began (`split_view::blink_caret`). Both lie under the words
/// in the game (`TMP_SelectionCaret`, the text's first sibling), so the
/// caller draws the words after this.
pub(crate) fn draw_field_marks(column: &mut ChildSpawnerCommands, place: &FieldPlace, figures: &trp_data::interface::screen::InputField, field: &TextField, s: f32) {
    let node = |[x, y, w, h]: [f32; 4]| Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(w), height: Val::Px(h), ..default() };
    let across = |gap: usize| place.gaps.get(gap).or(place.gaps.last()).copied().unwrap_or_default();
    match field.selection() {
        Some((from, to)) => {
            column.spawn((node([across(from), place.top, across(to) - across(from), place.height]), EXACT, BackgroundColor(rgba(figures.selection))));
        }
        None => {
            let color = figures.caret_color.map_or(place.color, rgba);
            let caret = SplitCaret { blink: 1.0 / figures.blink_rate.max(1e-3), since: field.blink_since };
            column.spawn((node([across(field.caret), place.top, caret_width(figures.caret_width, s), place.height]), EXACT, BackgroundColor(color), caret, Visibility::Inherited));
        }
    }
}
