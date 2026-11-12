//! The dummies on the range.
//! After `docs/spec/body-hits.md`, "The dummy on the range"; the options' `DUMMY`: PMCs down the
//! lanes, each a copy of the body on a skeleton of its own (`figure_view::Builder::dummy`) moved
//! and posed by a character of its own seen from outside with an AKM in its hands
//! (`trp_sim::puppet`: the same simulation as the player's), the AKM's model in its hands, the
//! player prefab's hit colliders on its bones, a kit and a look of its own, apart from the
//! player's, its clothing picked at random so as not to copy the player's: a new PMC's starting kit
//! with the AKM in its rifle's slot, its armour of that, and its suit and head picked as SPT's
//! server picks a PMC bot's; what searching takes off its body or puts on it stays its own (the
//! debug panel's HEAL AND STAND UP heals it and stands it up, renews its armour and dresses it in
//! nothing new), a PMC's health, and its death as the game's: the ragdoll on the game's physics
//! engine, the weapon let go on a spring while it stays in the body's slot, the killing hit's push
//! the frame after. One stands; the other walks to its right and its left in turn. The bullets meet
//! their colliders (`Dummies::ray`), ask their armour (`BulletWorld::wearer`) and hand each its
//! hits (`Dummies::take`), all in the game's axes. Its hits and its health reach its movement, its
//! kit's weight and armour slow it as a bot's, and its skills are at level 1 (`SKILL_LEVEL`). Since
//! 0.235.0 its armour's wear stays on its things (`keep_dummy_wear`), and its dead body's hung
//! weapon goes once the thing leaves its slot (`clear_dummy_weapons`). Since 0.238.0 (the dummy's
//! rest's last step) a knife's blade meets their colliders (`Dummies::blade_targets`, the stroke's
//! world: `knife::StrokeWorld`) and they take its blows (`blow_dummies`, `Dummies::take_blow`).
//! Since 0.247.0 (TRP's own posing economy, the field `posing_economy`; third-person spec, "Posing
//! by distance and off screen: the game's, and TRP's own") each is drawn by its size on screen
//! (`run_dummies`) and its colliders are placed only where a reader asks (`Dummy::placement`, the
//! bound first); `--start crowd-<n>` stands many (`crowd`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::camera::primitives::{Aabb, Frustum, Sphere};
use bevy::math::Affine3A;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use trp_data::character::player::{HandsData, LimbData};
use trp_data::character::ragdoll::{HitColliderData, Shape};
use trp_physics::{Body, BodyDesc, Collider as PhysicsCollider, Material, Shape as PhysicsShape, World};
use trp_sim::condition::armor::{self, ArmorRules, VestSkills, Wearer, Worn};
use trp_sim::combat::ballistics::{material, BodyRef, Hit, RayHit, Surface, V3};
use trp_sim::condition::health::{Health, HealthSettings, HealthSkills, Part};
use trp_sim::characters::lod::PosingDrawn;
use trp_sim::characters::puppet::{Posing, Puppet, PuppetPose};
use trp_sim::base::settings::WeightLimits;
use trp_sim::condition::skills::{Skill, SkillLevels};

use crate::platform::convert;

mod crowd_layout;
mod puppets;
mod dummy_state;
mod queries;
mod making;
mod upkeep;
mod running;
mod placing;

pub use crowd_layout::*;
pub use puppets::*;
pub use dummy_state::*;
pub use making::*;
pub use upkeep::*;
pub use running::*;
pub use placing::*;
use crowd_layout::{CHARACTER_SIZE, FACING, THINGS, THINGS_EACH, VIEW_MARGIN, VIEW_MARGIN_SHARE, WALK_SPAN};
use puppets::{Collider, Fall, Grip, Killing, Model, Placed, gear_of, health_skills, held_of};
use dummy_state::{PLACEMENTS_MADE, PosingLog};
#[cfg(test)]
use dummy_state::{Colliders};
use queries::{colliders_of, placed};
#[cfg(test)]
use queries::{blade_shape, enter};
use making::{in_slot, load_said, still_in_hands};
use running::{Posed, in_world, pose_dummy, pose_outfits, posed, world_of};
#[cfg(test)]
use running::{SeenBy, size_on_screen};

#[cfg(test)]
mod tests;
