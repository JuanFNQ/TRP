//! Tests of a shotgun's pellets against the game's, from a recorded shot.
//! After `docs/spec/ballistics.md`, "A shot leaves the weapon": `courses2`'s `5-pellets` (recorder 0.10.0),
//! the MR-133 with 12/70 7mm buckshot, its first shot (frame 264).

use std::sync::Arc;

use trp_sim::combat::ballistics::{Ballistics, Round, WeaponFigures};
use trp_sim::base::unity_random::{state_before_draws, UnityRandom};

const MR133: &str = "54491c4f4bdc2db1078b4568";
const BUCKSHOT: &str = "560d5e524bdc2d25448b4571";

/// The shot as recorded (`InitiateShot`'s start and way, after the spread)
/// and its pellets: each one's way and seed, in their made order, and the
/// direction entry each way was found to be turned by (`bin/bullets`).
const START: [f32; 3] = [3.20178556, 1.48496842, -0.7471771];
const WAY: [f32; 3] = [0.923279345, 0.0792097151, 0.375873774];
const PELLETS: [([f32; 3], i32, u32); 8] = [
    ([0.923415542, 0.06638788, 0.378016353], 84, 37),
    ([0.9229993, 0.06805308, 0.378736019], 306, 4),
    ([0.9214921, 0.07935818, 0.380203426], 58, 461),
    ([0.921864331, 0.07176521, 0.3808096], 1, 338),
    ([0.92275095, 0.06528266, 0.3798274], 225, 15),
    ([0.922252238, 0.0876747444, 0.376515448], 442, 154),
    ([0.9213629, 0.07180568, 0.3820135], 484, 231),
    ([0.9177416, 0.07778814, 0.389485925], 157, 67),
];
/// The recorded pellets' speed (`initial_speed`: the round's, times the
/// weapon's speed factor).
const SPEED: f32 = 437.41;

fn angle(a: [f32; 3], b: [f32; 3]) -> f32 {
    let c = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt().atan2(a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
}

/// From the state Unity's generator had before the pellets (found from
/// their draws), our shot (`Ballistics::shoot`) with the MR-133's own
/// dispersion makes the game's eight pellets: each seed and fragment
/// number, and each way within 1e-6 rad.
#[test]
fn a_recorded_shotgun_shot_is_made_again_pellet_for_pellet() {
    let Some(db) = crate::fixture::database() else { return };
    let preset = db.default_preset(MR133).expect("its preset");
    let parts: Vec<&str> = preset.items.iter().map(|part| part.template.as_str()).filter(|part| *part != MR133).collect();
    let figures = WeaponFigures::of(db, MR133, &parts).expect("its figures");
    println!("  the MR-133's dispersion {}", figures.shotgun_dispersion);
    assert!((figures.shotgun_dispersion - 1.55).abs() < 1e-6);
    let round = Arc::new(db.items.get(BUCKSHOT).and_then(Round::of).expect("the buckshot"));
    assert_eq!(round.projectiles, 8);

    let draws: Vec<u32> = PELLETS.iter().flat_map(|p| [p.2, p.1 as u32]).collect();
    let state = state_before_draws(&draws, 9).0.expect("one run of the generator");
    let mut sim = Ballistics::new(0.5, [0.0, -9.81, 0.0]);
    sim.shoot(0, &round, START, WAY, SPEED / round.initial_speed, figures.shotgun_dispersion, &mut UnityRandom::from_state(state));
    let made = sim.bullets();
    assert_eq!(made.len(), 8);
    for (k, (bullet, (way, seed, _))) in made.iter().zip(PELLETS).enumerate() {
        let off = angle(bullet.velocity, way);
        println!("  pellet {k}: seed {} (the game's {seed}), fragment {}, {off:.2e} rad off its way, at {} m/s", bullet.seed, bullet.fragment, bullet.speed);
        assert_eq!((bullet.seed, bullet.fragment), (seed, 2 * k as i32));
        assert!(off < 1e-6, "pellet {k} {off} rad off");
        assert!((bullet.speed - SPEED).abs() < 1e-2);
    }
}
