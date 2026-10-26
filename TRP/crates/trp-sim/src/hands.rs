//! Procedural weapon motion, as specified in `docs/spec/weapon-handling.md`:
//! the springs, the reaction to turning and moving, the walk bob, and what
//! a shot does to the weapon and the camera.
//!
//! Everything is in the game's space. Weapon values are in the weapon's
//! bone space (x right, -y along the barrel, z up), angles in degrees.

use trp_data::curve::Curve;
use trp_data::player::{effector, EffectorVal, HandsData, SpringData, TurnAwayVal, WalkEntry};
use trp_data::states::FirstPersonPose;

use crate::PointOfView;

use crate::math::{
    inverse_lerp, lerp, quat_axis, quat_euler, quat_lerp, quat_mul, quat_rotate, quat_slerp, quat_to_euler, smooth_damp,
};
use crate::breath::{Breath, Wind};
use crate::recoil::{Recoil, RecoilSettings, Rng};

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

/// A value that moves toward a target at a fixed rate, optionally after a
/// delay. The game reads these off the clock instead of stepping them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Blender {
    start_value: f32,
    start_time: f32,
    target: f32,
    speed: f32,
}

impl Blender {
    pub(crate) fn new(value: f32, speed: f32) -> Self {
        Self { start_value: value, start_time: 0.0, target: value, speed }
    }

    pub(crate) fn value(&self, now: f32) -> f32 {
        let elapsed = (now - self.start_time).max(0.0);
        let moved = self.start_value + elapsed * self.speed * (self.target - self.start_value).signum();
        if self.start_value > self.target {
            moved.clamp(self.target, self.start_value)
        } else {
            moved.clamp(self.start_value, self.target)
        }
    }

    pub(crate) fn set_target(&mut self, target: f32, now: f32) {
        if self.target != target {
            self.start_value = self.value(now);
            self.start_time = now;
            self.target = target;
        }
    }

    /// Jumps to `value` and holds it for `delay` seconds before moving on.
    pub(crate) fn jump(&mut self, value: f32, delay: f32, now: f32) {
        self.start_time = now + delay;
        self.start_value = value;
    }
}

/// How far a lean reaches the weapon: all the way when slow or low, not at
/// all when walking upright at full speed.
fn tilt_by_speed_and_stance(speed: f32, stance: f32) -> f32 {
    ((1.6 - 0.9 * speed - 0.7 * stance) / 0.63).clamp(0.0, 1.0)
}

/// The game's frame-stepped spring.
#[derive(Debug, Clone)]
pub struct Spring {
    pub zero: [f32; 3],
    pub current: [f32; 3],
    velocity: [f32; 3],
    force: [f32; 3],
    /// Pushes the step takes without its limit (`ForceAccumulatorLimitless`).
    limitless: [f32; 3],
    has_force: bool,
    pub input_intensity: f32,
    return_speed: f32,
    pub damping: f32,
    acceleration_max: f32,
}

impl Spring {
    /// Takes another's motion (where it is, how fast, the pushes it has
    /// still to take), keeping its own figures.
    pub fn take_state(&mut self, from: &Spring) {
        (self.current, self.velocity, self.force, self.limitless, self.has_force) = (from.current, from.velocity, from.force, from.limitless, from.has_force);
    }

    pub fn new(data: SpringData) -> Self {
        Self {
            zero: [0.0; 3],
            current: [0.0; 3],
            velocity: [0.0; 3],
            force: [0.0; 3],
            limitless: [0.0; 3],
            has_force: false,
            input_intensity: data.input_intensity,
            return_speed: data.return_speed,
            damping: data.damping,
            acceleration_max: data.acceleration_max,
        }
    }

    pub fn add(&mut self, component: usize, value: f32) {
        self.force[component] += value;
        self.has_force = true;
    }

    /// A push the step takes whole, past the spring's limit
    /// (`AddAccelerationLimitless`).
    pub fn add_limitless(&mut self, component: usize, value: f32) {
        self.limitless[component] += value;
        self.has_force = true;
    }

    /// One physics step. The formula has no time step in it.
    pub fn step(&mut self) {
        for i in 0..3 {
            if self.has_force {
                let limited = self.force[i].clamp(-self.acceleration_max, self.acceleration_max);
                self.velocity[i] += (limited + self.limitless[i]) * self.input_intensity;
            }
            // The spring is drawn to its zero, and the zero is added again
            // when the value is read: a held offset counts twice at rest.
            self.velocity[i] -= (self.current[i] - self.zero[i]) * self.return_speed;
            self.velocity[i] *= self.damping;
            self.current[i] += self.velocity[i];
        }
        self.force = [0.0; 3];
        self.limitless = [0.0; 3];
        self.has_force = false;
    }

    pub fn value(&self) -> [f32; 3] {
        [self.zero[0] + self.current[0], self.zero[1] + self.current[1], self.zero[2] + self.current[2]]
    }
}

/// The sway spring: an underdamped oscillator integrated exactly.
#[derive(Debug, Clone)]
pub struct Sway {
    pub position: [f32; 3],
    velocity: [f32; 3],
    frequency: f32,
    damping: f32,
}

impl Sway {
    pub fn new(frequency: f32, damping: f32) -> Self {
        Self { position: [0.0; 3], velocity: [0.0; 3], frequency, damping: damping.min(0.9999) }
    }

    pub fn kick(&mut self, component: usize, velocity: f32) {
        self.velocity[component] += velocity;
    }

    /// New tuning, the spring keeping where it is and how fast it goes.
    pub fn set(&mut self, frequency: f32, damping: f32) {
        (self.frequency, self.damping) = (frequency, damping.min(0.9999));
    }

    pub fn step(&mut self, dt: f32) {
        let decay = self.frequency * self.damping;
        let alpha = self.frequency * (1.0 - self.damping * self.damping).sqrt();
        let e = (-decay * dt).exp();
        let (s, c) = (alpha * dt).sin_cos();
        for i in 0..3 {
            let x = self.position[i];
            let b = (self.velocity[i] + decay * x) / alpha;
            self.position[i] = e * (x * c + b * s);
            self.velocity[i] = -e * ((x * decay - b * alpha) * c + (x * alpha + b * decay) * s);
        }
    }
}

/// `globals.json` `config.Aiming`: how weight and ergonomics set the time to aim.
#[derive(Debug, Clone, Copy)]
pub struct AimingConfig {
    pub light_weight: f32,
    pub heavy_weight: f32,
    pub max_time_light: f32,
    pub max_time_heavy: f32,
    pub min_time_light: f32,
    pub min_time_heavy: f32,
}

pub struct HandsSettings {
    pub data: HandsData,
    /// Seconds per physics step.
    pub fixed_timestep: f32,
    /// Angular frequency and damping ratio of the sway spring for a
    /// healthy, unburdened character. The settings object supplies them and
    /// overrides the prefab's.
    pub sway_frequency: f32,
    pub sway_damping: f32,
    /// The same for both arms damaged (`SWAY_FREQ_NORMAL_DAMAGED.y`,
    /// `SWAY_DAMPING_NORMAL_DAMAGED.y`); one arm takes half the way.
    pub sway_damaged: [f32; 2],
    pub sway_strength_per_kg: Curve,
    pub displacement_strength_per_kg: Curve,
    pub aiming: AimingConfig,
    /// Weight of the weapon in kilograms and the sum of its ergonomics.
    pub weapon_weight: f32,
    pub weapon_ergonomics: f32,
    /// The point the weapon swings about, in its own bone space, and the
    /// point recoil turns it about.
    pub rotation_center: [f32; 3],
    pub recoil_pivot: [f32; 3],
    /// Damping of the weapon's position spring while a weapon is in hand.
    pub hand_damping: f32,
    /// Scale of the procedural motion while aiming.
    pub aim_intensity: f32,
    /// The camera's aiming rate right after a shot.
    pub camera_snap: f32,
    /// Slowest and fastest blend per frame with which the camera follows
    /// the weapon's recoil, and how fast that blend changes per second.
    pub camera_follow_range: [f32; 2],
    pub camera_follow_step: f32,
    pub recoil: RecoilSettings,
    /// Scale of breathing by stance: prone, crouched, standing.
    pub intensity_by_pose: [f32; 3],
    /// What a shot from the hip adds to the muzzle's wander, how long that
    /// holds in seconds, and how fast it fades per second.
    pub breath_gain: [f32; 3],
    /// Seed of the recoil's random numbers.
    pub seed: u64,
    /// What a hit's push is scaled by (`AimPunchMagnitude` of the globals,
    /// the force reaction's `WiggleMagnitude`).
    pub aim_punch: f32,
    /// The player's field-of-view setting: vertical degrees, 50 to 75.
    pub field_of_view: f32,
    /// The player's head-bobbing setting, 0.2 to 1: how much of the
    /// animated camera rotation reaches the camera.
    pub head_bobbing: f32,
    /// What the character's skills add to the aiming speed, as a fraction
    /// (Aim Drills' `DrawSpeed`).
    pub aim_speed_bonus: f32,
    /// What the weapon's class skill adds to the share its ergonomics is
    /// multiplied by (`WeaponErgonomicsBuff`; `TotalErgonomicsFunc`), 0
    /// untrained.
    pub ergonomics_buff: f32,
    /// How far ahead of the weapon's moving bone the eye sits when aiming,
    /// along the barrel, in metres: the nearer of the weapon's two aim planes.
    pub aim_plane_depth: f32,
    /// The weapon's sights as the game picks them (`crate::sights`), for a
    /// weapon that has any: which one is aimed by, and what an optic does
    /// to the main camera's field of view and to the eye's place. Without,
    /// the eye goes to `aim_plane_depth` as over iron sights.
    pub sights: Option<crate::sights::Sights>,
    /// From the bone the procedural motion moves to the weapon's far end,
    /// in metres: the length of the line that feels for obstacles.
    pub weapon_length: f32,
    /// The bone that moved bone hangs from, in the frame the rig hangs
    /// in, with the weapon at rest: its place and its turn.
    pub weapon_root: ([f32; 3], [f32; 4]),
    /// The weapon is a pistol: it turns away from obstacles by a pose of
    /// its own.
    pub pistol: bool,
    /// The weapon is held closer to the body as built: one of compact
    /// handling with nothing on it that reaches the shoulder
    /// (`CheckShouldMoveWeaponCloser`). And the point it then swings
    /// about (`RotationCenterNoStock`).
    pub held_closer: bool,
    pub rotation_center_no_stock: [f32; 3],
    /// The weapon with its stock folded, if it has one that folds.
    pub folded: Option<FoldedHold>,
    /// What the recoil's strengths are made of (none for hands that hold
    /// no firearm), and the class skill's suppression they were last
    /// worked out with (0 untrained, as made).
    pub recoil_force: Option<RecoilForce>,
    pub recoil_suppression: f32,
    /// The weapon's mount figures (`crate::mounting::WeaponMount`) and the
    /// mount's own (its camera's speed, the sway on the bipod): what the
    /// motion goes by while mounted (mounting spec, "The weapon's motion
    /// while mounted").
    pub mount: crate::mounting::WeaponMount,
    pub mount_settings: crate::mounting::Settings,
    /// What a mount adds to the ergonomics' share just now (the class skill's
    /// gain while mounted: `Hands::set_mount_ergonomics`), 0 off a mount.
    pub mount_ergonomics: f32,
}

/// What a weapon's two recoil strengths are made of (`NewRecoilShotEffect.
/// RecalculateRecoilParamsOnChangeWeapon`): the template's forces and the
/// aiming settings' bonuses, the share the build's parts make of the forces
/// (`1 + modsFactorRecoil`, at least 0) as built and with the stock folded,
/// and the prefab's ranges and multiplier. The class skill's suppression
/// (`RecoilSupression.x`, 0.003 a level) takes its share off the forces:
/// `max(0, parts × (1 − x))`; Recoil Control's (`.y`) is always 0 here, its
/// skill being out of the game's list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecoilForce {
    pub up: f32,
    pub back: f32,
    pub vert_bonus: f32,
    pub back_bonus: f32,
    pub multiplier: f32,
    pub rotation_range: [f32; 2],
    pub position_range: [f32; 2],
    pub parts: f32,
    pub folded_parts: Option<f32>,
}

impl RecoilForce {
    /// The two strengths for a share of the parts and a suppression.
    pub fn strengths(&self, parts: f32, suppression: f32) -> ([f32; 2], [f32; 2]) {
        let parts = (parts * (1.0 - suppression)).max(0.0);
        let scale = |force: f32, bonus: f32| (force * parts + bonus) * self.multiplier;
        let (rotation, position) = (scale(self.up, self.vert_bonus), scale(self.back, self.back_bonus));
        (
            [self.rotation_range[0] * rotation, self.rotation_range[1] * rotation],
            [self.position_range[0] * position, self.position_range[1] * position],
        )
    }
}

/// What a folded stock changes of the weapon's motion: the recoil's two
/// strengths, which lose the stock's share, and whether the weapon is then
/// held closer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoldedHold {
    pub rotation_strength: [f32; 2],
    pub position_strength: [f32; 2],
    pub held_closer: bool,
}

/// How far a weapon held closer is drawn back, in metres
/// (`ProceduralWeaponAnimation.ZeroAdjustments`).
const HELD_CLOSER: f32 = 0.05;

/// The line through a weapon's two iron sights, as measured on its rig
/// (`rig::SightNodes::line`): how far it is pitched from the weapon's own
/// way, in degrees (the front sight higher is positive), and where it
/// passes the depth of the sight's aim bone, as a shift of the eye from
/// that bone: along the barrel, and up.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SightLine {
    pub angle: f32,
    pub shift: [f32; 2],
}

/// How fast the weapon is turned to level its sights' line and back, as a
/// share of what is left a second (`ApplyAimingAlignment`), and the angle
/// under which it is not turned at all, in degrees.
const SIGHT_LINE_RATE: f32 = 5.0;
const SIGHT_LINE_LEAST: f32 = 0.002;

/// Where the sight in use is, for placing the eye while aiming. Positions
/// are beside the weapon root, in the game's axes.
#[derive(Debug, Clone, Copy)]
pub struct AimPoint {
    /// The sight's aim bone.
    pub bone: [f32; 3],
    /// Origin of the bone the procedural motion moves, and the direction
    /// its barrel points.
    pub origin: [f32; 3],
    pub barrel: [f32; 3],
    /// The way the eye looks along the aim bone
    /// (`crate::rig::SightNodes::line_in_moved`), which the eye's target
    /// slides along in automatic fire.
    pub line: [f32; 3],
}

/// Where the body's animation holds the weapon, in the frame the weapon's
/// rig hangs in (x right, y up, z ahead, from the rig's origin, at true
/// size), and how far the first-person weapon follows it, 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Carried {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub authority: f32,
}

/// The body's right palm where its animation has it, in the same frame, and
/// how much of the hold is still the first-person rig's: at 1 the hand goes
/// to the weapon, at 0 the weapon goes to the hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub weight: f32,
}

/// The camera for one rendered frame.
#[derive(Debug, Clone, Copy)]
pub struct CameraPose {
    /// Position and rotation (`xyzw`) beside the weapon root.
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    /// Vertical field of view in degrees.
    pub field_of_view: f32,
}

/// What the rest of the controller tells the hands each step.
#[derive(Debug, Clone, Copy, Default)]
pub struct HandsInput {
    /// Look direction in degrees.
    pub yaw: f32,
    pub pitch: f32,
    /// Movement the controller asked for, and what it got, in metres per second.
    pub motion: [f32; 3],
    pub velocity: [f32; 3],
    /// The animator's speed parameter.
    pub speed: f32,
    /// In the walking state: the bob runs.
    pub walking: bool,
    pub sprinting: bool,
    /// Standing still.
    pub idle: bool,
    /// A foot came down since the last step: the seconds since the
    /// footstep before it.
    pub footstep: Option<f32>,
    /// Smoothed lean, -5 to 5, and the lean being moved to.
    pub tilt: f32,
    pub tilt_target: f32,
    /// Stance being moved to and the smoothed stance, 0 crouched to 1 standing.
    pub pose_level: f32,
    pub smoothed_pose_level: f32,
    pub aiming: bool,
    /// The character lies down (the game's prone flag, up from the key).
    pub prone: bool,
    /// The character crawls: the turn-away from obstacles is off.
    pub crawling: bool,
    /// Length of the rendered frame this step falls in, in seconds; zero
    /// when the host steps once per frame.
    pub frame_dt: f32,
    /// How tired the character is and whether it holds its breath
    /// (`Movement::wind`): rested, unless a host says otherwise.
    pub wind: Wind,
}

/// The turn-away from obstacles ("Turn-away" in the spec): how far the
/// weapon is drawn back and turned for the depth by which it would stick
/// into something.
#[derive(Debug, Clone, Default)]
struct TurnAway {
    /// The depth as last measured, in metres, and how far back along the
    /// barrel the position spring held the weapon then.
    depth: f32,
    held_back: f32,
    /// The value in use, the blend toward the far pose, and the camera's
    /// shift as it follows.
    value: f32,
    far: f32,
    camera_shift: f32,
    /// What the last step made of it: for the moved bone, for the
    /// camera's resting place along the view, and for the elbows' goals,
    /// left then right.
    position: [f32; 3],
    rotation: [f32; 3],
    camera: f32,
    elbows: [[f32; 3]; 2],
}

/// The force reaction's state (`ForceEffector`; weapon-handling spec,
/// "Force reaction"): the force gathered since the last step and its
/// eased sum, the steps still given to it, the blasts' shakes under way,
/// and its draws.
#[derive(Debug, Clone)]
struct ForceReact {
    force: [f32; 3],
    sum: [f32; 3],
    steps_left: i32,
    shakes: Vec<Shake>,
    rng: Rng,
}

/// A blast's shake under way (`GrenadeShake_CO`): whether its first kick
/// has come, the time to its next, the time it has left, its power and its
/// wait.
#[derive(Debug, Clone, Copy)]
struct Shake {
    started: bool,
    next: f32,
    left: f32,
    power: f32,
    wait: f32,
}

impl Shake {
    fn new(strength: f32) -> Self {
        Self { started: false, next: 0.0, left: strength * 0.75, power: strength * 1.2, wait: 0.1 }
    }

    /// A frame of the coroutine: the power of the kick it makes, if it
    /// makes one (the first at once; then, once its wait is up, with less
    /// left, three quarters of the power and a wait a tenth longer, while
    /// any time is left).
    fn frame(&mut self, dt: f32) -> Option<f32> {
        if !self.started {
            self.started = true;
        } else {
            self.next -= dt;
            if self.next > 0.0 {
                return None;
            }
            self.left -= self.wait;
            self.power *= 0.75;
            self.wait *= 1.1;
        }
        (self.left > 0.0).then(|| {
            self.next = self.wait;
            self.power
        })
    }
}

impl Default for ForceReact {
    fn default() -> Self {
        Self { force: [0.0; 3], sum: [0.0; 3], steps_left: -1, shakes: Vec::new(), rng: Rng::new(0x464f_5243) }
    }
}

/// `Vector3.Slerp`: the direction turned by the share of the angle between,
/// the length blended; straight where either is nought or they are as
/// one (`PROVISIONAL`: the engine's own code is not read).
pub(crate) fn slerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let length = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (la, lb) = (length(a), length(b));
    let straight = || std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
    if la < 1e-5 || lb < 1e-5 {
        return straight();
    }
    let dot = ((a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (la * lb)).clamp(-1.0, 1.0);
    if dot > 1.0 - 1e-5 || dot < -1.0 + 1e-5 {
        return straight();
    }
    let angle = dot.acos() * t;
    let ua = a.map(|x| x / la);
    // The part of b's direction square to a's, as a unit.
    let ub = b.map(|x| x / lb);
    let side: [f32; 3] = std::array::from_fn(|k| ub[k] - ua[k] * dot);
    let ls = length(side);
    let length_now = la + (lb - la) * t;
    std::array::from_fn(|k| (ua[k] * angle.cos() + side[k] / ls * angle.sin()) * length_now)
}

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
    aim_sway_random: crate::unity_random::UnityRandom,
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
    mounted: Option<crate::mounting::HandsMount>,
    mount_strategy: bool,
    bipod_used: bool,
    aim_weight: f32,
    rig_shift: [f32; 3],
}

impl HandsSettings {
    /// The weapon's weight as its ergonomics lighten it: what the sway
    /// scales with, and what aiming costs the arms by.
    /// The aim sway's strength (`UpdateWeaponVariables`): how far the
    /// weapon's weight, less its ergonomics' share, is from where the sway
    /// starts to where it is whole.
    pub fn aim_sway_strength(&self) -> f32 {
        let a = &self.data.aim_sway;
        let e = (self.total_ergonomics() / 100.0).clamp(0.0, 1.0);
        inverse_lerp(a.starts, a.full, self.weapon_weight * (1.0 - e))
    }

    pub fn ergonomic_weight(&self) -> f32 {
        self.weapon_weight * (1.0 - self.total_ergonomics().max(0.0).sqrt() / 25.0)
    }

    /// The ergonomics the character handles the weapon with
    /// (`TotalErgonomics`): its own, times one and the class skill's buff.
    pub fn total_ergonomics(&self) -> f32 {
        (self.weapon_ergonomics * (1.0 + self.ergonomics_buff + self.mount_ergonomics)).max(0.0)
    }

    /// What goes by the weapon's weight and ergonomics
    /// (`UpdateWeaponVariables`): the sway's and the displacement's
    /// strengths and the aiming speed.
    fn by_weight(&self) -> (f32, f32, f32) {
        // The weight that sway scales with shrinks as ergonomics rise.
        let ergonomic_weight = self.ergonomic_weight();
        let a = self.aiming;
        let t = inverse_lerp(a.light_weight, a.heavy_weight, self.weapon_weight);
        let (slowest, fastest) = (lerp(a.max_time_light, a.max_time_heavy, t), lerp(a.min_time_light, a.min_time_heavy, t));
        let e = (self.total_ergonomics() / 100.0).clamp(0.0, 1.0);
        let k = if e < 0.25 { 0.25 + 3.0 * e * e } else { 2.0 * e - e * e };
        (
            self.sway_strength_per_kg.evaluate(ergonomic_weight),
            self.displacement_strength_per_kg.evaluate(ergonomic_weight),
            1.0 / lerp(slowest, fastest, k) * (1.0 + self.aim_speed_bonus),
        )
    }
}

impl Hands {
    /// The same weapon built anew with another part, taken over mid-way
    /// (a reload's change of magazine: `Firearm::carry_on_from`): the
    /// motion as the build it replaces left it, its springs' and sway's
    /// motion, the breath, its clocks, blends, poses, lean, recoil, view,
    /// the field of view and the rest of what runs, on this build's own
    /// figures (its settings, its springs' and sway's constants, its
    /// strengths, its sights' line and muzzle as read off its own rig, its
    /// hold). The app's: the game changes the part on the weapon's own
    /// model, and nothing of its motion starts again.
    pub fn carry_on_from(&mut self, from: &Hands) {
        for (mine, theirs) in [(&mut self.hands_position, &from.hands_position), (&mut self.hands_rotation, &from.hands_rotation), (&mut self.camera_rotation, &from.camera_rotation), (&mut self.camera_position, &from.camera_position)] {
            mine.take_state(theirs);
        }
        (self.sway.position, self.sway.velocity) = (from.sway.position, from.sway.velocity);
        self.force_react = from.force_react.clone();
        (self.position_velocity, self.position_velocity_rate, self.position_acceleration, self.position_acceleration_rate, self.last_position_velocity) =
            (from.position_velocity, from.position_velocity_rate, from.position_acceleration, from.position_acceleration_rate, from.last_position_velocity);
        (self.rotation_velocity, self.rotation_velocity_rate, self.rotation_acceleration, self.rotation_acceleration_rate, self.last_rotation_velocity) =
            (from.rotation_velocity, from.rotation_velocity_rate, from.rotation_acceleration, from.rotation_acceleration_rate, from.last_rotation_velocity);
        (self.rotation_velocity_sum, self.rotation_acceleration_sum, self.last_forward, self.rig_view) = (from.rotation_velocity_sum, from.rotation_acceleration_sum, from.last_forward, from.rig_view);
        (self.walk_clocks, self.walk_clocks_lame, self.lame, self.walking) = (from.walk_clocks.clone(), from.walk_clocks_lame.clone(), from.lame, from.walking);
        (self.aim_sway_direction, self.aim_sway_since, self.aim_sway_random) = (from.aim_sway_direction, from.aim_sway_since, from.aim_sway_random.clone());
        (self.aiming, self.body_camera, self.accumulator) = (from.aiming, from.body_camera, from.accumulator);
        let settings = self.recoil.settings.clone();
        self.recoil = from.recoil.clone();
        self.recoil.settings = settings;
        // The breath goes on as it was, its hip penalty with it (PROVISIONAL:
        // whether a reload's change of magazine works it out again by the
        // new build's ergonomics, `WeaponModified`, was not read).
        (self.breath, self.combo_on, self.pose, self.time) = (from.breath.clone(), from.combo_on, from.pose, from.time);
        (self.camera_smooth, self.tilt_blend, self.possible_tilt) = (from.camera_smooth, from.tilt_blend, from.possible_tilt);
        (self.camera_follow, self.camera_follow_target, self.camera_follow_speed) = (from.camera_follow, from.camera_follow_target, from.camera_follow_speed);
        (self.fov, self.fov_target, self.fov_blend, self.fov_time) = (from.fov, from.fov_target, from.fov_blend, from.fov_time);
        (self.steps_since_frame, self.camera_local) = (from.steps_since_frame, from.camera_local);
        self.shots_pending = from.shots_pending.clone();
        (self.poses, self.pose_position, self.pose_rotation, self.pose_camera, self.lean_rest) = (from.poses.clone(), from.pose_position, from.pose_rotation, from.pose_camera, from.lean_rest);
        (self.turn_away, self.view, self.weapon_root, self.aim_pose) = (from.turn_away.clone(), from.view, from.weapon_root, from.aim_pose);
        (self.head, self.head_control, self.head_control_seen) = (from.head, from.head_control, from.head_control_seen);
        (self.scope_rotation, self.sight_angle) = (from.scope_rotation, from.sight_angle);
        (self.blind, self.blind_clock, self.blind_asked, self.blind_value, self.blind_strength, self.blind_camera) =
            (from.blind, from.blind_clock, from.blind_asked, from.blind_value, from.blind_strength, from.blind_camera);
        (self.left_shoulder, self.left_shoulder_curve) = (from.left_shoulder, from.left_shoulder_curve);
    }

    pub fn new(settings: HandsSettings) -> Self {
        let d = &settings.data;
        let (sway_strength, displacement_strength, aiming_speed) = settings.by_weight();
        let mut hands_position = Spring::new(d.hands_position);
        hands_position.damping = settings.hand_damping;
        let mut hands = Self {
            recoil: Recoil::new(settings.recoil.clone(), settings.seed),
            breath: Breath::new(d.breath.clone(), settings.breath_gain, settings.seed.wrapping_add(1)),
            combo_on: false,
            pose: 2,
            time: 0.0,
            camera_smooth: Blender::new(d.camera_smooth_steady, d.camera_smooth_blend_speed),
            // Starts standing still, where the lean reaches the weapon fully.
            tilt_blend: Blender::new(1.0, TILT_BLEND_SPEED),
            possible_tilt: 1.0,
            camera_follow: None,
            camera_follow_target: [0.0, 0.0, 0.0, 1.0],
            camera_follow_speed: settings.camera_follow_range[0],
            fov: settings.field_of_view,
            fov_target: settings.field_of_view,
            fov_blend: 0.0,
            fov_time: 1.0,
            steps_since_frame: 0,
            left_shoulder: LeftShoulderState::default(),
            left_shoulder_curve: 0.0,
            shoulder_root: ([0.0; 3], [0.0, 0.0, 0.0, 1.0]),
            shots_pending: Vec::new(),
            camera_local: d.camera_offset,
            poses: Vec::new(),
            pose_position: [0.0; 3],
            pose_rotation: [0.0; 3],
            pose_camera: [0.0; 3],
            lean_rest: 0.0,
            weapon_flip_speed: 1.0,
            closer: settings.held_closer,
            folded: false,
            sight_noted: None,
            sight: SightLine::default(),
            sight_angle: 0.0,
            scope_rotation: [0.0, 0.0, 0.0, 1.0],
            retake_line: false,
            muzzle: None,
            turn_away: TurnAway::default(),
            view: [0.0; 2],
            weapon_root: settings.weapon_root,
            point_of_view: PointOfView::FirstPerson,
            mask: d.mask,
            motion: true,
            recoil_on: true,
            aim_pose: Blender::new(0.0, d.aim_pose_speed),
            head: [0.0; 3],
            head_control: false,
            head_control_seen: false,
            blind: Blender::new(0.0, d.blind_fire.speed),
            blind_clock: 0.0,
            blind_asked: None,
            blind_value: 0.0,
            blind_strength: 1.0,
            blind_camera: [0.0; 3],
            mounted: None,
            mount_strategy: false,
            bipod_used: false,
            aim_weight: 0.0,
            rig_shift: [0.0; 3],
            body_camera: None,
            hands_position,
            hands_rotation: Spring::new(d.hands_rotation),
            camera_rotation: Spring::new(d.camera_rotation),
            camera_position: Spring::new(d.camera_position),
            sway: Sway::new(settings.sway_frequency, settings.sway_damping),
            force_react: ForceReact::default(),
            position_velocity: [0.0; 3],
            position_velocity_rate: [0.0; 3],
            position_acceleration: [0.0; 3],
            position_acceleration_rate: [0.0; 3],
            last_position_velocity: [0.0; 3],
            rotation_velocity: [0.0; 2],
            rotation_velocity_rate: [0.0; 2],
            rotation_acceleration: [0.0; 2],
            rotation_acceleration_rate: [0.0; 2],
            last_rotation_velocity: [0.0; 2],
            rotation_velocity_sum: [0.0; 2],
            rotation_acceleration_sum: [0.0; 2],
            last_forward: None,
            rig_view: None,
            walk_clocks: d.walk_normal.iter().map(|e| e.time_shift).collect(),
            walk_clocks_lame: d.walk_lame.iter().map(|e| e.time_shift).collect(),
            lame: false,
            walking: false,
            step_frequency: d.walk_step_frequency,
            sway_strength,
            displacement_strength,
            aiming_speed,
            aim_sway_strength: 0.0,
            aim_sway_direction: [0.0; 3],
            aim_sway_since: f32::INFINITY,
            aim_sway_random: crate::unity_random::UnityRandom::new(settings.seed.wrapping_add(3) as i32),
            aim_sway_override: None,
            manual: None,
            fov_kept_aimed: false,
            aiming: false,
            accumulator: 0.0,
            settings,
        };
        hands.aim_sway_strength = hands.settings.aim_sway_strength();
        // The sights' eye goes by the weapon's squeeze at the field of view.
        let scale = hands.fov_scale();
        if let Some(sights) = hands.settings.sights.as_mut() {
            sights.set_scale(scale);
        }
        // The hip penalty as the controller works it out when it is made
        // (`UpdateHipInaccuracy`), no combo unit on until told.
        hands.update_hip_inaccuracy(false);
        hands
    }

    /// The hip wander's scale as the firearm's controller works it out
    /// (`UpdateHipInaccuracy`: as the weapon is drawn or changed, and as a
    /// tactical device's request is taken): by the weapon's total
    /// ergonomics, and lower with a combo unit on (`tactical::hip_penalty`;
    /// tactical-devices spec, "What it does to the weapon").
    pub fn update_hip_inaccuracy(&mut self, combo_on: bool) {
        self.combo_on = combo_on;
        self.breath.set_hip_penalty(crate::tactical::hip_penalty(self.settings.total_ergonomics(), combo_on));
    }

    /// The hip penalty in force (`Breath.HipPenalty`).
    pub fn hip_penalty(&self) -> f32 {
        self.breath.hip_penalty()
    }

    /// Whether a combo unit was on when the hip penalty was last worked out.
    pub fn combo_on(&self) -> bool {
        self.combo_on
    }

    /// The weapon's sights, where it has any (`crate::sights`).
    pub fn sights(&self) -> Option<&crate::sights::Sights> {
        self.settings.sights.as_ref()
    }

    /// The bones the rig is read at for the sight aimed by, where the
    /// weapon has sights (`crate::sights::Sights::nodes`).
    pub fn sight_nodes(&self) -> Option<crate::rig::SightNodes> {
        self.sights().map(crate::sights::Sights::nodes)
    }

    /// The main camera's field of view with the sights up: 35 degrees
    /// through an optic, the setting less 15 over anything else.
    fn aimed_fov(&self) -> f32 {
        let fov = self.settings.field_of_view;
        // A controller with no weapon's data leaves it as set
        // (`OnAimOrPoseChanged` narrows it only with a firearm's: the
        // rangefinder's, rangefinder spec).
        if self.fov_kept_aimed {
            return fov;
        }
        self.sights().map_or(fov - AIM_FOV_DROP, |sights| sights.aimed_field_of_view(fov))
    }

    /// The field of view kept as set while aiming (`keep_fov_aimed`).
    pub fn keep_fov_aimed(&mut self) {
        self.fov_kept_aimed = true;
    }

    /// The key to the next sight (`ChangeAimScope`; optics spec, "The
    /// sights a weapon has"): refused by the caller in blind fire and with
    /// the left hand busy, and here, mounted, past any sight turned aside.
    /// Whether the sight changed: the eye then goes to the new one at the
    /// aiming rate, its line is measured again, and the field of view
    /// starts toward the new sight's (`CycleScopes`, `OnAimOrPoseChanged`).
    pub fn change_aim_scope(&mut self, mounted: bool) -> bool {
        let Some(sights) = self.settings.sights.as_mut() else { return false };
        let changed = sights.change_aim_scope(mounted);
        if changed {
            self.sight_changed();
        }
        changed
    }

    /// A mount found: a canted sight in use gives way to the first straight
    /// one (`OnFoundPoint`), and with none, whether the sight in use is still
    /// canted (the mount's start then puts the sights down:
    /// `ToggleFirearmAimByBodyAction`).
    pub fn straighten_for_mount(&mut self) -> bool {
        let Some(sights) = self.settings.sights.as_mut() else { return false };
        if sights.first_straight() {
            self.sight_changed();
        }
        self.settings.sights.as_ref().is_some_and(|sights| sights.scope_rotation() != 0.0)
    }

    /// The magnification key (`ChangeAimScopeMagnification`; optics spec,
    /// "Zoom and modes"): the scope aimed by goes to its next mode and
    /// every variable scope snaps to the far end of its range. Whether it
    /// was done; the host then gives the hands' animator its `ModSwitch`.
    pub fn change_magnification(&mut self) -> bool {
        let Some(sights) = self.settings.sights.as_mut() else { return false };
        let changed = sights.change_magnification();
        if changed {
            self.sight_changed();
        }
        changed
    }

    /// One frame of the zoom's wheel (`ScopeZoomHandler`): `input` is the
    /// wheel's turn on the frame with the zoom's keys held, times the
    /// options' `OpticSensitivity` (wheel up positive, which narrows), `dt`
    /// the frame's time.
    pub fn zoom_frame(&mut self, input: f32, dt: f32) {
        let aiming = self.aiming;
        if let Some(sights) = self.settings.sights.as_mut() {
            sights.zoom_frame(input, dt, aiming);
        }
    }

    /// What the mouse's aiming sensitivity is multiplied by while aiming:
    /// the sight's figure (`crate::sights::Sights::aim_sensitivity`), or the
    /// weapon's own where it has no sights.
    pub fn aim_sensitivity(&self, weapon: f32) -> f32 {
        self.sights().map_or(weapon, |sights| sights.aim_sensitivity(weapon))
    }

    /// Another sight or mode is in force (`OnAimOrPoseChanged`): the field
    /// of view starts again toward the figure for the sights as they are,
    /// unless the mouse has the head; the line of the iron sights is taken
    /// again with the sights up.
    fn sight_changed(&mut self) {
        if self.aiming {
            self.retake_line = true;
        }
        let in_range = self.sights().is_some_and(crate::sights::Sights::in_range);
        if in_range && !self.head_control {
            let target = if self.aiming { self.aimed_fov() } else { self.settings.field_of_view };
            self.set_fov(target, 1.0);
        }
    }

    /// A blast's shake of the view (`GrenadeShake_CO`), of the strength the
    /// blast gives (`trp_sim::blast`), its first kick as the next frame
    /// begins.
    pub fn grenade_shake(&mut self, strength: f32) {
        if strength > 0.0 {
            self.force_react.shakes.push(Shake::new(strength));
        }
    }

    /// A hit's push on the view (`ForceEffector.AddForce`, from
    /// `EffectsController.OnPlayerDamaged`; health spec, "A hit and
    /// health's other screen effects"): a turn drawn in the noise's box
    /// (`_eulersNoiseXYMaxValue`, `_eulersNoiseZMaxValue`), made of a length
    /// drawn between `1 − _deltaRandom` and 1, times the aim punch and the
    /// strength (held to 1), into the camera's rotation spring by `camera`
    /// and the hands' by `hands`, within the springs' limits.
    pub fn add_force(&mut self, strength: f32, hands: f32, camera: f32) {
        let d = &self.settings.data.force_react;
        let force = &mut self.force_react;
        let mut turn = [force.rng.range(-d.noise_xy, d.noise_xy), force.rng.range(-d.noise_xy, d.noise_xy), force.rng.range(-d.noise_z, d.noise_z)];
        let length = (turn[0] * turn[0] + turn[1] * turn[1] + turn[2] * turn[2]).sqrt();
        if length > 1e-5 {
            let drawn = force.rng.range(1.0 - d.delta_random, 1.0);
            turn = turn.map(|v| v / (length / drawn));
        }
        let size = self.settings.aim_punch * strength.clamp(0.0, 1.0);
        for (k, value) in turn.into_iter().enumerate() {
            if camera > 0.0 {
                self.camera_rotation.add(k, value * camera * size);
            }
            if hands > 0.0 {
                self.hands_rotation.add(k, value * hands * size);
            }
        }
    }

    /// The shakes' kicks of this frame, as their coroutines come back.
    fn shake_frame(&mut self, dt: f32) {
        let kicks: Vec<f32> = self.force_react.shakes.iter_mut().filter_map(|shake| shake.frame(dt)).collect();
        self.force_react.shakes.retain(|shake| shake.left > 0.0);
        for power in kicks {
            self.grenade_kick(power);
        }
    }

    /// A kick (`GrenadeShake`): a random direction's force eased into the
    /// springs over the next steps, and a random turn's acceleration on the
    /// camera's rotation, by the power and a draw.
    fn grenade_kick(&mut self, power: f32) {
        let force = &mut self.force_react;
        // `Random.onUnitSphere`, then the draw added to the power.
        let z = force.rng.range(-1.0, 1.0);
        let around = force.rng.range(0.0, std::f32::consts::TAU);
        let across = (1.0 - z * z).max(0.0).sqrt();
        let direction = [across * around.cos(), across * around.sin(), z];
        let hit = power + force.rng.range(0.0, 0.15);
        let d = &self.settings.data.force_react;
        let size = (d.intensity * hit * 0.0005).clamp(0.0003, 0.0006);
        for k in 0..3 {
            force.force[k] += direction[k] * size;
        }
        force.steps_left = 6;
        let turn = [force.rng.range(-d.noise_xy, d.noise_xy), force.rng.range(-d.noise_xy, d.noise_xy), force.rng.range(-d.noise_z, d.noise_z)];
        for (k, value) in turn.into_iter().enumerate() {
            self.camera_rotation.add(k, value * hit);
        }
    }

    /// One physics step of the force reaction (`ForceEffector.Process`):
    /// while steps are left, the sum eased toward the force by a sixth and
    /// pushed through the values, unlimited, into their springs.
    fn force_reaction(&mut self) {
        let force = &mut self.force_react;
        if force.steps_left < 0 {
            return;
        }
        force.steps_left -= 1;
        force.sum = slerp3(force.sum, force.force, 1.0 / 6.0);
        let now = force.sum;
        force.force = [0.0; 3];
        for val in self.settings.data.force_react.vals.clone() {
            let value = now[(val.source as usize).min(2)] * val.intensity;
            // Its targets as the game hands them over: the camera's
            // rotation, the camera's position, the hands' rotation.
            let spring = match val.target {
                0 => &mut self.camera_rotation,
                1 => &mut self.camera_position,
                _ => &mut self.hands_rotation,
            };
            spring.add_limitless(val.component as usize, value);
        }
    }

    /// Advances by `dt` seconds, running as many physics steps as fit.
    pub fn advance(&mut self, dt: f32, input: &HandsInput) {
        // A shot fired with no frame drawn since is felt before the step.
        self.feel_shots();
        if !self.force_react.shakes.is_empty() {
            self.shake_frame(dt);
        }
        self.accumulator += dt;
        let step = self.settings.fixed_timestep;
        // Half a step of slack keeps a caller at the same rate from drifting.
        while self.accumulator >= step * 0.5 {
            self.accumulator -= step;
            self.fixed_step(step, input);
        }
    }

    /// Turns the weapon's rig to the view: once a frame, after the frame's
    /// physics steps (`advance`). The game works the steps first and
    /// turns the rig after, so what follows the rig's turning (the sway,
    /// and the axes the movement is taken in) sees the view a frame late
    /// (weapon-handling spec, "Motion reaction"). A host that never calls
    /// this has the steps see the view they are given.
    pub fn turn_rig(&mut self, yaw: f32, pitch: f32) {
        self.rig_view = Some([yaw, pitch]);
    }

    /// The head's free look as the frame's `HeadLook::frame` left it
    /// (`Player.Look`, `ProceduralWeaponAnimation.SetHeadRotation`): its
    /// turn, whether the mouse has the head, and whether the sights are
    /// up (the weapon's own word, before this frame's steps). With the
    /// sights up, the mouse taking the head puts the field of view back
    /// to the hip's, and its letting go puts the aimed one on again.
    pub fn set_head_rotation(&mut self, head: [f32; 3], control: bool, aiming: bool) {
        (self.head, self.head_control) = (head, control);
        if aiming && self.head_control_seen != control {
            self.head_control_seen = control;
            let fov = self.settings.field_of_view;
            self.set_fov(if control { fov } else { self.aimed_fov() }, 1.0);
        }
    }

    /// Whose eyes the character is seen with
    /// (`ProceduralWeaponAnimation.PointOfView`). For anyone's but its own
    /// the physics steps run the turn-away, the aim pose, the weapon's two
    /// springs and the recoil and nothing else, the weapon is turned by
    /// its rotation spring alone, and the camera is left where it stands
    /// (`ThirdPersonStrategy`).
    pub fn set_point_of_view(&mut self, point_of_view: PointOfView) {
        self.point_of_view = point_of_view;
        let first_person = !self.outside();
        if let Some(sights) = self.settings.sights.as_mut() {
            sights.set_first_person(first_person);
        }
    }

    /// Which of its systems run (`crate::Systems`): with the weapon's
    /// motion off nothing is fed to the springs (the turning, the steps,
    /// the breath, the walk), the lean does not roll the weapon and it
    /// does not turn away from what it would stick into; with the recoil
    /// off a shot is felt by neither the weapon nor the camera.
    /// What the character's health does to the weapon in its hands
    /// (`ProceduralWeaponAnimation.PhysicalConditionUpdated`): damaged arms
    /// soften the sway spring, half the way for one and all of it for both
    /// (the carried weight's share, which the game adds, is not in here);
    /// a damaged leg without a painkiller plays the lame bob. The tremor
    /// and a broken arm's shake are the breathing's (`Wind`).
    pub fn set_health(&mut self, conditions: crate::health::Conditions) {
        let s = &self.settings;
        let factor = conditions.sway_damage();
        let (frequency, damping) = (lerp(s.sway_frequency, s.sway_damaged[0], factor), lerp(s.sway_damping, s.sway_damaged[1], factor));
        self.sway.set(frequency, damping);
        self.lame = conditions.lame();
    }

    pub fn set_systems(&mut self, systems: &crate::Systems) {
        (self.motion, self.recoil_on) = (systems.weapon_motion, systems.recoil);
    }

    pub fn point_of_view(&self) -> PointOfView {
        self.point_of_view
    }

    fn outside(&self) -> bool {
        self.point_of_view == PointOfView::ThirdPerson
    }

    /// One physics step of a character seen from outside
    /// (`ThirdPersonStrategy.ProcessEffectors`).
    fn fixed_step_outside(&mut self, dt: f32, input: &HandsInput) {
        // With the turn-away masked off, as with a first-person pose
        // held, its depth is put to nothing every step.
        if input.crawling || !self.poses.is_empty() || self.mask & effector::DRAW_DOWN == 0 || !self.motion {
            self.turn_away.depth = 0.0;
        }
        self.turn_away(dt);
        if self.mask & effector::AIMING != 0 {
            self.push_aim_pose();
        }
        self.hands_position.step();
        self.hands_rotation.step();
        self.recoil.step(dt);
    }

    /// What the sights coming up, held and going down push into the
    /// weapon's two springs for a character seen from outside
    /// (`CustomEffector.Process`): each curve of the prefab's pose read at
    /// how far up the sights are, each of its figures an acceleration with
    /// the spring's own input scale taken out again.
    fn push_aim_pose(&mut self) {
        let blend = self.aim_pose.value(self.time);
        if blend.abs() < f32::EPSILON {
            return;
        }
        let up = self.aim_pose.target == 1.0;
        let pose: Vec<WalkEntry> = self.settings.data.aim_pose.clone();
        for (curve, entry) in pose.iter().enumerate() {
            let value = entry.curve.evaluate(blend);
            if value.abs() < 1e-6 {
                continue;
            }
            for (index, val) in entry.vals.iter().enumerate() {
                // The game gives one figure a second factor: the first
                // curve's second, by its own size, one way as the sights
                // come up and the other as they go down.
                let own = match (curve, index) {
                    (0, 1) if up => val.intensity,
                    (0, 1) => -val.intensity,
                    _ => 1.0,
                };
                let spring = self.spring(val.target);
                let scale = spring.input_intensity;
                spring.add(val.component as usize, value * val.intensity / scale * own);
            }
        }
    }

    fn fixed_step(&mut self, dt: f32, input: &HandsInput) {
        self.time += dt;
        self.steps_since_frame += 1;
        self.set_aiming(input.aiming);
        // The aim pose's own clock runs whoever is looking: up with the
        // sights, unless a first-person pose is held.
        let posed = input.aiming && self.poses.is_empty();
        self.aim_pose.set_target(if posed { 1.0 } else { 0.0 }, self.time);
        // Aiming quiets everything: the springs take less of what is fed
        // to them, and the recoil's rotation kicks less.
        let intensity = self.intensity_by_aiming();
        self.hands_position.input_intensity = intensity * intensity;
        self.hands_rotation.input_intensity = intensity * intensity;
        self.recoil.set_aiming(input.aiming);
        // Only the lowest crouch counts as crouched here.
        self.pose = if input.prone {
            0
        } else if input.pose_level < 0.11 {
            1
        } else {
            2
        };
        self.view = [input.yaw, input.pitch];
        self.recoil.set_pose(self.pose);

        // What the view's turning, the movement, the breath and the steps
        // push into the springs is for the character's own eyes only, and
        // only while the weapon's motion runs.
        if !self.outside() && self.motion {
            self.track(dt, input);
            self.react(dt);
            if self.mask & effector::FORCE_REACTION != 0 {
                self.force_reaction();
            }
            // Mounted, the breath does not run at all but with the hands at
            // their floor or an arm broken, and then at the mount's share
            // (`BreathEffector.Process`).
            let breath_by = match self.mounted {
                Some(mount) => {
                    let floor = self.settings.data.breath.lower_limit;
                    if input.wind.stamina.max(floor) > floor && !input.wind.fracture {
                        None
                    } else {
                        Some(self.settings.mount.motion(&self.settings.mount_settings, mount.side, self.bipod_used).breath)
                    }
                }
                None => Some(1.0),
            };
            if let Some(by) = breath_by {
                let breath = self.breath.step(dt, self.time, self.breath_intensity() * by, self.aiming, self.settings.head_bobbing, input.wind);
                for (target, component, value) in breath.pushes {
                    self.spring(target).add(component as usize, value);
                }
                self.recoil.hand_rotation.offset = breath.muzzle_offset;
            }
            // The mount's way has no walk's bob (`MountingStrategy`).
            if !self.mount_strategy {
                self.walk(dt, input);
            }
        }

        // A lean reaches the weapon in full when standing still, and by
        // speed and stance otherwise. The game only works this out while a
        // lean is asked for; between leans the last figure stands.
        self.tilt_blend.set_target(if input.idle { 1.0 } else { 0.0 }, self.time);
        if input.tilt_target != 0.0 {
            let blend = self.tilt_blend.value(self.time);
            self.possible_tilt = if blend >= 1.0 {
                blend
            } else {
                blend.max(tilt_by_speed_and_stance(input.speed, input.smoothed_pose_level))
            };
        }
        self.lean_rest = if self.motion { input.tilt * self.possible_tilt } else { 0.0 };
        self.set_rests();

        if self.outside() {
            self.fixed_step_outside(dt, input);
            return;
        }

        // Crawling, and with a first-person pose held, the turn-away is
        // switched off: the depth is put to nothing every step. So it is
        // with the weapon's motion off.
        // (The mount's way leaves the depth alone but for the motion off.)
        if (!self.mount_strategy && (input.crawling || !self.poses.is_empty())) || !self.motion {
            self.turn_away.depth = 0.0;
        }
        self.turn_away(dt);

        self.hands_position.step();
        self.hands_rotation.step();
        self.sway.step(dt);
        self.recoil.step(dt);
        self.camera_rotation.step();
        self.camera_position.step();
    }

    /// One physics step of the turn-away: the value follows the depth, and
    /// the pose for the weapon, the camera's shift and the elbows' shifts
    /// are read off it.
    fn turn_away(&mut self, dt: f32) {
        // A character seen from outside has no squeeze, the prefab's
        // second pair of figures for every component, and no camera.
        let outside = self.outside();
        let scale = if outside { 1.0 } else { self.fov_scale() };
        let d = &self.settings.data.turn_away;
        let away = &mut self.turn_away;
        (away.position, away.rotation, away.elbows, away.camera) = ([0.0; 3], [0.0; 3], [[0.0; 3]; 2], 0.0);
        let mut wanted = away.depth / d.threshold;
        // Mounted it goes no further than 1 (`TurnAway.InMountState`).
        if self.mounted.is_some() {
            wanted = wanted.clamp(0.0, 1.0);
        }
        let rate = if wanted > away.value { d.in_speed } else { d.out_speed };
        away.value = lerp(away.value, wanted, rate * dt);
        if away.value <= 0.001 {
            return;
        }
        let (value, past) = (away.value, away.value > 1.0);
        away.far = lerp(away.far, if past { 1.0 } else { 0.0 }, d.blend_speed * dt);
        let far = away.far;
        let figures = |val: &TurnAwayVal| if outside { val.outside } else { [val.near, val.far] };
        let read = |vals: &[TurnAwayVal], out: &mut [f32; 3]| {
            for val in vals {
                let [near, further] = figures(val);
                out[usize::from(val.component.min(2))] = val.curve.evaluate(value) * lerp(near, further, far);
            }
        };
        let pose = if self.pose == 0 {
            &d.prone
        } else if self.settings.pistol {
            &d.pistol
        } else {
            &d.rifle
        };
        read(&pose.position, &mut away.position);
        // Drawn back no further than the pose's own reach, less what the
        // position spring already holds the weapon back by. (The engine's
        // clamp, which gives the upper bound when that lies below the lower.)
        let most = pose.position.get(1).map_or(0.0, |val| figures(val)[0]) - away.held_back;
        let back = away.position[1] * scale;
        away.position[1] = if back < 0.0 { 0.0 } else { back.min(most) };
        read(&pose.rotation, &mut away.rotation);
        read(&d.elbows[0].position, &mut away.elbows[0]);
        read(&d.elbows[1].position, &mut away.elbows[1]);
        if outside {
            return;
        }
        let target =
            if past { d.camera_shift_at_most } else { d.camera_shift.evaluate(value) * inverse_lerp(0.65, 1.0, scale) };
        away.camera_shift = lerp(away.camera_shift, target, dt * d.blend_speed);
        away.camera = away.camera_shift;
    }

    /// The line the game casts to feel for what the weapon would stick
    /// into, in the frame the rig hangs in: from where the position spring
    /// has the moved bone, along the barrel as that bone's parent points
    /// (`weapon_root`), for the weapon's length. A host turns it into the
    /// world's axes and measures (`Movement::weapon_overlap`), then calls
    /// `set_overlap`.
    pub fn weapon_line(&self) -> ([f32; 3], [f32; 3]) {
        let (at, turn) = self.weapon_root;
        let reach = quat_rotate(turn, self.hands_position.value());
        let from: [f32; 3] = std::array::from_fn(|i| at[i] + reach[i]);
        // The barrel runs along the bone's -y.
        let along = quat_rotate(turn, [0.0, -self.settings.weapon_length, 0.0]);
        (from, std::array::from_fn(|i| from[i] + along[i]))
    }

    /// How deep the weapon would stick into something, in metres, as
    /// measured at the end of a frame. The next physics steps turn the
    /// weapon away by it.
    pub fn set_overlap(&mut self, depth: f32) {
        // From this depth on the effector is told how far back the
        // spring already holds the weapon.
        if depth >= OVERLAP_THAT_COUNTS {
            self.turn_away.held_back = self.hands_position.value()[1];
        }
        self.turn_away.depth = depth;
    }

    /// The turn-away's value: the depth over the prefab's threshold, as
    /// far as it has been followed. `Firearm::overlap` waits for it to
    /// fall before the sights come back.
    pub fn turn_away_value(&self) -> f32 {
        self.turn_away.value
    }

    /// What the turn-away adds to the moved bone: metres in its parent,
    /// and degrees about the point a little way along the barrel.
    pub fn turn_away_offsets(&self) -> ([f32; 3], [f32; 3]) {
        (self.turn_away.position, self.turn_away.rotation)
    }

    /// What the turn-away adds to the weapon's goal for an elbow, in the
    /// frame the rig hangs in. The game turns its shift by the view's yaw
    /// and 0.65 of its pitch; in the rig's frame, which has all of both,
    /// that leaves a turn back by the rest of the pitch.
    pub fn elbow_shift(&self, right: bool) -> [f32; 3] {
        let shift = self.turn_away.elbows[usize::from(right)];
        quat_rotate(quat_axis(0, -(1.0 - ELBOW_SHIFT_PITCH) * self.view[1]), shift)
    }

    /// The stock has been folded or unfolded (`FoldStockOperation.
    /// SwitchToIdle`, `ProceduralWeaponAnimation.UpdateWeaponVariables`):
    /// the recoil goes by the build's strengths as it now stands, and
    /// whether the weapon is held closer is worked out afresh. That is
    /// only ever true with the sights down at this moment, and it is not
    /// looked at again when they next come up or go down (as read).
    pub fn set_folded(&mut self, folded: bool, aiming: bool) {
        self.folded = folded;
        let (rotation, position, closer) = match self.settings.folded.filter(|_| folded) {
            Some(hold) => (hold.rotation_strength, hold.position_strength, hold.held_closer),
            None => (self.settings.recoil.rotation_strength, self.settings.recoil.position_strength, self.settings.held_closer),
        };
        self.recoil.set_strengths(rotation, position);
        self.closer = closer && !aiming;
        self.set_rests();
    }

    /// Whether the weapon is held closer to the body just now.
    pub fn held_closer(&self) -> bool {
        self.closer
    }

    /// The iron sights' line and the muzzle as the weapon's rig has them
    /// on this frame (`rig::SightNodes`): a host reads them off the rig
    /// as its animator poses it and tells them here every frame, before
    /// `blend_poses`. The line is taken when the sights next come up
    /// (the game measures it then: `ProceduralWeaponAnimation.method_14`),
    /// and is none again once they are down.
    pub fn note_sights(&mut self, line: Option<SightLine>, muzzle: Option<[f32; 3]>) {
        (self.sight_noted, self.muzzle) = (line, muzzle);
        if std::mem::take(&mut self.retake_line) && self.aiming {
            self.sight = self.line_in_force(line);
        }
    }

    /// The line the weapon is levelled to and the eye put on: a launcher's
    /// own (`method_13`: from its collimator to the zeroing point, kept by
    /// `crate::sights`), else the iron sights' as the rig gives it.
    fn line_in_force(&self, iron: Option<SightLine>) -> SightLine {
        match self.sights().and_then(crate::sights::Sights::launcher_line) {
            Some(line) => line,
            None => iron.unwrap_or_default(),
        }
    }

    /// The zeroing keys (`OpticCalibrationSwitchUp`, `Down`; optics spec,
    /// "Zeroing"): while aiming, the current sight's scope one distance up
    /// or down. Whether it moved.
    pub fn step_zeroing(&mut self, up: bool) -> bool {
        let aiming = self.aiming;
        self.settings.sights.as_mut().is_some_and(|sights| sights.step_zeroing(up, aiming))
    }

    /// Whether a shot and the recoil take the field of view's squeeze
    /// (`ShotNeedsFovAdjustments`): not once a scope's optic has been
    /// turned to its zeroing point (optics spec, "Zeroing").
    pub fn shot_squeeze(&self) -> bool {
        self.sights().is_none_or(crate::sights::Sights::shot_squeeze)
    }

    /// What the sights have told the battle screen since the last call.
    pub fn take_sight_hud(&mut self) -> Vec<crate::sights::SightHud> {
        self.settings.sights.as_mut().map(crate::sights::Sights::take_hud).unwrap_or_default()
    }

    /// The sights' line in force, and how far the weapon is turned for
    /// it now, in degrees.
    pub fn sight_line(&self) -> (SightLine, f32) {
        (self.sight, self.sight_angle)
    }

    /// Where the weapon's two springs and the camera's rotation spring
    /// rest: the lean, blind fire's turn of the camera, and whatever
    /// first-person poses are blended in.
    fn set_rests(&mut self) {
        let (lean, turn, camera, blind) = (self.lean_rest, self.pose_rotation, self.pose_camera, self.blind_camera);
        self.hands_position.zero = self.pose_position;
        // A weapon held closer rests that much nearer along its own y.
        if self.closer {
            self.hands_position.zero[1] = HELD_CLOSER;
        }
        self.hands_rotation.zero = [turn[0], lean + turn[1], turn[2]];
        self.camera_rotation.zero = [camera[0] + blind[0], camera[1] + blind[1], lean + camera[2] + blind[2]];
        // The mount's camera rests on the lean alone (`MountedWeaponCamera`).
        if self.mount_strategy {
            self.camera_rotation.zero = [0.0, 0.0, lean];
        }
    }

    /// What a mount tells the weapon's motion (`ProceduralWeaponAnimation.
    /// SetMountingData`): mounted with this pivot and side, or not. The
    /// recoil takes its mounted figures; the breath, the sway and the
    /// tactical blend go by it.
    pub fn set_mounting(&mut self, mount: Option<crate::mounting::HandsMount>) {
        self.mounted = mount;
        let figures = mount.map(|m| self.settings.mount.motion(&self.settings.mount_settings, m.side, self.bipod_used));
        self.recoil.hand_rotation.set_mounted(figures.map(|f| (f.recoil, f.return_speed, self.settings.mount.return_speed)));
    }

    /// The mount's way of moving in force or not (`SetMountingStratеgy`, and
    /// `SetStrategy` back at the mount's end).
    pub fn set_mount_strategy(&mut self, on: bool) {
        self.mount_strategy = on;
        self.set_rests();
    }

    /// The bipod out or in (`IsBipodUsed`): the recoil, the breath and the
    /// sway read it while mounted.
    pub fn set_bipod(&mut self, out: bool) {
        self.bipod_used = out;
        if let Some(mount) = self.mounted {
            self.set_mounting(Some(mount));
        }
    }

    /// Whether mounted (`IsMountedState`), and the mount's way in force.
    pub fn mounting(&self) -> (Option<crate::mounting::HandsMount>, bool) {
        (self.mounted, self.mount_strategy)
    }

    /// Where the rig's origin is from the camera's object, in the camera's
    /// axes (a mount moves the rig and the mounted camera follows it): a
    /// host that places the rig sets it each frame, before `frame`.
    pub fn set_rig_shift(&mut self, shift: [f32; 3]) {
        self.rig_shift = shift;
    }

    /// Blind fire's side as the weapon's operation passed it on
    /// (`Firearm::set_blind_fire`; `ProceduralWeaponAnimation.
    /// StartBlindFire`): the blender's target, from this frame's moment
    /// (taken up by the frame's camera pass, `blind_fire_frame`). It reads
    /// where it was on this frame and moves from the next.
    pub fn start_blind_fire(&mut self, side: i32) {
        self.blind_asked = Some(side as f32);
    }

    /// Blind fire's blender on a new frame of `frame_dt` seconds: the
    /// frames' clock moves on to it, a target asked during it is taken from
    /// its moment, and the value is read.
    fn blind_fire_frame(&mut self, frame_dt: f32) {
        self.blind_clock += frame_dt;
        if let Some(target) = self.blind_asked.take() {
            self.blind.set_target(target, self.blind_clock);
        }
        self.blind_value = self.blind.value(self.blind_clock);
    }

    /// Blind fire's blender as this frame's camera pass read it: 0 at
    /// rest, toward 1 over the top and -1 to the right; and its target.
    pub fn blind_fire(&self) -> (f32, f32) {
        (self.blind_value, self.blind.target)
    }

    /// Blind fire's part of the camera's pass (`ZeroAdjustments`,
    /// `CalculateCameraPosition`): the blender read off the frame's
    /// moment; while it is off 0 the strength by the view's pitch (whole
    /// under 45 degrees either way, down to nothing at 90); and the camera
    /// rotation spring's rest given the prefab's turn over the top or to
    /// the right, times half the value's size, times the strength.
    fn blind_fire_rest(&mut self, frame_dt: f32) {
        self.blind_fire_frame(frame_dt);
        let value = self.blind_value;
        if value != 0.0 {
            let pitch = self.rig_view.map_or(self.view[1], |view| view[1]).abs();
            self.blind_strength = if pitch < 45.0 { 1.0 } else { (90.0 - pitch) / 45.0 };
        }
        let d = &self.settings.data.blind_fire;
        let turn = if value > 0.0 { d.camera[0] } else { d.camera[1] };
        let share = (value / 2.0).abs() * self.blind_strength;
        self.blind_camera = turn.map(|degrees| degrees * share);
        self.set_rests();
    }

    /// In blind fire the weapon is first felt for obstacles along a line
    /// from its end point (`FirearmController.WeaponOverlapping`): the
    /// prefab's point over the top or to the right by the value's sign,
    /// times the strength, with the held-closer shift, all times 1.9, in
    /// the frame the moved bone hangs in, down the barrel for the weapon's
    /// length. None while the blender is at 0.
    pub fn blind_fire_line(&self) -> Option<([f32; 3], [f32; 3])> {
        let value = self.blind_value;
        if value == 0.0 {
            return None;
        }
        let d = &self.settings.data.blind_fire;
        let end = if value > 0.0 { d.end[0] } else { d.end[1] };
        let held = if self.closer { HELD_CLOSER } else { 0.0 };
        let point = [end[0] * self.blind_strength * 1.9, (end[1] * self.blind_strength + held) * 1.9, end[2] * self.blind_strength * 1.9];
        let (at, turn) = self.weapon_root;
        let reach = quat_rotate(turn, point);
        let from: [f32; 3] = std::array::from_fn(|i| at[i] + reach[i]);
        let along = quat_rotate(turn, [0.0, -self.settings.weapon_length, 0.0]);
        Some((from, std::array::from_fn(|i| from[i] + along[i])))
    }

    /// How deep the weapon sticks into something as the game measures it at
    /// the end of a frame, `measure` casting a line in the rig's frame
    /// (`Movement::weapon_overlap`): in blind fire the line from its end
    /// point first, whose depth stands if it is clear (under 0.02 m); else,
    /// and out of blind fire, the plain line (`weapon_line`). A host hands
    /// the depth to `set_overlap` and `Firearm::overlap`.
    pub fn measure_overlap(&self, measure: impl FnMut(([f32; 3], [f32; 3])) -> f32) -> f32 {
        self.measure_overlap_shoulder(false, measure).0
    }

    /// `measure_overlap`, the plain line taken from where the left shoulder
    /// puts the weapon while it is wanted (`WeaponOverlapping`: the hands'
    /// place in the moved bone's own axes, not its parent's; the same way
    /// along the weapon root's). With it, whether the plain line was measured
    /// (not in blind fire with its own line clear), which the left shoulder's
    /// drop and return go by.
    pub fn measure_overlap_shoulder(&self, left_stance: bool, mut measure: impl FnMut(([f32; 3], [f32; 3])) -> f32) -> (f32, bool) {
        if let Some(line) = self.blind_fire_line() {
            let depth = measure(line);
            if depth < OVERLAP_THAT_COUNTS {
                return (depth, false);
            }
        }
        (measure(if left_stance { self.shoulder_line(0.0) } else { self.weapon_line() }), true)
    }

    /// The left shoulder's line (`shift` 0), or the one its way back is
    /// tried along (`shift` 0.2: as far again to the left of the weapon
    /// root and as much longer; `WeaponOverlapping`), in the rig's frame.
    pub fn shoulder_line(&self, shift: f32) -> ([f32; 3], [f32; 3]) {
        let (at, turn) = self.weapon_root;
        let (local_at, local_turn) = self.shoulder_root;
        let placed = quat_rotate(turn, local_at);
        let root: [f32; 3] = std::array::from_fn(|i| at[i] + placed[i]);
        let reach = quat_rotate(quat_mul(turn, local_turn), self.hands_position.value());
        let left = quat_rotate(turn, [-shift, 0.0, 0.0]);
        let from: [f32; 3] = std::array::from_fn(|i| root[i] + reach[i] + left[i]);
        let along = quat_rotate(turn, [0.0, -(self.settings.weapon_length + shift), 0.0]);
        (from, std::array::from_fn(|i| from[i] + along[i]))
    }

    /// The body's animator has entered a state: its first-person pose, if
    /// it has one, comes in, and every other pose held goes out.
    pub fn assign_pose(&mut self, pose: Option<&FirstPersonPose>) {
        // Told to the character's own eyes only
        // (`MovementContext.AssignAnimatorPose`).
        if self.outside() {
            return;
        }
        let mut held = pose.is_none();
        for (other, _, coming_in) in &mut self.poses {
            *coming_in = pose.is_some_and(|pose| pose.name == other.name);
            held |= *coming_in;
        }
        if let (false, Some(pose)) = (held, pose) {
            self.poses.push((pose.clone(), 0.0, true));
        }
    }

    /// Moves the held poses on by one rendered frame of `frame_dt` seconds
    /// and puts what they come to into the springs' rests; eases the iron
    /// sights' line and a canted sight's turn. A host calls it once a
    /// frame, after the frame's physics steps and before it reads the
    /// weapon's transform or the camera. On a frame with no physics step
    /// the time is the player's own (`frame_on_clock`'s `arms_dt`: the game
    /// gives its weapon motion `Player._armsTime`, the frame's time read in
    /// whole milliseconds), which a host with one clock takes for its
    /// frame's.
    pub fn blend_poses(&mut self, frame_dt: f32) {
        // A weapon with both points of an iron sights' line is turned
        // toward levelling it while the sights are up, and back when they
        // are down, by a share of what is left each frame.
        let launcher = self.sights().is_some_and(crate::sights::Sights::launcher);
        if self.sight_noted.is_some() || launcher || self.sight_angle != 0.0 {
            let steps = self.steps_since_frame;
            let dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { frame_dt };
            let goal = if self.aiming { self.sight.angle } else { 0.0 };
            self.sight_angle = lerp(self.sight_angle, goal, (dt * SIGHT_LINE_RATE).clamp(0.0, 1.0));
        }
        // A canted sight: the weapon turned about its barrel (its bone's
        // y) by the sight's own turn while aiming, and back while not, by
        // a share `CameraSmoothTime * aiming speed * dt` of what is left,
        // in the same late update and with the same time as the line
        // above (`ApplyComplexRotation`, `UpdateCurrentScopeRotation`).
        let degrees = self.sights().map_or(0.0, crate::sights::Sights::scope_rotation);
        if degrees != 0.0 || self.scope_rotation != [0.0, 0.0, 0.0, 1.0] {
            let steps = self.steps_since_frame;
            let dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { frame_dt };
            let goal = if self.aiming { quat_axis(1, degrees) } else { [0.0, 0.0, 0.0, 1.0] };
            self.scope_rotation = quat_lerp(self.scope_rotation, goal, self.settings.data.camera_smooth_time * self.aiming_speed * dt);
        }
        if self.poses.is_empty() && self.pose_position == [0.0; 3] && self.pose_rotation == [0.0; 3] {
            return;
        }
        // The time the camera gets: the physics time on a frame that had
        // a physics step, the frame's own otherwise.
        let steps = self.steps_since_frame;
        let mut dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { frame_dt };
        let (mut position, mut rotation, mut camera) = ([0.0f32; 3], [0.0f32; 3], [0.0f32; 3]);
        // From the newest back, the time scaled once more for each: the
        // game's own way of counting.
        let mut index = self.poses.len();
        while index > 0 {
            index -= 1;
            dt *= self.weapon_flip_speed;
            let (pose, clock, coming_in) = &mut self.poses[index];
            *clock += if *coming_in { dt } else { -dt };
            if *clock < 0.0 {
                self.poses.remove(index);
                continue;
            }
            let share = pose.blend.evaluate(*clock);
            for k in 0..3 {
                position[k] += pose.position[k] * share;
                rotation[k] += pose.rotation[k] * share;
            }
            *clock = clock.min(pose.blend.keys.last().map_or(0.0, |key| key.time));
        }
        // The camera takes its share when its target is worked out, from
        // the clocks as that left them.
        for (pose, clock, _) in &self.poses {
            let share = pose.blend.evaluate(*clock);
            for k in 0..3 {
                camera[k] += pose.camera_rotation[k] * share;
            }
        }
        (self.pose_position, self.pose_rotation, self.pose_camera) = (position, rotation, camera);
        self.set_rests();
    }

    /// 1 from the hip, less while aiming.
    pub fn intensity_by_aiming(&self) -> f32 {
        if self.aiming {
            self.settings.aim_intensity
        } else {
            1.0
        }
    }

    /// Scale of breathing: the stance's factor times the aiming factor.
    fn breath_intensity(&self) -> f32 {
        self.settings.intensity_by_pose[self.pose] * self.intensity_by_aiming()
    }

    /// Puts the breath where a recording has it, for a host that replays
    /// one. The breath is a loop of a few seconds; hands made fresh start
    /// at its beginning, a recorded character is somewhere in it, and the
    /// weapon rises and falls some millimetres with it (and the line that
    /// feels for obstacles starts from the weapon). `lifted` is how far up
    /// the position spring held the weapon on the recording's first physics
    /// steps, one figure a step, with the character still and the sights
    /// down; `pose` is the stance as the weapon's motion counts it (0
    /// lying, 1 crouched, 2 standing). The breath and the three springs it
    /// feeds are left where a character breathing since long before would
    /// have them at the point of the loop that fits those figures best.
    pub fn start_breath(&mut self, pose: usize, lifted: &[f32]) {
        let step = self.settings.fixed_timestep;
        let round = (self.breath.period() / step).ceil() as usize;
        if lifted.is_empty() || round == 0 {
            return;
        }
        let intensity = self.settings.intensity_by_pose[pose.min(2)];
        let head_bobbing = self.settings.head_bobbing;
        let mut breath = self.breath.clone();
        let mut springs = [self.camera_rotation.clone(), self.hands_position.clone(), self.hands_rotation.clone()];
        springs[1].input_intensity = 1.0;
        springs[2].input_intensity = 1.0;
        let mut time = self.time;
        // Once round to settle, then once round with each point kept.
        let mut kept = Vec::with_capacity(round + lifted.len());
        for index in 0..2 * round + lifted.len() {
            time += step;
            for (target, component, value) in breath.step(step, time, intensity, false, head_bobbing, Wind::default()).pushes {
                springs[usize::from(target.min(2))].add(component as usize, value);
            }
            for spring in &mut springs {
                spring.step();
            }
            if index >= round {
                kept.push((breath.clone(), springs.clone(), time));
            }
        }
        let miss = |from: usize| -> f32 {
            lifted.iter().zip(&kept[from..]).map(|(wanted, (_, springs, _))| (springs[1].value()[2] - wanted).powi(2)).sum()
        };
        let Some(best) = (0..round).min_by(|a, b| miss(*a).total_cmp(&miss(*b))) else { return };
        let (breath, [camera_rotation, hands_position, hands_rotation], time) = kept.swap_remove(best);
        (self.breath, self.time) = (breath, time);
        (self.camera_rotation, self.hands_position, self.hands_rotation) = (camera_rotation, hands_position, hands_rotation);
    }

    /// A shot leaves the barrel. `strength` is 1 plus the cartridge's
    /// recoil percentage.
    ///
    /// It is felt at the end of the frame's late update (`frame_on_clock`,
    /// `frame_outside`), after the weapon and the camera are placed: the
    /// game's firearm controller sets it off in its own late update, which
    /// the player runs after the procedural motion's
    /// (`Player.LateUpdate`: `ProceduralWeaponAnimation.LateTransformations`,
    /// then `HandsController.ManualLateUpdate`, which calls `Shoot`). So the
    /// weapon's own curves start again, and the camera's rate drops, from
    /// the frame after the shot's; the springs feel it on their next step
    /// either way. With no frame between, the next step feels it first.
    pub fn shoot(&mut self, strength: f32) {
        self.shots_pending.push(strength);
    }

    /// The shots fired and not yet felt (`ProceduralWeaponAnimation.Shoot`).
    fn feel_shots(&mut self) {
        for strength in std::mem::take(&mut self.shots_pending) {
            // A character whose mask has no shot feels none (a bot's), and
            // none is felt with the recoil switched off.
            if self.mask & effector::SHOOTING == 0 || !self.recoil_on {
                continue;
            }
            // Mounted, the camera's snap is the mount's share of itself.
            let snap = match self.mounted {
                Some(mount) => self.settings.mount.motion(&self.settings.mount_settings, mount.side, self.bipod_used).camera_snap,
                None => 1.0,
            };
            self.camera_smooth.jump(self.settings.camera_snap * snap, CAMERA_SNAP_DELAY, self.time);
            self.recoil.shoot(strength);
            // The breath is told of a shot for the character's own eyes only,
            // and none from the hip on a mount.
            if !self.outside() {
                self.breath.on_shot(self.time, self.breath_intensity(), self.aiming || self.mounted.is_some());
            }
        }
    }

    /// Whether the weapon is in a run of automatic fire.
    pub fn set_auto_fire(&mut self, on: bool) {
        self.recoil.set_auto_fire(on);
    }

    /// Rate per second at which the camera closes on the sight line while
    /// aiming. It drops after each shot and recovers.
    pub fn camera_aim_rate(&self) -> f32 {
        self.aiming_speed * self.camera_smooth.value(self.time)
    }

    /// The sights went up or down as this frame began, before its physics
    /// steps: the weapon's motion takes it at once, as the game's does (its
    /// controller sets `ProceduralWeaponAnimation.IsAiming` as it changes
    /// its aim, in the frame's update), so the late update of a frame with
    /// no physics step already goes by it. Left to the next step's input,
    /// the camera's ease and a canted sight's turn began a frame late on
    /// such a frame (`courses4`'s M4A1, its sights up at 0.5 s). The field
    /// of view starts toward its new figure here too, and takes its first
    /// turn in this frame's `frame`.
    pub fn aim_changed(&mut self, aiming: bool) {
        if aiming != self.aiming {
            self.set_aiming(aiming);
        }
    }

    /// The sights have gone up or down: the field of view starts toward
    /// its new figure, and what goes by the sights does so from here on.
    fn set_aiming(&mut self, aiming: bool) {
        // The sights' line is measured as they come up, and dropped as
        // they go down.
        // (`IsAiming`, as it changes: a launcher's line measured, the shot's
        // squeeze as the sight has it, the zeroing's calibration asked for.
        // This is called on every physics step, changed or not.)
        if aiming != self.aiming {
            if let Some(sights) = self.settings.sights.as_mut() {
                sights.aim_changed(aiming);
            }
            // A heavy weapon's sway drawn afresh and whole as the sights
            // come up (`IsAiming`: three of Unity's `Random.Range`, x, y, z,
            // in the prefab's box, times the strength and the stance's
            // intensity).
            if aiming && self.aim_sway_strength > 0.0 {
                let a = self.settings.data.aim_sway;
                let k = self.aim_sway_strength * self.settings.intensity_by_pose[self.pose];
                let random = &mut self.aim_sway_random;
                let drawn: [f32; 3] = std::array::from_fn(|i| random.range(a.min[i], a.max[i]) * k);
                self.aim_sway_direction = self.aim_sway_override.unwrap_or(drawn);
                self.aim_sway_since = 0.0;
            }
        }
        self.sight = if aiming { self.line_in_force(self.sight_noted) } else { SightLine::default() };
        // While the mouse has the head the sights going up or down leave
        // the field of view alone (`OnAimOrPoseChanged`).
        if aiming != self.aiming && !self.head_control {
            let fov = self.settings.field_of_view;
            self.set_fov(if aiming { self.aimed_fov() } else { fov }, 1.0);
        }
        self.aiming = aiming;
    }

    /// The sights went up or down after the frame's `frame`: the change an
    /// obstacle makes at the end of a frame (`Firearm::overlap`). The game
    /// tells its weapon motion at that moment, and the field of view's
    /// easing takes its first turn there and then, which is the one that
    /// changes nothing yet; the next frame's is the first to show. Left to
    /// the next physics step, the camera and the field of view start a
    /// frame or two late. `frame_dt` is the length of the frame just
    /// drawn. (A change the aim key makes comes before the frame's steps
    /// and needs no call: the step's input carries it.)
    pub fn aim_changed_after_frame(&mut self, aiming: bool, frame_dt: f32) {
        if aiming != self.aiming {
            self.set_aiming(aiming);
            self.field_of_view(frame_dt);
        }
    }

    /// The player has changed the game's field-of-view setting (50 to 75)
    /// while playing: the view eases to the new figure, less the aim's
    /// drop with the sights up, and the weapon's squeeze goes by it from
    /// here on. PROVISIONAL: the game takes a changed setting when its
    /// settings screen is saved, and how its camera gets there was not
    /// read; the ease is the one a change of aim uses.
    /// The skills' share in the handling (Aim Drills' aiming speed, the
    /// weapon class's ergonomics), as the game takes them when the weapon
    /// is drawn or changed (`UpdateWeaponVariables`): what goes by the
    /// weapon's weight and ergonomics is worked out again.
    pub fn set_skills(&mut self, aim_speed_bonus: f32, ergonomics_buff: f32, recoil_suppression: f32) {
        if (aim_speed_bonus, ergonomics_buff) != (self.settings.aim_speed_bonus, self.settings.ergonomics_buff) {
            (self.settings.aim_speed_bonus, self.settings.ergonomics_buff) = (aim_speed_bonus, ergonomics_buff);
            (self.sway_strength, self.displacement_strength, self.aiming_speed) = self.settings.by_weight();
            self.aim_sway_strength = self.settings.aim_sway_strength();
            self.keep_manual();
            self.update_hip_inaccuracy(self.combo_on);
        }
        // The recoil's strengths with the class skill's suppression
        // (`RecalculateRecoilParamsOnChangeWeapon`), as built and folded,
        // and those of the stock as it stands put on the recoil.
        if recoil_suppression != self.settings.recoil_suppression {
            let Some(force) = self.settings.recoil_force else { return };
            self.settings.recoil_suppression = recoil_suppression;
            (self.settings.recoil.rotation_strength, self.settings.recoil.position_strength) = force.strengths(force.parts, recoil_suppression);
            if let (Some(hold), Some(parts)) = (self.settings.folded.as_mut(), force.folded_parts) {
                (hold.rotation_strength, hold.position_strength) = force.strengths(parts, recoil_suppression);
            }
            let (rotation, position) = match self.settings.folded.filter(|_| self.folded) {
                Some(hold) => (hold.rotation_strength, hold.position_strength),
                None => (self.settings.recoil.rotation_strength, self.settings.recoil.position_strength),
            };
            self.recoil.set_strengths(rotation, position);
        }
    }

    /// The aiming speed and the aim sway's strength set by hand
    /// (`ManualSetVariables`; the overweight and the ergonomic weight it
    /// sets as well are not kept here: `PROVISIONAL`, the sway's strength
    /// stays the rig's).
    pub fn manual_set_variables(&mut self, aiming_speed: f32, aim_sway_strength: f32) {
        self.manual = Some((aiming_speed, aim_sway_strength));
        self.keep_manual();
    }

    fn keep_manual(&mut self) {
        if let Some((speed, sway)) = self.manual {
            (self.aiming_speed, self.aim_sway_strength) = (speed, sway);
        }
    }

    /// What a mount adds to the ergonomics' share (`TotalErgonomicsFunc`'s
    /// mounted bonus), worked in as the skills' are (`RecalculateErgonomic`).
    pub fn set_mount_ergonomics(&mut self, bonus: f32) {
        if bonus != self.settings.mount_ergonomics {
            self.settings.mount_ergonomics = bonus;
            (self.sway_strength, self.displacement_strength, self.aiming_speed) = self.settings.by_weight();
            self.aim_sway_strength = self.settings.aim_sway_strength();
            self.keep_manual();
            self.update_hip_inaccuracy(self.combo_on);
        }
    }

    pub fn set_field_of_view(&mut self, degrees: f32) {
        self.settings.field_of_view = degrees;
        self.set_fov(if self.aiming { self.aimed_fov() } else { degrees }, 1.0);
        // The weapon's squeeze goes by the setting, and the sights' eye by
        // the squeeze (`SetCompensationScale`, `SetFovParams`).
        let scale = self.fov_scale();
        if let Some(sights) = self.settings.sights.as_mut() {
            sights.set_scale(scale);
        }
    }

    /// Starts the field of view moving to `target`. `time` is the game's
    /// figure for the move; larger is slower.
    fn set_fov(&mut self, target: f32, time: f32) {
        self.fov_target = target;
        self.fov_time = time;
        self.fov_blend = 1.0;
    }

    /// The camera's vertical field of view in degrees, advanced by one
    /// rendered frame. The game's easing is tied to the frame rate.
    fn field_of_view(&mut self, frame_dt: f32) -> f32 {
        if self.fov_blend > 0.0 {
            self.fov = lerp(self.fov_target, self.fov, self.fov_blend);
            self.fov_blend -= frame_dt / self.fov_time;
        } else {
            self.fov = self.fov_target;
        }
        self.fov
    }

    /// The camera's field of view as it last stood, degrees.
    pub fn field_of_view_now(&self) -> f32 {
        self.fov
    }

    /// Advances the field of view's easing by one rendered frame alone, for
    /// a host that does not move the camera (`frame` does it with the
    /// rest): what a comparison of the sights against a recording drives.
    pub fn step_field_of_view(&mut self, frame_dt: f32) -> f32 {
        self.field_of_view(frame_dt)
    }

    /// How much the weapon is squeezed along the view at a wide field of
    /// view, so that it fills the same part of the screen.
    pub fn fov_scale(&self) -> f32 {
        lerp(1.0, 0.65, inverse_lerp(50.0, 75.0, self.settings.field_of_view))
    }

    /// Follows how the view turns and how the player moves.
    fn track(&mut self, dt: f32, input: &HandsInput) {
        let d = &self.settings.data;
        let mut motion = input.motion;
        let cap = input.velocity[1].abs();
        motion[1] = motion[1].clamp(-cap, cap);
        // Both trackings go by the weapon's rig, which still points where
        // the frame before turned it.
        let [yaw, pitch] = self.rig_view.unwrap_or([input.yaw, input.pitch]);
        let local = to_view(motion, yaw, pitch);
        // The game smooths these two with the rendered frame's time, though
        // it does so once per physics step: above the physics rate the
        // smoothing runs slow. The turning terms below use the step's time.
        let frame = if input.frame_dt > 0.0 { input.frame_dt } else { dt };
        self.position_velocity = smooth_damp(
            self.position_velocity,
            local,
            &mut self.position_velocity_rate,
            d.movement.vel_smooth,
            f32::INFINITY,
            frame,
        );
        let change: [f32; 3] = std::array::from_fn(|i| self.position_velocity[i] - self.last_position_velocity[i]);
        self.position_acceleration = smooth_damp(
            self.position_acceleration,
            change,
            &mut self.position_acceleration_rate,
            d.movement.acc_smooth,
            f32::INFINITY,
            frame,
        );
        self.last_position_velocity = self.position_velocity;

        // How far the view's forward moved since the last step, seen from
        // the view: x is the turn to the right, y the tilt upward.
        let forward = from_view([0.0, 0.0, 1.0], yaw, pitch);
        let previous = to_view(self.last_forward.unwrap_or(forward), yaw, pitch);
        self.last_forward = Some(forward);
        let gyro = [
            -previous[0].atan2(previous[2]).to_degrees(),
            previous[2].atan2(previous[1]).to_degrees() - 90.0,
        ];
        let rate = [gyro[0] / dt, gyro[1] / dt];

        let clamp = d.rotation_input_clamp;
        self.rotation_velocity = self.rotation_velocity.map(|v| v.clamp(-clamp, clamp));
        for i in 0..2 {
            let change = self.rotation_velocity[i] - self.last_rotation_velocity[i];
            self.rotation_acceleration_sum[i] += (change - self.rotation_acceleration_sum[i]) / 3.0;
            self.rotation_velocity_sum[i] += (rate[i] - self.rotation_velocity_sum[i]) * 0.2;
        }
        self.last_rotation_velocity = self.rotation_velocity;
        self.rotation_velocity = smooth_damp(
            self.rotation_velocity,
            self.rotation_velocity_sum,
            &mut self.rotation_velocity_rate,
            d.mouse.vel_smooth,
            f32::INFINITY,
            dt,
        );
        self.rotation_acceleration = smooth_damp(
            self.rotation_acceleration,
            self.rotation_acceleration_sum,
            &mut self.rotation_acceleration_rate,
            d.mouse.acc_smooth,
            f32::INFINITY,
            dt,
        );
    }

    fn source(&self, index: u8) -> f32 {
        match index {
            0 => self.rotation_velocity[0],
            1 => self.rotation_acceleration[0],
            2 => self.rotation_velocity[1],
            3 => self.rotation_acceleration[1],
            4..=6 => self.position_velocity[index as usize - 4],
            7..=9 => self.position_acceleration[index as usize - 7],
            _ => 0.0,
        }
    }

    fn spring(&mut self, target: u8) -> &mut Spring {
        match target {
            0 => &mut self.camera_rotation,
            1 => &mut self.hands_position,
            _ => &mut self.hands_rotation,
        }
    }

    /// Feeds turning into the sway spring and movement into the others.
    fn react(&mut self, dt: f32) {
        let movement: Vec<EffectorVal> = self.settings.data.movement.vals.clone();
        // Mounted forward on the bipod, the motion's reactions are a share
        // of themselves (`MotionEffector`: `MountingSwayFactorModifier`).
        let on_bipod = self.mounted.is_some_and(|m| !m.side) && self.bipod_used;
        let by = if on_bipod { self.settings.mount_settings.sway_with_bipod } else { 1.0 };
        let scale = dt * self.settings.data.motion_intensity * by;
        for val in movement {
            let force = self.source(val.source) * val.intensity * scale;
            self.spring(val.target).add(val.component as usize, force);
        }
        let s = self.sway_strength * self.intensity_by_aiming() * by;
        let factors = [s, if self.aiming { s * 0.3 } else { s }, s];
        let mouse: Vec<EffectorVal> = self.settings.data.mouse.vals.clone();
        for val in mouse {
            let shaped = self.settings.data.mouse.magnitude.evaluate(self.source(val.source));
            let component = val.component as usize;
            self.sway.kick(component, shaped * factors[component] * val.intensity * dt);
        }
    }

    /// The bob of each step, and the footsteps that set its tempo.
    fn walk(&mut self, dt: f32, input: &HandsInput) {
        // Each footstep retunes the bob to the time since the one before.
        if let Some(gap) = input.footstep.filter(|gap| *gap < MAX_STEP_INTERVAL) {
            let (shortest, longest) =
                if input.sprinting { (0.3, 0.8) } else { (0.7 - input.speed / 2.0, MAX_STEP_INTERVAL) };
            self.step_frequency = 0.5 / gap.clamp(shortest, longest.max(shortest));
        }

        if !input.walking {
            if self.walking {
                // Stopping rewinds every curve of the bob played to its
                // start.
                let clocks = if self.lame { &mut self.walk_clocks_lame } else { &mut self.walk_clocks };
                clocks.iter_mut().for_each(|c| *c = 0.0);
            }
            self.walking = false;
            return;
        }
        self.walking = true;
        let range = self.settings.data.walk_intensity_range;
        let intensity = lerp(range[0], range[1], input.speed);
        let count = if self.lame { self.walk_clocks_lame.len() } else { self.walk_clocks.len() };
        for index in 0..count {
            let (clock, entry) = if self.lame {
                (&mut self.walk_clocks_lame[index], &self.settings.data.walk_lame[index])
            } else {
                (&mut self.walk_clocks[index], &self.settings.data.walk_normal[index])
            };
            *clock += dt * self.step_frequency;
            let value = looped(&entry.curve, *clock) * intensity;
            if value.abs() < 1e-6 {
                continue;
            }
            let vals: Vec<EffectorVal> = entry.vals.clone();
            let head_bobbing = self.settings.head_bobbing;
            for val in vals {
                // What the bob does to the camera is scaled by the player's setting.
                let setting = if val.target == 0 { head_bobbing } else { 1.0 };
                self.spring(val.target).add(val.component as usize, value * val.intensity * setting);
            }
        }
    }

    /// Local position and rotation (`xyzw`) of the bone the weapon hangs
    /// from, relative to its rest of no offset and no rotation.
    pub fn weapon_transform(&self) -> ([f32; 3], [f32; 4]) {
        let d = &self.settings.data;
        let outside = self.outside();
        let sway = self.sway.position;
        let hands = self.hands_rotation.value();
        let displacement = self.displacement_strength * if self.aiming { d.aiming_displacement_str } else { 1.0 };
        // Seen from outside the weapon is turned by its rotation spring
        // alone, about the point the recoil turns it about, and the
        // recoil does not turn it (`ApplySimpleRotation`).
        let r = if outside {
            hands
        } else {
            [hands[0] + sway[0] + displacement * sway[0], hands[1] + sway[1], hands[2] + sway[2] + displacement * sway[2]]
        };
        // About y first, then about x and z together.
        let mut rotation = quat_mul(quat_mul(quat_axis(0, r[0]), quat_axis(2, r[2])), quat_axis(1, r[1]));
        let center = if let Some(mount) = self.mounted {
            // Mounted, every turn goes about the mount's pivot.
            mount.pivot
        } else if outside {
            self.settings.recoil_pivot
        } else if self.closer {
            self.settings.rotation_center_no_stock
        } else {
            self.settings.rotation_center
        };
        let swung = quat_rotate(rotation, center);
        let offset = self.hands_position.value();
        let kick = self.recoil.weapon_position();
        let mut position: [f32; 3] = std::array::from_fn(|i| offset[i] + kick[i] + center[i] - swung[i]);

        // Recoil turns the weapon about its own pivot, after everything else.
        let mut recoil = if outside { [0.0; 3] } else { self.recoil.weapon_rotation() };
        if recoil.iter().any(|a| a.abs() > f32::EPSILON) {
            let scale = self.fov_scale();
            if scale < 1.0 && self.shot_squeeze() {
                // A weapon squeezed along the view would otherwise show its
                // climb squeezed too; not once a scope's optic has been
                // turned to its zeroing point (`ShotNeedsFovAdjustments`).
                let adjust = |degrees: f32| (degrees.to_radians().tan() * scale).atan().to_degrees();
                recoil[0] = adjust(recoil[0]);
                recoil[2] = adjust(recoil[2]);
            }
            let turn = quat_euler(recoil);
            let pivot = self.mounted.map_or(self.settings.recoil_pivot, |m| m.pivot);
            let turned = quat_rotate(turn, pivot);
            let shift = quat_rotate(rotation, std::array::from_fn(|i| pivot[i] - turned[i]));
            position = std::array::from_fn(|i| position[i] + shift[i]);
            rotation = quat_mul(rotation, turn);
        }

        // The sights' line levelled: the weapon turned about its muzzle,
        // in its own axes (the game's form turns it by the same angles
        // taken about the world's axes, which differs in the second order).
        if let (true, Some(muzzle)) = (self.sight_angle.abs() > SIGHT_LINE_LEAST, self.muzzle) {
            let turn = quat_axis(0, self.sight_angle);
            let turned = quat_rotate(turn, muzzle);
            let shift = quat_rotate(rotation, std::array::from_fn(|i| muzzle[i] - turned[i]));
            position = std::array::from_fn(|i| position[i] + shift[i]);
            rotation = quat_mul(rotation, turn);
        }

        // A canted sight's turn about the barrel, after the line's, in the
        // bone's own axes about its own place (`SetPositionAndRotation(...,
        // _temporaryRotation * _scopeRotation)`): its place is kept.
        if self.scope_rotation != [0.0, 0.0, 0.0, 1.0] {
            rotation = quat_mul(rotation, self.scope_rotation);
        }

        // Last, the turn-away from an obstacle: drawn back, then turned
        // about a point a little way along the barrel. The game takes the
        // angles as those of a vector turned from the bone's axes into the
        // world's, and turns the bone by them about the world's axes.
        let away = &self.turn_away;
        if away.position != [0.0; 3] || away.rotation != [0.0; 3] {
            position = std::array::from_fn(|i| position[i] + away.position[i]);
            // Mounted, it draws back and does not turn (`AvoidObstacles`).
            if self.mounted.is_some() {
                return (position, rotation);
            }
            let to_world = quat_mul(quat_mul(quat_axis(1, self.view[0]), quat_axis(0, self.view[1])), self.weapon_root.1);
            let back = [-to_world[0], -to_world[1], -to_world[2], to_world[3]];
            let angles = quat_rotate(quat_mul(to_world, rotation), away.rotation);
            let turn = quat_mul(quat_mul(back, quat_euler(angles)), to_world);
            let pivot = quat_rotate(rotation, [0.0, -TURN_AWAY_PIVOT, 0.0]);
            let swung = quat_rotate(turn, pivot);
            position = std::array::from_fn(|i| position[i] + pivot[i] - swung[i]);
            rotation = quat_mul(turn, rotation);
        }
        (position, rotation)
    }

    /// The weapon's moved bone in its parent, once the body has had its say.
    ///
    /// `parent` is where that parent sits in the frame the weapon's rig
    /// hangs in. Where the body's animation claims the weapon, the bone is
    /// drawn toward the body's own weapon bone, keeping the motion worked
    /// out here as an offset from it.
    ///
    /// The game does this before it squeezes the rig for a wide field of
    /// view, so the carried weapon is squeezed like the rest and nothing
    /// is allowed for here. (A recorded sprint shows the swing along the
    /// view at true size in the bone's own figures.)
    ///
    /// Where the body's animation also takes the right hand off the
    /// weapon (`held`), the weapon is then put in that hand: `marker` is
    /// the weapon's right-hand marker in the moved bone's own axes, as
    /// the weapon's animation has it now.
    /// `weapon_transform_carried` with the left shoulder's part
    /// (`PlayerBones.ShiftWeaponRoot`, in first person; left-shoulder spec,
    /// "The weapon"), which keeps its own state from frame to frame. At the
    /// stance's full curve, out of a sprint, the moved bone goes to the
    /// body's own weapon bone with the procedural motion's own place and turn
    /// (`Offset`, `DeltaRotation`) taken onto it: its place at once, or
    /// eased after it (`leftStanceRotationAndPositionChangeSpeed`, 10 a
    /// second) aimed and not walked forward since the stance came in; its
    /// turn eased from the last frame's toward the body's (10 a second, 20
    /// as the animation goes back), turned only about its own y. Aimed, its
    /// place is held at the ribcage's own place in its parent plus a fixed
    /// way (`_offsterDirrectionForLeftStanceInAim` (0.13, -0.305, 0.023)
    /// times 0.2) eased by the frame's time, the recoil's push added, and its
    /// turn eased from the last frame's toward the procedural motion's own.
    /// Below the full curve the weapon goes to the body's bone by the curve
    /// or the body's own authority, whichever is more, turned only about y
    /// while the curve is up; at no curve, as `weapon_transform_carried`.
    pub fn weapon_transform_shifted(
        &mut self,
        parent: ([f32; 3], [f32; 4]),
        third: Carried,
        held: Option<Held>,
        marker: ([f32; 3], [f32; 4]),
        left: LeftShoulder,
    ) -> Shifted {
        self.left_shoulder_curve = left.curve;
        let mut untouched = false;
        let (offset, turn) = self.weapon_transform();
        let (parent_position, parent_rotation) = parent;
        let back = [-parent_rotation[0], -parent_rotation[1], -parent_rotation[2], parent_rotation[3]];
        let hanging = quat_rotate(parent_rotation, offset);
        let mut position: [f32; 3] = std::array::from_fn(|i| parent_position[i] + hanging[i]);
        let mut rotation = quat_mul(parent_rotation, turn);
        let swung = quat_rotate(third.rotation, offset);
        let wanted: [f32; 3] = std::array::from_fn(|i| third.position[i] + swung[i]);
        let own_euler = quat_to_euler(turn);
        // The turn about the moved bone's own y kept, the others as `euler`'s.
        let yaw_only = |rotation: [f32; 4], euler: [f32; 3]| {
            let local = quat_to_euler(quat_mul(back, rotation));
            quat_mul(parent_rotation, quat_euler([euler[0], local[1], euler[2]]))
        };
        let s = &mut self.left_shoulder;
        if left.curve >= 1.0 && !left.sprinting {
            if left.aiming && left.forward {
                s.was_moving_in_aim = true;
            }
            s.target = if left.aiming && !s.was_moving_in_aim { std::array::from_fn(|i| lerp(s.target[i], wanted[i], left.dt * LEFT_SHOULDER_SPEED)) } else { wanted };
            position = s.target;
            if left.aiming {
                let goal: [f32; 3] = std::array::from_fn(|i| left.ribcage_local[i] + LEFT_SHOULDER_AIM_WAY[i] * LEFT_SHOULDER_AIM_REACH);
                s.target_in_aim = std::array::from_fn(|i| lerp(s.target_in_aim[i], goal[i], left.dt));
                let kick = self.recoil.weapon_position();
                let local: [f32; 3] = std::array::from_fn(|i| s.target_in_aim[i] + kick[i]);
                let placed = quat_rotate(parent_rotation, local);
                position = std::array::from_fn(|i| parent_position[i] + placed[i]);
                rotation = yaw_only(quat_lerp(s.last_rotation, rotation, left.dt * LEFT_SHOULDER_SPEED), own_euler);
            } else {
                let goal = quat_mul(third.rotation, turn);
                s.last_rotation = if s.got_full_last_frame {
                    goal
                } else {
                    quat_lerp(s.last_rotation, goal, left.dt * if left.anim_value { LEFT_SHOULDER_SPEED } else { LEFT_SHOULDER_BACK_SPEED })
                };
                rotation = yaw_only(s.last_rotation, own_euler);
            }
            s.last_rotation = rotation;
            s.got_full_last_frame = false;
        } else {
            s.got_full_last_frame = true;
            s.was_moving_in_aim = false;
            let weight = if left.curve > 0.0 { left.curve.max(third.authority) } else { third.authority };
            if weight > 0.0 {
                position = std::array::from_fn(|i| lerp(position[i], wanted[i], weight));
                rotation = quat_lerp(rotation, quat_mul(third.rotation, turn), weight);
            }
            if left.curve > 0.0 && !left.sprinting {
                rotation = yaw_only(rotation, own_euler);
            }
            s.target = position;
            s.target_in_aim = quat_rotate(back, std::array::from_fn(|i| position[i] - parent_position[i]));
            s.last_rotation = rotation;
            untouched = weight <= 0.0 && left.curve <= 0.0;
        }
        // Nothing moved it: the procedural motion's own, to the bit.
        let carried = if untouched { (offset, turn) } else { (quat_rotate(back, std::array::from_fn(|i| position[i] - parent_position[i])), quat_mul(back, rotation)) };
        let held_too = if held.is_some_and(|h| h.weight < 1.0) { self.held_onto(parent, position, rotation, held, marker) } else { carried };
        self.shoulder_root = held_too;
        Shifted { carried, held: held_too }
    }

    /// Whether the weapon's motion is aiming (`ProceduralWeaponAnimation.IsAiming`).
    pub fn is_aiming(&self) -> bool {
        self.aiming
    }

    /// The left shoulder's curve as the weapon was last placed by it.
    pub fn left_shoulder_curve(&self) -> f32 {
        self.left_shoulder_curve
    }

    /// The right palm's hold of the weapon (`PlayerBones.Kinematics`), on a
    /// place and turn of the moved bone in the rig's frame, and the result
    /// in the parent's.
    fn held_onto(&self, parent: ([f32; 3], [f32; 4]), mut position: [f32; 3], mut rotation: [f32; 4], held: Option<Held>, marker: ([f32; 3], [f32; 4])) -> ([f32; 3], [f32; 4]) {
        let (parent_position, parent_rotation) = parent;
        if let Some(held) = held.filter(|h| h.weight < 1.0) {
            let (marker_position, marker_rotation) = marker;
            let reach = quat_rotate(rotation, marker_position);
            let marked: [f32; 3] = std::array::from_fn(|i| position[i] + reach[i]);
            let marked_facing = quat_mul(rotation, marker_rotation);
            let palm: [f32; 3] = std::array::from_fn(|i| lerp(held.position[i], marked[i], held.weight));
            let palm_facing = quat_slerp(held.rotation, marked_facing, held.weight);
            let at: [f32; 3] = std::array::from_fn(|i| lerp(palm[i], marked[i], held.weight));
            let facing = quat_lerp(palm_facing, marked_facing, held.weight);
            let unmarked = [-marker_rotation[0], -marker_rotation[1], -marker_rotation[2], marker_rotation[3]];
            rotation = quat_mul(facing, unmarked);
            let reach = quat_rotate(rotation, marker_position);
            position = std::array::from_fn(|i| at[i] - reach[i]);
        }
        let back = [-parent_rotation[0], -parent_rotation[1], -parent_rotation[2], parent_rotation[3]];
        let local = quat_rotate(back, std::array::from_fn(|i| position[i] - parent_position[i]));
        (local, quat_mul(back, rotation))
    }

    pub fn weapon_transform_carried(
        &self,
        parent: ([f32; 3], [f32; 4]),
        carried: Option<Carried>,
        held: Option<Held>,
        marker: ([f32; 3], [f32; 4]),
    ) -> ([f32; 3], [f32; 4]) {
        let (offset, turn) = self.weapon_transform();
        let carried = carried.filter(|c| c.authority > 0.0);
        let held = held.filter(|h| h.weight < 1.0);
        if carried.is_none() && held.is_none() {
            return (offset, turn);
        }
        let (parent_position, parent_rotation) = parent;

        // Where the bone is now, in the rig's frame.
        let hanging = quat_rotate(parent_rotation, offset);
        let mut position: [f32; 3] = std::array::from_fn(|i| parent_position[i] + hanging[i]);
        let mut rotation = quat_mul(parent_rotation, turn);
        if let Some(carried) = carried {
            // Toward where the body would have it.
            let swung = quat_rotate(carried.rotation, offset);
            let wanted: [f32; 3] = std::array::from_fn(|i| carried.position[i] + swung[i]);
            position = std::array::from_fn(|i| lerp(position[i], wanted[i], carried.authority));
            rotation = quat_lerp(rotation, quat_mul(carried.rotation, turn), carried.authority);
        }
        if let Some(held) = held {
            // The marker goes toward the palm, and the bone to wherever
            // that puts it. By then the arm's own solver, on the same
            // weight, has drawn the palm toward the marker.
            let (marker_position, marker_rotation) = marker;
            let reach = quat_rotate(rotation, marker_position);
            let marked: [f32; 3] = std::array::from_fn(|i| position[i] + reach[i]);
            let marked_facing = quat_mul(rotation, marker_rotation);
            let palm: [f32; 3] = std::array::from_fn(|i| lerp(held.position[i], marked[i], held.weight));
            let palm_facing = quat_slerp(held.rotation, marked_facing, held.weight);
            let at: [f32; 3] = std::array::from_fn(|i| lerp(palm[i], marked[i], held.weight));
            let facing = quat_lerp(palm_facing, marked_facing, held.weight);
            let unmarked = [-marker_rotation[0], -marker_rotation[1], -marker_rotation[2], marker_rotation[3]];
            rotation = quat_mul(facing, unmarked);
            let reach = quat_rotate(rotation, marker_position);
            position = std::array::from_fn(|i| at[i] - reach[i]);
        }

        let back = [-parent_rotation[0], -parent_rotation[1], -parent_rotation[2], parent_rotation[3]];
        let local = quat_rotate(back, std::array::from_fn(|i| position[i] - parent_position[i]));
        (local, quat_mul(back, rotation))
    }

    /// A rendered frame has gone by for a character seen from outside,
    /// whose camera is left where it stands
    /// (`ThirdPersonStrategy.ApplyCameraTransformations`): a host calls
    /// this where it would have called `frame`, and the physics steps a
    /// frame counts are counted afresh from here.
    pub fn frame_outside(&mut self, frame_dt: f32) {
        self.steps_since_frame = 0;
        // Blind fire's blender moves on whoever is looking (the camera's
        // part of it is the character's own eyes').
        self.blind_fire_frame(frame_dt);
        self.feel_shots();
    }

    /// The camera for one rendered frame of `frame_dt` seconds.
    ///
    /// `animated` is the local rotation of the weapon's animated camera
    /// bone; `aim` says where the sight is and is only used while aiming.
    ///
    /// The game gives its per-frame camera code the physics time that
    /// elapsed when a physics step ran in the frame, and the frame's own
    /// time otherwise. Above the physics rate the camera therefore moves
    /// faster than the clock; that is kept.
    pub fn frame(&mut self, frame_dt: f32, animated: [f32; 4], aim: Option<AimPoint>) -> CameraPose {
        self.frame_on_clock(frame_dt, frame_dt, animated, aim)
    }

    /// `frame` for a host that keeps the player's own clock apart from the
    /// engine's (the movement's `step_on_clock`): the camera's place and
    /// turn ease by `arms_dt`, the player's frame time read in whole
    /// milliseconds, on a frame with no physics step (the game's weapon
    /// motion is given the player's arms update's time there,
    /// `Player._armsTime`; the physics steps' time otherwise), and the field
    /// of view by `frame_dt`, the engine's (`CameraManager.SetFovCoroutine`
    /// goes by `Time.deltaTime`).
    pub fn frame_on_clock(&mut self, frame_dt: f32, arms_dt: f32, animated: [f32; 4], aim: Option<AimPoint>) -> CameraPose {
        let steps = std::mem::take(&mut self.steps_since_frame);
        let dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { arms_dt };
        self.aim_sway_frame(frame_dt);
        self.blind_fire_rest(frame_dt);
        // The aiming weight (`UpdateAimWeight`), every frame.
        let rate = if self.aiming { self.camera_aim_rate() } else { self.settings.data.camera_smooth_out };
        self.aim_weight = lerp(self.aim_weight, if self.aiming { 1.0 } else { 0.0 }, rate * dt);
        // With the left shoulder's curve up, and blind fire's blender at 0,
        // the camera's pass runs only late in the frame, by the frame's own
        // time (`ProcessEffectors` leaves it to `LateTransformations`).
        let late_only = self.left_shoulder_curve > 0.0 && self.blind_value == 0.0;
        let camera_dt = if late_only { frame_dt } else { dt };
        let mut pose = CameraPose {
            position: if self.mount_strategy { self.mounted_camera_position(camera_dt, aim) } else { self.camera_position(camera_dt, aim) },
            rotation: self.camera_rotation(camera_dt, animated),
            field_of_view: self.field_of_view(frame_dt),
        };
        // While blind fire's blender is off 0 the camera's pass runs again
        // late in the frame, by the frame's own time
        // (`ProceduralWeaponAnimation.LateTransformations`): its place eases
        // a second time, the position spring added again, and its turn's
        // follow of the recoil is blended again. PROVISIONAL: read from code;
        // in the recordings the camera rests then and the pass is unseen.
        if self.blind_value != 0.0 {
            pose.position = self.camera_position(frame_dt, aim);
            pose.rotation = self.camera_rotation(frame_dt, animated);
        }
        // Then the player's late update turns the sights to the zeroing
        // point (`StartFovCoroutine`: `PreCalibrate`, `Calibrate`, in first
        // person), which a launcher's line follows.
        self.calibrate();
        // And the weapon's controller sets off the frame's shots.
        self.feel_shots();
        pose
    }

    /// A heavy weapon's sway (weapon-handling spec, "Aim sway"): each frame
    /// the sights are up, the sway spring's velocity given the direction
    /// times what is left of it, from whole as they came up to nothing at
    /// `SwayFalloff` over the aiming speed a second, by the engine's clock
    /// (`_aimSwayBlender`, `SwaySpring.ApplyVelocity`, in the camera's
    /// frame: a kick a frame, not a physics step).
    fn aim_sway_frame(&mut self, frame_dt: f32) {
        if self.aim_sway_strength > 0.0 && self.aiming {
            let speed = self.settings.data.aim_sway.falloff / self.aiming_speed;
            let left = (1.0 - self.aim_sway_since * speed).clamp(0.0, 1.0);
            if left > 0.0 {
                for i in 0..3 {
                    self.sway.kick(i, self.aim_sway_direction[i] * left);
                }
            }
        }
        self.aim_sway_since += frame_dt;
    }

    /// The aim sway's strength, and the direction it was last drawn in.
    pub fn aim_sway(&self) -> (f32, [f32; 3]) {
        (self.aim_sway_strength, self.aim_sway_direction)
    }

    /// The zeroing's calibration (`crate::sights::Sights::calibrate`), in
    /// first person, and a launcher's line as it leaves it.
    fn calibrate(&mut self) {
        if self.outside() {
            return;
        }
        let aiming = self.aiming;
        let Some(sights) = self.settings.sights.as_mut() else { return };
        sights.calibrate(aiming);
        if aiming && sights.launcher() {
            self.sight = self.line_in_force(self.sight_noted);
        }
    }

    /// Moves the camera toward its rest beside the weapon, or toward the
    /// sight line while aiming.
    fn camera_position(&mut self, dt: f32, aim: Option<AimPoint>) -> [f32; 3] {
        let d = &self.settings.data;
        // The sight is the target only with blind fire's blender at 0
        // (`CalculateCameraPosition`): sights brought up on blind fire's
        // way out wait for it.
        let target = match aim.filter(|_| self.aiming && self.blind_value.abs() < f32::EPSILON) {
            Some(aim) => {
                // Sideways and up: the sight's aim bone. Forward: the
                // weapon's aim plane, drawn in with the weapon at wide
                // fields of view and then pushed out again a little.
                // The plane is the sight's (`crate::sights::SightEye`): an
                // optic's by the far plane and its own distance.
                let scale = self.fov_scale();
                let eye = self.sights().map_or(crate::sights::SightEye::iron(self.settings.aim_plane_depth), crate::sights::Sights::eye);
                let plane = aim.origin[2] + aim.barrel[2] * eye.depth * scale;
                // A weapon drawn back from an obstacle takes the eye back
                // with it. And the eye is put on the iron sights' line:
                // up from the sight's aim bone, and along the view.
                let [along, up] = self.sight.shift;
                // With the left shoulder's curve up the eye comes a little
                // nearer at a wide field of view (`_aimLeftStanceAdditionalOffset`,
                // -0.06, times one less the squeeze).
                let shoulder = if self.left_shoulder_curve > 0.0 { LEFT_SHOULDER_AIM_NEARER * (1.0 - scale) } else { 0.0 };
                let depth = plane + eye.compensation(scale) - self.turn_away.position[1] + along + shoulder;
                if self.recoil.auto_fire() {
                    // In a run of automatic fire the target is slid along
                    // the sight's own line to that depth, not straight
                    // along the view, so that the eye stays on the line as
                    // the recoil turns the weapon (`CheckAutoFire`; a way
                    // of no length leaves the bone where it is, as the
                    // engine's `normalized` does).
                    let size = (aim.line[0] * aim.line[0] + aim.line[1] * aim.line[1] + aim.line[2] * aim.line[2]).sqrt();
                    let way = if size > 1e-5 { aim.line.map(|v| v / size) } else { [0.0; 3] };
                    let reach = depth - aim.bone[2];
                    [aim.bone[0] + way[0] * reach, aim.bone[1] + way[1] * reach + up, aim.bone[2] + way[2] * reach]
                } else {
                    [aim.bone[0], aim.bone[1] + up, depth]
                }
            }
            None => [d.camera_offset[0], d.camera_offset[1], d.camera_offset[2] + self.turn_away.camera],
        };
        let (sideways, forward) = if self.aiming {
            (
                self.camera_aim_rate(),
                1.0 + self.hands_position.current[1] * 100.0 + self.turn_away.position[1] * 10.0,
            )
        } else {
            (d.camera_smooth_out, d.camera_smooth_out)
        };
        let c = self.camera_local;
        let spring = self.camera_position.current;
        // The game blends from where the camera was drawn, spring and all.
        self.camera_local = [
            lerp(c[0], target[0], dt * sideways) + spring[0],
            lerp(c[1], target[1], dt * sideways) + spring[1],
            lerp(c[2], target[2], d.camera_smooth_time * dt * forward) + spring[2],
        ];
        self.camera_local
    }

    /// The mount's camera place (`MountedWeaponCamera`): the camera's offset
    /// from the weapon's root (which a mount moves) blended toward the
    /// sight's eye by the aiming weight, the turn-away's pull not taken off;
    /// reached across and up at the mount's camera speed (at once) and along
    /// at the usual easing times it.
    fn mounted_camera_position(&mut self, dt: f32, aim: Option<AimPoint>) -> [f32; 3] {
        let d = &self.settings.data;
        let shift = self.rig_shift;
        let resting: [f32; 3] = std::array::from_fn(|k| shift[k] + d.camera_offset[k]);
        let eye = match aim {
            Some(aim) => {
                let scale = self.fov_scale();
                let eye = self.sights().map_or(crate::sights::SightEye::iron(self.settings.aim_plane_depth), crate::sights::Sights::eye);
                let plane = aim.origin[2] + aim.barrel[2] * eye.depth * scale;
                let [along, up] = self.sight.shift;
                let depth = plane + eye.compensation(scale) + along;
                let at = if self.recoil.auto_fire() {
                    let size = (aim.line[0] * aim.line[0] + aim.line[1] * aim.line[1] + aim.line[2] * aim.line[2]).sqrt();
                    let way = if size > 1e-5 { aim.line.map(|v| v / size) } else { [0.0; 3] };
                    let reach = depth - aim.bone[2];
                    [aim.bone[0] + way[0] * reach, aim.bone[1] + way[1] * reach + up, aim.bone[2] + way[2] * reach]
                } else {
                    [aim.bone[0], aim.bone[1] + up, depth]
                };
                std::array::from_fn(|k| shift[k] + at[k])
            }
            None => resting,
        };
        let weight = self.aim_weight;
        let target: [f32; 3] = std::array::from_fn(|k| lerp(resting[k], eye[k], weight));
        let speed = self.settings.mount_settings.camera_speed;
        let along_rate = if self.aiming { 1.0 + self.hands_position.current[1] * 100.0 + self.turn_away.position[1] * 10.0 } else { d.camera_smooth_out };
        let c = self.camera_local;
        let spring = self.camera_position.current;
        self.camera_local = [
            lerp(c[0], target[0], dt * speed) + spring[0],
            lerp(c[1], target[1], dt * speed) + spring[1],
            lerp(c[2], target[2], d.camera_smooth_time * dt * along_rate * speed) + spring[2],
        ];
        self.camera_local
    }

    /// Moves the camera by `push`, in the frame it is placed in, and says
    /// where that leaves it. Last in each frame the game pushes the
    /// camera's box out of the world (`Movement::camera_push`), and what it
    /// moves is the camera itself: the next frame's easing starts from
    /// there, so a camera whose target is in the ground rests on it.
    pub fn push_camera(&mut self, push: [f32; 3]) -> [f32; 3] {
        for (at, by) in self.camera_local.iter_mut().zip(push) {
            *at += by;
        }
        self.camera_local
    }

    /// The camera's rotation beside the weapon root, advanced by one
    /// rendered frame.
    ///
    /// `animated` is the local rotation of the weapon's animated camera
    /// bone. With the body's camera bone it rests at a half turn about x,
    /// which the game undoes at the end.
    fn camera_rotation(&mut self, frame_dt: f32, animated: [f32; 4]) -> [f32; 4] {
        let half_turn = quat_euler([180.0, 0.0, 0.0]);
        // The body's own camera bone moves with the body's clips; without a
        // body it stays at rest.
        let bones = quat_mul(animated, self.body_camera.unwrap_or(self.settings.data.body_camera_rotation));
        // The mount's camera takes the bones' turn whole (`MountedWeaponCamera`).
        let bone = if self.mount_strategy { bones } else { quat_lerp(quat_euler([-180.0, 0.0, 0.0]), bones, self.settings.head_bobbing) };
        // The head's free look is added to the spring's figure, and while
        // the head is turned at all the camera does not follow the
        // weapon's climb (`AddHandRecoilRotateToCamera`).
        let spring = self.camera_rotation.value();
        let looking = self.head != [0.0; 3];
        let turned: [f32; 3] = if looking { std::array::from_fn(|i| spring[i] + self.head[i]) } else { spring };
        let mut rotation = quat_mul(quat_mul(bone, quat_euler(turned)), half_turn);
        if !looking {
            rotation = self.follow_recoil(frame_dt, rotation);
        }
        // The camera's own kick is added to its angles, not composed.
        let kick = self.recoil.camera_angles();
        let angles = quat_to_euler(rotation);
        quat_euler(std::array::from_fn(|i| angles[i] + kick[i]))
    }

    /// Blends the camera toward the weapon's climb while aiming, so that
    /// the sight picture rises with the muzzle. From the hip the same blend
    /// only smooths the camera.
    fn follow_recoil(&mut self, frame_dt: f32, rotation: [f32; 4]) -> [f32; 4] {
        let [slowest, fastest] = self.settings.camera_follow_range;
        let step = self.settings.camera_follow_step * frame_dt;
        let climb = self.recoil.hand_rotation.current;
        let auto_fire = self.recoil.hand_rotation.auto_fire_on;
        // `Mathf.Clamp` with the bounds as the weapon gives them.
        let limit = |speed: f32| if speed < slowest { slowest } else { speed.min(fastest) };

        let (target, blend) = if !self.aiming {
            self.camera_follow_speed = limit(self.camera_follow_speed - step);
            self.camera_follow_target = rotation;
            (rotation, fastest)
        } else if auto_fire && climb == [0.0; 3] {
            // Nothing to follow yet; the game leaves the camera alone.
            return rotation;
        } else if auto_fire && self.recoil.hand_rotation.stable_on {
            // Held automatic fire: keep the last target, taking only one
            // component from the camera.
            self.camera_follow_speed = limit(self.camera_follow_speed + step);
            let mut target = self.camera_follow_target;
            target[2] = rotation[2];
            (target, self.camera_follow_speed)
        } else {
            let mut base = rotation;
            base[1] = 0.0;
            let target = quat_mul(base, quat_euler(climb));
            let change = if auto_fire { step } else { -step };
            self.camera_follow_speed = limit(self.camera_follow_speed + change);
            self.camera_follow_target = target;
            (target, self.camera_follow_speed)
        };
        // The first frame has nothing to blend from and lands on the target.
        let result = self.camera_follow.map_or(quat_lerp(target, target, 1.0), |from| quat_lerp(from, target, blend));
        self.camera_follow = Some(result);
        result
    }

    /// Offset the camera spring adds to the camera's position.
    pub fn camera_offset(&self) -> [f32; 3] {
        self.camera_position.current
    }

    /// The four springs' values, for comparing against the running game.
    pub fn spring_values(&self) -> [[f32; 3]; 4] {
        [
            self.hands_position.value(),
            self.hands_rotation.value(),
            self.camera_rotation.value(),
            self.camera_position.current,
        ]
    }
}

/// Evaluates a curve that repeats after its last key.
fn looped(curve: &Curve, time: f32) -> f32 {
    let (Some(first), Some(last)) = (curve.keys.first(), curve.keys.last()) else { return 0.0 };
    let span = last.time - first.time;
    if span <= 0.0 {
        return first.value;
    }
    curve.evaluate(first.time + (time - first.time).rem_euclid(span))
}

/// World vector into the view's axes: undo the yaw, then the pitch.
fn to_view(v: [f32; 3], yaw: f32, pitch: f32) -> [f32; 3] {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    let x = v[0] * cy - v[2] * sy;
    let z = v[0] * sy + v[2] * cy;
    [x, v[1] * cp + z * sp, -v[1] * sp + z * cp]
}

/// View vector into world axes: the pitch, then the yaw.
fn from_view(v: [f32; 3], yaw: f32, pitch: f32) -> [f32; 3] {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    let y = v[1] * cp - v[2] * sp;
    let z = v[1] * sp + v[2] * cp;
    [v[0] * cy + z * sy, y, -v[0] * sy + z * cy]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recoil's strengths as `recoil_settings` works them out with no
    /// skill, and with a class skill at 50 (0.15 off the forces, not off
    /// the aiming settings' bonus).
    #[test]
    fn a_class_skill_takes_its_share_off_the_recoil_forces() {
        let force = RecoilForce { up: 100.0, back: 300.0, vert_bonus: 20.0, back_bonus: 10.0, multiplier: 0.5, rotation_range: [0.9, 1.1], position_range: [0.8, 1.2], parts: 0.9, folded_parts: None };
        let (rotation, position) = force.strengths(0.9, 0.0);
        let plain = (100.0 * 0.9 + 20.0) * 0.5;
        assert_eq!(rotation, [0.9 * plain, 1.1 * plain]);
        assert_eq!(position[1], 1.2 * ((300.0 * 0.9 + 10.0) * 0.5));
        let (skilled, _) = force.strengths(0.9, 0.15);
        let less = (100.0 * (0.9 * 0.85) + 20.0) * 0.5;
        assert!((skilled[0] - 0.9 * less).abs() < 1e-4, "{skilled:?}");
    }

    #[test]
    fn view_axes_round_trip_and_follow_the_game_conventions() {
        // Looking down by 30 degrees tilts forward toward -y.
        let forward = from_view([0.0, 0.0, 1.0], 0.0, 30.0);
        assert!(forward[1] < -0.49 && forward[1] > -0.51);
        // Facing right by 90 degrees points forward along +x.
        let forward = from_view([0.0, 0.0, 1.0], 90.0, 0.0);
        assert!((forward[0] - 1.0).abs() < 1e-5);
        let v = [0.3, -0.2, 0.9];
        let back = to_view(from_view(v, 40.0, -25.0), 40.0, -25.0);
        for i in 0..3 {
            assert!((back[i] - v[i]).abs() < 1e-5);
        }
    }

    #[test]
    fn a_blast_s_shake_kicks_on_its_schedule() {
        // Strength 1 at 64 frames a second (no wait ends on a frame's
        // edge): kicks at once, then on the first frame each wait of 0.1,
        // 0.11, 0.121, 0.1331 and 0.14641 s is up; each three quarters of
        // the one before; then the time left is spent.
        let mut shake = Shake::new(1.0);
        let mut kicks = Vec::new();
        for frame in 0..120 {
            if let Some(power) = shake.frame(1.0 / 64.0) {
                kicks.push((frame, power));
            }
        }
        let frames: Vec<i32> = kicks.iter().map(|&(frame, _)| frame).collect();
        assert_eq!(frames, [0, 7, 15, 23, 32, 42]);
        assert!((kicks[0].1 - 1.2).abs() < 1e-6 && (kicks[1].1 - 0.9).abs() < 1e-6);
        assert!(shake.left <= 0.0);
    }

    #[test]
    fn slerp_turns_by_the_angle_and_blends_the_length() {
        let half = slerp3([1.0, 0.0, 0.0], [0.0, 2.0, 0.0], 0.5);
        let length = (half[0] * half[0] + half[1] * half[1]).sqrt();
        assert!((length - 1.5).abs() < 1e-5 && (half[0] - half[1]).abs() < 1e-5);
        // From nought, a straight blend.
        assert_eq!(slerp3([0.0; 3], [0.0, 0.0, 6.0], 1.0 / 6.0), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn spring_returns_to_zero_after_a_push() {
        let mut s = Spring::new(SpringData { return_speed: 0.066, damping: 0.66, acceleration_max: 10.0, input_intensity: 1.0 });
        s.add(0, 3.0);
        s.step();
        assert!(s.current[0] > 1.0);
        for _ in 0..600 {
            s.step();
        }
        assert!(s.current[0].abs() < 1e-4);
    }

    #[test]
    fn spring_clamps_one_push_to_its_limit() {
        let data = SpringData { return_speed: 0.1, damping: 0.5, acceleration_max: 10.0, input_intensity: 1.0 };
        let (mut a, mut b) = (Spring::new(data), Spring::new(data));
        a.add(1, 10.0);
        b.add(1, 500.0);
        a.step();
        b.step();
        assert_eq!(a.current, b.current);
    }

    #[test]
    fn sway_oscillates_and_dies_out() {
        let mut s = Sway::new(7.0, 0.8);
        s.kick(2, 10.0);
        let mut peak = 0.0f32;
        let mut crossed = false;
        for _ in 0..600 {
            s.step(1.0 / 60.0);
            peak = peak.max(s.position[2]);
            crossed |= s.position[2] < 0.0;
        }
        assert!(peak > 0.3);
        assert!(crossed, "an underdamped spring overshoots");
        assert!(s.position[2].abs() < 1e-4);
    }

    #[test]
    fn looped_curves_repeat() {
        let c = Curve::linear(&[(0.0, 0.0), (1.0, 2.0)]);
        assert!((looped(&c, 0.25) - 0.5).abs() < 1e-5);
        assert!((looped(&c, 3.25) - 0.5).abs() < 1e-5);
    }
}
