//! Recoil, as specified in `docs/spec/weapon-handling.md`.
//!
//! A shot picks a direction and a strength and pushes three springs: the
//! weapon's rotation, the weapon's position and the camera's rotation. They
//! are stepped once per physics step, and like the other springs their
//! formulas have no time step in them.
//!
//! Angles are in degrees, in the weapon bone's axes: a negative x raises
//! the muzzle, z turns it sideways.

use trp_data::formats::curve::Curve;
use trp_data::character::player::{RecoilProcessData, RecoilShotVal};

use crate::base::math::unity_clamp;

mod settings;
mod rotation;

pub use settings::*;
pub use rotation::*;
use rotation::{finished_curves};

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
        let curves = finished_curves(&settings);
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

    /// A firearm taken in hand after another thing, its springs carried on
    /// from that one's (`Hands::carry_on_from`): the game's shot effect is
    /// one object, told the new weapon's figures. The rotation spring takes
    /// this weapon's (`RecalculateRecoilParamsOnChangeWeapon`; its return
    /// speed and damping, `ProceduralWeaponAnimation.InitWeaponData`) and is
    /// put back to nought (`HandRotationRecoil.Reset`); the weapon's own
    /// curves are made anew (`new WeaponRecoil`), finished as `new` has
    /// them. The other two springs' figures are the player prefab's, the
    /// same for every weapon. Without this the spring kept the last thing's
    /// figures: the empty hands' (no class multiplier, no stable point, the
    /// prefab's return), so every weapon climbed too far.
    pub fn take_weapon_figures(&mut self) {
        self.hand_rotation.s = self.settings.rotation.clone();
        self.hand_position.data = self.settings.hand_position;
        self.camera_rotation.data = self.settings.camera_rotation;
        self.curves = finished_curves(&self.settings);
        self.hand_rotation.reset();
    }

    /// Whether the weapon is in a run of automatic fire.
    pub fn set_auto_fire(&mut self, on: bool) {
        self.auto_fire = on;
    }

    /// Whether the weapon's controller has a run of automatic fire on
    /// (`FirearmController.autoFireOn`), as it was last told.
    pub fn auto_fire(&self) -> bool {
        self.auto_fire
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
mod tests;
