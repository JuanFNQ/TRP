//! A grenade's fragments, shots of its fragment round flown from the blast.
//! After the grenade spec, "The fragments" (`ExplosionSharedMethods.Explosion`,
//! `SpawnRandomFragments`): shots of
//! the grenade's fragment round (`FragmentType`) from the blast, made as the
//! game makes them and flown by the ballistics as any shot of the thrower's,
//! whose own body they never meet (`Shot.IsHitIgnored`).
//!
//! **Aimed** at each character within the blast's reach, first: the share
//! of its fragments the sphere at its distance gives it, twice over
//! (`count / (4π d²) × 2`, at most 3), handed out in pieces of 0.3 to 2,
//! each a fragment toward its chest (0.8 m above its feet), scattered by
//! half a metre over the piece, at the round's speed. **Then at random** up
//! to 30 in all (or the count, if fewer): nine a frame from the blast's
//! frame on, each the way the grenades' table gives the fragment's number,
//! three in four turned up for a blast on the ground and down for one in
//! the air, its speed from half the round's to 0.95 of it by the number's
//! last figure. Every fragment's number is one counter's, the game's
//! (`GlobalFragmentNumber`), kept for the whole run.

use crate::base::unity_random::PrecalculatedRandoms;

type V3 = [f32; 3];

/// The grenades' table (`Grenade.GrenadeRandoms`: 128 of each kind from
/// seed 1337, made as the game's world wakes).
pub fn grenade_randoms() -> PrecalculatedRandoms {
    PrecalculatedRandoms::new(128, 1337)
}

/// Where a blast is (`Explosion`'s ray down within 4 m on
/// `TerrainLowPoly`): on the ground (met within 0.3 m), in the air (met
/// nothing), or between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lie {
    Ground,
    Air,
    Between,
}

impl Lie {
    /// From the distance down to what the ray met, if anything.
    pub fn of(below: Option<f32>) -> Self {
        match below {
            None => Lie::Air,
            Some(d) if d < 0.3 => Lie::Ground,
            Some(_) => Lie::Between,
        }
    }
}

/// How far down a blast looks for the ground, and how near it is on it.
pub const GROUND_REACH: f32 = 4.0;

/// The most fragments a blast makes at random and aimed together, and how
/// many of the random ones a frame.
pub const MOST: i32 = 30;
pub const EACH_FRAME: usize = 9;

/// One fragment: its way (a unit), its speed as a share of the round's
/// (`speedFactor`), the frame it is made on counted from the blast's, the
/// fragment number its way was taken at, and the character it was aimed at
/// (its place among those given), if any.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fragment {
    pub way: V3,
    pub speed_factor: f32,
    pub frame: usize,
    pub number: i32,
    pub aimed: Option<usize>,
}

fn normalized(v: V3) -> V3 {
    // `Vector3.normalized`: nought below 1e-5.
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length > 1e-5 {
        v.map(|c| c / length)
    } else {
        [0.0; 3]
    }
}

/// How many fragments a character `distance` from a blast of `count` is
/// given before they are handed out (`Clamp(count / (4π d²) × 2, 0, 3)`).
pub fn aimed_share(count: i32, distance: f32) -> f32 {
    (count as f32 / (std::f32::consts::PI * 4.0 * distance * distance) * 2.0).clamp(0.0, 3.0)
}

/// A blast's fragments, in the order the game makes them: `count` the
/// grenade's (`FragmentsCount`; none for nought), `number` the run's
/// counter as the blast finds it (moved on past them), `at` the blast's
/// place, `feet` each character within its reach (`Explosion`'s
/// `OverlapSphere`: the largest of twice the greatest distance, the
/// blindness's and the contusion's reaches) in the host's order, and where
/// the blast lies.
pub fn fragments(count: i32, number: &mut i32, table: &PrecalculatedRandoms, at: V3, feet: &[V3], lie: Lie) -> Vec<Fragment> {
    let mut out = Vec::new();
    if count <= 0 {
        return out;
    }
    let mut aimed = 0;
    for (who, feet) in feet.iter().enumerate() {
        let distance = ((feet[0] - at[0]).powi(2) + (feet[1] - at[1]).powi(2) + (feet[2] - at[2]).powi(2)).sqrt();
        let mut share = aimed_share(count, distance);
        while share > 0.3 {
            let piece = share.clamp(0.3, 2.0);
            share -= piece;
            *number = number.wrapping_add(1);
            aimed += 1;
            let scatter = table.direction(*number);
            let toward: V3 = std::array::from_fn(|k| feet[k] + if k == 1 { 0.8 } else { 0.0 } + 0.5 * scatter[k] / piece - at[k]);
            out.push(Fragment { way: normalized(toward), speed_factor: 1.0, frame: 0, number: *number, aimed: Some(who) });
        }
    }
    let random = count.min(MOST) - aimed;
    for i in 0..random.max(0) as usize {
        let mut way = table.direction(*number);
        let turned = i % 4 < 3;
        match lie {
            Lie::Air if way[1] > 0.0 && turned => way[1] = -way[1],
            Lie::Ground if way[1] < 0.0 && turned => way[1] = -way[1],
            _ => {}
        }
        // C#'s remainder keeps the sign: the counter is never below nought
        // within a run's reach.
        let speed_factor = 0.5 + (*number % 10) as f32 / 20.0;
        out.push(Fragment { way: normalized(way), speed_factor, frame: i / EACH_FRAME, number: *number, aimed: None });
        *number = number.wrapping_add(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_at_random_nine_a_frame_their_speed_by_the_number() {
        let table = grenade_randoms();
        let mut number = 7;
        let made = fragments(70, &mut number, &table, [0.0, 0.0, 0.0], &[], Lie::Ground);
        assert_eq!(made.len(), 30);
        assert_eq!(number, 37);
        assert_eq!(made.iter().map(|f| f.frame).collect::<Vec<_>>(), (0..30).map(|i| i / 9).collect::<Vec<_>>());
        // The 8th's number (7) gives 0.85, the 4th's (10) half.
        assert!((made[0].speed_factor - 0.85).abs() < 1e-6 && (made[3].speed_factor - 0.5).abs() < 1e-6);
        // On the ground, three in four of those going down turned up; the
        // fourth of each four kept.
        for (i, f) in made.iter().enumerate() {
            let raw = table.direction(f.number);
            if i % 4 < 3 {
                assert!(f.way[1] >= 0.0 && (f.way[1].abs() - raw[1].abs()).abs() < 1e-6);
            } else {
                assert!((0..3).all(|k| (f.way[k] - raw[k]).abs() < 1e-6));
            }
        }
    }

    #[test]
    fn a_character_near_enough_is_aimed_at_first_and_fewer_go_at_random() {
        let table = grenade_randoms();
        let mut number = 0;
        // 2 m from 70: 70 / (16π) × 2 = 2.79, so a piece of 2 and one of
        // 0.79; 9 m off, 0.14, none.
        let made = fragments(70, &mut number, &table, [0.0, 0.0, 0.0], &[[2.0, 0.0, 0.0], [9.0, 0.0, 0.0]], Lie::Between);
        let aimed: Vec<&Fragment> = made.iter().filter(|f| f.aimed.is_some()).collect();
        assert_eq!(aimed.len(), 2);
        assert_eq!(made.len(), 30);
        assert_eq!((aimed[0].number, aimed[1].number), (1, 2));
        // Toward the chest, the scatter half a metre over the piece.
        let scatter = table.direction(1);
        let toward = normalized([2.0 + 0.25 * scatter[0], 0.8 + 0.25 * scatter[1], 0.25 * scatter[2]]);
        assert!((0..3).all(|k| (aimed[0].way[k] - toward[k]).abs() < 1e-6));
        assert_eq!(aimed[0].speed_factor, 1.0);
        // The random ones follow on from the counter.
        assert_eq!(made[2].number, 2);
        assert_eq!(number, 30);
    }
}
