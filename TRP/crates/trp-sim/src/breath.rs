//! Breathing, as specified in `docs/spec/weapon-handling.md`: the slow
//! rise and fall of the weapon, small random pushes on it, and the wander
//! of the muzzle when not aiming, which a shot from the hip widens.
//!
//! Tiredness and a held breath are in: the hands' and the breath's
//! levels come from the stamina. What tremor, a broken arm, a mounted
//! weapon and a stiff draw change is not in yet.

use trp_data::curve::Curve;
use trp_data::player::{BreathData, SmoothRandomData};

use crate::hands::Blender;
use crate::recoil::Rng;

/// Where the wander's turn rate rests between shots.
const RESTING_HARDNESS: f32 = 0.1;
/// How strong the breath and the random pushes are with the breath held.
const HELD: f32 = 0.15;

/// What the breath goes by besides the weapon: how rested the hands count
/// as (`Stamina::breath_stamina`), how much breath there is
/// (`Stamina::breath_oxygen`), and whether it is being held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wind {
    pub stamina: f32,
    pub oxygen: f32,
    pub held: bool,
}

impl Default for Wind {
    /// Rested, with breath to spare.
    fn default() -> Self {
        Self { stamina: 1.0, oxygen: 1.0, held: false }
    }
}

/// A value that picks a new random target every so often and turns toward it.
#[derive(Debug, Clone)]
struct SmoothRandom {
    amplitude: f32,
    delay: [f32; 2],
    hardness: f32,
    count_down: f32,
    current: f32,
    increase: f32,
}

impl SmoothRandom {
    fn new(data: SmoothRandomData) -> Self {
        Self {
            amplitude: data.amplitude,
            delay: data.delay,
            hardness: data.hardness,
            count_down: 0.0,
            current: 0.0,
            increase: 0.0,
        }
    }

    fn value(&mut self, dt: f32, rng: &mut Rng) -> f32 {
        self.count_down -= dt;
        if self.count_down < 0.0 {
            self.count_down = rng.range(self.delay[0], self.delay[1]);
            let wanted = (rng.range(-self.amplitude, self.amplitude) - self.current) / self.count_down;
            self.increase += (wanted - self.increase) * self.hardness;
        }
        self.current += self.increase * dt;
        self.current
    }
}

/// What one step of breathing asks for.
#[derive(Debug, Clone, Default)]
pub struct BreathStep {
    /// Pushes on the springs: target (0 camera rotation, 1 hands position,
    /// 2 hands rotation), component, value.
    pub pushes: Vec<(u8, u8, f32)>,
    /// Where the recoil rotation's rest point should be.
    pub muzzle_offset: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct Breath {
    data: BreathData,
    x: SmoothRandom,
    y: SmoothRandom,
    hip_x: SmoothRandom,
    hip_z: SmoothRandom,
    /// Clock of each breath curve.
    clocks: Vec<f32>,
    /// How much shots from the hip have widened the wander.
    gain: Blender,
    hardness: Blender,
    gain_per_shot: f32,
    gain_delay: f32,
    muzzle_offset: [f32; 3],
    /// How fast the breath's clock ran in the last step that was not
    /// held, and how much of the breath reached the camera: a held breath
    /// leaves both as they were.
    breath_rate: f32,
    camera_share: f32,
    rng: Rng,
}

impl Breath {
    /// `gain` is what a hip shot adds, how long it holds and how fast it
    /// fades, as the weapon gives them.
    pub fn new(data: BreathData, gain: [f32; 3], seed: u64) -> Self {
        let mut hardness = Blender::new(0.0, data.hardness_speed);
        hardness.set_target(RESTING_HARDNESS, 0.0);
        Self {
            x: SmoothRandom::new(data.x),
            y: SmoothRandom::new(data.y),
            hip_x: SmoothRandom::new(data.hip_x),
            hip_z: SmoothRandom::new(data.hip_z),
            clocks: data.curves.iter().map(|c| c.time_shift).collect(),
            gain: Blender::new(0.0, gain[2]),
            hardness,
            gain_per_shot: gain[0],
            gain_delay: gain[1],
            muzzle_offset: [0.0; 3],
            breath_rate: 0.0,
            camera_share: 0.0,
            rng: Rng::new(seed),
            data,
        }
    }

    /// How long the breath takes to come round, in seconds, at rest: the
    /// longest of its curves' loops.
    pub(crate) fn period(&self) -> f32 {
        let span = |c: &Curve| c.keys.last().map_or(0.0, |last| last.time - c.keys[0].time);
        self.data.curves.iter().map(|entry| span(&entry.curve)).fold(0.0, f32::max)
    }

    /// A shot was fired. From the hip it widens the wander and throws the
    /// muzzle's rest point.
    pub fn on_shot(&mut self, now: f32, intensity: f32, aiming: bool) {
        if aiming {
            return;
        }
        let mut gain = self.gain.value(now);
        gain += (self.gain_per_shot - 0.2 * (gain.sqrt() - 1.0)).max(0.0);
        self.gain.jump(gain, self.gain_delay, now);
        self.hardness.jump(self.data.shot_hardness, 0.0, now);
        let base = self.data.base_hip_amplitudes;
        self.hip_x.amplitude = (base[0] + gain).clamp(0.0, 3.0);
        self.hip_z.amplitude = (base[1] + gain).clamp(0.0, 3.0);
        self.hip_x.hardness = self.data.shot_hardness;
        self.hip_z.hardness = self.data.shot_hardness;
        let between = self.data.random_between;
        let step = self.rng.range(between[0], between[1]);
        let up = self.hip_x.value(step, &mut self.rng);
        let step = self.rng.range(between[0], between[1]);
        let aside = self.hip_z.value(step, &mut self.rng);
        let scale = intensity * self.data.hip_penalty;
        self.muzzle_offset = [self.muzzle_offset[0].min(up) * scale, 0.0, aside / 2.0 * scale];
    }

    /// One physics step. `intensity` is the stance's factor times the
    /// aiming factor; `head_bobbing` is the player's setting; `wind` is
    /// how tired the character is and whether it holds its breath.
    pub fn step(&mut self, dt: f32, now: f32, intensity: f32, aiming: bool, head_bobbing: f32, wind: Wind) -> BreathStep {
        let (stamina, oxygen) = (wind.stamina, wind.oxygen);
        let mut out = BreathStep::default();

        let widened = self.gain.value(now).sqrt();
        let base = self.data.base_hip_amplitudes;
        self.hip_x.amplitude = (base[0] + widened).clamp(0.0, 3.0);
        self.hip_z.amplitude = (base[1] + widened).clamp(0.0, 3.0);
        let hardness = self.hardness.value(now);
        self.hip_x.hardness = hardness;
        self.hip_z.hardness = hardness;

        // Short of oxygen the breath is deeper and faster and reaches the
        // camera. Held, it and the random pushes are a small part of
        // themselves, whatever the stance, and its pace and its reach to
        // the camera stay as the last breath had them.
        let (breath_intensity, shake) = if wind.held {
            (HELD, HELD)
        } else {
            let t = self.data.lack_of_oxygen.evaluate(oxygen);
            let rest = if aiming { 0.75 } else { 1.0 };
            self.breath_rate = (4.0 + (1.0 - 4.0) * t).clamp(1.0, 2.5) * dt;
            self.camera_share = (2.0 - 2.0 * t) * intensity;
            ((4.0 + (rest - 4.0) * t).clamp(1.0, 1.5) * intensity, 1.0)
        };
        let (breath_rate, camera_share) = (self.breath_rate, self.camera_share);

        // Small random pushes on the weapon's roll and yaw.
        self.y.amplitude = self.data.amplitude_by_stamina.evaluate(stamina);
        let delay = self.data.delay_by_stamina.evaluate(stamina);
        self.x.delay = [delay / 2.0, delay];
        self.y.delay = [delay / 2.0, delay];
        self.y.hardness = self.data.hardness_by_stamina.evaluate(stamina);
        let roll = self.y.value(dt, &mut self.rng);
        let yaw = self.x.value(dt, &mut self.rng);
        let tired = (-roll).max(0.0) * (1.0 - stamina) * 2.0;
        for (component, value) in [tired, roll, yaw].into_iter().enumerate() {
            out.pushes.push((2, component as u8, value * shake * intensity));
        }

        // The muzzle wanders when the weapon is not at the shoulder.
        let wander = if aiming {
            [0.0; 3]
        } else {
            let scale = intensity * self.data.hip_penalty;
            [self.hip_x.value(dt, &mut self.rng) * scale, 0.0, self.hip_z.value(dt, &mut self.rng) * scale]
        };
        let apart: f32 = (0..3).map(|i| (wander[i] - self.muzzle_offset[i]).powi(2)).sum();
        self.muzzle_offset = if apart > 0.01 {
            std::array::from_fn(|i| self.muzzle_offset[i] + (wander[i] - self.muzzle_offset[i]) * 0.1)
        } else {
            wander
        };
        out.muzzle_offset = self.muzzle_offset;

        // The breath itself.
        for (index, entry) in self.data.curves.iter().enumerate() {
            let strength = if index == 0 { breath_intensity } else { breath_intensity * camera_share };
            self.clocks[index] += breath_rate;
            let value = looped(&entry.curve, self.clocks[index]) * strength;
            if value.abs() < 1e-6 {
                continue;
            }
            for val in &entry.vals {
                let setting = if val.target == 0 { head_bobbing } else { 1.0 };
                out.pushes.push((val.target, val.component, value * val.intensity * setting));
            }
        }
        out
    }
}

/// Evaluates a curve that repeats after its last key.
fn looped(curve: &Curve, time: f32) -> f32 {
    let (Some(first), Some(last)) = (curve.keys.first(), curve.keys.last()) else { return 0.0 };
    let span = last.time - first.time;
    if span <= 0.0 {
        return first.value;
    }
    curve.evaluate(first.time + (time - first.time).rem_euclid(span))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wandering_value_stays_near_its_amplitude() {
        let mut r = SmoothRandom::new(SmoothRandomData { amplitude: 1.0, delay: [0.2, 0.7], hardness: 0.1 });
        let mut rng = Rng::new(3);
        let (mut low, mut high) = (0.0f32, 0.0f32);
        for _ in 0..60 * 120 {
            let v = r.value(1.0 / 60.0, &mut rng);
            low = low.min(v);
            high = high.max(v);
        }
        assert!(high > 0.2 && low < -0.2, "it does not move: {low}..{high}");
        assert!(high < 2.5 && low > -2.5, "it runs away: {low}..{high}");
    }
}
