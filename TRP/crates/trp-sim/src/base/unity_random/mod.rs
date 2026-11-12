//! Unity's scripting random numbers as the engine makes them, and the game's tables filled from them.
//! The engine's `UnityEngine.Random`.
//!
//! Read from the engine itself (`UnityPlayer.dll` with Unity's public
//! symbols: findings, "Unity's random numbers"): a xorshift128 of four
//! 32-bit words. `InitState(seed)` puts the seed in the first word and each
//! next word is the one before times 1812433253, plus one. A draw shifts
//! the words along and makes the last from the first and the old last. A
//! number between 0 and 1 is the draw's low 23 bits times the engine's
//! constant (the 32-bit float with bits `0x34000001`, a hair above
//! 1 / 8388608), so 0 and 1 can both come. `Range(a, b)` for numbers is
//! `(1 − t) b + t a` of one such number `t`; for whole numbers it is `a`
//! plus the draw modulo `b − a` (below `a` the other way, and `a` itself
//! when the two are equal). A direction (`onUnitSphere`) takes two: its
//! height `1 − 2 t₁`, its turn about the height `(1 − t₂) 2π`; a point in
//! the ball (`insideUnitSphere`) is such a direction times a third number's
//! cube root.

/// The engine's own constants, kept as bits so that every draw is the
/// engine's to the last bit.
const UNIT: u32 = 0x3400_0001;
const TWO_PI: u32 = 0x40c9_0fdb;
const THIRD: u32 = 0x3eaa_aaab;

/// Unity's generator, from one seed.
#[derive(Debug, Clone)]
pub struct UnityRandom {
    words: [u32; 4],
}

impl UnityRandom {
    /// `Random.InitState(seed)`.
    pub fn new(seed: i32) -> Self {
        let mut words = [seed as u32, 0, 0, 0];
        for i in 1..4 {
            words[i] = words[i - 1].wrapping_mul(1_812_433_253).wrapping_add(1);
        }
        Self { words }
    }

    /// `Random.state` set: its four words as the engine keeps them (the
    /// next draw shifts the first out).
    pub fn from_state(words: [u32; 4]) -> Self {
        Self { words }
    }

    fn draw(&mut self) -> u32 {
        let first = self.words[0] ^ (self.words[0] << 11);
        let last = self.words[3];
        self.words = [self.words[1], self.words[2], last, last ^ (last >> 19) ^ first ^ (first >> 8)];
        self.words[3]
    }

    /// `Random.value`.
    pub fn value(&mut self) -> f32 {
        (self.draw() & 0x7f_ffff) as f32 * f32::from_bits(UNIT)
    }

    /// `Random.Range(min, max)` for numbers.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        let t = self.value();
        (1.0 - t) * max + t * min
    }

    /// `Random.Range(min, max)` for whole numbers: `max` itself never comes.
    pub fn range_int(&mut self, min: i32, max: i32) -> i32 {
        if min < max {
            min.wrapping_add((self.draw() % (max.wrapping_sub(min) as u32)) as i32)
        } else if max < min {
            min.wrapping_sub((self.draw() % (min.wrapping_sub(max) as u32)) as i32)
        } else {
            min
        }
    }

    /// `Random.onUnitSphere`, in Unity's axes.
    pub fn on_unit_sphere(&mut self) -> [f32; 3] {
        let t = self.value();
        let height = (1.0 - t) - t;
        let u = self.value();
        let turn = (1.0 - u) * f32::from_bits(TWO_PI) + u * 0.0;
        let across = (1.0 - height * height).sqrt();
        [turn.cos() * across, turn.sin() * across, height]
    }

    /// `Random.insideUnitSphere`, in Unity's axes.
    pub fn inside_unit_sphere(&mut self) -> [f32; 3] {
        let [x, y, z] = self.on_unit_sphere();
        let reach = self.value().powf(f32::from_bits(THIRD));
        [x * reach, y * reach, z * reach]
    }
}

/// The state the generator had before a run of whole-number draws from 0
/// to `2^bits` (`Random.Range(0, 2^bits)`: each the draw's low `bits`),
/// found from what they gave: the generator is made of shifts and xors
/// alone, so each bit a draw gives is a sum (xor) of some of the state's
/// 128, and the draws' bits are as many equations, solved. With the
/// equations' rank (the bits no draw reaches are left at nought), and
/// `None` where the equations disagree: the draws not one run of the
/// generator, in that order and with no draw between them. A recording's
/// draws read this way show the order the game drew them in.
pub fn state_before_draws(draws: &[u32], bits: u32) -> (Option<[u32; 4]>, u32) {
    type Word = [u128; 32];
    let shl = |w: &Word, n: usize| -> Word { std::array::from_fn(|b| if b >= n { w[b - n] } else { 0 }) };
    let shr = |w: &Word, n: usize| -> Word { std::array::from_fn(|b| if b + n < 32 { w[b + n] } else { 0 }) };
    let xor = |a: &Word, b: &Word| -> Word { std::array::from_fn(|k| a[k] ^ b[k]) };
    // Each bit of each word as the state's bits it sums.
    let mut words: [Word; 4] = std::array::from_fn(|k| std::array::from_fn(|b| 1u128 << (32 * k + b)));
    let mut rows: Vec<(u128, bool)> = Vec::new();
    for &value in draws {
        // `UnityRandom::draw`, on the sums.
        let first = xor(&words[0], &shl(&words[0], 11));
        let last = words[3];
        let next = xor(&xor(&last, &shr(&last, 19)), &xor(&first, &shr(&first, 8)));
        words = [words[1], words[2], words[3], next];
        for b in 0..bits.min(32) as usize {
            rows.push((next[b], value >> b & 1 == 1));
        }
    }
    // Gauss and Jordan's elimination, over sums of bits.
    let mut rank = 0usize;
    let mut pivots = Vec::new();
    for bit in 0..128 {
        let Some(at) = (rank..rows.len()).find(|&r| rows[r].0 >> bit & 1 == 1) else { continue };
        rows.swap(rank, at);
        let pivot = rows[rank];
        for (r, row) in rows.iter_mut().enumerate() {
            if r != rank && row.0 >> bit & 1 == 1 {
                row.0 ^= pivot.0;
                row.1 ^= pivot.1;
            }
        }
        pivots.push(bit);
        rank += 1;
    }
    if rows[rank..].iter().any(|row| row.1) {
        return (None, rank as u32);
    }
    let mut state = [0u32; 4];
    for (row, &bit) in rows.iter().zip(&pivots) {
        if row.1 {
            state[bit / 32] |= 1 << (bit % 32);
        }
    }
    (Some(state), rank as u32)
}

/// The game's precalculated random numbers (`PrecalculatedRandoms`): tables
/// of one length filled from one seed, each looked up by an index modulo
/// that length. The ballistics calculator's are 512 long from seed 0, so
/// every shot's draws are fixed by the shot's own seed (ballistics spec,
/// "The random numbers").
#[derive(Debug, Clone)]
pub struct PrecalculatedRandoms {
    floats: Vec<f32>,
    directions: Vec<[f32; 3]>,
    normal: Vec<f32>,
}

/// Where the normal table's numbers move to the next third of their range:
/// shares of the table, the last always reached (`_PERCENTAGE_RANGE`).
const NORMAL_STEPS: [f32; 6] = [0.021, 0.157, 0.498, 0.838_999_9, 0.974_999_9, 1.0];

impl PrecalculatedRandoms {
    pub fn new(count: usize, seed: i32) -> Self {
        let mut random = UnityRandom::new(seed);
        let floats = (0..count).map(|_| random.value()).collect();
        let mut random = UnityRandom::new(seed);
        let directions = (0..count).map(|_| random.on_unit_sphere()).collect();
        Self { floats, directions, normal: normal_table(count, seed) }
    }

    /// The ballistics calculator's tables.
    pub fn ballistics() -> Self {
        Self::new(512, 0)
    }

    fn at(&self, index: i32) -> usize {
        // C#'s remainder keeps the sign; a seed is never below zero.
        (index.rem_euclid(self.floats.len() as i32)) as usize
    }

    /// `GetRandomFloat`: between 0 and 1.
    pub fn float(&self, index: i32) -> f32 {
        self.floats[self.at(index)]
    }

    /// `GetRandomDirection`: of length 1, in Unity's axes.
    pub fn direction(&self, index: i32) -> [f32; 3] {
        self.directions[self.at(index)]
    }

    /// `GetNormalDistributedRandomFloat`: the table's number, between −1
    /// and 1, times the spread, plus the mean.
    pub fn normal(&self, index: i32, mean: f32, spread: f32) -> f32 {
        self.normal[self.at(index)] * spread + mean
    }
}

/// The normal table: from −1 to 1 in six thirds, more of them near the
/// middle, each number drawn within its third; then shuffled from the
/// seed again, from the end (each place swapped with one at or before it).
fn normal_table(count: usize, seed: i32) -> Vec<f32> {
    let mut random = UnityRandom::new(seed);
    let third = 1.0f32 / 3.0;
    let (mut step, mut low, mut high) = (0usize, -1.0f32, -0.666_666_6f32);
    let mut table: Vec<f32> = (0..count)
        .map(|i| {
            let share = i as f32 / count as f32;
            if share > NORMAL_STEPS[step] && share < 1.0 {
                step += 1;
                low = -1.0 + step as f32 * third;
                high = -1.0 + (step + 1) as f32 * third;
            }
            random.range(low, high)
        })
        .collect();
    let mut random = UnityRandom::new(seed);
    let mut n = table.len();
    while n > 1 {
        n -= 1;
        let other = random.range_int(0, n as i32 + 1) as usize;
        table.swap(other, n);
    }
    table
}

/// The game's shared random tables (`FastRndom`): six tables of 1024 made
/// once by Unity's generator at fixed seeds (numbers between 0 and 1 at
/// 14547; turns, a number times 360, at 97829; directions at 567813; ways
/// over half a sphere, a point within the ball with its up made positive
/// and 0.3 added, made a unit, at 34572; ways across, two numbers each
/// taken from a half and made a unit in the ground's plane, at 12578; and,
/// the generator going on from there, ways within 60° of up, turned about
/// x by a number from −60° to 60° and about y by a whole turn's share),
/// read backwards by one index they all share; and two xorshifts of one
/// word (shifts of 7 and 3), signed for its whole numbers and unsigned for
/// its seeds. One for the whole game, as the game's is.
#[derive(Debug, Clone)]
pub struct FastRandom {
    floats: Vec<f32>,
    rotations: Vec<f32>,
    vectors: Vec<[f32; 3]>,
    hemisphere: Vec<[f32; 3]>,
    circle: Vec<[f32; 3]>,
    cone60: Vec<[f32; 3]>,
    index: i32,
    whole: i32,
    unsigned: u32,
}

/// The tables' length, and both xorshifts' first word.
const FAST_COUNT: usize = 1024;
const FAST_START: u32 = 242_313_459;

impl Default for FastRandom {
    fn default() -> Self {
        Self::new()
    }
}

impl FastRandom {
    pub fn new() -> Self {
        let mut random = UnityRandom::new(14547);
        let floats = (0..FAST_COUNT).map(|_| random.value()).collect();
        let mut random = UnityRandom::new(97829);
        let rotations = (0..FAST_COUNT).map(|_| random.value() * 360.0).collect();
        let mut random = UnityRandom::new(567_813);
        let vectors = (0..FAST_COUNT).map(|_| random.on_unit_sphere()).collect();
        let mut random = UnityRandom::new(34572);
        let hemisphere = (0..FAST_COUNT)
            .map(|_| {
                let mut v = random.inside_unit_sphere();
                if v[1] < 0.0 {
                    v[1] = -v[1];
                }
                v[1] += 0.3;
                unit(v)
            })
            .collect();
        let mut random = UnityRandom::new(12578);
        let circle = (0..FAST_COUNT)
            .map(|_| {
                let x = 0.5 - random.value();
                let y = 0.5 - random.value();
                let m = ((x * x + y * y) as f64).sqrt() as f32;
                if m > 1e-5 { [x / m, 0.0, y / m] } else { [0.0; 3] }
            })
            .collect();
        // The generator not started again: on from the ways across.
        let cone60 = (0..FAST_COUNT)
            .map(|_| {
                let a = random.range(-60.0, 60.0).to_radians();
                let b = (random.value() * 360.0).to_radians();
                // `Quaternion.Euler(a, b, 0) * Vector3.up`: about x, then y.
                let (sa, ca) = a.sin_cos();
                let (sb, cb) = b.sin_cos();
                [sa * sb, ca, sa * cb]
            })
            .collect();
        Self { floats, rotations, vectors, hemisphere, circle, cone60, index: 0, whole: FAST_START as i32, unsigned: FAST_START }
    }

    /// The shared index as it stands (the next draw takes the one below).
    pub fn index(&self) -> i32 {
        self.index
    }

    /// The shared index set, so that the next draw takes the one below: for
    /// a recording's draws to be found in the tables.
    pub fn set_index(&mut self, index: i32) {
        self.index = index.rem_euclid(FAST_COUNT as i32);
    }

    /// The seeds' xorshift set to a word (its next seed is the word's next).
    pub fn set_unsigned(&mut self, word: u32) {
        self.unsigned = word;
    }

    /// The whole numbers' xorshift set to a word.
    pub fn set_whole(&mut self, word: i32) {
        self.whole = word;
    }

    /// The shared index, one back (from the last after the first).
    fn next(&mut self) -> usize {
        self.index -= 1;
        if self.index < 0 {
            self.index = FAST_COUNT as i32 - 1;
        }
        self.index as usize
    }

    /// `Float`: between 0 and 1.
    pub fn float(&mut self) -> f32 {
        let i = self.next();
        self.floats[i]
    }

    /// `FloatRotation`: between 0 and 360.
    pub fn rotation(&mut self) -> f32 {
        let i = self.next();
        self.rotations[i]
    }

    /// `VectorNormalized`.
    pub fn vector(&mut self) -> [f32; 3] {
        let i = self.next();
        self.vectors[i]
    }

    /// `VectorHemisphere`.
    pub fn hemisphere(&mut self) -> [f32; 3] {
        let i = self.next();
        self.hemisphere[i]
    }

    /// `VectorCircle`.
    pub fn circle(&mut self) -> [f32; 3] {
        let i = self.next();
        self.circle[i]
    }

    /// `Cone60`.
    pub fn cone60(&mut self) -> [f32; 3] {
        let i = self.next();
        self.cone60[i]
    }

    /// `Int(start, range)`: `start` plus the whole numbers' next, made
    /// positive, modulo `range`.
    pub fn int(&mut self, start: i32, range: i32) -> i32 {
        self.whole ^= self.whole.wrapping_shl(7);
        self.whole ^= self.whole >> 3;
        let size = if self.whole < 0 { self.whole.wrapping_neg() } else { self.whole };
        if range == 0 {
            return start;
        }
        start + size % range
    }

    /// `Uint`: the seeds' next.
    pub fn uint(&mut self) -> u32 {
        self.unsigned ^= self.unsigned << 7;
        self.unsigned ^= self.unsigned >> 3;
        self.unsigned
    }
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let m = ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) as f64).sqrt() as f32;
    if m > 1e-5 { [v[0] / m, v[1] / m, v[2] / m] } else { [0.0; 3] }
}

/// The game's own small whole-number shuffles (`FastRndom`): a xorshift of
/// one word, by shifts of 7 and 3.
pub mod fast {
    /// `RandomizeInt`: a seed's next, never below zero. A shot's fragment
    /// or ricochet takes its seed so from the shot's.
    pub fn next_seed(input: i32) -> i32 {
        let mut x = input.wrapping_add(1);
        x ^= x.wrapping_shl(7);
        x ^= x >> 3;
        if x >= 0 { x } else { x.wrapping_neg() }
    }

    /// `Int(input, start, range)`: `start` plus the shuffled input's size
    /// modulo `range`, `start` itself for an empty range.
    pub fn int(input: i32, start: i32, range: i32) -> i32 {
        if range == 0 {
            return start;
        }
        let mut x = input;
        x ^= x.wrapping_shl(7);
        x ^= x >> 3;
        let size = if x < 0 { x.wrapping_neg() } else { x };
        start + size % range
    }
}

#[cfg(test)]
mod tests;
