//! The fixed step, turning away from walls, overlaps and the sight line.

use super::*;

impl Hands {
    pub(super) fn outside(&self) -> bool {
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

    pub(super) fn fixed_step(&mut self, dt: f32, input: &HandsInput) {
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
        let pose = if input.prone {
            0
        } else if input.pose_level < 0.11 {
            1
        } else {
            2
        };
        // A change of stance works the sway's strengths out again
        // (`OnAimOrPoseChanged`).
        if std::mem::replace(&mut self.pose, pose) != pose {
            self.update_sway_factors();
        }
        self.view = [input.yaw, input.pitch];
        self.recoil.set_pose(self.pose);
        // Every change of the movement state into one other than the walk
        // rewinds the bob, whether the walk was the state left or not
        // (Idle to Sprint, Idle to Jump), and whether the bob runs or not
        // (the state's own handler turns it off: `WalkEffectorEnabled`): a
        // walk after it starts every curve from 0, not from the time shift
        // a new rig starts them at.
        if input.walk_stop || (self.walking && !input.walking) {
            self.rewind_walk();
        }
        self.walking = input.walking;

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
            // The mount's way has no walk's bob (`MountingStrategy`), nor
            // the hand shake, which comes after the walk where it runs
            // (`FirstPersonStrategy`). It moves the muzzle's rest point
            // from where the breath left it, and the breath's next step
            // goes on from there.
            if !self.mount_strategy {
                self.walk(dt, input);
                if self.hand_shake.is_on() {
                    let offset = self.recoil.hand_rotation.offset;
                    let shake = self.hand_shake.step(dt, input.wind.stamina, self.aiming, offset);
                    for (component, value) in shake.hands_rotation.into_iter().enumerate() {
                        self.hands_rotation.add(component, value);
                    }
                    self.recoil.hand_rotation.offset = shake.muzzle_offset;
                    self.breath.set_muzzle_offset(shake.muzzle_offset);
                }
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
    pub(super) fn turn_away(&mut self, dt: f32) {
        // A character seen from outside has no squeeze, the prefab's
        // second pair of figures for every component, and no camera.
        let outside = self.outside();
        let scale = if outside { 1.0 } else { self.fov_scale() };
        let d = &self.settings.data.turn_away;
        let away = &mut self.turn_away;
        (away.position, away.rotation, away.elbows, away.camera) = ([0.0; 3], [0.0; 3], [[0.0; 3]; 2], 0.0);
        // Against another character, by the prefab's own threshold for one
        // (`TurnAwayEffector.Process`).
        let mut wanted = away.depth / if away.with_character { d.player_threshold } else { d.threshold };
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
        self.set_overlap_met(depth, false);
    }

    /// `set_overlap`, and whether what the weapon met is another character
    /// (`Movement::placed_weapon_overlap_met`).
    pub fn set_overlap_met(&mut self, depth: f32, with_character: bool) {
        // From this depth on the effector is told how far back the
        // spring already holds the weapon.
        if depth >= OVERLAP_THAT_COUNTS {
            self.turn_away.held_back = self.hands_position.value()[1];
        }
        self.turn_away.depth = depth;
        self.turn_away.with_character = with_character;
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
    pub(super) fn line_in_force(&self, iron: Option<SightLine>) -> SightLine {
        match self.sights().and_then(crate::weapons::sights::Sights::launcher_line) {
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
        self.sights().is_none_or(crate::weapons::sights::Sights::shot_squeeze)
    }

    /// What the sights have told the battle screen since the last call.
    pub fn take_sight_hud(&mut self) -> Vec<crate::weapons::sights::SightHud> {
        self.settings.sights.as_mut().map(crate::weapons::sights::Sights::take_hud).unwrap_or_default()
    }

    /// The sights' line in force, and how far the weapon is turned for
    /// it now, in degrees.
    pub fn sight_line(&self) -> (SightLine, f32) {
        (self.sight, self.sight_angle)
    }
}
