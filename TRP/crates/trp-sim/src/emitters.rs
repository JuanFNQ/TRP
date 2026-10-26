//! The game's older emitters and the pool's light (`Effects.Effect.
//! ParticleSys`, `ParticleSystemAdapter`, `SimpleSparksAdapter`,
//! `LightPool`, `FastRndom`; grenades spec, "The flashbang's emitters and
//! the pool's light"): how many particles an emitter gives, which way each
//! goes, what an adapter gives each, a spark of the sparks' adapter, and a
//! pool light's fall. In the game's axes. The draws are the game's own
//! tables (`FastRndom`, made by Unity's generator at its fixed seeds:
//! `unity_random::FastRandom`) and, for a spark's place, Unity's generator;
//! `PROVISIONAL`: where in them a run starts (the game's index has moved
//! with everything else that drew from them before).

use trp_data::particles::{RandomRange, SparksAdapter};

use crate::particles::EmitParams;
use crate::sparks::SparkEmit;
use crate::unity_random::{FastRandom, UnityRandom};

/// How many particles an emitter gives (`MinCount` where `RandomCountRange`
/// is under one, else `FastRndom.Int(MinCount, RandomCountRange)`).
pub fn count(min_count: i64, count_range: i64, fast: &mut FastRandom) -> u32 {
    let n = if count_range < 1 { min_count } else { fast.int(min_count as i32, count_range as i32) as i64 };
    n.max(0) as u32
}

/// A way and how far up its place is lifted, by the emitter's kind
/// (`RandomType`): 0 the normal; 1 the normal plus a random unit way (not
/// made a unit); 2 a way over half a sphere (a point within the unit
/// sphere, its up made positive and 0.3 added to it, made a unit); 3 a way
/// across (two draws less a half each, made a unit, in the ground's plane)
/// from 0.3 m up; 4 the normal plus a way over half a sphere, made a unit;
/// 5 within 60° of the world's up (turned about the across axis by a draw
/// from −60° to 60°, then about up by a draw of a whole turn).
pub fn way(kind: i64, normal: [f32; 3], fast: &mut FastRandom) -> ([f32; 3], f32) {
    match kind {
        1 => (add(normal, fast.vector()), 0.0),
        2 => (fast.hemisphere(), 0.0),
        3 => (fast.circle(), 0.3),
        4 => (unit(add(normal, fast.hemisphere())), 0.0),
        5 => (fast.cone60(), 0.0),
        _ => (normal, 0.0),
    }
}

/// A way scaled axis by axis (`Vector3.Scale`, where `UseRandomScale`).
pub fn scaled(way: [f32; 3], scale: Option<[f32; 3]>) -> [f32; 3] {
    match scale {
        Some(s) => [way[0] * s[0], way[1] * s[1], way[2] * s[2]],
        None => way,
    }
}

/// An adapter's draws (`ParticleSystemAdapter.FillValues`): ten of each,
/// made at its start, gone through in turn by every particle it gives.
#[derive(Debug, Clone)]
pub struct AdapterDraws {
    lifetimes: [f32; 10],
    speeds: [f32; 10],
    sizes: [f32; 10],
    rotations: [f32; 10],
    seeds: [u32; 10],
    next: usize,
}

impl AdapterDraws {
    /// `FillValues`: for each of the ten in turn, its lifetime, speed and
    /// size (a draw from the floats only where the range is above the
    /// smallest float, C#'s `float.Epsilon`), its turn and its seed.
    pub fn new(lifetime: RandomRange, speed: RandomRange, size: RandomRange, fast: &mut FastRandom) -> Self {
        let mut out = Self { lifetimes: [0.0; 10], speeds: [0.0; 10], sizes: [0.0; 10], rotations: [0.0; 10], seeds: [0; 10], next: 0 };
        let draw = |r: RandomRange, fast: &mut FastRandom| if r.range > f32::from_bits(1) { r.min + fast.float() * r.range } else { r.min };
        for i in 0..10 {
            out.lifetimes[i] = draw(lifetime, fast);
            out.speeds[i] = draw(speed, fast);
            out.sizes[i] = draw(size, fast);
            out.rotations[i] = fast.rotation();
            out.seeds[i] = fast.uint();
        }
        out
    }

    /// The next particle (`ParticleSystemAdapter.Emit`): at `position`, its
    /// velocity the way times its speed, its lifetime and size, its turn
    /// (the drawn one where `rotation_random`, else `rotation`), its colour
    /// the adapter's.
    pub fn emit(&mut self, position: [f32; 3], way: [f32; 3], rotation_random: bool, rotation: f32, color: [u8; 4]) -> EmitParams {
        let i = self.next;
        self.next = (self.next + 1) % 10;
        let speed = self.speeds[i];
        EmitParams {
            position,
            velocity: [way[0] * speed, way[1] * speed, way[2] * speed],
            size: self.sizes[i],
            lifetime: self.lifetimes[i],
            color,
            rotation: if rotation_random { self.rotations[i] } else { rotation },
            seed: self.seeds[i],
        }
    }
}

/// A spark of the sparks' adapter (`SimpleSparksAdapter.Emit`): one of its
/// 128 places drawn (Unity's `Random.Range(0, 128)`), its lifetime, speed
/// (times the way), gravity, drag and four bytes from there, born at `now`
/// plus the adapter's shift.
pub fn spark(adapter: &SparksAdapter, position: [f32; 3], way: [f32; 3], now: f32, unity: &mut UnityRandom) -> SparkEmit {
    let n = adapter.lifetime.len().min(adapter.speed.len()).max(1);
    let i = (unity.range_int(0, 128) as usize).min(n - 1);
    let at_f = |table: &[f32]| table.get(i).copied().unwrap_or(0.0);
    let at_b = |table: &[u8]| table.get(i).copied().unwrap_or(255);
    let speed = at_f(&adapter.speed);
    SparkEmit {
        position,
        velocity: [way[0] * speed, way[1] * speed, way[2] * speed],
        time: now + adapter.time_shift,
        gravity: at_f(&adapter.gravity),
        drag: at_f(&adapter.drag),
        lifetime: at_f(&adapter.lifetime),
        emission: at_b(&adapter.emission),
        size: at_b(&adapter.size),
        turbulence: at_b(&adapter.turbulence),
        frequency: at_b(&adapter.frequency),
    }
}

/// A light of the pool (`LightPool.LightStruct`): where, its colour, its
/// range and intensity as given and its time, its shadows, and how long it
/// has been on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoolLight {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub range: f32,
    pub intensity: f32,
    pub time: f32,
    pub shadows: bool,
    pub age: f32,
}

impl PoolLight {
    /// Its range and intensity now, each falling straight from what it was
    /// given to nothing over its time (`Update`: less what was given over
    /// the time, times the frame's time), or none once its time is past.
    pub fn now(&self) -> Option<(f32, f32)> {
        if self.age > self.time || self.time <= 0.0 {
            return None;
        }
        let left = 1.0 - self.age / self.time;
        Some((self.range * left, self.intensity * left))
    }

    pub fn step(&mut self, dt: f32) {
        self.age += dt;
    }
}


fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l > 1e-12 {
        a.map(|c| c / l)
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The counts: `MinCount` alone where the range is under one, else
    /// from `MinCount` to one short of `MinCount` plus the range.
    #[test]
    fn an_emitter_s_count() {
        let mut rng = FastRandom::new();
        assert!((0..100).all(|_| count(1, 0, &mut rng) == 1));
        let seen: Vec<u32> = (0..2000).map(|_| count(5, 10, &mut rng)).collect();
        assert_eq!((seen.iter().min(), seen.iter().max()), (Some(&5), Some(&14)));
    }

    /// Each kind of way: the normal; the normal plus a unit way (its length
    /// up to 2); half a sphere's ways all at least 0.3 up before being made
    /// units (so above the ground's plane); across, lifted 0.3 m; within
    /// 60° of up.
    #[test]
    fn the_ways_of_each_kind() {
        let mut rng = FastRandom::new();
        let up = [0.0, 1.0, 0.0];
        assert_eq!(way(0, up, &mut rng), (up, 0.0));
        for _ in 0..500 {
            let (w, _) = way(1, up, &mut rng);
            let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
            assert!(l <= 2.0 + 1e-5, "{w:?}");
            let (h, _) = way(2, up, &mut rng);
            assert!(h[1] > 0.0 && ((h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt() - 1.0).abs() < 1e-4, "{h:?}");
            let (c, lift) = way(3, up, &mut rng);
            assert!(c[1] == 0.0 && lift == 0.3 && ((c[0] * c[0] + c[2] * c[2]).sqrt() - 1.0).abs() < 1e-4);
            let (k, _) = way(5, up, &mut rng);
            assert!(k[1] >= 60f32.to_radians().cos() - 1e-5, "{k:?}");
        }
    }

    /// An adapter's ten draws in turn, the eleventh the first again; its
    /// fixed turn where the turn is not drawn.
    #[test]
    fn an_adapter_goes_through_its_ten_draws() {
        let mut rng = FastRandom::new();
        let range = |min, range| RandomRange { min, range };
        let mut draws = AdapterDraws::new(range(0.5, 1.3), range(7.0, 7.0), range(1.0, 0.0), &mut rng);
        let first: Vec<EmitParams> = (0..10).map(|_| draws.emit([0.0; 3], [0.0, 1.0, 0.0], true, 0.0, [255; 4])).collect();
        let again = draws.emit([0.0; 3], [0.0, 1.0, 0.0], true, 0.0, [255; 4]);
        assert_eq!(again, first[0]);
        assert!(first.iter().all(|p| (0.5..=1.8).contains(&p.lifetime) && (7.0..=14.0).contains(&p.velocity[1]) && p.size == 1.0));
        assert_eq!(draws.emit([0.0; 3], [0.0, 1.0, 0.0], false, 12.0, [255; 4]).rotation, 12.0);
    }

    /// The pool's light falls straight to nothing over its time, then is off.
    #[test]
    fn a_pool_light_falls_over_its_time() {
        let mut light = PoolLight { position: [0.0; 3], color: [1.0; 4], range: 35.0, intensity: 12.0, time: 1.0, shadows: true, age: 0.0 };
        assert_eq!(light.now(), Some((35.0, 12.0)));
        light.step(0.25);
        assert_eq!(light.now(), Some((26.25, 9.0)));
        light.step(0.8);
        assert_eq!(light.now(), None);
    }
}
