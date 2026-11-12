//! Loading the hands' settings from the player's prefab, and the value readers.

use super::*;

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
        let presets = walk.req("Presets")?.as_array().context("walk effector has no presets")?;
        let preset_curves = |preset: &Value| -> Result<Vec<WalkEntry>> {
            let preset = referenced(assets, preset)?;
            let mut entries = Vec::new();
            for entry in preset.req("Curves")?.as_array().unwrap_or(&[]) {
                entries.push(walk_entry(entry)?);
            }
            Ok(entries)
        };
        let walk_normal = preset_curves(presets.first().context("walk effector has no presets")?)?;
        // The lame one, if the prefab has one (`EWalkPreset.lame`, the
        // second); else the normal one.
        let walk_lame = match presets.get(1) {
            Some(preset) => preset_curves(preset)?,
            None => walk_normal.clone(),
        };
        // The load's presets, the same two by the same index.
        let loaded: &[Value] = walk.get("OverweightPresets").and_then(Value::as_array).unwrap_or(&[]);
        let walk_overweight = match loaded.first() {
            Some(preset) => preset_curves(preset)?,
            None => Vec::new(),
        };
        let walk_overweight_lame = match loaded.get(1) {
            Some(preset) => preset_curves(preset)?,
            None => walk_overweight.clone(),
        };
        let ranges = walk.req("IntensityMinMax")?.as_array().unwrap_or(&[]);
        let range = ranges.first().context("walk effector has no intensity range")?;

        let breath = pwa.req("Breath")?;
        let breath_params = referenced(assets, breath.req("BreathParams")?)?;
        let mut breath_curves = Vec::new();
        for entry in breath.req("Curves")?.as_array().unwrap_or(&[]) {
            breath_curves.push(walk_entry(entry)?);
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
            tremor: [random("TremorXRandom")?, random("TremorYRandom")?, random("TremorZRandom")?],
            fracture_limit: float(breath, "EnergyFractureLimit")?,
            // The class's figure where the prefab has none (0.05).
            lower_limit: float(breath, "EnergyLowerLimit").unwrap_or(0.05),
        };

        let shake = pwa.req("HandShakeEffector")?;
        let shake_random = |name: &str| -> Result<SmoothRandomData> {
            let v = shake.req(name)?;
            Ok(SmoothRandomData {
                amplitude: float(v, "Amplitude")?,
                delay: vec2(v.req("MinMaxDelay")?)?,
                hardness: float(v, "Hardness")?,
            })
        };
        let flag = |name: &str| -> Result<bool> { Ok(shake.req(name)?.as_i64().with_context(|| name.to_owned())? != 0) };
        let hand_shake = HandShakeData {
            intensity: float(shake, "_shakeIntensity")?,
            lower_limit: float(shake, "_energyLowerLimit")?,
            fracture_limit: float(shake, "_energyFractureLimit")?,
            energy_multiplier: float(shake, "_energyMultiplier")?,
            hands_x: shake_random("_handsRotationSpringXRandom")?,
            hands_y: shake_random("_handsRotationSpringYRandom")?,
            tremor: [
                shake_random("_recoilRotationSpringTremorXRandom")?,
                shake_random("_recoilRotationSpringTremorYRandom")?,
                shake_random("_recoilRotationSpringTremorZRandom")?,
            ],
            damping: flag("_enableDamping")?,
            damping_start_percent: float(shake, "_dampingStartTimePercent")?,
            uses_params: flag("_useHandShakeParams")?,
            curves: shake.req("_curves")?.as_array().map_or(0, <[Value]>::len),
        };

        let custom = pwa.req("CustomEffector")?;
        let mut aim_pose = Vec::new();
        for entry in referenced(assets, custom.req("AimPose")?)?.req("Curves")?.as_array().unwrap_or(&[]) {
            aim_pose.push(walk_entry(entry)?);
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
            player_threshold: float(away, "_playerOverlapThreshold")?,
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
            walk_lame,
            walk_overweight,
            walk_overweight_lame,
            camera_smooth_time: float(&pwa, "CameraSmoothTime")?,
            camera_smooth_out: float(&pwa, "CameraSmoothOut")?,
            camera_smooth_steady: float(&pwa, "CameraSmoothSteady")?,
            camera_smooth_recoil: float(&pwa, "CameraSmoothRecoil")?,
            camera_smooth_blend_speed: float(pwa.req("CameraSmoothBlender")?, "Speed")?,
            aiming_displacement_str: float(&pwa, "AimingDisplacementStr")?,
            aim_sway: AimSwayData {
                min: pwa.req("AimSwayMin")?.as_vec3().context("AimSwayMin")?,
                max: pwa.req("AimSwayMax")?.as_vec3().context("AimSwayMax")?,
                falloff: float(&pwa, "SwayFalloff")?,
                starts: float(&pwa, "AimSwayStartsThreshold")?,
                full: float(&pwa, "AimSwayMaxThreshold")?,
            },
            recoil: recoil_data(pwa.req("Shootingg")?)?,
            breath,
            hand_shake,
            mask: pwa.req("Mask")?.as_i64().context("the weapon motion's mask is not a number")? as u32,
            aim_pose,
            aim_pose_speed: float(custom.req("PoseBlender")?, "Speed")?,
            force_react: {
                let force = pwa.req("ForceReact")?;
                ForceReactData {
                    intensity: float(force, "Intensity")?,
                    vals: vals(force.req("Vals")?)?,
                    noise_xy: float(force, "_eulersNoiseXYMaxValue")?,
                    noise_z: float(force, "_eulersNoiseZMaxValue")?,
                    delta_random: float(force, "_deltaRandom")?,
                }
            },
            blind_fire: {
                let vector = |name: &str| pwa.req(name).ok().and_then(Value::as_vec3).with_context(|| name.to_owned());
                BlindFireData {
                    speed: float(pwa.req("BlindfireBlender")?, "Speed")?,
                    camera: [vector("BlindFireCamera")?, vector("SideFireCamera")?],
                    end: [vector("BlindFireOffset")?, vector("SideFireOffset")?],
                }
            },
        })
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

pub(super) fn float(v: &Value, name: &str) -> Result<f32> {
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

pub(super) fn vals(v: &Value) -> Result<Vec<EffectorVal>> {
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

/// A serialized `AnimationCurve`'s wraps before its first key and after its
/// last (`m_PreInfinity`, `m_PostInfinity`), each held where the file has
/// none.
pub fn wraps(v: &Value) -> (Wrap, Wrap) {
    let wrap = |name: &str| v.get(name).and_then(Value::as_i64).map_or(Wrap::Clamp, Wrap::of);
    (wrap("m_PreInfinity"), wrap("m_PostInfinity"))
}

/// One `AnimVal`: its curve with its wraps, its time shift and where it
/// pushes.
fn walk_entry(entry: &Value) -> Result<WalkEntry> {
    let shape = entry.req("Curve")?;
    let (before, after) = wraps(shape);
    Ok(WalkEntry { curve: curve(shape)?, before, after, time_shift: float(entry, "TimeShift")?, vals: vals(entry.req("Vals")?)? })
}
