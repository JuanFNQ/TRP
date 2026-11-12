//! Tests of how skills and mastering are earned: the awards, fatigue and first levels.
use super::*;

fn rules() -> EarnRules {
    let config = serde_json::json!({
        "SkillFreshEffectiveness": 1.3, "SkillFreshPoints": 1, "SkillPointsBeforeFatigue": 1,
        "SkillFatiguePerPoint": 0.6, "SkillMinEffectiveness": 0.0001, "SkillFatigueReset": 200,
        "SkillsSettings": {
            "SkillProgressRate": 0.4, "WeaponSkillProgressRate": 1,
            "Health": { "SkillProgress": 0.5 },
            "Charisma": { "SkillProgressAtn": 0.5, "SkillProgressInt": 0.5, "SkillProgressPer": 0.5 },
            "Endurance": { "MovementAction": 0.005, "SprintAction": 0.04, "GainPerFatigueStack": 0.05 },
            "Strength": { "MovementActionMin": 0.005, "MovementActionMax": 0.015, "SprintActionMin": 0.04, "SprintActionMax": 0.12, "ThrowAction": 0.2 },
            "Throwing": { "ThrowAction": 0.25 },
            "Assault": { "WeaponReloadAction": 0.025, "WeaponShotAction": 0.1, "WeaponChamberAction": 0.1 },
            "SMG": { "WeaponReloadAction": 0.1, "WeaponShotAction": 0.4, "WeaponChamberAction": 0.4 },
            "AimDrills": { "WeaponShotAction": 0.2 },
            "MagDrills": { "RaidLoadedAmmoAction": 0.02 }
        }
    });
    EarnRules::read(&config)
}

/// The game's table: 1.3 fresh, 1 for one point, then 0.6 a point more,
/// 0.006 at eleven, the least from twenty.
#[test]
fn effectiveness_falls_with_the_session_s_points() {
    let r = rules();
    assert_eq!((r.effectiveness(0), r.effectiveness(1), r.effectiveness(2)), (1.3, 1.0, 0.6));
    assert!((r.effectiveness(3) - 0.36).abs() < 1e-6);
    assert!((r.effectiveness(10) - 0.6f32.powi(9)).abs() < 1e-7 && (r.effectiveness(11) - 0.006).abs() < 1e-4);
    assert_eq!(r.effectiveness(25), 0.0001);
}

/// Level 0 takes 10 points, each worth 10 progress; level 8 takes 90,
/// each worth 10/9; past 9 one a point; an award across levels.
#[test]
fn the_first_nine_levels_are_cheaper() {
    assert!((first_levels(0.0, 1.0) - 10.0).abs() < 1e-5);
    assert!((first_levels(0.0, 10.0) - 100.0).abs() < 1e-4);
    assert!((first_levels(0.0, 15.0) - 125.0).abs() < 1e-4, "10 for level 0, 5 of level 1's 20 at 5 each");
    assert!((first_levels(850.0, 45.0) - 50.0).abs() < 1e-3, "half of level 8's 90 points");
    assert!((first_levels(950.0, 3.0) - 3.0).abs() < 1e-5);
    assert!((first_levels(890.0, 10.0) - 11.0).abs() < 1e-3, "level 8's last 9 points, then one past it");
}

/// An award through the fatigue: the first point at 1.3, the next at 1,
/// then less; past the reset's 200 s whole again. With simple gain, at
/// its whole worth every time.
#[test]
fn an_award_passes_the_session_s_fatigue() {
    let r = rules();
    let mut session = Session::new(&r);
    let worth = session.use_effectiveness(Skill::MagDrills, 3.0, 0.0, &r);
    // Step 1: 1.3 (earned 1.3, eff 1); step 2: 1 (2.3, eff 0.6); step 3: 0.6.
    assert!((worth - 2.9).abs() < 1e-5, "{worth}");
    assert!((session.of(Skill::MagDrills).effectiveness - 0.6).abs() < 1e-6);
    let later = session.use_effectiveness(Skill::MagDrills, 1.0, 100.0, &r);
    assert!((later - 0.6).abs() < 1e-5, "still tired 100 s on");
    let rested = session.use_effectiveness(Skill::MagDrills, 1.0, 400.0, &r);
    assert!((rested - 1.0).abs() < 1e-5, "whole again past the reset: {rested}");

    let mut levels = SkillLevels::default();
    let mut session = Session::new(&r);
    let mut ups = Vec::new();
    for _ in 0..45 {
        award(&mut levels, &mut session, &r, Skill::MagDrills, 1.0, 0.0, true, &mut ups);
    }
    assert!((levels.progress(Skill::MagDrills) - 250.0).abs() < 1e-2, "10 + 20 points: two levels, then half the third's 30");
    assert_eq!(ups, vec![LevelUp { skill: Skill::MagDrills, level: 1 }, LevelUp { skill: Skill::MagDrills, level: 2 }]);
}

/// A sprint feeds Health a fifth of Endurance's award before its
/// fatigue; a throw gives Strength and Throwing the same; a hit gives
/// the class's shot figure, an SMG the assault rifles', twice at elite,
/// and Aim Drills within the fast-aim time.
#[test]
fn actions_give_by_the_install_s_figures() {
    let r = rules();
    let sprint = awards(Act::Moved { metres: 50.0, sprint: true, overweight: 0.0, fatigue: 0.0, quiet: false }, &r, |_| false);
    assert_eq!(sprint.len(), 1);
    assert!((sprint[0].1 - 50.0 * 0.04 * 0.4).abs() < 1e-5);
    let heavy = awards(Act::Moved { metres: 10.0, sprint: false, overweight: 0.5, fatigue: 0.0, quiet: true }, &r, |_| false);
    assert_eq!(heavy[0].0, Skill::Strength);
    assert!((heavy[0].1 - 10.0 * 0.01 * 0.4).abs() < 1e-5);
    let thrown = awards(Act::Thrown, &r, |_| false);
    assert!(thrown.iter().all(|(_, amount)| (amount - 0.1).abs() < 1e-6) && thrown.len() == 2);
    // A blow on a body: Strength's 0.2 times the progress rate.
    let blow = awards(Act::Fistfight, &r, |_| false);
    assert_eq!(blow.len(), 1);
    assert!(blow[0].0 == Skill::Strength && (blow[0].1 - 0.2 * 0.4).abs() < 1e-6, "{blow:?}");
    let hit = awards(Act::Hit { class: Skill::Smg, fast_aim: true }, &r, |_| true);
    assert_eq!(hit.len(), 2);
    assert!((hit[0].1 - 0.2).abs() < 1e-6 && (hit[1].1 - 2.0 * 0.2 * 0.4).abs() < 1e-6);

    let mut levels = SkillLevels::default();
    let mut session = Session::new(&r);
    let mut ups = Vec::new();
    award(&mut levels, &mut session, &r, Skill::Endurance, 0.8, 0.0, false, &mut ups);
    // Health takes 0.8 × 0.5 × 0.4 = 0.16 at 1.3: 2.08 progress.
    assert!((levels.progress(Skill::Health) - 0.16 * 1.3 * 10.0).abs() < 1e-4);
    assert!((levels.progress(Skill::Endurance) - 0.8 * 1.3 * 10.0).abs() < 1e-4);
    // At elite nothing more, and nothing fed.
    levels.set_level(Skill::Endurance, ELITE);
    let health = levels.progress(Skill::Health);
    award(&mut levels, &mut session, &r, Skill::Endurance, 5.0, 0.0, false, &mut ups);
    assert_eq!((levels.progress(Skill::Endurance), levels.progress(Skill::Health)), (MOST_PROGRESS, health));
}

fn with_metabolism() -> EarnRules {
    let mut config = serde_json::json!({
        "SkillFreshEffectiveness": 1.3, "SkillFreshPoints": 1, "SkillPointsBeforeFatigue": 1,
        "SkillFatiguePerPoint": 0.6, "SkillMinEffectiveness": 0.0001, "SkillFatigueReset": 200,
        "SkillsSettings": { "SkillProgressRate": 0.4, "WeaponSkillProgressRate": 1, "Metabolism": { "EnergyRecoveryRate": 20, "HydrationRecoveryRate": 20 } }
    });
    config["SkillsSettings"]["Assault"] = serde_json::json!({ "WeaponReloadAction": 0.025, "WeaponShotAction": 0.1, "WeaponChamberAction": 0.1 });
    config["SkillsSettings"]["AimDrills"] = serde_json::json!({ "WeaponShotAction": 0.2 });
    EarnRules::read(&config)
}

/// The game's raid rule: a gain by its amount, eight points a point,
/// through the fatigue (an MRE's 70 energy in thirteen loops is worth
/// about 12.6); with simple XP gain the gains themselves give nothing,
/// an item used gives eight for each of energy and hydration it raised.
#[test]
fn metabolism_earns_by_the_point_or_once_an_item() {
    let r = with_metabolism();
    assert_eq!(metabolism(&Event::EnergyGained(5.0), false), [Act::Energy(5.0)]);
    assert!(metabolism(&Event::EnergyGained(5.0), true).is_empty() && metabolism(&Event::HydrationGained(5.0), true).is_empty());
    assert!(metabolism(&Event::Fed { energy: 70.0, hydration: 0.0 }, false).is_empty());
    assert_eq!(metabolism(&Event::Fed { energy: 70.0, hydration: 0.0 }, true), [Act::Energy(1.0)]);
    assert_eq!(metabolism(&Event::Fed { energy: 10.0, hydration: 15.0 }, true), [Act::Energy(1.0), Act::Hydration(1.0)]);
    let worth = |simple: bool, events: &[Event]| {
        let (mut levels, mut session) = (SkillLevels::default(), Session::new(&r));
        for event in events {
            for act in metabolism(event, simple) {
                earn(&mut levels, &mut session, &r, act, 0.0, simple, |_| false);
            }
        }
        levels.progress(Skill::Metabolism)
    };
    let mre: Vec<Event> = (0..13).map(|_| Event::EnergyGained(70.0 / 13.0)).chain([Event::Fed { energy: 70.0, hydration: 0.0 }]).collect();
    // 1.3, then 1, then 0.6 to the power of the points less 1: 12.6
    // points, 10 for level 1 and 2.6 of level 1's 20 at 5 each.
    let fatigued = worth(false, &mre);
    assert!((fatigued - 113.2).abs() < 1.0, "{fatigued}");
    assert!((worth(true, &mre) - 80.0).abs() < 1e-3, "eight points at level 0's ten a point");
    assert!((worth(true, &[Event::Fed { energy: 10.0, hydration: 15.0 }]) - 130.0).abs() < 1e-3, "sixteen: level 1's ten at 10 each, six of level 1's twenty at 5");
    let trickle: Vec<Event> = (0..1500).map(|_| Event::EnergyGained(0.02)).collect();
    assert_eq!(worth(true, &trickle), 0.0, "a rate's gains alone give nothing");
}

/// A hit counts twice for Aim Drills where the class is elite as the
/// buffs have it (its summary level), whatever its own points.
#[test]
fn a_hit_counts_twice_by_the_class_s_summary_level() {
    let r = with_metabolism();
    let (mut levels, mut session) = (SkillLevels::default(), Session::new(&r));
    let hit = Act::Hit { class: Skill::Assault, fast_aim: true };
    earn(&mut levels, &mut session, &r, hit, 0.0, true, |class| class == Skill::Assault);
    assert!((levels.progress(Skill::AimDrills) - 2.0 * 0.2 * 0.4 * 10.0).abs() < 1e-4);
    assert!((levels.progress(Skill::Assault) - 2.0 * 0.1 * 10.0).abs() < 1e-4, "the class's own points too, below its elite");
}
