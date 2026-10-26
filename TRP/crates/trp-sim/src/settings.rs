//! Everything the movement simulation reads from the game's data, gathered
//! into one value. Sources are described in `docs/spec/movement.md`.

use anyhow::{anyhow, Context, Result};
use trp_data::curve::Curve;
use trp_data::hard_settings::HardSettings;
use trp_data::rootmotion::RootMotionTable;
use serde_json::Value as Json;

use crate::head_look::HeadLookSettings;
use crate::math::{inverse_lerp, lerp};
use crate::stamina::StaminaSettings;
use crate::vaulting::VaultingSettings;

#[derive(Clone)]
pub struct MovementSettings {
    /// Shared, not copied, by the settings for another weight
    /// (`reweighed`): its copy had cost some 9 ms every time what was
    /// carried changed.
    pub table: std::sync::Arc<RootMotionTable>,
    pub move_state: u32,
    pub sprint_state: u32,

    /// Carried weight in kilograms, which the inertia values derive from.
    pub weight: f32,
    /// 0 for an unburdened character, rising to 1 with load.
    pub inertia: f32,

    /// Upper end of the speed wheel.
    pub max_speed: f32,

    /// The figures of the sounds of moving (`docs/spec/footsteps.md`).
    pub steps: crate::steps::StepSettings,
    pub sprinting_speed: f32,
    /// Both of those for an untrained character and at the top of the
    /// Strength skill, and the share of the top walking speed that carried
    /// weight leaves.
    walk_speed_range: [f32; 2],
    sprint_speed_range: [f32; 2],
    walk_speed_limit: f32,
    /// Speed cap from carried weight; 1 means no cap.
    pub state_speed_limit: f32,
    /// The Strength skill's level, 0 to 60.
    strength: f32,
    /// What the hard caps on speed shrink to for a character too heavy to walk freely.
    sprint_speed_lower_limit: f32,
    /// After a jump the selected speed is capped for a while: the share of
    /// the top walking speed taken off, and for how many seconds.
    pub jump_penalty: f32,
    pub jump_penalty_duration: f32,
    /// A fall of this many metres or more caps the selected speed too: at
    /// what, and for how many seconds, by the height fallen.
    pub fall_threshold: f32,
    pub fall_speed_limit: Curve,
    pub fall_limit_duration: Curve,

    pub walk_inertia: f32,
    pub side_inertia: f32,
    pub diagonal_inertia: f32,
    pub move_time: f32,
    pub max_time_without_input: f32,
    pub exit_speed_threshold: f32,
    pub min_direction_blend_time: f32,
    pub sudden_changes_smoothness: f32,
    pub crouch_speed_acceleration: [f32; 2],
    /// Crawling: what the wanted speed is multiplied by, and what the
    /// direction's smoothing time is divided by.
    pub prone_speed_acceleration: f32,
    pub prone_direction_acceleration: f32,
    /// How fast the clips that go down and get up play.
    pub transition_speed: f32,

    pub pre_sprint_acceleration: f32,
    pub sprint_acceleration: f32,
    pub physical_sprint_speed: f32,
    /// What a sprint multiplies its turn rate and turn limit by: 1 unless
    /// the character is overweight for sprinting (`Physical.SprintSensitivity`).
    pub sprint_sensitivity: f32,
    pub starting_sprint_speed: f32,
    pub rotation_frame_span: usize,
    pub sprint_speed_inertia: Curve,

    pub tilt_inertia: f32,
    pub tilt_max_speed: f32,
    pub inertia_backward_coef: f32,
    /// How fast the step aside and the step back play: where that speed
    /// rests, what it gains a second on the way, and its top (movement
    /// spec, "The sidestep").
    pub sideback_speed: f32,
    pub sideback_acceleration: f32,
    pub sideback_max_speed: f32,
    /// How fast the first-person pose of a body state comes in and goes
    /// out, for `Hands::weapon_flip_speed`.
    pub weapon_flip_speed: f32,

    pub pose_increase_speed: f32,
    pub pose_decrease_speed: f32,
    pub pose_changing_speed: f32,

    pub idle_motion_preservation: f32,
    /// What the Strength skill multiplies the jump's push-off by; 1 untrained.
    pub jump_boost: f32,
    /// How much of the last motion carries over while a sprint comes to a stop.
    pub sprint_stop_motion_preservation: f32,
    pub inertia_input_max_speed: f32,
    pub deceleration_speed: f32,
    pub direction_lerp_speed: f32,
    pub rotation_lerp_speed: f32,
    /// How far the look direction may turn from the body before the body follows, in degrees.
    pub trunk_rotation_limit: f32,
    /// How far the head's free look turns it (`docs/spec/free-look.md`).
    pub head_look: HeadLookSettings,
    /// How fast a lying body is laid to the ground, per second.
    pub prone_align_speed: f32,
    /// The movement states, by the game's numbers, in which the sights
    /// cannot be up.
    pub no_aim_states: Vec<i32>,
    /// The movement states in which the hands take no input: a press of
    /// the trigger made in one is dropped (firing spec, "Pressing the
    /// trigger").
    pub no_hands_states: Vec<i32>,
    /// Refuse the prone key while a reload is in hand, as the player of
    /// an online raid does. The offline player does not, and every
    /// recording is of that one: off unless a host sets it (movement
    /// spec, "Prone").
    pub prone_key_refused_in_reload: bool,
    pub crouch_acceleration_weight: Curve,
    /// How far under the selected speed the walk's speed may not fall, by
    /// the walk's inertia. Its second key is keyed by a character's
    /// inertia, and the game keeps one curve for every character in the
    /// raid: here it is keyed by this character's own until a host says
    /// otherwise (`key_acceleration_range`; movement spec, "The floor's
    /// curve is shared").
    pub movement_acceleration_range: Curve,
    /// Where that second key lies for a character of no inertia and for
    /// one of full inertia (`MinMovementAccelerationRangeRight`,
    /// `MaxMovementAccelerationRangeRight`): time, value.
    pub acceleration_range_right: [[f32; 2]; 2],

    pub air_control_same: f32,
    pub air_control_back: f32,
    pub air_control_other: f32,
    pub air_min_speed: f32,
    pub air_lerp: f32,
    pub jump_descending_exit_time: f32,
    pub lift_velocity_by_speed: Curve,
    pub jump_delay_by_speed: Curve,
    pub gravity: f32,

    /// One notch of the speed wheel and of the stance wheel.
    pub delta_speed: f32,
    pub delta_level: f32,
    /// The rate the body's lean closes on one made slowly
    /// (`TILT_CHANGING_SPEED`): the share of what is left, a second.
    pub tilt_changing_speed: f32,

    /// How fast the stance rises and falls and how fast going down and
    /// getting up play, each from an unburdened character to a fully
    /// overweight one, and how overweight this one is. Spent legs count
    /// as overweight for these (stamina spec, "What running out does").
    pub pose_increase_range: [f32; 2],
    pub pose_decrease_range: [f32; 2],
    pub transition_speed_range: [f32; 2],
    pub overweight: f32,
    /// What Strength makes of the weight's limits (`WeightLimits`), which
    /// the inertia and every overweight share above were worked out with.
    pub limits: WeightLimits,
    /// What stamina runs on.
    pub stamina: StaminaSettings,
    /// What vaulting and climbing run on (vaulting spec).
    pub vaulting: VaultingSettings,
    /// How long after the key a container or a trunk does what it was told,
    /// and a searched container is asked to be shown, in seconds: the
    /// install's `DelayToOpenContainer` (0.55; the class's default is 0.5).
    /// A door waits a half second of its own (interactions spec).
    pub thing_answers_after: f32,
    /// The box a roll needs to that side and the ray its ground is felt
    /// with (`RollCheckShift`, `RollCheckSize`, `RollCheckGroundCastLength`:
    /// 0.5, 0 and 0.2 m from the ribcage over the feet, 0.25 by 0.1 by
    /// 2.1 m, and 0.5 m in the install; movement spec, "Room to move").
    pub roll_check_shift: [f32; 3],
    pub roll_check_size: [f32; 3],
    pub roll_check_cast: f32,
    /// The head's two capsules on the player prefab (movement spec, "The
    /// head's push out of walls"); none where the settings were made
    /// without the install's prefab, and then nothing pushes the head.
    pub head: Option<trp_data::player::HeadCollidersData>,
}

/// What the Strength skill makes of the carried weight's limits
/// (`PhysicalBase.UpdateWeightLimits`): the inertia's limits raised by
/// `InertiaLimitsStep` a level of Strength (`SummaryLevel`), and every
/// overweight limit times the skills' carrying factor (1 + Strength's lift,
/// `SkillManager.CarryingWeightRelativeModifier`). `BASE` is an untrained
/// character's, which the settings are made with unless told.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightLimits {
    pub strength_level: f32,
    pub carry: f32,
}

impl WeightLimits {
    pub const BASE: Self = Self { strength_level: 0.0, carry: 1.0 };

    /// From the skills' buffs.
    pub fn of(buffs: &crate::skills::Buffs) -> Self {
        Self { strength_level: buffs.strength_level as f32, carry: 1.0 + buffs.strength_lift }
    }
}

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

        // The overweight limits times the carrying factor (1 untrained).
        let carried = |name: &str| range(stamina_cfg, name).map(|[low, high]| [low * limits.carry, high * limits.carry]);
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
            steps: crate::steps::StepSettings::new(config, hard)?,
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
            limits,
            stamina: StaminaSettings::new_carrying(config, weight, hard.float("DELTA_LEVEL")?, limits.carry)?,
            vaulting: VaultingSettings::new(config)?,
            thing_answers_after: hard.float("DelayToOpenContainer")?,
            roll_check_shift: hard.vec3("RollCheckShift")?,
            roll_check_size: hard.vec3("RollCheckSize")?,
            roll_check_cast: hard.float("RollCheckGroundCastLength")?,
            head: None,
        })
    }
}

impl MovementSettings {
    /// These settings for another carried weight: everything that goes by
    /// the weight worked out again, as for a character made carrying it,
    /// with the Strength this one has. The game does so whenever the
    /// inventory's sum changes (inventory spec, "Weight"). The curve under
    /// the walk's floor is keyed by this character again, as the game keys
    /// it for whichever character's weight changed last.
    pub fn reweighed(&self, hard: &HardSettings, config: &Json, weight: f32) -> Result<Self> {
        self.relimited(hard, config, weight, self.limits)
    }

    /// These settings for a carried weight and the weight's limits as
    /// Strength makes them (`PhysicalBase.UpdateWeightLimits`, which the
    /// game runs when Strength's level changes as when the weight does).
    pub fn relimited(&self, hard: &HardSettings, config: &Json, weight: f32, limits: WeightLimits) -> Result<Self> {
        let mut fresh = Self::new_limited(self.table.clone(), hard, config, weight, self.gravity, limits)?;
        fresh.set_strength(self.strength);
        fresh.prone_key_refused_in_reload = self.prone_key_refused_in_reload;
        fresh.steps = self.steps.clone();
        fresh.head = self.head.clone();
        Ok(fresh)
    }

    /// Sets the character's Strength skill level, 0 to 60. The top walking
    /// speed and the top sprint speed both rise with it.
    pub fn set_strength(&mut self, level: f32) {
        let t = (level / 60.0).clamp(0.0, 1.0);
        self.strength = t * 60.0;
        self.max_speed = lerp(self.walk_speed_range[0], self.walk_speed_range[1], t);
        self.sprinting_speed = lerp(self.sprint_speed_range[0], self.sprint_speed_range[1], t);
        // The skill also pushes harder off the ground: a fifth more at
        // level 50, in proportion below, and no more above
        // (`StrengthBuffJumpHeightInc`, its rule held at 50).
        self.jump_boost = 1.0 + 0.2 * level.clamp(0.0, 50.0) / 50.0;
        self.state_speed_limit =
            if self.walk_speed_limit < 1.0 { self.walk_speed_limit * self.max_speed } else { 1.0 };
    }

    /// Keys the curve under the walk's floor as a character of this
    /// inertia leaves it. The game has one such curve for the whole raid
    /// and every character, a bot too, re-keys it when its carried weight
    /// changes, so the player's walk reads whichever character's inertia
    /// came last. A host with other characters of its own, or a replay of
    /// a raid that had them, says whose.
    pub fn key_acceleration_range(&mut self, inertia: f32) {
        let [lo, hi] = self.acceleration_range_right;
        let t = inertia.clamp(0.0, 1.0);
        self.movement_acceleration_range.move_key(1, lerp(lo[0], hi[0], t), lerp(lo[1], hi[1], t));
    }

    /// The inertia whose key gives that curve the value `value` at this
    /// character's walk inertia: what a recorded floor says of the key it
    /// was worked out with. None where no key gives it within `within`.
    pub fn acceleration_key_giving(&self, value: f32, within: f32) -> Option<f32> {
        let [lo, hi] = self.acceleration_range_right;
        let at = |inertia: f32| {
            let mut keyed = self.movement_acceleration_range.clone();
            keyed.move_key(1, lerp(lo[0], hi[0], inertia), lerp(lo[1], hi[1], inertia));
            keyed.evaluate(self.walk_inertia)
        };
        let (none, full) = (at(0.0), at(1.0));
        if value < none.min(full) - within || value > none.max(full) + within {
            return None;
        }
        // The curve's value there rises or falls all the way with the key.
        let (mut low, mut high) = (0.0f32, 1.0f32);
        for _ in 0..40 {
            let middle = 0.5 * (low + high);
            if (at(middle) < value) == (none < full) {
                low = middle;
            } else {
                high = middle;
            }
        }
        Some(0.5 * (low + high))
    }

    /// The fastest the character can actually be moved, in metres per
    /// second, in a movement state whose cap runs over `range` from an
    /// untrained character to the top of the Strength skill. Carried weight
    /// brings it down, except in a sprint.
    pub fn hard_speed_cap(&self, range: [f32; 2], sprinting: bool) -> f32 {
        // The game counts the skill in whole levels.
        let by_skill = lerp(range[0], range[1], self.strength.round() / 60.0);
        let weight = if sprinting { 1.0 } else { self.walk_speed_limit };
        by_skill * lerp(self.sprint_speed_lower_limit, 1.0, weight)
    }

    /// What the direction held multiplies the walk's smoothing time by:
    /// more for a diagonal or a sideways walk.
    pub fn direction_inertia(&self, input: [f32; 2]) -> f32 {
        match (input[0] != 0.0, input[1] != 0.0) {
            (true, true) => self.diagonal_inertia,
            (true, false) => self.side_inertia,
            _ => 1.0,
        }
    }

    /// The Strength level at which the top walking speed is `max_speed`.
    pub fn strength_for(&self, max_speed: f32) -> f32 {
        let [low, high] = self.walk_speed_range;
        if high > low {
            ((max_speed - low) / (high - low)).clamp(0.0, 1.0) * 60.0
        } else {
            0.0
        }
    }
}
