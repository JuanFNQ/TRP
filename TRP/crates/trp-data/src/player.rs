//! Tuning data on the player prefab: the springs and effectors of the
//! first-person weapon animation. See `docs/spec/weapon-handling.md`.

use anyhow::{anyhow, Context, Result};

use crate::curve::{Curve, Key};
use crate::unity::extract::script_class;
use crate::unity::{class, Assets, ObjRef, Value};

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

/// One curve of a walk preset, over one step cycle.
#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub curve: Curve,
    pub time_shift: f32,
    pub vals: Vec<EffectorVal>,
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
    /// The depth, in metres, at which the effector's value is 1.
    pub threshold: f32,
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

    pub camera_smooth_time: f32,
    pub camera_smooth_out: f32,
    /// Where the camera's aiming rate rests, the rate it drops to after a
    /// shot when the weapon names none, and how fast it recovers per second.
    pub camera_smooth_steady: f32,
    pub camera_smooth_recoil: f32,
    pub camera_smooth_blend_speed: f32,
    pub aiming_displacement_str: f32,

    pub recoil: RecoilData,
    pub breath: BreathData,

    /// Which effectors the prefab switches on (`effector`).
    pub mask: u32,
    /// What the sights coming up push into the springs of a character seen
    /// from outside (`CustomEffector.AimPose`): each curve is read at how
    /// far up the sights are, 0 to 1, and that goes up and down at
    /// `aim_pose_speed` a second.
    pub aim_pose: Vec<WalkEntry>,
    pub aim_pose_speed: f32,
}

impl HandsData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let files = assets.load(PLAYER_BUNDLE)?;
        let find = |assets: &Assets, name: &str| -> Result<(ObjRef, Value)> {
            let obj = files
                .iter()
                .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
                .find(|&obj| script_class(assets, obj).as_deref() == Some(name))
                .ok_or_else(|| anyhow!("player prefab has no {name}"))?;
            Ok((obj, assets.read(obj)?))
        };
        let (spring_obj, spring) = find(assets, "PlayerSpring")?;
        // The body's own camera bone. Its turn and the weapon's camera
        // bone's turn make the half turn the game takes as the camera's rest.
        let body_camera_bone = spring
            .req("CameraAnimatedTP")?
            .as_pptr()
            .and_then(|p| assets.resolve_loaded(spring_obj.file, p))
            .context("the player prefab names no body camera bone")?;
        let body_camera_rotation = assets
            .read(body_camera_bone)?
            .req("m_LocalRotation")?
            .as_vec4()
            .context("body camera bone has no rotation")?;
        let (pwa_obj, pwa) = find(assets, "ProceduralWeaponAnimation")?;
        let referenced = |assets: &Assets, v: &Value| -> Result<Value> {
            let obj = v
                .as_pptr()
                .and_then(|p| assets.resolve_loaded(pwa_obj.file, p))
                .context("unresolved reference on the player prefab")?;
            assets.read(obj)
        };

        let motion = pwa.req("MotionReact")?;
        let walk = pwa.req("Walk")?;
        let normal_preset = walk
            .req("Presets")?
            .as_array()
            .and_then(|a| a.first())
            .context("walk effector has no presets")?;
        let preset = referenced(assets, normal_preset)?;
        let mut walk_normal = Vec::new();
        for entry in preset.req("Curves")?.as_array().unwrap_or(&[]) {
            walk_normal.push(WalkEntry {
                curve: curve(entry.req("Curve")?)?,
                time_shift: float(entry, "TimeShift")?,
                vals: vals(entry.req("Vals")?)?,
            });
        }
        let ranges = walk.req("IntensityMinMax")?.as_array().unwrap_or(&[]);
        let range = ranges.first().context("walk effector has no intensity range")?;

        let breath = pwa.req("Breath")?;
        let breath_params = referenced(assets, breath.req("BreathParams")?)?;
        let mut breath_curves = Vec::new();
        for entry in breath.req("Curves")?.as_array().unwrap_or(&[]) {
            breath_curves.push(WalkEntry {
                curve: curve(entry.req("Curve")?)?,
                time_shift: float(entry, "TimeShift")?,
                vals: vals(entry.req("Vals")?)?,
            });
        }
        let random = |name: &str| -> Result<SmoothRandomData> {
            let v = breath.req(name)?;
            Ok(SmoothRandomData {
                amplitude: float(v, "Amplitude")?,
                delay: vec2(v.req("MinMaxDelay")?)?,
                hardness: float(v, "Hardness")?,
            })
        };
        let breath = BreathData {
            x: random("XRandom")?,
            y: random("YRandom")?,
            hip_x: random("HipXRandom")?,
            hip_z: random("HipZRandom")?,
            hip_penalty: float(breath, "HipPenalty")?,
            curves: breath_curves,
            amplitude_by_stamina: curve(breath_params.req("AmplitudeCurve")?)?,
            delay_by_stamina: curve(breath_params.req("Delay")?)?,
            hardness_by_stamina: curve(breath_params.req("Hardness")?)?,
            lack_of_oxygen: curve(breath.req("_lackOfOxygenStrength")?)?,
            gain_per_shot: float(breath, "AmplitudeGainPerShot")?,
            gain_delay: float(breath, "Delay")?,
            gain_speed: float(breath.req("AmplitudeGain")?, "Speed")?,
            hardness_speed: float(breath.req("Hardness")?, "Speed")?,
            base_hip_amplitudes: vec2(breath.req("_baseHipRandomAmplitudes")?)?,
            random_between: vec2(breath.req("_randomBetween")?)?,
            shot_hardness: float(breath, "_shotHardness")?,
        };

        let custom = pwa.req("CustomEffector")?;
        let mut aim_pose = Vec::new();
        for entry in referenced(assets, custom.req("AimPose")?)?.req("Curves")?.as_array().unwrap_or(&[]) {
            aim_pose.push(WalkEntry {
                curve: curve(entry.req("Curve")?)?,
                time_shift: float(entry, "TimeShift")?,
                vals: vals(entry.req("Vals")?)?,
            });
        }

        let away = pwa.req("TurnAway")?;
        let pose = |assets: &Assets, v: &Value| -> Result<TurnAwayPose> {
            let object = referenced(assets, v)?;
            let vals = |name: &str| -> Result<Vec<TurnAwayVal>> {
                let mut out = Vec::new();
                for val in object.req(name)?.as_array().unwrap_or(&[]) {
                    out.push(TurnAwayVal {
                        component: val.req("Component")?.as_i64().unwrap_or(0) as u8,
                        near: float(val, "Intensity")?,
                        far: float(val, "Intensity1")?,
                        outside: [float(val, "AltIntensity")?, float(val, "AltIntensity1")?],
                        curve: curve(val.req("Curve")?)?,
                    });
                }
                Ok(out)
            };
            Ok(TurnAwayPose { position: vals("Pos")?, rotation: vals("Rot")? })
        };
        let elbows = away.req("_elbows")?.as_array().unwrap_or(&[]);
        let (Some(left), Some(right)) = (elbows.first(), elbows.get(1)) else {
            return Err(anyhow!("the turn-away has no poses for the elbows"));
        };
        let turn_away = TurnAwayData {
            threshold: float(away, "_turnAwayThreshold")?,
            in_speed: float(away, "_inSmoothTime")?,
            out_speed: float(away, "_outSmoothTime")?,
            blend_speed: float(away, "_blendSpeed")?,
            rifle: pose(assets, away.req("_rifleShift")?)?,
            pistol: pose(assets, away.req("_pistolShift")?)?,
            prone: pose(assets, away.req("_proneShift")?)?,
            elbows: [pose(assets, left)?, pose(assets, right)?],
            camera_shift: curve(away.req("CameraShiftCurve")?)?,
            camera_shift_at_most: float(away, "_cameraShiftAtMaxOverlap")?,
        };

        Ok(Self {
            turn_away,
            hands_position: spring_data(spring.req("HandsPosition")?)?,
            hands_rotation: spring_data(spring.req("HandsRotation")?)?,
            camera_rotation: spring_data(spring.req("CameraRotation")?)?,
            camera_position: spring_data(spring.req("CameraPosition")?)?,
            sway_frequency: float(spring.req("SwaySpring")?, "_angularFrequency")?,
            sway_damping: float(spring.req("SwaySpring")?, "_dampingRatio")?,
            camera_offset: spring.req("CameraOffset")?.as_vec3().context("CameraOffset")?,
            rotation_center: spring.req("RotationCenter")?.as_vec3().context("RotationCenter")?,
            body_camera_rotation,
            motion_intensity: float(motion, "Intensity")?,
            rotation_input_clamp: float(motion, "RotationInputClamp")?,
            mouse: motion_parameters(&referenced(assets, motion.req("MouseParameters")?)?)?,
            movement: motion_parameters(&referenced(assets, motion.req("MovementParameters")?)?)?,
            walk_step_frequency: float(walk, "StepFrequency")?,
            walk_intensity_range: [float(range, "x")?, float(range, "y")?],
            walk_normal,
            camera_smooth_time: float(&pwa, "CameraSmoothTime")?,
            camera_smooth_out: float(&pwa, "CameraSmoothOut")?,
            camera_smooth_steady: float(&pwa, "CameraSmoothSteady")?,
            camera_smooth_recoil: float(&pwa, "CameraSmoothRecoil")?,
            camera_smooth_blend_speed: float(pwa.req("CameraSmoothBlender")?, "Speed")?,
            aiming_displacement_str: float(&pwa, "AimingDisplacementStr")?,
            recoil: recoil_data(pwa.req("Shootingg")?)?,
            breath,
            mask: pwa.req("Mask")?.as_i64().context("the weapon motion's mask is not a number")? as u32,
            aim_pose,
            aim_pose_speed: float(custom.req("PoseBlender")?, "Speed")?,
        })
    }
}

/// The shares of the free look's turn that the neck and the head each take
/// (`PlayerBones.NeckChain`, `HeadChain` on the player prefab).
pub fn head_chain(assets: &mut Assets) -> Result<[f32; 2]> {
    let files = assets.load(PLAYER_BUNDLE)?;
    let bones = files
        .iter()
        .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("PlayerBones"))
        .ok_or_else(|| anyhow!("player prefab has no PlayerBones"))?;
    let v = assets.read(bones)?;
    Ok([float(&v, "NeckChain")?, float(&v, "HeadChain")?])
}

/// The capsule the game moves the character about in (`CharacterControllerSpawner`
/// on the player prefab), in metres.
#[derive(Debug, Clone, Copy)]
pub struct CapsuleData {
    pub radius: f32,
    pub height: f32,
    /// How far outside the capsule the game's controller keeps things.
    pub skin_width: f32,
    /// The tallest step it walks up.
    pub step_offset: f32,
    /// The steepest slope it walks up, in degrees.
    pub slope_limit: f32,
}

impl CapsuleData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let files = assets.load(PLAYER_BUNDLE)?;
        let spawner = files
            .iter()
            .flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR))
            .find(|&obj| script_class(assets, obj).as_deref() == Some("CharacterControllerSpawner"))
            .ok_or_else(|| anyhow!("player prefab has no character controller"))?;
        let v = assets.read(spawner)?;
        Ok(Self {
            radius: float(&v, "_radius")?,
            height: float(&v, "_height")?,
            skin_width: float(&v, "_skinWidth")?,
            step_offset: float(&v, "_stepOffset")?,
            slope_limit: float(&v, "_slopeLimit")?,
        })
    }
}

/// The box that keeps the first-person camera out of the world: the box
/// collider the player prefab's `Cam` object pushes clear of things
/// (`ColliderExtrusion`), in the camera's own axes (x right, y up, z
/// ahead) and in metres.
#[derive(Debug, Clone, Copy)]
pub struct CameraBoxData {
    /// Its full width, height and depth.
    pub size: [f32; 3],
    /// Its centre from the camera.
    pub centre: [f32; 3],
}

impl CameraBoxData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let files = assets.load(PLAYER_BUNDLE)?;
        for obj in files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect::<Vec<_>>() {
            if script_class(assets, obj).as_deref() != Some("ColliderExtrusion") {
                continue;
            }
            let script = assets.read(obj)?;
            let named = |pointer: &str| -> Option<Value> {
                let target = script.get(pointer)?.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p))?;
                assets.read(target).ok()
            };
            // The prefab has two of these scripts; the camera's is on `Cam`.
            let on_camera = named("m_GameObject").is_some_and(|owner| owner.get("m_Name").and_then(Value::as_str) == Some("Cam"));
            if !on_camera {
                continue;
            }
            let collider = named("_collider").ok_or_else(|| anyhow!("the camera's collider is not in the player prefab"))?;
            let vec3 = |name: &str| collider.req(name)?.as_vec3().with_context(|| format!("{name} is not a vector"));
            return Ok(Self { size: vec3("m_Size")?, centre: vec3("m_Center")? });
        }
        Err(anyhow!("the player prefab's camera has no collider"))
    }
}

/// One of the forearm's two twist bones: how it is turned about its own
/// length after the arm is solved (`TwistRelax` on the player prefab).
#[derive(Debug, Clone, Copy)]
pub struct TwistData {
    pub weight: f32,
    /// How far from the first forearm bone's turning toward the palm's.
    pub crossfade: f32,
    /// The bone's own axis it is turned about, and another across it.
    pub twist_axis: [f32; 3],
    pub axis: [f32; 3],
    /// Where that second axis pointed at rest, in the first forearm bone's
    /// axes and in the palm's.
    pub in_parent: [f32; 3],
    pub in_child: [f32; 3],
}

/// How one arm's limb solver is set up on the player prefab (`LimbIK` on
/// the collarbone), with the forearm's second and third bones' twist.
#[derive(Debug, Clone, Copy)]
pub struct LimbData {
    /// How the elbow's side is chosen: 4 is toward a goal.
    pub bend_modifier: i64,
    pub bend_weight: f32,
    /// How much of the palm's own turning is kept through the solve.
    pub maintain_rotation: f32,
    pub twists: [TwistData; 2],
}

impl LimbData {
    /// The right arm's, then the left's.
    pub fn load(assets: &mut Assets) -> Result<[Self; 2]> {
        let files = assets.load(PLAYER_BUNDLE)?;
        // Every script of the prefab with the name of the object it is on.
        let mut scripts = Vec::new();
        for obj in files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect::<Vec<_>>() {
            let Some(class_name) = script_class(assets, obj) else { continue };
            if class_name != "LimbIK" && class_name != "TwistRelax" {
                continue;
            }
            let value = assets.read(obj)?;
            let owner = value
                .req("m_GameObject")?
                .as_pptr()
                .and_then(|p| assets.resolve_loaded(obj.file, p))
                .and_then(|owner| assets.read(owner).ok())
                .and_then(|owner| owner.get("m_Name").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_default();
            scripts.push((class_name, owner, value));
        }
        let on = |class_name: &str, owner: &str| -> Result<&Value> {
            scripts
                .iter()
                .find(|(c, o, _)| c == class_name && o == owner)
                .map(|(_, _, value)| value)
                .ok_or_else(|| anyhow!("the player prefab has no {class_name} on {owner}"))
        };
        let vec3 = |v: &Value, name: &str| v.req(name)?.as_vec3().with_context(|| format!("{name} is not a vector"));
        let twist = |owner: &str| -> Result<TwistData> {
            let v = on("TwistRelax", owner)?;
            Ok(TwistData {
                weight: float(v, "weight")?,
                crossfade: float(v, "parentChildCrossfade")?,
                twist_axis: vec3(v, "twistAxis")?,
                axis: vec3(v, "axis")?,
                in_parent: vec3(v, "_axisRelativeToParentDefault")?,
                in_child: vec3(v, "_axisRelativeToChildDefault")?,
            })
        };
        let limb = |side: char| -> Result<Self> {
            let solver = on("LimbIK", &format!("Base Human{side}Collarbone"))?.req("solver")?;
            Ok(Self {
                bend_modifier: solver.req("bendModifier")?.as_i64().context("bendModifier is not a number")?,
                bend_weight: float(solver, "bendModifierWeight")?,
                maintain_rotation: float(solver, "maintainRotationWeight")?,
                twists: [twist(&format!("Base Human{side}Forearm2"))?, twist(&format!("Base Human{side}Forearm3"))?],
            })
        };
        Ok([limb('R')?, limb('L')?])
    }
}

fn recoil_data(shot_effector: &Value) -> Result<RecoilData> {
    // 1 selects the current recoil model; the older one is still on the
    // prefab but nothing switches to it.
    let pipeline = shot_effector.req("RecoilPipeline")?.as_i64();
    if pipeline != Some(1) {
        return Err(anyhow!("the player prefab selects recoil pipeline {pipeline:?}, which is not implemented"));
    }
    let v = shot_effector.req("NewShotRecoil")?;
    let process = |name: &str| -> Result<RecoilProcessData> {
        let p = v.req(name)?;
        Ok(RecoilProcessData {
            return_speed: float(p, "ReturnSpeed")?,
            damping: float(p, "Damping")?,
            intensity: float(p, "Intensity")?,
        })
    };
    let rotation = v.req("HandRotationRecoil")?;
    let mut shot_vals = Vec::new();
    for val in v.req("ShotRecoilProcessValues")?.as_array().unwrap_or(&[]) {
        shot_vals.push(RecoilShotVal {
            from_axis: val.req("FromAxis")?.as_i64().unwrap_or(0) as u8,
            target: val.req("ProcessType")?.as_i64().unwrap_or(0) as u8,
            to_axis: val.req("ToAxis")?.as_i64().unwrap_or(0) as u8,
            intensity: float(val, "IntensityMultiplicator")?,
        });
    }
    Ok(RecoilData {
        progress_power_on: v.req("ProgressPowerOn")?.as_i64().unwrap_or(0) != 0,
        progress_power_shot: v.req("ProgressPowerShot")?.as_i64().unwrap_or(1) as i32,
        incoming_strength_multiplier: float(v, "IncomingRotationStrengthMultiplier")?,
        shot_vals,
        rotation_strength_range: vec2(v.req("BasicRecoilRotationStrengthRange")?)?,
        position_strength_range: vec2(v.req("BasicRecoilPositionStrengthRange")?)?,
        hand_rotation: process("HandRotationRecoil")?,
        hand_position: process("HandPositionRecoil")?,
        camera_rotation: process("CameraRotationRecoil")?,
        max_recoil_rotation_angle: float(rotation, "MaxRecoilRotationAngle")?,
        to_next_stable_point_distance_limit: float(rotation, "ToNextStablePointDistanceLimit")?,
        next_stable_point_distance_range: vec2(rotation.req("NextStablePointDistanceRange")?)?,
        to_next_current_speed: float(rotation, "ToNextCurrentSpeed")?,
        return_speed_curve: curve(rotation.req("ReturnSpeedCurve")?)?,
    })
}

fn vec2(v: &Value) -> Result<[f32; 2]> {
    Ok([float(v, "x")?, float(v, "y")?])
}

fn float(v: &Value, name: &str) -> Result<f32> {
    v.req(name)?.as_f32().with_context(|| format!("{name} is not a number"))
}

fn spring_data(v: &Value) -> Result<SpringData> {
    Ok(SpringData {
        return_speed: float(v, "ReturnSpeed")?,
        damping: float(v, "Damping")?,
        acceleration_max: float(v, "AccelerationMax")?,
        input_intensity: float(v, "InputIntensity")?,
    })
}

fn motion_parameters(v: &Value) -> Result<MotionParameters> {
    Ok(MotionParameters {
        acc_smooth: float(v, "AccSmooth")?,
        vel_smooth: float(v, "VelSmooth")?,
        magnitude: curve(v.req("Magnitude")?)?,
        vals: vals(v.req("Vals")?)?,
    })
}

fn vals(v: &Value) -> Result<Vec<EffectorVal>> {
    let mut out = Vec::new();
    for val in v.as_array().unwrap_or(&[]) {
        out.push(EffectorVal {
            // Walk entries have no source field.
            source: val.get("From").and_then(Value::as_i64).unwrap_or(0) as u8,
            target: val.req("To")?.as_i64().unwrap_or(0) as u8,
            component: val.req("Component")?.as_i64().unwrap_or(0) as u8,
            intensity: float(val, "Intensity")?,
        });
    }
    Ok(out)
}

/// Reads a serialized `AnimationCurve`.
pub fn curve(v: &Value) -> Result<Curve> {
    let mut keys = Vec::new();
    for k in v.req("m_Curve")?.as_array().unwrap_or(&[]) {
        keys.push(Key {
            time: float(k, "time")?,
            value: float(k, "value")?,
            in_slope: float(k, "inSlope")?,
            out_slope: float(k, "outSlope")?,
        });
    }
    Ok(Curve { keys })
}
