//! The window that asks how many of a stack to move, drawn as the game draws it.
//!
//! The window that asks how many as the game draws it (`TARKOV UI`;
//! inventory spec, "Part of a stack"): the install's `SplitDialog`, laid
//! out as its scripts leave it for a stack (`StepSlider`) or for food or
//! drink (`IntSlider`), over the screen darkened by its own ground; its
//! field's selection and caret as TextMeshPro draws them
//! (`TMP_InputField.GenerateHightlight`, `GenerateCaret`), and its buttons
//! and handle tinted by their state (`Selectable.colors`).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::screens::health_view::{draw_nodes, rgba, shown_of, Drawn, Frame, HealthLook, Marks, Piece, Words, EXACT};

/// Where the window's objects are numbered among the drawn pieces.
pub(crate) const SPLIT_DIALOG: usize = 33 << 20;

/// The install's words the window says: its captions (`"Split".Localized()`
/// and `"Transfer"` as the words have them, `"Use"` in capitals) and its
/// button's (`LocalizedText`, `ACCEPT`).
pub(crate) const WORD_KEYS: [&str; 4] = ["Split", "Transfer", "Use", "ACCEPT"];

/// What the window shows: its caption, which slider (the whole-number one
/// for food or drink), for each notch but the last whether the knob may
/// stand there, the handle's place as a share of the slider, what its
/// field says, whether the field has the keys and all it says is
/// selected, and the window's top left corner in the window's pixels
/// (`corner`).
pub(crate) struct SplitShown {
    pub(crate) title: String,
    pub(crate) int: bool,
    pub(crate) notches: Vec<bool>,
    pub(crate) share: f32,
    pub(crate) field: String,
    pub(crate) focused: bool,
    pub(crate) selected: bool,
    pub(crate) corner: Vec2,
}

/// Where the parts of the window that take the mouse lie, in the window's
/// pixels, each left, top, width and height: the window, its caption bar
/// (which moves it), its cross, its button, its field, the slider's track
/// with its last notch (all of the slider that takes the mouse besides its
/// handle: every picture in it takes it), its handle, and the area the
/// handle slides in (`Handle Slide Area`, the slider's whole width).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub(crate) struct SplitPlaces {
    pub(crate) window: [f32; 4],
    pub(crate) caption: [f32; 4],
    pub(crate) close: [f32; 4],
    pub(crate) accept: [f32; 4],
    pub(crate) field: [f32; 4],
    pub(crate) track: [f32; 4],
    pub(crate) handle: [f32; 4],
    pub(crate) slide: [f32; 4],
}

/// The window's size in units (its frame's own, `_window`).
pub(crate) fn split_size(look: &HealthLook) -> Option<Vec2> {
    let split = look.split.as_ref()?;
    let [w, h] = split.panel.nodes[split.window].layout.size_delta;
    Some(Vec2::new(w, h))
}

/// The window's top left corner, in the window's pixels, for its middle
/// asked at `middle` (`_window.position`, the pivot its middle) on a
/// screen of that size, `pixels` of the screen's own to one of the
/// window's: moved in to the screen where it would leave it, by the left
/// edge before the right and the bottom before the top, then its corner
/// put on a whole pixel of the screen's, half to even, the height counted
/// up from the bottom (`RectTransformExtensions.CorrectPositionResolution`,
/// `Math.Round`).
pub(crate) fn corner(middle: Vec2, size: Vec2, screen: Vec2, pixels: f32) -> Vec2 {
    let (middle, size, screen) = (middle * pixels, size * pixels, screen * pixels);
    let mut at = middle - size / 2.0;
    if at.x < 0.0 {
        at.x = 0.0;
    } else if at.x + size.x > screen.x {
        at.x = screen.x - size.x;
    }
    if at.y + size.y > screen.y {
        at.y = screen.y - size.y;
    } else if at.y < 0.0 {
        at.y = 0.0;
    }
    Vec2::new(at.x.round_ties_even(), screen.y - (screen.y - at.y).round_ties_even()) / pixels
}

/// The window laid out for drawing: its panel as its scripts leave it,
/// what of it is drawn, where its parts lie, and the screen in units.
pub(crate) struct SplitBuilt {
    pub(crate) panel: trp_data::interface::screen::ScenePanel,
    pub(crate) drawn: Drawn,
    pub(crate) places: SplitPlaces,
    pub(crate) units: [f32; 2],
}

/// The window as shown, on a screen of that many of the window's pixels,
/// `s` of them to a unit.
pub(crate) fn split_built(look: &HealthLook, shown: &SplitShown, screen: Vec2, s: f32) -> Option<SplitBuilt> {
    let split = look.split.as_ref()?;
    let size = split_size(look)?;
    let units = [screen.x / s, screen.y / s];
    let middle = shown.corner / s + size / 2.0;
    let mut panel = split.laid_out(shown.int, &shown.notches, shown.share, [middle.x, units[1] - middle.y]);
    let slider = split.slider(shown.int);
    if let Some(size) = fitted_size(look, &panel, slider.text, &shown.field, units) {
        if let Some(text) = panel.nodes[slider.text].text.as_mut() {
            text.size = size;
        }
    }
    let mut pictures = look.split_pictures.clone();
    pictures.resize(panel.nodes.len(), None);
    let drawn = Drawn { shown: shown_of(&panel.nodes, &[], &[], &[]), pictures };
    let px = |index: usize| panel.rect(index, units).map(|v| v * s);
    // The track and what hangs from it (its picture and last notch).
    let track = (0..panel.nodes.len()).filter(|&index| index == slider.container || panel.nodes[index].parent == Some(slider.container) && !panel.nodes[index].path.contains("(Clone)")).fold(px(slider.container), |[x, y, w, h], index| {
        let [ox, oy, ow, oh] = px(index);
        let (left, top) = (x.min(ox), y.min(oy));
        [left, top, (x + w).max(ox + ow) - left, (y + h).max(oy + oh) - top]
    });
    let slide = panel.nodes[slider.handle].parent.map_or(px(slider.object), px);
    let places = SplitPlaces {
        window: px(split.window),
        caption: px(split.caption),
        close: px(split.close.object),
        accept: px(split.accept.object),
        field: px(slider.input),
        track,
        handle: px(slider.handle),
        slide,
    };
    Some(SplitBuilt { panel, drawn, places, units })
}

/// The size a text sized to fit (`enableAutoSizing`: the field's, 5 to 18,
/// its own 16.7) is drawn at saying `said` in its rectangle on a screen of
/// so many units (`trp_data::tmp_text::auto_size`): the words fit where
/// their advance is no wider than the rectangle within its margins and
/// their line no higher. None for a text not sized so, or one that may
/// narrow its letters first (`PROVISIONAL`: not built; the field's may
/// not).
fn fitted_size(look: &HealthLook, panel: &trp_data::interface::screen::ScenePanel, index: usize, said: &str, units: [f32; 2]) -> Option<f32> {
    let text = panel.nodes[index].text.as_ref()?;
    let [least, most] = text.auto_size?;
    if text.narrowing > 0.0 {
        return None;
    }
    let (asset, _) = look.tmp_of(panel, text)?;
    let face = look.tmp_faces.get(&asset)?;
    let [_, _, w, h] = panel.rect(index, units);
    let (room, high) = (w - text.margin[0] - text.margin[2], h - text.margin[1] - text.margin[3]);
    let fits = |size: f32| {
        let layout = trp_data::interface::tmp_text::TmpLayout { em: size, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7, lead: 0.0 };
        let run = |words: &str| trp_data::interface::tmp_text::TmpRun { text: words.to_owned(), size, color: [1.0; 4], bold: text.style & 1 != 0 };
        let [wide, _] = trp_data::interface::tmp_text::measure(face, &[run(said)], &layout);
        let [_, tall] = trp_data::interface::tmp_text::measure(face, &[run("0")], &layout);
        wide <= room + 1e-4 && tall <= high + 1e-4
    };
    Some(trp_data::interface::tmp_text::auto_size(text.size, least, most, fits))
}

/// The caret of the window's field, shown on the first half of each blink
/// from when it is drawn (`TMP_InputField.CaretBlink`: shown while the
/// time since `m_BlinkStartTime` is less than half a blink), a blink
/// lasting one over the field's rate.
#[derive(Component)]
pub(crate) struct SplitCaret {
    pub(crate) blink: f32,
    pub(crate) since: Option<f32>,
}

/// A caret's width on the screen: its units at the screen's scale, and
/// never under a pixel. TRP's own: narrower, it lies between two pixels'
/// middles at some places and is not drawn at all (seen at 900 lines,
/// 0.83 of a pixel wide).
pub(crate) fn caret_width(units: f32, s: f32) -> f32 {
    (units * s).max(1.0)
}

/// Which of the window's controls a drawn piece is, tinted by its state.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum SplitControl {
    Close,
    Accept,
    Slider,
}

/// The pieces the window's controls tint (`Selectable.targetGraphic`),
/// each with its own colour and its tints, kept on the window's root.
#[derive(Component, Clone, Default)]
pub(crate) struct SplitTints(pub(crate) Vec<(usize, SplitControl, Color, trp_data::interface::screen::Tints)>);

/// The window drawn as the game draws it, in a frame as large as the
/// screen whose top left is the screen's: everything but its field's text,
/// then the field's selection (all it says, while it has the keys and all
/// is selected) or its caret (after the last figure, while it has the
/// keys and nothing is selected), then the field's text over them (the
/// selection and caret lie under the text: `TMP_SelectionCaret`, made the
/// text's first sibling). The pieces its controls tint, for `tints`.
pub(crate) fn draw_split_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &SplitBuilt, shown: &SplitShown, s: f32) -> SplitTints {
    let Some(split) = look.split.as_ref() else { return SplitTints::default() };
    let panel = &built.panel;
    let slider = split.slider(shown.int);
    let mut words: HashMap<usize, String> = (0..panel.nodes.len()).filter_map(|index| trp_data::interface::screen::localized(panel, index).map(|(key, case)| (index, trp_data::interface::screen::in_case(said.word(&key, &key), case)))).collect();
    words.insert(split.title, shown.title.clone());
    words.insert(slider.text, shown.field.clone());
    let frame = Frame::Is(built.units);
    let text = [slider.text];
    draw_nodes(column, look, panel, &built.drawn, SPLIT_DIALOG, Marks { words: Some(&words), skip: &text, ..Marks::default() }, said, frame, [0.0, 0.0], s);
    if let (true, Some(field)) = (shown.focused, field_box(look, panel, slider.text, &shown.field, Frame::Is(built.units), s)) {
        let at = |[x, y, w, h]: [f32; 4]| Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(w), height: Val::Px(h), ..default() };
        if shown.selected && !shown.field.is_empty() {
            column.spawn((at([field.left, field.top, field.right - field.left, field.height]), EXACT, BackgroundColor(rgba(slider.selection))));
        } else if !shown.selected {
            let color = slider.caret_color.map_or(field.color, rgba);
            column.spawn((at([field.right, field.top, caret_width(slider.caret_width, s), field.height]), EXACT, BackgroundColor(color), SplitCaret { blink: 1.0 / slider.blink_rate.max(1e-3), since: None }, Visibility::Inherited));
        }
    }
    draw_nodes(column, look, panel, &built.drawn, SPLIT_DIALOG, Marks { words: Some(&words), within: &text, ..Marks::default() }, said, frame, [0.0, 0.0], s);
    let mut tints = Vec::new();
    for (control, tinted, own) in [(SplitControl::Close, split.close.tinted, split.close.tints), (SplitControl::Accept, split.accept.tinted, split.accept.tints), (SplitControl::Slider, slider.tinted, slider.tints)] {
        let (Some(tinted), Some(own)) = (tinted, own) else { continue };
        let Some(image) = panel.nodes[tinted].image else { continue };
        tints.push((SPLIT_DIALOG + tinted, control, rgba(image.color), own));
    }
    SplitTints(tints)
}

/// Where the field's words lie as TextMeshPro lays them in its text's
/// rectangle (`field_box`), in the window's pixels: from the first
/// figure's origin to the last's advance (with the spacing after it), the
/// line's ascender to its descender, and the text's colour. With nothing
/// typed, both ends at the middle (the field's zero-width space).
pub(crate) struct FieldBox {
    pub(crate) left: f32,
    pub(crate) right: f32,
    pub(crate) top: f32,
    pub(crate) height: f32,
    pub(crate) color: Color,
}

/// `frame` is how the panel's top lies, as its drawing has it.
pub(crate) fn field_box(look: &HealthLook, panel: &trp_data::interface::screen::ScenePanel, index: usize, said: &str, frame: Frame, s: f32) -> Option<FieldBox> {
    let text = panel.nodes[index].text.as_ref()?;
    let (asset, _) = look.tmp_of(panel, text)?;
    let face = look.tmp_faces.get(&asset)?;
    let size = text.size * s;
    let layout = trp_data::interface::tmp_text::TmpLayout { em: size, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7, lead: 0.0 };
    let run = |words: &str| trp_data::interface::tmp_text::TmpRun { text: words.to_owned(), size, color: [1.0; 4], bold: text.style & 1 != 0 };
    let [widest, _] = trp_data::interface::tmp_text::measure(face, &[run(said)], &layout);
    // The line's height whatever it says (a figure's, empty or not).
    let [_, height] = trp_data::interface::tmp_text::measure(face, &[run("0")], &layout);
    let advance = if said.is_empty() { 0.0 } else { widest + (face.font.normal_spacing + text.spacing) * size / 100.0 };
    let rect = match frame {
        Frame::Is(units) => panel.rect(index, units),
        Frame::In(outer) => panel.placed_in(index, outer).rect,
    };
    let [x, y, w, h] = rect.map(|v| v * s);
    let (x, w) = (x + text.margin[0] * s, w - (text.margin[0] + text.margin[2]) * s);
    let (y, h) = (y + text.margin[1] * s, h - (text.margin[1] + text.margin[3]) * s);
    let left = match text.horizontal & 7 {
        2 => x + (w - widest) / 2.0,
        4 => x + w - widest,
        _ => x,
    };
    let top = match text.vertical {
        256 => y,
        1024 => y + h - height,
        _ => y + (h - height) / 2.0,
    };
    Some(FieldBox { left, right: left + advance, top, height, color: rgba(text.color) })
}

/// The caret blinks: shown on the first half of each blink from when it
/// was drawn.
pub(crate) fn blink_caret(time: Res<Time<Real>>, mut carets: Query<(&mut SplitCaret, &mut Visibility)>) {
    let now = time.elapsed_secs();
    for (mut caret, mut shown) in &mut carets {
        let since = *caret.since.get_or_insert(now);
        let on = (now - since) % caret.blink < caret.blink / 2.0;
        let wanted = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *shown != wanted {
            *shown = wanted;
        }
    }
}

/// A control's state as the engine tints by it (`Selectable.
/// currentSelectionState`): held down first, then chosen (the slider
/// after a press on it, until a press elsewhere), then under the mouse.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum SplitState {
    #[default]
    Normal,
    Highlighted,
    Selected,
    Pressed,
}

/// A tint's colour: the state's, by the multiplier.
pub(crate) fn tint_of(tints: &trp_data::interface::screen::Tints, state: SplitState) -> [f32; 4] {
    let tint = match state {
        SplitState::Normal => tints.normal,
        SplitState::Highlighted => tints.highlighted,
        SplitState::Selected => tints.selected,
        SplitState::Pressed => tints.pressed,
    };
    tint.map(|channel| channel * tints.multiplier)
}

/// A tint fading to another (`Graphic.CrossFadeColor`: straight across in
/// the tints' fade, unscaled time), by its piece.
#[derive(Clone, Copy)]
pub(crate) struct Fading {
    pub(crate) from: [f32; 4],
    pub(crate) to: [f32; 4],
    pub(crate) since: f32,
}

impl Fading {
    pub(crate) fn now(&self, length: f32) -> [f32; 4] {
        let share = if length > 0.0 { (self.since / length).min(1.0) } else { 1.0 };
        std::array::from_fn(|channel| self.from[channel] + (self.to[channel] - self.from[channel]) * share)
    }
}

/// The window's controls tinted by their states (`states`, by control),
/// each fading to its state's tint in its fade, over the pieces the window
/// drew them as.
pub(crate) fn tint_controls(
    tints: &SplitTints,
    states: &HashMap<SplitControl, SplitState>,
    fades: &mut HashMap<SplitControl, Fading>,
    delta: f32,
    pieces: &mut Query<(&Piece, &mut ImageNode)>,
) {
    let mut colors: Vec<(usize, Color)> = Vec::with_capacity(tints.0.len());
    for (piece, control, own, tints_of) in &tints.0 {
        let wanted = tint_of(tints_of, states.get(control).copied().unwrap_or_default());
        let fade = fades.entry(*control).or_insert(Fading { from: wanted, to: wanted, since: tints_of.fade });
        if fade.to != wanted {
            *fade = Fading { from: fade.now(tints_of.fade), to: wanted, since: 0.0 };
        }
        fade.since += delta;
        let tint = fade.now(tints_of.fade);
        let base = own.to_srgba();
        colors.push((*piece, Color::srgba(base.red * tint[0], base.green * tint[1], base.blue * tint[2], base.alpha * tint[3])));
    }
    for (drawn, mut image) in pieces.iter_mut() {
        let Some((_, color)) = colors.iter().find(|(piece, _)| *piece == drawn.0) else { continue };
        if image.color != *color {
            image.color = *color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's window over a stack at 2560 by 1440 (a capture of the
    /// game): its black ground from
    /// 1084 across and 317 down. A middle asked a pixel either side of its
    /// own lands on the same corner.
    #[test]
    fn the_window_s_corner_is_on_a_whole_pixel_of_the_screen() {
        let (size, screen) = (Vec2::new(418.1, 200.0) * (1440.0 / 1080.0), Vec2::new(2560.0, 1440.0));
        assert_eq!(corner(Vec2::new(1363.0, 450.0), size, screen, 1.0), Vec2::new(1084.0, 317.0));
        // Asked past the screen's edge, it is moved in: its right edge on
        // the screen's, its corner then rounded.
        let right = corner(Vec2::new(2550.0, 1430.0), size, screen, 1.0);
        assert_eq!(right, Vec2::new((2560.0 - size.x).round(), 1440.0 - (size.y).round()));
        let left = corner(Vec2::new(3.0, 2.0), size, screen, 1.0);
        assert_eq!(left, Vec2::ZERO);
    }

    /// A control's tint: held before chosen before under the mouse.
    #[test]
    fn a_control_is_tinted_by_its_state() {
        let tints = trp_data::interface::screen::Tints { normal: [1.0; 4], highlighted: [0.96, 0.96, 0.96, 1.0], pressed: [0.78, 0.78, 0.78, 1.0], selected: [0.96, 0.96, 0.96, 1.0], disabled: [0.78, 0.78, 0.78, 0.5], multiplier: 1.0, fade: 0.1 };
        assert_eq!(tint_of(&tints, SplitState::Pressed)[0], 0.78);
        assert_eq!(tint_of(&tints, SplitState::Normal)[0], 1.0);
        let fade = Fading { from: [1.0; 4], to: [0.78; 4], since: 0.05 };
        assert!((fade.now(0.1)[0] - 0.89).abs() < 1e-6);
    }
}
