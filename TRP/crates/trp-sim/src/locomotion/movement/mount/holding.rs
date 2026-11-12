//! Staying mounted: the update, the approach, the body, looking and rotating, and drain.

use super::*;

impl Movement {
    /// The mount's frame (`IdleWeaponMountingState.ManualAnimatorMoveUpdate`),
    /// after each of the movement state's own updates in the frame.
    /// `before` is where the character stood before the movement state's
    /// update moved it: the game moves by the last motion asked for in a
    /// frame, so an approach's takes the place of the state's.
    pub(crate) fn update_mount(&mut self, dt: f32, world: &dyn World, before: [f32; 3]) {
        let Some(m) = self.mount else { return };
        let s = self.mount_settings;
        if m.exiting {
            if m.timer > s.exit_time {
                self.leave_mount_now();
                return;
            }
            let Some(m) = self.mount.as_mut() else { return };
            if m.timer <= s.exit_time {
                m.timer += dt;
                m.progress = 1.0 - m.timer / s.exit_time;
            }
        } else if m.timer <= m.data.approach {
            self.approach_mount(dt, world, before);
        } else if squared(sub3(self.position, m.data.target)) > mounting::reach_squared().0 {
            self.start_leaving_mount();
        } else {
            let l = m.data.yaw_limit;
            self.yaw_limit = Some(((l[0] + l[1]) / 2.0, (l[1] - l[0]) / 2.0));
            self.pitch_target = [m.data.pitch_limit[0], m.down_limit];
            self.mounted_body(world);
        }
    }

    /// `TryApproachTargetPosition`. Where the character is, it reads before
    /// the frame's scheduled motion (`before`).
    fn approach_mount(&mut self, dt: f32, world: &dyn World, before: [f32; 3]) {
        let Some(mut m) = self.mount else { return };
        self.ignore_root_motion = true;
        m.timer += dt;
        let t = m.timer / m.data.approach;
        m.progress = t;
        m.yaw_offset = lerp_angle(0.0, m.yaw_shift, t);
        let x = lerp_angle(m.start_look[0], m.target_yaw, t);
        let y = lerp_angle(m.start_look[1], m.target_pitch, t);
        self.body_yaw = lerp_turn(m.start_body, m.data.body_yaw, t);
        // `Vector2.Lerp` of the two looks, then the view's limits.
        let look = [lerp(m.start_look[0], x, t), lerp(m.start_look[1], y, t)];
        let range = self.pitch_limit.map(|limit| limit + self.surface_pitch);
        self.yaw = look[0];
        self.pitch = look[1].clamp(range[0], range[1]);
        self.note_rotation();
        if !self.prone {
            let tilt = lerp(m.entry_tilt, m.target_tilt, t);
            self.tilt = tilt.clamp(-MAX_TILT, MAX_TILT);
            m.mounted_tilt = tilt;
            if m.side != 0 {
                m.camera_tilt = tilt;
                m.body_tilt = Some(tilt);
                m.drop = mounting::SIDE_DROP * m.yaw_offset.abs() / m.span;
            }
        }
        self.mount = Some(m);
        let along: [f32; 3] = std::array::from_fn(|k| lerp(m.start_position[k], m.data.target[k], t));
        let motion = [along[0] - before[0], 0.0, along[2] - before[2]];
        let off = [before[0] - m.data.target[0], 0.0, before[2] - m.data.target[2]];
        if squared(motion) > 0.0 {
            if self.walk_refused() {
                self.leave_mount_now();
                return;
            }
            // The state's own motion of this frame is replaced.
            self.position = before;
            self.apply_motion(motion, dt, world);
            let size = squared(motion).sqrt();
            self.movement_direction = self.to_local([motion[0] / size, motion[2] / size]);
            self.inert = squared(off) > mounting::reach_squared().1;
        }
        if m.timer > m.data.approach {
            if squared(off) > mounting::reach_squared().0 {
                self.leave_mount_now();
                return;
            }
            self.position = m.data.target;
            self.apply_motion([0.0; 3], dt, world);
            self.inert = false;
            if let Some(m) = self.mount.as_mut() {
                m.transition = false;
            }
        }
    }

    /// `UpdatePlayerBody`: the lean and the stance by the yaw and the pitch,
    /// and a side mount's drop.
    fn mounted_body(&mut self, world: &dyn World) {
        let Some(m) = self.mount else { return };
        let share = m.yaw_offset.abs() / m.span;
        if !self.prone {
            // `UpdateCurrentTilt`.
            let side = m.side as f32;
            if side * m.yaw_offset < 0.0 || m.side == 0 {
                let toward = if side * m.yaw_offset < 0.0 { side } else { -m.yaw_offset };
                let tilt = sign(toward) * lerp(0.0, MAX_TILT, share);
                self.tilt = tilt.clamp(-MAX_TILT, MAX_TILT);
                if let Some(m) = self.mount.as_mut() {
                    m.mounted_tilt = tilt;
                    if m.side != 0 {
                        m.camera_tilt = tilt;
                        m.body_tilt = Some(tilt);
                    }
                }
            }
            // `UpdatePoseLevel`.
            let up = m.data.pitch_limit[0];
            let level = lerp(m.data.pose_limit[0], m.data.pose_limit[1], inverse_lerp(up, m.down_limit, self.pitch));
            self.set_pose_level(level, world);
        }
        if m.side != 0 {
            if let Some(m) = self.mount.as_mut() {
                m.drop = mounting::SIDE_DROP * share;
            }
        }
    }

    /// Where the mount's turn (`mount_rotate`) will put the view for a turn
    /// still to come, without turning it: what a host draws ahead of the
    /// frame. The same lower limit by the weapon's depth in what it rests on,
    /// the same clamp of the turn and the same limits; the head's room is not
    /// felt (the frame may still refuse an outward turn there). `None` when
    /// not mounted.
    pub(crate) fn mount_look_ahead(&self, delta: [f32; 2], overlap_depth: f32) -> Option<[f32; 2]> {
        let m = self.mount.as_ref()?;
        if m.timer <= m.data.approach || m.exiting {
            return Some([self.yaw, self.pitch]);
        }
        let s = self.mount_settings;
        let mut limit = self.pitch_limit;
        if m.side == 0 {
            let down = if overlap_depth >= mounting::down_limit_overlap() { self.pitch } else { m.data.pitch_limit[1] };
            limit = [m.data.pitch_limit[0], down];
        }
        let mut delta = delta;
        if s.turn_clamp > 0.0 {
            delta = delta.map(|d| d * s.sensitivity);
            let size = length(delta);
            if size > s.turn_clamp {
                delta = delta.map(|d| d * s.turn_clamp / size);
            }
        }
        let mut yaw = self.yaw + delta[0];
        if let Some((centre, reach)) = self.yaw_limit {
            yaw = centre + delta_angle(centre, yaw).clamp(-reach, reach);
        }
        let range = limit.map(|l| l + self.surface_pitch);
        Some([yaw, (self.pitch + delta[1]).clamp(range[0], range[1])])
    }

    /// The turn of the view while mounted (`IdleWeaponMountingState.Rotate`),
    /// in place of the movement state's. Nothing turns in the approach or
    /// the exit.
    pub(crate) fn mount_rotate(&mut self, delta: [f32; 2], overlap_depth: f32, world: &dyn World) {
        let Some(mut m) = self.mount else { return };
        if m.timer <= m.data.approach || m.exiting {
            return;
        }
        let s = self.mount_settings;
        let up = m.data.pitch_limit[0];
        if m.side == 0 {
            // `CalculateDynamicDownLimit`, then the limits at once.
            m.down_limit = if overlap_depth >= mounting::down_limit_overlap() { self.pitch } else { m.data.pitch_limit[1] };
            self.pitch_limit = [up, m.down_limit];
        }
        // `ClampRotation`: the state's sensitivity, then cut to its clamp.
        let mut delta = delta;
        if s.turn_clamp > 0.0 {
            delta = delta.map(|d| d * s.sensitivity);
            let size = length(delta);
            if size > s.turn_clamp {
                delta = delta.map(|d| d * s.turn_clamp / size);
            }
        }
        let (low, high) = match m.side {
            0 => (-m.span, m.span),
            side if side > 0 => (-m.span, 0.0),
            _ => (0.0, m.span),
        };
        let before = m.yaw_offset;
        m.yaw_offset = (before + delta[0]).clamp(low, high);
        if m.yaw_offset.abs() > before.abs() {
            let share = m.yaw_offset.abs() / m.span;
            let side = m.side as f32;
            let toward = if side * m.yaw_offset < 0.0 { side } else if m.side == 0 { sign(-m.yaw_offset) } else { 0.0 };
            let right = quat_rotate(quat_axis(1, self.body_yaw), [1.0, 0.0, 0.0]);
            let shift = right.map(|r| r * toward * share * mounting::TILT_POSITION_OFFSET);
            // The head's capsule carried by the turn and the shift is to
            // meet nothing at all (`RotationOverlapPrediction`, compared
            // with nothing exactly); settings made without the prefab's
            // capsule ask the host about the character's own instead.
            let blocked = if self.settings.head.is_some() {
                self.head_turn_push(shift, 2.0 * delta[0], world) != [0.0; 3]
            } else {
                world.turn_blocked(self.position, shift, 2.0 * delta[0], self.capsule())
            };
            if blocked {
                m.yaw_offset = before;
                delta[0] = 0.0;
            }
        }
        self.mount = Some(m);
        [self.yaw, self.pitch] = self.turned(delta);
        self.note_rotation();
    }

    /// The movement state's lean as the body's animator is shown it, and
    /// the lean the weapon's motion reads, once the smoothing has run: the
    /// smoothed lean takes no value while mounted (`SmoothedTilt`'s setter),
    /// and the weapon's reads the mount's camera lean (`SmoothTilt`).
    pub(crate) fn mounted_tilt_shown(&self) -> Option<f32> {
        self.mount.as_ref().and_then(|m| m.body_tilt)
    }

    pub(crate) fn mount_camera_tilt(&self) -> Option<f32> {
        self.mount.as_ref().map(|m| m.camera_tilt)
    }

    /// The body's `Tilt` as its animator is told it (a side mount sets it;
    /// otherwise the smoothed lean, which takes no value while mounted).
    pub fn body_tilt_shown(&self) -> f32 {
        self.mounted_tilt_shown().unwrap_or(self.smoothed_tilt)
    }

    /// The gesture the body's animator is shown (`InteractionIndex`), if one.
    pub fn shown_interaction(&self) -> Option<u8> {
        self.shown_interaction
    }

    /// The aiming arms' drain multiplier while mounted (`Physical`).
    pub(crate) fn mount_drain(&self) -> f32 {
        match (self.mount.as_ref(), self.mount_weapon) {
            (Some(m), Some((_, bipod_out))) => mounting::drain(&self.mount_settings, m.data.point.side, bipod_out),
            _ => 1.0,
        }
    }
}
