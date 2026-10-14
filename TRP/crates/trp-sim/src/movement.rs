//! Movement, as specified in `docs/spec/movement.md`.
//!
//! The simulation works in the game's own space: x right, y up, z forward,
//! angles in degrees, yaw positive to the right, pitch positive downward.
//! The game moves the player once per drawn frame, by that frame's own
//! length, and several of its rules apply a fixed amount per frame rather
//! than per second: `Movement::frame` is that, and is what the recordings
//! were compared on. `Movement::step` is the same on steps of the host's
//! choosing.

use std::collections::VecDeque;

use crate::body::{movement_state, part, ArmPose, Body, BodyInput, Entered};
use crate::hands::{Carried, Held};
use crate::head_look::{HeadLook, HeadLookInput};
use crate::interactions::{AnimatedInteractions, Interaction, Show};
use crate::math::{
    body_to_view, delta_angle, dot, inverse_lerp, length, lerp, lerp_angle, lerp_turn, normalize, quat_axis, quat_euler, quat_look,
    quat_mul, quat_rotate, quat_to_euler, smooth_damp,
};
use crate::settings::MovementSettings;
use crate::stamina::Stamina;
use crate::vaulting::{Move, Obstacle, Standing, Vaulting};

const MAX_TILT: f32 = 5.0;
/// Lean units per second at a step multiplier of 1.
const TILT_STEP: f32 = 25.0;
/// What a frame of the slow lean's axis, pushed all the way, adds to the
/// lean asked for (`Player.SlowLean`).
const SLOW_LEAN_STEP: f32 = 0.1;
/// How quickly the body's height catches up with the feet, in seconds,
/// once the gap between them has stopped opening (body-animator spec, "A
/// lag in the body's height").
const HEIGHT_SMOOTH_TIME: f32 = 0.066;
/// Distance to the ground within which the character counts as standing on
/// it: to begin with, and as the game sets it on entering the walk or the
/// crawl, the sprint, and the jump. Other states leave it as it was.
/// Defaults written in the game's code (movement spec, "Gravity").
pub const GROUND_CHECK: f32 = 0.07;
pub const GROUND_CHECK_MOVING: f32 = 0.08;
pub const GROUND_CHECK_SPRINT: f32 = 0.15;
pub const GROUND_CHECK_JUMP: f32 = 0.03;
/// Free fall shorter than this does not yet bleed off horizontal motion.
const FREEFALL_GRACE: f32 = 0.25;
/// Free fall longer than this puts the sights down (movement spec,
/// "Falling"; written in the game's code).
const LONG_FALL: f32 = 0.5;
/// Stance above which a jump is allowed and sprinting can start.
const JUMP_POSE_THRESHOLD: f32 = 0.6;
const SPRINT_POSE_THRESHOLD: f32 = 0.9;
/// Input smaller than this on every axis is no input.
const MIN_INPUT: f32 = 1e-5;
/// The cap on the speed setting while aiming, before any skill bonus.
const AIMING_SPEED_LIMIT: f32 = 0.33;
/// The fastest the character can actually be moved in each kind of state,
/// in metres per second, untrained and at the top of the Strength skill
/// ("The hard cap on speed" in the movement spec).
const STANDING_SPEED_CAP: [f32; 2] = [0.5, 1.5];
const WALKING_SPEED_CAP: [f32; 2] = [3.6, 4.6];
const SPRINTING_SPEED_CAP: [f32; 2] = [6.6, 7.9];
const CRAWLING_SPEED_CAP: [f32; 2] = [1.2, 1.5];
/// How far the view may pitch, up then down, upright and lying down; how
/// far it may turn from the body lying down before the body follows, and
/// how far at all; and how fast the pitch limits move to a new range
/// ("Prone" in the movement spec).
const UPRIGHT_PITCH_RANGE: [f32; 2] = [-90.0, 90.0];
/// The most degrees a frame may turn the view in a state that sets no
/// limit of its own: the default written in the game's state script
/// ("Looking" in the movement spec). The character starts standing, which
/// is such a state.
const FREE_TURN_LIMIT: f32 = 99.0;
const PRONE_PITCH_RANGE: [f32; 2] = [-16.0, 25.0];
/// The same rolled onto one side, and how fast the body's yaw is then eased
/// to the bearing the view had when the roll began, per second.
const ROLL_PITCH_RANGE: [f32; 2] = [-16.0, 2.0];
const ROLL_YAW_SPEED: f32 = 5.0;
/// Lying still, a roll asked for waits until the smoothed lean is under this.
const LEAN_STRAIGHT_FOR_ROLL: f32 = 0.1;
const PRONE_TRUNK_LIMIT: f32 = 25.0;
const PRONE_YAW_REACH: f32 = 35.0;
const PITCH_LIMIT_SPEED: f32 = 5.0;
/// The engine reports an angle as negative only down to this, in degrees
/// (0.0001 rad); below it the angle comes back as nearly a full turn.
const ENGINE_ANGLE_ALLOWANCE: f32 = 0.005_729_578;

/// The movement states. Which one is current follows the body's animator:
/// each of its states names one of these, and entering that animator state
/// enters this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Move,
    Sprint,
    Jump,
    /// Coming to a stop out of a sprint, through the animator's own clip.
    SprintStop,
    /// Landing from a jump made while moving.
    Landing,
    /// Going down to lie prone.
    ToProne,
    /// Lying still.
    ProneIdle,
    /// Crawling.
    ProneMove,
    /// Getting up from prone.
    FromProne,
    /// Stepped out to one side from standing, on the way there or back.
    Sidestep,
    /// Rolled onto one side lying down, on the way there or back.
    Roll,
    /// Over a low obstacle and down the other side (the game's
    /// `ClimbOver`; vaulting spec).
    Vault,
    /// Up onto an obstacle (the game's `ClimbUp`).
    Climb,
    /// Coming down behind a vault where the ground is far below.
    VaultFall,
    /// Landing out of a vault.
    VaultLanding,
    /// Set before a door to kick it (the game's `Approach`; interactions
    /// spec, "A kick").
    Approach,
    /// Kicking a door (the game's `BreachDoor`).
    Breach,
}

impl State {
    const ALL: [State; 18] = [
        State::Approach,
        State::Breach,
        State::Sidestep,
        State::Roll,
        State::Vault,
        State::Climb,
        State::VaultFall,
        State::VaultLanding,
        State::Idle,
        State::Move,
        State::Sprint,
        State::Jump,
        State::SprintStop,
        State::Landing,
        State::ToProne,
        State::ProneIdle,
        State::ProneMove,
        State::FromProne,
    ];

    /// The game's number for the state, as the body's animator names it.
    fn number(self) -> i32 {
        match self {
            State::Idle => movement_state::IDLE,
            State::Move => movement_state::RUN,
            State::Sprint => movement_state::SPRINT,
            State::Jump => movement_state::JUMP,
            State::SprintStop => movement_state::TRANSITION,
            State::Landing => movement_state::JUMP_LANDING,
            State::ToProne => movement_state::TO_PRONE,
            State::ProneIdle => movement_state::PRONE_IDLE,
            State::ProneMove => movement_state::PRONE_MOVE,
            State::FromProne => movement_state::PRONE_TO_STAND,
            State::Sidestep => movement_state::SIDESTEP,
            State::Roll => movement_state::ROLL,
            State::Vault => movement_state::CLIMB_OVER,
            State::Climb => movement_state::CLIMB_UP,
            State::VaultFall => movement_state::VAULTING_FALL_DOWN,
            State::VaultLanding => movement_state::VAULTING_LANDING,
            State::Approach => movement_state::APPROACH,
            State::Breach => movement_state::BREACH_DOOR,
        }
    }

    fn from_number(number: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.number() == number)
    }

    /// The states the game builds on its standing state: they take
    /// movement keys and sprint commands as standing does.
    fn stands(self) -> bool {
        matches!(
            self,
            State::Idle | State::ToProne | State::ProneIdle | State::FromProne | State::Sidestep | State::Roll
        )
    }
}

/// The two lean keys as an input has them, the left and the right.
fn lean_keys(input: &Input) -> [bool; 2] {
    [input.lean_keys[0] || input.lean < 0.0, input.lean_keys[1] || input.lean > 0.0]
}

/// What the player asks for during one step.
#[derive(Debug, Clone, Copy, Default)]
pub struct Input {
    /// x right, y forward, each -1..1.
    pub move_dir: [f32; 2],
    /// Yaw and pitch change in degrees, sensitivity already applied.
    pub look_delta: [f32; 2],
    /// Sprint key held.
    pub sprint: bool,
    /// Jump pressed this step.
    pub jump: bool,
    /// Lie down, or get up, pressed this step.
    pub prone: bool,
    /// The crouch key pressed this step: between crouched and standing,
    /// or back to the stance last chosen if something else changed it.
    pub crouch: bool,
    /// Stance change this step; one wheel notch is `delta_level`.
    pub pose_delta: f32,
    /// Speed change this step; one wheel notch is `delta_speed`.
    pub speed_delta: f32,
    /// A lean key held: -1 the left one, 0 neither, 1 the right one. The
    /// keys are heard as presses and releases (movement spec, "Lean"), so
    /// a change of this is one key let go and the other pressed.
    pub lean: f32,
    /// Both lean keys as they stand, the left and the right, for a host
    /// that can have the two down at once; `lean` counts as its side's
    /// key being down as well.
    pub lean_keys: [bool; 2],
    /// The step-aside key held: -1 left, 0 neither, 1 right.
    pub step: i32,
    /// Aiming down the sights, which caps the walking speed.
    pub aiming: bool,
    /// The weapon's reload is in hand (`Firearm::reloading`). For that
    /// long a lying body is held still.
    pub reloading: bool,
    /// The hands are busy with the weapon (`Firearm::hands_busy`). In a
    /// sprint the body brings the weapon up into view for that long.
    pub hands_busy: bool,
    /// The animator of the thing in hand is in its resting state
    /// (`Firearm::animator_idling`): only then is a gesture of the hands
    /// on the world begun (interactions spec, "What starts one").
    pub hands_idle: bool,
    /// That animator is in its states for an open inventory
    /// (`Firearm::in_inventory`), which is when the body takes its own
    /// pose for one.
    pub hands_in_inventory: bool,
    /// The hold-breath key pressed this step: it holds the breath, or lets
    /// a held one go (this install binds it as a press; the game also has
    /// a command for the key coming up, for a binding that holds). Only a
    /// character standing still (or lying still, or stepped aside) with
    /// the sights up holds one (weapon-handling spec, "Breathing").
    pub hold_breath: bool,
    /// The vault key held (in the game's default binding the jump key,
    /// which then jumps when let go: vaulting spec, "Two moves, one
    /// key"). Every frame it is held tries a vault or a climb.
    pub vault: bool,
    /// The game's own commands for the sprint, the lean, the step aside,
    /// the breath and the crouch, for a host that hears its keys as the
    /// game's input manager gives them out (settings spec, "What the
    /// bindings do"; `trp_sim::bindings`): each is carried out once on
    /// this frame. The keys above are held keys, whose going down and
    /// coming up stand for the same commands; a binding that is pressed
    /// and not held gives the first of each pair here and never the
    /// second, which a held key cannot say. The lean's and the step's go
    /// left then right.
    pub sprint_toggle: bool,
    pub sprint_end: bool,
    pub lean_toggle: [bool; 2],
    pub lean_end: [bool; 2],
    pub step_toggle: [bool; 2],
    pub step_end: [bool; 2],
    /// The hold-breath key coming up, where it is bound to be held: a
    /// held breath is let go.
    pub breath_end: bool,
    /// The crouch key coming up, where it is bound to be held.
    pub restore_pose: bool,
    /// The walk key's two commands (`Player.ToggleMinSpeed`,
    /// `RestoreSpeed`): the speed put to its least, or back to what it
    /// was; and back, where the key is bound to be held and comes up.
    pub walk_toggle: bool,
    pub walk_end: bool,
    /// The slow lean's axis as its keys push it, -1 the left to 1 the
    /// right (`LeanX`; movement spec, "The slow lean"): a tenth of a
    /// degree of lean a frame for as long as it is pushed.
    pub slow_lean: f32,
    /// The weapon's logic has something in hand: a reload, a shot, a look
    /// at the weapon (`Firearm::occupied`). Only an obstacle low enough to
    /// leave the hands on the weapon is then vaulted or climbed.
    pub weapon_occupied: bool,
    /// Take `look_delta` as it is, without the movement state's own
    /// slowing and limit on the view's turning: for a host that follows a
    /// view recorded from the game, which has been through them already.
    pub free_look: bool,
    /// A screen of the host's has the keys and the mouse, as the game's
    /// inventory screen and its menus have (inventory spec, "What an open
    /// screen does to the keys"): the character is handed no movement, no
    /// turn of the view and none of its keys, and a key pressed or let go
    /// meanwhile is never heard of. What the weapon tells (`aiming` and
    /// its neighbours) and `inventory` are taken all the same.
    pub screen: bool,
    /// The character's inventory is open (`Player.SetInventoryOpened`).
    /// The body's animator is told as it is of a reload, and a change
    /// either way switches a held vault key off.
    pub inventory: bool,
    /// The head's free look (`docs/spec/free-look.md`): what the mouse's
    /// travel gave the look's two axes on this frame, in degrees (across,
    /// to the right positive, and down), and whether the look's key came
    /// up (`ResetLookDirection`). The mouse gives the look what it does
    /// not give the turn (`bindings::Preset::mouse`).
    pub head_look: [f32; 2],
    pub head_reset: bool,
    /// The camera's pitch below level as last drawn, in degrees: a
    /// sprinting character whose camera is far down looks no further
    /// down. A host with no camera leaves it at nothing.
    pub camera_pitch: f32,
}

/// The upright space the character's collision takes up over its feet, in
/// metres: from `underside` to `top`. Standing, crouched and lying the
/// underside is at the feet. In a vault or a climb the game shrinks its
/// capsule from below to get it over the obstacle (vaulting spec, "The
/// capsule"), and a host's own collision has to let the character past
/// whatever is lower than the underside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub underside: f32,
    pub top: f32,
    /// Whether what the character is in may push it upward out of itself,
    /// which is how the game's own controller sets it on the ground and
    /// over an edge. In a vault or a climb the game switches that off
    /// (movement spec, "The character's own collision"): the character is
    /// then pushed out sideways only and rises by the move's motion alone.
    pub lift: bool,
}

/// The ground the character stands on, and whatever else of the host's
/// world the controller has to feel.
pub trait World {
    fn ground_height(&self, x: f32, z: f32) -> f32;

    /// How high the ground holds the feet of a character whose feet are at
    /// `at`. Left alone, the ground's height under its axis. A host whose
    /// character has a foot with some width to it (the game's capsule has
    /// a rounded one) answers with where that foot rests: over an edge it
    /// overhangs, on the edge. The ground check and the feet go by this.
    fn footing(&self, at: [f32; 3]) -> f32 {
        self.ground_height(at[0], at[2])
    }

    /// Which way the ground faces there. Level unless the host says
    /// otherwise; a lying body is laid to it.
    fn ground_normal(&self, _x: f32, _z: f32) -> [f32; 3] {
        [0.0, 1.0, 0.0]
    }

    /// How far along the line from `from` to `to` the first solid thing
    /// lies, in metres, if anything does: the game's line cast, which the
    /// weapon feels for obstacles with (`Movement::weapon_overlap`). A host
    /// with walls gives its own. Left alone, only the ground counts: where
    /// the line first goes under it, and nothing for a line that starts
    /// under it (the game's ground is hit from above only).
    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let above = |along: f32| {
            let t = if length > 0.0 { along / length } else { 0.0 };
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * t);
            at[1] - self.ground_height(at[0], at[2])
        };
        if length <= 0.0 || above(0.0) < 0.0 {
            return None;
        }
        // Walked in short steps, then closed in on where it goes under.
        let steps = (length / LINE_STEP).ceil().max(1.0) as usize;
        let under = (1..=steps).map(|step| length * step as f32 / steps as f32).find(|along| above(*along) < 0.0)?;
        let (mut over, mut under) = (under - length / steps as f32, under);
        for _ in 0..16 {
            let middle = (over + under) / 2.0;
            if above(middle) < 0.0 { under = middle } else { over = middle }
        }
        Some(under)
    }

    /// Whether what is solid at a point a line hit is the terrain itself
    /// and not something standing on it: the game never vaults or climbs
    /// its terrain (vaulting spec, "The obstacle"). Left alone everything
    /// is, as for a world that is only ground; a host with walls and
    /// crates says no for those.
    fn terrain(&self, _at: [f32; 3]) -> bool {
        true
    }

    /// Where a move from `from` toward `to` leaves the character's feet,
    /// once whatever stands in the way has had its say. The controller
    /// has no collision of its own beyond the ground: a host with walls
    /// answers with the place its own collision allows, for a character
    /// that takes up `capsule` over its feet. `to` is where the motion alone would put the
    /// feet, which may be under the ground there: the ground lifts them
    /// afterwards (`ground_height` at the place answered), so a rise too
    /// high to step onto has to be refused here. Nothing stands in the way
    /// unless it says so.
    fn obstructed(&self, _from: [f32; 3], to: [f32; 3], _capsule: Capsule) -> [f32; 3] {
        to
    }

    /// Whether the capsule, with its feet where a frame's motion put them
    /// (`at`), is in something that lifts the hard cap on speed for the
    /// move: a door's leaf (`SimpleCharacterController`'s colliders with no
    /// speed limit, which the game fills with every door's). A door that
    /// swings into the character so pushes it out of its way at whatever
    /// pace that takes (interactions spec, "A door: which gesture, and how
    /// it swings"). Nothing does unless the host says so.
    fn lifts_cap(&self, _at: [f32; 3], _capsule: Capsule) -> bool {
        false
    }

    /// Whether `obstructed` also sets the character on the ground it has
    /// moved onto or into, as the game's own controller does by pushing
    /// its capsule up out of whatever it is in (movement spec, "The
    /// character's own collision"). Left alone it does not, and the
    /// simulation puts the feet up onto `footing` itself after every move:
    /// that lifts a character onto anything its axis is over, a wall it
    /// has just vaulted and not yet left as well.
    fn sets_feet(&self) -> bool {
        false
    }

    /// Whether a character whose feet are at `at` is held up by something
    /// it touches other than what lies under its axis: the edge of a
    /// ledge its foot overhangs. The game's controller counts a character
    /// it has had to push upward out of something as on the ground
    /// (movement spec, "The character's own collision"), which is what
    /// keeps one standing that has climbed to a top's edge and no
    /// further. Nothing does unless the host says so.
    fn held(&self, _at: [f32; 3], _capsule: Capsule) -> bool {
        false
    }

    /// How far a box has to move to be clear of everything solid: for each
    /// thing it is in, the shortest move that takes it out of that thing,
    /// lengthened by `margin`, all added up. This is the game's push of
    /// the camera's box (`Movement::camera_push`). `centre` is the box's
    /// middle, `half` its half-width, half-height and half-depth along its
    /// own axes, and `turn` how those axes are turned. A host with walls
    /// gives its own. Left alone, only the ground counts: the box is
    /// lifted along the ground's normal by as far as its lowest corner is
    /// under it.
    fn push_out(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4], margin: f32) -> [f32; 3] {
        let mut deepest: Option<(f32, [f32; 3])> = None;
        for corner in 0..8 {
            let reach = quat_rotate(turn, std::array::from_fn(|k| if corner >> k & 1 == 0 { half[k] } else { -half[k] }));
            let at: [f32; 3] = std::array::from_fn(|k| centre[k] + reach[k]);
            let normal = self.ground_normal(at[0], at[2]);
            // Under the ground by this much, measured square to it.
            let under = (self.ground_height(at[0], at[2]) - at[1]) * normal[1];
            if under > 0.0 && deepest.is_none_or(|(most, _)| under > most) {
                deepest = Some((under, normal));
            }
        }
        deepest.map_or([0.0; 3], |(under, normal)| normal.map(|n| n * (under + margin)))
    }
}

/// How finely the ground is felt for along a line, in metres.
const LINE_STEP: f32 = 0.05;

/// The clock the game times the player by: a stopwatch that is read once
/// a frame in whole milliseconds ("The player's clock" in the movement
/// spec). At 120 frames a second it gives 8, 8 and 9 ms in turn.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stopwatch {
    seconds: f64,
    read: u64,
}

impl Stopwatch {
    /// A frame of `dt` seconds has passed: the time since the last
    /// reading, as the stopwatch shows it.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.seconds += f64::from(dt.max(0.0));
        let now = (self.seconds * 1000.0) as u64;
        let passed = now - self.read;
        self.read = now;
        passed as f32 / 1000.0
    }
}

/// Level ground at height zero.
pub struct Flat;

impl World for Flat {
    fn ground_height(&self, _x: f32, _z: f32) -> f32 {
        0.0
    }
}

/// Unity's `Mathf.Sign`: zero counts as positive.
fn sign(v: f32) -> f32 {
    if v >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

fn is_zero(v: [f32; 2]) -> bool {
    v[0] == 0.0 && v[1] == 0.0
}

#[derive(Default)]
struct MoveData {
    /// Inertia-smoothed velocity command, world axes. Its length is the speed parameter.
    speed: [f32; 2],
    speed_velocity: [f32; 2],
    inertia_direction: [f32; 2],
    direction_velocity: [f32; 2],
    transition_coef: f32,
    lower_range: f32,
    previous_target: f32,
    time_without_input: f32,
    /// Remaining pause after the input reversed.
    reversal_pause: f32,
    next_direction: [f32; 2],
    prev_direction: [f32; 2],
    last_direction_input: [f32; 2],
    last_nonzero_direction: [f32; 2],
    smoothed_yaw: f32,
    sprint_postponed: bool,
    /// What the wanted speed is multiplied by and the direction's
    /// smoothing time divided by: 1 on foot, the game's figures crawling.
    speed_acceleration: f32,
    direction_acceleration: f32,
    /// The game's time "without a call to move", which nothing ever sets
    /// back: once it has passed `MAX_WITHOUT_MOVE_CALL` every update ends
    /// by dropping the input it was handed (movement spec, "A frame with
    /// no input"). It is the state object's and outlasts the state.
    unheard_time: f32,
}

/// How long the walking state runs before its updates begin to drop their
/// input at their end (`MovePlayerState.MAX_WITHOUT_MOVE_CALL`).
const MAX_WITHOUT_MOVE_CALL: f32 = 0.2;

impl MoveData {
    /// The state as the game first makes it, but for the time without a
    /// call: a character met any later than its first fifth of a second
    /// of walking has that behind it.
    fn fresh() -> Self {
        Self {
            transition_coef: 1.0,
            speed_acceleration: 1.0,
            direction_acceleration: 1.0,
            unheard_time: 2.0 * MAX_WITHOUT_MOVE_CALL,
            ..Self::default()
        }
    }
}

#[derive(Default)]
struct JumpData {
    time: f32,
    airborne: bool,
    move_dir: [f32; 2],
    direction: [f32; 3],
    initial_speed: f32,
    speed: f32,
    lift: f32,
    lift_delay: f32,
    lift_velocity: f32,
    from_sprint: bool,
    potential_height: f32,
    time_descending: f32,
    last_height: Option<f32>,
    frames_without_motion: u32,
    /// The jump was stopped short and only waits for the animator.
    halted: bool,
    /// How far the player's clock was behind the frames' when the jump
    /// began (`Movement::clock_lag`).
    lag_at_start: f32,
    /// How fast the body turned after the view in the frame just run, in
    /// degrees a second of the player's clock; nothing in a frame in which
    /// the jump's own update did not run.
    turn_rate: f32,
}

/// What the sidestep's state holds: which part of the step the animator is
/// in (`part`), how long that part has run by the player's clock, and the
/// value the step runs to (1 right, -1 left). The last two fields are the
/// state's own playback speed for the step and whether it last worked a
/// way out, which lowers where that speed rests.
#[derive(Default)]
struct StepData {
    part: i32,
    progress: f32,
    far: f32,
    speed: f32,
    stepped_out: bool,
}

impl StepData {
    /// One frame of the playback speed of a step aside or a roll, which
    /// the game keeps per state (`IdlePlayerInertia`): in the hold it goes
    /// to where it rests; on a way in or out it gains what the inertia
    /// allows, never under where it rests nor over its top. Where it rests
    /// is lower once the state has worked a way out.
    fn play(&mut self, s: &MovementSettings, dt: f32) -> f32 {
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
struct VaultData {
    /// The obstacle's height, as the animator was given it.
    height: f32,
    /// The move came out of a sprint.
    from_sprint: bool,
    /// The obstacle is high enough to take a hand off the weapon: the
    /// view is then the move's own and the trigger is dead.
    hands: bool,
    /// The game's capsule for the move: its height, and how far over the
    /// feet its centre is.
    capsule: [f32; 2],
}

pub struct Movement {
    pub settings: MovementSettings,

    pub position: [f32; 3],
    pub velocity: [f32; 3],
    /// Where the player looks.
    pub yaw: f32,
    pub pitch: f32,
    /// Where the body faces. It follows the look direction with a lag.
    pub body_yaw: f32,

    pub state: State,
    previous_state: State,

    /// The speed the player selected with the wheel.
    pub character_speed: f32,
    /// The wheel's setting as a share of the top walking speed. The
    /// selected speed goes back to it at every change of state.
    relative_speed: f32,
    /// The cap on actual speed for the current state, in metres per second.
    speed_cap: f32,
    /// The cap on the selected speed left by a landing or a fall, and the
    /// time on the frame clock at which it ends.
    landing_limit: Option<(f32, f32)>,
    /// That cap as the game has worked it in: a change to the one above
    /// shows from the next frame's upkeep on.
    landing_limit_in_effect: Option<f32>,
    limits_changed: bool,
    /// The engine's time added up over the frames so far.
    frame_clock: f32,
    /// The highest the character has been since it last stood on the
    /// ground, and whether it stood on it a frame ago.
    fall_from: f32,
    was_grounded: bool,
    /// The animator's speed parameter.
    pub smoothed_speed: f32,
    /// The animator's direction parameters, in the body's axes.
    pub movement_direction: [f32; 2],
    discrete_direction: f32,

    pub pose_level: f32,
    pub smoothed_pose_level: f32,
    /// The stance the player last chose, which getting up returns to.
    pub pose_memo: f32,
    /// Lying down, or on the way down.
    pub prone: bool,
    /// The weapon's reload is in hand, and its hands busy, as the host
    /// last said.
    reloading: bool,
    hands_busy: bool,
    /// The inventory is open, as the host last said; and the flag the
    /// body's animator has for a reload, which the game sets from both: at
    /// a reload's start and end and at the inventory's opening and
    /// shutting, whichever came last (`MovementState.OnReload`,
    /// `OnInventory`).
    inventory: bool,
    prone_reload: bool,
    /// What the movement state does to the view's turning, as the
    /// animator's state last entered has it: a factor on each frame's
    /// turn, and the most degrees one frame may turn (none at all if 0).
    pub turn_factor: f32,
    pub turn_limit: f32,
    /// Whether the animator's state last entered lets the player look at a
    /// thing to work it (`CanInteract` on the state's script).
    can_interact: bool,
    /// How far the view may turn from the body before the body follows.
    trunk_limit: f32,
    /// The view's pitch range, and the range it is moving to.
    pitch_limit: [f32; 2],
    pitch_target: [f32; 2],
    /// Where the view's yaw is held: a bearing and how far either side of
    /// it. Lying still and stepped aside it is; otherwise it turns freely.
    yaw_limit: Option<(f32, f32)>,
    /// The step aside asked for (1 right, -1 left, 0 none), the key as
    /// last seen, how far the body is stepped (-1 to 1), and the playback
    /// speed of the step as the animator has it.
    pub step: i32,
    step_key: i32,
    /// The lean keys as last noted, the left and the right.
    lean_keys: [bool; 2],
    /// The lean was made slowly on the last frame its axis was looked at
    /// (`_lastSlowLean`): the lean keys then do nothing. And the lean in
    /// hand is a slow one (`LeanType.SlowLean`), which the body follows by
    /// its older rule, until a lean key next leans it.
    last_slow_lean: bool,
    slow_leaning: bool,
    /// The speed the walk key put aside, to go back to (`_speedMemo`).
    speed_memo: f32,
    /// A backpack's drop has been asked of the body and its mark has not
    /// come yet.
    dropping_backpack: bool,
    pub sidestep: f32,
    sideback_speed: f32,
    sidestep_data: StepData,
    /// The roll lying down: its own state's figures; a step asked for lying
    /// still that waits for the lean to straighten; and the bearing the
    /// view had when the roll began, with the body's yaw as it is eased
    /// toward it.
    roll_data: StepData,
    pending_step: i32,
    roll_bearing: f32,
    roll_yaw: f32,
    /// Crawling with the sprint key held winds the speed setting up.
    prone_sprint: bool,
    /// Lying down, the body is laid to the ground: the way it is made to
    /// point and the ground's normal, both eased, in world axes; the
    /// rotation that gives the body within the player's own axes; and the
    /// pitch of that rotation.
    surface_direction: [f32; 3],
    surface_normal: [f32; 3],
    pub body_tilt: [f32; 4],
    surface_pitch: f32,
    pub tilt: f32,
    pub smoothed_tilt: f32,
    tilt_multiplier: f32,
    prev_tilt: f32,

    /// The animator's sprint speed parameter.
    pub sprint_speed: f32,
    pub sprinting: bool,
    sprint_input: bool,
    idle_sprint_held: bool,
    aiming: bool,
    /// For the weapon's logic, until it asks: the sprint has switched on,
    /// and a state that does not allow aiming has been entered.
    was_sprinting: bool,
    sprint_started: bool,
    aim_lowered: bool,
    aim_taken: bool,

    /// The angle from the body round to the view, and the view's pitch, as
    /// the game last took them: inside the last state update.
    hands_to_body: f32,
    aim_angle: f32,
    /// What the character pays for all this, and what running out stops
    /// (stamina spec).
    pub stamina: Stamina,

    /// `body_yaw` to more places than it holds, while the body follows the
    /// view at the limit; taken afresh whenever the two no longer agree.
    body_yaw_fine: f64,

    pub grounded: bool,
    /// How far over the ground still counts as on it. The last state to
    /// set it decides; nothing pulls a character down that is within it.
    pub ground_check: f32,
    pub freefall_time: f32,
    last_blend_velocity: [f32; 3],
    velocity_before_limit: [f32; 3],
    yaw_deltas: VecDeque<f32>,
    /// Movement input as the current state has it: what it was last
    /// handed, or what its own update has since made of that.
    direction: [f32; 2],
    /// Movement input as the character was last handed it, whatever the
    /// state did with it (`Player.InputDirection`). A try at a vault goes
    /// by it.
    input_direction: [f32; 2],
    /// The frames stepped so far, and the one in which the sprinting
    /// state last dropped its input: a second update in that same frame
    /// does not take the dropped input for keys let go.
    frame_count: u64,
    sprint_dropped_in: u64,
    /// The current frame's length by the engine's time.
    frame_dt: f32,
    /// For `frame`: the player's own clock, and frame time it has not yet
    /// seen pass.
    stopwatch: Stopwatch,
    unstepped: f32,
    /// How far the player's clock is behind the frames' own, in seconds:
    /// the frame time passed less the player's time passed. Before this
    /// frame, and after it.
    clock_lag_before: f32,
    clock_lag: f32,
    mv: MoveData,
    /// The crawl is a second walking state in the game, with fields of its
    /// own. It is swapped in for `mv` while it runs.
    mv_prone: MoveData,
    jump: JumpData,

    /// The body's animator. It decides which movement state is current
    /// and supplies the motion.
    pub body: Body,
    /// The head's free look: the camera's own turn, and the neck's and
    /// the head's with it.
    pub head_look: HeadLook,
    /// The flags the movement raises for the animator: the player is
    /// asking to move, the sprint may start, a jump is under way, the
    /// jump has landed.
    inert: bool,
    sprint_flag: bool,
    jump_flag: bool,
    landing_flag: bool,
    /// How far the animator moved the body this step, in world axes.
    animated_motion: [f32; 3],
    /// For the landing state: seconds without input, and whether the
    /// landing has been left to play out without moving.
    landing_idle_time: f32,
    landing_still: bool,
    /// Seconds since the start, the time of the last footstep, which way
    /// the footstep curve last pointed, and a footstep not yet reported.
    clock: f32,
    last_step: f32,
    step_side: f32,
    footstep: Option<f32>,
    /// Vaulting and climbing (vaulting spec): the grid and what it last
    /// measured; how long the key has been held; the two flags a chosen
    /// move raises for the animator, and the one that is up while a move
    /// is under way; the obstacle as the animator was last given it
    /// (height, length, the drop behind, the forward speed); the pace of
    /// the moves' clips; and the move in hand.
    pub vaulting: Vaulting,
    vault_time: f32,
    /// The vault key as last seen, and whether its per-frame try is
    /// switched on. Every change of the key switches it over, and the
    /// inventory opening or shutting switches it off, so after a key held
    /// through that the two no longer go together (vaulting spec, "Two
    /// moves, one key").
    vault_key: bool,
    vault_on: bool,
    do_vault: bool,
    do_climb: bool,
    pub is_vaulting: bool,
    vault_figures: [f32; 4],
    vaulting_speed: f32,
    vault: VaultData,
    /// A measurement for the next try to take in place of the grid's own:
    /// the obstacle, and the forward speed it was measured at. A replay
    /// gives the game's recorded one each frame (`Vaulting::take`); a host
    /// leaves it alone.
    pub vault_given: Option<(Obstacle, f32)>,
    /// The game's lock on moving into what stands in the way (movement
    /// spec, "Stopped by what stands in the way"): whether it is on, the
    /// movement direction it was put on with, how long the character has
    /// been all but still, how fast it really went over the last frame,
    /// and where it was when that was last worked out.
    pub blocked: bool,
    blocked_direction: [f32; 2],
    slow_time: f32,
    real_speed: f32,
    place_before: [f32; 3],
    /// How far the body is held above or below where the feet put it,
    /// while it catches up with a change of height.
    pub body_height_offset: f32,
    height_before: f32,
    height_lag_velocity: [f32; 1],
    height_lag_time: f32,
    height_lag_before: f32,
    /// The stance held when something was approached that had to be
    /// crouched to, to be taken again when it is let go
    /// (`LevelOnApproachStart`); none while nothing is.
    approach_level: Option<f32>,
    /// The hands' gestures on the world (interactions spec): the game's
    /// model of them, and the one the body's animator is being shown.
    interactions: AnimatedInteractions,
    shown_interaction: Option<u8>,
    /// What the character is doing to a thing of the world, in one of the
    /// game's two movement states for it; and what came of it, for the
    /// host to take.
    acting: Option<Acting>,
    acted: Vec<Acted>,
    /// A door being kicked, from the key until the kick's state is left.
    kick: Option<Kick>,
    /// A container searched and not yet shown: where it stands, how far
    /// from it the character was at the key, the seconds since, and
    /// whether the key was in this frame (which the game's wait does not
    /// count).
    searching: Option<([f32; 3], f32, f32, bool)>,
    /// The animator of the thing in hand is at rest, as last told.
    hands_idle: bool,
    /// The body's two flags for an open inventory, and whether the hands
    /// have been down for it since it opened.
    body_inventory: [bool; 2],
    inventory_reached: bool,
}

/// A thing of the world that is worked with something in the hand, a key
/// or a card, or that is kicked in: the game then holds the character at
/// it (`Movement::unlock`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldInteraction {
    /// The gesture the thing asks for.
    pub interaction: Interaction,
    /// Whether the thing has the character put at a place of its own to
    /// work it, or has a grip for the hand: the state then lasts its full
    /// three seconds.
    pub snap: bool,
    /// How high the thing is reached for, in the world: a low one is
    /// crouched to.
    pub height: f32,
}

/// What came of something the character was doing to a thing of the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acted {
    /// The wait after a container was searched is over and the character
    /// is still at it: it is now shown.
    Searched,
    /// That wait is over and the character has gone from the container,
    /// or is in a jump: it is not shown, and shuts again.
    LeftUnsearched,
    /// A thing unlocked or kicked: the hand has been on it long enough,
    /// and the thing now does what was asked of it.
    Worked,
    /// The state for such a thing is over.
    Done,
    /// The thing reached for is taken: it goes into the inventory now.
    PickedUp,
    /// A kick has landed on a door, 0.45 of the way through its state: it
    /// throws the door open, or the door holds.
    Kicked { opens: bool },
    /// The backpack being dropped has left the shoulders: its gesture has
    /// come to the mark its state has for that, and the thing itself now
    /// leaves the inventory (`OnBackpackDrop`).
    BackpackDropped,
}

/// A door being kicked (`MovementContext.StartDoorBreak`): whether the
/// kick will throw it open, as worked out at the key; whether it has
/// landed; and whether the character counts as standing where it is to.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Kick {
    opens: bool,
    landed: bool,
    approached: bool,
    /// Where the door puts the character first and what it has it look
    /// at, if it does; and the approach that takes it there, once the
    /// approach has begun.
    snap: Option<([f32; 3], [f32; 3])>,
    approach: Option<Approaching>,
}

/// The approach to a thing that puts the character at a place of its own
/// (`ApproachState`): how long it has run and how long it takes, in
/// seconds of the engine's clock, and where the character stood, looked
/// and faced when it began against where it is to.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Approaching {
    time: f32,
    length: f32,
    from: [f32; 3],
    to: [f32; 3],
    look_from: [f32; 2],
    look_to: [f32; 2],
    body_from: f32,
    direction: [f32; 2],
    fresh: bool,
}

/// How fast the approach walks the character to its place, in metres a
/// second, unburdened and at the most overweight; how fast it turns the
/// view, in degrees a second; and the longest it takes, in seconds
/// (`ApproachState.ApproachSpeed`, `ApproachAngularSpeed`).
const APPROACH_SPEED: [f32; 2] = [2.5, 0.01];
const APPROACH_TURN: f32 = 90.0;
const APPROACH_LONGEST: f32 = 1.5;

/// How far through the kick's state the body begins to move by its
/// animation's own motion, and how far through it the kick lands
/// (`BreachDoorState`).
const KICK_MOVES_FROM: f32 = 0.15;
const KICK_LANDS_AT: f32 = 0.45;

/// The game's two movement states for the hands on the world.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Acting {
    /// Unlocking or kicking a door or a container (`DoorInteractState`):
    /// the seconds in it, whether the thing has been told to do its part,
    /// and whether the state runs its full length.
    Working { time: f32, worked: bool, snap: bool },
    /// Taking a thing up (`PickUpState`): the seconds in it, the stance it
    /// began with, and whether the thing has been taken.
    PickingUp { time: f32, level: f32, taken: bool },
}

/// How long the hand is laid on a thing before the thing does its part,
/// how long the state for it lasts where it runs its full length, and how
/// long a thing is reached for before it is taken, in seconds. The first
/// two are written in the game's state for a door (it leaves the first
/// sooner when the hands' animator says the left hand has arrived, which
/// a firearm's never does); the third in its state for a pick-up.
const HAND_LAID_AFTER: f32 = 1.6;
const WORKING_ENDS_AFTER: f32 = 3.0;
const PICKED_UP_AFTER: f32 = 0.6;
/// How far down the view has to point for a pick-up to crouch the
/// character to a third of its stance, in degrees.
const PICK_UP_CROUCH_PITCH: f32 = 60.0;
/// How much further from a searched container than at the key the
/// character may be when the wait is over and still be shown it, in
/// metres (`InteractionContextHelper.OnContainerOpen`).
const SEARCH_REACH: f32 = 0.25;

impl Movement {
    /// The character's carried weight changes, as when something is put
    /// on or taken off: the inertia, the speed limits and the stamina's
    /// costs all follow, from the next frame (`PhysicalBase` reads the
    /// inventory's sum whenever it changes; inventory spec, "Weight").
    /// The skills, the pools and the character's food and water stay.
    pub fn set_weight(
        &mut self,
        hard: &trp_data::hard_settings::HardSettings,
        config: &serde_json::Value,
        weight: f32,
    ) -> anyhow::Result<()> {
        let mut settings = self.settings.reweighed(hard, config, weight)?;
        self.stamina.reweigh(settings.stamina.overweight, settings.stamina.walk_overweight);
        settings.stamina = self.stamina.settings.clone();
        self.settings = settings;
        Ok(())
    }

    pub fn new(settings: MovementSettings, body: Body) -> Self {
        let stamina = Stamina::new(settings.stamina.clone());
        let speed_memo = settings.max_speed;
        Self {
            head_look: HeadLook::new(settings.head_look),
            trunk_limit: settings.trunk_rotation_limit,
            character_speed: settings.max_speed,
            relative_speed: 1.0,
            speed_cap: settings.hard_speed_cap(STANDING_SPEED_CAP, false),
            landing_limit: None,
            landing_limit_in_effect: None,
            limits_changed: false,
            frame_clock: 0.0,
            fall_from: 0.0,
            was_grounded: true,
            // The game starts the smoothed speed at the selected speed, and
            // standing still does not bring it down.
            smoothed_speed: settings.max_speed.clamp(0.0, settings.state_speed_limit),
            // A character is made with it at 1, and it only comes down to
            // the starting figure when a sprint ends (movement spec,
            // "Sprinting").
            sprint_speed: 1.0,
            // Every standing-type state sets the step's playback speed to
            // its resting figure on being entered; a character met
            // standing has it there.
            sideback_speed: settings.sideback_speed,
            sidestep_data: StepData { speed: settings.sideback_speed, ..StepData::default() },
            roll_data: StepData { speed: settings.sideback_speed, ..StepData::default() },
            pending_step: 0,
            roll_bearing: 0.0,
            roll_yaw: 0.0,
            settings,
            position: [0.0; 3],
            velocity: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            body_yaw: 0.0,
            state: State::Idle,
            previous_state: State::Idle,
            movement_direction: [0.0; 2],
            discrete_direction: 1.0,
            pose_level: 1.0,
            smoothed_pose_level: 1.0,
            pose_memo: 1.0,
            prone: false,
            reloading: false,
            hands_busy: false,
            inventory: false,
            prone_reload: false,
            turn_factor: 1.0,
            turn_limit: FREE_TURN_LIMIT,
            can_interact: true,
            pitch_limit: UPRIGHT_PITCH_RANGE,
            pitch_target: UPRIGHT_PITCH_RANGE,
            yaw_limit: None,
            step: 0,
            step_key: 0,
            lean_keys: [false; 2],
            last_slow_lean: false,
            slow_leaning: false,
            speed_memo,
            dropping_backpack: false,
            sidestep: 0.0,
            prone_sprint: false,
            surface_direction: [0.0; 3],
            surface_normal: [0.0, 1.0, 0.0],
            body_tilt: [0.0, 0.0, 0.0, 1.0],
            surface_pitch: 0.0,
            tilt: 0.0,
            smoothed_tilt: 0.0,
            tilt_multiplier: 0.0,
            prev_tilt: 0.0,
            sprinting: false,
            sprint_input: false,
            idle_sprint_held: false,
            aiming: false,
            was_sprinting: false,
            sprint_started: false,
            aim_lowered: false,
            aim_taken: false,
            hands_to_body: 0.0,
            aim_angle: 0.0,
            body_yaw_fine: 0.0,
            stamina,
            grounded: true,
            ground_check: GROUND_CHECK,
            freefall_time: 0.0,
            last_blend_velocity: [0.0; 3],
            velocity_before_limit: [0.0; 3],
            yaw_deltas: VecDeque::new(),
            direction: [0.0; 2],
            input_direction: [0.0; 2],
            frame_count: 0,
            sprint_dropped_in: 0,
            frame_dt: 0.0,
            stopwatch: Stopwatch::default(),
            unstepped: 0.0,
            clock_lag_before: 0.0,
            clock_lag: 0.0,
            mv: MoveData::fresh(),
            mv_prone: MoveData::fresh(),
            jump: JumpData::default(),
            body,
            inert: false,
            sprint_flag: false,
            jump_flag: false,
            landing_flag: false,
            animated_motion: [0.0; 3],
            landing_idle_time: 0.0,
            landing_still: false,
            clock: 0.0,
            last_step: 0.0,
            step_side: 1.0,
            footstep: None,
            vaulting: Vaulting::default(),
            vault_time: 0.0,
            vault_key: false,
            vault_on: false,
            do_vault: false,
            do_climb: false,
            is_vaulting: false,
            // As the controller has them before anything is measured.
            vault_figures: [0.0, 1.0, 0.0, 0.0],
            vaulting_speed: 1.0,
            vault: VaultData::default(),
            vault_given: None,
            blocked: false,
            blocked_direction: [0.0; 2],
            slow_time: 0.0,
            real_speed: 0.0,
            place_before: [0.0; 3],
            body_height_offset: 0.0,
            height_before: 0.0,
            height_lag_velocity: [0.0],
            // The game starts this at zero and lets it drift to its resting
            // value over the first seconds of a raid; a character met any
            // later has it there.
            height_lag_time: HEIGHT_SMOOTH_TIME,
            height_lag_before: 0.0,
            approach_level: None,
            interactions: AnimatedInteractions::default(),
            shown_interaction: None,
            acting: None,
            kick: None,
            searching: None,
            acted: Vec::new(),
            hands_idle: true,
            body_inventory: [false; 2],
            inventory_reached: false,
        }
    }

    /// The selected speed after the active speed limits: the standing one
    /// from carried weight, the one aiming adds, and the one a landing
    /// leaves for a while.
    pub fn clamped_speed(&self) -> f32 {
        let mut limit = self.settings.state_speed_limit;
        if self.aiming {
            limit = limit.min(AIMING_SPEED_LIMIT);
        }
        if let Some(landing) = self.landing_limit_in_effect {
            limit = limit.min(landing);
        }
        self.character_speed.clamp(0.0, limit)
    }

    /// The highest speed setting the movement state allows just now
    /// (`MovementContext.StateSpeedLimit` with the limits in force): what
    /// the game's speed display lights its notches up to.
    pub fn speed_limit(&self) -> f32 {
        let mut limit = self.settings.state_speed_limit;
        if self.aiming {
            limit = limit.min(AIMING_SPEED_LIMIT);
        }
        if let Some(landing) = self.landing_limit_in_effect {
            limit = limit.min(landing);
        }
        limit.min(self.settings.max_speed)
    }

    /// A change of the speed asked of the movement state in hand
    /// (`MovementState.ChangeSpeed`): from the speed as now limited. A
    /// sprint takes none.
    fn change_speed(&mut self, delta: f32) {
        if self.state != State::Sprint {
            self.select_speed(self.clamped_speed() + delta);
        }
    }

    /// Sets the speed wheel.
    pub fn select_speed(&mut self, speed: f32) {
        let top = self.settings.max_speed;
        self.character_speed = speed.clamp(0.0, top);
        self.relative_speed = if top > 0.0 { self.character_speed / top } else { 0.0 };
    }

    fn hard_speed_cap(&self, state: State) -> f32 {
        let range = match state {
            State::Idle | State::ProneIdle | State::Roll => STANDING_SPEED_CAP,
            // A landing, going down and getting up have no cap of their
            // own and take the general one. The sidestep is given it.
            State::Move | State::Landing | State::ToProne | State::FromProne | State::Sidestep => WALKING_SPEED_CAP,
            // A vault, a climb and what follows them have none of their
            // own either; nor have the approach to a door and the kick.
            State::Vault | State::Climb | State::VaultFall | State::VaultLanding => WALKING_SPEED_CAP,
            State::Approach | State::Breach => WALKING_SPEED_CAP,
            State::Sprint | State::Jump | State::SprintStop => SPRINTING_SPEED_CAP,
            State::ProneMove => CRAWLING_SPEED_CAP,
        };
        // The carried weight's say on the walking cap is left out of a
        // sprint's, and of a vault's and a climb's (vaulting spec).
        self.settings.hard_speed_cap(range, matches!(state, State::Sprint | State::Vault | State::Climb))
    }

    /// Height of the collision capsule for the current stance.
    pub fn capsule_height(&self) -> f32 {
        if matches!(self.state, State::ToProne | State::ProneIdle | State::ProneMove | State::Roll) {
            0.3
        } else {
            lerp(1.2, 1.6, self.pose_level)
        }
    }

    /// The space the character's collision takes up over its feet just
    /// now (`Capsule`): from the feet up to the stance's height, but for
    /// a vault or a climb, whose capsule the game shrinks from below.
    pub fn capsule(&self) -> Capsule {
        if matches!(self.state, State::Vault | State::Climb) {
            let [height, centre] = self.vault.capsule;
            Capsule { underside: (centre - height / 2.0).max(0.0), top: centre + height / 2.0, lift: false }
        } else {
            Capsule { underside: 0.0, top: self.capsule_height(), lift: true }
        }
    }

    /// The game's capsule as its code sets it: the height, and how far
    /// over the feet the centre is. By the stance (`SetPoseLevel`: 1.2 to
    /// 1.6 m high with the centre 0.6 to 0.8 up, and 0.3 with the centre
    /// 0.35 up lying down; the game's own controller for the player has no
    /// skin to add), but for a vault or a climb under way.
    pub fn capsule_figures(&self) -> [f32; 2] {
        if matches!(self.state, State::Vault | State::Climb) {
            self.vault.capsule
        } else if matches!(self.state, State::ToProne | State::ProneIdle | State::ProneMove | State::Roll) {
            [0.3, 0.35]
        } else {
            [lerp(1.2, 1.6, self.pose_level), lerp(0.6, 0.8, self.pose_level)]
        }
    }

    /// What the animator has been told of a vault or a climb: the
    /// obstacle's height, its length, the drop behind it and the forward
    /// speed it was measured at; the pace of the move's clips; and the two
    /// flags that ask for a vault and for a climb.
    pub fn vault_parameters(&self) -> ([f32; 4], f32, [bool; 2]) {
        (self.vault_figures, self.vaulting_speed, [self.do_vault, self.do_climb])
    }

    /// Puts the figures the animator holds of the last vault or climb where
    /// a character met some way into a raid has them: they stay as the
    /// last move left them.
    pub fn set_vault_parameters(&mut self, figures: [f32; 4], pace: f32) {
        (self.vault_figures, self.vaulting_speed) = (figures, pace);
    }

    /// The view's pitch as it is to be shown. In a vault or a climb that
    /// takes a hand the view is the move's own (vaulting spec, "The
    /// view"): the pitch it began with, brought up to at most 50 degrees
    /// down by the middle of the move and back by its end, by the clips'
    /// `Mouse_look` curve. The mouse does not move it meanwhile.
    pub fn shown_pitch(&self) -> f32 {
        self.pitch_as_shown(self.pitch)
    }

    fn pitch_as_shown(&self, pitch: f32) -> f32 {
        /// The furthest down the view is left in such a move, degrees.
        const DEEPEST: f32 = 50.0;
        if self.vault.hands && matches!(self.state, State::Vault | State::Climb) {
            lerp(pitch, pitch.min(DEEPEST), self.body.mouse_look())
        } else {
            pitch
        }
    }

    /// How far the view may pitch just now: up, then down.
    pub fn pitch_range(&self) -> [f32; 2] {
        self.pitch_limit
    }

    /// Horizontal speed in metres per second.
    pub fn ground_speed(&self) -> f32 {
        length([self.velocity[0], self.velocity[2]])
    }

    /// The velocity the controller asked for this step, before the world
    /// had its say.
    pub fn input_velocity(&self) -> [f32; 3] {
        self.velocity_before_limit
    }

    /// Turns the body's axes into those of the frame the first-person rig
    /// hangs in: at the ribcage, turned to where the player looks.
    pub fn body_in_view(&self) -> [f32; 4] {
        quat_mul(quat_mul(quat_axis(0, -self.pitch), quat_axis(1, self.body_yaw - self.yaw)), self.body_tilt)
    }

    /// Where the ribcage is from the feet, in the axes of the way the
    /// body faces (x right, y up, z ahead): what the body's animation
    /// gives, laid to the ground if the body is, and held back by the lag
    /// in the body's height. The first-person rig and the camera hang there.
    /// Where to draw the character's feet. It differs from `position` only
    /// in a jump. The game moves a jumping character by the player's own
    /// clock, which counts whole milliseconds and so runs up to a
    /// millisecond behind the frames and catches up in steps (8, 8 and 9
    /// ms at 120 frames a second): the travel through the air is uneven
    /// from frame to frame, where travel on the ground, which is the
    /// animator's, is even. This is the place the same jump has reached by
    /// the frames' own time, which is that jump without the ripple. Level
    /// travel only: the eye's height already trails the feet's.
    pub fn drawn_position(&self) -> [f32; 3] {
        if self.state != State::Jump {
            return self.position;
        }
        let behind = self.clock_lag - self.jump.lag_at_start;
        let [x, y, z] = self.position;
        [x + self.velocity[0] * behind, y, z + self.velocity[2] * behind]
    }

    /// Which way to draw the body facing. It differs from `body_yaw` only
    /// in a jump, and for the reason `drawn_position` does: there the game
    /// turns the body after the view by the player's whole-millisecond
    /// clock, so on frames of even length the body turns by uneven amounts
    /// (an eighth more on every third frame at 120 a second), and the
    /// weapon it carries in the air shakes against the view. This is where
    /// the same turn has got to by the frames' own time.
    pub fn drawn_body_yaw(&self) -> f32 {
        if self.state != State::Jump {
            return self.body_yaw;
        }
        self.body_yaw + self.jump.turn_rate * (self.clock_lag - self.jump.lag_at_start)
    }

    /// `body_in_view` for a host that draws: with the body facing as it
    /// is to be drawn (`drawn_body_yaw`).
    pub fn drawn_body_in_view(&self) -> [f32; 4] {
        quat_mul(quat_mul(quat_axis(0, -self.pitch), quat_axis(1, self.drawn_body_yaw() - self.yaw)), self.body_tilt)
    }

    pub fn ribcage(&self) -> [f32; 3] {
        let mut at = quat_rotate(self.body_tilt, self.body.ribcage());
        at[1] += self.body_height_offset;
        at
    }

    /// The angle from the body round to the view and the view's pitch
    /// over the ground's, as last noted: what the animator's
    /// `DeltaRotation` and `Aim_angle` stand at when the frame is over.
    pub fn noted_angles(&self) -> [f32; 2] {
        [self.hands_to_body, self.aim_angle]
    }

    /// The view the last frame's pose of the body was made for, as its
    /// yaw and pitch. The body's animation bends and turns the upper body
    /// by the angles the game noted when it last turned the body, which is
    /// before the frame's own turn of the view: the ribcage, and the eye
    /// on it, trail the view by a frame's turn (movement spec, "Looking").
    pub fn posed_for(&self) -> [f32; 2] {
        let [delta_rotation, aim_angle] = self.body.posed_for();
        [self.drawn_body_yaw() + delta_rotation, self.pitch + aim_angle - self.aim_angle]
    }

    /// `ribcage` as it would be now had the body's pose been made for the
    /// view at another yaw and pitch, nothing else different. For a host
    /// that draws between the simulation's frames and shows the view's
    /// turning ahead of them: with this it can put the ribcage where the
    /// view it shows would have it, where the frames it draws the rest of
    /// the body from were posed for the view as it stood some frames ago.
    /// `ribcage_posed_for(posed_for())` is `ribcage()`.
    pub fn ribcage_posed_for(&self, look: [f32; 2]) -> [f32; 3] {
        let delta_rotation = delta_angle(self.drawn_body_yaw(), look[0]);
        let aim_angle = self.aim_angle + look[1] - self.pitch;
        let mut at = quat_rotate(self.body_tilt, self.body.ribcage_posed_for(delta_rotation, aim_angle));
        at[1] += self.body_height_offset;
        at
    }

    /// Puts a bone the body gives in its own axes (`Body::arm_bone`,
    /// `Body::weapon_bone`) where it is from the feet, as `ribcage` does
    /// the ribcage: laid to the ground with the body and held back by the
    /// lag in its height.
    pub fn in_player(&self, bone: ([f32; 3], [f32; 4])) -> ([f32; 3], [f32; 4]) {
        let mut at = quat_rotate(self.body_tilt, bone.0);
        at[1] += self.body_height_offset;
        (at, quat_mul(self.body_tilt, bone.1))
    }

    /// Whether the movement state lets the sights be up: not in a crawl,
    /// a sprint or a jump. For `Firearm::set_aim`.
    pub fn can_aim(&self) -> bool {
        !self.settings.no_aim_states.contains(&self.state.number())
    }

    /// Whether the character has been off the ground long enough for the
    /// sights to be put down (movement spec, "Falling"). The game does
    /// that on every such frame, after the movement's update, through the
    /// aim key's own call: a host calls `Firearm::set_aim(false, ..)` for
    /// as long as this holds.
    pub fn long_fall(&self) -> bool {
        self.freefall_time > LONG_FALL
    }

    /// The frame the first-person rig hangs in, in the world's axes: where
    /// its origin is (the body's ribcage) and how it is turned (to where
    /// the player looks).
    pub fn rig_frame(&self) -> ([f32; 3], [f32; 4]) {
        let ribcage = quat_rotate(quat_axis(1, self.body_yaw), self.ribcage());
        (
            std::array::from_fn(|k| self.position[k] + ribcage[k]),
            quat_mul(quat_axis(1, self.yaw), quat_axis(0, self.shown_pitch())),
        )
    }

    /// What the world does to the first-person camera's box ("The
    /// camera's collider" in the weapon-handling spec): how far the camera
    /// has to move for its box to be clear of everything solid, in the
    /// frame the rig hangs in. `position` and `rotation` are the camera as
    /// the weapon motion has just placed it (`Hands::frame`); `size` and
    /// `centre` are the box on the player prefab, in the camera's own axes.
    /// For `Hands::push_camera`, last in a frame.
    pub fn camera_push(
        &self,
        world: &dyn World,
        position: [f32; 3],
        rotation: [f32; 4],
        size: [f32; 3],
        centre: [f32; 3],
    ) -> [f32; 3] {
        /// What the game adds to each push, in metres.
        const MARGIN: f32 = 0.001;
        let (origin, turn) = self.rig_frame();
        let facing = quat_mul(turn, rotation);
        let (beside, middle) = (quat_rotate(turn, position), quat_rotate(facing, centre));
        let at: [f32; 3] = std::array::from_fn(|k| origin[k] + beside[k] + middle[k]);
        let push = world.push_out(at, size.map(|s| s / 2.0), facing, MARGIN);
        quat_rotate([-turn[0], -turn[1], -turn[2], turn[3]], push)
    }

    /// How deep the weapon would stick into something, in metres
    /// ("Turn-away" in the weapon-handling spec). `line` is the weapon's
    /// line in the rig's frame (`Hands::weapon_line`). If it hits, the
    /// depth is what is left of it past the hit. If it does not, a second
    /// line is cast from the character's own axis, at the height the
    /// first starts at, out to that start: a hit there means the weapon's
    /// near end is through something already, and counts as the whole
    /// length. For `Hands::set_overlap` and `Firearm::overlap`, once a
    /// frame, when the weapon has been placed.
    pub fn weapon_overlap(&self, world: &dyn World, line: ([f32; 3], [f32; 3])) -> f32 {
        let (origin, turn) = self.rig_frame();
        let out = |at: [f32; 3]| -> [f32; 3] {
            let turned = quat_rotate(turn, at);
            std::array::from_fn(|k| origin[k] + turned[k])
        };
        let (from, to) = (out(line.0), out(line.1));
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        if let Some(hit) = world.line(from, to) {
            return (length - hit).max(0.0);
        }
        let axis = [self.position[0], from[1], self.position[2]];
        if world.line(axis, from).is_some() { length } else { 0.0 }
    }

    /// Whether the movement state lets the hands take a press of the
    /// trigger: not in a sprint or a crawl. For `Firearm::set_trigger`.
    pub fn can_press_trigger(&self) -> bool {
        // Nor in a vault or a climb of something high enough to take a
        // hand (`FirearmController.CheckVaulting`).
        let s = &self.settings.vaulting;
        let clear = if self.do_vault { s.vault.max_without_hand } else { s.climb.max_without_hand };
        let vaulting = self.is_vaulting && self.vault.height > clear;
        !vaulting && !self.settings.no_hands_states.contains(&self.state.number())
    }

    /// How tired the character is and whether it holds its breath, for
    /// the weapon's breathing (`HandsInput::wind`).
    pub fn wind(&self) -> crate::breath::Wind {
        crate::breath::Wind {
            stamina: self.stamina.breath_stamina(),
            oxygen: self.stamina.breath_oxygen(),
            held: self.stamina.holding_breath(),
        }
    }

    /// Whether a vault or a climb has taken the sights down since this
    /// was last asked: one over something high enough to take a hand does,
    /// through the aim key's own call. A host answers with
    /// `Firearm::set_aim(false, ..)`.
    pub fn take_aim_taken(&mut self) -> bool {
        std::mem::take(&mut self.aim_taken)
    }

    /// Whether a movement state that does not allow aiming has been
    /// entered since this was last asked. For `Firearm::lower_aim`.
    pub fn take_aim_lowered(&mut self) -> bool {
        std::mem::take(&mut self.aim_lowered)
    }

    /// The first-person pose of each state the body's animator has entered
    /// since this was last asked, in order; `None` for a state without
    /// one. For `Hands::assign_pose`.
    pub fn take_poses(&mut self) -> Vec<Option<trp_data::states::FirstPersonPose>> {
        self.body.take_poses()
    }

    /// Whether the sprint has switched on since this was last asked:
    /// `sprinting`, which the key sets in a walk forward some time before
    /// the sprinting state begins. For `Firearm::sprint_started`.
    pub fn take_sprint_started(&mut self) -> bool {
        std::mem::take(&mut self.sprint_started)
    }

    /// Notes the sprint switching on. The weapon's logic lowers the sights
    /// at that moment (firing spec, "Aiming, and what takes it away"), and
    /// with them goes the cap aiming puts on the speed, before the frame's
    /// look at its limits; a host whose weapon kept them up says so with
    /// the next frame's input.
    /// How fast going down and getting up play (the animator's
    /// `TransitionMultiplier`): by the carried weight, and as slowly as
    /// fully overweight on spent legs.
    fn transition_speed(&self) -> f32 {
        let s = &self.settings;
        let weight = s.overweight.max(if self.stamina.legs.exhausted() { 1.0 } else { 0.0 });
        lerp(s.transition_speed_range[0], s.transition_speed_range[1], weight)
    }

    /// Asks for the sprint or lets it go (`MovementContext.EnableSprint`).
    /// The flag is the stamina's: a sprint is one being paid for, and one
    /// asked for on spent legs does not start (stamina spec, "Who calls
    /// it").
    fn enable_sprint(&mut self, on: bool) {
        self.sprinting = self.stamina.sprint(on, self.prone, self.frame_clock);
    }

    fn watch_sprint(&mut self) {
        if self.sprinting && !self.was_sprinting {
            self.sprint_started = true;
            self.aiming = false;
        }
        self.was_sprinting = self.sprinting;
    }

    /// Where the body's animation holds the weapon, where it does (a
    /// sprint, a crawl), for `Hands::weapon_transform_carried`.
    pub fn carried(&self) -> Option<Carried> {
        self.body.carried(self.body_in_view())
    }

    /// The body's right palm, where its animation takes the weapon in that
    /// hand (a crawl), for the same.
    pub fn held(&self) -> Option<Held> {
        self.body.held(self.body_in_view())
    }

    /// An arm as the body's animation has it, while it takes any of the
    /// arm from the weapon (a crawl), for a host that draws arms.
    pub fn arm(&self, right: bool) -> Option<ArmPose> {
        self.body.arm(right, self.body_in_view())
    }

    /// An arm as the body's animation has it, whatever share of it that
    /// animation claims: what the arm's solver starts from (`arms::hold`).
    pub fn arm_pose(&self, right: bool) -> ArmPose {
        self.body.arm_pose(right, self.body_in_view())
    }

    /// A footstep that has happened since this was last asked: the seconds
    /// between it and the footstep before. The weapon's bob is tuned by it.
    pub fn take_footstep(&mut self) -> Option<f32> {
        self.footstep.take()
    }

    /// Footsteps come from the body's animation: a curve in its clips
    /// changes sign each time a foot comes down. One counts if the last
    /// was more than 0.2 s ago and the character has not been falling long.
    fn watch_footsteps(&mut self, dt: f32) {
        const SHORTEST_GAP: f32 = 0.2;
        self.clock += dt;
        let longest_fall = match self.state {
            State::Move => 1.0,
            State::Sprint => 0.6,
            _ => return,
        };
        let side = sign(self.body.foot_step());
        if side == self.step_side {
            return;
        }
        self.step_side = side;
        let gap = self.clock - self.last_step;
        if gap > SHORTEST_GAP && self.freefall_time < longest_fall {
            self.last_step = self.clock;
            self.footstep = Some(gap);
        }
    }

    /// What the body's animator is told this step.
    fn body_input(&self) -> BodyInput {
        BodyInput {
            speed: self.smoothed_speed,
            direction: self.movement_direction,
            discrete_direction: self.discrete_direction,
            level: self.smoothed_pose_level,
            tilt: self.smoothed_tilt,
            inert: self.inert,
            sprint: self.sprint_flag,
            sprint_speed: self.sprint_speed,
            jumping: self.jump_flag,
            landing: self.landing_flag,
            // Falls are not simulated yet, so the animator is never told of one.
            falling: false,
            grounded: self.grounded,
            prone: self.prone,
            transition_speed: self.transition_speed(),
            aim_angle: self.aim_angle,
            delta_rotation: self.hands_to_body,
            aiming: self.aiming,
            reloading: self.prone_reload,
            hands_busy: self.hands_busy,
            step: self.step,
            sidestep: self.sidestep,
            sideback_speed: self.sideback_speed,
            do_vault: self.do_vault,
            do_climb: self.do_climb,
            vaulting: self.is_vaulting,
            vault_figures: self.vault_figures,
            vaulting_speed: self.vaulting_speed,
            interaction: self.shown_interaction,
            inventory: self.body_inventory[0],
            inventory_operation: self.body_inventory[1],
            kick: self.kick.is_some(),
            kick_succeeds: self.kick.is_some_and(|kick| kick.opens),
            approached: self.kick.is_some_and(|kick| kick.approached),
        }
    }

    /// Puts on the body's animator what the interactions' model asks.
    fn show(&mut self, shows: Vec<Show>) {
        for show in shows {
            match show {
                Show::Play(index) => self.shown_interaction = Some(index),
                Show::Stop => self.shown_interaction = None,
                Show::Reset => {
                    self.shown_interaction = None;
                    self.body.reset_interactions();
                }
            }
        }
    }

    /// Asks for a gesture of the hands (`MovementContext.SetInteractInHands`).
    /// It is refused in a sprint, and while the animator of the thing in
    /// hand is anywhere but at rest (a backpack's dropping excepted).
    pub fn set_interaction(&mut self, interaction: Interaction) -> bool {
        if self.sprinting || (!self.hands_idle && interaction != Interaction::DropBackpack) {
            return false;
        }
        let shows = self.interactions.set(interaction);
        self.show(shows);
        true
    }

    /// The gesture playing on the body's animator, if one is.
    pub fn interaction(&self) -> Option<Interaction> {
        self.interactions.current()
    }

    /// Whether the movement lets a backpack be dropped (`Player.DropBackpack`,
    /// of what it asks of the movement): not in a sprint, not through a
    /// kick, and only with no gesture of the hands playing.
    pub fn can_drop_backpack(&self) -> bool {
        !self.sprinting && self.state != State::Breach && self.interactions.can_interact()
    }

    /// The backpack is dropped (the hands' `DropBackpackOperation`, which
    /// asks the body for the gesture): `Acted::BackpackDropped` comes when
    /// the gesture's state passes its mark for the drop. The gesture is
    /// asked for whatever the thing in hand is doing; in a sprint it is
    /// refused, and then nothing is dropped. Returns whether it plays.
    pub fn drop_backpack(&mut self) -> bool {
        let plays = self.set_interaction(Interaction::DropBackpack);
        self.dropping_backpack |= plays;
        plays
    }

    /// The character works a thing of the world: a door pushed or pulled,
    /// a container opened or shut (`Player.ExecuteInteraction`,
    /// `MovementState.ExecuteDoorInteraction`; interactions spec, "Working
    /// a door or a container"). **The thing itself begins to move half a
    /// second later or a little more** (`things::Thing` does it). Here a
    /// character lying down is no longer lying, and the
    /// thing's gesture is asked of the left hand: it is refused in a
    /// sprint and while the thing in hand is busy (`set_interaction`), and
    /// the thing works all the same; a thing worked without a gesture
    /// (`interactWithoutAnimation`) asks for none. No movement state is
    /// taken: the character walks, turns and does as it likes meanwhile.
    /// Returns whether a gesture plays.
    pub fn work(&mut self, gesture: impl Into<Option<Interaction>>) -> bool {
        if self.prone {
            self.toggle_prone_for_interaction();
        }
        gesture.into().is_some_and(|gesture| self.set_interaction(gesture))
    }

    /// The character searches a container standing at `at`: it is worked
    /// as above, and when the game's wait for it is over
    /// (`MovementSettings::thing_answers_after`, the install's
    /// `DelayToOpenContainer`) `Acted::Searched` says it is to be shown,
    /// or `Acted::LeftUnsearched` that it is not: the character is in a
    /// jump, or more than a quarter of a metre further from it than it
    /// was at the key.
    pub fn search(&mut self, gesture: impl Into<Option<Interaction>>, at: [f32; 3]) -> bool {
        let played = self.work(gesture);
        self.searching = Some((at, self.distance_to(at), 0.0, true));
        played
    }

    fn distance_to(&self, at: [f32; 3]) -> f32 {
        let d = [at[0] - self.position[0], at[1] - self.position[1], at[2] - self.position[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    /// The character works a thing that is unlocked with a key or a card
    /// in the hand, or kicks a door in: the game's other way to a thing
    /// (`MovementContext.StartInteraction`, `ApproachState`,
    /// `DoorInteractState`; interactions spec, "Unlocking and kicking").
    /// Lying down it gets up first. The lean is let go and a low thing
    /// crouched to (`approach`); the gesture begins; `Acted::Worked` comes
    /// when the hand has been laid on the thing, and `Acted::Done` when
    /// the state is over. Nothing is started while something else is
    /// being worked. PROVISIONAL: read from the game's code and compared
    /// with nothing; no recording has a lock or a kick.
    pub fn unlock(&mut self, with: WorldInteraction) -> bool {
        if self.acting.is_some() {
            return false;
        }
        if self.prone {
            // The game takes the character out of its lying pose here.
            self.toggle_prone_for_interaction();
        }
        self.approach(with.height);
        self.tilt = 0.0;
        self.set_interaction(with.interaction);
        self.acting = Some(Acting::Working { time: 0.0, worked: false, snap: with.snap });
        true
    }

    /// The character kicks a door (`MovementContext.StartDoorBreak`;
    /// interactions spec, "A kick"). `opens` is whether the kick will throw
    /// the door open, which the door says for where the character stands
    /// (`Thing::breach_succeeds`), and which picks the body's clip. The
    /// body's animator takes it from here: out of standing or a walk into
    /// its approach, and from there into the kick, and
    /// `Acted::Kicked` says when it lands. Nothing is started while
    /// another kick, an unlocking or a pick-up is in hand.
    ///
    /// `snap` is for a door that puts the character at a place of its own
    /// first: that place and the point it has the character look at
    /// (`Thing::start_breach`). The approach then walks the character
    /// there and turns its view to the point, levelled, over as long as
    /// the longest of the three takes, and the kick waits for it. A door
    /// that does not leaves the character where it stands, looking as it
    /// looks.
    pub fn kick(&mut self, opens: bool, snap: Option<([f32; 3], [f32; 3])>) -> bool {
        if self.kick.is_some() || self.acting.is_some() {
            return false;
        }
        // Where the door puts the character nowhere, it counts as
        // standing where it is to from the key on.
        self.kick = Some(Kick { opens, landed: false, approached: snap.is_none(), snap, approach: None });
        true
    }

    /// The approach begins (`ApproachState.Enter`), for a thing that puts
    /// the character at a place: how long it takes is the longest of the
    /// walk there, the turn of the view and its levelling, and no more
    /// than a second and a half.
    fn begin_approach(&mut self) {
        let Some(Kick { snap: Some((to, target)), .. }) = self.kick else { return };
        let from = self.position;
        // The view is to look from that place, at the ribcage's height,
        // to the point, and level (`ViewTargetWithZeroPitch`).
        let eye = [to[0], self.position[1] + self.ribcage()[1], to[2]];
        let yaw = (target[0] - eye[0]).atan2(target[2] - eye[2]).to_degrees();
        let look_to = [yaw, 0.0];
        let far = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let speed = lerp(APPROACH_SPEED[0], APPROACH_SPEED[1], self.stamina.settings.overweight.clamp(0.0, 1.0));
        let turns = [delta_angle(look_to[0], self.yaw).abs(), delta_angle(look_to[1], self.pitch).abs()];
        let length = (far / speed).max(turns[0] / APPROACH_TURN).max(turns[1] / APPROACH_TURN).min(APPROACH_LONGEST);
        let approach = Approaching {
            time: 0.0,
            length,
            from,
            to,
            look_from: [self.yaw, self.pitch],
            look_to,
            body_from: self.body_yaw,
            direction: [0.0; 2],
            fresh: true,
        };
        if let Some(kick) = &mut self.kick {
            kick.approach = Some(approach);
        }
    }

    /// The approach's own handling of a frame (`ApproachState.
    /// ManualAnimatorMoveUpdate`, `Approach`): the character is where the
    /// approach's curve has it between where it stood and its place, its
    /// view and its body turned by the same share; when the approach's
    /// time is out it counts as standing there. The frame the state is
    /// entered on has run none of it.
    fn update_approach(&mut self, dt: f32, world: &dyn World) {
        let Some(Kick { approach: Some(mut approach), .. }) = self.kick else { return };
        if !std::mem::take(&mut approach.fresh) {
            approach.time += self.frame_dt;
        }
        // The curve eases in and out: flat at both ends.
        let share = if approach.length > 0.0 {
            let s = (approach.time / approach.length).clamp(0.0, 1.0);
            (3.0 * s * s - 2.0 * s * s * s).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.body_yaw = lerp_angle(approach.body_from, approach.look_to[0], share);
        self.yaw = lerp_angle(approach.look_from[0], approach.look_to[0], share);
        self.pitch = lerp_angle(approach.look_from[1], approach.look_to[1], share);
        let motion: [f32; 3] = std::array::from_fn(|k| lerp(approach.from[k], approach.to[k], share) - self.position[k]);
        let size = (motion[0] * motion[0] + motion[1] * motion[1] + motion[2] * motion[2]).sqrt();
        if size > 0.0 {
            self.travel(motion, dt, world, true);
        }
        // The body's animator is told which way it is walked, a tenth of
        // the way there each frame.
        let heading = if size > 0.0 { [motion[0] / size, motion[2] / size] } else { [0.0; 2] };
        let local = self.to_local(heading);
        approach.direction = std::array::from_fn(|k| lerp(approach.direction[k], local[k], 0.1));
        self.movement_direction = approach.direction;
        self.note_rotation();
        let there = approach.time > approach.length;
        if let Some(kick) = &mut self.kick {
            kick.approach = Some(approach);
            kick.approached |= there;
        }
    }

    /// Whether a door is being kicked: from the key until the kick's
    /// state is left.
    pub fn kicking(&self) -> bool {
        self.kick.is_some()
    }

    /// Whether the movement state keeps the inventory screen shut (the
    /// approach to a thing does: `StateLocksInventory`).
    pub fn locks_inventory(&self) -> bool {
        self.state == State::Approach
    }

    /// The kick's own handling of a frame (`BreachDoorState.
    /// ManualAnimatorMoveUpdate`): the body does not turn after the view;
    /// from 0.15 of the way through the state it moves by the clip's own
    /// motion and turn; and 0.45 of the way through, once, the kick lands,
    /// which costs the legs what a jump does (`Physical.OnBreach`).
    fn update_breach(&mut self, dt: f32, world: &dyn World) {
        let through = self.body.time_in(movement_state::BREACH_DOOR).unwrap_or(0.0);
        if through > KICK_MOVES_FROM {
            let motion = self.animated_motion;
            self.apply_motion(motion, dt, world);
            self.body_yaw += self.body.turn();
            self.note_rotation();
        }
        if through >= KICK_LANDS_AT {
            if let Some(kick) = self.kick.as_mut().filter(|kick| !kick.landed) {
                kick.landed = true;
                self.acted.push(Acted::Kicked { opens: kick.opens });
                self.stamina.jump(self.frame_clock);
            }
        }
    }

    /// The character takes up a thing it looks at (`IdlePlayerState.Pickup`,
    /// `PickUpState`). Only standing still: in any other movement state
    /// the game's call does nothing. The view is held, the lean let go,
    /// and with the view more than 60 degrees down the character goes
    /// down to a third of its stance; `Acted::PickedUp` comes 0.6 s in,
    /// and with it the state is over and the stance taken again.
    pub fn pick_up(&mut self) -> bool {
        if self.acting.is_some() || self.state != State::Idle {
            return false;
        }
        self.tilt = 0.0;
        self.set_interaction(Interaction::TakeItem);
        let level = self.pose_level;
        if self.pitch > PICK_UP_CROUCH_PITCH {
            self.change_pose(level / 3.0 - level);
        }
        self.acting = Some(Acting::PickingUp { time: 0.0, level, taken: false });
        true
    }

    /// Whether the movement state lets the player look at a thing to work
    /// it (`MovementContext.CanInteract`, as the interaction's line cast
    /// asks it; interactions spec, "What is looked at"): standing, walking,
    /// lying still or crawling and blind fire do; a sprint, a jump and its
    /// landing, a sidestep, going down or getting up, a vault, a reload or a
    /// turn lying down, and the states for the hands on the world do not.
    /// The thing in hand has its own say (`Firearm::can_interact`).
    pub fn can_interact(&self) -> bool {
        self.can_interact
    }

    /// Whether the character is unlocking a thing of the world or taking
    /// one up: the walk is then refused, and for a pick-up the view too.
    pub fn acting(&self) -> bool {
        self.acting.is_some()
    }

    /// Whether a container has been searched and is not shown yet.
    pub fn searching(&self) -> bool {
        self.searching.is_some()
    }

    /// What came of it since this was last asked.
    pub fn take_acted(&mut self) -> Vec<Acted> {
        std::mem::take(&mut self.acted)
    }

    /// Moves the state for the hands on the world on by a frame of the
    /// engine's clock, which is what the game's two states time by.
    fn act(&mut self, dt: f32) {
        match &mut self.acting {
            Some(Acting::Working { time, worked, snap }) => {
                *time += dt;
                if !*worked {
                    if *time > HAND_LAID_AFTER {
                        *worked = true;
                        self.acted.push(Acted::Worked);
                    }
                } else if !*snap || *time > WORKING_ENDS_AFTER {
                    // The frame after the thing has done its part, or the
                    // state's full length.
                    self.acting = None;
                    self.acted.push(Acted::Done);
                }
            }
            Some(Acting::PickingUp { time, level, taken }) => {
                *time += dt;
                if *time > PICKED_UP_AFTER && !*taken {
                    *taken = true;
                    // The thing is taken, which ends the state: the stance
                    // it began with is taken again.
                    let level = *level;
                    self.acting = None;
                    self.acted.push(Acted::PickedUp);
                    if !self.prone {
                        self.change_pose(level - self.pose_level);
                    }
                }
            }
            None => {}
        }
    }

    /// A character lying down that is to work a thing of the world is no
    /// longer lying, and its stance is put at its lowest
    /// (`StartInteraction`).
    fn toggle_prone_for_interaction(&mut self) {
        self.prone = false;
        self.pose_level = 0.0;
    }

    /// One step of `dt` seconds on a single clock. For a host that runs
    /// the simulation on steps of its own; one that draws frames calls
    /// `frame`.
    pub fn step(&mut self, dt: f32, input: &Input, world: &dyn World) {
        self.step_on_clock(dt, dt, input, world);
    }

    /// One drawn frame of `dt` seconds, timed as the game times it: the
    /// animator by the frame's own length and the movement by a stopwatch
    /// read in whole milliseconds. This is the stepping the recordings
    /// were compared on. A frame the stopwatch does not see pass (under a
    /// millisecond) is added to the next.
    pub fn frame(&mut self, dt: f32, input: &Input, world: &dyn World) {
        self.unstepped += dt;
        let player_dt = self.stopwatch.tick(dt);
        if player_dt > 0.0 {
            let dt = std::mem::take(&mut self.unstepped);
            self.step_on_clock(dt, player_dt, input, world);
        }
    }

    /// One frame that lasted `dt` seconds by the engine's time and
    /// `player_dt` by the player's own clock.
    ///
    /// The game keeps the two apart ("The player's clock" in the movement
    /// spec): its animator runs on the engine's time and the movement
    /// states on a stopwatch read in whole milliseconds, so the two differ
    /// by up to half a millisecond a frame. A host with one clock calls
    /// `step`.
    pub fn step_on_clock(&mut self, dt: f32, player_dt: f32, input: &Input, world: &dyn World) {
        if dt <= 0.0 || player_dt <= 0.0 {
            // The engine's timers still see the frame.
            if dt > 0.0 {
                self.wait_for_search(dt);
            }
            return;
        }
        self.frame_dt = dt;
        // The engine's clock, which the landing's cap and the stamina's
        // pauses are timed by.
        self.frame_clock += dt;
        self.clock_lag_before = self.clock_lag;
        self.clock_lag += dt - player_dt;
        self.frame_count += 1;
        self.jump.turn_rate = 0.0;
        // With a screen open the view is handed no turn at all: not a
        // turn of nothing, which would count among the last frames' turns.
        // Nor while a thing is being taken up: that state does not turn.
        let picking_up = matches!(self.acting, Some(Acting::PickingUp { .. }));
        if !input.screen && !picking_up {
            self.look(if input.free_look { input.look_delta } else { self.limited_turn(input.look_delta) });
        }
        // The head's free look is handed its axes on every frame the keys
        // reach the character, whatever the movement state makes of the
        // turn (`MoveInputTranslator.TranslateAxes`).
        if !input.screen {
            if input.head_reset {
                self.head_look.reset();
            }
            self.head_look.frame(&HeadLookInput {
                travel: input.head_look,
                dt,
                aiming: input.aiming,
                camera_pitch: input.camera_pitch,
                sprinting: self.sprinting,
            });
            self.body.set_head_turn(self.head_look.bones_turn());
        }
        self.hands_idle = input.hands_idle;
        self.act(dt);
        self.commands(input, world);
        self.watch_sprint();
        self.manual_update(player_dt, world);
        self.watch_obstacle(dt);
        // Stamina is worked here, before the animator: a movement state the
        // animator enters in this frame shows in the pools from the next.
        self.work_stamina(player_dt);

        // The animator runs on what the movement set so far. Entering one
        // of its states enters the movement state it names, and this
        // happens when a blend into it starts, not when it ends.
        let entered = self.body.update(dt, &self.body_input());
        for entered in entered {
            if let Some(state) = State::from_number(entered.state) {
                self.enter_state(state, entered);
            }
        }
        // A gesture whose state the animator has left in this update is
        // over with it: the state's own script says so as it is left, and
        // the flag is down in the same frame (recorded).
        for _ in self.body.take_finished_interactions() {
            let shows = self.interactions.finished();
            self.show(shows);
        }
        // The mark of a backpack's drop, passed by the gesture's state.
        // A drop whose gesture is gone without the mark is over too: the
        // game runs such an operation on to its end (`FastForward`).
        let passed = self.body.take_backpack_dropped();
        if self.dropping_backpack && (passed || !self.interactions.has(Interaction::DropBackpack)) {
            self.dropping_backpack = false;
            self.acted.push(Acted::BackpackDropped);
        }
        // The animator sits on the part of the body that is laid to the
        // ground, so its motion comes out turned with it.
        let moved = quat_rotate(self.body_tilt, self.body.motion());
        let across = self.to_world([moved[0], moved[2]]);
        self.animated_motion = [across[0], moved[1], across[1]];

        // The current state's own handling of the frame. The animator calls
        // on a movement state once for each of its own states that names it
        // and had a part in the frame: not at all while none is playing,
        // and twice where one such state gives way to another.
        for _ in 0..self.body.calls(self.state.number()) {
            match self.state {
                State::Idle => self.update_idle(player_dt, world),
                State::Move => self.update_move(player_dt, world),
                State::Sprint => self.update_sprint(player_dt, world),
                State::Jump => self.update_jump(player_dt, world),
                State::SprintStop => self.update_sprint_stop(player_dt, world),
                State::Landing => self.update_landing(player_dt, world),
                State::ToProne => self.update_to_prone(player_dt, world),
                State::ProneIdle => self.update_prone_idle(player_dt, world),
                State::ProneMove => self.update_prone_move(player_dt, world),
                // Getting up is handled as standing is, once the body
                // has been let off the ground it was laid to.
                State::FromProne => {
                    self.rise_from_surface();
                    self.update_idle(player_dt, world);
                }
                State::Sidestep => self.update_sidestep(player_dt, world),
                State::Roll => self.update_roll(player_dt, world),
                State::Vault | State::Climb => self.update_vault(player_dt, world),
                State::VaultFall => self.update_vault_fall(player_dt, world),
                // The landing out of a vault is the jump's.
                State::VaultLanding => self.update_landing(player_dt, world),
                // Set before a door that puts the character nowhere, the
                // approach does nothing to the body at all.
                State::Approach => self.update_approach(player_dt, world),
                State::Breach => self.update_breach(player_dt, world),
            }
        }
        self.watch_sprint();
        self.watch_footsteps(dt);
        self.lag_body_height(player_dt);
        self.wait_for_search(dt);
    }

    /// The game's wait after a search (a timer of the player's): it runs
    /// after the frame's update, so it goes by where the character is once
    /// the frame has moved it, and the frame of the key does not count. It
    /// is over on the first frame at least `thing_answers_after` after the
    /// key's (recorded: the screen open 0.550 to 0.553 s after the key).
    fn wait_for_search(&mut self, dt: f32) {
        let Some((at, from, time, fresh)) = self.searching else { return };
        let time = if fresh { time } else { time + dt };
        if time + 1e-6 < self.settings.thing_answers_after {
            self.searching = Some((at, from, time, false));
            return;
        }
        self.searching = None;
        let there = self.state != State::Jump && self.distance_to(at) - from <= SEARCH_REACH;
        self.acted.push(if there { Acted::Searched } else { Acted::LeftUnsearched });
    }

    /// The stamina's frame (stamina spec, "A pool, each frame"): what is
    /// running is paid for and the pools refill. It comes after the keys
    /// and the stance have been seen to and before the animator, so it
    /// goes by the movement state the last frame ended in and by the
    /// sights as the keys and a sprint begun by them leave them. Legs
    /// that run dry end a sprint, which the sprinting state then sees in
    /// this same frame.
    fn work_stamina(&mut self, player_dt: f32) {
        self.stamina.set_pose(self.prone, self.pose_level);
        self.stamina.moving(self.state == State::Move, self.state == State::ProneMove);
        self.stamina.aim(self.aiming);
        self.stamina.update(player_dt, self.frame_clock);
        if self.sprinting && !self.stamina.sprinting() {
            self.sprinting = false;
        }
    }

    /// The body does not follow the feet up and down at once: it is held
    /// back by what the feet just moved and eased home, never more than
    /// 0.2 m away. Going up it trails below, coming down it rides above.
    /// Lying down nothing is held back, and what is left eases home.
    ///
    /// `dt` is the player's own frame time, the whole milliseconds the
    /// movement states go by, not the engine's (body animator spec, "A
    /// lag in the body's height"; recorded on a frame where the two were
    /// 75 and 8 ms).
    fn lag_body_height(&mut self, dt: f32) {
        /// The share of the catching-up time used while the gap is still
        /// opening.
        const OPENING_SHARE: f32 = 0.3;
        const LIMIT: f32 = 0.2;
        let moved = if self.prone { 0.0 } else { self.position[1] - self.height_before };
        self.height_before = self.position[1];
        let eased = smooth_damp(
            [self.body_height_offset - moved],
            [0.0],
            &mut self.height_lag_velocity,
            self.height_lag_time,
            1_000_000.0,
            dt,
        )[0];
        self.body_height_offset = eased.clamp(-LIMIT, LIMIT);
        let wanted = if eased.abs() > self.height_lag_before { HEIGHT_SMOOTH_TIME * OPENING_SHARE } else { HEIGHT_SMOOTH_TIME };
        self.height_lag_time = lerp(self.height_lag_time, wanted, dt);
        self.height_lag_before = eased.abs();
    }

    /// Leaves the current movement state for another, as the game does
    /// when its animator enters a state that names a different one.
    /// `entered` is that animator state's own word on which part of the
    /// movement it is and for which side.
    fn enter_state(&mut self, state: State, entered: Entered) {
        // Each of the animator's states says how freely the view turns in
        // it, and says so again every time it is entered.
        (self.turn_factor, self.turn_limit) = (entered.turn_factor, entered.turn_limit);
        self.can_interact = entered.can_interact;
        if state == self.state {
            // Another of the animator's states for the same movement
            // state: the game only tells its state object so. The
            // sidestep's starts its run over, for the new state's side.
            if state == State::Sidestep {
                self.sidestep_data.part = entered.part;
                self.sidestep_data.progress = 0.0;
                self.sidestep_data.far = if entered.side < 0.0 { -1.0 } else { 1.0 };
            }
            // The roll's only has to know which part it is in.
            if state == State::Roll {
                self.roll_data.part = entered.part;
            }
            return;
        }
        // Leaving. Standing and the states built on it let a held breath
        // go on the way out, if the sights are up to hold one by.
        if self.state.stands() && self.aiming {
            self.stamina.hold_breath(false);
        }
        match self.state {
            State::Sidestep => {
                // The view turns freely again and the step is no longer
                // asked for. A way in or out leaves the body unstepped;
                // the hold leaves it as it stands.
                self.yaw_limit = None;
                self.step = 0;
                if self.sidestep_data.part != part::HOLD {
                    self.sidestep = 0.0;
                }
            }
            State::Roll => {
                // Rolled back: the view is as free as lying still leaves
                // it, at once, and the roll is no longer asked for.
                self.yaw_limit = None;
                self.pitch_limit = PRONE_PITCH_RANGE;
                self.pitch_target = PRONE_PITCH_RANGE;
                self.step = 0;
            }
            State::Sprint => {
                self.enable_sprint(false);
                self.sprint_flag = false;
                self.reset_speed_after_sprint();
                // A jump out of a sprint keeps the sprint's pace for the
                // landing; anything else puts it back.
                if state != State::Jump {
                    self.sprint_speed = self.settings.starting_sprint_speed;
                }
            }
            State::Jump => {
                self.tilt = 0.0;
                // A landing caps the selected speed for a while.
                let s = &self.settings;
                let limit = ((1.0 - s.jump_penalty) * s.max_speed).max(0.0);
                self.limit_after_landing(limit, s.jump_penalty_duration);
            }
            State::Vault => {
                self.do_vault = false;
                // Out of a sprint, the sprint is asked for again and the
                // move is over at once; otherwise it is over when the
                // landing is left.
                if self.vault.from_sprint && !self.blocked {
                    self.enable_sprint(true);
                    self.is_vaulting = false;
                }
            }
            State::Climb => {
                self.do_climb = false;
                self.is_vaulting = false;
            }
            State::VaultLanding => self.is_vaulting = false,
            // The approach is over: its flag goes down with it
            // (`ApproachState.Exit`).
            State::Approach => {
                if let Some(kick) = &mut self.kick {
                    kick.approached = false;
                }
            }
            // The kick is over (`BreachDoorState.Exit`, `StopAnyInteractions`).
            State::Breach => {
                self.kick = None;
                self.acted.push(Acted::Done);
            }
            State::FromProne => {
                // Up again: the view and the body's turning are free as
                // they were, at once.
                self.pitch_limit = UPRIGHT_PITCH_RANGE;
                self.pitch_target = UPRIGHT_PITCH_RANGE;
                self.yaw_limit = None;
                self.trunk_limit = self.settings.trunk_rotation_limit;
                self.stand_upright();
            }
            _ => {}
        }
        let from = self.state;
        self.set_state(state);
        self.aim_lowered |= self.settings.no_aim_states.contains(&state.number());
        // Entering.
        match state {
            State::Idle => {
                self.enable_sprint(false);
                self.idle_sprint_held = self.sprint_input;
            }
            State::ToProne => {
                self.enable_sprint(false);
                self.trunk_limit = PRONE_TRUNK_LIMIT;
                self.pitch_target = PRONE_PITCH_RANGE;
            }
            State::ProneIdle => {
                self.pose_level = 0.0;
                self.smoothed_pose_level = 0.0;
                self.pitch_target = PRONE_PITCH_RANGE;
                self.pending_step = 0;
            }
            State::Roll => {
                self.enable_sprint(false);
                self.pose_level = 0.0;
                self.smoothed_pose_level = 0.0;
                // The view is held around where it points now, and may
                // pitch much less far down; the body turns to that bearing.
                self.roll_bearing = self.yaw;
                self.yaw_limit = Some((self.yaw, self.trunk_limit - 1.0));
                self.pitch_target = ROLL_PITCH_RANGE;
                self.roll_yaw = self.body_yaw;
                self.inert = false;
                let s = &self.settings;
                let data = &mut self.roll_data;
                data.part = entered.part;
                data.speed = s.sideback_speed * if data.stepped_out { s.inertia_backward_coef } else { 1.0 };
                self.sideback_speed = data.speed;
            }
            State::ProneMove => {
                std::mem::swap(&mut self.mv, &mut self.mv_prone);
                self.enter_move(from);
                std::mem::swap(&mut self.mv, &mut self.mv_prone);
                self.yaw_limit = None;
                self.pose_level = 0.0;
                self.smoothed_pose_level = 0.0;
            }
            State::FromProne => self.enable_sprint(false),
            State::Sidestep => {
                self.enable_sprint(false);
                // The view may turn only so far from the way the body
                // faces, as the body's angle to it was last noted.
                let reach = self.trunk_limit - 1.0;
                self.yaw_limit = Some((self.yaw - self.hands_to_body, reach));
                self.pitch_limit = UPRIGHT_PITCH_RANGE;
                self.pitch_target = UPRIGHT_PITCH_RANGE;
                self.tilt = 0.0;
                let far = if entered.side < 0.0 { -1.0 } else { 1.0 };
                let data = &mut self.sidestep_data;
                (data.part, data.progress, data.far) = (entered.part, 0.0, far);
                self.sidestep = if entered.part == part::IN { 0.0 } else { far };
                // The state's own playback speed goes to where it rests,
                // which is lower if it last worked a way out.
                let s = &self.settings;
                data.speed = s.sideback_speed * if data.stepped_out { s.inertia_backward_coef } else { 1.0 };
                self.sideback_speed = data.speed;
            }
            State::Move => self.enter_move(from),
            State::Sprint => {
                // The sprint is a state object of its own in the game, so
                // what the walking state holds stays as the walk left it.
                self.mv.smoothed_yaw = self.body_yaw;
                self.direction = self.movement_direction;
                self.pose_level = 1.0;
                self.tilt = 0.0;
                // An overweight sprint turns slower still.
                self.turn_factor *= self.settings.sprint_sensitivity;
                self.turn_limit *= self.settings.sprint_sensitivity;
            }
            State::Jump => self.enter_jump(from),
            State::SprintStop => {
                self.sprint_flag = false;
                self.tilt = 0.0;
                self.mv.smoothed_yaw = self.body_yaw;
                self.turn_factor *= self.settings.sprint_sensitivity;
                self.turn_limit *= self.settings.sprint_sensitivity;
            }
            State::Vault => self.enter_vault(Move::Vault, from),
            State::Climb => self.enter_vault(Move::Climb, from),
            State::VaultFall => {}
            // The approach lets the lean go and takes the character out of
            // its lying pose (`ApproachState.Enter`).
            State::Approach => {
                self.tilt = 0.0;
                self.prone = false;
                self.enable_sprint(false);
                self.begin_approach();
            }
            State::Breach => {}
            State::Landing | State::VaultLanding => {
                self.landing_idle_time = 0.0;
                self.landing_still = false;
                // The landing has been handed no input yet on the frame
                // it starts, so that frame counts as one with none.
                self.direction = [0.0; 2];
            }
        }
        // Every change of state sets the cap on actual speed for the new
        // state and puts the selected speed back to the wheel's setting,
        // which the wind-up before a sprint had raised.
        self.speed_cap = self.hard_speed_cap(state);
        self.select_speed(self.relative_speed * self.settings.max_speed);
    }

    // ------------------------------------------------------------ input

    /// What the movement state lets through of a frame's turn of the view
    /// ("Looking" in the movement spec): scaled, then cut to a length.
    fn limited_turn(&self, delta: [f32; 2]) -> [f32; 2] {
        if self.turn_limit <= 0.0 {
            return [0.0; 2];
        }
        let turn = delta.map(|d| d * self.turn_factor);
        let size = length(turn);
        if size > self.turn_limit { turn.map(|d| d * self.turn_limit / size) } else { turn }
    }

    /// Where a turn of `delta` leaves the view: yaw, then pitch.
    fn turned(&self, delta: [f32; 2]) -> [f32; 2] {
        let mut yaw = self.yaw + delta[0];
        // Lying still, the view turns only so far from where the body lay
        // a frame ago; stepped aside, only so far from where the body
        // faced when the step began.
        if let Some((centre, reach)) = self.yaw_limit {
            yaw = centre + delta_angle(centre, yaw).clamp(-reach, reach);
        }
        // The range is measured from the ground the body is laid to.
        let range = self.pitch_limit.map(|limit| limit + self.surface_pitch);
        [yaw, (self.pitch + delta[1]).clamp(range[0], range[1])]
    }

    /// Where the view will point once the next frame has turned it by
    /// `delta`, yaw then pitch, if the movement state stays as it is.
    ///
    /// A host that draws more often than the simulation's frames come
    /// shows this for the mouse's travel no frame has had yet, so that the
    /// view turns at once. It has to go through the state's factor and
    /// limits as the frame's turn will: shown whole, the travel would be
    /// taken back when a frame in a sprint, a jump or lying down lets only
    /// part of it through, and the view would shake.
    pub fn look_ahead(&self, delta: [f32; 2]) -> [f32; 2] {
        let [yaw, pitch] = self.turned(self.limited_turn(delta));
        [yaw, self.pitch_as_shown(pitch)]
    }

    fn look(&mut self, delta: [f32; 2]) {
        [self.yaw, self.pitch] = self.turned(delta);
        self.yaw_deltas.push_back(delta[0]);
        while self.yaw_deltas.len() > self.settings.rotation_frame_span {
            self.yaw_deltas.pop_front();
        }
    }

    fn commands(&mut self, input: &Input, world: &dyn World) {
        self.aiming = input.aiming;
        // The body's flag for a reload is set at a reload's start and end
        // and at the inventory's opening and shutting, whichever came last.
        if input.reloading != self.reloading {
            self.prone_reload = input.reloading;
        }
        self.reloading = input.reloading;
        self.hands_busy = input.hands_busy;
        if input.inventory != self.inventory {
            self.inventory = input.inventory;
            self.prone_reload = input.inventory;
            // Opening and shutting both switch a held vault key off.
            self.vault_on = false;
            self.vault_time = 0.0;
            // Opening stops the hands' gestures where they are, all but
            // those that are operations (`Player.SetInventoryOpened`).
            if input.inventory {
                let shows = self.interactions.force_stop();
                self.show(shows);
            }
        }
        // The body's own pose for an open inventory: taken when the hands
        // have gone down for it, and given up for as long as they are
        // doing something else meanwhile (`Player.WaitWeaponInventory`).
        if !input.inventory {
            (self.body_inventory, self.inventory_reached) = ([false; 2], false);
        } else {
            if input.hands_in_inventory && !self.inventory_reached {
                self.inventory_reached = true;
                self.body_inventory[0] = true;
            }
            if self.inventory_reached {
                self.body_inventory[1] = !input.hands_in_inventory;
            }
        }
        // The game's states for unlocking a thing and for taking one up
        // take no walk and no jump. PROVISIONAL: they are not movement
        // states of their own here; the keys are noted and not heard, as
        // under a screen, so a walk runs out as with the keys let go
        // (the game's body stops for the approach), and a stance or a
        // lean asked for meanwhile, which the game's state for a door
        // takes, is dropped. (The pick-up's is compared with a recording:
        // 0.61 s, the view held, a third of the stance; interactions
        // spec.)
        if input.screen || self.acting.is_some() {
            self.unheard(input);
        } else {
            self.heard(input);
        }
        // Neither of these is a key's doing. The sights coming down let a
        // held breath go whatever the state; and the vault key's try is
        // made on every frame it is switched on, timed by the engine's
        // clock. Standing and the states built on it try (lying still and
        // rolled over do not), and so do the walk, the crawl and the sprint.
        if !self.aiming {
            self.stamina.hold_breath(false);
        }
        if self.vault_on {
            let asks = (self.state.stands() && !matches!(self.state, State::ProneIdle | State::Roll))
                || matches!(self.state, State::Move | State::ProneMove | State::Sprint);
            if asks {
                self.try_vaulting(input, world);
            }
            self.vault_time += self.frame_dt;
        }
    }

    /// A frame in which a screen has the keys: nothing is handed over.
    /// Each key is noted as it stands, so that what it did meanwhile is
    /// lost and not made up for when the screen shuts. The walking, the
    /// crawling and the sprinting state are left with the input their
    /// last update left them, which is none; a jump keeps its direction;
    /// standing, nothing is asked at all.
    fn unheard(&mut self, input: &Input) {
        self.step_key = input.step;
        self.lean_keys = lean_keys(input);
        self.sprint_input = input.sprint;
        self.vault_key = input.vault;
    }

    /// A lean asked of the movement state in hand (`MovementState.SetTilt`
    /// and what overrides it). A jump, a sprint and its stop, a step aside
    /// and a roll do not listen: the lean they put to zero as they began
    /// stays, key or no key. A crawl takes any request as one for none.
    fn ask_tilt(&mut self, tilt: f32) {
        match self.state {
            State::Jump | State::Sprint | State::SprintStop | State::Sidestep | State::Roll => {}
            State::ProneMove => self.tilt = 0.0,
            _ => self.tilt = tilt.clamp(-MAX_TILT, MAX_TILT),
        }
    }

    /// A frame's keys, in the order the game takes them.
    fn heard(&mut self, input: &Input) {
        if input.speed_delta != 0.0 && self.state != State::Sprint {
            self.select_speed(self.clamped_speed() + input.speed_delta);
        }
        // The walk key (`Player.ToggleMinSpeed`): a speed above none is
        // put aside and the speed put to none, which walks at the floor
        // under a walk's pace; at none already, what was put aside is
        // taken up again. The key coming up, where it is bound to be held,
        // only takes it up again (`RestoreSpeed`).
        if input.walk_toggle && self.clamped_speed() >= f32::EPSILON {
            self.speed_memo = self.character_speed;
            self.change_speed(-self.speed_memo);
        } else if input.walk_toggle || input.walk_end {
            if self.clamped_speed() < f32::EPSILON {
                self.change_speed(self.speed_memo - self.character_speed);
            }
        }
        if input.prone {
            self.prone_command();
        }
        if input.crouch {
            // Back to the stance last chosen with the stance keys, if the
            // stance is not there now (the key's own crouch, or a sprint
            // that stood the character up); from there, between crouched
            // and standing. The key does not itself count as choosing.
            // Lying down it asks for no change at all, and what that does
            // is the state's.
            let target = if self.prone {
                self.pose_level
            } else if (self.pose_memo - self.pose_level).abs() < 1e-6 {
                if self.pose_level >= 0.5 { 0.0 } else { 1.0 }
            } else {
                self.pose_memo
            };
            self.change_pose(target - self.pose_level);
        }
        // The crouch key coming up, where it is bound to be held
        // (`Player.RestorePose`): a character lower than half way is put
        // back to the stance last chosen, or upright if that was as low.
        if input.restore_pose && !self.prone && self.pose_level < 0.5 {
            let to = if self.pose_memo < 0.5 { 1.0 } else { self.pose_memo };
            self.change_pose(to - self.pose_level);
        }
        if input.pose_delta != 0.0 {
            self.change_pose(input.pose_delta);
            self.pose_memo = self.pose_level;
        }
        // The lean keys are presses and releases (`Player.ToggleLean`,
        // `Straighten`). A key let go straightens the character if it
        // still leans to that key's side; a key pressed leans it to its
        // side, or straightens it if it leans that way already. Nothing
        // holds the lean to a key in between.
        // Neither does anything on a frame after one in which the lean
        // was made slowly (`_lastSlowLean`).
        let keys = lean_keys(input);
        let sides = [(0, -1.0f32), (1, 1.0)];
        for (key, side) in sides {
            if ((self.lean_keys[key] && !keys[key]) || input.lean_end[key]) && !self.last_slow_lean && self.tilt * side > 0.0 {
                self.ask_tilt(0.0);
            }
        }
        for (key, side) in sides {
            if ((!self.lean_keys[key] && keys[key]) || input.lean_toggle[key]) && !self.last_slow_lean {
                self.slow_leaning = false;
                self.ask_tilt(if self.tilt * side > 0.0 { 0.0 } else { side * MAX_TILT });
            }
        }
        self.lean_keys = keys;
        // The step-aside key: pressed, it asks for the step to its side,
        // or takes it back if that is what is already asked for; let go,
        // it takes its own side's step back.
        if input.step != self.step_key {
            let before = std::mem::replace(&mut self.step_key, input.step);
            if before != 0 && self.step == before {
                self.set_step(0);
            }
            if input.step != 0 {
                self.set_step(if self.step == input.step { 0 } else { input.step });
            }
        }
        // The same as commands (`Player.ToggleStep`, `ReturnFromStep`).
        for (at, side) in [(0, -1), (1, 1)] {
            if input.step_end[at] && self.step == side {
                self.set_step(0);
            }
            if input.step_toggle[at] {
                self.set_step(if self.step == side { 0 } else { side });
            }
        }
        // The key press is a toggle command and the release an explicit stop.
        if input.sprint != self.sprint_input {
            self.sprint_input = input.sprint;
            if input.sprint {
                self.sprint_command(!self.sprinting, true);
            } else {
                self.sprint_command(false, false);
            }
        }
        // The two as commands (`Player.ToggleSprint`, `EnableSprint`).
        if input.sprint_toggle {
            self.sprint_command(!self.sprinting, true);
        }
        if input.sprint_end {
            self.sprint_command(false, false);
        }
        // A toggle pressed before the character is moving forward is ignored
        // by the movement states, and nothing repeats it: in a recorded run
        // with sprint held first and forward pressed after, the game walks.
        self.direction = input.move_dir;
        self.input_direction = input.move_dir;
        match self.state {
            State::Jump => self.jump.move_dir = input.move_dir,
            state if state.stands() => self.idle_move(input.move_dir),
            _ => {}
        }
        // The slow lean's axis (`Player.SlowLean`), which the game hands
        // out with the walk's, after the frame's commands: pushed, and
        // with the body's lean within a degree of the one asked for, it
        // asks the state in hand for a tenth of a degree more that way.
        // A frame, not a time: the lean goes as fast as frames come.
        self.last_slow_lean = false;
        if (self.smoothed_tilt - self.tilt).abs() < 1.0 && input.slow_lean != 0.0 {
            self.slow_leaning = true;
            self.ask_tilt(self.tilt + input.slow_lean * SLOW_LEAN_STEP);
            self.last_slow_lean = true;
        }
        // The jump is refused once the vault key has been held past its
        // time, and while a vault is under way (`Player.Jump`). In the
        // game's default binding the two are one key, and the jump comes
        // when it is let go.
        let jump = input.jump && self.vault_time <= self.settings.vaulting.input_time && !self.is_vaulting;
        if jump && matches!(self.state, State::ProneIdle | State::ProneMove) {
            // Lying down, jump gets up.
            self.prone_command();
        } else if jump && matches!(self.state, State::Idle | State::Move | State::Sprint) {
            if self.pose_level > JUMP_POSE_THRESHOLD && self.grounded {
                // The animator takes it from here: its jump state starts
                // the jump proper. Legs at or under the threshold do not
                // jump, and nothing else happens for the key.
                if self.stamina.can_jump() {
                    self.jump_flag = true;
                }
            } else {
                // Too low to jump: stand up instead.
                self.change_pose(1.0 - self.pose_level);
            }
        } else if jump && self.state == State::Sidestep {
            // Stepped aside, jump only takes the step back.
            self.set_step(0);
            self.sidestep = 0.0;
        } else if jump && self.state == State::Roll {
            // Rolled over, the same.
            self.set_step(0);
        }
        // The hold-breath key: a press holds the breath or lets it go.
        // Only the standing state and those built on it listen
        // (`IdlePlayerState.EnableBreath`), and only with the sights up
        // (`MovementContext.HoldBreath`).
        if input.hold_breath && self.state.stands() && self.aiming {
            let held = self.stamina.holding_breath();
            self.stamina.hold_breath(!held);
        }
        // The key coming up, where it is bound to be held, lets a held
        // breath go (`Player.StopHoldingBreath`).
        if input.breath_end && self.state.stands() && self.aiming && self.stamina.holding_breath() {
            self.stamina.hold_breath(false);
        }
        // The vault key, pressed or let go, switches its try over
        // (`Player.Vaulting`); switched off, the hold's time is cleared.
        if input.vault != self.vault_key {
            self.vault_key = input.vault;
            self.vault_on = !self.vault_on;
            if !self.vault_on {
                self.vault_time = 0.0;
            }
        }
    }

    /// Asks for a step aside (1 right, -1 left) or takes it back (0), as
    /// the movement state in hand allows. Standing still takes it and the
    /// animator steps; stepped aside it is taken unless the step back is
    /// already under way. The walk, the sprint, the crawl and a jump do
    /// not listen. Lying still it is the roll: the lean is put to zero and
    /// the step waits for it to straighten, unless it already leaned that
    /// way; rolled over it is taken unless the roll back is under way.
    /// (The game also refuses a step with something in the way or no
    /// ground to that side; there is nothing here to be in the way.)
    fn set_step(&mut self, step: i32) {
        match self.state {
            State::Idle => self.step = step,
            State::Sidestep if self.sidestep_data.part != part::OUT => self.step = step,
            State::ProneIdle if step != 0 => {
                if (step < 0 && self.tilt < 0.0) || (step > 0 && self.tilt > 0.0) {
                    self.step = step;
                } else {
                    self.pending_step = step;
                }
                self.tilt = 0.0;
            }
            // Taking it back does not take back a step still waiting.
            State::ProneIdle => self.step = 0,
            State::Roll if self.roll_data.part != part::OUT => self.step = step,
            _ => {}
        }
    }

    /// Standing still, a movement key only tells the animator to start
    /// walking; the walk itself begins when the animator's walking state does.
    fn idle_move(&mut self, d: [f32; 2]) {
        if d[0].abs() <= MIN_INPUT && d[1].abs() <= MIN_INPUT {
            self.movement_direction = [0.0; 2];
            return;
        }
        let signs = [if d[0] == 0.0 { 0.0 } else { d[0].signum() }, if d[1] == 0.0 { 0.0 } else { d[1].signum() }];
        self.movement_direction = [signs[0] * 0.01, signs[1] * 0.01];
        // Locked against an obstacle, nothing starts.
        if self.blocked {
            return;
        }
        // The states built on standing each keep their own note of a held
        // sprint key, and only standing's is ever set.
        self.enable_sprint(self.state == State::Idle && self.idle_sprint_held && signs[1] > 0.1);
        if self.sprinting {
            self.pose_level = 1.0;
            if self.pose_level > SPRINT_POSE_THRESHOLD && self.smoothed_speed >= 1.0 {
                self.sprint_flag = true;
            }
        }
        self.inert = true;
    }

    fn sprint_command(&mut self, enable: bool, is_toggle: bool) {
        match self.state {
            // Standing still, a toggle does nothing.
            State::Idle => {
                if !is_toggle {
                    self.idle_sprint_held = enable;
                }
            }
            State::Move => {
                if self.movement_direction[1] > 0.1 {
                    self.enable_sprint(enable);
                    // Only a sprint that started stands the character up.
                    if self.sprinting {
                        self.pose_level = 1.0;
                    } else {
                        self.reset_speed_after_sprint();
                    }
                } else if !is_toggle {
                    self.mv.sprint_postponed = enable;
                }
            }
            State::Sprint => self.enable_sprint(enable),
            State::Jump => self.jump.from_sprint &= enable,
            // Crawling, the key winds the speed setting up instead.
            State::ProneMove => self.prone_sprint = enable,
            State::SprintStop
            | State::Landing
            | State::ToProne
            | State::ProneIdle
            | State::FromProne
            | State::Sidestep
            | State::Roll
            | State::Vault
            | State::Climb
            | State::VaultFall
            | State::VaultLanding
            | State::Approach
            | State::Breach => {}
        }
    }

    /// The prone key: down from anything upright, up from lying. The
    /// player of an online raid refuses the change both ways with a
    /// reload in hand, and still sets the stance of a lying character to
    /// the one last chosen; the offline player takes it at any time.
    fn prone_command(&mut self) {
        // The approach to a thing does not hear the key at all.
        if self.state == State::Approach {
            return;
        }
        let refused = self.reloading && self.settings.prone_key_refused_in_reload;
        if matches!(self.state, State::ProneIdle | State::ProneMove | State::Roll) {
            if !refused {
                // Getting up is paid for at the key.
                if self.prone {
                    self.stamina.stand_up(self.frame_clock);
                }
                self.prone = false;
            }
            self.pose_level = self.pose_memo.clamp(0.0, 1.0);
        } else if !refused {
            // The animator takes it from here, when it can.
            self.prone = true;
        }
    }

    /// The character turns to something to open it (`ApproachState`;
    /// inventory spec, "Opening a container"): a lean is let go, and a
    /// thing reached for more than 0.65 m under the ribcage is crouched
    /// to, the stance held before being kept for `let_go`. `height` is
    /// where the thing is reached for, in the world. Lying down nothing
    /// is approached. Not in this: the character's walk and turn to the
    /// place the thing is opened from, and the state of its own the game
    /// has for the while.
    pub fn approach(&mut self, height: f32) {
        /// How far under the ribcage a thing is crouched to.
        const LOWER: f32 = 0.65;
        if self.prone {
            return;
        }
        self.tilt = 0.0;
        if self.ribcage()[1] - height > LOWER && self.pose_level > 0.0 {
            self.approach_level = Some(self.pose_level);
            self.change_pose(-self.pose_level);
        }
    }

    /// What was approached is let go: the stance held before it is taken
    /// again (`Player.TryInteractionCallback`).
    pub fn let_go(&mut self) {
        if let Some(level) = self.approach_level.take().filter(|_| !self.prone) {
            self.change_pose(level - self.pose_level);
        }
    }

    fn change_pose(&mut self, delta: f32) {
        match self.state {
            State::Sprint => {
                // Crouching out of a sprint. The slide itself is not implemented.
                if delta < 0.0 {
                    self.pose_level = 0.0;
                    self.enable_sprint(false);
                    self.inert = false;
                }
            }
            // In a vault or a climb the stance is not to be changed.
            State::Vault | State::Climb => {}
            // Lying still, any change of stance gets up into a full crouch;
            // crawling, into the stance held before.
            State::ProneIdle | State::Roll => {
                self.prone_command();
                // Not if the command was refused and the character lies on.
                if !self.prone {
                    self.pose_level = 0.0;
                    self.smoothed_pose_level = 0.0;
                }
            }
            State::ProneMove => self.prone_command(),
            // Walking, a stance too low to sprint from ends the sprint's
            // wind-up.
            State::Move => {
                self.pose_level = (self.pose_level + delta).clamp(0.0, 1.0);
                if self.pose_level < SPRINT_POSE_THRESHOLD {
                    self.enable_sprint(false);
                }
            }
            // Everywhere else the stance changes and nothing more: in the
            // air too.
            State::Idle
            | State::Jump
            | State::SprintStop
            | State::Landing
            | State::ToProne
            | State::FromProne
            | State::Sidestep
            | State::VaultFall
            | State::VaultLanding
            | State::Approach
            | State::Breach => self.pose_level = (self.pose_level + delta).clamp(0.0, 1.0),
        }
    }

    fn reset_speed_after_sprint(&mut self) {
        self.select_speed(self.settings.max_speed);
    }

    /// Follows the character's height while it is off the ground, and on
    /// the frame it finds the ground again charges it for the drop from the
    /// highest point: a fall of some height caps the selected speed, lower
    /// and for longer the further it was.
    fn watch_fall(&mut self) {
        let height = self.position[1];
        if self.grounded {
            let fallen = self.fall_from - height;
            self.fall_from = height;
            let s = &self.settings;
            if !self.was_grounded && fallen >= s.fall_threshold {
                let (limit, duration) = (s.fall_speed_limit.evaluate(fallen), s.fall_limit_duration.evaluate(fallen));
                if duration >= f32::EPSILON && limit < 1.0 {
                    self.limit_after_landing(limit, duration);
                }
            }
        } else if height > self.fall_from {
            self.fall_from = height;
        }
        self.was_grounded = self.grounded;
    }

    /// Caps the selected speed for a time. With a cap already running the
    /// lower of the two and the later end are kept.
    fn limit_after_landing(&mut self, limit: f32, duration: f32) {
        let (before, until) = self.landing_limit.unwrap_or((f32::MAX, 0.0));
        self.landing_limit = Some((limit.min(before), (self.frame_clock + duration).max(until)));
        self.limits_changed = true;
    }

    /// The game's once-a-frame look at its speed limits: a change made
    /// since the last look takes effect, and then a limit whose time is up
    /// is dropped, which is itself a change for the next look.
    fn work_in_limits(&mut self) {
        if std::mem::take(&mut self.limits_changed) {
            self.landing_limit_in_effect = self.landing_limit.map(|(limit, _)| limit);
        }
        if self.landing_limit.is_some_and(|(_, until)| self.frame_clock >= until) {
            self.landing_limit = None;
            self.limits_changed = true;
        }
    }

    // ---------------------------------------------------- per-frame upkeep

    fn manual_update(&mut self, dt: f32, world: &dyn World) {
        let ground = world.footing(self.position);
        // The check reaches down from the capsule, whose underside is at
        // the feet but in a vault or a climb. A character its controller
        // had to push up out of something is on the ground too, whatever
        // is under its axis: a foot that rests on a ledge's edge.
        let capsule = self.capsule();
        self.grounded =
            self.position[1] + capsule.underside - ground <= self.ground_check || world.held(self.position, capsule);
        self.watch_fall();
        self.smooth_pose(dt);
        if self.sprinting {
            self.pre_sprint_acceleration(dt);
        }
        self.work_in_limits();
        self.smooth_tilt(dt);
        // The view's pitch range closes in, or opens, a share at a time.
        let gap = length([self.pitch_limit[0] - self.pitch_target[0], self.pitch_limit[1] - self.pitch_target[1]]);
        self.pitch_limit = if gap < 0.1 {
            self.pitch_target
        } else {
            [0, 1].map(|i| lerp(self.pitch_limit[i], self.pitch_target[i], dt * PITCH_LIMIT_SPEED))
        };
    }

    fn smooth_pose(&mut self, dt: f32) {
        let s = &self.settings;
        let diff = self.pose_level - self.smoothed_pose_level;
        if diff == 0.0 {
            return;
        }
        if diff.abs() > 0.001 {
            // Spent legs change the stance as slowly as a character at
            // nine tenths overweight.
            let weight = s.overweight.max(if self.stamina.legs.exhausted() { 0.9 } else { 0.0 });
            let range = if diff > 0.0 { s.pose_increase_range } else { s.pose_decrease_range };
            let rate = lerp(range[0], range[1], weight);
            let before = self.smoothed_pose_level;
            self.smoothed_pose_level = lerp(before, self.pose_level, dt * s.pose_changing_speed * rate);
            // A rise is paid for, by the third of a notch.
            let rise = self.smoothed_pose_level - before;
            if rise > 0.0 {
                self.stamina.stance_raised(rise, self.frame_clock);
            }
        } else {
            self.smoothed_pose_level = self.pose_level;
        }
    }

    /// Winds the speed up to full before the sprint proper starts.
    fn pre_sprint_acceleration(&mut self, dt: f32) {
        if self.movement_direction[1] < 0.1 {
            self.enable_sprint(false);
        } else if self.smoothed_speed < 1.0 {
            self.smoothed_speed = (self.smoothed_speed + dt * self.settings.pre_sprint_acceleration).clamp(0.0, 1.0);
            self.character_speed = self.smoothed_speed;
        }
    }

    fn smooth_tilt(&mut self, dt: f32) {
        let s = &self.settings;
        let diff = (self.smoothed_tilt - self.tilt).abs();
        if diff < f32::EPSILON {
            return;
        }
        // A lean made slowly is followed by the older rule
        // (`LegacySmoothTilt`): a share of what is left each frame.
        if self.slow_leaning {
            self.smoothed_tilt =
                if diff > 0.001 { self.smoothed_tilt + (self.tilt - self.smoothed_tilt) * (dt * s.tilt_changing_speed).clamp(0.0, 1.0) } else { self.tilt };
            return;
        }
        let max = s.tilt_max_speed;
        if self.prev_tilt != self.tilt {
            // The target changed: shed the current motion before adopting it.
            if self.tilt_multiplier.abs() <= s.tilt_inertia {
                self.prev_tilt = self.tilt;
                self.tilt_multiplier = 0.0;
            } else {
                self.tilt_multiplier =
                    (self.tilt_multiplier - s.tilt_inertia * sign(self.tilt_multiplier)).clamp(-max, max);
            }
        } else {
            let room = self.tilt_distance_to_stop(dt);
            self.tilt_multiplier = if self.tilt == 0.0 {
                self.tilt_multiplier - s.tilt_inertia * sign(room * self.smoothed_tilt) * s.inertia_backward_coef
            } else {
                self.tilt_multiplier + s.tilt_inertia * sign(room)
            }
            .clamp(-max, max);
        }
        let step = TILT_STEP * self.tilt_multiplier * sign(self.prev_tilt);
        if diff > (step * dt).abs() {
            self.smoothed_tilt = (self.smoothed_tilt + step * dt).clamp(-MAX_TILT, MAX_TILT);
        } else {
            self.smoothed_tilt = self.tilt;
            self.tilt_multiplier = 0.0;
        }
    }

    /// How much lean remains beyond what braking from the current rate would cover.
    fn tilt_distance_to_stop(&self, dt: f32) -> f32 {
        let s = &self.settings;
        let mut covered = 0.0;
        let mut m = self.tilt_multiplier;
        while m > 0.0 {
            covered += TILT_STEP * m * dt;
            m = (m - s.tilt_inertia).clamp(0.0, s.tilt_max_speed);
            if s.tilt_inertia <= 0.0 {
                break;
            }
        }
        (self.smoothed_tilt - self.tilt).abs() - covered + 0.1
    }

    // ------------------------------------------------------------- states

    /// Takes the angle from the body round to the view, and the view's
    /// pitch, as they now stand. The game does this whenever it turns the
    /// body, which is inside a state's update: what the animator and the
    /// next frame's follow see is therefore a frame behind the view.
    pub fn note_rotation(&mut self) {
        self.hands_to_body = body_to_view(self.body_yaw, self.yaw);
        self.aim_angle = self.pitch - self.surface_pitch;
    }

    /// The body turns only once the look direction has left its
    /// comfortable range, going by the angle as last noted.
    fn turn_body(&mut self, dt: f32) {
        let angle = self.hands_to_body;
        let limit = self.trunk_limit;
        if angle.abs() > limit {
            let excess = delta_angle(sign(angle) * limit, angle);
            // The engine's blend of the two rotations, not a share of
            // the angle; the two part only for a large excess.
            let step = lerp_turn(0.0, excess, 30.0 * dt);
            // The game's body is a rotation, which takes a turn far
            // smaller than a bearing in single precision can show. Kept
            // as finely here, the following runs on until the angle
            // reads the limit itself, as the game's does, where a
            // bearing would stall a step short of it.
            if self.body_yaw_fine as f32 != self.body_yaw {
                self.body_yaw_fine = f64::from(self.body_yaw);
            }
            self.body_yaw_fine += f64::from(step);
            self.body_yaw = self.body_yaw_fine as f32;
            self.note_rotation();
        }
    }

    fn update_idle(&mut self, dt: f32, world: &dyn World) {
        self.turn_body(dt);
        // Standing has no motion of its own, but while a walk blends out
        // the animator still supplies some; that is mixed with what
        // carries over from the last motion, and fades.
        let keep = self.settings.idle_motion_preservation;
        let motion = [0, 1, 2].map(|i| lerp(self.animated_motion[i], self.last_blend_velocity[i] * dt, keep));
        self.apply_motion(motion, dt, world);
        // A turn on the spot is the animation's own: its clip swings the
        // body a quarter turn.
        self.body_yaw += self.body.turn();
        self.note_rotation();
    }

    /// Stepped aside, or on the way there or back: standing's handling of
    /// the frame, then the step's playback speed and how far the body
    /// counts as stepped (movement spec, "The sidestep").
    fn update_sidestep(&mut self, dt: f32, world: &dyn World) {
        self.update_idle(dt, world);
        self.sideback_speed = self.sidestep_data.play(&self.settings, dt);

        // The value for this frame comes from the run so far; the frame's
        // own time is added after.
        let data = &mut self.sidestep_data;
        let length = self.body.sidestep_length();
        let run = if length > 0.0 { (data.progress / length).clamp(0.0, 1.0) } else { 1.0 };
        self.sidestep = match data.part {
            part::IN => lerp(0.0, data.far, run),
            part::OUT => lerp(data.far, 0.0, run),
            _ => data.far,
        };
        data.progress += dt;
    }

    /// Rolled onto one side, or on the way there or back ("The roll" in
    /// the movement spec): the clip's motion as lying still takes it, the
    /// body's yaw eased to the bearing the view had when the roll began
    /// with whatever the clip turns the root this frame on top, and the
    /// roll's playback speed. The body is not laid to the ground meanwhile
    /// and does not follow the view.
    fn update_roll(&mut self, dt: f32, world: &dyn World) {
        let motion = self.animated_motion;
        self.fall_and_move(motion, dt, world);
        self.roll_yaw = lerp_angle(self.roll_yaw, self.roll_bearing, ROLL_YAW_SPEED * dt);
        self.body_yaw = self.roll_yaw + self.body.turn();
        self.note_rotation();
        self.sideback_speed = self.roll_data.play(&self.settings, dt);
    }

    /// Going down: the clip's motion as it is, with none of the carry-over
    /// standing has.
    fn update_to_prone(&mut self, dt: f32, world: &dyn World) {
        self.turn_body(dt);
        let motion = self.animated_motion;
        self.fall_and_move(motion, dt, world);
        self.body_yaw += self.body.turn();
        self.note_rotation();
        self.align_to_surface(dt, world);
    }

    /// Lying still.
    fn update_prone_idle(&mut self, dt: f32, world: &dyn World) {
        let motion = self.animated_motion;
        self.fall_and_move(motion, dt, world);
        // Lying down the angle is taken afresh before the body follows.
        self.note_rotation();
        self.turn_body(dt);
        // The view may lead the body only so far, measured from where the
        // body lies now.
        self.yaw_limit = Some((self.body_yaw, PRONE_YAW_REACH));
        self.align_to_surface(dt, world);
        // A roll asked for goes to the animator once the lean has
        // straightened. That is here, after the animator has run, so the
        // roll starts a frame after the key.
        if self.pending_step != 0 && self.smoothed_tilt.abs() < LEAN_STRAIGHT_FOR_ROLL {
            self.step = self.pending_step;
        }
    }

    /// Crawling: the walking state with figures and fields of its own.
    fn update_prone_move(&mut self, dt: f32, world: &dyn World) {
        std::mem::swap(&mut self.mv, &mut self.mv_prone);
        self.update_move(dt, world);
        std::mem::swap(&mut self.mv, &mut self.mv_prone);
        self.align_to_surface(dt, world);
        // The sprint key held: the speed setting climbs a notch a frame
        // until it is at the top.
        if self.prone_sprint {
            self.select_speed(self.clamped_speed() + self.settings.delta_speed);
            if self.character_speed >= self.settings.max_speed.min(self.settings.state_speed_limit) {
                self.prone_sprint = false;
            }
        }
    }

    /// Lays the lying body to the ground ("Lying to the ground" in the
    /// movement spec): it is turned to point from the ground under it to
    /// the ground half a metre ahead, with the ground's normal as its up,
    /// both eased, and its roll is then limited.
    fn align_to_surface(&mut self, dt: f32, world: &dyn World) {
        /// How far ahead the second point of the ground is taken, from how
        /// many capsule heights up the ray for it starts, and how far it
        /// reaches. The ground check's own ray looks this much further
        /// than the check itself.
        const AHEAD: f32 = 0.5;
        const RAY_FROM: f32 = 1.5;
        const RAY_REACH: f32 = 2.0;
        const GROUND_RAY_BEYOND: f32 = 0.25;
        /// The steepest ground a body is laid to, and the most it rolls.
        const STEEPEST: f32 = 30.0;
        const MOST_ROLL: f32 = 8.0;

        let rate = dt * self.settings.prone_align_speed;
        let [x, y, z] = self.position;
        let facing = self.to_world([0.0, 1.0]);
        let forward = [facing[0], 0.0, facing[1]];
        let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let ease = |from: [f32; 3], to: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| lerp(from[i], to[i], rate)) };

        // The two points of the ground, where the rays reach them.
        let under = world.ground_height(x, z);
        let under = (y - under <= self.ground_check + GROUND_RAY_BEYOND).then_some([x, under, z]);
        let (ax, az) = (x + forward[0] * AHEAD, z + forward[2] * AHEAD);
        let from = y + self.capsule_height() * RAY_FROM;
        let ahead = world.ground_height(ax, az);
        let ahead = (ahead <= from && from - ahead <= RAY_REACH).then_some([ax, ahead, az]);
        let normal = world.ground_normal(x, z);
        let wanted = match (ahead, under) {
            (Some(ahead), Some(under)) => {
                let along: [f32; 3] = std::array::from_fn(|i| ahead[i] - under[i]);
                let len = (along[0] * along[0] + along[1] * along[1] + along[2] * along[2]).sqrt();
                along.map(|c| c / len)
            }
            // One point: the player's forward laid into its plane.
            _ => cross(normal, cross(forward, normal)),
        };
        let pointing = quat_rotate(quat_mul(quat_axis(1, self.body_yaw), self.body_tilt), [0.0, 0.0, 1.0]);
        self.surface_direction = ease(pointing, wanted);

        // The normal, no steeper than the limit.
        let lean = normal[1].clamp(-1.0, 1.0).acos().to_degrees();
        let normal = if lean >= STEEPEST {
            let level = length([normal[0], normal[2]]);
            let (sin, cos) = STEEPEST.to_radians().sin_cos();
            [normal[0] / level * sin, cos, normal[2] / level * sin]
        } else {
            normal
        };
        self.surface_normal = ease(self.surface_normal, normal);

        let Some(laid) = quat_look(self.surface_direction, self.surface_normal) else { return };
        let mut angles = quat_to_euler(quat_mul(quat_axis(1, -self.body_yaw), laid));
        // The engine gives these angles from 0 to 360, bar a hair below
        // zero, so a roll to the right reads as nearly a full turn and the
        // limit makes the most roll to the left of it.
        let roll = if angles[2] < -ENGINE_ANGLE_ALLOWANCE { angles[2] + 360.0 } else { angles[2] };
        angles[2] = roll.clamp(-MOST_ROLL, MOST_ROLL);
        self.body_tilt = quat_euler(angles);
        self.surface_pitch = delta_angle(0.0, angles[0]);
    }

    /// Getting up: nothing for the first part of the clip, then the body
    /// is let off the ground by the share of the clip that has played
    /// since. The roll is not limited here.
    fn rise_from_surface(&mut self) {
        const HELD_FOR: f32 = 0.3;
        let Some(time) = self.body.time_in(movement_state::PRONE_TO_STAND) else { return };
        if time <= HELD_FOR {
            return;
        }
        let share = (time - HELD_FOR) / (1.0 - HELD_FOR);
        if share >= 1.0 {
            self.stand_upright();
            return;
        }
        let facing = self.to_world([0.0, 1.0]);
        let forward = [facing[0], 0.0, facing[1]];
        self.surface_direction = std::array::from_fn(|i| lerp(self.surface_direction[i], forward[i], share));
        self.surface_normal = std::array::from_fn(|i| lerp(self.surface_normal[i], [0.0, 1.0, 0.0][i], share));
        if let Some(laid) = quat_look(self.surface_direction, self.surface_normal) {
            self.body_tilt = quat_mul(quat_axis(1, -self.body_yaw), laid);
            self.surface_pitch = delta_angle(0.0, quat_to_euler(self.body_tilt)[0]);
        }
    }

    fn stand_upright(&mut self) {
        self.surface_direction = [0.0; 3];
        self.surface_normal = [0.0, 1.0, 0.0];
        self.body_tilt = [0.0, 0.0, 0.0, 1.0];
        self.surface_pitch = 0.0;
    }

    fn enter_move(&mut self, from: State) {
        let s = &self.settings;
        let clamped = self.clamped_speed();
        self.mv = MoveData {
            inertia_direction: self.to_world(self.movement_direction),
            previous_target: clamped,
            smoothed_yaw: self.body_yaw,
            speed: if from == State::Sprint { [0.0, 1.0] } else { [0.0; 2] },
            lower_range: (clamped - s.movement_acceleration_range.evaluate(s.walk_inertia)).max(0.0),
            // The game does not clear these on entering the state: a walk
            // begins with the rates the last walk's smoothing ended on
            // ("What entering does not clear" in the movement spec).
            speed_velocity: self.mv.speed_velocity,
            direction_velocity: self.mv.direction_velocity,
            transition_coef: self.mv.transition_coef,
            last_direction_input: self.mv.last_direction_input,
            sprint_postponed: self.mv.sprint_postponed,
            unheard_time: self.mv.unheard_time,
            // Lying down the game reads two factors that are 1 on foot.
            speed_acceleration: if self.prone { s.prone_speed_acceleration } else { 1.0 },
            direction_acceleration: if self.prone { s.prone_direction_acceleration } else { 1.0 },
            ..MoveData::default()
        };
        // The state takes the direction it was entered with as its input
        // until the next command arrives, a frame later.
        self.direction = self.movement_direction;
    }

    /// What the walking state still holds from the last walk: the rate its
    /// direction smoothing was moving at when that walk ended, per second
    /// in the body's axes. The next walk starts with it.
    pub fn walk_leftover(&self) -> [f32; 2] {
        self.to_local(self.mv.direction_velocity)
    }

    pub fn set_walk_leftover(&mut self, rate: [f32; 2]) {
        self.mv.direction_velocity = self.to_world(rate);
    }

    /// The leftover that carries the direction `shift` away from where a
    /// walk begun from standing with `keys` held starts it, on a first
    /// frame `dt` long. For starting a replay as a recording started.
    pub fn walk_leftover_for(&self, shift: [f32; 2], keys: [f32; 2], dt: f32) -> [f32; 2] {
        let s = &self.settings;
        let target = self.clamped_speed().clamp(0.01, 1.0);
        let mut smooth_time = target * s.walk_inertia * s.direction_inertia(keys);
        if target < self.smoothed_speed {
            smooth_time /= s.deceleration_speed;
        }
        let blend_time = s.min_direction_blend_time.max(smooth_time);
        // On that frame the smoothing aims at where it stands, so all that
        // moves the direction is the leftover: this far for a rate of one.
        let reach = smooth_damp([0.0], [0.0], &mut [1.0], blend_time, s.inertia_input_max_speed, dt)[0];
        [shift[0] / reach, shift[1] / reach]
    }

    fn update_move(&mut self, dt: f32, world: &dyn World) {
        self.update_movement_direction(dt);

        // With no input for a while and the speed run down, the animator
        // is told the player no longer wants to move; it blends to standing.
        let s = &self.settings;
        let no_input_for_long = self.mv.time_without_input > s.max_time_without_input
            && length(self.mv.speed) < s.exit_speed_threshold
            && self.mv.reversal_pause <= 0.0;
        // So is it when the character is locked against an obstacle.
        if no_input_for_long || self.blocked {
            self.inert = false;
        }

        self.follow_look_yaw(dt);
        let motion = self.animated_motion;
        self.apply_motion(motion, dt, world);
        self.check_obstacle();

        if self.movement_direction[1] > 0.1 {
            if self.mv.sprint_postponed {
                self.enable_sprint(true);
                if self.sprinting {
                    self.pose_level = 1.0;
                }
                self.mv.sprint_postponed = false;
            }
            // The sprint itself starts when the animator's sprint state does.
            if self.sprinting && self.pose_level > SPRINT_POSE_THRESHOLD && self.smoothed_speed >= 1.0 {
                self.sprint_flag = true;
            }
        }
    }

    fn update_movement_direction(&mut self, dt: f32) {
        let clamped = self.clamped_speed();
        let mut input = self.direction;
        let direction_factor = self.settings.direction_inertia(input);

        // A reversal is answered with a short pause before the new direction takes over.
        let m = &mut self.mv;
        if !is_zero(m.prev_direction)
            && !is_zero(input)
            && m.reversal_pause <= 0.0
            && dot(normalize(m.prev_direction), normalize(input)) < 0.0
        {
            m.next_direction = input;
            m.reversal_pause = self.settings.move_time;
        }
        if m.reversal_pause > 0.0 {
            input = [0.0; 2];
            self.direction = input;
            m.reversal_pause -= dt;
            if m.reversal_pause < 0.0 {
                m.reversal_pause = 0.0;
                self.direction = m.next_direction;
                // The pause ends by telling the animator the player has
                // stopped, so a reversal goes through standing: the walk
                // blends out and starts again the other way.
                self.inert = false;
                return;
            }
        }
        if !is_zero(input) {
            m.prev_direction = input;
        }

        let world_input = self.to_world(input);
        let s = &self.settings;
        let m = &mut self.mv;
        let target = clamped.clamp(0.01, 1.0);
        let crouch = lerp(
            1.0,
            lerp(s.crouch_speed_acceleration[0], s.crouch_speed_acceleration[1], clamped),
            s.crouch_acceleration_weight.evaluate(1.0 - self.pose_level),
        );
        let unit = normalize(world_input);
        let wanted = target * m.speed_acceleration * crouch;
        let desired = [unit[0] * wanted, unit[1] * wanted];

        let just_released = !is_zero(m.last_direction_input) && is_zero(world_input);
        let just_pressed = !is_zero(world_input) && is_zero(m.last_nonzero_direction);
        if just_released {
            m.transition_coef = self.smoothed_speed;
        } else if just_pressed {
            m.transition_coef = target;
        } else if m.previous_target != target {
            m.lower_range = (clamped - s.movement_acceleration_range.evaluate(s.walk_inertia)).max(0.0);
            m.transition_coef = (target - length(m.speed)).abs();
        }
        m.previous_target = target;

        let mut smooth_time = m.transition_coef * s.walk_inertia * direction_factor;
        if target < self.smoothed_speed || is_zero(world_input) {
            smooth_time /= s.deceleration_speed;
        }
        // The game gives this smoothing no time of its own, so it runs on
        // the engine's; the direction's, below, is given the player's.
        m.speed =
            smooth_damp(m.speed, desired, &mut m.speed_velocity, smooth_time, s.inertia_input_max_speed, self.frame_dt);
        let mut magnitude = length(m.speed);
        if magnitude < m.lower_range {
            m.speed = [desired[0] / target * m.lower_range, desired[1] / target * m.lower_range];
            magnitude = m.lower_range;
        }
        if !self.sprinting {
            // Within a thousandth the two count as equal (step 10 of
            // "Moving" in the movement spec).
            self.smoothed_speed = if (magnitude - self.smoothed_speed).abs() < 0.001 {
                magnitude
            } else {
                lerp(self.smoothed_speed, magnitude, s.sudden_changes_smoothness * dt)
            };
        }

        m.last_direction_input = world_input;
        if !is_zero(world_input) {
            m.last_nonzero_direction = world_input;
        }
        let blend_time = s.min_direction_blend_time.max(smooth_time) / m.direction_acceleration;
        m.inertia_direction =
            smooth_damp(m.inertia_direction, world_input, &mut m.direction_velocity, blend_time, s.inertia_input_max_speed, dt);
        if is_zero(world_input) {
            m.time_without_input += dt;
        } else {
            m.time_without_input = 0.0;
        }
        // The update ends by dropping the input it was handed, once the
        // state's time without a call has run out, which it has for good
        // a fifth of a second into a character's first walk: the next
        // frame's input has to be handed over anew, and a second update in
        // the same frame, or a frame in which a screen has the keys, runs
        // with none.
        m.unheard_time += dt;
        if m.unheard_time > MAX_WITHOUT_MOVE_CALL {
            self.direction = [0.0; 2];
        }

        let inertia_direction = m.inertia_direction;
        self.movement_direction = self.to_local(inertia_direction);
        let facing = if is_zero(input) { self.movement_direction } else { input };
        if let Some(discrete) = discrete_direction(facing) {
            self.discrete_direction = discrete;
        }
    }

    fn follow_look_yaw(&mut self, dt: f32) {
        let behind = delta_angle(self.yaw, self.mv.smoothed_yaw).abs();
        let hurry = inverse_lerp(10.0, 45.0, behind) + 1.0;
        self.mv.smoothed_yaw =
            lerp_angle(self.mv.smoothed_yaw, self.yaw, self.settings.rotation_lerp_speed * dt * hurry);
        self.body_yaw = self.mv.smoothed_yaw;
        self.note_rotation();
    }

    fn update_sprint(&mut self, dt: f32, world: &dyn World) {
        let input = self.direction;
        // Letting go of forward, or of sprint, only lowers the animator's
        // flag; the sprint ends when the animator leaves its sprint state.
        // Nothing moves the character in such a frame. A character locked
        // against an obstacle is treated the same. Not in a frame in which
        // an update has already dropped its input (below): that one goes
        // on to sprint with none.
        let dropped = self.frame_count == self.sprint_dropped_in;
        if (input[1].abs() < f32::EPSILON || self.blocked) && !dropped {
            self.sprint_flag = false;
            if input[0].abs() < f32::EPSILON {
                self.inert = false;
            }
            self.check_obstacle();
            return;
        }
        if !self.sprinting {
            self.sprint_flag = false;
            self.check_obstacle();
            return;
        }
        // The sprinting state looks at the speed limits again itself, so
        // one set since the frame's own look (the cap a jump leaves, on
        // the frame a sprint is landed in) holds the sprint's pace at once.
        self.work_in_limits();
        let t = (dt * self.settings.direction_lerp_speed).clamp(0.0, 1.0);
        self.movement_direction = [
            self.movement_direction[0] + (input[0] - self.movement_direction[0]) * t,
            self.movement_direction[1] + (input[1] - self.movement_direction[1]) * t,
        ];
        if let Some(discrete) = discrete_direction(input) {
            self.discrete_direction = discrete;
        }
        // A sprinting update drops the input it was handed, so that it
        // has to be handed over again every frame: a frame without, as
        // when a screen has the keys, ends the sprint like keys let go.
        self.direction = [0.0; 2];
        self.sprint_dropped_in = self.frame_count;
        self.sprint_acceleration(dt);
        self.follow_look_yaw(dt);
        let motion = self.animated_motion;
        self.apply_motion(motion, dt, world);
        self.check_obstacle();
    }

    /// A sprint coming to a stop: the animator plays its own clip for it.
    fn update_sprint_stop(&mut self, dt: f32, world: &dyn World) {
        self.follow_look_yaw(dt);
        let keep = self.settings.sprint_stop_motion_preservation;
        let motion = [0, 1, 2].map(|i| lerp(self.animated_motion[i], self.last_blend_velocity[i] * dt, keep));
        self.apply_motion(motion, dt, world);
    }

    /// Landing from a jump made while moving. The landing clip carries the
    /// character on unless the player has let go for a while.
    fn update_landing(&mut self, dt: f32, world: &dyn World) {
        /// The speed setting a landing with no input settles toward.
        const LANDING_SPEED: f32 = 0.35;
        if self.landing_idle_time > self.settings.max_time_without_input / 2.0 {
            self.landing_still = true;
        }
        if is_zero(self.direction) {
            self.landing_idle_time += dt;
            if self.smoothed_speed > LANDING_SPEED {
                self.smoothed_speed =
                    lerp(self.smoothed_speed, LANDING_SPEED, self.settings.sudden_changes_smoothness * dt);
            }
        }
        self.turn_body(dt);
        if !self.landing_still {
            let motion = self.animated_motion;
            self.apply_motion(motion, dt, world);
            self.note_rotation();
        }
    }

    fn sprint_acceleration(&mut self, dt: f32) {
        let s = &self.settings;
        let step = s.sprint_acceleration * dt;
        // What a landing left as a cap on the selected speed also scales
        // the sprint's top speed while it lasts.
        let after_landing = self.landing_limit_in_effect.unwrap_or(1.0);
        let limit = (s.physical_sprint_speed * s.sprinting_speed + 1.0) * after_landing;
        // Turning hard while sprinting costs speed. The measure is the
        // average yaw change per frame over the last frames.
        let turn_rate = if self.yaw_deltas.is_empty() {
            0.0
        } else {
            self.yaw_deltas.iter().sum::<f32>() / self.yaw_deltas.len() as f32
        };
        let floor = s.sprint_speed_inertia.evaluate(f32::MAX) * (2.0 - s.inertia);
        let turn = s.sprint_speed_inertia.evaluate(turn_rate.abs()).max(floor);
        let limit = (limit * turn).clamp(0.1, limit);
        self.sprint_speed = (self.sprint_speed + step * sign(limit - self.sprint_speed)).clamp(0.01, limit);
    }

    fn enter_jump(&mut self, from: State) {
        // The jump is paid for as its state is entered.
        self.stamina.jump(self.frame_clock);
        self.blocked = false;
        let s = &self.settings;
        let before = self.velocity_before_limit;
        self.landing_flag = false;
        let heading = if matches!(from, State::Move | State::Sprint) {
            let d = normalize(self.to_world(self.movement_direction));
            [d[0], 0.0, d[1]]
        } else {
            [0.0; 3]
        };
        // Per axis, whichever of the two has the larger magnitude.
        let pick = |a: f32, b: f32| if a.abs() > b.abs() { a } else { b };
        let v = [pick(before[0], heading[0]), pick(before[2], heading[2])];
        let speed = length(v);
        let direction = if speed > 0.1 {
            [v[0] / speed, 0.0, v[1] / speed]
        } else {
            let yaw = self.body_yaw.to_radians();
            [yaw.sin(), 0.0, yaw.cos()]
        };
        self.jump = JumpData {
            direction,
            initial_speed: speed,
            speed,
            lift: s.lift_velocity_by_speed.evaluate(speed),
            lift_delay: s.jump_delay_by_speed.evaluate(speed),
            from_sprint: from == State::Sprint,
            potential_height: self.position[1],
            move_dir: self.direction,
            // The jump moves the character in the frame it begins.
            lag_at_start: self.clock_lag_before,
            ..JumpData::default()
        };
        self.tilt = 0.0;
    }

    fn update_jump(&mut self, dt: f32, world: &dyn World) {
        let s = &self.settings;
        let j = &mut self.jump;
        // The jump ends by telling the animator it has landed: either a
        // touchdown, where a held direction carries on into a walk or a
        // sprint, or a halt when the jump never got going or stuck.
        let (mut touchdown, mut halt) = (false, false);
        self.inert = length(j.move_dir) > 0.1;

        if j.halted {
        } else if !j.airborne {
            if j.time > 2.0 + j.lift_delay {
                halt = true;
            } else if !self.grounded && j.time > j.lift_delay {
                j.airborne = true;
            }
        } else {
            if j.time_descending > s.jump_descending_exit_time && self.grounded {
                touchdown = true;
            }
            if let Some(last) = j.last_height {
                if (self.position[1] - last).abs() < 0.0001 {
                    j.frames_without_motion += 1;
                    if j.frames_without_motion > 3 {
                        halt = true;
                    }
                } else {
                    j.frames_without_motion = 0;
                }
            }
        }
        if self.grounded {
            self.freefall_time = dt;
            j.last_height = None;
        } else {
            self.freefall_time += dt;
            j.last_height = Some(self.position[1]);
        }
        j.time += dt;

        let turned_from = self.body_yaw;
        self.body_yaw = lerp_turn(self.body_yaw, self.yaw, s.rotation_lerp_speed * dt);
        j.turn_rate = delta_angle(turned_from, self.body_yaw) / dt;
        (self.hands_to_body, self.aim_angle) = (body_to_view(self.body_yaw, self.yaw), self.pitch);

        // Vertical: nothing until the push-off delay has passed, then a
        // launch velocity that gravity eats into.
        let since_lift = j.time - j.lift_delay;
        j.lift_velocity = if j.time < j.lift_delay { 0.0 } else { j.lift * s.jump_boost + s.gravity * since_lift };
        if self.position[1] < j.potential_height {
            j.lift_velocity += (j.potential_height - self.position[1]) / dt * 0.9;
        }

        // Horizontal: the direction is fixed at take-off. Input only decides
        // how much of the take-off speed is kept.
        let wish = if is_zero(j.move_dir) {
            [0.0; 2]
        } else {
            let unit = normalize(j.move_dir);
            let yaw = self.body_yaw.to_radians();
            [unit[0] * yaw.cos() + unit[1] * yaw.sin(), -unit[0] * yaw.sin() + unit[1] * yaw.cos()]
        };
        let along = j.direction[0] * wish[0] + j.direction[2] * wish[1];
        let target = if along < -0.966 {
            j.initial_speed * s.air_control_back
        } else if along > 0.966 {
            s.air_min_speed.max(j.initial_speed * s.air_control_same)
        } else {
            j.initial_speed * s.air_control_other
        };
        if j.airborne {
            j.speed = lerp(j.speed, target, dt * s.air_lerp);
        }

        let motion = [j.direction[0] * j.speed * dt, j.lift_velocity * dt, j.direction[2] * j.speed * dt];
        let height_before = self.position[1];
        j.potential_height = self.position[1] + j.lift_velocity * dt;
        let descending_by_velocity = j.lift_velocity < 0.0;
        self.move_by(motion, dt, world);
        if self.position[1] - height_before < 0.001 || descending_by_velocity {
            self.jump.time_descending += dt;
        }

        if touchdown {
            self.jump_flag = false;
            self.landing_flag = true;
            let move_dir = self.jump.move_dir;
            if dot(move_dir, move_dir) > 0.1 {
                self.enable_sprint(self.jump.from_sprint && move_dir[1] > 0.1);
                self.sprint_flag = self.sprinting;
                self.movement_direction = move_dir;
                self.inert = true;
            } else {
                self.inert = false;
            }
        }
        if halt {
            self.jump_flag = false;
            self.landing_flag = true;
            self.jump.halted = true;
        }
    }

    /// Entering a vault or a climb (vaulting spec, "The moves as
    /// movement states").
    fn enter_vault(&mut self, kind: Move, from: State) {
        /// The capsule's standing height, which both moves start from.
        const STANDING: f32 = 1.6;
        let s = self.settings.vaulting;
        let clear = s.of(kind).max_without_hand;
        let height = self.vault_figures[0];
        self.is_vaulting = true;
        self.blocked = false;
        // A vault takes the hand over that height; a climb at it.
        let hands = if kind == Move::Vault { height > clear } else { height >= clear };
        let capsule = match kind {
            // Shrunk from below at once, to pass over the top.
            Move::Vault => [STANDING - height * 1.1, height + (STANDING - height) / 1.5],
            // A climb's shrinks as it rises, from the standing one, whose
            // centre is half its height up: the player's controller is the
            // game's own, which has no skin (recorded: 1.6 and 0.8).
            Move::Climb => [STANDING, STANDING / 2.0],
        };
        self.vault = VaultData { height, from_sprint: from == State::Sprint, hands, capsule };
        if height >= s.vault.max_without_hand.min(s.climb.max_without_hand) {
            self.tilt = 0.0;
        }
        if kind == Move::Vault && self.vault.from_sprint {
            // The sprint was switched off on the way out of its state; the
            // animator is told as much, and that the player is moving.
            self.sprint_flag = self.sprinting;
            self.inert = true;
        }
        // The cost, and the pace of the clips: standing by the walk's
        // overweight, crouched by the general one.
        self.stamina.set_pose(self.prone, self.pose_level);
        let (now, st) = (self.frame_clock, &self.settings.stamina);
        let overweight = if self.pose_level < 0.11 { st.overweight } else { st.walk_overweight };
        match kind {
            Move::Vault => self.stamina.vault(height > clear, now),
            Move::Climb => self.stamina.climb(height > clear, height > s.climb.max_one_hand, now),
        }
        self.vaulting_speed = s.pace(kind, overweight);
        if hands {
            // The sights go down and the view is the move's own.
            self.aim_taken = true;
            self.turn_limit = 0.0;
        }
    }

    /// A vault or a climb: the animation's own motion as it comes. No
    /// gravity and no limit; only, on the ground, nothing downward.
    fn update_vault(&mut self, dt: f32, world: &dyn World) {
        self.turn_body(dt);
        let mut motion = self.animated_motion;
        if self.grounded {
            motion[1] = motion[1].max(0.0);
        }
        self.travel(motion, dt, world, false);
        if self.state == State::Climb && motion[1] > 0.0 {
            // The capsule's underside rises as fast again as the feet.
            self.vault.capsule[0] -= motion[1];
            self.vault.capsule[1] += motion[1] / 2.0;
        }
        self.body_yaw += self.body.turn();
        self.note_rotation();
    }

    /// Coming down behind a vault: any state's motion, with gravity.
    fn update_vault_fall(&mut self, dt: f32, world: &dyn World) {
        self.turn_body(dt);
        let motion = self.animated_motion;
        self.apply_motion(motion, dt, world);
        self.body_yaw += self.body.turn();
        self.note_rotation();
    }

    /// Once a frame, before the animator: how fast the character really
    /// went over the frame before, and whether the lock on moving into an
    /// obstacle comes off (movement spec, "Stopped by what stands in the
    /// way"). The game measures from one frame's place to the next over
    /// the engine's frame time.
    fn watch_obstacle(&mut self, dt: f32) {
        /// The lock comes off past this many degrees, of the movement
        /// direction from the one it was put on with or of the view from
        /// the body.
        const TURNED: f32 = 5.0;
        let gone: [f32; 3] = std::array::from_fn(|k| self.position[k] - self.place_before[k]);
        self.place_before = self.position;
        if dt > 0.0 {
            self.real_speed = (gone[0] * gone[0] + gone[1] * gone[1] + gone[2] * gone[2]).sqrt() / dt;
        }
        if self.blocked {
            let (was, now) = (self.blocked_direction, self.movement_direction);
            let lengths = (length(was) * length(now)).max(1e-15);
            // Against no direction at all the engine's angle is nothing.
            let between = if length(was) * length(now) < 1e-15 {
                0.0
            } else {
                ((was[0] * now[0] + was[1] * now[1]) / lengths).clamp(-1.0, 1.0).acos().to_degrees()
            };
            if between > TURNED || self.hands_to_body.abs() > TURNED {
                self.blocked = false;
            }
        }
    }

    /// The walking and the sprinting state's look at whether the character
    /// is getting anywhere: slower than `threshold` with the movement
    /// direction all but full, for more than a tenth of a second of such
    /// looks, puts the lock on. Not looked at while the body's animator is
    /// between two states.
    fn check_obstacle(&mut self) {
        /// How long the character has to have been that slow, in seconds,
        /// and how long its movement direction has to be.
        const WAIT: f32 = 0.1;
        const PUSHING: f32 = 0.9;
        if self.body.in_transition() {
            return;
        }
        let threshold = if self.state == State::Sprint { 0.5 } else { 0.2 };
        let slow = self.real_speed < threshold;
        let waited = self.slow_time > WAIT;
        self.slow_time = if slow { self.slow_time + self.frame_dt } else { 0.0 };
        if !self.blocked && slow && waited && length(self.movement_direction) > PUSHING {
            self.blocked = true;
            self.blocked_direction = self.movement_direction;
        }
    }

    /// What the choice of a move goes by besides the obstacle.
    fn standing(&self) -> Standing {
        let s = &self.settings.vaulting;
        let height = self.vaulting.obstacle.height;
        Standing {
            vaulting: self.is_vaulting,
            backing: self.movement_direction[1] < 0.0,
            grounded: self.grounded,
            in_transition: self.body.in_transition(),
            jumping: self.jump_flag,
            sprinting: self.sprinting,
            can_vault: self.stamina.can_vault(height > s.vault.max_without_hand),
            can_climb: self.stamina.can_vault(height > s.climb.max_without_hand),
        }
    }

    /// The vault key's work for one frame it is held
    /// (`MovementContext.TryVaulting`; vaulting spec, "What a try does").
    fn try_vaulting(&mut self, input: &Input, world: &dyn World) {
        let s = self.settings.vaulting;
        // The game's restrictions go by the move the obstacle as last
        // measured would get: not lying down, and with the weapon busy
        // only what leaves the hands on it.
        let stale = self.vaulting.choose(&s, &self.standing());
        let high = stale.is_some_and(|kind| self.vaulting.obstacle.height >= s.of(kind).max_without_hand);
        if self.prone || (input.weapon_occupied && high) || self.input_direction[1] <= 0.0 {
            return;
        }
        // How fast the character goes along the view's heading; nothing if
        // it goes against it.
        let (sin, cos) = self.yaw.to_radians().sin_cos();
        let speed = (self.velocity[0] * sin + self.velocity[2] * cos).max(0.0);
        match self.vault_given.take() {
            Some((obstacle, speed)) => self.vaulting.take(&s, obstacle, speed),
            None => self.vaulting.measure(&s, world, self.position, self.body_yaw, speed, self.sprinting),
        }
        let Some(kind) = self.vaulting.choose(&s, &self.standing()) else { return };
        let o = self.vaulting.obstacle;
        self.vault_figures = [o.height, o.length, o.behind, self.vaulting.forward_speed];
        match kind {
            Move::Vault => self.do_vault = true,
            Move::Climb => self.do_climb = true,
        }
    }

    fn set_state(&mut self, state: State) {
        self.previous_state = self.state;
        self.state = state;
        match state {
            State::Move | State::ProneMove => self.ground_check = GROUND_CHECK_MOVING,
            State::Sprint => self.ground_check = GROUND_CHECK_SPRINT,
            State::Jump => self.ground_check = GROUND_CHECK_JUMP,
            _ => {}
        }
    }

    // ------------------------------------------------------------- motion

    /// The common tail of the grounded states: remember the motion, add
    /// gravity, move.
    fn apply_motion(&mut self, motion: [f32; 3], dt: f32, world: &dyn World) {
        self.last_blend_velocity = motion.map(|m| m / dt);
        self.fall_and_move(motion, dt, world);
    }

    /// Adds gravity and moves, for the states that do not remember their
    /// motion for standing's carry-over.
    fn fall_and_move(&mut self, mut motion: [f32; 3], dt: f32, world: &dyn World) {
        let g = self.settings.gravity;
        if self.freefall_time >= FREEFALL_GRACE && !self.grounded {
            let damp = 1.0 / (1.0 + 2.0 * self.freefall_time);
            motion = motion.map(|m| m * damp);
        }
        motion[1] += g * self.freefall_time * dt;
        if self.grounded {
            self.freefall_time = 0.0;
        } else {
            self.freefall_time += dt;
        }
        self.move_by(motion, dt, world);
    }

    fn move_by(&mut self, motion: [f32; 3], dt: f32, world: &dyn World) {
        self.travel(motion, dt, world, true);
    }

    /// Moves the character. `on_ground` is whether the ground holds the
    /// feet up: not in a vault or a climb, where the game's capsule is
    /// clear of whatever the feet pass through.
    fn travel(&mut self, motion: [f32; 3], dt: f32, world: &dyn World, on_ground: bool) {
        self.velocity_before_limit = motion.map(|m| m / dt);
        let before = self.position;
        for i in 0..3 {
            self.position[i] += motion[i];
        }
        // Whether the cap is lifted is asked where the motion put the
        // capsule, as the game's controller notes what it overlaps there.
        let uncapped = world.lifts_cap(self.position, self.capsule());
        // What stands in the way first, then the ground where that leaves
        // the character: ground too high to step onto is the host's to
        // refuse, before the feet are put on it.
        self.position = world.obstructed(before, self.position, self.capsule());
        if on_ground && !world.sets_feet() {
            let ground = world.footing(self.position);
            if self.position[1] < ground {
                self.position[1] = ground;
            }
        }
        for i in 0..3 {
            self.velocity[i] = (self.position[i] - before[i]) / dt;
        }
        // The hard cap for the state: whatever asked for more, the
        // character goes no faster. Falling speed is not counted.
        let [x, y, z] = self.velocity;
        let speed = (x * x + y.max(0.0) * y.max(0.0) + z * z).sqrt();
        if speed > self.speed_cap && !uncapped {
            let whole = (x * x + y * y + z * z).sqrt();
            // A falling character keeps its direction and is slowed until
            // the level part is at the cap.
            let allowed = if y < 0.0 { self.speed_cap * whole / speed } else { self.speed_cap };
            for i in 0..3 {
                self.velocity[i] *= allowed / whole;
                self.position[i] = before[i] + self.velocity[i] * dt;
            }
        }
    }

    /// Body axes (x right, y forward) to world (x, z).
    fn to_world(&self, v: [f32; 2]) -> [f32; 2] {
        let (sin, cos) = self.body_yaw.to_radians().sin_cos();
        [v[0] * cos + v[1] * sin, -v[0] * sin + v[1] * cos]
    }

    fn to_local(&self, v: [f32; 2]) -> [f32; 2] {
        let (sin, cos) = self.body_yaw.to_radians().sin_cos();
        [v[0] * cos - v[1] * sin, v[0] * sin + v[1] * cos]
    }
}

/// The animator's eight-way direction value: left 0, front 1, right 2,
/// back 3, with the diagonals halfway between. `None` for no direction.
fn discrete_direction(v: [f32; 2]) -> Option<f32> {
    let [x, y] = v;
    if x.abs() < f32::EPSILON {
        return if y >= f32::EPSILON {
            Some(1.0)
        } else if y < 0.0 {
            Some(3.0)
        } else {
            None
        };
    }
    let ratio = (y / x).abs();
    Some(if ratio > 2.0 {
        if y > 0.0 { 1.0 } else { 3.0 }
    } else if ratio < 0.5 {
        if x > 0.0 { 2.0 } else { 0.0 }
    } else {
        match (x > 0.0, y > 0.0) {
            (true, true) => 1.5,
            (false, true) => 0.5,
            (true, false) => 2.5,
            (false, false) => 3.5,
        }
    })
}
