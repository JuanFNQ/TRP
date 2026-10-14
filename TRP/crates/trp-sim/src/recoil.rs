//! Recoil, as specified in `docs/spec/weapon-handling.md`.
//!
//! A shot picks a direction and a strength and pushes three springs: the
//! weapon's rotation, the weapon's position and the camera's rotation. They
//! are stepped once per physics step, and like the other springs their
//! formulas have no time step in them.
//!
//! Angles are in degrees, in the weapon bone's axes: a negative x raises
//! the muzzle, z turns it sideways.

use trp_data::curve::Curve;
use trp_data::player::{RecoilProcessData, RecoilShotVal};

/// The game draws from one global random source. This one is seeded, so a
/// run can be repeated.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    /// Uniform in 0..1.
    pub fn unit(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let bits = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40;
        bits as f32 / (1u64 << 24) as f32
    }

    /// `Random.Range` for floats. The ends may come in either order.
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

/// `Mathf.Clamp`, which unlike Rust's accepts a lower bound above the upper
/// one. The game relies on what it returns then.
fn unity_clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn square_length(a: [f32; 3]) -> f32 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}

/// One group of shots within a burst that adds to the weapon's base values.
#[derive(Debug, Clone, Copy)]
pub struct ShotGroup {
    pub start: i32,
    pub end: i32,
    pub rotation_strength: [f32; 2],
    pub position_strength: [f32; 2],
    /// Added to the two ends of the recoil angle range, in degrees.
    pub angle_range: [f32; 2],
}

/// The weapon's own curve played from each shot.
#[derive(Debug, Clone)]
pub struct CurveRecoil {
    /// 0 camera rotation, 1 hands position, 2 hands rotation.
    pub target: u8,
    pub axis: usize,
    pub time_multiply: f32,
    pub value_multiply: f32,
    pub aiming_value_multiply: f32,
    pub curve: Curve,
}

/// What the weapon's template sets on the rotation spring.
#[derive(Debug, Clone)]
pub struct RotationSettings {
    pub return_speed: f32,
    pub damping: f32,
    pub category_multiplier: f32,
    pub return_path_offset: f32,
    pub return_path_damping: f32,
    pub stable_angle_increase_step: f32,
    pub progress_angle_on_stable: [f32; 2],
    pub after_recoil_vertical_range: [f32; 2],
    pub after_recoil_horizontal_range: [f32; 2],
    pub reset_after_shot: bool,
    pub to_next_stable_point_distance_limit: f32,
    pub next_stable_point_distance_range: [f32; 2],
    pub to_next_current_speed: f32,
    pub return_speed_curve: Curve,
}

/// Everything about recoil for one weapon build, resolved.
#[derive(Debug, Clone)]
pub struct RecoilSettings {
    pub progress_power_on: bool,
    pub progress_power_shot: i32,
    /// The shot of a burst from which the weapon holds around one point.
    pub stable_shot_index: i32,
    pub shot_vals: Vec<RecoilShotVal>,
    /// Range the rotation and position strength of a shot is drawn from.
    pub rotation_strength: [f32; 2],
    pub position_strength: [f32; 2],
    /// Range of the recoil's direction, in radians; a quarter turn is straight up.
    pub angle_range: [f32; 2],
    pub groups: Vec<ShotGroup>,
    /// Scale of the three axes of a shot, by stance: prone, crouched, standing.
    pub intensity_by_pose: [[f32; 3]; 3],
    /// Scale of the rotation spring's kick while aiming.
    pub aim_intensity: f32,
    pub rotation: RotationSettings,
    pub hand_position: RecoilProcessData,
    pub camera_rotation: RecoilProcessData,
    pub curves: Vec<CurveRecoil>,
}

impl RecoilSettings {
    /// The shot effector as the player prefab has it, with nothing a
    /// weapon's template would set: for hands that hold no weapon, which
    /// never make a shot.
    pub fn of_prefab(base: &trp_data::player::RecoilData) -> Self {
        Self {
            progress_power_on: base.progress_power_on,
            progress_power_shot: base.progress_power_shot,
            stable_shot_index: 0,
            shot_vals: base.shot_vals.clone(),
            rotation_strength: [0.0; 2],
            position_strength: [0.0; 2],
            angle_range: [0.0; 2],
            groups: Vec::new(),
            intensity_by_pose: [[1.0; 3]; 3],
            aim_intensity: 1.0,
            rotation: RotationSettings {
                return_speed: base.hand_rotation.return_speed,
                damping: base.hand_rotation.damping,
                category_multiplier: 1.0,
                return_path_offset: 0.0,
                return_path_damping: 0.0,
                stable_angle_increase_step: 0.0,
                progress_angle_on_stable: [0.0; 2],
                after_recoil_vertical_range: [0.0; 2],
                after_recoil_horizontal_range: [0.0; 2],
                reset_after_shot: false,
                to_next_stable_point_distance_limit: base.to_next_stable_point_distance_limit,
                next_stable_point_distance_range: base.next_stable_point_distance_range,
                to_next_current_speed: base.to_next_current_speed,
                return_speed_curve: base.return_speed_curve.clone(),
            },
            hand_position: base.hand_position,
            camera_rotation: base.camera_rotation,
            curves: Vec::new(),
        }
    }
}

/// The position and camera springs.
#[derive(Debug, Clone)]
pub struct Process {
    data: RecoilProcessData,
    /// Where the spring rests; other effects move it.
    pub offset: [f32; 3],
    velocity: [f32; 3],
    pub current: [f32; 3],
    force: [f32; 3],
    need_update: bool,
}

impl Process {
    fn new(data: RecoilProcessData) -> Self {
        Self { data, offset: [0.0; 3], velocity: [0.0; 3], current: [0.0; 3], force: [0.0; 3], need_update: false }
    }

    fn push(&mut self, axis: usize, value: f32) {
        self.force[axis] += value;
        self.need_update = true;
    }

    fn step(&mut self) {
        if self.need_update {
            self.velocity = add(self.velocity, scale(self.force, self.data.intensity));
            self.force = [0.0; 3];
            self.need_update = false;
        }
        self.velocity = sub(self.velocity, scale(sub(self.current, self.offset), self.data.return_speed));
        self.velocity = scale(self.velocity, self.data.damping);
        self.current = add(self.current, self.velocity);
    }

    pub fn value(&self) -> [f32; 3] {
        add(self.offset, self.current)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
    None,
}

/// The weapon's rotation spring, with its extra behaviour in automatic fire
/// and on the way back down.
#[derive(Debug, Clone)]
pub struct RotationProcess {
    s: RotationSettings,
    pub intensity: f32,
    pub offset: [f32; 3],
    velocity: [f32; 3],
    pub current: [f32; 3],
    force: [f32; 3],
    need_update: bool,
    pub stable_on: bool,
    pub auto_fire_on: bool,
    multiplier_by_shot_index: f32,
    current_angle_add: f32,
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
}

impl RotationProcess {
    fn new(s: RotationSettings) -> Self {
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
        }
    }

    fn reset(&mut self) {
        let fresh = Self::new(self.s.clone());
        *self = Self {
            intensity: self.intensity,
            stable_on: self.stable_on,
            auto_fire_on: self.auto_fire_on,
            multiplier_by_shot_index: self.multiplier_by_shot_index,
            current_angle_add: self.current_angle_add,
            need_update: self.need_update,
            side: self.side,
            ..fresh
        };
    }

    fn push(&mut self, axis: usize, value: f32) {
        self.return_trajectory_on = true;
        if self.stable_on {
            self.velocity = [0.0; 3];
        }
        self.force[axis] += value * self.s.category_multiplier * self.multiplier_by_shot_index;
        self.need_update = true;
    }

    fn set_stable(&mut self, on: bool) {
        if on && !self.stable_on {
            self.stable_on = true;
            self.stable_current = self.current;
        } else if !on && self.stable_on {
            self.current_angle_add = 0.0;
            self.stable_on = false;
        }
    }

    fn set_auto_fire(&mut self, on: bool) {
        if on {
            self.auto_fire_on = true;
        } else if self.auto_fire_on {
            self.curve_time = square_length(self.current).min(self.curve_time);
            self.auto_fire_on = false;
        }
    }

    fn step(&mut self, dt: f32, rng: &mut Rng) {
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

/// All of a weapon's recoil.
#[derive(Debug, Clone)]
pub struct Recoil {
    pub settings: RecoilSettings,
    pub hand_rotation: RotationProcess,
    pub hand_position: Process,
    pub camera_rotation: Process,
    /// Clock and value of each of the weapon's curves.
    curves: Vec<(f32, f32)>,
    /// Shots so far in the current run of automatic fire.
    pub shot_index: i32,
    auto_fire: bool,
    pose: usize,
    aiming: bool,
    rng: Rng,
}

impl Recoil {
    pub fn new(settings: RecoilSettings, seed: u64) -> Self {
        // The game starts each curve's clock at zero, so the curves play
        // once as the weapon is taken in hand. Here they start finished.
        let curves = settings
            .curves
            .iter()
            .map(|c| (c.curve.keys.last().map_or(0.0, |k| k.time), 0.0))
            .collect();
        Self {
            hand_rotation: RotationProcess::new(settings.rotation.clone()),
            hand_position: Process::new(settings.hand_position),
            camera_rotation: Process::new(settings.camera_rotation),
            curves,
            shot_index: 0,
            auto_fire: false,
            pose: 2,
            aiming: false,
            rng: Rng::new(seed),
            settings,
        }
    }

    /// The weapon's build has changed (a stock folded): the least and the
    /// most a shot turns the weapon and pushes it back.
    pub fn set_strengths(&mut self, rotation: [f32; 2], position: [f32; 2]) {
        (self.settings.rotation_strength, self.settings.position_strength) = (rotation, position);
    }

    /// Whether the weapon is in a run of automatic fire.
    pub fn set_auto_fire(&mut self, on: bool) {
        self.auto_fire = on;
    }

    /// 0 prone, 1 crouched, 2 standing.
    pub fn set_pose(&mut self, pose: usize) {
        self.pose = pose.min(2);
    }

    pub fn set_aiming(&mut self, aiming: bool) {
        self.aiming = aiming;
        self.hand_rotation.intensity = if aiming { self.settings.aim_intensity } else { 1.0 };
    }

    /// One shot. `strength` is 1 plus the cartridge's recoil percentage.
    pub fn shoot(&mut self, strength: f32) {
        let s = &self.settings;
        self.shot_index += 1;
        if self.shot_index == s.stable_shot_index {
            self.hand_rotation.set_stable(true);
        }
        self.hand_rotation.multiplier_by_shot_index =
            if s.progress_power_on && self.shot_index >= s.progress_power_shot && s.stable_shot_index > 0 {
                self.shot_index.clamp(1, s.stable_shot_index) as f32 / s.stable_shot_index as f32
            } else {
                1.0
            };

        let in_group = |g: &&ShotGroup| self.shot_index >= g.start && self.shot_index <= g.end;
        let (mut rotation, mut position, mut angle) = (s.rotation_strength, s.position_strength, [0.0f32; 2]);
        for g in s.groups.iter().filter(in_group) {
            for i in 0..2 {
                rotation[i] += g.rotation_strength[i];
                position[i] += g.position_strength[i];
                angle[i] += g.angle_range[i];
            }
        }
        let rotation_power = self.rng.range(rotation[0], rotation[1]) * strength;
        let position_power = self.rng.range(position[0], position[1]) * strength;

        if self.hand_rotation.stable_on {
            // Held automatic fire widens the cone a little with every shot.
            let limits = s.rotation.progress_angle_on_stable;
            let widened = self.hand_rotation.current_angle_add + s.rotation.stable_angle_increase_step;
            self.hand_rotation.current_angle_add = unity_clamp(widened, limits[0], limits[1]);
            angle[0] -= self.hand_rotation.current_angle_add;
            angle[1] += self.hand_rotation.current_angle_add;
        }
        let from = s.angle_range[0] + angle[0].to_radians();
        let to = s.angle_range[1] + angle[1].to_radians();
        let direction = self.rng.range(from, to);

        let factors = s.intensity_by_pose[self.pose];
        let kick = scale(
            [
                -direction.sin() * rotation_power * factors[0],
                direction.cos() * rotation_power * factors[1],
                position_power * factors[2],
            ],
            s.hand_position.intensity,
        );
        for val in &s.shot_vals {
            let value = kick[val.from_axis as usize % 3] * val.intensity;
            let axis = val.to_axis as usize % 3;
            match val.target {
                0 => self.camera_rotation.push(axis, value),
                1 => self.hand_position.push(axis, value),
                2 => self.hand_rotation.push(axis, value),
                _ => {}
            }
        }
        for curve in &mut self.curves {
            *curve = (0.0, 0.0);
        }
    }

    /// One physics step.
    pub fn step(&mut self, dt: f32) {
        self.hand_rotation.set_auto_fire(self.auto_fire);
        if !self.auto_fire {
            self.shot_index = 0;
            self.hand_rotation.set_stable(false);
        }
        self.hand_position.step();
        self.camera_rotation.step();
        self.hand_rotation.step(dt, &mut self.rng);
        for (state, c) in self.curves.iter_mut().zip(&self.settings.curves) {
            let multiply = if self.aiming { c.aiming_value_multiply } else { c.value_multiply };
            state.1 = multiply * c.curve.evaluate(state.0);
            state.0 += dt * c.time_multiply;
        }
    }

    fn curve_sum(&self, target: u8) -> [f32; 3] {
        let mut sum = [0.0; 3];
        for (state, c) in self.curves.iter().zip(&self.settings.curves) {
            if c.target == target {
                sum[c.axis] += state.1;
            }
        }
        sum
    }

    /// Rotation to add to the weapon, about its recoil pivot.
    pub fn weapon_rotation(&self) -> [f32; 3] {
        add(self.hand_rotation.value(), self.curve_sum(2))
    }

    /// Offset to add to the weapon.
    pub fn weapon_position(&self) -> [f32; 3] {
        add(self.hand_position.value(), self.curve_sum(1))
    }

    /// Angles to add to the camera's own.
    pub fn camera_angles(&self) -> [f32; 3] {
        add(self.camera_rotation.current, self.curve_sum(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(return_speed: f32, damping: f32) -> RecoilProcessData {
        RecoilProcessData { return_speed, damping, intensity: 1.0 }
    }

    /// Settings shaped like an assault rifle's, with round numbers.
    pub(crate) fn rifle() -> RecoilSettings {
        RecoilSettings {
            progress_power_on: true,
            progress_power_shot: 2,
            stable_shot_index: 5,
            shot_vals: vec![
                RecoilShotVal { from_axis: 0, target: 2, to_axis: 0, intensity: 0.15 },
                RecoilShotVal { from_axis: 1, target: 2, to_axis: 2, intensity: 0.15 },
                RecoilShotVal { from_axis: 2, target: 1, to_axis: 1, intensity: 0.0007 },
                RecoilShotVal { from_axis: 0, target: 0, to_axis: 0, intensity: 0.05 },
                RecoilShotVal { from_axis: 1, target: 0, to_axis: 1, intensity: -0.05 },
                RecoilShotVal { from_axis: 1, target: 4, to_axis: 1, intensity: 1.0 },
            ],
            rotation_strength: [18.0, 23.0],
            position_strength: [30.0, 48.0],
            angle_range: [80f32.to_radians(), 90f32.to_radians()],
            groups: Vec::new(),
            intensity_by_pose: [[0.6, 0.7, 0.7], [0.7, 0.7, 0.7], [1.0, 1.0, 1.0]],
            aim_intensity: 0.75,
            rotation: RotationSettings {
                return_speed: 2.4,
                damping: 0.86,
                category_multiplier: 0.19,
                return_path_offset: 0.01,
                return_path_damping: 0.48,
                stable_angle_increase_step: 2.5,
                progress_angle_on_stable: [0.0, 28.0],
                after_recoil_vertical_range: [-1.5, 0.0],
                after_recoil_horizontal_range: [-1.5, 1.5],
                reset_after_shot: false,
                to_next_stable_point_distance_limit: 0.95,
                next_stable_point_distance_range: [0.1, 8.0],
                to_next_current_speed: 0.2,
                return_speed_curve: Curve::flat(&[(0.0, 0.008), (2.5, 0.008)]),
            },
            hand_position: data(0.08, 0.5),
            camera_rotation: data(0.05, 0.5),
            curves: Vec::new(),
        }
    }

    #[test]
    fn random_numbers_stay_in_range_and_repeat() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..1000 {
            let x = a.range(-2.0, 3.0);
            assert!((-2.0..=3.0).contains(&x));
            assert_eq!(x, b.range(-2.0, 3.0));
        }
        // Reversed ends are allowed.
        let y = a.range(5.0, 1.0);
        assert!((1.0..=5.0).contains(&y));
    }

    #[test]
    fn clamp_with_crossed_bounds_follows_the_engine() {
        assert_eq!(unity_clamp(-3.0, 0.01, -0.01), 0.01);
        assert_eq!(unity_clamp(0.5, 0.01, -0.01), -0.01);
        assert_eq!(unity_clamp(-0.5, -2.0, -0.01), -0.5);
    }

    #[test]
    fn one_shot_kicks_up_and_back_then_settles() {
        let mut r = Recoil::new(rifle(), 1);
        r.shoot(1.0);
        let (mut climb, mut back, mut camera) = (0.0f32, 0.0f32, 0.0f32);
        for _ in 0..600 {
            r.step(1.0 / 60.0);
            climb = climb.min(r.weapon_rotation()[0]);
            back = back.max(r.weapon_position()[1]);
            camera = camera.min(r.camera_angles()[0]);
        }
        assert!(climb < -1.0 && climb > -8.0, "muzzle climb {climb}");
        assert!(back > 0.005 && back < 0.06, "kick back {back}");
        assert!(camera < -0.3 && camera > -3.0, "camera kick {camera}");
        // The muzzle comes to rest near, not at, where it started.
        let rest = r.weapon_rotation();
        assert!(rest[0] <= 0.05 && rest[0] > -1.6, "rest {rest:?}");
        assert!(rest[2].abs() < 1.6);
        assert!(r.weapon_position()[1].abs() < 1e-4);
        assert!(r.camera_angles()[0].abs() < 1e-3);
    }

    #[test]
    fn crouching_and_lying_down_soften_the_kick() {
        let climb = |pose: usize| {
            let mut r = Recoil::new(rifle(), 3);
            r.set_pose(pose);
            r.shoot(1.0);
            (0..120).fold(0.0f32, |low, _| {
                r.step(1.0 / 60.0);
                low.min(r.weapon_rotation()[0])
            })
        };
        let (prone, standing) = (climb(0), climb(2));
        assert!((prone / standing - 0.6).abs() < 0.02, "{prone} {standing}");
    }

    #[test]
    fn automatic_fire_ramps_up_then_holds_around_a_point() {
        let mut r = Recoil::new(rifle(), 11);
        r.set_auto_fire(true);
        let mut after_shot = Vec::new();
        for shot in 0..20 {
            r.shoot(1.0);
            for _ in 0..6 {
                r.step(1.0 / 60.0);
            }
            after_shot.push(r.weapon_rotation()[0]);
            assert_eq!(r.shot_index, shot + 1);
        }
        assert!(r.hand_rotation.stable_on);
        // The first shots climb.
        assert!(after_shot[3] < after_shot[0]);
        // Once stable, the muzzle stays within a band instead of climbing on.
        let held = &after_shot[6..];
        let (low, high) = held.iter().fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
        assert!(high - low < 6.0, "band {low}..{high}");

        // Letting go ends the run and the muzzle comes back down.
        r.set_auto_fire(false);
        for _ in 0..900 {
            r.step(1.0 / 60.0);
        }
        assert_eq!(r.shot_index, 0);
        assert!(!r.hand_rotation.stable_on);
        assert!(r.weapon_rotation()[0] > -1.6, "{}", r.weapon_rotation()[0]);
    }

    #[test]
    fn weapon_curves_play_from_each_shot() {
        let mut settings = rifle();
        settings.curves.push(CurveRecoil {
            target: 2,
            axis: 1,
            time_multiply: 2.0,
            value_multiply: 1.0,
            aiming_value_multiply: 0.5,
            curve: Curve::flat(&[(0.0, 0.0), (0.1, 2.0), (0.3, -0.75), (1.0, 0.0)]),
        });
        let mut r = Recoil::new(settings, 5);
        r.step(1.0 / 60.0);
        assert_eq!(r.weapon_rotation()[1], 0.0, "nothing plays before the first shot");
        r.shoot(1.0);
        let mut peak = 0.0f32;
        for _ in 0..3 {
            r.step(1.0 / 60.0);
            peak = peak.max(r.weapon_rotation()[1]);
        }
        // The curve's clock runs twice as fast, so its peak at 0.1 is
        // reached on the fourth step.
        r.step(1.0 / 60.0);
        peak = peak.max(r.weapon_rotation()[1]);
        assert!(peak > 1.9 && peak <= 2.0, "{peak}");
        r.set_aiming(true);
        r.shoot(1.0);
        for _ in 0..4 {
            r.step(1.0 / 60.0);
        }
        assert!((r.weapon_rotation()[1] - 1.0).abs() < 0.06);
    }
}
