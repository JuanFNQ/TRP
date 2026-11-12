//! The character's death as the game shows it.
//! After the health spec, "Death"; `docs/spec/physics.md`.
//!
//! The body becomes a ragdoll, as the game's corpse makes one
//! (`Corpse.CreateCorpse`, `CorpseRagdoll`): the player prefab's rigid
//! bodies and joints (`trp_data::ragdoll`) on a copy of the body as it is
//! drawn at that moment, in **the game's own physics engine, PhysX 4.1**,
//! set up from the install's physics settings and stepped as the game
//! steps it (`trp_physics`: the corpse there is the one `bin/corpse` sets
//! beside four recorded deaths), heard as it falls on the surface under
//! its feet (`step_sound::BodyFell`). The weapon in hand is let go a moment
//! later, a body of its own on the corpse's sling (`Player.DropItemDead`,
//! `CorpseRagdoll.AttachWeapon`). The view rides the chest, where the
//! camera's place hangs (`Cam`, under the spine). The picture fades (the
//! camera's `DeathFade`) as the raid's end blacks it out over a second,
//! and then the death screen (`health_view`). `Enter` there begins the run again where it began, in
//! place: what the run started with is put back (`StartOfRun`).
//!
//! The engine works in the game's axes (z ahead): what goes in is turned
//! from Bevy's and what comes out turned back (`convert`).

use std::collections::HashMap;

use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::camera::primitives::Aabb;
use bevy::transform::helper::TransformHelper;
use trp_data::character::ragdoll::RagdollData;
use trp_physics::corpse::Stepper;
use trp_physics::{Body, BodyDesc, Collider, Material, Shape, World};

use crate::player::body_view::{BodyParts, BodyRig, OutfitArms, OutfitOn};
use crate::player::controller::Controller;
use crate::screens::inventory_view::Carried;
use crate::world::range::Range;
use crate::weapons::weapon::{Eye, FirstPersonHands, WeaponRig};
use crate::{platform::convert, MainCamera, View};

mod corpse_state;
mod dying;
mod fade_again;

pub use corpse_state::*;
pub use dying::*;
pub use fade_again::*;
use corpse_state::{from_game, to_game, transform_of, under};

/// The camera's death effects (`DeathFade` on the camera prefab, health
/// spec, "Death"): the colour fades over 2 s, and from 1.8 s the eyes
/// close over 0.25 s, both by the prefab's curve (from 0 with no slope to
/// 1 with a slope of 2: the square of the time's share); the colour's
/// strength is that over 1.5, the eyes' over 2. Measured: the recorded
/// fade's colour stands at 0.251 one second after the death.
const FADE_TIME: f32 = 2.0;
const EYES_DELAY: f32 = 1.8;
const EYES_TIME: f32 = 0.25;
const FADE_SHARE: f32 = 1.0 / 1.5;
const EYES_SHARE: f32 = 0.5;
/// The raid's end (`BaseLocalGame.InitiateGameStopping`, `Stop`; health
/// spec, "How soon the raid ends"): a black image over everything, its
/// opacity raised by each frame's length from the frame of the death
/// (`PreloaderUI.StartBlackScreenShow(1, 1, …)`, `Co_ShowBlackImage`), whole
/// after a second, until it is 0.05 past whole; then the raid is let go.
/// Recorded: `courses2`'s deaths end 1.04 s after the death.
const BLACK_STEP: f32 = 1.0;
const BLACK_DONE: f32 = 1.05;
/// The death screen (the app's, which the game has not) comes when the
/// game's black is done and its raid would end.
pub const SCREEN_AT: f32 = BLACK_DONE / BLACK_STEP;
/// The weapon let go: its figures are the corpse's (`trp_physics::corpse`).
pub(crate) use trp_physics::corpse::{PISTOL_MASS, STRAP_BY_PALM, STRAP_DAMPER, STRAP_ON_PELVIS, STRAP_SPRING, UNITY_ANGULAR_DRAG, WEAPON_MASS};

pub struct DeathPlugin;

impl Plugin for DeathPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dying>().add_message::<BeginAgain>();
    }
}
