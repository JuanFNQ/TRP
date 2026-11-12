//! Reading the movement settings from the install.

use super::*;

impl MovementSettings {
    /// `config` is the `config` section of `globals.json`; `gravity` is the
    /// engine's vertical gravity, negative. An untrained character's limits
    /// (`WeightLimits::BASE`).
    pub fn new(table: impl Into<std::sync::Arc<RootMotionTable>>, hard: &HardSettings, config: &Json, weight: f32, gravity: f32) -> Result<Self> {
        Self::new_limited(table, hard, config, weight, gravity, WeightLimits::BASE)
    }

    /// As `new`, with the weight's limits as Strength makes them.
    pub fn new_limited(table: impl Into<std::sync::Arc<RootMotionTable>>, hard: &HardSettings, config: &Json, weight: f32, gravity: f32, limits: WeightLimits) -> Result<Self> {
        let table = table.into();
        let section = |name: &str| config.get(name).ok_or_else(|| anyhow!("globals have no {name}"));
        let inertia_cfg = section("Inertia")?;
        let stamina_cfg = section("Stamina")?;
        let range = |from: &Json, name: &str| -> Result<[f32; 2]> {
            let v = from.get(name).ok_or_else(|| anyhow!("globals have no {name}"))?;
            let part = |k: &str| v.get(k).and_then(Json::as_f64).map(|f| f as f32);
            Ok([part("x").context(name.to_owned())?, part("y").context(name.to_owned())?])
        };
        let number = |from: &Json, name: &str| -> Result<f32> {
            from.get(name).and_then(Json::as_f64).map(|f| f as f32).ok_or_else(|| anyhow!("globals have no {name}"))
        };
        let at = |r: [f32; 2], t: f32| lerp(r[0], r[1], t);
        let inverse = |r: [f32; 2], v: f32| inverse_lerp(r[0], r[1], v);

        // The inertia's limits raised by Strength's level, a step a level
        // (an untrained character's: the base values).
        let inertia_limits = inertia_cfg.get("InertiaLimits").ok_or_else(|| anyhow!("globals have no InertiaLimits"))?;
        let mix = inertia_limits.get("z").and_then(Json::as_f64).unwrap_or(0.0) as f32;
        let step = number(inertia_cfg, "InertiaLimitsStep").unwrap_or(0.0) * limits.strength_level;
        let [low, high] = range(inertia_cfg, "InertiaLimits")?;
        let n = inverse([low + step, high + step], weight);
        let inertia = lerp(n * n * n, n, mix);

        // The overweight limits times the carrying factor (1 untrained), or
        // out of reach with encumbrance off.
        let carried = |name: &str| range(stamina_cfg, name).map(|limit| limits.overweight_limit(limit));
        let overweight = inverse(carried("BaseOverweightLimits")?, weight);
        let sprint_overweight = inverse(carried("SprintOverweightLimits")?, weight);
        let walk_speed_limit = 1.0 - inverse(carried("WalkSpeedOverweightLimits")?, weight);
        let max_speed = range(config, "WalkSpeed")?[0];

        let mut movement_acceleration_range = hard.curve("MovementAccelerationRange")?.clone();
        let (lo, hi) = (
            range(inertia_cfg, "MinMovementAccelerationRangeRight")?,
            range(inertia_cfg, "MaxMovementAccelerationRangeRight")?,
        );
        movement_acceleration_range.move_key(1, lerp(lo[0], hi[0], inertia), lerp(lo[1], hi[1], inertia));

        let curve_from = |min: &str, max: &str| -> Result<Curve> {
            let (a, b) = (range(inertia_cfg, min)?, range(inertia_cfg, max)?);
            Ok(Curve::flat(&[(a[0], a[1]), (b[0], b[1])]))
        };
        let tilt_curve = curve_from("InertiaTiltCurveMin", "InertiaTiltCurveMax")?;
        // How much a landing costs grows with the weight carried.
        let after_jump = at(range(inertia_cfg, "SpeedInertiaAfterJump")?, inertia);

        Ok(Self {
            steps: crate::sound::steps::StepSettings::new(config, hard)?,
            move_state: table.state_named("MOVE").context("table has no MOVE state")?,
            sprint_state: table.state_named("Sprint").context("table has no Sprint state")?,
            table,
            weight,
            inertia,
            max_speed,
            sprinting_speed: range(config, "SprintSpeed")?[0],
            walk_speed_range: range(config, "WalkSpeed")?,
            sprint_speed_range: range(config, "SprintSpeed")?,
            walk_speed_limit,
            state_speed_limit: if walk_speed_limit < 1.0 { walk_speed_limit * max_speed } else { 1.0 },
            strength: 0.0,
            sprint_speed_lower_limit: number(stamina_cfg, "SprintSpeedLowerLimit")?,
            jump_penalty: number(inertia_cfg, "BaseJumpPenalty")?
                + after_jump.powf(number(inertia_cfg, "PenaltyPower")?)
                - 1.0,
            jump_penalty_duration: number(inertia_cfg, "BaseJumpPenaltyDuration")?
                + after_jump.powf(number(inertia_cfg, "DurationPower")?)
                - 1.0,
            fall_threshold: number(inertia_cfg, "FallThreshold")?,
            fall_speed_limit: curve_from("SpeedLimitAfterFallMin", "SpeedLimitAfterFallMax")?,
            fall_limit_duration: curve_from("SpeedLimitDurationMin", "SpeedLimitDurationMax")?,
            walk_inertia: at(range(inertia_cfg, "WalkInertia")?, inertia),
            side_inertia: at(range(inertia_cfg, "SideTime")?, inertia),
            diagonal_inertia: at(range(inertia_cfg, "DiagonalTime")?, inertia),
            move_time: at(range(inertia_cfg, "MoveTimeRange")?, inertia),
            max_time_without_input: at(range(inertia_cfg, "MaxTimeWithoutInput")?, inertia),
            exit_speed_threshold: at(range(inertia_cfg, "ExitMovementStateSpeedThreshold")?, inertia),
            min_direction_blend_time: number(inertia_cfg, "MinDirectionBlendTime")?,
            // Not in globals.json; this is the default written in the game's code.
            sudden_changes_smoothness: number(inertia_cfg, "SuddenChangesSmoothness").unwrap_or(3.0),
            crouch_speed_acceleration: range(inertia_cfg, "CrouchSpeedAccelerationRange")?,
            prone_speed_acceleration: at(range(inertia_cfg, "ProneSpeedAccelerationRange")?, inertia),
            prone_direction_acceleration: at(range(inertia_cfg, "ProneDirectionAccelerationRange")?, inertia),
            transition_speed: at(range(stamina_cfg, "TransitionSpeed")?, overweight),
            pre_sprint_acceleration: at(range(inertia_cfg, "PreSprintAccelerationLimits")?, inertia),
            sprint_acceleration: inverse(range(inertia_cfg, "SprintAccelerationLimits")?, inertia),
            physical_sprint_speed: lerp(1.0, number(stamina_cfg, "SprintSpeedLowerLimit")?, sprint_overweight),
            sprint_sensitivity: lerp(1.0, number(stamina_cfg, "SprintSensitivityLowerLimit")?, sprint_overweight),
            starting_sprint_speed: hard.float("StartingSprintSpeed")?,
            rotation_frame_span: number(inertia_cfg, "AverageRotationFrameSpan")?.max(1.0) as usize,
            sprint_speed_inertia: curve_from("SprintSpeedInertiaCurveMin", "SprintSpeedInertiaCurveMax")?,
            tilt_inertia: tilt_curve.evaluate(inertia),
            tilt_max_speed: at(range(inertia_cfg, "TiltInertiaMaxSpeed")?, inertia),
            inertia_backward_coef: at(range(inertia_cfg, "InertiaBackwardCoef")?, inertia),
            sideback_speed: at(range(inertia_cfg, "TiltStartSideBackSpeed")?, inertia),
            sideback_acceleration: at(range(inertia_cfg, "TiltAcceleration")?, inertia),
            sideback_max_speed: at(range(inertia_cfg, "TiltMaxSideBackSpeed")?, inertia),
            weapon_flip_speed: at(range(inertia_cfg, "WeaponFlipSpeed")?, inertia),
            pose_increase_speed: at(range(stamina_cfg, "PoseLevelIncreaseSpeed")?, overweight),
            pose_decrease_speed: at(range(stamina_cfg, "PoseLevelDecreaseSpeed")?, overweight),
            pose_changing_speed: hard.float("POSE_CHANGING_SPEED")?,
            idle_motion_preservation: hard.float("IdleStateMotionPreservation")?,
            jump_boost: 1.0,
            sprint_stop_motion_preservation: at(range(inertia_cfg, "SprintTransitionMotionPreservation")?, inertia),
            sprint_brake_inertia: at(range(inertia_cfg, "SprintBrakeInertia")?, inertia),
            inertia_input_max_speed: hard.float("InertiaInputMaxSpeed")?,
            deceleration_speed: hard.float("DecelerationSpeed")?,
            direction_lerp_speed: hard.float("DIRECTION_LERP_SPEED")?,
            rotation_lerp_speed: hard.float("TRANSFORM_ROTATION_LERP_SPEED")?,
            trunk_rotation_limit: hard.float("HANDS_TO_BODY_MAX_ANGLE")?,
            head_look: HeadLookSettings {
                across: hard.vec2("MOUSE_LOOK_HORIZONTAL_LIMIT")?,
                down: hard.vec2("MOUSE_LOOK_VERTICAL_LIMIT")?,
                aiming: hard.float("MOUSE_LOOK_LIMIT_IN_AIMING_COEF")?,
            },
            prone_align_speed: hard.float("PRONE_ALIGN_SPEED")?,
            no_aim_states: hard.bytes("NO_AIM_STATES")?.iter().map(|state| i32::from(*state)).collect(),
            no_hands_states: hard.bytes("UnsuitableStates")?.iter().map(|state| i32::from(*state)).collect(),
            prone_key_refused_in_reload: false,
            crouch_acceleration_weight: hard.curve("CrouchAccelerationWeightRange")?.clone(),
            movement_acceleration_range,
            acceleration_range_right: [lo, hi],
            floor_key: inertia,
            air_control_same: hard.float("AIR_CONTROL_SAME_DIR")?,
            air_control_back: hard.float("AIR_CONTROL_BACK_DIR")?,
            air_control_other: hard.float("AIR_CONTROL_NONE_OR_ORT_DIR")?,
            air_min_speed: hard.float("AIR_MIN_SPEED")?,
            air_lerp: hard.float("AIR_LERP")?,
            jump_descending_exit_time: hard.float("JumpTimeDescendingForStateExit")?,
            lift_velocity_by_speed: hard.curve("LIFT_VELOCITY_BY_SPEED")?.clone(),
            jump_delay_by_speed: hard.curve("JUMP_DELAY_BY_SPEED")?.clone(),
            gravity,
            delta_speed: hard.float("DELTA_SPEED")?,
            delta_level: hard.float("DELTA_LEVEL")?,
            tilt_changing_speed: hard.float("TILT_CHANGING_SPEED")?,
            pose_increase_range: range(stamina_cfg, "PoseLevelIncreaseSpeed")?,
            pose_decrease_range: range(stamina_cfg, "PoseLevelDecreaseSpeed")?,
            transition_speed_range: range(stamina_cfg, "TransitionSpeed")?,
            overweight,
            sprint_overweight,
            limits,
            stamina: StaminaSettings::new_limited(config, weight, hard.float("DELTA_LEVEL")?, limits)?,
            vaulting: VaultingSettings::new(config)?,
            thing_answers_after: hard.float("DelayToOpenContainer")?,
            roll_check_shift: hard.vec3("RollCheckShift")?,
            roll_check_size: hard.vec3("RollCheckSize")?,
            roll_check_cast: hard.float("RollCheckGroundCastLength")?,
            head: None,
        })
    }
}
