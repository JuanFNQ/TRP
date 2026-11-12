//! Procedural weapon motion.
//! As specified in `docs/spec/weapon-handling.md`: the springs, the reaction to turning and moving, the walk bob, and what
//! a shot does to the weapon and the camera.
//!
//! Everything is in the game's space. Weapon values are in the weapon's
//! bone space (x right, -y along the barrel, z up), angles in degrees.

use trp_data::formats::curve::Curve;
use trp_data::character::player::{effector, EffectorVal, HandsData, SpringData, TurnAwayVal, WalkEntry};
use trp_data::weapons::states::FirstPersonPose;

use crate::PointOfView;

use crate::base::math::{
    inverse_lerp, lerp, quat_axis, quat_euler, quat_lerp, quat_mul, quat_rotate, quat_slerp, quat_to_euler, smooth_damp,
};
use crate::weapons::breath::{Breath, Wind};
use crate::weapons::hand_shake::HandShake;
use crate::weapons::recoil::{Recoil, RecoilSettings, Rng};

mod springs;
mod settings;
mod input;
mod creation;
mod zoom;
mod feeds;
mod fixed_step;
mod rests;
mod aiming;
mod bob;
mod transforms;
mod camera;

pub use springs::*;
pub use settings::*;
pub use input::*;
use springs::{tilt_by_speed_and_stance};
use settings::{HELD_CLOSER, SIGHT_LINE_LEAST, SIGHT_LINE_RATE};
use input::{ForceReact, Shake, TurnAway};
use camera::{from_view, to_view};

/// Footsteps further apart than this do not retune the bob.
const MAX_STEP_INTERVAL: f32 = 1.2;
/// Seconds the camera stays at its after-shot rate before recovering.
const CAMERA_SNAP_DELAY: f32 = 0.2;
/// Rate per second at which the lean's reach onto the weapon changes when
/// the player stops or starts moving.
const TILT_BLEND_SPEED: f32 = 5.0;
/// How much of the field of view is given up while aiming over iron
/// sights, in degrees. Sprinting does not change it: the game has code for
/// a 10 degree drop, but nothing ever switches it on, and the recorded
/// field of view stays put through a sprint.
pub(crate) const AIM_FOV_DROP: f32 = 15.0;

/// The depth of overlap, in metres, from which the turn-away is told how
/// far back the weapon is already held (and blind fire is refused).
const OVERLAP_THAT_COUNTS: f32 = 0.02;
/// How far ahead of the moved bone, along its barrel, the point lies that
/// the turn-away turns the weapon about, in metres.
const TURN_AWAY_PIVOT: f32 = 0.2;
/// The share of the view's pitch by which the elbows' shifts are turned.
const ELBOW_SHIFT_PITCH: f32 = 0.65;

/// The figures the left shoulder places the weapon by (`PlayerBones`, the
/// player prefab's: `leftStanceRotationAndPositionChangeSpeed` 10,
/// `leftStanceReturnBackSpeed` 20, `probe script ... PlayerBones`,
/// `local\playerbones_s133.txt`; the code's own: `_offsetForRootAimInLeftStance`
/// 0.2 and `_offsterDirrectionForLeftStanceInAim`).
const LEFT_SHOULDER_SPEED: f32 = 10.0;
const LEFT_SHOULDER_BACK_SPEED: f32 = 20.0;
const LEFT_SHOULDER_AIM_REACH: f32 = 0.2;
const LEFT_SHOULDER_AIM_WAY: [f32; 3] = [0.13, -0.305, 0.023];
/// How much nearer the aimed eye is put with the left shoulder in, at the
/// widest squeeze (`ProceduralWeaponAnimation._aimLeftStanceAdditionalOffset`).
const LEFT_SHOULDER_AIM_NEARER: f32 = -0.06;

/// The moved bone as `Hands::weapon_transform_shifted` places it, in its
/// parent's axes: as the body (and the left shoulder) carry it, and then as
/// the right palm holds it too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shifted {
    pub carried: ([f32; 3], [f32; 4]),
    pub held: ([f32; 3], [f32; 4]),
}

/// What placing the weapon on the left shoulder goes by, each frame
/// (`PlayerBones.ShiftWeaponRoot`'s arguments; `weapon_transform_shifted`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LeftShoulder {
    /// The body's `Leftstance_curve`.
    pub curve: f32,
    /// In the sprint's movement state.
    pub sprinting: bool,
    /// The controller's `LastAnimValue`.
    pub anim_value: bool,
    /// The weapon's motion aiming (`ProceduralWeaponAnimation.IsAiming`).
    pub aiming: bool,
    /// The forward key held (`Player.IsForwardInputDirection`).
    pub forward: bool,
    /// The body's frame time (`Player._bodyTime`).
    pub dt: f32,
    /// The ribcage's own place in its parent (`Body::ribcage_local`).
    pub ribcage_local: [f32; 3],
    /// A gesture playing on the body, in first person
    /// (`AnimatedInteractions.IsInteractionPlaying`; the left hand using a
    /// thing counts the same in the game, and none is built).
    pub interaction: bool,
}

/// What the left shoulder keeps between frames (`PlayerBones`:
/// `targetLSPosition`, `_targetLSInAimPosition` in the parent's axes,
/// `_lastWeaponRotationInLeftStance`, `_gotFullLeftStanceLastFrame`,
/// `_wasMovingInLsAim`), in the rig's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LeftShoulderState {
    target: [f32; 3],
    target_in_aim: [f32; 3],
    last_rotation: [f32; 4],
    got_full_last_frame: bool,
    was_moving_in_aim: bool,
}

impl Default for LeftShoulderState {
    fn default() -> Self {
        Self { target: [0.0; 3], target_in_aim: [0.0; 3], last_rotation: [0.0, 0.0, 0.0, 1.0], got_full_last_frame: false, was_moving_in_aim: false }
    }
}

pub struct Hands {
    pub settings: HandsSettings,
    /// The left shoulder's own state between frames, and its curve as the
    /// weapon was last placed by it (`weapon_transform_shifted`).
    left_shoulder: LeftShoulderState,
    left_shoulder_curve: f32,
    /// The moved bone as last placed, in its parent's axes: the left
    /// shoulder's overlap line starts from it (`shoulder_line`).
    shoulder_root: ([f32; 3], [f32; 4]),
    pub hands_position: Spring,
    pub hands_rotation: Spring,
    pub camera_rotation: Spring,
    pub camera_position: Spring,
    pub sway: Sway,
    force_react: ForceReact,

    position_velocity: [f32; 3],
    position_velocity_rate: [f32; 3],
    position_acceleration: [f32; 3],
    position_acceleration_rate: [f32; 3],
    last_position_velocity: [f32; 3],
    rotation_velocity: [f32; 2],
    rotation_velocity_rate: [f32; 2],
    rotation_acceleration: [f32; 2],
    rotation_acceleration_rate: [f32; 2],
    last_rotation_velocity: [f32; 2],
    rotation_velocity_sum: [f32; 2],
    rotation_acceleration_sum: [f32; 2],
    last_forward: Option<[f32; 3]>,
    /// Where the weapon's rig points: the view as the last frame left it.
    /// The game turns the rig to the view after the frame's physics steps
    /// have been worked, so those steps see the frame before's view.
    rig_view: Option<[f32; 2]>,

    walk_clocks: Vec<f32>,
    /// The lame bob's clocks, and whether it is the one played
    /// (`WalkEffector.CurrentWalkPreset`: a damaged leg, no painkiller).
    walk_clocks_lame: Vec<f32>,
    lame: bool,
    /// The load's own bob's clocks, normal and lame (`OverweightPresets`).
    walk_clocks_load: Vec<f32>,
    walk_clocks_load_lame: Vec<f32>,
    /// The arms' share of the sway spring's softening as health last gave it
    /// (`_swayFactor`: half for one damaged arm, all for both), which the load
    /// is added to.
    arms_sway: f32,
    walking: bool,
    pub step_frequency: f32,

    sway_strength: f32,
    displacement_strength: f32,
    /// Reciprocal of the time to bring the sights up, per second.
    pub aiming_speed: f32,
    /// A heavy weapon's sway as the sights come up (weapon-handling spec,
    /// "Aim sway"): its strength (`_aimSwayStrength`), the direction drawn
    /// as the sights last came up (`_aimSwayDirection`), the time since
    /// then by the engine's clock (`_aimSwayBlender`, which goes by
    /// `Time.time`), and the draws it is made of.
    aim_sway_strength: f32,
    aim_sway_direction: [f32; 3],
    aim_sway_since: f32,
    aim_sway_random: crate::base::unity_random::UnityRandom,
    /// A direction a host sets in place of the drawn ones (the recording's,
    /// for a comparison).
    pub aim_sway_override: Option<[f32; 3]>,
    /// The aiming speed and the aim sway's strength a controller set by
    /// hand (`ManualSetVariables`: the rangefinder's), kept over what the
    /// weapon's weight and ergonomics would give.
    manual: Option<(f32, f32)>,
    /// Aiming leaves the field of view as set (`keep_fov_aimed`).
    fov_kept_aimed: bool,
    aiming: bool,
    /// The body's camera bone as its clips turn it, set by the host each
    /// frame when it runs the body's animator.
    pub body_camera: Option<[f32; 4]>,
    accumulator: f32,

    pub recoil: Recoil,
    breath: Breath,
    /// The hand shake after a quick draw that leaves the arms empty
    /// (`crate::hand_shake`): off until a host starts it.
    hand_shake: HandShake,
    /// Whether a combo unit on the weapon was on when the hip penalty was
    /// last worked out (`update_hip_inaccuracy`).
    combo_on: bool,
    /// 0 prone, 1 crouched, 2 standing.
    pose: usize,
    /// Seconds simulated so far.
    time: f32,
    camera_smooth: Blender,
    tilt_blend: Blender,
    /// How much of the lean currently reaches the weapon and camera, 0 to 1.
    pub possible_tilt: f32,
    /// The camera's rotation as last blended toward the weapon's recoil.
    camera_follow: Option<[f32; 4]>,
    camera_follow_target: [f32; 4],
    camera_follow_speed: f32,
    fov: f32,
    fov_target: f32,
    fov_blend: f32,
    fov_time: f32,
    /// Physics steps since the last rendered frame.
    steps_since_frame: u32,
    /// Shots fired this frame and not yet felt (their strengths): the game
    /// feels a shot after the frame's pose (`Hands::shoot`).
    shots_pending: Vec<f32>,
    /// The camera's place as last drawn.
    camera_local: [f32; 3],
    /// The first-person poses of the body's states that are blended in or
    /// on their way out: each with its clock in seconds and whether it is
    /// coming in ("First-person poses of body states" in the spec).
    poses: Vec<(FirstPersonPose, f32, bool)>,
    /// What they add to the rest of the weapon's position spring, of its
    /// rotation spring and of the camera's rotation spring.
    pose_position: [f32; 3],
    pose_rotation: [f32; 3],
    pose_camera: [f32; 3],
    /// The lean as it reaches the weapon and camera: the other part of
    /// those rests.
    lean_rest: f32,
    /// How fast such a pose comes in and goes out. A host that knows the
    /// character's inertia sets it (`MovementSettings::weapon_flip_speed`);
    /// 1 is where the game starts it.
    pub weapon_flip_speed: f32,
    /// Whether the weapon is held closer to the body just now, as the
    /// game last worked it out (when the weapon was taken in hand, and
    /// when its stock was last folded or unfolded).
    closer: bool,
    /// Whether the stock is folded just now (`set_folded`).
    folded: bool,
    /// The iron sights' line as the host last read it off the rig, the
    /// one in force (taken as the sights come up, and none once they are
    /// down), and how far the weapon is turned for it just now.
    sight_noted: Option<SightLine>,
    sight: SightLine,
    sight_angle: f32,
    /// The weapon's turn about its barrel onto a canted sight just now
    /// (`_scopeRotation`), in the axes of the bone the procedural motion
    /// moves: eased toward the sight's own turn while aiming and back to
    /// none while not.
    scope_rotation: [f32; 4],
    /// Another sight was chosen with the sights up: its line is taken from
    /// the next reading of the rig (`CycleScopes` measures it again).
    retake_line: bool,
    /// The muzzle in the axes of the bone the procedural motion moves, as
    /// the host last read it off the rig: what the weapon is turned about
    /// to level its sights' line.
    muzzle: Option<[f32; 3]>,
    turn_away: TurnAway,
    /// Where the player looked on the last physics step: yaw, pitch.
    view: [f32; 2],
    /// The bone the moved bone hangs from, in the frame the rig hangs in:
    /// its place and its turn. It starts at the weapon's rest; a host
    /// whose rig plays the weapon's clips sets it each frame.
    pub weapon_root: ([f32; 3], [f32; 4]),
    /// Whose eyes the character is seen with: for anyone's but its own the
    /// motion is the game's cut-down one (`docs/spec/third-person.md`).
    point_of_view: PointOfView,
    /// Which effectors run, as the prefab's mask has them
    /// (`trp_data::player::effector`). A host with a character the game
    /// would give another mask sets it: a bot's is the turn-away alone.
    /// Of its bits only the shot's, and for a character seen from outside
    /// the turn-away's and the aim pose's, are looked at; the rest of what
    /// the character's own eyes see runs as the prefab has it.
    pub mask: u32,
    /// Two systems of the features' list (`crate::Systems`, set by
    /// `set_systems`): the weapon's motion, and the recoil.
    motion: bool,
    recoil_on: bool,
    /// How far up the sights are for the aim pose of a character seen
    /// from outside, 0 to 1 (`CustomEffector.PoseBlender`).
    aim_pose: Blender,
    /// The head's free look (`docs/spec/free-look.md`): its turn, which
    /// the camera adds to its rotation spring's figure; whether the mouse
    /// has the head; and whether it had when the sights were last up with
    /// the look's frame run (the aimed field of view goes by the change).
    head: [f32; 3],
    head_control: bool,
    head_control_seen: bool,
    /// Blind fire (`docs/spec/blind-fire.md`, "The weapon and the camera"):
    /// its blender (`BlindfireBlender`), read off the frames' own clock
    /// (the engine's `Time.time`, which counts the frame being drawn); a
    /// target asked during the frame, which takes this frame's moment; the
    /// blender's value as this frame's camera pass read it; its strength by
    /// the view's pitch, kept while the blender is at 0; and what it adds to
    /// the camera rotation spring's rest.
    blind: Blender,
    blind_clock: f32,
    blind_asked: Option<f32>,
    blind_value: f32,
    blind_strength: f32,
    blind_camera: [f32; 3],
    /// Mounting (`docs/spec/mounting.md`): the mount as the motion was last
    /// told it (`SetMountingData`: `IsMountedState`, the pivot, at a side),
    /// whether the mount's way of moving is in force (`MountingStrategy`,
    /// from the mount's start to its state's end), whether the bipod is out
    /// (`IsBipodUsed`), the aiming weight (`_aimingWeight`, which the mounted
    /// camera goes by), and the rig's origin from the camera's object in
    /// the camera's axes (the mounted camera follows the weapon to the mount).
    mounted: Option<crate::weapons::mounting::HandsMount>,
    mount_strategy: bool,
    bipod_used: bool,
    aim_weight: f32,
    rig_shift: [f32; 3],
    /// The tactical reload's blend on this frame, 0 to 1, as the weapon's
    /// controller has it (`Firearm::tactical_reload`; `set_tactical_reload`).
    tactical_blend: f32,
}

/// The sway's and the displacement's strengths as the motion is told to
/// forget its weapon (`ClearPreviousWeapon`).
const CLEARED_STRENGTH: f32 = 0.5;

/// What the weapon motion keeps of the last firearm for hands that hold
/// none (`Hands::keep_from`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kept {
    pub rotation_center: [f32; 3],
    pub rotation_center_no_stock: [f32; 3],
    pub recoil_pivot: [f32; 3],
    pub hand_damping: f32,
    pub ergonomic_weight: f32,
    /// The sway's strength as last worked out (`MotionReact.SwayFactors`).
    pub sway_strength: f32,
}

impl Hands {
}

#[cfg(test)]
mod tests;
