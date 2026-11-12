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

use crate::locomotion::body::{movement_state, part, ArmPose, Body, BodyInput, Entered};
use crate::weapons::hands::{Carried, Held};
use crate::locomotion::head_look::{HeadLook, HeadLookInput};
use crate::held::interactions::{AnimatedInteractions, Interaction, Show};
use crate::base::math::{
    body_to_view, delta_angle, dot, inverse_lerp, length, lerp, lerp_angle, lerp_turn, normalize, quat_axis, quat_euler, quat_look,
    quat_mul, quat_rotate, quat_to_euler, smooth_damp, unity_clamp,
};
use crate::base::settings::MovementSettings;
use crate::locomotion::stamina::Stamina;
use crate::locomotion::vaulting::{Move, Obstacle, Standing, Vaulting};

mod left_stance;
mod mount;
pub use mount::mounted_apart;
pub mod pom;
mod room;
pub mod slope;

mod tuning;
mod input;
mod world;
mod reports;
mod creation;
mod views;
mod footsteps;
mod blind_fire;
mod interaction;
mod stepping;
mod states;
mod commands;
mod orders;
mod posture;
mod prone;
mod walking;
mod jumps;
mod motion;

pub use tuning::*;
pub use input::*;
pub use world::*;
pub use reports::*;
use tuning::{AIMING_SPEED_LIMIT, CRAWLING_SPEED_CAP, ENGINE_ANGLE_ALLOWANCE, FREEFALL_GRACE, FREE_TURN_LIMIT, HEIGHT_SMOOTH_TIME, JUMP_POSE_THRESHOLD, JumpData, LEAN_STRAIGHT_FOR_ROLL, LONG_FALL, MAX_TILT, MAX_WITHOUT_MOVE_CALL, MIN_INPUT, MoveData, PITCH_LIMIT_SPEED, PRONE_PITCH_RANGE, PRONE_TRUNK_LIMIT, PRONE_YAW_REACH, ROLL_PITCH_RANGE, ROLL_YAW_SPEED, SLOW_LEAN_STEP, SPRINTING_SPEED_CAP, SPRINT_POSE_THRESHOLD, STANDING_SPEED_CAP, StepData, TILT_STEP, UPRIGHT_PITCH_RANGE, VaultData, WALKING_SPEED_CAP};
use input::{blind_side, lean_keys};
use world::{is_zero, sign};
use reports::{APPROACH_LONGEST, APPROACH_SPEED, APPROACH_TURN, Acting, Approaching, HAND_LAID_AFTER, KICK_LANDS_AT, KICK_MOVES_FROM, Kick, PICKED_UP_AFTER, PICK_UP_CROUCH_PITCH, PICK_UP_LASTS, SEARCH_REACH, WORKING_ENDS_AFTER};
use motion::{discrete_direction};

// ==== Movement: the simulation's state ====

#[derive(Clone)]
pub struct Movement {
    pub settings: MovementSettings,
    /// Which of its systems run (`crate::Systems`): all, unless a host
    /// switches some off.
    pub systems: crate::Systems,

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
    /// What the character's health allows (health spec, "What health does
    /// to the character"), as a host's health system sets it
    /// (`set_conditions`): nothing is held back unless one does.
    pub conditions: crate::condition::health::Conditions,
    /// The speed limits health, barbed wire and the armour worn put on the
    /// character (`ESpeedLimit.HealthCondition`, `BarbedWire`, `Armor`), and
    /// the same as the frame's upkeep has worked them in.
    held_limits: [Option<f32>; 3],
    held_limits_in_effect: [Option<f32>; 3],
    /// The cap a hit by a weapon leaves on the selected speed, and the time
    /// on the frame clock it ends (`ESpeedLimit.Shot`); the same as worked
    /// in; and the stamina burnt by the frame's hits so far, with the
    /// frame it was (movement spec, "Speed limits").
    shot_limit: Option<(f32, f32)>,
    shot_limit_in_effect: Option<f32>,
    hits_burnt: (u64, f32),
    /// What happened in the frame that health takes (`take_moved`).
    moved: Vec<Moved>,
    /// How far the sprint under way has gone (`Pedometer`).
    sprint_distance: f32,
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
    /// The animator's `SprintInertia`, set as a sprint begins
    /// (`MovementContext.UpdateSprintInertia`).
    sprint_inertia: f32,
    pub sprinting: bool,
    sprint_input: bool,
    idle_sprint_held: bool,
    aiming: bool,
    aim_kept_from_body: bool,
    /// What the weapon in hand's mastering adds to the walk's cap with the
    /// sights up (`Firearm::aim_walk_bonus`; 0 for anything else in hand,
    /// a rangefinder's cap being 0.33 alone): a host sets it.
    pub aim_walk_bonus: f32,
    /// The cap on the speed setting with the sights up, set as they come
    /// up (`SetAimingSlowdown(IsAiming, 0.33 + AimMovementSpeed)`): a
    /// change of the bonus with them up waits for the next time.
    aiming_cap: f32,
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
    /// Which way the ground under the character faces this frame
    /// (`SurfaceNormal`), its upward part over the last five frames, and
    /// the pull of ground too steep to stand on (`Slope`; movement spec,
    /// "Slopes and sliding").
    pub surface: [f32; 3],
    pub normal_history: slope::NormalHistory,
    pub slope: [f32; 3],
    /// Crouched in a sprint: the sprint is let go of from the next frame
    /// on, whatever the keys, until its state is left
    /// (`SprintPlayerState._slide`).
    slide: bool,
    /// Going down against something, the push back as last eased
    /// (`Transit2ProneState._depenetration`, kept from one going down to
    /// the next), and the view's yaw before this frame's turn, which such
    /// a push puts back (`RestorePreviousYaw`; movement spec, "Room to move").
    down_push: f32,
    yaw_before_turn: f32,
    /// Whether the head's lying capsule is the one in use (`SetPOMCollider`:
    /// from going down until getting up is over), and the head's push as
    /// the last limit found it (movement spec, "The head's push out of walls").
    head_lying: bool,
    head_push: [f32; 3],
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
    /// The last frame's time by the player's own clock (`player_dt`).
    last_player_dt: f32,
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
    /// Blind fire (`docs/spec/blind-fire.md`): the movement's own figure
    /// (`MovementContext.BlindFire`: 1 over the top, -1 to the right, 0
    /// none); what the body's animator is told of it (`BlindFire`, which the
    /// lean alone puts to 0); the float the state works out
    /// (`BlindFireFloat`); the part and the side of the blind-fire state
    /// last entered (its side 1 over the top, -1 to the right); and the
    /// figure as last taken, for the host to tell the weapon and its motion
    /// (`take_blind_fire`).
    blind_fire: i32,
    blind_shown: f32,
    blind_float: f32,
    blind_part: i32,
    blind_side: f32,
    blind_told: Option<i32>,
    /// How far the animator moved the body this step, in world axes.
    animated_motion: [f32; 3],
    /// For the landing state: seconds without input, and whether the
    /// landing has been left to play out without moving.
    landing_idle_time: f32,
    landing_still: bool,
    /// Seconds since the start, the time of the last footstep, which way
    /// the footstep curve last pointed, and a footstep not yet reported.
    /// No footstep has been taken at the start: the game's last step time
    /// is 0 against a clock that has run since the game was started
    /// (`Player._lastStepTime`, `SinceLastStep`), so the first steps of a
    /// raid are heard and never retune the weapon's bob.
    clock: f32,
    last_step: f32,
    step_side: f32,
    footstep: Option<f32>,
    /// The state changed into one other than the walk since this was last
    /// asked (`take_walk_stop`): the weapon's bob is rewound
    /// (`WalkEffectorEnabled = false`, `WalkEffector.OnStop`).
    walk_stop: bool,
    /// Whether a footstep was heard on this step; the crawl's marks the
    /// body's animation passed on it; the ground found again on it (how
    /// far below the highest point and how far above the take-off); and
    /// the height the feet left the ground at. The sounds of moving go by
    /// them (`sounds`).
    stepped: bool,
    crawl_marks: usize,
    landed: Option<(f32, f32)>,
    /// The speed was set on this step (`select_speed`, the game's
    /// `SetCharacterMovementSpeed`), which alone has the sounds' rules work
    /// out again how quietly one walks.
    speed_set: bool,
    fly_from: f32,
    /// The sounds of moving: the game's rules for when each plays, run on
    /// every step (`docs/spec/footsteps.md`). The host takes what they ask
    /// for (`StepSounds::take`).
    pub sounds: crate::sound::steps::StepSounds,
    /// Vaulting and climbing (vaulting spec): the grid and what it last
    /// measured; how long the key has been held; the two flags a chosen
    /// move raises for the animator, and the one that is up while a move
    /// is under way; the obstacle as the animator was last given it
    /// (height, length, the drop behind, the forward speed); the pace of
    /// the moves' clips; and the move in hand.
    pub vaulting: Vaulting,
    vault_time: f32,
    /// The surfaces the vault's sounds go by (footsteps spec, "The vault's
    /// sounds"): the obstacle's and the terrain's, each as last found
    /// (`CurrentObstacleSurface`, `CurrentTerrainSurface`: concrete to begin).
    vault_surfaces: [u8; 2],
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
    /// Mounting (`docs/spec/mounting.md`): its figures; the weapon in hand's
    /// mount figures and whether its bipod is out, as the host last said
    /// (`set_mount_weapon`); a search under way; the mount's own state while
    /// it overrides the movement state; whether the animator's root motion
    /// is ignored (`IgnoreDeltaMovement`, from a mount's approach until its
    /// state is taken off); the lean the weapon's motion reads
    /// (`ProceduralWeaponAnimation.SmoothedTilt`); and what it has to tell.
    pub mount_settings: crate::weapons::mounting::Settings,
    mount_weapon: Option<(crate::weapons::mounting::WeaponMount, bool)>,
    mount_search: Option<crate::weapons::mounting::Search>,
    mount_search_fresh: bool,
    mount: Option<mount::Mounted>,
    /// The bipod's gesture asked for and its mark not yet passed.
    toggling_bipod: Option<Interaction>,
    ignore_root_motion: bool,
    weapon_tilt: f32,
    mount_events: Vec<crate::weapons::mounting::MountEvent>,
    /// The data of the last mount entered, kept after it (for a report).
    mount_last: Option<crate::weapons::mounting::Data>,
    /// The rig's turn as the frame began (`rig_frame`'s, the frame before's
    /// view): what the mount turns its pivot by (`mounted_rig_apart`).
    rig_turn_before: [f32; 4],
    /// The left shoulder (`docs/spec/left-shoulder.md`): its controller;
    /// whether a firearm is in hand, as the frame's keys last said
    /// (`HasFirearmInHands`, which its putting back asks); whether a wall
    /// has taken it off (`FirearmController._disableLeftStanceByOverlap`);
    /// and whether the body played an animated interaction last frame.
    left_stance: crate::weapons::left_stance::LeftStance,
    left_stance_firearm: bool,
    left_stance_overlap: bool,
    left_stance_interaction: bool,
    /// The wall's wait (`float_7`) and whether the inventory was open on the
    /// last frame it was looked at (`bool_12`).
    left_stance_overlap_clock: f32,
    left_stance_inventory_was: bool,
}
