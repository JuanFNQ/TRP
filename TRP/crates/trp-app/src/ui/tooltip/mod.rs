//! The game's two tooltips, the simple one and the one of lines, which the screens share.
//!
//! The game's tooltips, which the screens share (inventory spec,
//! "Tooltips"): the simple one (`ItemUiContext.Tooltip`, a `SimpleTooltip`)
//! and the one of lines (`ItemUiContext.MultiLineTooltip`). Each is one
//! object that a screen shows and closes (`Tooltip.Show`, `Close`); here a
//! screen asks for one on each frame it wants it (`SimpleTip::show`,
//! `LinesTip::show`), and on a frame no screen asks for it, it is closed.
//! Drawn as their prefabs have them (`HealthLook::simple_tip_look`,
//! `lines_tip_look`), placed by the mouse on every frame (`Tooltip.Update`,
//! `SetPosition`, `RectTransformExtensions.CorrectPositionResolution`).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::screens::health_view::{HealthLook, Sprite, TmpText, EXACT};
use trp_sim::items::item_attributes::TipLine;

mod tip_layout;
mod tip_draw;

pub(crate) use tip_layout::*;
use tip_layout::{ground, leading_mark, plain_spans, resize_frame, root_node, tmp_text, words_at, words_node, FramePieces};
use tip_draw::{draw_lines_tip, draw_simple_tip};
#[cfg(test)]
use tip_draw::{place_tip, Roots};

/// What a tooltip is shown for: the screen that asks and which of its
/// things (an item's number, a part's view). Asked for another, the wait
/// starts again, as `Tooltip.Show` is called as the mouse comes onto
/// something new.
pub type TipKey = (&'static str, u64);

/// Over every screen and window: the game's tooltips are on a canvas of
/// their own sorted over the inventory's (order 100).
const TIP_Z: i32 = 60;
/// The screens' height in units (the canvas's scale is the window's
/// height over it).
const HIGH: f32 = 1080.0;
/// A break in the tooltip of lines (`MultiLineRow/Line`, a sliced
/// `tooltip_separator_2`, 256 by 1 pixels with a pixel's border above):
/// its sprite's pixels to a unit, read from the install (`probe sprite
/// sharedassets44.assets 868`: 65.31). It is as high as its border
/// (`Image.preferredHeight` of a sliced picture), 100 / 65.31 units: 2.04
/// pixels at 1440 lines, as `courses10`'s `hover-tooltips` has it.
const SEPARATOR_PER_UNIT: f32 = 65.30612;
/// Room for an armour class's mark before a line of the same class as the
/// one above (`MultiLineInfo`: `<space=1.5em>`), in ems of its size.
const INDENT_EMS: f32 = 1.5;
/// How far an armour class's mark puts the words after it on (TextMeshPro's
/// `<sprite>`, its glyph's advance not read), in ems of the size:
/// `courses10`'s `hover-tooltips`, frame 600, "CHEST - 36 / 36" after its
/// mark 171.72 pixels wide and "FRONT PLATE - 55 / 55" 230.62, their words
/// alone 145.21 and 204.10 as the app lays them out (`probe tmptext`): 26.52
/// pixels at 18.67 (the rows after `<space=1.5em>` 28.01 more, as 1.5 em
/// makes them).
const MARK_EMS: f32 = 1.4205;
/// Where the mark lies in its row and how large, in units, the sprite's
/// pixels to units: measured on the take's picture (`anything-pic6.png`,
/// the "II" before "CHEST" lit (brighter than 60 of 255) over 1017 to 1037
/// across and 198 to 217 down, its row from 1014.33, 198.24; the sprite's
/// lit part is four fifths of it each way): drawn at its own size (a unit
/// a pixel, its 20 by 19 at 26.67 by 25.33 pixels at 1440 lines), 0.15
/// units in from the row's left and 1.9 above its top (its lit part then
/// 2.67 to 23.67 pixels in and 0.24 above to 18.76 below the row's top,
/// the game's 2.67 to 22.67 and the same). `PROVISIONAL`: TextMeshPro's sprite asset is not
/// read.
const MARK_IN: f32 = 0.15;
const MARK_UP: f32 = 1.9;
const MARK_PER_PIXEL: f32 = 1.0;

pub struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimpleTip>().init_resource::<LinesTip>().add_systems(
            PostUpdate,
            // Before the engine works out what is seen, so that a tooltip made anew as its
            // words change is drawn on the frame it is made and does not blink.
            (draw_simple_tip, draw_lines_tip)
                .before(crate::screens::health_view::draw_tmp_texts)
                .before(bevy::ui::UiSystems::Layout)
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
    }
}

/// How a tooltip's words are drawn: the font, its size in units and its
/// colour, and, where the game's font asset is had, as the game draws it
/// (its asset and material, the text's spacing and kerning).
#[derive(Clone)]
pub(crate) struct TipWords {
    pub font: Handle<Font>,
    pub size: f32,
    pub color: Color,
    pub tmp: Option<((String, i64), (String, i64))>,
    pub spacing: f32,
    pub kerning: bool,
}

/// A tooltip's look: its words; its padding about them in units (left,
/// right, top, bottom) and the spacing of its rows; the most it is wide, in
/// units; its frame, and its frame's shadow with how far it lies out
/// (pixels: left, top, right, bottom); the app's frame's colour where the
/// game's pictures are not had; a break's line.
pub(crate) struct TipLook {
    pub words: TipWords,
    pub padding: [f32; 4],
    pub spacing: f32,
    pub widest: f32,
    pub border: Option<Sprite>,
    pub shadow: Option<(Sprite, [f32; 4])>,
    pub frame: Color,
    pub separator: Option<Sprite>,
}

/// The simple tooltip as a screen asks for it on this frame, and as it is
/// drawn.
#[derive(Resource, Default)]
pub struct SimpleTip {
    want: Option<(TipKey, String, f32, Vec2)>,
    since: Option<(TipKey, f32)>,
    /// Asked for afresh on this frame: the wait begun again, its key the
    /// same (`show_anew`).
    anew: bool,
    shown: Shown,
}

impl SimpleTip {
    /// Its lower left's offset from the mouse unless a screen gives one
    /// (`SimpleTooltip._defaultOffset`), in units, right and up.
    pub const OFFSET: Vec2 = Vec2::new(10.0, 10.0);

    /// Shows `words` (with TextMeshPro's colour tags) for `key`, once it has
    /// been asked for that long running (`SimpleTooltip.Show(text, null,
    /// delay)`: `StaticManager.WaitSeconds`, then `Display`).
    pub fn show(&mut self, key: TipKey, words: impl Into<String>, delay: f32) {
        self.show_at(key, words, delay, Self::OFFSET);
    }

    /// The same, its lower left `offset` units right of and above the mouse.
    pub fn show_at(&mut self, key: TipKey, words: impl Into<String>, delay: f32, offset: Vec2) {
        self.want = Some((key, words.into(), delay, offset));
    }

    /// Shows `words` for `key` as `show` does, but closed on this frame and
    /// its wait begun again, the key the same (`Tooltip.Close`, then
    /// `Show`: what the mouse was on gone from under it and the thing's
    /// view found under it again, its exit and its enter in one frame).
    pub fn show_anew(&mut self, key: TipKey, words: impl Into<String>, delay: f32) {
        self.show(key, words, delay);
        self.anew = true;
    }
}

/// The tooltip of lines as a screen asks for it on this frame, and as it is
/// drawn: shown at once, its lower left at the mouse (`MultiLineTooltip.
/// Show`: `Tooltip.Show()` with no offset and no delay).
#[derive(Resource, Default)]
pub struct LinesTip {
    want: Option<Vec<TipLine>>,
    shown: Shown,
}

impl LinesTip {
    pub fn show(&mut self, lines: Vec<TipLine>) {
        self.want = Some(lines);
    }
}

/// A tooltip as drawn: its root, what it was made of, its size as made and
/// as the layout last left it, and what of it is kept for new words.
#[derive(Default)]
struct Shown {
    root: Option<Entity>,
    made: Option<String>,
    size: Vec2,
    laid: Vec2,
    kept: Option<Kept>,
}

/// What of a simple tooltip drawn in the game's face is kept while its words
/// change (`SimpleTooltip.SetText`: the game sets its one label's text and
/// its layout sizes the ground to it): its shape (`tip_shape`), its frame's
/// pictures, and its words' box and text.
struct Kept {
    shape: String,
    frame: Option<FramePieces>,
    words: Entity,
    text: Entity,
}

/// What words must share with those a simple tooltip shows for it to keep
/// its entities and take them in place (else it is made afresh): the
/// screen's scale and the armour class's mark they begin with, drawn in the
/// game's face (none where the face is not had: always made afresh).
fn tip_shape(s: f32, mark: Option<u32>, measured: bool) -> Option<String> {
    measured.then(|| format!("{s}|{mark:?}"))
}

#[derive(Component)]
struct TipRoot;

#[cfg(test)]
mod tests;
