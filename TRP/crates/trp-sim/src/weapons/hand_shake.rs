//! The hand shake after a quick draw.
//! After the stamina spec, "The quick draw's hand shake" (`HandShakeEffector`, `ProceduralWeaponAnimation.StartHandShake`):
//! when the pistol's quick draw leaves the arms empty, the weapon's motion
//! shakes the hands for a time by how much the arms lacked. Two wandering
//! values push the hands' rotation spring, the lower the hands' level the
//! more the muzzle sags; three more move the recoil rotation's rest point,
//! and from the hip the first of them throws the muzzle down by the roll.
//! Whole for the first half of its time, it fades to nothing over the
//! second.
//!
//! It runs after the walk's bob in each physics step of the first person's
//! weapon motion only (`FirstPersonStrategy`: the mount's, a stationary
//! weapon's and the outside view's run none), while its time lasts by the
//! frame's clock (the coroutine's `WaitForSeconds`).

use trp_data::character::player::HandShakeData;

use crate::weapons::breath::SmoothRandom;
use crate::weapons::recoil::Rng;

/// What one step of the shake asks for.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ShakeStep {
    /// The push on the hands' rotation spring (`AddAcceleration`).
    pub hands_rotation: [f32; 3],
    /// Where the recoil rotation's rest point is now (`Offset`).
    pub muzzle_offset: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct HandShake {
    data: HandShakeData,
    hands_x: SmoothRandom,
    hands_y: SmoothRandom,
    tremor: [SmoothRandom; 3],
    /// Its strength (`_shakeIntensity`): the settings' while a shake runs,
    /// the prefab's again once it ends (`OnStop`).
    strength: f32,
    /// Whether it runs (the mask's `HandShake` bit), and what is left of
    /// its time by the frame's clock.
    on: bool,
    left: f32,
    /// The fade (`CalculateDamping`, `Process`): time run, time faded,
    /// where the fade starts and how long it takes, and the factor now
    /// (`_dampingCoef`, which only a new rig puts back to whole).
    passed: f32,
    faded: f32,
    fade_from: f32,
    fade_for: f32,
    damping: f32,
    rng: Rng,
}

impl HandShake {
    pub fn new(data: HandShakeData, seed: u64) -> Self {
        Self {
            hands_x: SmoothRandom::new(data.hands_x),
            hands_y: SmoothRandom::new(data.hands_y),
            tremor: data.tremor.map(SmoothRandom::new),
            strength: data.intensity,
            on: false,
            left: 0.0,
            passed: 0.0,
            faded: 0.0,
            fade_from: 0.0,
            fade_for: 0.0,
            damping: 1.0,
            rng: Rng::new(seed),
            data,
        }
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// A shake begun (`StartHandShake`, its coroutine's first part): on for
    /// `duration` seconds at `strength`, its fade starting at the prefab's
    /// share of that time; a shake under way is begun again. The fade's
    /// factor is left as it stands (a new rig made it whole, and the quick
    /// draw that begins a shake always has one).
    pub fn start(&mut self, duration: f32, strength: f32) {
        self.on = true;
        self.left = duration;
        self.passed = 0.0;
        self.faded = 0.0;
        self.fade_from = duration / 100.0 * (100.0 - self.data.damping_start_percent);
        self.fade_for = duration - self.fade_from;
        self.strength = strength;
    }

    /// Tied to a rig just taken in hand (`HandShakeEffector.Initialize`, in
    /// `InitTransforms`): its fade's factor whole again.
    pub fn tie_to_new_rig(&mut self) {
        self.damping = 1.0;
    }

    /// A frame begins, `frame_dt` long: the shake's time runs on, and a
    /// shake whose time is up stops before the frame's steps (the
    /// coroutine wakes in the frame's update, the weapon's motion runs late
    /// in it) and its strength goes back to the prefab's (`OnStop`).
    pub fn pass_frame(&mut self, frame_dt: f32) {
        if !self.on {
            return;
        }
        self.left -= frame_dt;
        if self.left <= 0.0 {
            self.on = false;
            self.strength = self.data.intensity;
        }
    }

    /// One physics step (`HandShakeEffector.Process`). `level` is how
    /// rested the hands count as (the arms' level, or the lower of the
    /// arms' and the legs' where the settings have the legs shake the hands:
    /// `Stamina::breath_stamina`); `offset` is the recoil rotation's rest
    /// point as the breath left it this step.
    pub fn step(&mut self, dt: f32, level: f32, aiming: bool, offset: [f32; 3]) -> ShakeStep {
        let d = &self.data;
        self.passed += dt;
        if d.damping {
            if self.passed >= self.fade_from {
                self.faded += dt;
                self.damping = 1.0 - (self.faded / self.fade_for).clamp(0.0, 1.0);
            }
        } else {
            self.damping = 1.0;
        }
        let level = level.clamp(d.lower_limit, d.fracture_limit) * d.energy_multiplier;
        let scale = self.strength * self.damping;
        let roll = self.hands_y.value(dt, &mut self.rng);
        let yaw = self.hands_x.value(dt, &mut self.rng);
        let hands_rotation = [(-roll).max(0.0) * (1.0 - level) * 2.0 * scale, roll * scale, yaw * scale];
        let [mut x, y, z] = [0, 1, 2].map(|i| self.tremor[i].value(dt, &mut self.rng));
        // From the hip the muzzle drops with the roll, far more the more
        // rested the hands (up to the level's top).
        if !aiming {
            x += roll.max(0.0) * (1.0 + (100.0 / d.fracture_limit - 1.0) * level.clamp(0.0, 1.0));
        }
        let wanted = [x * scale, y * scale, z * scale];
        let apart: f32 = (0..3).map(|i| (wanted[i] - offset[i]).powi(2)).sum();
        let muzzle_offset = if apart > 0.01 { std::array::from_fn(|i| offset[i] + (wanted[i] - offset[i]) * 0.1) } else { wanted };
        ShakeStep { hands_rotation, muzzle_offset }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_data::character::player::SmoothRandomData;

    /// The player prefab's figures (`probe script ... ProceduralWeaponAnimation`).
    fn data() -> HandShakeData {
        let random = SmoothRandomData { amplitude: 0.1, delay: [0.3, 0.6], hardness: 0.1 };
        HandShakeData {
            intensity: 3.0,
            lower_limit: 0.05,
            fracture_limit: 0.3,
            energy_multiplier: 1.0,
            hands_x: random,
            hands_y: random,
            tremor: [random; 3],
            damping: true,
            damping_start_percent: 50.0,
            uses_params: false,
            curves: 0,
        }
    }

    /// Whole for the first half of its time, fading to nothing by its end,
    /// and off once the frames have run its time out.
    #[test]
    fn it_fades_over_its_second_half_and_stops_with_its_time() {
        let mut shake = HandShake::new(data(), 7);
        shake.start(4.0, 3.0);
        let dt = 1.0 / 60.0;
        let at = |shake: &mut HandShake, seconds: f32| {
            for _ in 0..(seconds / dt).round() as usize {
                shake.pass_frame(dt);
                if shake.is_on() {
                    shake.step(dt, 0.05, true, [0.0; 3]);
                }
            }
            shake.damping
        };
        assert_eq!(at(&mut shake, 1.9), 1.0, "whole in its first half");
        let mid = at(&mut shake, 1.1);
        assert!((mid - 0.5).abs() < 0.02, "half way through the fade at three quarters of its time: {mid}");
        assert!(shake.is_on());
        at(&mut shake, 1.1);
        assert!(!shake.is_on(), "off once its time is up");
        assert_eq!(shake.strength, 3.0, "and its strength the prefab's again");
    }

    /// From the hip the muzzle's rest point is thrown down by the roll, the
    /// more the more rested the hands; aimed it only trembles.
    #[test]
    fn from_the_hip_the_muzzle_drops_more_with_rested_hands() {
        let reach = |level: f32, aiming: bool| {
            let mut shake = HandShake::new(data(), 11);
            shake.start(100.0, 3.0);
            let (mut offset, mut most) = ([0.0f32; 3], 0.0f32);
            for _ in 0..(20.0 * 60.0) as usize {
                offset = shake.step(1.0 / 60.0, level, aiming, offset).muzzle_offset;
                most = most.max(offset[0]);
            }
            most
        };
        let (aimed, spent, rested) = (reach(0.05, true), reach(0.05, false), reach(1.0, false));
        assert!(spent > aimed * 3.0, "the hip's drop: {spent} against {aimed} aimed");
        assert!(rested > spent * 3.0, "more as the hands count as rested (to 0.3): {rested} against {spent}");
    }
}
