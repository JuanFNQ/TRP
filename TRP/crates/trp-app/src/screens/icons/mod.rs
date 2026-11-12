//! The inventory's pictures, drawn from each item's own model as the game makes them and kept on disk.
//!
//! The inventory's pictures as the game makes them (`docs/spec/icons.md`):
//! each item's own model, posed by its prefab's own icon settings, drawn
//! once by a copy of the game's icon camera with its three lights, finished
//! with its shadow and halved, kept as a PNG in `local\icon_cache\` and
//! read from there by every later run.
//!
//! The model is read on a thread of its own (as a weapon's rig is); the
//! frame puts it in a studio nothing else sees, draws it to a picture,
//! reads the picture back, and takes the model away again. One at a time.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::camera::{Exposure, RenderTarget, ScalingMode};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::camera::visibility::RenderLayers;
use bevy::tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task};
use trp_data::interface::icon::{Pose, Studio};
use trp_data::unity::assets::ObjRef;
use trp_data::unity::extract::{Hierarchy, MeshData, Node};

use crate::platform::convert;
use crate::screens::inventory_view::{Carried, Picture};
use crate::assets::render_data::PreparedLooks;

mod worker;
mod model_framing;
mod keys;
mod asking;
mod drawing;

pub(crate) use worker::*;
pub use keys::*;
use keys::{ANSWER_FRAMES_AT_MOST, Drawing, place_of, split_key};
#[cfg(test)]
use keys::{SWITCHED};
use asking::{Arrived, arrived, ask_icons, picture_of, receive_icons, report_icons};
use drawing::{draw_icons, prepare_icons};

/// The studio's own render layer: its camera sees this alone, and its lights
/// light this alone.
const LAYER: usize = 23;
/// The game's pixels to a cell (`ItemViewFactory.GetCellPixelSize`: so many
/// cells at 63, and one more).
const CELL_PIXELS: u32 = 63;
/// The cache's own folder under `local\icon_cache`: a new one when what is
/// drawn changes, so that an older picture is not read. (`v2`: a picture
/// of `v1` may have been read back before all
/// of its model was on the graphics card, without some of its parts.)
/// `v3`: a skinned mesh is drawn on its bones where they
/// lie (`render_data::pose_on_bones`); `v4`, a mesh on one
/// bone with it (it had been read with no weights, and drawn as it lay),
/// and a mesh's quads drawn (they had been left out). `v5`: a mesh that
/// casts its shadow alone is left out (the
/// repair kits' shadow meshes had drawn them white). `v6`: a weapon's iron
/// sights folded under a scope that looks along
/// the bore (`trp_sim::sights::fold_sights_as_made`). `v7`: framed by each
/// renderer's whole box, shadow meshes and lower
/// LODs among them, as the game frames them (the Bagariy's had been cut).
/// `v8`: normal and gloss pictures packed as BC7
/// drawn (`bc7_maps`; 348 and 48 keys had been drawn without some). `v9`:
/// a normal map's X red times alpha, as the game's shader
/// reads it (the Glock 17's slide's had been taken from red, full). `v10`:
/// drawn with the game's own surface (`game_surface`: its
/// shader's `_Icon` twin, where Bevy's surface had greyed and paled every
/// picture). `v11`: a mirrored object's faces culled as the
/// engine culls them (`Hierarchy::mirrored`: the Keymod 6" CASV rail's had
/// been drawn inside out). `v12`, with `SEE-THROUGH GLASS`:
/// the dithered glass drawn as its `_Icon` twin (`game_surface`: the PMAG
/// GEN M3 Window's rounds seen through its window, a face shield's glass
/// half clear); with it off, the pictures are `v11`'s, and kept there.
const CACHE: &str = "v12";
/// The folder of the pictures drawn with the glass opaque.
const CACHE_OPAQUE_GLASS: &str = "v11";

/// The cache's folder for this run: the glass's pictures differ with
/// `SEE-THROUGH GLASS`, and nothing else does.
fn cache_folder(glass: bool) -> &'static str {
    if glass { CACHE } else { CACHE_OPAQUE_GLASS }
}
/// How many frames a model stands in the studio at the least once all of
/// it is on the graphics card (`weapon::UploadWatch`) before it is read
/// back.
const SETTLE_FRAMES: u32 = 4;
/// What the studio waited before it waited for the card: so many frames,
/// and one more for each so many bytes of the model's pictures, where the
/// app sends 8 MB a frame (`main::UPLOAD_BUDGET`) and meshes besides, and
/// shares that with every rig and picture going up. Kept to say, in the
/// log, which pictures that had read back short.
const OLD_RULE_BYTES_A_FRAME: usize = 24 << 20;

/// An item's model as the icon thread makes it ready: its tree, what each
/// node draws, how the whole is posed in the studio and how much of it the
/// camera takes in.
pub struct PreparedIcon {
    template: String,
    /// The picture's size, in pixels (drawn at twice that).
    size: [u32; 2],
    hierarchy: Hierarchy,
    /// Whether the tree's first node keeps its own place and turn (a
    /// weapon's object, hung from its container) or is the posed object
    /// itself (any other item's prefab, whose own turn the pose replaces).
    keeps_own: bool,
    looks: PreparedLooks,
    parts: Vec<Vec<(usize, usize)>>,
    extra: Vec<(usize, usize, usize)>,
    /// The whole's turn and place in the studio, in the engine's axes, and
    /// the camera's half height.
    turn: Quat,
    place: Vec3,
    half_height: f32,
    /// The bytes of its pictures, for how long to let them reach the card.
    picture_bytes: usize,
    /// The icon camera as the install has it, with the first of a run.
    studio: Option<Studio>,
}

/// The turn of a model of the other game's that has no pose for a picture
/// of its own, in the install's axes: an outfit's lies on the ground face
/// up with its head towards minus z (the SEVA's visor is there), and is
/// looked down on with its head at the top: up turned to the camera, minus
/// z up, x to the left. (This work's choice, `PROVISIONAL`: the game draws
/// a model with no pose as it stands, which for one lying flat is its
/// edge; and the other game's own pictures are drawings, not models.)
const LAID_FACE_UP: Pose = Pose { rotation: [0.0, std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2, 0.0], position: [0.0; 3], has_offset: false, bounds_scale: 1.0 };

pub struct IconsPlugin;

impl Plugin for IconsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (ask_icons, receive_icons, prepare_icons, draw_icons, report_icons).chain());
    }
}

#[cfg(test)]
mod tests;
