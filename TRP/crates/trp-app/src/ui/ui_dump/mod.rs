//! The app's interface written out frame by frame in the lines the game's recorder writes.
//!
//! The app's interface written out frame by frame (`--dump-ui <file>`,
//! `--dump-ui-from <frame>`), in the lines the game's recorder writes of
//! the game's (`hosts\spt-oracle\Interface.cs`, recorder 0.16.0; the manual,
//! "Comparing with the game"), so that `uitrace` can set the two side by
//! side: a line for each frame on which anything of it changed, from the
//! frame asked for until the run ends. Frames are counted as the
//! screenshot's are (`capture`: the first frame after the warm-up is 1),
//! and `t` is the engine's clock, as `--log-frames` writes it.
//!
//! What the engine draws of a node is split as the game's interface splits
//! it, each drawn thing an element of its own: a node's ground
//! (`BackgroundColor`) and border (`BorderColor`) as `Image`s, its picture
//! (`ImageNode`) as an `Image` too (the game's pictures are mostly sprites
//! on `Image`s; its `RawImage`s are few), a UI material's (the radial fill
//! as a `Filled` image, the glow), and its words (`Text` with its spans, or
//! words drawn as the game draws them, `TmpText`) as `TextMeshProUGUI`. An
//! element's id is its entity's bits times 8 plus its part (0 the ground, 1
//! the border, 2 the picture, 3 the material, 4 the words), and its depth
//! the node's stack index times 8 plus its part, so that the depths keep
//! the engine's order of drawing. The copies of words drawn round them
//! (`WordsCopy`: a rim's, a shadow's) are left out, as the game draws those
//! in the words' own shader. What the engine does not draw is not written
//! either: a node hidden or of no size, a ground or picture fully clear.
//!
//! Boxes, ink and clips are in the window's physical pixels from its top
//! left; colours as authored (the screen's numbers, as the game's UI colours
//! are). `alpha` is always 1: the app folds its fades into the colours
//! themselves. A font's size is in the app's logical units, and the one
//! canvas's scale is the window's scale factor times `UiScale`, so that
//! size times scale is pixels in both files. Only nodes drawn to the window
//! are walked (the rangefinder's screen, drawn into a picture in the world,
//! is not). The interface's sounds played on the frame are written as the
//! recorder hears the game's (`ui_sound::play`, `Told`). Nothing of this is
//! added to the app without `--dump-ui`.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;

use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::widget::TextUiReader;
use bevy::ui::{CalculatedClip, ComputedUiTargetCamera, UiGlobalTransform, UiSystems};
use bevy::window::{CursorOptions, PrimaryWindow, WindowRef};

use crate::player::controller::Controller;
use crate::screens::health_view::{HealthLook, TmpText, WordsCopy};
use crate::screens::inventory_view::Selected;
use crate::ui::radial_fill::RadialFill;
use crate::audio::ui_sound::{Heard, Told};
use crate::weapons::wear::GlowMaterial;

mod frame_write;
mod elements;
mod recorder_format;

use frame_write::{Dump, Figures, Frames, Looks, Seen, count_frame, write_frame};
use elements::{box_of, element, font_name, ink_of, path_of, screen_of, words_colour};
use recorder_format::{color, f, font_of, p, r2, r4, rect, rgba, text, told};

/// Writes the interface to `path` from the frame `from` on, where a path is
/// given; adds nothing otherwise.
pub struct UiDumpPlugin {
    pub path: Option<PathBuf>,
    pub from: u32,
}

impl Plugin for UiDumpPlugin {
    fn build(&self, app: &mut App) {
        let Some(path) = &self.path else { return };
        let out = match std::fs::File::create(path) {
            Ok(file) => std::io::BufWriter::new(file),
            Err(e) => {
                warn!("the interface is not written to {}: {e}", path.display());
                return;
            }
        };
        info!("the interface is written to {} from frame {}", path.display(), self.from);
        app.insert_resource(Dump::new(out, self.from)).init_resource::<Frames>().init_resource::<Heard>();
        // Counted where the screenshot counts its frames: before the
        // warm-up's end is seen in the frame.
        app.add_systems(Update, count_frame.run_if(crate::running).before(crate::warm_up));
        // After the layout, the stack, the texts' glyphs and the clips are
        // all worked out for the frame, and the frame's sounds played.
        app.add_systems(
            PostUpdate,
            write_frame
                .after(crate::audio::ui_sound::play)
                .after(crate::ui::screen_fade::ScreensFaded)
                .after(UiSystems::Stack)
                .after(UiSystems::PostLayout)
                .after(bevy::ui::update::update_clipping_system)
                .after(bevy::transform::TransformSystems::Propagate),
        );
    }
}

#[cfg(test)]
mod tests;
