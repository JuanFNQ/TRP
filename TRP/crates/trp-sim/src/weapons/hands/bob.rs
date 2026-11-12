//! Tracking springs, the walk bob and the stiffened weapon bone.

use super::*;

impl Hands {
    /// Follows how the view turns and how the player moves.
    pub(super) fn track(&mut self, dt: f32, input: &HandsInput) {
        let d = &self.settings.data;
        let mut motion = input.motion;
        let cap = input.velocity[1].abs();
        motion[1] = motion[1].clamp(-cap, cap);
        // Both trackings go by the weapon's rig, which still points where
        // the frame before turned it.
        let [yaw, pitch] = self.rig_view.unwrap_or([input.yaw, input.pitch]);
        let local = to_view(motion, yaw, pitch);
        // The game smooths these two with the rendered frame's time, though
        // it does so once per physics step: above the physics rate the
        // smoothing runs slow. The turning terms below use the step's time.
        let frame = if input.frame_dt > 0.0 { input.frame_dt } else { dt };
        self.position_velocity = smooth_damp(
            self.position_velocity,
            local,
            &mut self.position_velocity_rate,
            d.movement.vel_smooth,
            f32::INFINITY,
            frame,
        );
        let change: [f32; 3] = std::array::from_fn(|i| self.position_velocity[i] - self.last_position_velocity[i]);
        self.position_acceleration = smooth_damp(
            self.position_acceleration,
            change,
            &mut self.position_acceleration_rate,
            d.movement.acc_smooth,
            f32::INFINITY,
            frame,
        );
        self.last_position_velocity = self.position_velocity;

        // How far the view's forward moved since the last step, seen from
        // the view: x is the turn to the right, y the tilt upward.
        let forward = from_view([0.0, 0.0, 1.0], yaw, pitch);
        let previous = to_view(self.last_forward.unwrap_or(forward), yaw, pitch);
        self.last_forward = Some(forward);
        let gyro = [
            -previous[0].atan2(previous[2]).to_degrees(),
            previous[2].atan2(previous[1]).to_degrees() - 90.0,
        ];
        let rate = [gyro[0] / dt, gyro[1] / dt];

        let clamp = d.rotation_input_clamp;
        self.rotation_velocity = self.rotation_velocity.map(|v| v.clamp(-clamp, clamp));
        for i in 0..2 {
            let change = self.rotation_velocity[i] - self.last_rotation_velocity[i];
            self.rotation_acceleration_sum[i] += (change - self.rotation_acceleration_sum[i]) / 3.0;
            self.rotation_velocity_sum[i] += (rate[i] - self.rotation_velocity_sum[i]) * 0.2;
        }
        self.last_rotation_velocity = self.rotation_velocity;
        self.rotation_velocity = smooth_damp(
            self.rotation_velocity,
            self.rotation_velocity_sum,
            &mut self.rotation_velocity_rate,
            d.mouse.vel_smooth,
            f32::INFINITY,
            dt,
        );
        self.rotation_acceleration = smooth_damp(
            self.rotation_acceleration,
            self.rotation_acceleration_sum,
            &mut self.rotation_acceleration_rate,
            d.mouse.acc_smooth,
            f32::INFINITY,
            dt,
        );
    }

    pub(super) fn source(&self, index: u8) -> f32 {
        match index {
            0 => self.rotation_velocity[0],
            1 => self.rotation_acceleration[0],
            2 => self.rotation_velocity[1],
            3 => self.rotation_acceleration[1],
            4..=6 => self.position_velocity[index as usize - 4],
            7..=9 => self.position_acceleration[index as usize - 7],
            _ => 0.0,
        }
    }

    pub(super) fn spring(&mut self, target: u8) -> &mut Spring {
        match target {
            0 => &mut self.camera_rotation,
            1 => &mut self.hands_position,
            _ => &mut self.hands_rotation,
        }
    }

    /// Feeds turning into the sway spring and movement into the others.
    pub(super) fn react(&mut self, dt: f32) {
        let movement: Vec<EffectorVal> = self.settings.data.movement.vals.clone();
        // Mounted forward on the bipod, the motion's reactions are a share
        // of themselves (`MotionEffector`: `MountingSwayFactorModifier`).
        let on_bipod = self.mounted.is_some_and(|m| !m.side) && self.bipod_used;
        let by = if on_bipod { self.settings.mount_settings.sway_with_bipod } else { 1.0 };
        let scale = dt * self.settings.data.motion_intensity * by;
        for val in movement {
            let force = self.source(val.source) * val.intensity * scale;
            self.spring(val.target).add(val.component as usize, force);
        }
        let s = self.sway_strength * self.intensity_by_aiming() * by;
        let factors = [s, if self.aiming { s * 0.3 } else { s }, s];
        let mouse: Vec<EffectorVal> = self.settings.data.mouse.vals.clone();
        for val in mouse {
            let shaped = self.settings.data.mouse.magnitude.evaluate(self.source(val.source));
            let component = val.component as usize;
            self.sway.kick(component, shaped * factors[component] * val.intensity * dt);
        }
    }

    /// The bob stopped (`WalkEffector.OnStop`, on every change of the
    /// movement state into one other than the walk): the current preset's
    /// curves, normal or lame, put back to their start, at 0 and not at
    /// their time shift (`AnimValProcessor.ResetTime`), and the load's of
    /// the same preset with them; the other preset's run on from where
    /// they were. A host whose character's past is not known calls it to
    /// start as after such a change (the replay).
    pub fn rewind_walk(&mut self) {
        let clocks = if self.lame { &mut self.walk_clocks_lame } else { &mut self.walk_clocks };
        clocks.iter_mut().for_each(|c| *c = 0.0);
        let clocks = if self.lame { &mut self.walk_clocks_load_lame } else { &mut self.walk_clocks_load };
        clocks.iter_mut().for_each(|c| *c = 0.0);
    }

    /// Where the current preset's curves of the bob are in their cycles,
    /// and the load's of the same preset (0 to 1 a cycle, unwrapped).
    pub fn walk_clocks(&self) -> (&[f32], &[f32]) {
        if self.lame {
            (&self.walk_clocks_lame, &self.walk_clocks_load_lame)
        } else {
            (&self.walk_clocks, &self.walk_clocks_load)
        }
    }

    /// The bob of each step, and the footsteps that set its tempo.
    pub(super) fn walk(&mut self, dt: f32, input: &HandsInput) {
        // Each footstep retunes the bob to the time since the one before.
        if let Some(gap) = input.footstep.filter(|gap| *gap < MAX_STEP_INTERVAL) {
            let (shortest, longest) =
                if input.sprinting { (0.3, 0.8) } else { (0.7 - input.speed / 2.0, MAX_STEP_INTERVAL) };
            self.step_frequency = 0.5 / gap.clamp(shortest, longest.max(shortest));
        }

        if !input.walking {
            return;
        }
        let range = self.settings.data.walk_intensity_range;
        let intensity = lerp(range[0], range[1], input.speed);
        let count = if self.lame { self.walk_clocks_lame.len() } else { self.walk_clocks.len() };
        for index in 0..count {
            let (clock, entry) = if self.lame {
                (&mut self.walk_clocks_lame[index], &self.settings.data.walk_lame[index])
            } else {
                (&mut self.walk_clocks[index], &self.settings.data.walk_normal[index])
            };
            // Each curve by its own wraps past its keys (`Curve.Evaluate`):
            // the lame walk's hands' pitch runs back and forth, the rest
            // start over.
            *clock += dt * self.step_frequency;
            let value = entry.at(*clock) * intensity;
            if value.abs() < 1e-6 {
                continue;
            }
            let vals: Vec<EffectorVal> = entry.vals.clone();
            let head_bobbing = self.settings.head_bobbing;
            for val in vals {
                // What the bob does to the camera is scaled by the player's setting.
                let setting = if val.target == 0 { head_bobbing } else { 1.0 };
                self.spring(val.target).add(val.component as usize, value * val.intensity * setting);
            }
        }
        // The load's own bob after it, on clocks of its own at the same
        // tempo, by the walk's share of the load (`WalkEffector.Process`).
        let share = self.settings.load_walk_share();
        if share <= 0.0 {
            return;
        }
        let count = if self.lame { self.walk_clocks_load_lame.len() } else { self.walk_clocks_load.len() };
        for index in 0..count {
            let (clock, entry) = if self.lame {
                (&mut self.walk_clocks_load_lame[index], &self.settings.data.walk_overweight_lame[index])
            } else {
                (&mut self.walk_clocks_load[index], &self.settings.data.walk_overweight[index])
            };
            *clock += dt * self.step_frequency;
            let value = entry.at(*clock) * share;
            if value.abs() < 1e-6 {
                continue;
            }
            let vals: Vec<EffectorVal> = entry.vals.clone();
            let head_bobbing = self.settings.head_bobbing;
            for val in vals {
                let setting = if val.target == 0 { head_bobbing } else { 1.0 };
                self.spring(val.target).add(val.component as usize, value * val.intensity * setting);
            }
        }
    }

    /// The tactical reload's blend for this frame (`Firearm::tactical_reload`),
    /// which the weapon bone's stiffening and the camera's share of the clip's
    /// turn go by. The host sets it before posing the rig.
    pub fn set_tactical_reload(&mut self, blend: f32) {
        self.tactical_blend = blend.clamp(0.0, 1.0);
    }

    /// The `weapon` bone in its parent (the bone the procedural motion moves)
    /// as the tactical reload's blend holds it toward its rest, after the
    /// clip has posed it (`ApplyTacticalReloadTransformations`; firing spec,
    /// "The reload with the sights up"; mounting spec, "The weapon's motion
    /// while mounted"). Its turn about x and z is cut by the stiffness's x
    /// and z times the blend, and its place drawn toward nought by the
    /// fixation times the blend; its turn about y, cut by the stiffness's y
    /// times the blend, is put back about the parent's up through a point at
    /// the blend's share of the pivot's depth (the recoil pivot's, or the
    /// mount's while mounted: its own, or the bipod's forward with it out),
    /// which carries the bone aside; of that place, the blend's share is
    /// taken. The angles are the turn's as the game reads them (`eulerAngles`,
    /// each within a half turn). As it was: nothing while the blend is nought.
    pub fn stiffen_weapon_bone(&self, (at, turn): ([f32; 3], [f32; 4])) -> ([f32; 3], [f32; 4]) {
        let blend = self.tactical_blend;
        if blend <= f32::EPSILON {
            return (at, turn);
        }
        let s = &self.settings;
        let [x, y, z] = quat_to_euler(turn);
        let x = lerp(x, 0.0, s.tactical_stiffness[0] * blend);
        let angle = lerp(y, 0.0, s.tactical_stiffness[1] * blend);
        let z = lerp(z, 0.0, s.tactical_stiffness[2] * blend);
        let fixed: [f32; 3] = std::array::from_fn(|i| lerp(at[i], 0.0, s.tactical_fixation * blend));
        let depth = self.mounted.map_or(s.recoil_pivot, |mount| mount.pivot)[2];
        let point = [0.0, 0.0, depth * blend];
        let about = quat_axis(1, angle);
        let swung = quat_rotate(about, std::array::from_fn(|i| fixed[i] - point[i]));
        let turned_at: [f32; 3] = std::array::from_fn(|i| point[i] + swung[i]);
        (std::array::from_fn(|i| lerp(at[i], turned_at[i], blend)), quat_mul(about, quat_euler([x, 0.0, z])))
    }
}
