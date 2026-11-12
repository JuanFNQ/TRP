//! Smoke draws, fume puffs, sparks and the barrel haze.

use super::*;

/// `FastRndom.Int(input, start, range)`: the number shifted left 7 and
/// xored, right 3 and xored, its size modulo the range, from the start.
pub(super) fn hashed(input: i32, start: i32, range: i32) -> i32 {
    if range == 0 {
        return start;
    }
    let mut n = input;
    n ^= n.wrapping_shl(7);
    n ^= n >> 3;
    start + n.wrapping_abs() % range
}

/// `MuzzleSmoke.GetRnd`: between the draws of a place's whole number and
/// the next, by a smooth step.
pub fn smoke_draw(t: f32) -> f32 {
    let n = t as i32;
    let a = hashed(n.wrapping_add(8_324_234), -1000, 2000) as f32 * 0.001;
    let b = hashed(n.wrapping_add(1).wrapping_add(8_324_234), -1000, 2000) as f32 * 0.001;
    let f = t - n as f32;
    let f = f * f * (3.0 - 2.0 * f);
    a + (b - a) * f
}

/// A shot's puffs from one fume (`MuzzleFume.Emit` and the manager's
/// `IFumeEmitter.Emit`): its object's place and forward (minus its up), and
/// how far the object has moved since its place was noted, in the game's
/// axes. `rng` is the engine's own draw for the size (`Random.Range`).
pub fn fume_puffs(fume: &FumeData, place: [f32; 3], forward: [f32; 3], moved: [f32; 3], fast: &mut FastRandom, rng: &mut Rng) -> Vec<crate::effects::particles::EmitParams> {
    let start = add(place, scale(forward, fume.start_pos));
    let count = if fume.count_range < 1 { fume.count_min } else { fast.int(fume.count_min, fume.count_range) };
    (0..count.max(0))
        .map(|_| {
            let mut position = start;
            if fume.emitter_radius >= f32::EPSILON {
                position = add(position, scale(fast.vector(), fume.emitter_radius));
            }
            let time = fast.float();
            let way = add(forward, scale(fast.vector(), fume.conus_size));
            let length = square(way).sqrt();
            let way = if length > 1e-6 { scale(way, 1.0 / length) } else { [0.0; 3] };
            let velocity = add(scale(moved, 0.5), scale(way, fume.speeds.evaluate(time) * fume.speed));
            let size = (fume.sizes.evaluate(time) + rng.range(-fume.sizes_rnd, fume.sizes_rnd)) * fume.size;
            let lifetime = fume.lifetimes.evaluate(time) * fume.lifetime;
            // `Color32` of the colour: each channel held to 0..1, times 255,
            // rounded.
            let color = fume.color.evaluate(time).map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            // The manager's own: a seed, then the turn.
            let seed = fast.uint();
            let rotation = fast.rotation();
            crate::effects::particles::EmitParams { position, velocity, size, lifetime, color, rotation, seed }
        })
        .collect()
}

/// A shot's sparks from one muzzle's (`MuzzleSparks.Emit`): its object's
/// place and forward (minus its up) in the game's axes, at the time `now`.
/// `rng` is the engine's own draw of the tables' entry
/// (`Random.Range(0, 128)`).
pub fn spark_emits(sparks: &SparksData, place: [f32; 3], forward: [f32; 3], now: f32, fast: &mut FastRandom, rng: &mut Rng) -> Vec<crate::effects::sparks::SparkEmit> {
    let start = add(place, scale(forward, sparks.start_pos));
    let count = if sparks.count_range < 1 { sparks.count_min } else { fast.int(sparks.count_min, sparks.count_range) };
    let time = now + sparks.emission_time_shift;
    (0..count.max(0))
        .map(|_| {
            let mut position = start;
            if sparks.emitter_radius >= f32::EPSILON {
                position = add(position, scale(fast.vector(), sparks.emitter_radius));
            }
            let way = add(forward, scale(fast.vector(), sparks.conus_size));
            let length = square(way).sqrt();
            let way = if length > 1e-6 { scale(way, 1.0 / length) } else { [0.0; 3] };
            // Each table has 128 (`OnValidate`); held within the shortest.
            let shortest = [sparks.lifetime.len(), sparks.speed.len(), sparks.gravity.len(), sparks.drag.len(), sparks.emission.len(), sparks.size.len(), sparks.turbulence.len(), sparks.frequency.len()].into_iter().min().unwrap_or(0);
            let at = ((rng.unit() * 128.0) as usize).min(shortest.saturating_sub(1));
            crate::effects::sparks::SparkEmit {
                position,
                velocity: scale(way, sparks.speed[at]),
                time,
                gravity: sparks.gravity[at],
                drag: sparks.drag[at],
                lifetime: sparks.lifetime[at],
                emission: sparks.emission[at],
                size: sparks.size[at],
                turbulence: sparks.turbulence[at],
                frequency: sparks.frequency[at],
            }
        })
        .collect()
}

/// Whether a shot has this haze (`HeatHazeEmitter.OnShot`: a draw from
/// 0.001 to 1 not above its chance; none with no puffs). Its puffs come
/// its delay later (`haze_puffs`).
pub fn haze_shot(haze: &HazeData, rng: &mut Rng) -> bool {
    haze.count > 0 && rng.range(0.001, 1.0) <= haze.chance
}

/// A haze's puffs (`HeatHazeEmitter.Emit`), its delay after the shot: its
/// object's place and its place from its parent in the parent's turn
/// (`rotation × inverse(localRotation) × localPosition`), in the game's
/// axes. One size for all; each its own velocity (that place plus a draw in
/// a ball of `Angle`, times `VelocityFactor`), half the size ahead along it,
/// a turn of 0 to 89 degrees, a lifetime between its two; white.
pub fn haze_puffs(haze: &HazeData, place: [f32; 3], from_parent: [f32; 3], rng: &mut Rng) -> Vec<crate::effects::particles::EmitParams> {
    let size = rng.range(haze.size[0], haze.size[1]);
    (0..haze.count.max(0))
        .map(|_| {
            let ball = loop {
                let v = [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)];
                if square(v) <= 1.0 {
                    break v;
                }
            };
            let velocity = scale(add(from_parent, scale(ball, haze.angle)), haze.velocity_factor);
            let rotation = (rng.unit() * 90.0).floor().min(89.0);
            let length = square(velocity).sqrt();
            let ahead = if length > 1e-6 { scale(velocity, size * 0.5 / length) } else { [0.0; 3] };
            let lifetime = rng.range(haze.lifetime[0], haze.lifetime[1]);
            crate::effects::particles::EmitParams { position: add(place, ahead), velocity, size, lifetime, color: [255; 4], rotation, seed: rng.unit().to_bits() }
        })
        .collect()
}

pub(super) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub(super) fn square(a: [f32; 3]) -> f32 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}

/// How far from the camera a weapon's hot parts still shimmer: none past
/// 5 m from its prefab (`HotObject.NeedProcessEffects`, a square of 25).
pub const BARREL_HAZE_REACH: f32 = 5.0;

/// The fewest a step of the count's curve that shimmers at all
/// (`HotObject.MinHeatParticleCount`).
const BARREL_HAZE_LEAST: f32 = 4.0;

/// The share of a part's own velocity its shimmer is born with
/// (`HotObject.HeatHazeVelocityFactor`).
const BARREL_HAZE_VELOCITY: f32 = 0.5;

/// A hot part's shimmer (`HotObject` with `UseHeatHaze`; malfunctions spec,
/// "The shimmer over a hot barrel"): the time it last emitted, and the
/// figures the heat it was last given set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarrelHaze {
    pub last_emit: f32,
    pub count: f32,
    pub life: f32,
    pub size: f32,
}

/// One particle of a shimmer: where, how fast, how large, how long.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HazePuff {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub size: f32,
    pub lifetime: f32,
}

/// The heat's share the shimmer's curves are read at: the part's
/// temperature (`HotObject.ConvertHeat2Celsio`, `10 heat / 220 + 30`) less
/// 30 over 5.6 (`ConvertCelsioHeatHaze`), so the heat over 123.2.
pub fn barrel_haze_share(heat: f32) -> f32 {
    let celsius = 10.0 * heat / 220.0 + 30.0;
    (celsius - 30.0).max(0.0) / 5.599_998_5
}

impl BarrelHaze {
    /// Woken at `now` (`Awake`): its last emission then, and its
    /// serialized 29 degrees given, which is no heat.
    pub fn woken(now: f32, haze: &trp_data::weapons::heat::HotHaze) -> Self {
        let mut this = Self { last_emit: now, count: 0.0, life: 0.0, size: 0.0 };
        this.set(haze, 0.0);
        this
    }

    /// A heat given (`SetTemperatureToRenderer`, from the weapon's
    /// `ManualUpdate` once it has moved a hundredth): the curves read at
    /// its share.
    pub fn give(&mut self, haze: &trp_data::weapons::heat::HotHaze, heat: f32) {
        self.set(haze, barrel_haze_share(heat));
    }

    fn set(&mut self, haze: &trp_data::weapons::heat::HotHaze, share: f32) {
        self.count = haze.count.evaluate(share);
        self.life = haze.life.evaluate(share);
        self.size = haze.size.evaluate(share);
    }

    /// A frame's shimmer (`ManualSyncEffects`): nothing below four a step;
    /// else for each whole step of a count's share of a second since it
    /// last emitted, the count's whole number of particles, each at an even
    /// draw within its box in the part's own axes (`place` takes it to the
    /// world), with half the part's own velocity, its size, and its life
    /// the curve's and a draw between the two more; its last emission moved
    /// on by those steps. The game's loop has no end but the system's room
    /// (200 for `HeatEffect`): at most `most` are made here.
    pub fn puffs(&mut self, haze: &trp_data::weapons::heat::HotHaze, now: f32, place: impl Fn([f32; 3]) -> [f32; 3], own_velocity: [f32; 3], most: usize, rng: &mut Rng) -> Vec<HazePuff> {
        if self.count < BARREL_HAZE_LEAST {
            return Vec::new();
        }
        let step = 1.0 / self.count;
        let steps = ((now - self.last_emit) / step).floor().max(0.0);
        let made = ((self.count as i64).max(0) as f32 * steps).min(most as f32) as usize;
        let velocity = scale(own_velocity, BARREL_HAZE_VELOCITY);
        let low = sub(haze.center, haze.extent);
        let high = add(haze.center, haze.extent);
        let puffs = (0..made)
            .map(|_| {
                let at = [rng.range(low[0], high[0]), rng.range(low[1], high[1]), rng.range(low[2], high[2])];
                let lifetime = self.life + rng.range(haze.lifetime_delta[0], haze.lifetime_delta[1]);
                HazePuff { position: place(at), velocity, size: self.size, lifetime }
            })
            .collect();
        self.last_emit += steps * step;
        puffs
    }
}
