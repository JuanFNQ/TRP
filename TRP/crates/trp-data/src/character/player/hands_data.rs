//! The hands' shake, turn-away, sway, blind fire and force-react settings.

use super::*;

/// The hand shake's tuning (`HandShakeEffector` on the player prefab):
/// what shakes the weapon for a time after a quick draw that leaves the
/// arms empty (stamina spec, "The quick draw's hand shake").
#[derive(Debug, Clone)]
pub struct HandShakeData {
    /// Its strength until a shake sets its own (`_shakeIntensity`; the
    /// class puts it back to 3 as a shake ends).
    pub intensity: f32,
    /// The hands' level is taken between these two, then multiplied by the
    /// third (`_energyLowerLimit`, `_energyFractureLimit`, `_energyMultiplier`).
    pub lower_limit: f32,
    pub fracture_limit: f32,
    pub energy_multiplier: f32,
    /// The two wandering pushes on the hands' rotation spring: its yaw
    /// (`_handsRotationSpringXRandom`) and its roll (`...YRandom`).
    pub hands_x: SmoothRandomData,
    pub hands_y: SmoothRandomData,
    /// The three that move the muzzle's rest point
    /// (`_recoilRotationSpringTremorXRandom`, `Y`, `Z`).
    pub tremor: [SmoothRandomData; 3],
    /// Whether the shake fades out, and from what share of its time
    /// (`_enableDamping`, `_dampingStartTimePercent`).
    pub damping: bool,
    pub damping_start_percent: f32,
    /// Two parts the install leaves unused, kept to be checked: the hands'
    /// pushes taken from curves over the level (`_useHandShakeParams`, off)
    /// and the shake's curves (`_curves`, none).
    pub uses_params: bool,
    pub curves: usize,
}

/// One component of a turn-away pose: a curve of the effector's value
/// times an intensity that goes from `near` to `far` once the value is
/// past 1. Those two are the first-person figures; `outside` is the pair
/// for a character seen from outside (`AltIntensity`, `AltIntensity1`).
#[derive(Debug, Clone)]
pub struct TurnAwayVal {
    /// 0 x, 1 y, 2 z.
    pub component: u8,
    pub near: f32,
    pub far: f32,
    pub outside: [f32; 2],
    pub curve: Curve,
}

/// What the turn-away adds to the weapon's moved bone, or to an elbow's goal.
#[derive(Debug, Clone, Default)]
pub struct TurnAwayPose {
    pub position: Vec<TurnAwayVal>,
    pub rotation: Vec<TurnAwayVal>,
}

/// The turn-away from obstacles (`TurnAwayEffector` on the player prefab).
#[derive(Debug, Clone)]
pub struct TurnAwayData {
    /// The depth, in metres, at which the effector's value is 1; and where
    /// what the weapon meets is another character (`_playerOverlapThreshold`).
    pub threshold: f32,
    pub player_threshold: f32,
    /// How fast the value follows the depth, rising and falling, per second.
    pub in_speed: f32,
    pub out_speed: f32,
    /// How fast the far blend and the camera's shift follow, per second.
    pub blend_speed: f32,
    pub rifle: TurnAwayPose,
    pub pistol: TurnAwayPose,
    pub prone: TurnAwayPose,
    /// Shifts of the elbows' goals: left, then right.
    pub elbows: [TurnAwayPose; 2],
    /// The camera's shift along the view by the value, and what it is
    /// with the value past 1.
    pub camera_shift: Curve,
    pub camera_shift_at_most: f32,
}

/// The effectors a character's weapon motion runs, as bits of the prefab's
/// mask (`EProceduralAnimationMask`).
pub mod effector {
    pub const BREATHING: u32 = 1;
    pub const WALKING: u32 = 2;
    pub const MOTION_REACTION: u32 = 4;
    pub const FORCE_REACTION: u32 = 8;
    pub const SHOOTING: u32 = 16;
    pub const DRAW_DOWN: u32 = 32;
    pub const AIMING: u32 = 64;
    pub const HAND_SHAKE: u32 = 128;
}

/// The aim sway's figures (`ProceduralWeaponAnimation`): the box its
/// direction is drawn in, lowest and highest each way; how fast it fades
/// (`SwayFalloff`, over the aiming speed); and the weight, ergonomics taken
/// off, at which it starts and is whole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AimSwayData {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub falloff: f32,
    pub starts: f32,
    pub full: f32,
}

#[derive(Debug, Clone)]
pub struct HandsData {
    pub turn_away: TurnAwayData,
    pub hands_position: SpringData,
    pub hands_rotation: SpringData,
    pub camera_rotation: SpringData,
    pub camera_position: SpringData,
    pub sway_frequency: f32,
    pub sway_damping: f32,
    /// Resting place of the camera beside the weapon root.
    pub camera_offset: [f32; 3],
    /// The point the hands' rig swings about as the prefab has it, which
    /// stands until a weapon taken in hand names its own.
    pub rotation_center: [f32; 3],
    /// Rest rotation (`xyzw`) of the body's camera bone, which the game
    /// multiplies onto the weapon's animated camera bone.
    pub body_camera_rotation: [f32; 4],

    pub motion_intensity: f32,
    pub rotation_input_clamp: f32,
    pub mouse: MotionParameters,
    pub movement: MotionParameters,

    pub walk_step_frequency: f32,
    /// Walk intensity at speed 0 and at speed 1.
    pub walk_intensity_range: [f32; 2],
    pub walk_normal: Vec<WalkEntry>,
    /// The walk's bob on a damaged leg (`WalkEffector.Presets`' second,
    /// `EWalkPreset.lame`).
    pub walk_lame: Vec<WalkEntry>,
    /// The load's own bob, played over the walk's in proportion to the
    /// overweight (`WalkEffector.OverweightPresets`: the normal one and the
    /// lame one; empty where the prefab has none).
    pub walk_overweight: Vec<WalkEntry>,
    pub walk_overweight_lame: Vec<WalkEntry>,

    pub camera_smooth_time: f32,
    pub camera_smooth_out: f32,
    /// Where the camera's aiming rate rests, the rate it drops to after a
    /// shot when the weapon names none, and how fast it recovers per second.
    pub camera_smooth_steady: f32,
    pub camera_smooth_recoil: f32,
    pub camera_smooth_blend_speed: f32,
    pub aiming_displacement_str: f32,
    /// A heavy weapon's sway as the sights come up (`ProceduralWeaponAnimation`
    /// `AimSwayMin`, `AimSwayMax`, `SwayFalloff`, `AimSwayStartsThreshold`,
    /// `AimSwayMaxThreshold`; weapon-handling spec, "Aim sway").
    pub aim_sway: AimSwayData,

    pub recoil: RecoilData,
    pub breath: BreathData,
    pub hand_shake: HandShakeData,

    /// Which effectors the prefab switches on (`effector`).
    pub mask: u32,
    /// What the sights coming up push into the springs of a character seen
    /// from outside (`CustomEffector.AimPose`): each curve is read at how
    /// far up the sights are, 0 to 1, and that goes up and down at
    /// `aim_pose_speed` a second.
    pub aim_pose: Vec<WalkEntry>,
    pub aim_pose_speed: f32,
    /// The force reaction (`ForceReact`, a `ForceEffector`).
    pub force_react: ForceReactData,
    /// Blind fire's figures (blind-fire spec, "The weapon and the camera").
    pub blind_fire: BlindFireData,
}

/// Blind fire's figures on the player prefab's `ProceduralWeaponAnimation`:
/// its blender's speed a second (`BlindfireBlender.Speed`), the camera's
/// turn over the top and to the right at full value before it is halved
/// (`BlindFireCamera`, `SideFireCamera`, degrees), and the end points the
/// weapon's line is first cast from, over the top and to the right
/// (`BlindFireOffset`, `SideFireOffset`, metres).
#[derive(Debug, Clone, Copy, Default)]
pub struct BlindFireData {
    pub speed: f32,
    pub camera: [[f32; 3]; 2],
    pub end: [[f32; 3]; 2],
}

/// The force reaction's figures (`ForceEffector`): its intensity, what it
/// pushes (each value from a component of the force, `From`, into a
/// spring's component, `To`: 0 the camera's rotation, 1 the camera's
/// position, which the game's `Initialize` passes where it names the
/// hands' position, 2 the hands' rotation), and its noise's reach about x
/// and y and about z (`_eulersNoiseXYMaxValue`, `_eulersNoiseZMaxValue`).
#[derive(Debug, Clone, Default)]
pub struct ForceReactData {
    pub intensity: f32,
    pub vals: Vec<EffectorVal>,
    pub noise_xy: f32,
    pub noise_z: f32,
    /// How much shorter a hit's push may be drawn (`_deltaRandom`).
    pub delta_random: f32,
}
