//! The mount's own state.
//! After the mounting spec, "Entering" to "What ends it, and what it ignores"
//! (`IdleWeaponMountingState`). While it lasts it is
//! the state the commands go to (`Player.CurrentManagedState`), and its
//! frame runs after the movement state's own (`MovementState.OnStateMove`);
//! the movement state in force goes on underneath, without the animator's
//! root motion and without turning the body.

use super::*;
use crate::weapons::mounting::{self, Asking, Data, Grid, MountEvent, Point, Search, Side, WeaponMount};

mod reading;
mod entering;
mod holding;


/// A mount under way: entering, held or leaving.
#[derive(Debug, Clone, Copy)]
pub(super) struct Mounted {
    pub(super) data: Data,
    /// The side's number (0 forward, -1 left, 1 right).
    side: i32,
    /// The approach's or the exit's time so far (`float_4`).
    timer: f32,
    /// The look's target pitch and yaw (`float_5`, `float_6`), the lean at
    /// the press and the entry's target lean (`float_7`, `float_8`).
    target_pitch: f32,
    target_yaw: f32,
    entry_tilt: f32,
    target_tilt: f32,
    /// The yaw offset the approach ends at (`float_9`), the yaw offset into
    /// the span (`float_10`) and the span's half forward, whole at a side
    /// (`float_11`).
    yaw_shift: f32,
    yaw_offset: f32,
    span: f32,
    exiting: bool,
    /// The lower pitch limit in force (`_downPitchLimit`).
    down_limit: f32,
    start_position: [f32; 3],
    start_look: [f32; 2],
    start_body: f32,
    /// `TransitionMounting`, `TransitionProgress`.
    transition: bool,
    progress: f32,
    /// A side mount's drop of the point (`CurrentMountingPointVerticalOffset`).
    drop: f32,
    /// `MountedSmoothedTilt`, `MountedSmoothedTiltForCamera`.
    mounted_tilt: f32,
    camera_tilt: f32,
    /// What the body's `Tilt` was last set to by the mount (only a side
    /// mount sets it: `PlayerAnimatorSetTilt`).
    body_tilt: Option<f32>,
}

/// The rig's origin from the ribcage `origin` in the rig's frame `turn`, on
/// a mount at `point` (`MountingRotationFunction`): the place that puts
/// `pivot`, turned by `before`, on the point, by the transition's share.
/// The share is held to 0..1 there (`Vector3.Lerp`): the approach's last
/// frame takes it past 1 (`TransitionProgress`, 1.01 recorded at 120 frames
/// a second, 1.0833 at 60), and the exit's below 0, but the rig stops on its
/// place (recorded: the M700's on its bipod, `local\traces\m700-bipod`).
pub fn mounted_apart(point: [f32; 3], origin: [f32; 3], turn: [f32; 4], before: [f32; 4], pivot: [f32; 3], progress: f32) -> [f32; 3] {
    let reach = quat_rotate(before, pivot);
    let on_point: [f32; 3] = std::array::from_fn(|k| point[k] - reach[k] - origin[k]);
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    let share = progress.clamp(0.0, 1.0);
    quat_rotate(back, on_point).map(|v| v * share)
}

/// What a host draws a mount by (`MountingRotationFunction`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountView {
    /// The point the pivot is put on, lowered by a side mount's drop.
    pub point: [f32; 3],
    pub side: Side,
    /// How far the weapon is moved from the ribcage to the mount, 0 to 1
    /// (`TransitionProgress`; it can pass 1 at the approach's end).
    pub progress: f32,
    /// Whether the pivot is the bipod's.
    pub on_bipod: bool,
    /// The exit is under way.
    pub leaving: bool,
    /// The approach or the exit is under way (`TransitionMounting`).
    pub moving: bool,
}

impl Movement {
}

fn squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `ClampAngle360Sensitive`'s first steps: a yaw within a turn either way,
/// and the same yaw the other way round.
fn mounting_both_ways(yaw: f32) -> (f32, f32) {
    let mut a = yaw;
    if a < -360.0 {
        a += 360.0;
    }
    if a > 360.0 {
        a -= 360.0;
    }
    (a, if a > 0.0 { a - 360.0 } else { a + 360.0 })
}
