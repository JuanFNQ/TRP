//! Tuning data on the player prefab: the springs and effectors of the
//! first-person weapon animation. See `docs/spec/weapon-handling.md`.

use anyhow::{anyhow, Context, Result};

use crate::formats::curve::{Curve, Key, Wrap};
use crate::unity::extract::script_class;
use crate::unity::{class, Assets, ObjRef, Value};

mod hands_data;
mod hands_load;
mod body_colliders;

pub use hands_data::*;
pub use hands_load::*;
pub use body_colliders::*;
use hands_load::float;

pub const PLAYER_BUNDLE: &str = "assets/content/commonprefabs/playersuperior.bundle";

#[derive(Debug, Clone, Copy)]
pub struct SpringData {
    pub return_speed: f32,
    pub damping: f32,
    pub acceleration_max: f32,
    pub input_intensity: f32,
}

/// Where an effector entry reads from and writes to.
#[derive(Debug, Clone, Copy)]
pub struct EffectorVal {
    /// Motion entries only: 0..3 mouse x velocity, x acceleration, y
    /// velocity, y acceleration; 4..6 movement velocity; 7..9 movement acceleration.
    pub source: u8,
    /// 0 camera rotation, 1 hands position, 2 hands rotation.
    pub target: u8,
    pub component: u8,
    pub intensity: f32,
}

#[derive(Debug, Clone)]
pub struct MotionParameters {
    pub acc_smooth: f32,
    pub vel_smooth: f32,
    pub magnitude: Curve,
    pub vals: Vec<EffectorVal>,
}

/// One curve of a walk preset, over one step cycle (`AnimVal`); the breath's
/// and the aim pose's are kept so too.
#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub curve: Curve,
    /// How the curve goes on before its first key and after its last
    /// (`m_PreInfinity`, `m_PostInfinity`): most of the walk's repeat, the
    /// lame walk's hands' pitch runs back and forth (weapon-handling spec,
    /// "Walking").
    pub before: Wrap,
    pub after: Wrap,
    pub time_shift: f32,
    pub vals: Vec<EffectorVal>,
}

impl WalkEntry {
    /// The curve at a time on its clock, which runs on past its keys
    /// (`AnimValProcessor.Process`: `Curve.Evaluate(_time)`), taken back
    /// into them by the curve's own wraps.
    pub fn at(&self, time: f32) -> f32 {
        self.curve.evaluate_wrapped(time, self.before, self.after)
    }
}

/// One of the three recoil processes: a spring stepped once per physics step.
#[derive(Debug, Clone, Copy)]
pub struct RecoilProcessData {
    pub return_speed: f32,
    pub damping: f32,
    pub intensity: f32,
}

/// Routes one axis of a shot's recoil direction into one axis of a process.
#[derive(Debug, Clone, Copy)]
pub struct RecoilShotVal {
    pub from_axis: u8,
    /// 0 camera rotation, 1 hands position, 2 hands rotation. Other values
    /// are stored on the prefab but the game ignores them.
    pub target: u8,
    pub to_axis: u8,
    pub intensity: f32,
}

/// The shot effector's tuning on the player prefab. The weapon's template
/// replaces several of these when it is taken in hand.
#[derive(Debug, Clone)]
pub struct RecoilData {
    pub progress_power_on: bool,
    pub progress_power_shot: i32,
    pub incoming_strength_multiplier: f32,
    pub shot_vals: Vec<RecoilShotVal>,
    pub rotation_strength_range: [f32; 2],
    pub position_strength_range: [f32; 2],
    pub hand_rotation: RecoilProcessData,
    pub hand_position: RecoilProcessData,
    pub camera_rotation: RecoilProcessData,
    pub max_recoil_rotation_angle: f32,
    pub to_next_stable_point_distance_limit: f32,
    pub next_stable_point_distance_range: [f32; 2],
    pub to_next_current_speed: f32,
    pub return_speed_curve: Curve,
}

/// A value that wanders: every so often it picks a new random target
/// within its amplitude and turns toward it.
#[derive(Debug, Clone, Copy)]
pub struct SmoothRandomData {
    pub amplitude: f32,
    /// Shortest and longest time between new targets, in seconds.
    pub delay: [f32; 2],
    /// How sharply it turns toward a new target, 0 to 1 and beyond.
    pub hardness: f32,
}

/// The breathing effector's tuning.
#[derive(Debug, Clone)]
pub struct BreathData {
    /// Small random pushes on the weapon's roll (`y`) and yaw (`x`).
    pub x: SmoothRandomData,
    pub y: SmoothRandomData,
    /// The slow wander of the muzzle when not aiming.
    pub hip_x: SmoothRandomData,
    pub hip_z: SmoothRandomData,
    pub hip_penalty: f32,
    /// The breath itself: one curve per cycle, pushed into the springs.
    /// The second entry is the camera's share.
    pub curves: Vec<WalkEntry>,
    /// The random pushes' amplitude, delay and hardness against arm stamina, 0 to 1.
    pub amplitude_by_stamina: Curve,
    pub delay_by_stamina: Curve,
    pub hardness_by_stamina: Curve,
    /// 1 with enough oxygen, falling to 0 without.
    pub lack_of_oxygen: Curve,
    /// What a shot from the hip adds to the wander, how long it holds and
    /// how fast it fades; the weapon replaces all three.
    pub gain_per_shot: f32,
    pub gain_delay: f32,
    pub gain_speed: f32,
    pub hardness_speed: f32,
    pub base_hip_amplitudes: [f32; 2],
    pub random_between: [f32; 2],
    pub shot_hardness: f32,
    /// The hands' tremor, from pain or a broken arm (`TremorXRandom`,
    /// `TremorYRandom`, `TremorZRandom`).
    pub tremor: [SmoothRandomData; 3],
    /// The highest a broken arm lets the hands' level count
    /// (`EnergyFractureLimit`).
    pub fracture_limit: f32,
    /// The lowest the hands' level counts (`EnergyLowerLimit`): mounted, the
    /// breath runs only with the hands at it (mounting spec, "Breath").
    pub lower_limit: f32,
}
