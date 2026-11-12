//! The rotation process: pushes, steps and the return trajectory.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Side {
    Left,
    Right,
    None,
}

/// The weapon's rotation spring, with its extra behaviour in automatic fire
/// and on the way back down.
#[derive(Debug, Clone)]
pub struct RotationProcess {
    pub(super) s: RotationSettings,
    pub intensity: f32,
    pub offset: [f32; 3],
    pub(super) velocity: [f32; 3],
    pub current: [f32; 3],
    pub(super) force: [f32; 3],
    pub(super) need_update: bool,
    pub stable_on: bool,
    pub auto_fire_on: bool,
    pub(super) multiplier_by_shot_index: f32,
    pub(super) current_angle_add: f32,
    previous_current: [f32; 3],
    after_recoil_rest: [f32; 2],
    stable_current: [f32; 3],
    next_current: [f32; 3],
    curve_time: f32,
    auto_fire_speed: f32,
    recoil_offset_impulse: f32,
    after_offset_current_x: f32,
    return_trajectory_on: bool,
    side: Side,
    /// Mounted (`_mountedMode`; mounting spec, "Recoil"): what a shot's push
    /// is multiplied by (the side's, the bipod's or the forward's), the
    /// return's speed (the bipod's or the mount's) and the return's swing
    /// aside (always the mount's: `MountingReturnHandSpeedMultiplier`).
    mounted: Option<(f32, f32, f32)>,
}

impl RotationProcess {
    pub(super) fn new(s: RotationSettings) -> Self {
        Self {
            s,
            intensity: 1.0,
            offset: [0.0; 3],
            velocity: [0.0; 3],
            current: [0.0; 3],
            force: [0.0; 3],
            need_update: false,
            stable_on: false,
            auto_fire_on: false,
            multiplier_by_shot_index: 1.0,
            current_angle_add: 0.0,
            previous_current: [0.0; 3],
            after_recoil_rest: [0.0; 2],
            stable_current: [0.0; 3],
            next_current: [0.0; 3],
            curve_time: 0.0,
            auto_fire_speed: 0.0,
            recoil_offset_impulse: 0.01,
            after_offset_current_x: 0.01,
            return_trajectory_on: false,
            side: Side::None,
            mounted: None,
        }
    }

    /// Mounted with these figures, or not (`SetMountedMode`, `SetBipodMode`).
    pub fn set_mounted(&mut self, mounted: Option<(f32, f32, f32)>) {
        self.mounted = mounted;
    }

    /// Back to nought (`NewRotationRecoilProcess.Reset`): the place, its
    /// speed, the breath's offset and the return's state; the mounted
    /// figures and the run of fire kept.
    pub(crate) fn reset(&mut self) {
        let fresh = Self::new(self.s.clone());
        *self = Self {
            intensity: self.intensity,
            stable_on: self.stable_on,
            auto_fire_on: self.auto_fire_on,
            multiplier_by_shot_index: self.multiplier_by_shot_index,
            current_angle_add: self.current_angle_add,
            need_update: self.need_update,
            side: self.side,
            mounted: self.mounted,
            ..fresh
        };
    }

    pub(super) fn push(&mut self, axis: usize, value: f32) {
        self.return_trajectory_on = true;
        if self.stable_on {
            self.velocity = [0.0; 3];
        }
        let mounted = self.mounted.map_or(1.0, |m| m.0);
        self.force[axis] += value * self.s.category_multiplier * mounted * self.multiplier_by_shot_index;
        self.need_update = true;
    }

    pub(super) fn set_stable(&mut self, on: bool) {
        if on && !self.stable_on {
            self.stable_on = true;
            self.stable_current = self.current;
        } else if !on && self.stable_on {
            self.current_angle_add = 0.0;
            self.stable_on = false;
        }
    }

    pub(super) fn set_auto_fire(&mut self, on: bool) {
        if on {
            self.auto_fire_on = true;
        } else if self.auto_fire_on {
            self.curve_time = square_length(self.current).min(self.curve_time);
            self.auto_fire_on = false;
        }
    }

    pub(super) fn step(&mut self, dt: f32, rng: &mut Rng) {
        if self.need_update {
            self.velocity = add(self.velocity, scale(self.force, self.intensity));
            self.force = [0.0; 3];
            self.curve_time = square_length(self.current).min(self.curve_time);
            if self.stable_on {
                let range = self.s.next_stable_point_distance_range;
                let distance = rng.range(range[0], range[1]);
                self.next_current = add(self.stable_current, scale(self.velocity, distance));
                let direction = sub(self.next_current, self.current);
                let limit = self.s.to_next_stable_point_distance_limit;
                if square_length(direction).sqrt() > limit {
                    // The step is scaled by the limit, not cut to its length.
                    self.next_current = add(self.current, scale(direction, limit));
                }
            }
        }
        if !self.auto_fire_on {
            self.auto_fire_speed = self.s.return_speed * self.s.return_speed_curve.evaluate(self.curve_time);
            if let Some((_, back, _)) = self.mounted {
                self.auto_fire_speed *= back;
            }
        }
        if self.need_update && self.auto_fire_on && !self.stable_on {
            self.auto_fire_speed *= self.multiplier_by_shot_index;
        }
        self.curve_time += dt;
        self.velocity = scale(self.velocity, self.s.damping);
        if self.stable_on {
            let toward = sub(self.next_current, self.current);
            self.current = add(self.current, scale(toward, self.s.to_next_current_speed));
        } else {
            self.velocity = sub(self.velocity, scale(self.current, self.auto_fire_speed));
            self.current = add(self.current, self.velocity);
        }
        self.return_trajectory(rng);
        if !self.auto_fire_on && self.after_offset_current_x != 0.0 {
            // How far the muzzle still is from where it peaked decides how
            // hard it is drawn to the place it will settle.
            let remaining =
                (unity_clamp(self.current[0], self.after_offset_current_x, -0.01) / self.after_offset_current_x).abs();
            let speed = (1.0 - remaining).clamp(0.01, 1.0) * 0.01;
            self.current[0] += (self.after_recoil_rest[0] - self.current[0]) * speed;
            self.current[2] += (self.after_recoil_rest[1] - self.current[2]) * speed;
        }
        self.previous_current = self.current;
        self.need_update = false;
    }

    /// While the muzzle still climbs, picks where it will settle and how
    /// hard it will swing aside on the way down; afterwards applies that swing.
    fn return_trajectory(&mut self, rng: &mut Rng) {
        if self.current[0] < self.previous_current[0] && self.return_trajectory_on {
            let horizontal = self.s.after_recoil_horizontal_range;
            let aside = match self.side {
                Side::Right => rng.range(self.current[2], horizontal[1]),
                Side::Left => rng.range(horizontal[0], self.current[2]),
                Side::None => 0.0,
            };
            let vertical = self.s.after_recoil_vertical_range;
            self.after_recoil_rest = [rng.range(vertical[0], vertical[1]), aside];
            self.after_offset_current_x = self.current[0];

            self.recoil_offset_impulse = (self.current[0] * self.s.return_speed).abs() * self.s.return_path_offset;
            if let Some((_, _, aside)) = self.mounted {
                self.recoil_offset_impulse *= aside;
            }
            self.side = if self.current[2] > 0.0 {
                Side::Right
            } else if self.current[2] < 0.0 {
                Side::Left
            } else if rng.range(-1.0, 1.0) >= 0.0 {
                Side::Right
            } else {
                Side::Left
            };
            return;
        }
        if self.auto_fire_on || !self.return_trajectory_on {
            return;
        }
        self.recoil_offset_impulse *= self.s.return_path_damping;
        if self.recoil_offset_impulse < 0.001 {
            if self.s.reset_after_shot {
                self.reset();
            } else {
                self.return_trajectory_on = false;
            }
        } else {
            self.velocity[2] += self.recoil_offset_impulse * if self.side == Side::Right { 1.0 } else { -1.0 };
        }
    }

    pub fn value(&self) -> [f32; 3] {
        add(self.offset, self.current)
    }
}

/// Each of a weapon's curves at its end, as the clock and the value. The
/// game starts each curve's clock at zero, so the curves play once as the
/// weapon is taken in hand. Here they start finished.
pub(super) fn finished_curves(settings: &RecoilSettings) -> Vec<(f32, f32)> {
    settings.curves.iter().map(|c| (c.curve.keys.last().map_or(0.0, |k| k.time), 0.0)).collect()
}
