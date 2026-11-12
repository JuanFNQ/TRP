//! Tests of the skills' levels, buffs and names.
use super::*;

/// Every listed skill with buffs has some, the locked none; what the
/// page shows at 0, at 40 and at elite (Endurance's: the plebian
/// restoration gone at elite, the two elites come).
#[test]
fn the_skill_page_s_buffs_are_the_game_s() {
    assert!(Skill::ALL.iter().all(|skill| skill.locked() == buffs_of(*skill).is_empty()));
    let shown = |skill: Skill, level: u32| buffs_of(skill).iter().filter(|buff| buff.shown(level)).map(|buff| buff.id).collect::<Vec<_>>();
    assert!(shown(Skill::Endurance, 0).is_empty());
    assert_eq!(shown(Skill::Endurance, 40), ["EnduranceBuffEnduranceInc", "EnduranceBuffJumpCostRed", "EnduranceBuffBreathTimeInc", "EnduranceBuffRestorationTimeRed"]);
    assert_eq!(shown(Skill::Endurance, 51), ["EnduranceBuffEnduranceInc", "EnduranceHands", "EnduranceBuffJumpCostRed", "EnduranceBuffBreathTimeInc", "EnduranceBreathElite"]);
    assert_eq!(buffs_of(Skill::Endurance)[0].value(51), BuffValue::Figure(0.7));
    let covert = buffs_of(Skill::CovertMovement);
    assert!(matches!(covert[0].value(40), BuffValue::Figure(v) if (v - 0.48).abs() < 1e-6));
    assert_eq!(shown(Skill::Assault, 10).len(), 4, "the hidden three and the two elites are not shown");
}

/// The table's figures are those `Buffs` puts on the systems.
#[test]
fn the_table_agrees_with_the_buffs() {
    let mut levels = SkillLevels::default();
    for (skill, level) in [(Skill::Endurance, 51), (Skill::Strength, 37), (Skill::CovertMovement, 51), (Skill::MagDrills, 23)] {
        levels.set_level(skill, level);
    }
    let b = levels.buffs();
    let figure = |skill: Skill, id: &str| match buffs_of(skill).iter().find(|buff| buff.id == id).expect(id).value(levels.level(skill)) {
        BuffValue::Figure(v) => v,
        BuffValue::Flag(on) => on as u8 as f32,
    };
    assert_eq!(figure(Skill::Endurance, "EnduranceBuffEnduranceInc"), b.endurance_capacity);
    assert_eq!(figure(Skill::Endurance, "EnduranceBuffRestorationTimeRed"), b.endurance_restoration);
    assert_eq!(figure(Skill::Strength, "StrengthBuffLiftWeightInc"), b.strength_lift);
    assert_eq!(figure(Skill::CovertMovement, "CovertMovementLoud"), b.covert.loud);
    assert_eq!(figure(Skill::CovertMovement, "CovertMovementSpeed"), b.covert.speed);
    assert_eq!(figure(Skill::MagDrills, "MagDrillsInventoryCheckSpeed"), b.mag_drills.check);
}

#[test]
fn untrained_buffs_leave_everything_as_it_was() {
    let b = Buffs::default();
    assert_eq!((b.endurance_capacity, b.endurance_hands, b.endurance_jump, b.endurance_breath, b.endurance_restoration), (0.0, 0.0, 0.0, 0.0, 0.0));
    assert!(!b.endurance_breath_elite && !b.strength_elite && !b.throwing_elite && !b.search_double && !b.aim_drills_elite);
    assert_eq!((b.strength_level, b.strength_jump, b.strength_lift, b.strength_aim, b.aim_speed), (0, 0.0, 0.0, 0.0, 0.0));
    assert_eq!(b.covert, Covert::default());
    assert_eq!(b.covert.step_volume(true, 1.0), 1.0);
    assert_eq!(b.covert.volume(false), 1.0);
    assert_eq!(b.mag_drills, MagDrills::default());
    assert_eq!(b.weapon(Some(Skill::Assault)), WeaponBuffs::default());
    assert_eq!(b.weapon(None), WeaponBuffs::default());
}

#[test]
fn a_level_is_its_progress_over_a_hundred_and_the_rules_hold_it_at_fifty() {
    let mut levels = SkillLevels::default();
    levels.set_progress(Skill::CovertMovement, 4099.9);
    assert_eq!(levels.level(Skill::CovertMovement), 40);
    levels.set_progress(Skill::Strength, 9000.0);
    assert_eq!((levels.progress(Skill::Strength), levels.level(Skill::Strength)), (5100.0, 51));
    let b = levels.buffs();
    // Covert Movement 40: 48% quieter on a soft surface walking quietly.
    assert!((b.covert.soft - 0.48).abs() < 1e-6, "{}", b.covert.soft);
    // Strength at elite: the level rules at 50, the elite on top.
    assert!((b.strength_lift - 0.3).abs() < 1e-6 && b.strength_elite && b.strength_level == 51);
    levels.set_level(Skill::Endurance, 51);
    let b = levels.buffs();
    assert_eq!((b.endurance_capacity, b.endurance_hands, b.endurance_restoration), (0.7, 0.5, 0.75));
    assert!((b.endurance_jump - 0.3).abs() < 1e-6 && b.endurance_breath_elite);
    levels.set_level(Skill::Assault, 10);
    let w = levels.buffs().weapon(Skill::of_weapon(&["AssaultCarbine".to_owned(), "Weapon".to_owned()]));
    assert!((w.reload_speed - 1.04).abs() < 1e-6 && (w.swap_speed - 1.08).abs() < 1e-6 && !w.stiff_draw);
    assert_eq!(Skill::of_weapon(&["GrenadeLauncher".to_owned(), "Weapon".to_owned()]), None);
    // Revolver 20: a fifth of a double action's penalty off; elite holds
    // it at a half.
    levels.set_level(Skill::Revolver, 20);
    let revolver = Skill::of_weapon(&["Revolver".to_owned(), "Weapon".to_owned()]);
    assert!((levels.buffs().weapon(revolver).double_action_reduce - 0.2).abs() < 1e-6);
    levels.set_level(Skill::Revolver, 51);
    assert!((levels.buffs().weapon(revolver).double_action_reduce - 0.5).abs() < 1e-6);
}

#[test]
fn names_are_the_games() {
    assert_eq!(Skill::of_name("StressResistance"), Some(Skill::StressResistance));
    assert_eq!(Skill::of_name("smg"), Some(Skill::Smg));
    assert!(Skill::ALL.iter().enumerate().all(|(i, s)| s.index() == i));
    assert_eq!(Skill::ALL.iter().filter(|s| s.locked()).count(), 8);
}
