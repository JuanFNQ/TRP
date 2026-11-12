//! The character's own collision, for a world that is one line of ground with steps in it.
//! As the game's controller for the player works it (`SimpleCharacterController`;
//! movement spec, "The character's own collision"): what a replay rebuilds from a recorded path, and what a test sets
//! a wall or a box up in.
//!
//! The game moves the capsule by the frame's motion and then, for each
//! solid thing the capsule is in, moves it out by the shortest way. A way
//! out that points upward at all counts as ground under the character; one
//! that points mostly upward is taken straight up, so that the character
//! rides up onto low things and does not slide down gentle slopes. In a
//! vault or a climb the upward part of every way out is dropped. A move
//! that comes to more than a fifth of a metre in all is cut to a twentieth,
//! which is how a capsule that finds itself well inside something comes
//! out of it over several frames.
//!
//! A host with a collision engine of its own answers `World::obstructed`
//! from that; nothing here is asked of it.

use crate::locomotion::movement::Capsule;

mod course;
mod block;
mod upright;

pub use course::*;
pub use block::*;
pub use upright::*;

/// A way out counts as ground from this much of it pointing upward
/// (the cosine of 85 degrees: the controller's `_groundDotThreshold`).
const GROUND_DOT: f32 = 0.087_155_8;
/// From this much upward it is taken straight up
/// (`_snapGroundDenepentrationToUp`).
const SNAP_DOT: f32 = 0.64;
/// A step's move of this much or more in all is cut to `DEEP_STEP`
/// (`_maxDepenetrationDistance`, `_maxDepenetrationDistanceResolve`).
const DEEPEST: f32 = 0.2;
const DEEP_STEP: f32 = 0.05;
/// How near a surface the capsule counts as touching it, in metres.
const TOUCH: f32 = 0.0005;
/// How far into the ground under it the capsule's foot has to have been
/// for the move to be dropped (the controller's `_depenetrationBugRayAddition`).
const SUNK: f32 = 0.001;
/// Two neighbouring places of a path whose heights differ by more than
/// `FACE_RISE`, no further apart than `FACE_RUN`, have a face between them.
const FACE_RISE: f32 = 0.15;
const FACE_RUN: f32 = 0.12;

#[cfg(test)]
mod tests;
