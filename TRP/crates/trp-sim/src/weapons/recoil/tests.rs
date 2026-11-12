//! Tests of recoil: one shot, automatic fire, the stances and the weapon's curves.
use super::*;

fn data(return_speed: f32, damping: f32) -> RecoilProcessData {
    RecoilProcessData { return_speed, damping, intensity: 1.0 }
}

/// Settings shaped like an assault rifle's, with round numbers.
pub(crate) fn rifle() -> RecoilSettings {
    RecoilSettings {
        progress_power_on: true,
        progress_power_shot: 2,
        stable_shot_index: 5,
        shot_vals: vec![
            RecoilShotVal { from_axis: 0, target: 2, to_axis: 0, intensity: 0.15 },
            RecoilShotVal { from_axis: 1, target: 2, to_axis: 2, intensity: 0.15 },
            RecoilShotVal { from_axis: 2, target: 1, to_axis: 1, intensity: 0.0007 },
            RecoilShotVal { from_axis: 0, target: 0, to_axis: 0, intensity: 0.05 },
            RecoilShotVal { from_axis: 1, target: 0, to_axis: 1, intensity: -0.05 },
            RecoilShotVal { from_axis: 1, target: 4, to_axis: 1, intensity: 1.0 },
        ],
        rotation_strength: [18.0, 23.0],
        position_strength: [30.0, 48.0],
        angle_range: [80f32.to_radians(), 90f32.to_radians()],
        groups: Vec::new(),
        intensity_by_pose: [[0.6, 0.7, 0.7], [0.7, 0.7, 0.7], [1.0, 1.0, 1.0]],
        aim_intensity: 0.75,
        rotation: RotationSettings {
            return_speed: 2.4,
            damping: 0.86,
            category_multiplier: 0.19,
            return_path_offset: 0.01,
            return_path_damping: 0.48,
            stable_angle_increase_step: 2.5,
            progress_angle_on_stable: [0.0, 28.0],
            after_recoil_vertical_range: [-1.5, 0.0],
            after_recoil_horizontal_range: [-1.5, 1.5],
            reset_after_shot: false,
            to_next_stable_point_distance_limit: 0.95,
            next_stable_point_distance_range: [0.1, 8.0],
            to_next_current_speed: 0.2,
            return_speed_curve: Curve::flat(&[(0.0, 0.008), (2.5, 0.008)]),
        },
        hand_position: data(0.08, 0.5),
        camera_rotation: data(0.05, 0.5),
        curves: Vec::new(),
    }
}

#[test]
fn random_numbers_stay_in_range_and_repeat() {
    let (mut a, mut b) = (Rng::new(7), Rng::new(7));
    for _ in 0..1000 {
        let x = a.range(-2.0, 3.0);
        assert!((-2.0..=3.0).contains(&x));
        assert_eq!(x, b.range(-2.0, 3.0));
    }
    // Reversed ends are allowed.
    let y = a.range(5.0, 1.0);
    assert!((1.0..=5.0).contains(&y));
}

#[test]
fn clamp_with_crossed_bounds_follows_the_engine() {
    assert_eq!(unity_clamp(-3.0, 0.01, -0.01), 0.01);
    assert_eq!(unity_clamp(0.5, 0.01, -0.01), -0.01);
    assert_eq!(unity_clamp(-0.5, -2.0, -0.01), -0.5);
}

#[test]
fn one_shot_kicks_up_and_back_then_settles() {
    let mut r = Recoil::new(rifle(), 1);
    r.shoot(1.0);
    let (mut climb, mut back, mut camera) = (0.0f32, 0.0f32, 0.0f32);
    for _ in 0..600 {
        r.step(1.0 / 60.0);
        climb = climb.min(r.weapon_rotation()[0]);
        back = back.max(r.weapon_position()[1]);
        camera = camera.min(r.camera_angles()[0]);
    }
    assert!(climb < -1.0 && climb > -8.0, "muzzle climb {climb}");
    assert!(back > 0.005 && back < 0.06, "kick back {back}");
    assert!(camera < -0.3 && camera > -3.0, "camera kick {camera}");
    // The muzzle comes to rest near, not at, where it started.
    let rest = r.weapon_rotation();
    assert!(rest[0] <= 0.05 && rest[0] > -1.6, "rest {rest:?}");
    assert!(rest[2].abs() < 1.6);
    assert!(r.weapon_position()[1].abs() < 1e-4);
    assert!(r.camera_angles()[0].abs() < 1e-3);
}

#[test]
fn crouching_and_lying_down_soften_the_kick() {
    let climb = |pose: usize| {
        let mut r = Recoil::new(rifle(), 3);
        r.set_pose(pose);
        r.shoot(1.0);
        (0..120).fold(0.0f32, |low, _| {
            r.step(1.0 / 60.0);
            low.min(r.weapon_rotation()[0])
        })
    };
    let (prone, standing) = (climb(0), climb(2));
    assert!((prone / standing - 0.6).abs() < 0.02, "{prone} {standing}");
}

#[test]
fn automatic_fire_ramps_up_then_holds_around_a_point() {
    let mut r = Recoil::new(rifle(), 11);
    r.set_auto_fire(true);
    let mut after_shot = Vec::new();
    for shot in 0..20 {
        r.shoot(1.0);
        for _ in 0..6 {
            r.step(1.0 / 60.0);
        }
        after_shot.push(r.weapon_rotation()[0]);
        assert_eq!(r.shot_index, shot + 1);
    }
    assert!(r.hand_rotation.stable_on);
    // The first shots climb.
    assert!(after_shot[3] < after_shot[0]);
    // Once stable, the muzzle stays within a band instead of climbing on.
    let held = &after_shot[6..];
    let (low, high) = held.iter().fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    assert!(high - low < 6.0, "band {low}..{high}");

    // Letting go ends the run and the muzzle comes back down.
    r.set_auto_fire(false);
    for _ in 0..900 {
        r.step(1.0 / 60.0);
    }
    assert_eq!(r.shot_index, 0);
    assert!(!r.hand_rotation.stable_on);
    assert!(r.weapon_rotation()[0] > -1.6, "{}", r.weapon_rotation()[0]);
}

#[test]
fn weapon_curves_play_from_each_shot() {
    let mut settings = rifle();
    settings.curves.push(CurveRecoil {
        target: 2,
        axis: 1,
        time_multiply: 2.0,
        value_multiply: 1.0,
        aiming_value_multiply: 0.5,
        curve: Curve::flat(&[(0.0, 0.0), (0.1, 2.0), (0.3, -0.75), (1.0, 0.0)]),
    });
    let mut r = Recoil::new(settings, 5);
    r.step(1.0 / 60.0);
    assert_eq!(r.weapon_rotation()[1], 0.0, "nothing plays before the first shot");
    r.shoot(1.0);
    let mut peak = 0.0f32;
    for _ in 0..3 {
        r.step(1.0 / 60.0);
        peak = peak.max(r.weapon_rotation()[1]);
    }
    // The curve's clock runs twice as fast, so its peak at 0.1 is
    // reached on the fourth step.
    r.step(1.0 / 60.0);
    peak = peak.max(r.weapon_rotation()[1]);
    assert!(peak > 1.9 && peak <= 2.0, "{peak}");
    r.set_aiming(true);
    r.shoot(1.0);
    for _ in 0..4 {
        r.step(1.0 / 60.0);
    }
    assert!((r.weapon_rotation()[1] - 1.0).abs() < 0.06);
}

/// A weapon taken up after the empty hands, its springs carried on from
/// theirs as a change of hands does, climbs as one made afresh does: the
/// rotation spring takes the weapon's figures, not the empty hands'.
#[test]
fn a_weapon_taken_up_after_the_empty_hands_climbs_as_its_own() {
    let base = trp_data::character::player::RecoilData {
        progress_power_on: true,
        progress_power_shot: 2,
        shot_vals: rifle().shot_vals,
        rotation_strength_range: [0.9, 1.15],
        position_strength_range: [0.65, 1.05],
        incoming_strength_multiplier: 1.0,
        hand_rotation: data(0.5, 0.7),
        hand_position: rifle().hand_position,
        camera_rotation: rifle().camera_rotation,
        max_recoil_rotation_angle: 0.0,
        to_next_stable_point_distance_limit: 0.95,
        next_stable_point_distance_range: [0.1, 8.0],
        to_next_current_speed: 0.2,
        return_speed_curve: Curve::flat(&[(0.0, 0.008), (2.5, 0.008)]),
    };
    let climb = |r: &mut Recoil| {
        r.set_auto_fire(true);
        let mut top = 0.0f32;
        for shot in 0..30 {
            if shot % 6 == 0 {
                r.shoot(1.0);
            }
            r.step(1.0 / 60.0);
            top = top.min(r.weapon_rotation()[0]);
        }
        top
    };
    let own = climb(&mut Recoil::new(rifle(), 9));
    // As `Hands::carry_on_from` leaves it: the empty hands' recoil with
    // the weapon's settings on top.
    let mut carried = Recoil::new(RecoilSettings::of_prefab(&base), 9);
    carried.settings = rifle();
    let before = climb(&mut carried.clone());
    carried.take_weapon_figures();
    assert_eq!(climb(&mut carried), own);
    assert!(before < own * 2.0, "the empty hands' figures climb far further: {before} against {own}");
}
