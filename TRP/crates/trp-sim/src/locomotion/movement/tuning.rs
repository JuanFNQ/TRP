//! The movement's constants and the move, jump, step and vault data read from the install.

use super::*;

pub(super) const MAX_TILT: f32 = 5.0;

/// Lean units per second at a step multiplier of 1.
pub(super) const TILT_STEP: f32 = 25.0;

/// What a frame of the slow lean's axis, pushed all the way, adds to the
/// lean asked for (`Player.SlowLean`).
pub(super) const SLOW_LEAN_STEP: f32 = 0.1;

/// How quickly the body's height catches up with the feet, in seconds,
/// once the gap between them has stopped opening (body-animator spec, "A
/// lag in the body's height").
pub(super) const HEIGHT_SMOOTH_TIME: f32 = 0.066;

/// Distance to the ground within which the character counts as standing on
/// it: to begin with, and as the game sets it on entering the walk or the
/// crawl, the sprint, and the jump. Other states leave it as it was.
/// Defaults written in the game's code (movement spec, "Gravity").
pub const GROUND_CHECK: f32 = 0.07;

pub const GROUND_CHECK_MOVING: f32 = 0.08;

pub const GROUND_CHECK_SPRINT: f32 = 0.15;

pub const GROUND_CHECK_JUMP: f32 = 0.03;

/// Free fall shorter than this does not yet bleed off horizontal motion.
pub(super) const FREEFALL_GRACE: f32 = 0.25;

/// Free fall longer than this puts the sights down (movement spec,
/// "Falling"; written in the game's code).
pub(super) const LONG_FALL: f32 = 0.5;

/// Stance above which a jump is allowed and sprinting can start.
pub(super) const JUMP_POSE_THRESHOLD: f32 = 0.6;

pub(super) const SPRINT_POSE_THRESHOLD: f32 = 0.9;

/// Input smaller than this on every axis is no input.
pub(super) const MIN_INPUT: f32 = 1e-5;

/// The cap on the speed setting while aiming, before any skill bonus.
pub(super) const AIMING_SPEED_LIMIT: f32 = 0.33;

/// The fastest the character can actually be moved in each kind of state,
/// in metres per second, untrained and at the top of the Strength skill
/// ("The hard cap on speed" in the movement spec).
pub(super) const STANDING_SPEED_CAP: [f32; 2] = [0.5, 1.5];

pub(super) const WALKING_SPEED_CAP: [f32; 2] = [3.6, 4.6];

pub(super) const SPRINTING_SPEED_CAP: [f32; 2] = [6.6, 7.9];

pub(super) const CRAWLING_SPEED_CAP: [f32; 2] = [1.2, 1.5];

/// How far the view may pitch, up then down, upright and lying down; how
/// far it may turn from the body lying down before the body follows, and
/// how far at all; and how fast the pitch limits move to a new range
/// ("Prone" in the movement spec).
pub(super) const UPRIGHT_PITCH_RANGE: [f32; 2] = [-90.0, 90.0];

/// The most degrees a frame may turn the view in a state that sets no
/// limit of its own: the default written in the game's state script
/// ("Looking" in the movement spec). The character starts standing, which
/// is such a state.
pub(super) const FREE_TURN_LIMIT: f32 = 99.0;

pub(super) const PRONE_PITCH_RANGE: [f32; 2] = [-16.0, 25.0];

/// The same rolled onto one side, and how fast the body's yaw is then eased
/// to the bearing the view had when the roll began, per second.
pub(super) const ROLL_PITCH_RANGE: [f32; 2] = [-16.0, 2.0];

pub(super) const ROLL_YAW_SPEED: f32 = 5.0;

/// Lying still, a roll asked for waits until the smoothed lean is under this.
pub(super) const LEAN_STRAIGHT_FOR_ROLL: f32 = 0.1;

pub(super) const PRONE_TRUNK_LIMIT: f32 = 25.0;

pub(super) const PRONE_YAW_REACH: f32 = 35.0;

pub(super) const PITCH_LIMIT_SPEED: f32 = 5.0;

/// The engine reports an angle as negative only down to this, in degrees
/// (0.0001 rad); below it the angle comes back as nearly a full turn.
pub(super) const ENGINE_ANGLE_ALLOWANCE: f32 = 0.005_729_578;

#[derive(Clone, Default)]
pub(super) struct MoveData {
    /// Inertia-smoothed velocity command, world axes. Its length is the speed parameter.
    pub(super) speed: [f32; 2],
    pub(super) speed_velocity: [f32; 2],
    pub(super) inertia_direction: [f32; 2],
    pub(super) direction_velocity: [f32; 2],
    pub(super) transition_coef: f32,
    pub(super) lower_range: f32,
    pub(super) previous_target: f32,
    pub(super) time_without_input: f32,
    /// Remaining pause after the input reversed.
    pub(super) reversal_pause: f32,
    pub(super) next_direction: [f32; 2],
    pub(super) prev_direction: [f32; 2],
    pub(super) last_direction_input: [f32; 2],
    pub(super) last_nonzero_direction: [f32; 2],
    pub(super) smoothed_yaw: f32,
    pub(super) sprint_postponed: bool,
    /// What the wanted speed is multiplied by and the direction's
    /// smoothing time divided by: 1 on foot, the game's figures crawling.
    pub(super) speed_acceleration: f32,
    pub(super) direction_acceleration: f32,
    /// The game's time "without a call to move", which nothing ever sets
    /// back: once it has passed `MAX_WITHOUT_MOVE_CALL` every update ends
    /// by dropping the input it was handed (movement spec, "A frame with
    /// no input"). It is the state object's and outlasts the state.
    pub(super) unheard_time: f32,
}

/// How long the walking state runs before its updates begin to drop their
/// input at their end (`MovePlayerState.MAX_WITHOUT_MOVE_CALL`).
pub(super) const MAX_WITHOUT_MOVE_CALL: f32 = 0.2;

impl MoveData {
    /// The state as the game first makes it, but for the time without a
    /// call: a character met any later than its first fifth of a second
    /// of walking has that behind it.
    pub(super) fn fresh() -> Self {
        Self {
            transition_coef: 1.0,
            speed_acceleration: 1.0,
            direction_acceleration: 1.0,
            unheard_time: 2.0 * MAX_WITHOUT_MOVE_CALL,
            ..Self::default()
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct JumpData {
    pub(super) time: f32,
    pub(super) airborne: bool,
    pub(super) move_dir: [f32; 2],
    pub(super) direction: [f32; 3],
    pub(super) initial_speed: f32,
    pub(super) speed: f32,
    pub(super) lift: f32,
    pub(super) lift_delay: f32,
    pub(super) lift_velocity: f32,
    pub(super) from_sprint: bool,
    pub(super) potential_height: f32,
    pub(super) time_descending: f32,
    pub(super) last_height: Option<f32>,
    pub(super) frames_without_motion: u32,
    /// The jump was stopped short and only waits for the animator.
    pub(super) halted: bool,
    /// How far the player's clock was behind the frames' when the jump
    /// began (`Movement::clock_lag`).
    pub(super) lag_at_start: f32,
    /// How fast the body turned after the view in the frame just run, in
    /// degrees a second of the player's clock; nothing in a frame in which
    /// the jump's own update did not run.
    pub(super) turn_rate: f32,
}

/// What the sidestep's state holds: which part of the step the animator is
/// in (`part`), how long that part has run by the player's clock, and the
/// value the step runs to (1 right, -1 left). The last two fields are the
/// state's own playback speed for the step and whether it last worked a
/// way out, which lowers where that speed rests.
#[derive(Clone, Default)]
pub(super) struct StepData {
    pub(super) part: i32,
    pub(super) progress: f32,
    pub(super) far: f32,
    pub(super) speed: f32,
    pub(super) stepped_out: bool,
}

impl StepData {
    /// One frame of the playback speed of a step aside or a roll, which
    /// the game keeps per state (`IdlePlayerInertia`): in the hold it goes
    /// to where it rests; on a way in or out it gains what the inertia
    /// allows, never under where it rests nor over its top. Where it rests
    /// is lower once the state has worked a way out.
    pub(super) fn play(&mut self, s: &MovementSettings, dt: f32) -> f32 {
        let back = |stepped_out: bool| if stepped_out { s.inertia_backward_coef } else { 1.0 };
        if self.part == part::HOLD {
            self.speed = s.sideback_speed * back(self.stepped_out);
        } else {
            self.stepped_out = self.part == part::OUT;
            let gained = self.speed + s.sideback_acceleration * back(self.stepped_out) * dt;
            // The engine's clamp, lower bound first.
            let low = s.sideback_speed * back(self.stepped_out);
            self.speed = if gained < low { low } else { gained.min(s.sideback_max_speed) };
        }
        self.speed
    }
}

/// A vault or a climb under way.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct VaultData {
    /// The obstacle's height, as the animator was given it.
    pub(super) height: f32,
    /// The move came out of a sprint.
    pub(super) from_sprint: bool,
    /// The obstacle is high enough to take a hand off the weapon: the
    /// view is then the move's own and the trigger is dead.
    pub(super) hands: bool,
    /// The game's capsule for the move: its height, and how far over the
    /// feet its centre is.
    pub(super) capsule: [f32; 2],
}
