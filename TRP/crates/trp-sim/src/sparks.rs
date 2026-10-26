//! The game's sparks (`SimpleSparksRenderer`; firing spec, "The muzzle's
//! sparks"): each spark emitted is kept in a ring of slots as the game
//! keeps it, with what it was born with, and drawn as a streak whose
//! corners the shader moves (`simple_sparks.wgsl`) by the time since it
//! was born. Here: the ring, the corners' layout of one streak, and the
//! flight's sum the shader makes (without its turbulence), for tests.

use crate::recoil::Rng;

/// What a spark is born with (`EmitSeg`): its place and velocity (the
/// game's axes), the time it was born at, its gravity, drag and lifetime,
/// and four bytes: emission, size, turbulence, frequency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SparkEmit {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub time: f32,
    pub gravity: f32,
    pub drag: f32,
    pub lifetime: f32,
    pub emission: u8,
    pub size: u8,
    pub turbulence: u8,
    pub frequency: u8,
}

/// One corner of a streak (`Fill`, and `Awake`'s shares): its picture's
/// place, its offset along (−1 at the start, 0 between, 1 at the end) and
/// across (−1, 1), and its share of the streak's time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Corner {
    pub uv: [f32; 2],
    pub offset: [f32; 2],
    pub share: f32,
}

/// The corners of one streak of `segments`: two at each of its
/// `segments + 1` places, the ends' pushed out along it.
pub fn corners(segments: u32) -> Vec<Corner> {
    let places = segments as usize + 1;
    let share = |j: usize| (j.saturating_sub(1).min(segments as usize - 2)) as f32 / (segments - 2).max(1) as f32;
    (0..places)
        .flat_map(|j| {
            let (u, along) = match j {
                0 => (0.0, -1.0),
                _ if j == places - 1 => (1.0, 1.0),
                _ => (0.5, 0.0),
            };
            [Corner { uv: [u, 0.0], offset: [along, -1.0], share: share(j) }, Corner { uv: [u, 1.0], offset: [along, 1.0], share: share(j) }]
        })
        .collect()
}

/// The triangles of one streak, by its corners' numbers.
pub fn indices(segments: u32) -> Vec<u32> {
    (0..segments)
        .flat_map(|j| {
            let a = j * 2;
            let (b, c, d) = (a + 1, a + 2, a + 3);
            [a, c, d, d, b, a]
        })
        .collect()
}

/// The ring of sparks (`Count` slots): each new one in the next slot, the
/// oldest overwritten; each slot's own draw (`Random.value` at `Awake`).
#[derive(Debug, Clone)]
pub struct SparksRing {
    pub segments: u32,
    slots: Vec<Option<SparkEmit>>,
    randoms: Vec<f32>,
    next: usize,
}

impl SparksRing {
    pub fn new(count: u32, segments: u32, rng: &mut Rng) -> Self {
        let count = count.max(1) as usize;
        Self { segments: segments.max(2), slots: vec![None; count], randoms: (0..count).map(|_| rng.unit()).collect(), next: 0 }
    }

    pub fn emit(&mut self, spark: SparkEmit) {
        self.slots[self.next] = Some(spark);
        self.next = (self.next + 1) % self.slots.len();
    }

    /// The sparks a frame from `last` to `now` may show (the shader draws
    /// a streak over at least a thirtieth of a second), each with its
    /// slot's draw.
    pub fn live(&self, now: f32, last: f32) -> impl Iterator<Item = (&SparkEmit, f32)> + '_ {
        let span = (now - last).max(1.0 / 30.0);
        self.slots.iter().zip(&self.randoms).filter_map(move |(slot, &random)| slot.as_ref().filter(|s| now - s.time > 0.0 && now - span - s.time < s.lifetime).map(|s| (s, random)))
    }
}

/// Where a spark is `age` seconds after it was born, as the shader has it
/// without its turbulence: its velocity slowed by `drag` to the power of
/// the time (summed), less `gravity` times the time squared in height.
pub fn place(spark: &SparkEmit, age: f32) -> [f32; 3] {
    let ln = spark.drag.ln();
    let gone = if ln.is_finite() && ln.abs() > 1e-6 { (spark.drag.powf(age) - 1.0) / ln } else { age };
    let mut at = std::array::from_fn(|k| spark.position[k] + spark.velocity[k] * gone);
    at[1] -= spark.gravity * age * age;
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spark(time: f32) -> SparkEmit {
        SparkEmit { position: [0.0, 1.5, 0.0], velocity: [0.0, 0.0, 10.0], time, gravity: 0.4, drag: 0.0016, lifetime: 0.5, emission: 255, size: 13, turbulence: 27, frequency: 134 }
    }

    #[test]
    fn a_streak_s_corners_are_the_game_s() {
        // `Fill` for 12 segments: 26 corners, the ends' pushed out along,
        // the rest's picture in the middle; the shares of `Awake`.
        let c = corners(12);
        assert_eq!(c.len(), 26);
        assert_eq!((c[0].uv, c[0].offset, c[1].uv, c[1].offset), ([0.0, 0.0], [-1.0, -1.0], [0.0, 1.0], [-1.0, 1.0]));
        assert_eq!((c[24].uv, c[24].offset, c[25].offset), ([1.0, 0.0], [1.0, -1.0], [1.0, 1.0]));
        assert_eq!((c[2].uv, c[2].offset), ([0.5, 0.0], [0.0, -1.0]));
        let shares: Vec<f32> = c.iter().step_by(2).map(|c| c.share).collect();
        assert_eq!(shares[0], 0.0);
        assert_eq!(shares[1], 0.0);
        assert!((shares[2] - 0.1).abs() < 1e-6);
        assert_eq!((shares[11], shares[12]), (1.0, 1.0));
        let i = indices(12);
        assert_eq!(i.len(), 72);
        assert_eq!(&i[..6], &[0, 2, 3, 3, 1, 0]);
        assert!(i.iter().all(|&n| n < 26));
    }

    #[test]
    fn the_ring_keeps_the_newest_and_shows_the_living() {
        let mut ring = SparksRing::new(3, 12, &mut Rng::new(1));
        for k in 0..4 {
            ring.emit(spark(k as f32));
        }
        // The fourth took the first's slot.
        let times: Vec<f32> = ring.slots.iter().flatten().map(|s| s.time).collect();
        assert_eq!(times, vec![3.0, 1.0, 2.0]);
        // At 3.2 only the newest is alive; not before it is born, and
        // still for a frame's streak after its end.
        assert_eq!(ring.live(3.2, 3.19).count(), 1);
        assert_eq!(ring.live(2.99, 2.98).filter(|(s, _)| s.time == 3.0).count(), 0);
        assert_eq!(ring.live(3.52, 3.51).count(), 1);
        assert_eq!(ring.live(3.6, 3.59).count(), 0);
    }

    #[test]
    fn a_spark_flies_slowing_and_falls() {
        let s = spark(0.0);
        let at = |t| place(&s, t);
        assert_eq!(at(0.0), [0.0, 1.5, 0.0]);
        // Slowed: each tenth of a second it goes less far.
        let (a, b, c) = (at(0.1)[2], at(0.2)[2], at(0.3)[2]);
        assert!(a > 0.0 && b - a < a && c - b < b - a, "{a} {b} {c}");
        // The velocity summed: (drag^t − 1) / ln drag.
        assert!((a - 10.0 * (0.0016f32.powf(0.1) - 1.0) / 0.0016f32.ln()).abs() < 1e-5);
        assert!((at(0.3)[1] - (1.5 - 0.4 * 0.09)).abs() < 1e-6);
    }
}
