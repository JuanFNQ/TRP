//! Pose changes, falls, landing limits, slope, tilt smoothing and body turn.

use super::*;

impl Movement {
    pub(super) fn change_pose(&mut self, delta: f32, world: &dyn World) {
        match self.state {
            State::Sprint => {
                // Crouching in a sprint is the slide: a full crouch at
                // once, the smoothed stance with it (`SetPoseLevel`'s
                // force), and the sprint let go of from the next frame on
                // (`SprintPlayerState.ChangePose`). The body plays its
                // crouched way out of the sprint, and the sprint itself is
                // switched off only as its state is left. Recorded
                // (courses8, `sprint-into-crouch`): the stance on the key's
                // frame, the stop on the next, the slide within 0.8 mm.
                if delta < 0.0 {
                    self.slide = true;
                    self.inert = false;
                    self.pose_level = 0.0;
                    self.smoothed_pose_level = 0.0;
                }
            }
            // In a vault or a climb the stance is not to be changed.
            State::Vault | State::Climb => {}
            // Lying still, any change of stance gets up into a full crouch;
            // crawling, into the stance held before.
            State::ProneIdle | State::Roll => {
                self.prone_command(world);
                // Not if the command was refused and the character lies on.
                if !self.prone {
                    self.pose_level = 0.0;
                    self.smoothed_pose_level = 0.0;
                }
            }
            State::ProneMove => self.prone_command(world),
            // Walking, a stance too low to sprint from ends the sprint's
            // wind-up.
            State::Move => {
                self.set_pose_level(self.pose_level + delta, world);
                if self.pose_level < SPRINT_POSE_THRESHOLD {
                    self.enable_sprint(false);
                }
            }
            // Everywhere else the stance changes and nothing more: in the
            // air too.
            // (Blind fire's too: recorded, crouched and stood up inside it.)
            State::Idle
            | State::BlindFire
            | State::Jump
            | State::SprintStop
            | State::Landing
            | State::ToProne
            | State::FromProne
            | State::Sidestep
            | State::VaultFall
            | State::VaultLanding
            | State::Approach
            | State::Breach => {
                self.set_pose_level(self.pose_level + delta, world);
            }
        }
    }

    pub(super) fn reset_speed_after_sprint(&mut self) {
        self.select_speed(self.settings.max_speed);
    }

    /// Follows the character's height while it is off the ground, and on
    /// the frame it finds the ground again charges it for the drop from the
    /// highest point: a fall of some height caps the selected speed, lower
    /// and for longer the further it was.
    fn watch_fall(&mut self) {
        let height = self.position[1];
        if self.grounded {
            let fallen = self.fall_from - height;
            self.fall_from = height;
            // Health is told the height on every frame on the ground
            // (`HandleFall`), which is 0 but on a landing's.
            if fallen > 0.0 {
                self.moved.push(Moved::Fell(fallen));
            }
            // The sounds hear a landing by both heights (`OnGrounded`).
            if !self.was_grounded {
                self.landed = Some((fallen, height - self.fly_from));
            }
            let s = &self.settings;
            if !self.was_grounded && fallen >= s.fall_threshold {
                let (limit, duration) = (s.fall_speed_limit.evaluate(fallen), s.fall_limit_duration.evaluate(fallen));
                if duration >= f32::EPSILON && limit < 1.0 {
                    self.limit_after_landing(limit, duration);
                }
            }
        } else {
            if self.was_grounded {
                self.fly_from = height;
            }
            if height > self.fall_from {
                self.fall_from = height;
            }
        }
        self.was_grounded = self.grounded;
    }

    /// Caps the selected speed for a time. With a cap already running the
    /// lower of the two and the later end are kept.
    pub(super) fn limit_after_landing(&mut self, limit: f32, duration: f32) {
        let (before, until) = self.landing_limit.unwrap_or((f32::MAX, 0.0));
        self.landing_limit = Some((limit.min(before), (self.frame_clock + duration).max(until)));
        self.limits_changed = true;
    }

    /// The game's once-a-frame look at its speed limits: a change made
    /// since the last look takes effect, and then a limit whose time is up
    /// is dropped, which is itself a change for the next look.
    pub(super) fn work_in_limits(&mut self) {
        if std::mem::take(&mut self.limits_changed) {
            self.landing_limit_in_effect = self.landing_limit.map(|(limit, _)| limit);
            self.shot_limit_in_effect = self.shot_limit.map(|(limit, _)| limit);
            self.held_limits_in_effect = self.held_limits;
        }
        if self.shot_limit.is_some_and(|(_, until)| self.frame_clock >= until) {
            self.shot_limit = None;
            self.limits_changed = true;
        }
        if self.landing_limit.is_some_and(|(_, until)| self.frame_clock >= until) {
            self.landing_limit = None;
            self.limits_changed = true;
        }
    }

    // ---------------------------------------------------- per-frame upkeep

    pub(super) fn manual_update(&mut self, dt: f32, world: &dyn World) {
        let ground = world.footing(self.position);
        // The check reaches down from the capsule, whose underside is at
        // the feet but in a vault or a climb. A character its controller
        // had to push up out of something is on the ground too, whatever
        // is under its axis: a foot that rests on a ledge's edge.
        let capsule = self.capsule();
        self.grounded =
            self.position[1] + capsule.underside - ground <= self.ground_check || world.held(self.position, capsule);
        self.feel_slope(world);
        self.watch_fall();
        self.smooth_pose(dt);
        if self.sprinting {
            self.pre_sprint_acceleration(dt);
        }
        self.work_in_limits();
        self.smooth_tilt(dt);
        // The view's pitch range closes in, or opens, a share at a time.
        let gap = length([self.pitch_limit[0] - self.pitch_target[0], self.pitch_limit[1] - self.pitch_target[1]]);
        self.pitch_limit = if gap < 0.1 {
            self.pitch_target
        } else {
            [0, 1].map(|i| lerp(self.pitch_limit[i], self.pitch_target[i], dt * PITCH_LIMIT_SPEED))
        };
    }

    /// The ground's slope, as the frame's ground check finds it
    /// (`UpdateGroundCollision`; movement spec, "Slopes and sliding"): the
    /// ray down the capsule's axis reports the normal of what it meets a
    /// quarter metre further than the check reaches, the capsule's contact
    /// is the host's, and the two give the frame's surface, its average
    /// and the slide's pull. Standing on another character (the game's
    /// `HumanPyramidSlopeLength`) is not built.
    fn feel_slope(&mut self, world: &dyn World) {
        let [x, y, z] = self.position;
        let capsule = self.capsule();
        let reached = y + capsule.underside - world.ground_height(x, z) <= self.ground_check + slope::GROUND_SEARCH;
        let hit = reached.then(|| world.ground_normal(x, z));
        self.surface = slope::surface_normal(hit, world.contact_normal(self.position, capsule));
        self.normal_history.add(self.surface[1]);
        self.slope = slope::slide(self.surface, self.normal_history.average());
    }

    fn smooth_pose(&mut self, dt: f32) {
        let s = &self.settings;
        let diff = self.pose_level - self.smoothed_pose_level;
        if diff == 0.0 {
            return;
        }
        if diff.abs() > 0.001 {
            // Spent legs change the stance as slowly as a character at
            // nine tenths overweight.
            let weight = s.overweight.max(if self.stamina.legs_spent() { 0.9 } else { 0.0 });
            let range = if diff > 0.0 { s.pose_increase_range } else { s.pose_decrease_range };
            let rate = lerp(range[0], range[1], weight);
            let before = self.smoothed_pose_level;
            self.smoothed_pose_level = lerp(before, self.pose_level, dt * s.pose_changing_speed * rate);
            // A rise is paid for, by the third of a notch.
            let rise = self.smoothed_pose_level - before;
            if rise > 0.0 {
                self.stamina.stance_raised(rise, self.frame_clock);
            }
        } else {
            self.smoothed_pose_level = self.pose_level;
        }
    }

    /// Winds the speed up to full before the sprint proper starts.
    fn pre_sprint_acceleration(&mut self, dt: f32) {
        if self.movement_direction[1] < 0.1 {
            self.enable_sprint(false);
        } else if self.smoothed_speed < 1.0 {
            self.smoothed_speed = (self.smoothed_speed + dt * self.settings.pre_sprint_acceleration).clamp(0.0, 1.0);
            self.character_speed = self.smoothed_speed;
        }
    }

    fn smooth_tilt(&mut self, dt: f32) {
        let diff = (self.smoothed_tilt - self.tilt).abs();
        if diff < f32::EPSILON {
            return;
        }
        // Mounted, the smoothed lean takes no value (`SmoothedTilt`'s setter)
        // and the weapon's motion reads the mount's camera lean.
        if let Some(camera) = self.mount_camera_tilt() {
            self.weapon_tilt = camera;
            return;
        }
        self.smooth_tilt_on(dt, diff);
        self.weapon_tilt = self.smoothed_tilt;
    }

    fn smooth_tilt_on(&mut self, dt: f32, diff: f32) {
        let s = &self.settings;
        // A lean made slowly is followed by the older rule
        // (`LegacySmoothTilt`): a share of what is left each frame.
        if self.slow_leaning {
            self.smoothed_tilt =
                if diff > 0.001 { self.smoothed_tilt + (self.tilt - self.smoothed_tilt) * (dt * s.tilt_changing_speed).clamp(0.0, 1.0) } else { self.tilt };
            return;
        }
        let max = s.tilt_max_speed;
        if self.prev_tilt != self.tilt {
            // The target changed: shed the current motion before adopting it.
            if self.tilt_multiplier.abs() <= s.tilt_inertia {
                self.prev_tilt = self.tilt;
                self.tilt_multiplier = 0.0;
            } else {
                self.tilt_multiplier =
                    (self.tilt_multiplier - s.tilt_inertia * sign(self.tilt_multiplier)).clamp(-max, max);
            }
        } else {
            let room = self.tilt_distance_to_stop(dt);
            self.tilt_multiplier = if self.tilt == 0.0 {
                self.tilt_multiplier - s.tilt_inertia * sign(room * self.smoothed_tilt) * s.inertia_backward_coef
            } else {
                self.tilt_multiplier + s.tilt_inertia * sign(room)
            }
            .clamp(-max, max);
        }
        let step = TILT_STEP * self.tilt_multiplier * sign(self.prev_tilt);
        if diff > (step * dt).abs() {
            self.smoothed_tilt = (self.smoothed_tilt + step * dt).clamp(-MAX_TILT, MAX_TILT);
        } else {
            self.smoothed_tilt = self.tilt;
            self.tilt_multiplier = 0.0;
        }
    }

    /// How much lean remains beyond what braking from the current rate would cover.
    fn tilt_distance_to_stop(&self, dt: f32) -> f32 {
        let s = &self.settings;
        let mut covered = 0.0;
        let mut m = self.tilt_multiplier;
        while m > 0.0 {
            covered += TILT_STEP * m * dt;
            m = (m - s.tilt_inertia).clamp(0.0, s.tilt_max_speed);
            if s.tilt_inertia <= 0.0 {
                break;
            }
        }
        (self.smoothed_tilt - self.tilt).abs() - covered + 0.1
    }

    // ------------------------------------------------------------- states

    /// Takes the angle from the body round to the view, and the view's
    /// pitch, as they now stand. The game does this whenever it turns the
    /// body, which is inside a state's update: what the animator and the
    /// next frame's follow see is therefore a frame behind the view.
    pub fn note_rotation(&mut self) {
        self.hands_to_body = body_to_view(self.body_yaw, self.yaw);
        self.aim_angle = self.pitch - self.surface_pitch;
    }

    /// The body turns only once the look direction has left its
    /// comfortable range, going by the angle as last noted.
    pub(super) fn turn_body(&mut self, dt: f32) {
        // Mounted, the body is the mount's (`ProcessRotation`).
        if self.mount.is_some() {
            return;
        }
        let angle = self.hands_to_body;
        let limit = self.trunk_limit;
        if angle.abs() > limit {
            let excess = delta_angle(sign(angle) * limit, angle);
            // The engine's blend of the two rotations, not a share of
            // the angle; the two part only for a large excess.
            let step = lerp_turn(0.0, excess, 30.0 * dt);
            // The game's body is a rotation, which takes a turn far
            // smaller than a bearing in single precision can show. Kept
            // as finely here, the following runs on until the angle
            // reads the limit itself, as the game's does, where a
            // bearing would stall a step short of it.
            if self.body_yaw_fine as f32 != self.body_yaw {
                self.body_yaw_fine = f64::from(self.body_yaw);
            }
            self.body_yaw_fine += f64::from(step);
            self.body_yaw = self.body_yaw_fine as f32;
            self.note_rotation();
        }
    }
}
