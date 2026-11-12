//! Tests of the weapon's motion: the springs, the sway, the recoil's forces and a blast's shake.
use super::*;

/// The recoil's strengths as `recoil_settings` works them out with no
/// skill, and with a class skill at 50 (0.15 off the forces, not off
/// the aiming settings' bonus).
#[test]
fn a_class_skill_takes_its_share_off_the_recoil_forces() {
    let force = RecoilForce { up: 100.0, back: 300.0, vert_bonus: 20.0, back_bonus: 10.0, multiplier: 0.5, rotation_range: [0.9, 1.1], position_range: [0.8, 1.2], parts: 0.9, folded_parts: None };
    let (rotation, position) = force.strengths(0.9, 0.0);
    let plain = (100.0 * 0.9 + 20.0) * 0.5;
    assert_eq!(rotation, [0.9 * plain, 1.1 * plain]);
    assert_eq!(position[1], 1.2 * ((300.0 * 0.9 + 10.0) * 0.5));
    let (skilled, _) = force.strengths(0.9, 0.15);
    let less = (100.0 * (0.9 * 0.85) + 20.0) * 0.5;
    assert!((skilled[0] - 0.9 * less).abs() < 1e-4, "{skilled:?}");
}

#[test]
fn view_axes_round_trip_and_follow_the_game_conventions() {
    // Looking down by 30 degrees tilts forward toward -y.
    let forward = from_view([0.0, 0.0, 1.0], 0.0, 30.0);
    assert!(forward[1] < -0.49 && forward[1] > -0.51);
    // Facing right by 90 degrees points forward along +x.
    let forward = from_view([0.0, 0.0, 1.0], 90.0, 0.0);
    assert!((forward[0] - 1.0).abs() < 1e-5);
    let v = [0.3, -0.2, 0.9];
    let back = to_view(from_view(v, 40.0, -25.0), 40.0, -25.0);
    for i in 0..3 {
        assert!((back[i] - v[i]).abs() < 1e-5);
    }
}

#[test]
fn a_blast_s_shake_kicks_on_its_schedule() {
    // Strength 1 at 64 frames a second (no wait ends on a frame's
    // edge): kicks at once, then on the first frame each wait of 0.1,
    // 0.11, 0.121, 0.1331 and 0.14641 s is up; each three quarters of
    // the one before; then the time left is spent.
    let mut shake = Shake::new(1.0);
    let mut kicks = Vec::new();
    for frame in 0..120 {
        if let Some(power) = shake.frame(1.0 / 64.0) {
            kicks.push((frame, power));
        }
    }
    let frames: Vec<i32> = kicks.iter().map(|&(frame, _)| frame).collect();
    assert_eq!(frames, [0, 7, 15, 23, 32, 42]);
    assert!((kicks[0].1 - 1.2).abs() < 1e-6 && (kicks[1].1 - 0.9).abs() < 1e-6);
    assert!(shake.left <= 0.0);
}

#[test]
fn slerp_turns_by_the_angle_and_blends_the_length() {
    let half = slerp3([1.0, 0.0, 0.0], [0.0, 2.0, 0.0], 0.5);
    let length = (half[0] * half[0] + half[1] * half[1]).sqrt();
    assert!((length - 1.5).abs() < 1e-5 && (half[0] - half[1]).abs() < 1e-5);
    // From nought, a straight blend.
    assert_eq!(slerp3([0.0; 3], [0.0, 0.0, 6.0], 1.0 / 6.0), [0.0, 0.0, 1.0]);
}

#[test]
fn spring_returns_to_zero_after_a_push() {
    let mut s = Spring::new(SpringData { return_speed: 0.066, damping: 0.66, acceleration_max: 10.0, input_intensity: 1.0 });
    s.add(0, 3.0);
    s.step();
    assert!(s.current[0] > 1.0);
    for _ in 0..600 {
        s.step();
    }
    assert!(s.current[0].abs() < 1e-4);
}

#[test]
fn spring_clamps_one_push_to_its_limit() {
    let data = SpringData { return_speed: 0.1, damping: 0.5, acceleration_max: 10.0, input_intensity: 1.0 };
    let (mut a, mut b) = (Spring::new(data), Spring::new(data));
    a.add(1, 10.0);
    b.add(1, 500.0);
    a.step();
    b.step();
    assert_eq!(a.current, b.current);
}

#[test]
fn sway_oscillates_and_dies_out() {
    let mut s = Sway::new(7.0, 0.8);
    s.kick(2, 10.0);
    let mut peak = 0.0f32;
    let mut crossed = false;
    for _ in 0..600 {
        s.step(1.0 / 60.0);
        peak = peak.max(s.position[2]);
        crossed |= s.position[2] < 0.0;
    }
    assert!(peak > 0.3);
    assert!(crossed, "an underdamped spring overshoots");
    assert!(s.position[2].abs() < 1e-4);
}
