//! Mounting and the bipod.
//! After `docs/spec/mounting.md`: the search for a
//! place to rest the weapon, the mount's data worked out from what was
//! found, the figures the game reads for both, the weapon's own figures,
//! and what a mount tells its host. The mount's own state, which overrides
//! the movement state while it lasts (`IdleWeaponMountingState`), is the
//! movement's (`crate::movement`); the hand-over to the weapon and its
//! motion is here (`tell`).

use anyhow::{Context, Result};
use serde_json::Value as Json;

use crate::base::math::{delta_angle, inverse_lerp, lerp, quat_axis, quat_rotate};
use crate::locomotion::movement::World;

mod settings;
mod search;
mod mount_data;
mod weapon_figures;
mod telling;

pub use settings::*;
pub use search::*;
pub use mount_data::*;
pub use weapon_figures::*;
pub use telling::*;
use search::{add, magnitude, normalized, sub};

/// Which way a mount meets its cover (`EMountSideDirection`): on a top
/// ahead, or at a wall's free edge to the left or the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Forward,
    Left,
    Right,
}

impl Side {
    /// The game's number for it: 0 forward, -1 left, 1 right.
    pub fn number(self) -> i32 {
        match self {
            Side::Forward => 0,
            Side::Left => -1,
            Side::Right => 1,
        }
    }
}

/// A place found to rest the weapon on (`MountPointData`): the point, the
/// way the cover faces turned round, and the side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub at: [f32; 3],
    pub direction: [f32; 3],
    pub side: Side,
}

/// The world's layers a search casts on (`LayersMaskController`): the
/// detailed ones and the terrain (`HighPolyWithTerrainMask` and the
/// transparent layer), the terrain's coarse one and doors (`TerrainLowPoly`
/// and the door layer: the side search's second cast and the prone search),
/// and the character's static collision (`PlayerStaticCollisionsMask`: the
/// target place's ground and the way there). A host with one world answers
/// the same for all three. And the effects' (`ENVIRONMENT_HIT_MASK`: the
/// default layer, the terrain, the detailed colliders and the transparent
/// one, no character's): where blood goes (`crate::impacts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Detailed,
    Coarse,
    Static,
    Environment,
}

/// What a ray met first (`RaycastHit`): how far along it, where, and the
/// face's normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub distance: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
}

/// `TILT_POSITION_OFFSET`: how far the body is shifted sideways at the
/// yaw span's end, in metres, for a mount's yaw limit and its turn's check.
pub const TILT_POSITION_OFFSET: f32 = 0.25;
/// `PITCH_DEGREE_TO_POSE_LEVEL`: degrees of pitch to a whole unit of stance.
const PITCH_PER_POSE: f32 = 50.0;
/// The target place's offset from the pivot, crouched and standing
/// (`_localSitRsOffset`, `_localRsOffset`).
const SIT_OFFSET: [f32; 3] = [0.1866, 0.0, 0.2853];
const STAND_OFFSET: [f32; 3] = [0.1849, 0.0, 0.074];
/// `MAX_MOUNTING_POINT_OFFSET`: a side mount's drop at the span's end.
pub const SIDE_DROP: f32 = -0.1;
/// `POSSIBLE_POSITON_DELTASQR`: the target place's reach, squared.
const TARGET_REACH_SQUARED: f32 = 0.008;
/// The share of the way the body's `Inert` stays up past: 2.2 cm, squared.
const INERT_REACH_SQUARED: f32 = 0.0005;
/// `MAX_GROUND_SEARCH_DISTANCE`: the prone search's ray at most.
const PRONE_RAY_MOST: f32 = 0.4;
/// The prone search's free sphere's radius.
const PRONE_SPHERE: f32 = 0.2;
/// How far down the target place's ground is looked for.
const TARGET_GROUND_RAY: f32 = 3.0;
/// The weapon sticking this far into something puts a forward mount's
/// lower pitch limit at the current pitch (`CalculateDynamicDownLimit`).
const DOWN_LIMIT_OVERLAP: f32 = 0.01;
/// The fallback pivot's offset from the left hand's marker
/// (`ProceduralWeaponAnimation.FindMountingPoint`).
const MARKER_TO_PIVOT: [f32; 3] = [0.05, -0.18, 0.025];

pub(crate) const fn reach_squared() -> (f32, f32) {
    (TARGET_REACH_SQUARED, INERT_REACH_SQUARED)
}

pub(crate) const fn down_limit_overlap() -> f32 {
    DOWN_LIMIT_OVERLAP
}

#[cfg(test)]
mod tests;
