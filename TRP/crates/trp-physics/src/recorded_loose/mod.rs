//! Things let fall and thrown, recorded, set beside the engine's.
//! For `bin/loose` (`docs/spec/physics.md`, "Loose things"). Recorder 0.9.0 on
//! (course 1 of `courses2`) writes each thing as the game sets it going
//! (`GameWorld.OnThrowItem`): its template, weight, place, turn, velocity
//! and spin, the thrower, the ground under it and its whole physical
//! set-up; and on every frame after, its place, turn, velocity, spin and
//! sleep, until the game takes its body away (`"stopped"`). Each is set
//! beside the rule that set it going (`loose::thrown`, `loose::let_fall`),
//! the body the install's model gives (`loose::add`), and that body's
//! fall, stepped as the game steps (once a frame, by the frame's recorded
//! step) on the recorded ground under where it set off.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use crate::loose::{self, Dice, Start, Thrower, Watch, SWEPT_UNDER};
use crate::setup::{add, angle, distance, f, into, last, q4, show, sub, tensor, v3, Report};
use crate::*;

mod trace_read;
mod setup_compare;
mod falls;

pub use trace_read::*;
pub use setup_compare::*;
pub use falls::*;
use trace_read::{unspent};

/// A frame of a thing's fall as recorded: after the frame's step, or as
/// it was set going on the first.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The recording's time.
    pub t: f32,
    /// The frame's own time (`Time.deltaTime`).
    pub dt: f32,
    /// The physics' step on this frame (`phys`); none where the game did
    /// not step.
    pub step: Option<f32>,
    /// Its place (its centre of mass: the game moves its model back by it)
    /// and turn.
    pub at: V3,
    pub turn: Q4,
    /// Its velocity, spin and sleep while it has a body; none on the frame
    /// the game took the body away (its place then as it lies).
    pub moving: Option<(V3, V3, bool)>,
}

/// A thing set going, as recorded.
#[derive(Debug, Clone)]
pub struct Thrown {
    pub id: u64,
    pub template: String,
    /// Its weight with all in it (`TotalWeight`).
    pub kg: f32,
    /// Where and how it set off (its object's place and turn, its body's
    /// velocity and spin).
    pub start: Start,
    pub player_pos: Option<V3>,
    pub player_vel: Option<V3>,
    /// The thrower's look (`Player.Rotation`: yaw, pitch).
    pub look: Option<[f32; 2]>,
    pub camera_rot: Option<Q4>,
    /// The thrower's `Weapon_root_anim` in the player's axes, on that frame.
    pub weapon_root_anim: Option<Q4>,
    /// The thrower's capsule on that frame: its height, centre's height
    /// and radius.
    pub capsule: Option<V3>,
    /// The frame's own time on that frame.
    pub dt: f32,
    /// The ground under where it set off: the point, its normal and the
    /// layer of what was met.
    pub ground: Option<(V3, V3, String)>,
    pub setup: Value,
    /// Its frames, the first the one it was set going on.
    pub frames: Vec<Frame>,
    /// Taken up while it moved (its last frame, far off where the game
    /// keeps what it has taken up, left out), so long after its start.
    pub taken_up: Option<f32>,
}

impl Thrown {
    /// Let fall (`DropMod`: 1 cm/s down, nothing of the thrower's) rather
    /// than thrown.
    pub fn let_fall(&self) -> bool {
        distance(self.start.velocity, [0.0, -0.01, 0.0]) < 1e-6
    }

    /// The time from its start to a frame.
    pub fn since(&self, frame: &Frame) -> f32 {
        frame.t - self.frames.first().map_or(frame.t, |f| f.t)
    }

    /// Whether the game took its body away within the recording.
    pub fn stopped(&self) -> bool {
        self.frames.last().is_some_and(|f| f.moving.is_none())
    }
}

pub struct Trace {
    pub header: Value,
    pub things: Vec<Thrown>,
}
