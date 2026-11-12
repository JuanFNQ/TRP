//! Idle, sidestep, roll and prone updates, and aligning to the surface.

use super::*;

impl Movement {
    pub(super) fn update_idle(&mut self, dt: f32, world: &dyn World) {
        self.turn_body(dt);
        // Standing has no motion of its own, but while a walk blends out
        // the animator still supplies some; that is mixed with what
        // carries over from the last motion, and fades.
        let keep = self.settings.idle_motion_preservation;
        let motion = [0, 1, 2].map(|i| lerp(self.animated_motion[i], self.last_blend_velocity[i] * dt, keep));
        self.apply_motion(motion, dt, world);
        // A turn on the spot is the animation's own: its clip swings the
        // body a quarter turn.
        self.body_yaw += self.body.turn();
        self.note_rotation();
    }

    /// Stepped aside, or on the way there or back: standing's handling of
    /// the frame, then the step's playback speed and how far the body
    /// counts as stepped (movement spec, "The sidestep").
    pub(super) fn update_sidestep(&mut self, dt: f32, world: &dyn World) {
        self.update_idle(dt, world);
        self.sideback_speed = self.sidestep_data.play(&self.settings, dt);

        // The value for this frame comes from the run so far; the frame's
        // own time is added after.
        let data = &mut self.sidestep_data;
        let length = self.body.sidestep_length();
        let run = if length > 0.0 { (data.progress / length).clamp(0.0, 1.0) } else { 1.0 };
        self.sidestep = match data.part {
            part::IN => lerp(0.0, data.far, run),
            part::OUT => lerp(data.far, 0.0, run),
            _ => data.far,
        };
        data.progress += dt;
    }

    /// Rolled onto one side, or on the way there or back ("The roll" in
    /// the movement spec): the clip's motion as lying still takes it, the
    /// body's yaw eased to the bearing the view had when the roll began
    /// with whatever the clip turns the root this frame on top, and the
    /// roll's playback speed. The body is not laid to the ground meanwhile
    /// and does not follow the view.
    pub(super) fn update_roll(&mut self, dt: f32, world: &dyn World) {
        let motion = self.animated_motion;
        self.fall_and_move(motion, dt, world);
        self.roll_yaw = lerp_angle(self.roll_yaw, self.roll_bearing, ROLL_YAW_SPEED * dt);
        self.body_yaw = self.roll_yaw + self.body.turn();
        self.note_rotation();
        self.sideback_speed = self.roll_data.play(&self.settings, dt);
    }

    /// Going down: the clip's motion as it is, with none of the carry-over
    /// standing has.
    pub(super) fn update_to_prone(&mut self, dt: f32, world: &dyn World) {
        self.turn_body(dt);
        let motion = self.animated_motion;
        self.fall_and_move(motion, dt, world);
        self.body_yaw += self.body.turn();
        self.note_rotation();
        self.align_to_surface(dt, world);
    }

    /// Lying still.
    pub(super) fn update_prone_idle(&mut self, dt: f32, world: &dyn World) {
        let motion = self.animated_motion;
        self.fall_and_move(motion, dt, world);
        // Lying down the angle is taken afresh before the body follows.
        self.note_rotation();
        self.turn_body(dt);
        // The view may lead the body only so far, measured from where the
        // body lies now.
        self.yaw_limit = Some((self.body_yaw, PRONE_YAW_REACH));
        self.align_to_surface(dt, world);
        // A roll asked for goes to the animator once the lean has
        // straightened. That is here, after the animator has run, so the
        // roll starts a frame after the key.
        if self.pending_step != 0 && self.smoothed_tilt.abs() < LEAN_STRAIGHT_FOR_ROLL {
            self.step = self.pending_step;
        }
    }

    /// Crawling: the walking state with figures and fields of its own.
    pub(super) fn update_prone_move(&mut self, dt: f32, world: &dyn World) {
        std::mem::swap(&mut self.mv, &mut self.mv_prone);
        self.update_move(dt, world);
        std::mem::swap(&mut self.mv, &mut self.mv_prone);
        self.align_to_surface(dt, world);
        if !self.conditions.can_crawl() {
            self.inert = false;
        }
        // The sprint key held: the speed setting climbs a notch a frame
        // until it is at the top.
        if self.prone_sprint {
            self.select_speed(self.clamped_speed() + self.settings.delta_speed);
            if self.character_speed >= self.settings.max_speed.min(self.settings.state_speed_limit) {
                self.prone_sprint = false;
            }
        }
    }

    /// Lays the lying body to the ground ("Lying to the ground" in the
    /// movement spec): it is turned to point from the ground under it to
    /// the ground half a metre ahead, with the ground's normal as its up,
    /// both eased, and its roll is then limited.
    fn align_to_surface(&mut self, dt: f32, world: &dyn World) {
        /// How far ahead the second point of the ground is taken, from how
        /// many capsule heights up the ray for it starts, and how far it
        /// reaches. The ground check's own ray looks this much further
        /// than the check itself.
        const AHEAD: f32 = 0.5;
        const RAY_FROM: f32 = 1.5;
        const RAY_REACH: f32 = 2.0;
        const GROUND_RAY_BEYOND: f32 = 0.25;
        /// The steepest ground a body is laid to, and the most it rolls.
        const STEEPEST: f32 = 30.0;
        const MOST_ROLL: f32 = 8.0;

        let rate = dt * self.settings.prone_align_speed;
        let [x, y, z] = self.position;
        let facing = self.to_world([0.0, 1.0]);
        let forward = [facing[0], 0.0, facing[1]];
        let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let ease = |from: [f32; 3], to: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| lerp(from[i], to[i], rate)) };

        // The two points of the ground, where the rays reach them.
        let under = world.ground_height(x, z);
        let under = (y - under <= self.ground_check + GROUND_RAY_BEYOND).then_some([x, under, z]);
        let (ax, az) = (x + forward[0] * AHEAD, z + forward[2] * AHEAD);
        let from = y + self.capsule_height() * RAY_FROM;
        let ahead = world.ground_height(ax, az);
        let ahead = (ahead <= from && from - ahead <= RAY_REACH).then_some([ax, ahead, az]);
        let normal = world.ground_normal(x, z);
        let wanted = match (ahead, under) {
            (Some(ahead), Some(under)) => {
                let along: [f32; 3] = std::array::from_fn(|i| ahead[i] - under[i]);
                let len = (along[0] * along[0] + along[1] * along[1] + along[2] * along[2]).sqrt();
                along.map(|c| c / len)
            }
            // One point: the player's forward laid into its plane.
            _ => cross(normal, cross(forward, normal)),
        };
        let pointing = quat_rotate(quat_mul(quat_axis(1, self.body_yaw), self.body_tilt), [0.0, 0.0, 1.0]);
        self.surface_direction = ease(pointing, wanted);

        // The normal, no steeper than the limit.
        let lean = normal[1].clamp(-1.0, 1.0).acos().to_degrees();
        let normal = if lean >= STEEPEST {
            let level = length([normal[0], normal[2]]);
            let (sin, cos) = STEEPEST.to_radians().sin_cos();
            [normal[0] / level * sin, cos, normal[2] / level * sin]
        } else {
            normal
        };
        self.surface_normal = ease(self.surface_normal, normal);

        let Some(laid) = quat_look(self.surface_direction, self.surface_normal) else { return };
        let mut angles = quat_to_euler(quat_mul(quat_axis(1, -self.body_yaw), laid));
        // The engine gives these angles from 0 to 360, bar a hair below
        // zero, so a roll to the right reads as nearly a full turn and the
        // limit makes the most roll to the left of it.
        let roll = if angles[2] < -ENGINE_ANGLE_ALLOWANCE { angles[2] + 360.0 } else { angles[2] };
        angles[2] = roll.clamp(-MOST_ROLL, MOST_ROLL);
        self.body_tilt = quat_euler(angles);
        self.surface_pitch = delta_angle(0.0, angles[0]);
    }

    /// Getting up: nothing for the first part of the clip, then the body
    /// is let off the ground by the share of the clip that has played
    /// since. The roll is not limited here.
    pub(super) fn rise_from_surface(&mut self) {
        const HELD_FOR: f32 = 0.3;
        let Some(time) = self.body.time_in(movement_state::PRONE_TO_STAND) else { return };
        if time <= HELD_FOR {
            return;
        }
        let share = (time - HELD_FOR) / (1.0 - HELD_FOR);
        if share >= 1.0 {
            self.stand_upright();
            return;
        }
        let facing = self.to_world([0.0, 1.0]);
        let forward = [facing[0], 0.0, facing[1]];
        self.surface_direction = std::array::from_fn(|i| lerp(self.surface_direction[i], forward[i], share));
        self.surface_normal = std::array::from_fn(|i| lerp(self.surface_normal[i], [0.0, 1.0, 0.0][i], share));
        if let Some(laid) = quat_look(self.surface_direction, self.surface_normal) {
            self.body_tilt = quat_mul(quat_axis(1, -self.body_yaw), laid);
            self.surface_pitch = delta_angle(0.0, quat_to_euler(self.body_tilt)[0]);
        }
    }

    pub(super) fn stand_upright(&mut self) {
        self.surface_direction = [0.0; 3];
        self.surface_normal = [0.0, 1.0, 0.0];
        self.body_tilt = [0.0, 0.0, 0.0, 1.0];
        self.surface_pitch = 0.0;
    }
}
