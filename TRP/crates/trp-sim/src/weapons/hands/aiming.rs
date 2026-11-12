//! Pose blends, breath, shots and aiming.

use super::*;

impl Hands {
    /// Moves the held poses on by one rendered frame of `frame_dt` seconds
    /// and puts what they come to into the springs' rests; eases the iron
    /// sights' line and a canted sight's turn. A host calls it once a
    /// frame, after the frame's physics steps and before it reads the
    /// weapon's transform or the camera. On a frame with no physics step
    /// the time is the player's own (`frame_on_clock`'s `arms_dt`: the game
    /// gives its weapon motion `Player._armsTime`, the frame's time read in
    /// whole milliseconds), which a host with one clock takes for its
    /// frame's.
    pub fn blend_poses(&mut self, frame_dt: f32) {
        // A weapon with both points of an iron sights' line is turned
        // toward levelling it while the sights are up, and back when they
        // are down, by a share of what is left each frame.
        let launcher = self.sights().is_some_and(crate::weapons::sights::Sights::launcher);
        if self.sight_noted.is_some() || launcher || self.sight_angle != 0.0 {
            let steps = self.steps_since_frame;
            let dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { frame_dt };
            let goal = if self.aiming { self.sight.angle } else { 0.0 };
            self.sight_angle = lerp(self.sight_angle, goal, (dt * SIGHT_LINE_RATE).clamp(0.0, 1.0));
        }
        // A canted sight: the weapon turned about its barrel (its bone's
        // y) by the sight's own turn while aiming, and back while not, by
        // a share `CameraSmoothTime * aiming speed * dt` of what is left,
        // in the same late update and with the same time as the line
        // above (`ApplyComplexRotation`, `UpdateCurrentScopeRotation`).
        let degrees = self.sights().map_or(0.0, crate::weapons::sights::Sights::scope_rotation);
        if degrees != 0.0 || self.scope_rotation != [0.0, 0.0, 0.0, 1.0] {
            let steps = self.steps_since_frame;
            let dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { frame_dt };
            let goal = if self.aiming { quat_axis(1, degrees) } else { [0.0, 0.0, 0.0, 1.0] };
            self.scope_rotation = quat_lerp(self.scope_rotation, goal, self.settings.data.camera_smooth_time * self.aiming_speed * dt);
        }
        if self.poses.is_empty() && self.pose_position == [0.0; 3] && self.pose_rotation == [0.0; 3] {
            return;
        }
        // The time the camera gets: the physics time on a frame that had
        // a physics step, the frame's own otherwise.
        let steps = self.steps_since_frame;
        let mut dt = if steps > 0 { steps as f32 * self.settings.fixed_timestep } else { frame_dt };
        let (mut position, mut rotation, mut camera) = ([0.0f32; 3], [0.0f32; 3], [0.0f32; 3]);
        // From the newest back, the time scaled once more for each: the
        // game's own way of counting.
        let mut index = self.poses.len();
        while index > 0 {
            index -= 1;
            dt *= self.weapon_flip_speed;
            let (pose, clock, coming_in) = &mut self.poses[index];
            *clock += if *coming_in { dt } else { -dt };
            if *clock < 0.0 {
                self.poses.remove(index);
                continue;
            }
            let share = pose.blend.evaluate(*clock);
            for k in 0..3 {
                position[k] += pose.position[k] * share;
                rotation[k] += pose.rotation[k] * share;
            }
            *clock = clock.min(pose.blend.keys.last().map_or(0.0, |key| key.time));
        }
        // The camera takes its share when its target is worked out, from
        // the clocks as that left them.
        for (pose, clock, _) in &self.poses {
            let share = pose.blend.evaluate(*clock);
            for k in 0..3 {
                camera[k] += pose.camera_rotation[k] * share;
            }
        }
        (self.pose_position, self.pose_rotation, self.pose_camera) = (position, rotation, camera);
        self.set_rests();
    }


    /// 1 from the hip, less while aiming.
    pub fn intensity_by_aiming(&self) -> f32 {
        if self.aiming {
            self.settings.aim_intensity
        } else {
            1.0
        }
    }

    /// Scale of breathing: the stance's factor times the aiming factor.
    pub(super) fn breath_intensity(&self) -> f32 {
        self.settings.intensity_by_pose[self.pose] * self.intensity_by_aiming()
    }

    /// Puts the breath where a recording has it, for a host that replays
    /// one. The breath is a loop of a few seconds; hands made fresh start
    /// at its beginning, a recorded character is somewhere in it, and the
    /// weapon rises and falls some millimetres with it (and the line that
    /// feels for obstacles starts from the weapon). `lifted` is how far up
    /// the position spring held the weapon on the recording's first physics
    /// steps, one figure a step, with the character still and the sights
    /// down; `pose` is the stance as the weapon's motion counts it (0
    /// lying, 1 crouched, 2 standing). The breath and the three springs it
    /// feeds are left where a character breathing since long before would
    /// have them at the point of the loop that fits those figures best.
    pub fn start_breath(&mut self, pose: usize, lifted: &[f32]) {
        let step = self.settings.fixed_timestep;
        let round = (self.breath.period() / step).ceil() as usize;
        if lifted.is_empty() || round == 0 {
            return;
        }
        let intensity = self.settings.intensity_by_pose[pose.min(2)];
        let head_bobbing = self.settings.head_bobbing;
        let mut breath = self.breath.clone();
        let mut springs = [self.camera_rotation.clone(), self.hands_position.clone(), self.hands_rotation.clone()];
        springs[1].input_intensity = 1.0;
        springs[2].input_intensity = 1.0;
        let mut time = self.time;
        // Once round to settle, then once round with each point kept.
        let mut kept = Vec::with_capacity(round + lifted.len());
        for index in 0..2 * round + lifted.len() {
            time += step;
            for (target, component, value) in breath.step(step, time, intensity, false, head_bobbing, Wind::default()).pushes {
                springs[usize::from(target.min(2))].add(component as usize, value);
            }
            for spring in &mut springs {
                spring.step();
            }
            if index >= round {
                kept.push((breath.clone(), springs.clone(), time));
            }
        }
        let miss = |from: usize| -> f32 {
            lifted.iter().zip(&kept[from..]).map(|(wanted, (_, springs, _))| (springs[1].value()[2] - wanted).powi(2)).sum()
        };
        let Some(best) = (0..round).min_by(|a, b| miss(*a).total_cmp(&miss(*b))) else { return };
        let (breath, [camera_rotation, hands_position, hands_rotation], time) = kept.swap_remove(best);
        (self.breath, self.time) = (breath, time);
        (self.camera_rotation, self.hands_position, self.hands_rotation) = (camera_rotation, hands_position, hands_rotation);
    }

    /// A shot leaves the barrel. `strength` is 1 plus the cartridge's
    /// recoil percentage.
    ///
    /// It is felt at the end of the frame's late update (`frame_on_clock`,
    /// `frame_outside`), after the weapon and the camera are placed: the
    /// game's firearm controller sets it off in its own late update, which
    /// the player runs after the procedural motion's
    /// (`Player.LateUpdate`: `ProceduralWeaponAnimation.LateTransformations`,
    /// then `HandsController.ManualLateUpdate`, which calls `Shoot`). So the
    /// weapon's own curves start again, and the camera's rate drops, from
    /// the frame after the shot's; the springs feel it on their next step
    /// either way. With no frame between, the next step feels it first.
    pub fn shoot(&mut self, strength: f32) {
        self.shots_pending.push(strength);
    }

    /// The shots fired and not yet felt (`ProceduralWeaponAnimation.Shoot`).
    pub(super) fn feel_shots(&mut self) {
        for strength in std::mem::take(&mut self.shots_pending) {
            // A character whose mask has no shot feels none (a bot's), and
            // none is felt with the recoil switched off.
            if self.mask & effector::SHOOTING == 0 || !self.recoil_on {
                continue;
            }
            // Mounted, the camera's snap is the mount's share of itself.
            let snap = match self.mounted {
                Some(mount) => self.settings.mount.motion(&self.settings.mount_settings, mount.side, self.bipod_used).camera_snap,
                None => 1.0,
            };
            self.camera_smooth.jump(self.settings.camera_snap * snap, CAMERA_SNAP_DELAY, self.time);
            self.recoil.shoot(strength);
            // The breath is told of a shot for the character's own eyes only,
            // and none from the hip on a mount.
            if !self.outside() {
                self.breath.on_shot(self.time, self.breath_intensity(), self.aiming || self.mounted.is_some());
            }
        }
    }

    /// Whether the weapon is in a run of automatic fire.
    pub fn set_auto_fire(&mut self, on: bool) {
        self.recoil.set_auto_fire(on);
    }

    /// Rate per second at which the camera closes on the sight line while
    /// aiming, and the aim's weight with it. It drops after each shot and
    /// recovers, and the load slows it (weapon handling spec, "Aiming speed").
    pub fn camera_aim_rate(&self) -> f32 {
        self.aiming_speed * self.camera_smooth.value(self.time) * self.settings.load_aiming_share()
    }

    /// The sights went up or down as this frame began, before its physics
    /// steps: the weapon's motion takes it at once, as the game's does (its
    /// controller sets `ProceduralWeaponAnimation.IsAiming` as it changes
    /// its aim, in the frame's update), so the late update of a frame with
    /// no physics step already goes by it. Left to the next step's input,
    /// the camera's ease and a canted sight's turn began a frame late on
    /// such a frame (`courses4`'s M4A1, its sights up at 0.5 s). The field
    /// of view starts toward its new figure here too, and takes its first
    /// turn in this frame's `frame`.
    pub fn aim_changed(&mut self, aiming: bool) {
        if aiming != self.aiming {
            self.set_aiming(aiming);
        }
    }

    /// The sights have gone up or down: the field of view starts toward
    /// its new figure, and what goes by the sights does so from here on.
    pub(super) fn set_aiming(&mut self, aiming: bool) {
        // The sights' line is measured as they come up, and dropped as
        // they go down.
        // (`IsAiming`, as it changes: a launcher's line measured, the shot's
        // squeeze as the sight has it, the zeroing's calibration asked for.
        // This is called on every physics step, changed or not.)
        if aiming != self.aiming {
            if let Some(sights) = self.settings.sights.as_mut() {
                sights.aim_changed(aiming);
            }
            // And so does a change of aim (`OnAimOrPoseChanged`).
            self.update_sway_factors();
            // The stiff draw holds the breath as the sights come up.
            if aiming {
                self.breath.sights_up();
            }
            // A heavy weapon's sway drawn afresh and whole as the sights
            // come up (`IsAiming`: three of Unity's `Random.Range`, x, y, z,
            // in the prefab's box, times the strength and the stance's
            // intensity).
            if aiming && self.aim_sway_strength > 0.0 {
                let a = self.settings.data.aim_sway;
                let k = self.aim_sway_strength * self.settings.intensity_by_pose[self.pose];
                let random = &mut self.aim_sway_random;
                let drawn: [f32; 3] = std::array::from_fn(|i| random.range(a.min[i], a.max[i]) * k);
                self.aim_sway_direction = self.aim_sway_override.unwrap_or(drawn);
                self.aim_sway_since = 0.0;
            }
        }
        self.sight = if aiming { self.line_in_force(self.sight_noted) } else { SightLine::default() };
        // While the mouse has the head the sights going up or down leave
        // the field of view alone (`OnAimOrPoseChanged`).
        if aiming != self.aiming && !self.head_control {
            let fov = self.settings.field_of_view;
            self.set_fov(if aiming { self.aimed_fov() } else { fov }, 1.0);
        }
        self.aiming = aiming;
    }

    /// The sights went up or down after the frame's `frame`: the change an
    /// obstacle makes at the end of a frame (`Firearm::overlap`). The game
    /// tells its weapon motion at that moment, and the field of view's
    /// easing takes its first turn there and then, which is the one that
    /// changes nothing yet; the next frame's is the first to show. Left to
    /// the next physics step, the camera and the field of view start a
    /// frame or two late. `frame_dt` is the length of the frame just
    /// drawn. (A change the aim key makes comes before the frame's steps
    /// and needs no call: the step's input carries it.)
    pub fn aim_changed_after_frame(&mut self, aiming: bool, frame_dt: f32) {
        if aiming != self.aiming {
            self.set_aiming(aiming);
            self.field_of_view(frame_dt);
            // A stiff draw begun here began on this frame's clock, which
            // the next frame's steps read a frame on.
            if aiming && self.breath.stiff_draw() {
                self.breath.pass_frame(frame_dt);
            }
        }
    }
}
