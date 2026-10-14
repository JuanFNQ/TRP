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
use crate::recoil::{Recoil, RecoilSettings};

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
const AIM_FOV_DROP: f32 = 15.0;

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
    has_force: bool,
    pub input_intensity: f32,
    return_speed: f32,
    pub damping: f32,
    acceleration_max: f32,
}

impl Spring {
    pub fn new(data: SpringData) -> Self {
        Self {
            zero: [0.0; 3],
            current: [0.0; 3],
            velocity: [0.0; 3],
            force: [0.0; 3],
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

    /// One physics step. The formula has no time step in it.
    pub fn step(&mut self) {
        for i in 0..3 {
            if self.has_force {
                let limited = self.force[i].clamp(-self.acceleration_max, self.acceleration_max);
                self.velocity[i] += limited * self.input_intensity;
            }
            // The spring is drawn to its zero, and the zero is added again
            // when the value is read: a held offset counts twice at rest.
            self.velocity[i] -= (self.current[i] - self.zero[i]) * self.return_speed;
            self.velocity[i] *= self.damping;
            self.current[i] += self.velocity[i];
        }
        self.force = [0.0; 3];
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
    /// The player's field-of-view setting: vertical degrees, 50 to 75.
    pub field_of_view: f32,
    /// The player's head-bobbing setting, 0.2 to 1: how much of the
    /// animated camera rotation reaches the camera.
    pub head_bobbing: f32,
    /// What the character's skills add to the aiming speed, as a fraction.
    pub aim_speed_bonus: f32,
    /// How far ahead of the weapon's moving bone the eye sits when aiming,
    /// along the barrel, in metres: the nearer of the weapon's two aim planes.
    pub aim_plane_depth: f32,
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

pub struct Hands {
    pub settings: HandsSettings,
    pub hands_position: Spring,
    pub hands_rotation: Spring,
    pub camera_rotation: Spring,
    pub camera_position: Spring,
    pub sway: Sway,

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
    walking: bool,
    pub step_frequency: f32,

    sway_strength: f32,
    displacement_strength: f32,
    /// Reciprocal of the time to bring the sights up, per second.
    pub aiming_speed: f32,
    aiming: bool,
    /// The body's camera bone as its clips turn it, set by the host each
    /// frame when it runs the body's animator.
    pub body_camera: Option<[f32; 4]>,
    accumulator: f32,

    pub recoil: Recoil,
    breath: Breath,
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
    /// The iron sights' line as the host last read it off the rig, the
    /// one in force (taken as the sights come up, and none once they are
    /// down), and how far the weapon is turned for it just now.
    sight_noted: Option<SightLine>,
    sight: SightLine,
    sight_angle: f32,
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
}

impl HandsSettings {
    /// The weapon's weight as its ergonomics lighten it: what the sway
    /// scales with, and what aiming costs the arms by.
    pub fn ergonomic_weight(&self) -> f32 {
        self.weapon_weight * (1.0 - self.weapon_ergonomics.max(0.0).sqrt() / 25.0)
    }
}

impl Hands {
    pub fn new(settings: HandsSettings) -> Self {
        let d = &settings.data;
        // The weight that sway scales with shrinks as ergonomics rise.
        let ergonomic_weight = settings.ergonomic_weight();
        let a = settings.aiming;
        let t = inverse_lerp(a.light_weight, a.heavy_weight, settings.weapon_weight);
        let (slowest, fastest) = (lerp(a.max_time_light, a.max_time_heavy, t), lerp(a.min_time_light, a.min_time_heavy, t));
        let e = (settings.weapon_ergonomics / 100.0).clamp(0.0, 1.0);
        let k = if e < 0.25 { 0.25 + 3.0 * e * e } else { 2.0 * e - e * e };
        let mut hands_position = Spring::new(d.hands_position);
        hands_position.damping = settings.hand_damping;
        Self {
            recoil: Recoil::new(settings.recoil.clone(), settings.seed),
            breath: Breath::new(d.breath.clone(), settings.breath_gain, settings.seed.wrapping_add(1)),
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
            camera_local: d.camera_offset,
            poses: Vec::new(),
            pose_position: [0.0; 3],
            pose_rotation: [0.0; 3],
            pose_camera: [0.0; 3],
            lean_rest: 0.0,
            weapon_flip_speed: 1.0,
            closer: settings.held_closer,
            sight_noted: None,
            sight: SightLine::default(),
            sight_angle: 0.0,
            muzzle: None,
            turn_away: TurnAway::default(),
            view: [0.0; 2],
            weapon_root: settings.weapon_root,
            point_of_view: PointOfView::FirstPerson,
            mask: d.mask,
            aim_pose: Blender::new(0.0, d.aim_pose_speed),
            head: [0.0; 3],
            head_control: false,
            head_control_seen: false,
            body_camera: None,
            hands_position,
            hands_rotation: Spring::new(d.hands_rotation),
            camera_rotation: Spring::new(d.camera_rotation),
            camera_position: Spring::new(d.camera_position),
            sway: Sway::new(settings.sway_frequency, settings.sway_damping),
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
            walking: false,
            step_frequency: d.walk_step_frequency,
            sway_strength: settings.sway_strength_per_kg.evaluate(ergonomic_weight),
            displacement_strength: settings.displacement_strength_per_kg.evaluate(ergonomic_weight),
            aiming_speed: 1.0 / lerp(slowest, fastest, k) * (1.0 + settings.aim_speed_bonus),
            aiming: false,
            accumulator: 0.0,
            settings,
        }
    }

    /// Advances by `dt` seconds, running as many physics steps as fit.
    pub fn advance(&mut self, dt: f32, input: &HandsInput) {
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
            self.set_fov(if control { fov } else { fov - AIM_FOV_DROP }, 1.0);
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
        if input.crawling || !self.poses.is_empty() || self.mask & effector::DRAW_DOWN == 0 {
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
        // push into the springs is for the character's own eyes only.
        if !self.outside() {
            self.track(dt, input);
            self.react(dt);
            let breath = self.breath.step(dt, self.time, self.breath_intensity(), self.aiming, self.settings.head_bobbing, input.wind);
            for (target, component, value) in breath.pushes {
                self.spring(target).add(component as usize, value);
            }
            self.recoil.hand_rotation.offset = breath.muzzle_offset;
            self.walk(dt, input);
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
        self.lean_rest = input.tilt * self.possible_tilt;
        self.set_rests();

        if self.outside() {
            self.fixed_step_outside(dt, input);
            return;
        }

        // Crawling, and with a first-person pose held, the turn-away is
        // switched off: the depth is put to nothing every step.
        if input.crawling || !self.poses.is_empty() {
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
        let wanted = away.depth / d.threshold;
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
    }

    /// The sights' line in force, and how far the weapon is turned for
    /// it now, in degrees.
    pub fn sight_line(&self) -> (SightLine, f32) {
        (self.sight, self.sight_angle)
    }

    /// Where the weapon's two springs and the camera's rotation spring
    /// rest: the lean, and whatever first-person poses are blended in.
    fn set_rests(&mut self) {
        let (lean, turn, camera) = (self.lean_rest, self.pose_rotation, self.pose_camera);
        self.hands_position.zero = self.pose_position;
        // A weapon held closer rests that much nearer along its own y.
        if self.closer {
            self.hands_position.zero[1] = HELD_CLOSER;
        }
        self.hands_rotation.zero = [turn[0], lean + turn[1], turn[2]];
        self.camera_rotation.zero = [camera[0], camera[1], lean + camera[2]];
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
    /// and puts what they come to into the springs' rests. A host calls it
    /// once a frame, after the frame's physics steps and before it reads
    /// the weapon's transform or the camera.
    pub fn blend_poses(&mut self, frame_dt: f32) {
        // A weapon with both points of an iron sights' line is turned
        // toward levelling it while the sights are up, and back when they
        // are down, by a share of what is left each frame.
        if self.sight_noted.is_some() || self.sight_angle != 0.0 {
            let steps = self.steps_since_frame;
            let dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { frame_dt };
            let goal = if self.aiming { self.sight.angle } else { 0.0 };
            self.sight_angle = lerp(self.sight_angle, goal, (dt * SIGHT_LINE_RATE).clamp(0.0, 1.0));
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
    pub fn shoot(&mut self, strength: f32) {
        // A character whose mask has no shot feels none (a bot's).
        if self.mask & effector::SHOOTING == 0 {
            return;
        }
        self.camera_smooth.jump(self.settings.camera_snap, CAMERA_SNAP_DELAY, self.time);
        self.recoil.shoot(strength);
        // The breath is told of a shot for the character's own eyes only.
        if !self.outside() {
            self.breath.on_shot(self.time, self.breath_intensity(), self.aiming);
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

    /// The sights have gone up or down: the field of view starts toward
    /// its new figure, and what goes by the sights does so from here on.
    fn set_aiming(&mut self, aiming: bool) {
        // The sights' line is measured as they come up, and dropped as
        // they go down.
        self.sight = if aiming { self.sight_noted.unwrap_or_default() } else { SightLine::default() };
        // While the mouse has the head the sights going up or down leave
        // the field of view alone (`OnAimOrPoseChanged`).
        if aiming != self.aiming && !self.head_control {
            let fov = self.settings.field_of_view;
            self.set_fov(if aiming { fov - AIM_FOV_DROP } else { fov }, 1.0);
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
    pub fn set_field_of_view(&mut self, degrees: f32) {
        self.settings.field_of_view = degrees;
        self.set_fov(if self.aiming { degrees - AIM_FOV_DROP } else { degrees }, 1.0);
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
        let scale = dt * self.settings.data.motion_intensity;
        for val in movement {
            let force = self.source(val.source) * val.intensity * scale;
            self.spring(val.target).add(val.component as usize, force);
        }
        let s = self.sway_strength * self.intensity_by_aiming();
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
                // Stopping rewinds every curve to its start.
                self.walk_clocks.iter_mut().for_each(|c| *c = 0.0);
            }
            self.walking = false;
            return;
        }
        self.walking = true;
        let range = self.settings.data.walk_intensity_range;
        let intensity = lerp(range[0], range[1], input.speed);
        for index in 0..self.walk_clocks.len() {
            self.walk_clocks[index] += dt * self.step_frequency;
            let entry = &self.settings.data.walk_normal[index];
            let value = looped(&entry.curve, self.walk_clocks[index]) * intensity;
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
        let center = if outside {
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
            if scale < 1.0 {
                // A weapon squeezed along the view would otherwise show its
                // climb squeezed too.
                let adjust = |degrees: f32| (degrees.to_radians().tan() * scale).atan().to_degrees();
                recoil[0] = adjust(recoil[0]);
                recoil[2] = adjust(recoil[2]);
            }
            let turn = quat_euler(recoil);
            let pivot = self.settings.recoil_pivot;
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

        // Last, the turn-away from an obstacle: drawn back, then turned
        // about a point a little way along the barrel. The game takes the
        // angles as those of a vector turned from the bone's axes into the
        // world's, and turns the bone by them about the world's axes.
        let away = &self.turn_away;
        if away.position != [0.0; 3] || away.rotation != [0.0; 3] {
            position = std::array::from_fn(|i| position[i] + away.position[i]);
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
    pub fn frame_outside(&mut self) {
        self.steps_since_frame = 0;
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
        let steps = std::mem::take(&mut self.steps_since_frame);
        let dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { frame_dt };
        CameraPose {
            position: self.camera_position(dt, aim),
            rotation: self.camera_rotation(dt, animated),
            field_of_view: self.field_of_view(frame_dt),
        }
    }

    /// Moves the camera toward its rest beside the weapon, or toward the
    /// sight line while aiming.
    fn camera_position(&mut self, dt: f32, aim: Option<AimPoint>) -> [f32; 3] {
        let d = &self.settings.data;
        let target = match aim.filter(|_| self.aiming) {
            Some(aim) => {
                // Sideways and up: the sight's aim bone. Forward: the
                // weapon's aim plane, drawn in with the weapon at wide
                // fields of view and then pushed out again a little.
                let scale = self.fov_scale();
                let plane = aim.origin[2] + aim.barrel[2] * self.settings.aim_plane_depth * scale;
                // A weapon drawn back from an obstacle takes the eye back
                // with it. And the eye is put on the iron sights' line:
                // up from the sight's aim bone, and along the view.
                let [along, up] = self.sight.shift;
                [aim.bone[0], aim.bone[1] + up, plane + (1.0 - scale) * 0.075 - self.turn_away.position[1] + along]
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
        let bone = quat_lerp(quat_euler([-180.0, 0.0, 0.0]), bones, self.settings.head_bobbing);
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
