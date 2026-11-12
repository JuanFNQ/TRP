//! Asking for a mount, answering its search, entering and leaving it.

use super::*;

impl Movement {
    /// The lean as a mounted state takes it (`IdleWeaponMountingState.
    /// SetTilt`): anything but none starts the exit; none does nothing.
    pub(crate) fn mount_tilt(&mut self, tilt: f32) {
        if tilt != 0.0 {
            self.start_leaving_mount();
        }
    }

    /// The mount's key (`Player.TryMountWeapon`, `WeaponMountingComponent.
    /// TryMountWeapon`).
    pub(crate) fn ask_mount(&mut self, input: &Input, world: &dyn World) {
        if !(input.mount_may && self.grounded && self.blind_fire == 0) {
            return;
        }
        let Some((weapon, bipod_out)) = self.mount_weapon else { return };
        if self.mount.is_some() {
            self.start_leaving_mount();
            return;
        }
        let root = self.grid_root();
        if self.prone {
            // Lying down only the bipod mounts: out, no step aside, no keys.
            if bipod_out && self.step == 0 && is_zero(input.move_dir) {
                let pivot = weapon.bipod_pivot.unwrap_or([0.0; 3]);
                self.mount_search = None;
                match mounting::search_prone(world, &self.mount_settings, root, self.yaw, self.surface_pitch, pivot[1].abs(), pivot[2].abs()) {
                    Some(point) => self.found_mount(point, world),
                    None => self.mount_events.push(MountEvent::Refused),
                }
            } else {
                self.mount_events.push(MountEvent::Refused);
            }
            return;
        }
        let column_raise = if bipod_out { weapon.bipod_pivot.unwrap_or([0.0; 3])[2] } else { weapon.pivot[2] };
        let grid = Grid { root, yaw: self.yaw, feet: self.position[1], column_raise, row_raise: weapon.pivot[2] };
        self.mount_search = Some(Search::standing(world, &self.mount_settings, grid));
        self.mount_search_fresh = true;
    }

    /// The search's grid root: the body's `Weapon_root` bone in the world.
    pub fn grid_root(&self) -> [f32; 3] {
        let (at, _) = self.in_player(self.body.weapon_bone());
        let turned = quat_rotate(quat_axis(1, self.body_yaw), at);
        std::array::from_fn(|k| self.position[k] + turned[k])
    }

    /// A frame after the frame of the key: a search under way is answered
    /// on its frame (`yield return null`: after the frame's updates, before
    /// the animator's).
    pub(crate) fn answer_mount_search(&mut self, world: &dyn World) {
        if std::mem::take(&mut self.mount_search_fresh) {
            return;
        }
        let Some(search) = self.mount_search.as_mut() else { return };
        let Some(answer) = search.frame() else { return };
        self.mount_search = None;
        match answer {
            Some(point) => self.found_mount(point, world),
            None => self.mount_events.push(MountEvent::Refused),
        }
    }

    /// A point found (`OnFoundPoint`): the data, then the mount; refused,
    /// nothing (`NoPointFound`). A door or a moving platform under the
    /// point: none in this world.
    fn found_mount(&mut self, point: Point, world: &dyn World) {
        let Some((weapon, bipod_out)) = self.mount_weapon else { return };
        let [height, centre] = self.capsule_figures();
        let asking = Asking {
            position: self.position,
            yaw: self.yaw,
            pitch: self.pitch,
            pose_level: self.pose_level,
            prone: self.prone,
            grounded: self.grounded,
            weapon,
            bipod_out,
            capsule: [self.mount_settings.capsule_radius, height, self.mount_settings.step_offset, centre],
        };
        match mounting::data_for(world, &self.mount_settings, point, &asking) {
            Some(data) => self.enter_mount(data, world),
            None => self.mount_events.push(MountEvent::Refused),
        }
    }

    /// `IdleWeaponMountingState.Enter`, and the rest of `EnterMountedState`.
    fn enter_mount(&mut self, data: Data, world: &dyn World) {
        let forward = data.point.side == Side::Forward;
        let side = data.point.side.number();
        // The lean becomes the slow lean's; the lean at the press is kept.
        self.slow_leaning = true;
        let entry_tilt = self.smoothed_tilt;
        self.tilt = 0.0;
        self.step = 0;
        // The left shoulder off for good.
        self.left_stance_mounted();
        let limit = data.yaw_limit;
        let mut span = delta_angle(limit[0], limit[1]).abs();
        if forward {
            span /= 2.0;
        }
        let (mut target_yaw, mut yaw_shift, mut target_tilt) = (data.target_yaw, 0.0, 0.0);
        if !forward {
            let (mut yaw, other) = mounting_both_ways(self.yaw);
            if yaw < limit[0] || yaw > limit[1] {
                yaw = other;
            }
            target_yaw = if yaw >= limit[0] && yaw <= limit[1] {
                self.yaw
            } else if self.yaw < limit[0] {
                limit[0]
            } else {
                limit[1]
            };
            yaw_shift = if (data.target_yaw - limit[0]).abs() <= 1e-6 * data.target_yaw.abs().max(1.0) { target_yaw - limit[0] } else { target_yaw - limit[1] };
            let toward = if side as f32 * yaw_shift < 0.0 { side as f32 } else if side == 0 { -yaw_shift } else { 0.0 };
            target_tilt = sign(toward) * lerp(0.0, MAX_TILT, yaw_shift.abs() / span);
        }
        let up = data.pitch_limit[0];
        let down = data.pitch_limit[1];
        self.pitch_target = [up, down];
        let target_pitch = if forward && !self.prone {
            0.0
        } else if self.pitch < self.pitch_limit[0] {
            up
        } else if self.pitch > self.pitch_limit[1] {
            self.pitch_limit[1]
        } else {
            self.pitch
        };
        let pose = lerp(data.pose_limit[0], data.pose_limit[1], inverse_lerp(up, down, target_pitch));
        self.set_pose_level(pose, world);
        self.mount = Some(Mounted {
            data,
            side,
            timer: 0.0,
            target_pitch,
            target_yaw,
            entry_tilt,
            target_tilt,
            yaw_shift,
            yaw_offset: 0.0,
            span,
            exiting: false,
            down_limit: down,
            start_position: self.position,
            start_look: [self.yaw, self.pitch],
            start_body: self.body_yaw,
            transition: true,
            progress: 0.0,
            drop: 0.0,
            mounted_tilt: 0.0,
            camera_tilt: 0.0,
            body_tilt: None,
        });
        self.mount_events.push(MountEvent::Entered { approach: data.approach, side: data.point.side, on_bipod: data.on_bipod, prone: self.prone });
        self.mount_last = Some(data);
    }


    /// `StartExitingMountedState`, `IdleWeaponMountingState.StartExiting`.
    pub(crate) fn start_leaving_mount(&mut self) {
        let Some(m) = self.mount.as_mut() else { return };
        if m.exiting {
            return;
        }
        m.exiting = true;
        m.timer = 0.0;
        m.transition = true;
        m.progress = 1.0;
        if m.side != 0 {
            m.body_tilt = Some(m.mounted_tilt);
        }
        let side = m.data.point.side;
        self.mount_events.push(MountEvent::Leaving { side });
    }

    /// `ExitMountedState`, with `IdleWeaponMountingState.Exit`.
    pub(crate) fn leave_mount_now(&mut self) {
        let Some(m) = self.mount.take() else { return };
        // The state is off before its own exit runs: the lean set with
        // force reaches the smoothed lean (`SmoothedTilt` takes no value
        // while mounted).
        if m.side == 0 {
            self.tilt = 0.0;
        } else {
            self.tilt = m.mounted_tilt.clamp(-MAX_TILT, MAX_TILT);
            self.smoothed_tilt = self.tilt;
        }
        self.ignore_root_motion = false;
        self.yaw_limit = None;
        self.pitch_target = if self.prone { PRONE_PITCH_RANGE } else { UPRIGHT_PITCH_RANGE };
        self.weapon_tilt = self.smoothed_tilt;
        self.mount_events.push(MountEvent::Left { side: m.data.point.side });
    }
}
