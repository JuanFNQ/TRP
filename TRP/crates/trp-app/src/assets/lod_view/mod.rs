//! Levels of detail by distance on what is drawn, by TRP's own rule.
//! After the third-person spec, "TRP's own rule": the levels of detail and
//! the culling are TRP's own, an exception to rule 8, a pass of TRP's own
//! for speed made with many characters in mind. The game's pick (`trp_sim::lod::pick`, Unity's
//! rule) is kept in `trp_sim` as the game's, for reference; nothing here
//! draws by it since.
//!
//! Every level is spawned, each mesh tagged with its group and the levels
//! it is drawn at (`LodPiece`), and shown and hidden as the rest are (worn,
//! fitted, parked). Each group carries its levels' measured deviations
//! from the fullest (`trp_data::lod::deviations`); a level is drawn where
//! its deviation comes to under the options' tolerance of pixels in the
//! view that draws it (`trp_sim::lod::tolerance`, from `OBJECT LOD
//! QUALITY`), at that view's own resolution and field of view: the main
//! camera's, an optic's, a screen's figure's, each its own. So each piece
//! has a band of pixel sizes it is drawn between (`trp_sim::lod::band`),
//! handed to the shader as its mesh's tag (`MeshTag`); each frame, after
//! the engine has listed what each view sees, a piece its view's pixel at
//! it puts outside its band is taken off that view's list. Across a switch
//! the two levels are cross-faded by a dither in every view: Bevy's own
//! dithered visibility ranges, their pipeline switched on by a range that
//! never culls (`carrier`), their shader's level patched to TRP's band
//! (`trp_lod_dither`, `patch`), so each view fades by its own pixel.
//!
//! **No piece leaves alone**: past its last level a group keeps its
//! coarsest; a whole thing (all the tagged pieces under one top object: a
//! character, a thing on the ground, a prop) goes only when its largest
//! group covers under `trp_sim::lod::CULL_PIXELS` in that view. **A
//! model's parts switch together** (a weapon's with their weapon, by the
//! largest deviation among its groups at each level:
//! `trp_sim::lod::together`, `LodSpec::bundle`). **Shadows**: the sun's
//! cascades cast for a view take each group's coarsest level allowed by the
//! larger of the cascade's texel and the view's pixel at the piece, at once
//! (Bevy's shadow pass draws no dither); a light's own views (a point's, a
//! spot's) take the main camera's steady level.
//!
//! **Many characters**: the pick costs a distance and two comparisons for
//! each tagged mesh in a view's list (what is seen), one more for each
//! whole thing, nothing per frame for what no view sees; the bands are made
//! once a piece, again only when the tolerance changes. Bevy's own range
//! check and its extraction see every tagged mesh (an empty test each: the
//! carrier's range never culls). `PROVISIONAL`/`ESTIMATE`: the fade's width
//! (`trp_sim::lod::FADE`), the whole cull's pixel, a point or a spot light's
//! own shadow views taking the main camera's levels; a rigid level's pieces
//! fade by each one's own origin (Bevy hands the shader the mesh's place),
//! so where a group's levels stand on objects of their own the coarser
//! fades in a little sooner (`trp_sim::lod::OVERLAP`), every pixel covered
//! while they stand apart by less than that share of the distance (the log
//! says where they part: `origins`).
//!
//! **Two fields of the options' switches** (restart required): the
//! characters' (`distance_detail`, of `CHARACTER BODY`: `on`) and the
//! world's things' (`world_distance_detail`, of `WORLD INTERACTIONS`:
//! `world_on`: things on the ground and in flight with the rounds they
//! show, spent cases, the range's benches and poppers). Off, no coarser
//! level of theirs is spawned and nothing of theirs tagged, so the fullest
//! is drawn at any distance and nothing culled; both
//! off, no coarser level is read or measured and nothing here runs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bevy::asset::AssetEvent;
use bevy::camera::visibility::{CascadesVisibleEntities, CubemapVisibleEntities, VisibilityRange, VisibilitySystems, VisibleEntities, VisibleMeshEntities};
use bevy::light::cascade::Cascades;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use bevy::shader::{Shader, Source};

use crate::platform::convert;

mod levels;
mod dither_shader;

pub use levels::*;
use levels::{band_pieces, pick_levels};
#[cfg(test)]
use levels::{carrier};
use dither_shader::{hold, patch};
#[cfg(test)]
use dither_shader::{ASKED, trp_lod_dither};

/// The run's `distance_detail` (the characters') and
/// `world_distance_detail` (the world's things'), set once as the run
/// begins (restart required): every reader and spawner of models asks its
/// own.
static ON: AtomicBool = AtomicBool::new(false);
static WORLD_ON: AtomicBool = AtomicBool::new(false);

/// Whether the characters' levels of detail are built this run (their
/// bodies, gear, hung weapons and weapons in hand).
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Whether the world's things' levels of detail are built this run: things
/// on the ground and in flight and the rounds they show, spent cases, the
/// range's benches and poppers.
pub fn world_on() -> bool {
    WORLD_ON.load(Ordering::Relaxed)
}

/// Whether either is: a model read for both (`loose::Ready`, shared by the
/// ground and the hung weapons) carries its coarser levels.
pub fn any_on() -> bool {
    on() || world_on()
}

/// Sets the run's switches, before anything is spawned; with either on,
/// the rigid models' groups are measured as they are read
/// (`trp_data::lod::set_measuring`).
pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
    trp_data::drawing::lod::set_measuring(any_on());
}

pub fn set_world_on(on: bool) {
    WORLD_ON.store(on, Ordering::Relaxed);
    trp_data::drawing::lod::set_measuring(any_on());
}

pub struct LodPlugin;

impl Plugin for LodPlugin {
    fn build(&self, app: &mut App) {
        if !any_on() {
            return;
        }
        app.init_resource::<Wholes>().add_systems(Startup, hold).add_systems(Update, patch).add_systems(
            PostUpdate,
            (
                band_pieces.after(bevy::transform::TransformSystems::Propagate).before(VisibilitySystems::CheckVisibility),
                pick_levels
                    .after(VisibilitySystems::CheckVisibility)
                    .after(bevy::light::SimulationLightSystems::CheckLightVisibility)
                    .before(VisibilitySystems::MarkNewlyHiddenEntitiesInvisible),
            ),
        );
    }
}

#[cfg(test)]
mod tests;
