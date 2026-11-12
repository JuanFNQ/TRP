//! Forces, shakes and what feeds the hands: rig, head, health, load and handling.

use super::*;

impl Hands {
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
    pub(super) fn force_reaction(&mut self) {
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
        // A hand shake whose time is up stops before the frame's steps.
        self.hand_shake.pass_frame(dt);
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
    /// soften the sway spring, half the way for one and all of it for both,
    /// with the load's share added (`update_sway_spring`); a damaged leg
    /// without a painkiller plays the lame bob. The tremor and a broken
    /// arm's shake are the breathing's (`Wind`).
    ///
    /// And a firearm's controller listens to the arms itself
    /// (`FirearmController.PhysicalConditionChanged`): on a change of either
    /// arm's damage it works the weapon out again at once, on this call
    /// (`RecalculateErgonomic`, `UpdateWeaponVariables`): the ergonomics, 0
    /// with an arm damaged, and what goes by them (the ergonomic weight, the
    /// sway's and the displacement's strengths, the aiming speed, the aim
    /// sway's strength), ending in the recoil's rotation spring put back to
    /// nought (`weapon_parameters_changed`). The hip penalty is not worked
    /// out again (`UpdateHipInaccuracy` is not called). Whether it did: the
    /// host then takes the stamina's aiming weight again (`Physical.Aim` with
    /// the new ergonomic weight). Hands that hold no firearm only keep the
    /// arms, for the firearm taken in hand after them (`carry_on_from`).
    pub fn set_health(&mut self, conditions: crate::condition::health::Conditions) -> bool {
        self.arms_sway = conditions.sway_damage();
        self.update_sway_spring();
        self.lame = conditions.lame();
        let arms = [conditions.left_arm, conditions.right_arm];
        if arms == self.settings.arms_damaged {
            return false;
        }
        self.settings.arms_damaged = arms;
        if self.settings.recoil_force.is_none() {
            return false;
        }
        self.weapon_variables();
        self.weapon_parameters_changed();
        true
    }

    /// What goes by the weapon's weight and ergonomics worked out again
    /// (`UpdateWeaponVariables`): the sway's and the displacement's
    /// strengths, the aiming speed and the aim sway's strength, those set by
    /// hand kept (`ManualSetVariables`).
    pub(super) fn weapon_variables(&mut self) {
        (self.sway_strength, self.displacement_strength, self.aiming_speed) = self.settings.by_weight();
        self.aim_sway_strength = self.settings.aim_sway_strength();
        self.keep_manual();
    }

    /// The sway spring's two figures (`UpdateSwaySettings`): from the sound
    /// arms' toward the damaged ones by the arms' share and the load
    /// together, held within the two (`Mathf.Lerp`).
    pub(super) fn update_sway_spring(&mut self) {
        let s = &self.settings;
        let share = (self.arms_sway + s.overweight).clamp(0.0, 1.0);
        let (frequency, damping) = (lerp(s.sway_frequency, s.sway_damaged[0], share), lerp(s.sway_damping, s.sway_damaged[1], share));
        self.sway.set(frequency, damping);
    }

    /// The character's load (`Physical.Overweight`, 0 to 1) and the gear's
    /// share of the ergonomics (the armour's and backpack's
    /// `WeaponErgonomicPenalty` over 100), as the host has them (weapon
    /// handling spec, "The load", "The gear's share of the ergonomics"). A
    /// new load has the sway's and the displacement's strengths and the sway
    /// spring worked out again at once (`WeightRelatedValuesUpdated`); a new
    /// share, everything the weapon's weight and ergonomics give
    /// (`UpdateWeaponVariables`). The same figures again change nothing.
    pub fn set_load(&mut self, overweight: f32, gear_ergonomics: f32) {
        let overweight = overweight.clamp(0.0, 1.0);
        if gear_ergonomics != self.settings.gear_ergonomics {
            (self.settings.gear_ergonomics, self.settings.overweight) = (gear_ergonomics, overweight);
            (self.sway_strength, self.displacement_strength, self.aiming_speed) = self.settings.by_weight();
            self.aim_sway_strength = self.settings.aim_sway_strength();
            self.keep_manual();
            self.update_hip_inaccuracy(self.combo_on);
            self.update_sway_spring();
        } else if overweight != self.settings.overweight {
            self.settings.overweight = overweight;
            let (sway, displacement, _) = self.settings.by_weight();
            (self.sway_strength, self.displacement_strength) = (sway, displacement);
            self.update_sway_spring();
        }
    }

    /// The motion tied to the rig of a thing just taken in hand
    /// (`ProceduralWeaponAnimation.InitTransforms`, as each hands
    /// controller is made, and its `Reset`): the four springs put back where
    /// they rest (their place only, `Spring.Reset`: their speed and pushes
    /// go on), the turn-away's depth to nought, the reaction to the
    /// character's motion started afresh from where the rig is
    /// (`MotionEffector.Reset`, `ResetNow` at its next update), and the
    /// breath's and the walk's curves started again, their processors made
    /// anew (`BreathEffector.Initialize`, `WalkEffector.Initialize`: the
    /// steps' tempo is handed in, and goes on). The rest runs on
    /// (`carry_on_from`).
    pub fn tie_to_new_rig(&mut self) {
        self.breath.restart_loop();
        self.hand_shake.tie_to_new_rig();
        let d = &self.settings.data;
        self.walk_clocks = d.walk_normal.iter().map(|e| e.time_shift).collect();
        self.walk_clocks_lame = d.walk_lame.iter().map(|e| e.time_shift).collect();
        self.walk_clocks_load = d.walk_overweight.iter().map(|e| e.time_shift).collect();
        self.walk_clocks_load_lame = d.walk_overweight_lame.iter().map(|e| e.time_shift).collect();
        for spring in [&mut self.hands_position, &mut self.hands_rotation, &mut self.camera_rotation, &mut self.camera_position] {
            spring.current = [0.0; 3];
        }
        self.turn_away.depth = 0.0;
        (self.position_velocity, self.position_velocity_rate, self.position_acceleration, self.position_acceleration_rate, self.last_position_velocity) = ([0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3]);
        (self.rotation_velocity, self.rotation_velocity_rate, self.rotation_acceleration, self.rotation_acceleration_rate, self.last_rotation_velocity) = ([0.0; 2], [0.0; 2], [0.0; 2], [0.0; 2], [0.0; 2]);
        (self.rotation_velocity_sum, self.rotation_acceleration_sum, self.last_forward) = ([0.0; 2], [0.0; 2], None);
    }

    /// A quick draw that left the arms empty shakes the hands for
    /// `duration` seconds at `strength` (`StartHandShake`: the duration
    /// from `QuickDraw::shake_for`, the strength the settings'); one under
    /// way is begun again. A host starts it after the drawn weapon's rig is
    /// tied (`tie_to_new_rig`), as the game's draw does.
    pub fn start_hand_shake(&mut self, duration: f32, strength: f32) {
        self.hand_shake.start(duration, strength);
    }

    /// Whether a hand shake runs (the weapon motion's `HandShake` bit).
    pub fn hand_shake_on(&self) -> bool {
        self.hand_shake.is_on()
    }

    /// What this motion would leave to hands that take up a thing that is
    /// not a firearm next (`kept_from`).
    pub fn kept(&self) -> Kept {
        Kept {
            rotation_center: self.settings.rotation_center,
            rotation_center_no_stock: self.settings.rotation_center_no_stock,
            recoil_pivot: self.settings.recoil_pivot,
            hand_damping: self.hands_position.damping,
            ergonomic_weight: self.settings.ergonomic_weight(),
            sway_strength: self.sway_strength,
        }
    }

    /// Hands that hold no firearm (the empty hands, an item used, a
    /// grenade) take the motion as the last firearm left it
    /// (`ClearPreviousWeapon`; inventory spec, "Empty hands"): the game's
    /// motion is one object, told only to forget the weapon. What it is not
    /// told stays the last weapon's: the point the rig swings about (with
    /// its stock and without) and the recoil's pivot, the position spring's
    /// damping, the ergonomic weight and the sway's strength as last worked
    /// out (`MotionReact.SwayFactors`). The displacement's strength is put
    /// to 0.5, and both strengths are worked out again from the ergonomic
    /// weight left at the next change of the stance, the aim or the load
    /// (`OnAimOrPoseChanged`, `WeightRelatedValuesUpdated`). Rigs that are
    /// not a firearm's have no ergonomics of their own, so their ergonomic
    /// weight is their weight.
    pub fn keep_from(&mut self, kept: Kept) {
        self.settings.rotation_center = kept.rotation_center;
        self.settings.rotation_center_no_stock = kept.rotation_center_no_stock;
        self.settings.recoil_pivot = kept.recoil_pivot;
        self.settings.hand_damping = kept.hand_damping;
        self.hands_position.damping = kept.hand_damping;
        (self.settings.weapon_weight, self.settings.weapon_ergonomics) = (kept.ergonomic_weight, 0.0);
        self.sway_strength = kept.sway_strength;
        self.displacement_strength = CLEARED_STRENGTH;
    }

    /// The weapon in hand told its figures again (`ShotEffector.
    /// OnWeaponParametersChanged`, hooked to the weapon item's attributes:
    /// `RecalculateRecoilParamsOnChangeWeapon`), as every update of them
    /// does while it is held: a change of fire mode (`FireModeComponent.
    /// SetFireMode`, at once as the switch is taken) and a fold
    /// (`FoldableComponent.SetFolded`, as the fold's operation ends). Nothing
    /// in a reload's change of magazine updates them. The figures are the
    /// same; what shows is its end, the recoil's rotation spring put back to
    /// nought (`HandRotationRecoil.Reset`), and with it the breath's hip
    /// wander held in its offset.
    pub fn weapon_parameters_changed(&mut self) {
        self.recoil.hand_rotation.reset();
    }

    /// A firearm's controller made for this weapon in another thing's
    /// place, after `carry_on_from` (`FirearmController.Setup`): the
    /// recoil's springs run on and take this weapon's figures
    /// (`InitWeaponData`, `Recoil::take_weapon_figures`), and its hip
    /// penalty is worked out anew by its own total ergonomics and whether a
    /// combo unit on it is on (`UpdateHipInaccuracy` at the controller's
    /// making; `combo_on`, the host's word of the weapon's own devices: off
    /// where it has none, or the devices are switched off). Hands that hold
    /// no firearm keep the last one's figures instead, its hip penalty too
    /// (`keep_from`; only a firearm's controller writes the breath's).
    pub fn firearm_made(&mut self, combo_on: bool) {
        self.recoil.take_weapon_figures();
        self.update_hip_inaccuracy(combo_on);
    }

    /// An underbarrel launcher's switch over (`InitWeaponData` at its
    /// `LauncherAppeared` with the launcher, at its `LauncherDisappeared`
    /// with the rifle): the weapon's motion takes that side's handling
    /// (`WeaponHandling`), its strengths with the class skill's suppression
    /// as it stands (the rifle's buffs), the stock as it stands, and the
    /// recoil's springs run on with its figures, the rotation spring put back
    /// to nought (`OnWeaponParametersChanged`, `HandRotationRecoil.Reset`); a
    /// mount's figures are worked out again.
    pub fn take_handling(&mut self, handling: &WeaponHandling) {
        let settings = &mut self.settings;
        settings.recoil = handling.recoil.clone();
        settings.recoil_force = handling.recoil_force;
        settings.folded = handling.folded;
        settings.camera_snap = handling.camera_snap;
        settings.breath_gain = handling.breath_gain;
        settings.tactical_stiffness = handling.tactical_stiffness;
        settings.tactical_fixation = handling.tactical_fixation;
        let m = &mut settings.mount;
        [m.recoil_side, m.recoil_forward, m.camera_snap, m.return_speed, m.breath_side, m.breath_forward] = handling.mount;
        if let Some(force) = settings.recoil_force {
            (settings.recoil.rotation_strength, settings.recoil.position_strength) = force.strengths(force.parts, settings.recoil_suppression);
            if let (Some(hold), Some(parts)) = (settings.folded.as_mut(), force.folded_parts) {
                (hold.rotation_strength, hold.position_strength) = force.strengths(parts, settings.recoil_suppression);
            }
        }
        self.recoil.settings = self.settings.recoil.clone();
        if let Some(hold) = self.settings.folded.filter(|_| self.folded) {
            self.recoil.set_strengths(hold.rotation_strength, hold.position_strength);
        }
        self.recoil.take_weapon_figures();
        self.breath.set_gain(handling.breath_gain);
        self.mount_figures();
    }

    /// The sway's and the displacement's strengths as they stand.
    pub fn sway_strengths(&self) -> (f32, f32) {
        (self.sway_strength, self.displacement_strength)
    }

    /// The sway's and the displacement's strengths worked out again from the
    /// ergonomic weight and the load (`UpdateSwayFactors`), as every change
    /// of the stance or the aim does (`OnAimOrPoseChanged`).
    pub(super) fn update_sway_factors(&mut self) {
        let (sway, displacement, _) = self.settings.by_weight();
        (self.sway_strength, self.displacement_strength) = (sway, displacement);
    }

    pub fn set_systems(&mut self, systems: &crate::Systems) {
        (self.motion, self.recoil_on) = (systems.weapon_motion, systems.recoil);
    }

    pub fn point_of_view(&self) -> PointOfView {
        self.point_of_view
    }
}
