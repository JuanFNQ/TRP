//! Tests of a shot's spread, flight and penetration.
use super::*;

/// The barrel's deviation by durability (`GetBarrelDeviation`): 1 new,
/// its most worn out, between the two along the curve (the AKM's
/// `DeviationCurve` 1.35 and `DeviationMax` 23).
#[test]
fn the_barrel_s_deviation_grows_as_the_weapon_wears() {
    let akm = WeaponFigures {
        centre_of_impact: 0.1,
        speed_factor: 1.0,
        shotgun_dispersion: 0.0,
        iron_sight_range: 100,
        default_round: None,
        deviation_curve: 1.35,
        deviation_max: 23.0,
        double_action_penalty: 1.5,
        stock_double_action: 1.0,
    };
    assert!((akm.barrel_deviation(100.0) - 1.0).abs() < 1e-5);
    assert!((akm.barrel_deviation(0.0) - 23.0).abs() < 1e-4);
    let (half, worn) = (akm.barrel_deviation(50.0), akm.barrel_deviation(20.0));
    assert!(half > 1.0 && worn > half && worn < 23.0, "{half} {worn}");
}

/// A shot from a hammer left down (`Shot`): the double action's
/// penalty added to the centre of impact before the round's factor,
/// the heat and the wear multiply it; the skill's share taken off
/// before the stock's multiplier; nothing with the hammer up.
#[test]
fn a_hammer_left_down_widens_the_spread() {
    // The MTs-255-12's figures with its wooden stock (2.5, 0.3).
    let mts = WeaponFigures {
        centre_of_impact: 0.2,
        speed_factor: 1.0,
        shotgun_dispersion: 0.0,
        iron_sight_range: 100,
        default_round: None,
        deviation_curve: 1.0,
        deviation_max: 100.0,
        double_action_penalty: 2.5,
        stock_double_action: 0.3,
    };
    assert_eq!(mts.double_action(false, 0.0), 0.0);
    assert!((mts.double_action(true, 0.0) - 0.75).abs() < 1e-6);
    // Revolver 50: half of it off.
    let half = mts.double_action(true, 0.5);
    assert!((half - 0.375).abs() < 1e-6);
    let round = Round { accuracy: -20, ..m882() };
    let plain = mts.spread_worn(&round, 100.0, 1.0, 0.0, 1.0);
    assert_eq!(plain, mts.spread(&round));
    let pulled = mts.spread_worn(&round, 100.0, 2.0, half, 1.0);
    assert!((pulled - (0.2 + 0.375) * 1.2 * 2.0).abs() < 1e-5, "{pulled}");
    // A repair buff's spread of 0.9 (`WeaponSpread`, read from code):
    // in the barrel's deviation and again on its own, 0.81 of the cone.
    let buffed = mts.spread_worn(&round, 100.0, 1.0, 0.0, 0.9);
    assert!((buffed - plain * 0.81).abs() < 1e-6, "{buffed} against {plain}");
}

fn m882() -> Round {
    Round {
        initial_speed: 385.0,
        mass_g: 5.4,
        diameter_mm: 9.0,
        ballistic_coefficient: 0.068,
        damage: 56.0,
        penetration_power: 18.0,
        penetration_deviation: 0.8,
        penetration_chance: 0.2,
        ricochet_chance: 0.1,
        fragmentation_chance: 0.2,
        min_fragments: 1,
        max_fragments: 3,
        armor_damage: 0.35,
        life: 5.0,
        accuracy: 0,
        projectiles: 1,
        penetration_damage_mod: 0.0,
        stamina_burn: 0.0,
        heavy_bleeding: 0.0,
        light_bleeding: 0.0,
        fuse: None,
        tracer: None,
    }
}

#[test]
fn the_g1_table_is_drawn_between_its_places() {
    assert_eq!(g1_drag(10.0), 0.0);
    // The list's places are taken as if 0.05 Mach apart, which from
    // Mach 0.6 they are not: at Mach 1 the game draws on through its
    // places 19 and 20 (Mach 0.85 and 0.875), not the table's 0.4805.
    let at_one = g1_drag(343.0);
    assert!((at_one - 0.4311).abs() < 1e-3, "{at_one}");
    // Below Mach 0.6 the places are what they say.
    assert!((g1_drag(0.3 * 343.0) - 0.2214).abs() < 1e-4);
    assert_eq!(g1_drag(10_000.0), 0.4988);
}

#[test]
fn a_flight_slows_and_falls() {
    let mut t = Trajectory::new([0.0; 3], [0.0, 0.0, 378.301], 5.4, 9.0, 0.068, [0.0, -9.81, 0.0]);
    let (at, v) = t.at(1.0);
    assert!(at[2] > 200.0 && at[2] < 378.0, "{at:?}");
    assert!(at[1] < -3.0, "{at:?}");
    assert!(magnitude(v) < 378.0);
}

/// Recorded (`courses3`, `2-far`): an M882 from the MP-443 left at
/// 378.301 m/s; 0.015 s on it was 5.5714 m out (frame 118).
#[test]
fn the_recorded_flight_is_flown() {
    let start = [-1.734_767_9, 1.358_049_4, -0.858_825_7];
    let velocity = [-361.611_18, -30.546_637, 106.845_17];
    let mut t = Trajectory::new(start, velocity, 5.4, 9.0, 0.068, [0.0, -9.81, 0.0]);
    let (at, v) = t.at(0.015);
    let recorded = [-7.06057, 0.906_938_1, 0.714_782_7];
    let off = magnitude(sub(at, recorded));
    assert!(off < 1e-3, "{at:?} is {off} m off");
    let recorded_v = [-353.515_38, -29.960_854, 104.4531];
    assert!(magnitude(sub(v, recorded_v)) < 0.05, "{v:?}");
}

struct Wall;
impl BulletWorld for Wall {
    fn ray(&self, _: Asker, from: V3, to: V3, hits: &mut Vec<RayHit>) {
        // A plane at z = 10 facing back, thick concrete.
        let surface = Surface { material: material::CONCRETE, figures: [100.0, 0.0, 0.41, 0.16, 0.23, 0.037] };
        let way = sub(to, from);
        if from[2] < 10.0 && to[2] >= 10.0 {
            let t = (10.0 - from[2]) / way[2];
            let point = add(from, scale(way, t));
            hits.push(RayHit { distance: t * magnitude(way), point, normal: [0.0, 0.0, -1.0], collider: 1, thing: 1, mesh: false, surface, body: None });
        }
    }
}

#[test]
fn a_bullet_stops_in_concrete_head_on() {
    let mut b = Ballistics::new(0.5, [0.0, -9.81, 0.0]);
    let round = std::sync::Arc::new(m882());
    b.create(1, &round, 0, 15, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0);
    let mut hits = Vec::new();
    for _ in 0..10 {
        hits.extend(b.tick(1.0 / 60.0, &mut Wall));
    }
    assert_eq!(hits.len(), 1);
    let hit = &hits[0];
    assert!((hit.point[2] - 10.0).abs() < 1e-4);
    // Head on there is no ricochet; concrete has no penetration chance.
    assert_eq!(hit.state, State::StopHit);
    assert!(hit.cos < -0.99);
    assert!(hit.damage < 56.0 && hit.damage > 50.0);
}

/// A 40 mm grenade's hit sets off its fuse by its age at the hit
/// (`ShotDelegate`: `TimeSinceShot >= FuzeArmTimeSec`): the VOG-25's
/// 0.05 s is armed on the wall 10 m out (0.13 s at 76 m/s), an M406's
/// 0.26 s is not, and its hit is a bullet's alone (the VOG-25's figures,
/// `install.md`).
#[test]
fn a_grenade_s_fuse_is_armed_by_its_age_at_the_hit() {
    let vog = |arm_time: f32| Round {
        initial_speed: 76.0,
        mass_g: 250.0,
        diameter_mm: 40.0,
        ballistic_coefficient: 0.222,
        damage: 199.0,
        penetration_power: 0.0,
        life: 40.0,
        ricochet_chance: 0.0,
        fuse: Some(Fuse { round: "5656eb674bdc2d35148b457c".to_owned(), arm_time, effect: Some("smallgrenade_expl".to_owned()), hit_effect_too: false, dummy: false }),
        ..m882()
    };
    for (arm_time, armed) in [(0.05, true), (0.26, false)] {
        let mut b = Ballistics::new(0.5, [0.0, -9.81, 0.0]);
        b.create(1, &std::sync::Arc::new(vog(arm_time)), 0, 15, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0);
        let mut hits = Vec::new();
        for _ in 0..20 {
            hits.extend(b.tick(1.0 / 60.0, &mut Wall));
        }
        assert_eq!(hits.len(), 1, "one hit, the grenade stopped");
        let hit = &hits[0];
        assert!((hit.age - 10.0 / 76.0).abs() < 0.01, "its age at the hit {}", hit.age);
        assert!(hit.first_hit && hit.forward);
        assert_eq!(hit.sets_off().is_some(), armed, "armed after {arm_time} s at an age of {}", hit.age);
    }
}
