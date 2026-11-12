//! Building the movement, its weight, skills, conditions and hits.

use super::*;

impl Movement {
    /// The character's carried weight changes, as when something is put
    /// on or taken off: the inertia, the speed limits and the stamina's
    /// costs all follow, from the next frame (`PhysicalBase` reads the
    /// inventory's sum whenever it changes; inventory spec, "Weight").
    /// The skills, the pools and the character's food and water stay.
    pub fn set_weight(
        &mut self,
        hard: &trp_data::setup::hard_settings::HardSettings,
        config: &serde_json::Value,
        weight: f32,
    ) -> anyhow::Result<()> {
        // With the weight switched off what is carried weighs nothing.
        let weight = if self.systems.weight { weight } else { 0.0 };
        let mut settings = self.settings.reweighed(hard, config, weight)?;
        self.stamina.reweigh(settings.stamina.overweight, settings.stamina.walk_overweight);
        settings.stamina = self.stamina.settings.clone();
        self.settings = settings;
        Ok(())
    }

    /// The inventory told of a change (`PhysicalBase.LateUpdate`, its
    /// weight marked out of date by an event: `inventory::Weighed`): the
    /// weight worked out again (`set_weight`) only where it has moved more
    /// than `WEIGHT_STEP` since it last was (movement spec, "The weight, as
    /// the inventory tells of it"). Whether it was.
    pub fn weigh(
        &mut self,
        hard: &trp_data::setup::hard_settings::HardSettings,
        config: &serde_json::Value,
        weight: f32,
    ) -> anyhow::Result<bool> {
        let seen = if self.systems.weight { weight } else { 0.0 };
        if (seen - self.settings.weight).abs() <= WEIGHT_STEP {
            return Ok(false);
        }
        self.set_weight(hard, config, weight)?;
        Ok(true)
    }

    /// Strength's level or its lift changes (`PhysicalBase.
    /// UpdateWeightLimits`, which the game runs on Strength's level up as
    /// on a change of weight): the inertia and every overweight share
    /// worked out again for the weight carried, the pools and the skills'
    /// stamina buffs kept.
    pub fn set_weight_limits(
        &mut self,
        hard: &trp_data::setup::hard_settings::HardSettings,
        config: &serde_json::Value,
        limits: crate::base::settings::WeightLimits,
    ) -> anyhow::Result<()> {
        if limits == self.settings.limits {
            return Ok(());
        }
        let mut settings = self.settings.relimited(hard, config, self.settings.weight, limits)?;
        self.stamina.reweigh(settings.stamina.overweight, settings.stamina.walk_overweight);
        settings.stamina = self.stamina.settings.clone();
        self.settings = settings;
        Ok(())
    }

    /// The skills' buffs put on the character (skills spec): Strength's top
    /// speeds and jump, the weight's limits (Strength's inertia step, the
    /// carrying factor `carry`, which is its lift times what health adds;
    /// encumbrance as it stands), the stamina's buffs (Endurance and
    /// Strength) and the steps' (Covert Movement). The pools keep what they
    /// hold: a host settles them to the new capacities
    /// (`Stamina::resettle`). The weight's limits not made is the error,
    /// the rest put on all the same.
    pub fn take_skills(
        &mut self,
        buffs: &crate::condition::skills::Buffs,
        carry: f32,
        hard: &trp_data::setup::hard_settings::HardSettings,
        config: &serde_json::Value,
    ) -> anyhow::Result<()> {
        self.settings.set_strength(buffs.strength_level as f32);
        let limits = crate::base::settings::WeightLimits { carry, unencumbered: self.settings.limits.unencumbered, ..crate::base::settings::WeightLimits::of(buffs) };
        let limited = self.set_weight_limits(hard, config, limits);
        self.stamina.settings.set_skill_buffs(buffs);
        self.settings.stamina = self.stamina.settings.clone();
        self.sounds.set_covert(buffs.covert);
        limited
    }

    pub fn new(settings: MovementSettings, body: Body) -> Self {
        let stamina = Stamina::new(settings.stamina.clone());
        let speed_memo = settings.max_speed;
        let sounds = crate::sound::steps::StepSounds::new(settings.steps.clone());
        Self {
            systems: crate::Systems::default(),
            head_look: HeadLook::new(settings.head_look),
            trunk_limit: settings.trunk_rotation_limit,
            character_speed: settings.max_speed,
            relative_speed: 1.0,
            speed_cap: settings.hard_speed_cap(STANDING_SPEED_CAP, false),
            landing_limit: None,
            landing_limit_in_effect: None,
            limits_changed: false,
            conditions: crate::condition::health::Conditions::default(),
            held_limits: [None; 3],
            held_limits_in_effect: [None; 3],
            shot_limit: None,
            shot_limit_in_effect: None,
            hits_burnt: (0, 0.0),
            moved: Vec::new(),
            sprint_distance: 0.0,
            frame_clock: 0.0,
            fall_from: 0.0,
            was_grounded: true,
            // The game starts the smoothed speed at the selected speed, and
            // standing still does not bring it down.
            smoothed_speed: settings.max_speed.clamp(0.0, settings.state_speed_limit),
            // A character is made with it at 1, and it only comes down to
            // the starting figure when a sprint ends (movement spec,
            // "Sprinting").
            sprint_speed: 1.0,
            sprint_inertia: settings.sprint_brake_inertia,
            // Every standing-type state sets the step's playback speed to
            // its resting figure on being entered; a character met
            // standing has it there.
            sideback_speed: settings.sideback_speed,
            sidestep_data: StepData { speed: settings.sideback_speed, ..StepData::default() },
            roll_data: StepData { speed: settings.sideback_speed, ..StepData::default() },
            pending_step: 0,
            roll_bearing: 0.0,
            roll_yaw: 0.0,
            settings,
            position: [0.0; 3],
            velocity: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            body_yaw: 0.0,
            state: State::Idle,
            previous_state: State::Idle,
            movement_direction: [0.0; 2],
            discrete_direction: 1.0,
            pose_level: 1.0,
            smoothed_pose_level: 1.0,
            pose_memo: 1.0,
            prone: false,
            reloading: false,
            hands_busy: false,
            inventory: false,
            prone_reload: false,
            turn_factor: 1.0,
            turn_limit: FREE_TURN_LIMIT,
            can_interact: true,
            pitch_limit: UPRIGHT_PITCH_RANGE,
            pitch_target: UPRIGHT_PITCH_RANGE,
            yaw_limit: None,
            step: 0,
            step_key: 0,
            lean_keys: [false; 2],
            last_slow_lean: false,
            slow_leaning: false,
            speed_memo,
            dropping_backpack: false,
            sidestep: 0.0,
            prone_sprint: false,
            surface_direction: [0.0; 3],
            surface_normal: [0.0, 1.0, 0.0],
            body_tilt: [0.0, 0.0, 0.0, 1.0],
            surface_pitch: 0.0,
            tilt: 0.0,
            smoothed_tilt: 0.0,
            tilt_multiplier: 0.0,
            prev_tilt: 0.0,
            sprinting: false,
            sprint_input: false,
            idle_sprint_held: false,
            aiming: false,
            aim_kept_from_body: false,
            aim_walk_bonus: 0.0,
            aiming_cap: AIMING_SPEED_LIMIT,
            was_sprinting: false,
            sprint_started: false,
            aim_lowered: false,
            aim_taken: false,
            hands_to_body: 0.0,
            aim_angle: 0.0,
            body_yaw_fine: 0.0,
            stamina,
            grounded: true,
            ground_check: GROUND_CHECK,
            freefall_time: 0.0,
            surface: [0.0, 1.0, 0.0],
            normal_history: slope::NormalHistory::default(),
            slope: [0.0; 3],
            slide: false,
            down_push: 0.0,
            yaw_before_turn: 0.0,
            head_lying: false,
            head_push: [0.0; 3],
            last_blend_velocity: [0.0; 3],
            velocity_before_limit: [0.0; 3],
            yaw_deltas: VecDeque::new(),
            direction: [0.0; 2],
            input_direction: [0.0; 2],
            frame_count: 0,
            sprint_dropped_in: 0,
            frame_dt: 0.0,
            stopwatch: Stopwatch::default(),
            unstepped: 0.0,
            last_player_dt: 0.0,
            clock_lag_before: 0.0,
            clock_lag: 0.0,
            mv: MoveData::fresh(),
            mv_prone: MoveData::fresh(),
            jump: JumpData::default(),
            body,
            inert: false,
            sprint_flag: false,
            jump_flag: false,
            landing_flag: false,
            blind_fire: 0,
            blind_shown: 0.0,
            blind_float: 0.0,
            blind_part: part::IN,
            blind_side: 1.0,
            blind_told: None,
            animated_motion: [0.0; 3],
            landing_idle_time: 0.0,
            landing_still: false,
            clock: 0.0,
            last_step: f32::NEG_INFINITY,
            step_side: 1.0,
            footstep: None,
            walk_stop: false,
            stepped: false,
            crawl_marks: 0,
            speed_set: true,
            landed: None,
            fly_from: 0.0,
            sounds,
            vaulting: Vaulting::default(),
            vault_time: 0.0,
            vault_surfaces: [crate::sound::steps::CONCRETE; 2],
            vault_key: false,
            vault_on: false,
            do_vault: false,
            do_climb: false,
            is_vaulting: false,
            // As the controller has them before anything is measured.
            vault_figures: [0.0, 1.0, 0.0, 0.0],
            vaulting_speed: 1.0,
            vault: VaultData::default(),
            vault_given: None,
            blocked: false,
            blocked_direction: [0.0; 2],
            slow_time: 0.0,
            real_speed: 0.0,
            place_before: [0.0; 3],
            body_height_offset: 0.0,
            height_before: 0.0,
            height_lag_velocity: [0.0],
            // The game starts this at zero and lets it drift to its resting
            // value over the first seconds of a raid; a character met any
            // later has it there.
            height_lag_time: HEIGHT_SMOOTH_TIME,
            height_lag_before: 0.0,
            approach_level: None,
            interactions: AnimatedInteractions::default(),
            shown_interaction: None,
            acting: None,
            kick: None,
            searching: None,
            acted: Vec::new(),
            hands_idle: true,
            body_inventory: [false; 2],
            inventory_reached: false,
            mount_settings: crate::weapons::mounting::Settings::default(),
            mount_weapon: None,
            mount_search: None,
            mount_search_fresh: false,
            mount: None,
            toggling_bipod: None,
            ignore_root_motion: false,
            weapon_tilt: 0.0,
            mount_events: Vec::new(),
            mount_last: None,
            rig_turn_before: [0.0, 0.0, 0.0, 1.0],
            left_stance: crate::weapons::left_stance::LeftStance::default(),
            left_stance_firearm: false,
            left_stance_overlap: false,
            left_stance_interaction: false,
            left_stance_overlap_clock: 0.0,
            left_stance_inventory_was: false,
        }
    }

    /// The selected speed after the active speed limits: the standing one
    /// from carried weight, the one aiming adds, and the one a landing
    /// leaves for a while.
    pub fn clamped_speed(&self) -> f32 {
        let mut limit = self.settings.state_speed_limit;
        if self.aiming {
            limit = limit.min(self.aiming_cap);
        }
        if let Some(landing) = self.landing_limit_in_effect {
            limit = limit.min(landing);
        }
        for held in self.held_limits_in_effect.into_iter().chain([self.shot_limit_in_effect]).flatten() {
            limit = limit.min(held);
        }
        self.character_speed.clamp(0.0, limit)
    }

    /// The highest speed setting the movement state allows just now
    /// (`MovementContext.StateSpeedLimit` with the limits in force): what
    /// the game's speed display lights its notches up to.
    pub fn speed_limit(&self) -> f32 {
        let mut limit = self.settings.state_speed_limit;
        if self.aiming {
            limit = limit.min(self.aiming_cap);
        }
        if let Some(landing) = self.landing_limit_in_effect {
            limit = limit.min(landing);
        }
        for held in self.held_limits_in_effect.into_iter().chain([self.shot_limit_in_effect]).flatten() {
            limit = limit.min(held);
        }
        limit.min(self.settings.max_speed)
    }

    /// What the character's health allows now (`Player.UpdateSpeedLimitByHealth`,
    /// `UpdateArmsCondition` and the effects' handlers setting the
    /// movement context's physical conditions): a damaged leg holds the
    /// speed down and ends a sprint, unless a painkiller is at work; the
    /// stamina is told what it goes by (stamina spec, "Who calls it").
    pub fn set_conditions(&mut self, conditions: crate::condition::health::Conditions) {
        if conditions == self.conditions {
            return;
        }
        let before = std::mem::replace(&mut self.conditions, conditions);
        let limit = conditions.speed_limit();
        if limit != self.held_limits[0] {
            self.held_limits[0] = limit;
            self.limits_changed = true;
        }
        self.stamina.health_lets_sprint = conditions.can_sprint_drain();
        if conditions.leg_damaged() && !conditions.on_painkillers {
            self.enable_sprint(false);
        }
        if (before.left_arm, before.right_arm, before.on_painkillers) != (conditions.left_arm, conditions.right_arm, conditions.on_painkillers) {
            self.stamina.set_aim_factor(conditions.aim_cost());
        }
    }

    /// Whether the character is in barbed wire (`BarbedWire`): `inside`,
    /// its capsule in the wire's box, holds the speed to 0.15 of the top
    /// while it is (`AddPenalty`, on the trigger's entering); `cut`, one of
    /// its hit colliders in the box, ends a sprint on every frame of it
    /// (`ProceedDamage`, which runs for the hit colliders alone: recorded,
    /// a sprint into wire ended ten frames after the speed was held down).
    pub fn set_in_barbed_wire(&mut self, inside: bool, cut: bool) {
        /// `BarbedWire._speedLimitPenalty`.
        const PENALTY: f32 = 0.15;
        let limit = inside.then_some(PENALTY);
        if limit != self.held_limits[1] {
            self.held_limits[1] = limit;
            self.limits_changed = true;
        }
        if cut && self.sprinting {
            self.enable_sprint(false);
        }
    }

    /// What the armour and the backpack worn take off the speed, in percent
    /// (`armor::speed_penalty`; `Player.UpdatePenalties`, `ESpeedLimit.Armor`):
    /// below nought, the speed is held to that share of the top while it
    /// is (the top as it stands when this is told; the game works it out
    /// whenever what is worn changes).
    pub fn set_armour_penalty(&mut self, percent: f32) {
        let limit = (percent < 0.0).then(|| (100.0 + percent) / 100.0 * self.settings.max_speed);
        if limit != self.held_limits[2] {
            self.held_limits[2] = limit;
            self.limits_changed = true;
        }
    }

    /// A hit taken, as its movement half (`Player.ApplyHitDebuff`, which
    /// every hit given to the player calls): `burn` the stamina it burns (the
    /// round's figure a point times the damage: `DamageInfo.StaminaBurnRate
    /// * Damage`). A weapon's hit (a bullet, a blow, a blunt one) caps the
    /// selected speed for 0.66 s, by the stamina burnt by the frame's hits
    /// together: none under 10, down to standing still at 55
    /// (`InverseLerp(55, 10, burnt)`; a cap already running keeps the lower
    /// and the later end), its burn comes off the legs' stamina at once
    /// (`Physical.BulletHit`; stamina spec, "A drain"), and a leg's hit
    /// without a painkiller ends a sprint (`Physical.Sprint(false)`).
    /// Movement spec, "Speed limits".
    /// PROVISIONAL: read from code; no recording has the player shot and
    /// living.
    pub fn hit(&mut self, burn: f32, part: crate::condition::health::Part, kind: crate::condition::health::DamageKind) {
        /// The burn that leaves the speed whole and the one that stops it.
        const BURN_RANGE: [f32; 2] = [55.0, 10.0];
        /// How long the cap lasts, in seconds.
        const SHOT_FOR: f32 = 0.66;
        if !kind.weapon() {
            return;
        }
        // The hit's burn comes off the legs (`Physical.BulletHit`).
        self.stamina.wound(burn, self.frame_clock);
        let burnt = if self.hits_burnt.0 == self.frame_count { self.hits_burnt.1 + burn } else { burn };
        self.hits_burnt = (self.frame_count, burnt);
        let share = inverse_lerp(BURN_RANGE[0], BURN_RANGE[1], burnt);
        if share < 1.0 {
            let (before, until) = self.shot_limit.unwrap_or((f32::MAX, 0.0));
            self.shot_limit = Some((share.min(before), (self.frame_clock + SHOT_FOR).max(until)));
            self.limits_changed = true;
        }
        if matches!(part, crate::condition::health::Part::LeftLeg | crate::condition::health::Part::RightLeg) && !self.conditions.on_painkillers {
            self.enable_sprint(false);
        }
    }

    /// What happened in the frame that health takes: falls, jumps, sprints
    /// and vaults (health spec, "Falls", "What health does to the
    /// character").
    pub fn take_moved(&mut self) -> Vec<Moved> {
        std::mem::take(&mut self.moved)
    }
}
