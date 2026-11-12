//! Tests of a weapon's wear, malfunction chances and heat, against the recorded AKM.
use super::*;

/// The install's AKM as built by default (`59d6088586f774275f37482f`,
/// its preset's parts: the gas tube 0.995 and 1.004, the handguard 1.054
/// and 0.95, the dust cover's burn 0.97).
fn akm() -> WeaponFigures {
    WeaponFigures {
        allow_jam: true,
        allow_feed: true,
        allow_misfire: true,
        allow_slide: true,
        allow_overheat: true,
        base_chance: 0.1771,
        fire_rate: 600.0,
        operating_resource: 6000.0,
        burn_ratio: 1.15,
        parts_burn: 0.97,
        heat_by_shot: 1.34,
        heat_gun: 0.9306,
        parts_heat: 0.995 * 1.054,
        cool: 3.168 * 0.9503 * 1.004 * 0.95,
        bolt_catch: false,
    }
}

const PS: RoundFigures = RoundFigures { misfire: 0.154, feed: 0.02, heat: 1.125, burn: 1.2 };

/// The durability's chance by its floor, as the game's two formulas give
/// it for the AKM: 0.0758% at 92, 0.410% at 50, 19.5% at 0; nothing at 93
/// to 100 but the heat's.
#[test]
fn the_durability_s_chance_climbs_as_the_weapon_wears() {
    let (w, s) = (akm(), Settings::default());
    let at = |durability: f32| chances(&w, &s, durability, Some(&PS), Some(0.09), 0.0, &Skills::default(), 0, 1.0);
    assert!((at(92.4).durability - 0.000758).abs() < 2e-6, "{:?}", at(92.4));
    assert!((at(50.0).durability - 0.00410).abs() < 2e-5, "{:?}", at(50.0));
    assert!((at(0.0).durability - 0.195).abs() < 1e-3, "{:?}", at(0.0));
    assert_eq!(at(95.0).total, 0.0);
    let ninety = at(92.0);
    assert!((ninety.total - (ninety.durability + (0.174 + 0.09) / 1000.0)).abs() < 1e-5);
    let hot = chances(&w, &s, 97.0, Some(&PS), Some(0.09), 150.0, &Skills::default(), 0, 1.0);
    assert!((hot.heat - 4.75).abs() < 1e-4 && (hot.total - 0.00475).abs() < 1e-5, "{hot:?}");
}

/// Recorded (`courses2\5-malfunction`, recorder 0.10.0, an AKM
/// at durability 35 with 7.62x39 FMJ, `64b7af5a8532cf95ee0a0dbd`: misfire
/// 0.154, feed 0.02, heat 1, burn 1): the game's chances at the shot
/// [0.01674, 0.0164727252, 0.09, 0.174, 0, 35], a shot's wear 0.0185928
/// (35 to 34.9814072) and its heat 1.30777049.
#[test]
fn the_recorded_akm_s_chance_wear_and_heat() {
    let (w, s) = (akm(), Settings::default());
    let fmj = RoundFigures { misfire: 0.154, feed: 0.02, heat: 1.0, burn: 1.0 };
    let c = chances(&w, &s, 35.0, Some(&fmj), Some(0.09), 0.0, &Skills::default(), 0, 1.0);
    assert!((c.durability - 0.016_472_725).abs() < 1e-6, "{c:?}");
    assert!((c.total - 0.01674).abs() < 1e-6 && (c.ammo - 0.174).abs() < 1e-6 && (c.magazine - 0.09).abs() < 1e-6 && c.floor == 35.0, "{c:?}");
    // A repair buff's protection of 0.95 (`MalfunctionProtections`, read
    // from code, no buffed weapon recorded): the durability's 0.016473
    // to 0.0156493, the round's and the magazine's as they were, the
    // total that and 0.264 / 1000 to five places; the heat's 4.75 per
    // mille at 150 to 4.5125.
    let buffed = chances(&w, &s, 35.0, Some(&fmj), Some(0.09), 0.0, &Skills::default(), 0, 0.95);
    assert!((buffed.durability - 0.016_472_725 * 0.95).abs() < 1e-7 && (buffed.durability - 0.015_649_35).abs() < 1e-6, "{buffed:?}");
    assert!((buffed.total - 0.01591).abs() < 1e-6 && buffed.ammo == c.ammo && buffed.magazine == c.magazine, "{buffed:?}");
    let hot = chances(&w, &s, 97.0, Some(&PS), Some(0.09), 150.0, &Skills::default(), 0, 0.95);
    assert!((hot.heat - 4.75 * 0.95).abs() < 1e-4 && (hot.total - 0.00451).abs() < 1e-5, "{hot:?}");
    let mut state = State::default();
    let (mut durability, mut most) = (35.0, 100.0);
    on_shot(&mut state, 1.0, &w, &s, &fmj, &Skills::default(), &mut durability, &mut most, &mut UnityRandom::new(1));
    assert!((35.0 - durability - 0.018_592_8).abs() < 2e-6, "{durability}");
    assert!((state.last_shot_heat - 1.307_770_5).abs() < 1e-5, "{state:?}");
}

/// Recorded (`courses2\5-heat`, recorder 0.10.0, an AKM from
/// 100 with the same FMJ: five magazines in bursts at 600 a minute, a
/// reload between, then one shot after 7.7 s and a magazine after it
/// went cold; 182 shots in all): each shot's heat from the one before
/// and the raid's clock at both, within 0.005 (the game's clock for the
/// heat goes by steps of about a millisecond), and the
/// durability 100 to 96.6161041 at 0.0185928 a shot.
#[test]
fn the_recorded_akm_heats_and_cools_as_courses2_5_heat() {
    let (w, s) = (akm(), Settings::default());
    let fmj = RoundFigures { misfire: 0.154, feed: 0.02, heat: 1.0, burn: 1.0 };
    // (the heat at a shot, its clock; the next shot's heat, its clock).
    let pairs = [
        (1.307_770_5, 451.6052, 2.328_164_6, 451.7052),
        (30.900_47, 454.5076, 31.920_95, 454.6076),
        (31.920_95, 454.6076, 16.032_57, 460.5929),
        (89.137_23, 488.823_76, 90.156_08, 488.924_56),
        (90.156_08, 488.924_56, 69.067_84, 496.674_13),
        (69.067_84, 496.674_13, 1.307_770_5, 524.2827),
    ];
    for (h0, t0, h1, t1) in pairs {
        let mut state = State { last_shot_heat: h0, last_shot_time: t0, ..State::default() };
        let (mut durability, mut most) = (100.0, 100.0);
        on_shot(&mut state, t1, &w, &s, &fmj, &Skills::default(), &mut durability, &mut most, &mut UnityRandom::new(1));
        assert!((state.last_shot_heat - h1).abs() < 5e-3, "from {h0} at {t0}: {} against {h1}", state.last_shot_heat);
    }
    let (mut durability, mut most) = (100.0f32, 100.0f32);
    for shot in 0..182 {
        let mut state = State::default();
        on_shot(&mut state, shot as f64, &w, &s, &fmj, &Skills::default(), &mut durability, &mut most, &mut UnityRandom::new(1));
    }
    assert!((durability - 96.616_104).abs() < 2e-4 && most == 100.0, "{durability} {most}");
}

/// A shot of PS wears the AKM 0.02231 below 100 heat and heats it 1.4712;
/// the heat falls 2.87 a second at low heat and 4.03 from 200.
#[test]
fn a_shot_wears_and_heats_and_the_heat_falls_back() {
    let (w, s) = (akm(), Settings::default());
    let mut state = State::default();
    let (mut durability, mut most) = (100.0, 100.0);
    let mut random = UnityRandom::new(1);
    on_shot(&mut state, 10.0, &w, &s, &PS, &Skills::default(), &mut durability, &mut most, &mut random);
    assert!((100.0 - durability - 0.02231).abs() < 2e-5, "{durability}");
    assert!((state.last_shot_heat - 1.4712).abs() < 1e-3, "{state:?}");
    let rate = |h0: f32| {
        let state = State { last_shot_heat: h0, last_shot_time: 0.0, ..State::default() };
        h0 - current_heat(&state, 0.1, &w, &s)
    } / 0.1;
    assert!((rate(10.0) - 2.87).abs() < 0.02, "{}", rate(10.0));
    assert!((rate(200.0) - 4.03).abs() < 0.03, "{}", rate(200.0));
}

/// Kept firing, the heat reaches 100 about the 85th round and the
/// maximum about the 171st, where the slide is held; the fire rate's
/// factor is drawn past 140.
#[test]
fn sustained_fire_overheats_the_weapon() {
    let (w, s) = (akm(), Settings::default());
    let mut state = State::default();
    let (mut durability, mut most) = (100.0, 100.0);
    let mut random = UnityRandom::new(7);
    let mut hundred = None;
    let mut most_at = None;
    for shot in 1..=200 {
        on_shot(&mut state, shot as f64 * 0.1, &w, &s, &PS, &Skills::default(), &mut durability, &mut most, &mut random);
        if hundred.is_none() && state.last_shot_heat >= 100.0 {
            hundred = Some(shot);
        }
        if most_at.is_none() && state.last_shot_heat >= 200.0 {
            most_at = Some(shot);
        }
        if state.last_shot_heat > 140.0 {
            assert!((0.85..=1.15).contains(&state.firerate_mult));
        }
    }
    assert!(hundred.is_some_and(|n| (83..=87).contains(&n)), "{hundred:?}");
    assert!(most_at.is_some_and(|n| (168..=174).contains(&n)), "{most_at:?}");
    assert!(state.slide_reached);
    assert!(most < 100.0, "the heat wore the greatest durability once below it: {most}");
}

/// The roll: nothing above the chance; with a chance of 1, a durability's
/// malfunction of one of its allowed kinds; the heat's slide with no
/// draw.
#[test]
fn the_roll_picks_a_source_and_a_kind() {
    let (w, s) = (akm(), Settings::default());
    let mut random = MalfRandom::new(12345);
    let none = Chances { total: 0.0, durability: 0.0, ..Chances::default() };
    assert_eq!(roll(&mut random, &w, &s, &none, &State::default(), 0.0, true, true).0, Malf::None);
    let certain = Chances { total: 1.0, durability: 1.0, floor: 20.0, ..Chances::default() };
    let mut kinds = std::collections::HashMap::new();
    for _ in 0..2000 {
        *kinds.entry(roll(&mut random, &w, &s, &certain, &State::default(), 0.0, true, true).0).or_insert(0) += 1;
    }
    let jam = kinds[&Malf::Jam] as f32 / 2000.0;
    assert!((jam - 0.6).abs() < 0.05, "{kinds:?}");
    assert!(!kinds.contains_key(&Malf::HardSlide), "no hard slide above 5");
    let hot = State { slide_reached: true, ..State::default() };
    let count = random.count;
    assert_eq!(roll(&mut random, &w, &s, &none, &hot, 180.0, true, true), (Malf::SoftSlide, Source::Overheat));
    assert_eq!(random.count, count, "no draw spent");
}
