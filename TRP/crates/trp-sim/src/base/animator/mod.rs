//! An animator: parameters, state machines, blend trees and the pose they make of a set of bones.
//! Transitions, root motion and curve-driven parameters too, through the layer
//! stack. See `docs/spec/body-animator.md`.
//!
//! It runs any controller read by `trp_data::states`; nothing here knows
//! about bodies or weapons.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use trp_data::character::rootmotion::{freeform_cartesian, freeform_directional, simple_directional, weight_1d};
use trp_data::weapons::states::{self, AnimState, MovementBehaviour, ParameterKind, StateTable, TreeNode};
use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property, MOTION_CURVE};
use trp_data::unity::{Assets, ObjRef};

use crate::base::math::{quat_lerp, quat_mul, quat_normalize, quat_rotate};

mod defs;
mod controller;
mod mixes;
mod access;
mod transitions;
mod stepping;
mod posing;

pub use defs::*;
pub use controller::*;
pub use mixes::*;
use defs::{Bone, BoneTracks};
use controller::{stays_in_pose};
use mixes::{Blend, DrawnMix, Driven, Held, LayerMix, MachineState, Playing, Share};

/// A state with no motion lasts this long, as in the engine.
const EMPTY_STATE_LENGTH: f32 = 1.0;
/// How many entry and exit points a transition may pass through.
const SELECTOR_DEPTH: usize = 8;

#[derive(Clone)]
pub struct Animator {
    pub controller: Arc<Controller>,
    values: Vec<f32>,
    machines: Vec<MachineState>,
    weights: Vec<f32>,
    root_delta: [f32; 3],
    /// How far the root turned over the last update, degrees to the right.
    root_turn: f32,
    pose: Vec<([f32; 3], [f32; 4])>,
    /// The scale of each bone of the controller's `scaled`, in its order.
    scales: Vec<[f32; 3]>,
    /// Whether a second pose is worked out at each update **for drawing**,
    /// and that pose: the same states at the same moments, their clips'
    /// sampled curves taken on curves without corners in place of the
    /// engine's straight lines (`Clip::sample_smooth`). Nothing of the
    /// animator goes by it.
    smooth: bool,
    drawn: Option<Vec<([f32; 3], [f32; 4])>>,
    events: Vec<Event>,
    /// The clips' animation events passed on the last update, each by its
    /// clip's place in the controller and its own in the clip.
    passed: Vec<(usize, usize)>,
    /// What the clips' parts were last sampled into: the part every update
    /// samples, and the rest (`ClipDef::rest`).
    samples: [Vec<f32>; 2],
    /// Whether an update leaves the bones it need not work out (those not
    /// `Controller::always`) to be made from its mix when asked (`pose_of`):
    /// their places in `pose` (and the pose for drawing's but the bones a
    /// layer leaves standing) are then the last that were worked out.
    deferred: bool,
    /// The last update's mix.
    mix: Mix,
    /// Whether `pose` and `drawn` hold every bone as the last update made
    /// it: it was not deferred.
    complete: bool,
    /// How long the update being worked out is, in seconds.
    step: f32,
    /// States asked for outright since the last update, by machine: the
    /// state and the point in its play.
    asked: Vec<Option<(usize, f32)>>,
    /// What the last update looked at, in order.
    looks: Vec<Look>,
    /// The engine's blend workspace of each layer of a machine (by machine
    /// and the layer's slot), one for all the machine's states: the
    /// children's weights the last blend worked out there
    /// (`CreateControllerWorkspace`, `EvaluateBlendTree`). Followed only on
    /// a machine that `keeps_weights`.
    workspace: HashMap<(usize, usize), Vec<f32>>,
    /// What each node `missing` its parameter took from its workspace at
    /// the last update, by machine, state, slot and node.
    kept: HashMap<(usize, usize, usize, usize), Vec<f32>>,
}

impl Animator {
}

#[cfg(test)]
mod tests;
