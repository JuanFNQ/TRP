//! Tests of what the character earns by what it does.
use super::*;

/// The game's blender: from nought, the first aim is no fast one; the
/// sights down a second and up again, a second of fast aim.
#[test]
fn the_fast_aim_timer_is_the_game_s() {
    let mut t = FastAim::default();
    t.set_target(0.0, 0.0);
    assert!(!t.fast(0.5), "the first aim");
    t.set_target(2.0, 1.0);
    assert!((t.value(2.0) - 1.0).abs() < 1e-6 && (t.value(9.0) - 2.0).abs() < 1e-6);
    t.set_target(0.0, 2.0);
    assert!(t.fast(2.5) && t.fast(2.9) && !t.fast(3.1));
}

/// Credited to the state at each second's sample; a walk's metres from
/// its mark.
#[test]
fn the_pedometer_credits_each_second_to_its_state() {
    let mut p = Pedometer::default();
    p.sample(0.0, [3.0, 0.0, 4.0], State::Idle);
    p.mark(State::Move);
    p.sample(0.5, [6.0, 0.0, 8.0], State::Move);
    p.sample(1.0, [6.0, 5.0, 8.0], State::Move);
    assert!((p.from_mark(false) - 5.0).abs() < 1e-6, "the level distance alone: {}", p.from_mark(false));
    p.sample(2.0, [6.0, 0.0, 9.0], State::Sprint);
    assert!((p.from_mark(true) - 1.0).abs() < 1e-6 && (p.from_mark(false) - 5.0).abs() < 1e-6);
}
