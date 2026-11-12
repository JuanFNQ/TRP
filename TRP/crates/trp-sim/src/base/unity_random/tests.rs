//! Tests of Unity's random numbers and the game's tables filled from them.
use super::*;

#[test]
fn a_seed_s_words_follow_the_engine_s_rule() {
    let r = UnityRandom::new(1);
    assert_eq!(r.words[0], 1);
    assert_eq!(r.words[1], 1_812_433_254);
    assert_eq!(r.words[2], 1_812_433_254u32.wrapping_mul(1_812_433_253).wrapping_add(1));
}

#[test]
fn numbers_and_directions_are_in_their_ranges() {
    let mut r = UnityRandom::new(12345);
    for _ in 0..10_000 {
        let v = r.value();
        assert!((0.0..=1.0 + 1e-6).contains(&v));
        let d = r.on_unit_sphere();
        let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((length - 1.0).abs() < 1e-5);
        let i = r.range_int(3, 9);
        assert!((3..9).contains(&i));
    }
}

#[test]
fn the_fast_tables_are_made_and_read_backwards() {
    let mut f = FastRandom::new();
    // The shared index starts at the last.
    let first = f.float();
    assert_eq!(first, FastRandom::new().floats[1023]);
    assert_eq!(f.rotation(), FastRandom::new().rotations[1022]);
    let h = f.hemisphere();
    assert!(h[1] > 0.0);
    let c = f.circle();
    assert!(c[1] == 0.0 && ((c[0] * c[0] + c[2] * c[2]).sqrt() - 1.0).abs() < 1e-5);
    let k = f.cone60();
    assert!(k[1] >= 0.5 - 1e-5);
    assert!((0..1000).all(|_| (5..15).contains(&f.int(5, 10))));
    assert_eq!(f.int(3, 0), 3);
}

#[test]
fn a_fragment_s_seed_is_the_game_s() {
    // Recorded (courses3, `2-far`): a bullet of seed 15 went on, after
    // its first hit, as one of seed 2322.
    assert_eq!(fast::next_seed(15), 2322);
}

/// Recorded (courses2, `5-pellets`, frame 264): the MR-133's eight
/// pellets' direction entries and seeds, in their made order. As one
/// run of the generator, entry then seed for each pellet, they agree
/// (144 equations, 17 more than the 127 they reach); taken each seed
/// before its entry, they do not; and drawn again from the state found,
/// they come in that order.
#[test]
fn a_shotgun_shot_s_draws_are_one_run_of_the_game_s_generator() {
    let draws = [37, 84, 4, 306, 461, 58, 338, 1, 15, 225, 154, 442, 231, 484, 67, 157];
    let (state, rank) = state_before_draws(&draws, 9);
    assert_eq!(rank, 127);
    let state = state.expect("one run");
    let mut r = UnityRandom::from_state(state);
    let again: Vec<u32> = (0..draws.len()).map(|_| r.range_int(0, 512) as u32).collect();
    assert_eq!(again, draws);
    let swapped: Vec<u32> = draws.chunks(2).flat_map(|pair| [pair[1], pair[0]]).collect();
    assert!(state_before_draws(&swapped, 9).0.is_none());
}

#[test]
fn the_ballistics_tables_give_the_recorded_penetration() {
    // Recorded (courses3, `2-far`): M882 (18 ± 0.8) fired with seed 15
    // left the barrel with a penetration of 17.785305.
    let tables = PrecalculatedRandoms::ballistics();
    let got = tables.normal(15, 18.0, 0.8);
    assert!((got - 17.785305).abs() < 1e-4, "{got}");
}

/// The shared index set takes the draw below it (the index 0 the last
/// place's); the seeds' and whole numbers' words set give the draws
/// that follow from them.
#[test]
fn the_fast_tables_drawn_from_where_they_are_set() {
    let mut fresh = FastRandom::new();
    let draws: Vec<f32> = (0..FAST_COUNT).map(|_| fresh.float()).collect();
    let mut set = FastRandom::new();
    set.set_index(1000);
    assert_eq!(set.float(), draws[FAST_COUNT - 1 - 999]);
    assert_eq!(set.index(), 999);
    set.set_index(0);
    assert_eq!(set.float(), draws[0]);
    let (mut a, mut b) = (FastRandom::new(), FastRandom::new());
    let word = a.uint();
    b.set_unsigned(word);
    assert_eq!(a.uint(), b.uint());
    a.set_whole(-123_456_789);
    b.set_whole(-123_456_789);
    assert!((0..50).all(|_| a.int(5, 10) == b.int(5, 10)));
}
