//! The character's own model, drawn behind the gear page's slots and on the OVERALL tab.
//!
//! The character's own model on the inventory's gear page, behind the
//! equipment's slots where the app had a paper figure (`CHARACTER MODEL`):
//! a still model of the player's own model, standing straight in the stance of
//! the game's pre-raid screen.
//!
//! It is the body as drawn (`body_view`): **the same meshes, pictures,
//! materials and bind poses, by their handles**, on a skeleton of its own,
//! so that nothing of it is read or sent to the graphics card a second
//! time. Its skeleton stands as the game's own screens stand the character
//! with nothing in its hands (`trp_sim::body::BodyData::load_standing`: the
//! body animator's `GUI` layer, `inv_idle_hands_down`), turned to face its
//! camera as the game turns it, and its camera stands where the game's
//! screens put theirs (`PlayerModelView.Show`: the model 5 m ahead and 1 m
//! below). Each piece is shown while the body's own piece is: what is worn
//! comes and goes on it as it does on the body.
//!
//! The camera draws it onto a picture the page shows, on a render layer
//! nothing else is on, and **only when something has changed** (what it
//! wears, the window's size): a few frames and it is off again. The game's
//! screens let the animator run on; TRP's stands still by choice.
//!
//! The OVERALL tab has it too (`overall_view`), drawn by a camera of its
//! own as the game's scene has it (`trp_data::screen::ModelCamera`, the
//! scene's `Camera_inventory`: 28 degrees up and down, 0.3 to 25 m, from a
//! place before the model the scene gives), onto a picture twice the
//! view's size in the screen's units (`CameraImage.RecreateTexture`), and
//! turned by a drag across it (`XCoordRotation`). The two are drawn one
//! after the other, each with the model turned as its page has it; with
//! `TARKOV UI` the gear page has no model, and only the OVERALL tab's is
//! drawn.
//!
//! With the options' `OVERALL FIGURE` (a field of `CHARACTER BODY`; plan
//! steps 12-13, session B) the OVERALL tab has **a figure of its
//! own**, as the game's menu figure is (inventory spec, "The model's view",
//! "The hold and the random looks"): the same pieces again on the menu
//! player's skeleton (`Made::Overall`, on a layer of its own), run by
//! `trp_sim::figure` from nought at each showing and posed every frame
//! while the page is shown (the game's animator runs on), holding the
//! weapon in hand (its animated model, made anew at each showing from
//! `HandsHeld`; nothing for the empty hands), its hung weapons the
//! character's but that one; drawn every frame while shown, and lit by the
//! game's eight lights of `PlayerMVObjectLights` with no ambient and its
//! camera's PRISM filmic tonemap (`trp_data::screen::OverallPanel::lights`,
//! read once),
//! summed as the game sums them (in its numbers, by its
//! engine's light: `shading`), with the shadow at its feet on its root
//! (`boot_shadow`), its pieces drawn with the game's own surface since
//! 0.250.0 (`figure_surface`: twins of their materials of their own). The
//! gear page's figure stays as it was: still, hands down, the weapon in
//! hand left off, by its own light. Off, the OVERALL tab draws the gear
//! page's still figure by the inspect window's light, as before.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::window::PrimaryWindow;

use crate::platform::convert;
use crate::screens::inventory_view::Carried;
use crate::weapons::weapon::UploadWatch;

mod figure_state;
mod builder;
mod camera_draw;
mod live_weapon;

pub use figure_state::*;
pub use builder::*;
pub use camera_draw::*;
pub use live_weapon::*;
use figure_state::{GEAR, Held, OVERALL, Shot};
use camera_draw::{draw, mirror, turned};
#[cfg(test)]
use camera_draw::{overall_lights};
use live_weapon::{live, pictures_of};
#[cfg(test)]
use live_weapon::{held_now};

/// The figure's own render layer: its camera sees this alone, and its light
/// lights this alone.
pub const LAYER: usize = 22;
/// The OVERALL tab's own figure's layer, with `OVERALL FIGURE` on: its
/// camera's alone, as the gear page's figure is `LAYER`'s (the layers in
/// use: `weapon::HANDS_LAYER`).
pub const OVERALL_LAYER: usize = 18;
/// Where it stands: anywhere, nothing else being on its layer.
const PLACE: Vec3 = Vec3::new(0.0, -50.0, 0.0);
/// The game's turn of the model, half round the vertical, in the install's
/// axes (`PlayerModelLoader.CreatePlayerBody`).
const TURN: [f32; 4] = [0.0, 1.0, 0.0, 0.0];
/// Its camera, from the model's feet: 5 m before it and 1 m up, looking
/// level (`PlayerModelView.Show`: the model at (0, -1, 5) of the view).
const AHEAD: f32 = 5.0;
const UP: f32 = 1.0;
/// Room above the head's bone for the head and what is on it, and about
/// the whole, in metres. `ESTIMATE`: the app's framing in the panel.
const HEAD_ROOM: f32 = 0.28;
const MARGIN: f32 = 0.04;
/// The figure's light: the range's sun, from the same way and as bright,
/// without shadows. `ESTIMATE`: the game's screens light the model in a
/// scene of their own, not read.
const LIGHT_LUX: f32 = 12_000.0;
/// Its ambient, the range's own (the world's camera's light with `FACTORY
/// LIGHT` off; with it on the world has none of Bevy's, and the figure
/// keeps this). `ESTIMATE`, as its light.
const FIGURE_AMBIENT: f32 = 400.0;
const LIGHT_FROM: Vec3 = Vec3::new(3.0, 8.0, 4.0);
/// The layers the two shots' lights are on, each seen by its own camera
/// alone (a camera takes the directional lights whose layers it shares):
/// the gear page's light, and the OVERALL tab's.
const GEAR_LIGHTS: usize = 29;
const OVERALL_LIGHTS: usize = 30;
/// The camera stays on so many frames at the least for a drawing, and
/// until the renderer has nothing left to build; at most so many.
const DRAW_FRAMES: u32 = 2;
const DRAW_FRAMES_AT_MOST: u32 = 240;
/// The OVERALL tab's picture until its page is first shown, in pixels: any
/// size builds the same pipelines.
const WARM_SIZE: UVec2 = UVec2::splat(16);
/// The name it waits for its meshes and pictures under (`UploadWatch`).
const ON_CARD: &str = "the character model";

pub struct FigurePlugin;

impl Plugin for FigurePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FigureImage>()
            .init_resource::<OverallFigure>()
            .init_resource::<FigureWeapon>()
            .add_systems(
                Update,
                // (Then its pieces given the game's own surface, as made
                // this frame: `figure_surface`.)
                (mirror, draw, live, show_hung, crate::screens::figure_surface::swap.run_if(crate::screens::figure_surface::ready))
                    .chain()
                    .after(crate::player::body_view::show_outfit)
                    .after(crate::player::body_view::show_torso)
                    .after(crate::player::body_view::show_worn)
                    .after(crate::player::body_view::show_head_parts)
                    .after(crate::player::body_view::turn_hinges),
            );
    }
}

#[cfg(test)]
mod tests;
