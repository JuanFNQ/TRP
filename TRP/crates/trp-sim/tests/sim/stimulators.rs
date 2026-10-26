//! Stimulators' and buffed foods' sets of buffs as the install has them
//! (`docs/spec/health.md`, "Buffs"): each item used to its end, then the
//! set's buffs at work, timed by its delays and durations. Passes with a
//! note where no install is configured.

use std::sync::Arc;

use trp_data::Database;
use trp_sim::health::{Health, HealthSettings, Kind, Medicine, Part};
use trp_sim::skills::{Skill, SkillLevels};

const PROPITAL: &str = "5c0e530286f7747fa1419862";
const SJ1: &str = "5c0e531286f7747fa54205c2";
const SJ6: &str = "5c0e531d86f7747fa23f4d42";
const ZAGUSTIN: &str = "5c0e533786f7747fa23f4d47";
const MULE: &str = "5ed51652f6c34d2cc26336a1";

const DT: f32 = 1.0 / 60.0;

fn database() -> Option<&'static Database> {
    crate::fixture::install()?;
    Some(crate::fixture::database().expect("database"))
}

fn health(db: &Database) -> Health {
    Health::new(Arc::new(HealthSettings::new(&db.config).expect("config.Health")), 7)
}

fn run(health: &mut Health, seconds: f32) {
    for _ in 0..(seconds / DT).round() as u32 {
        health.step(DT);
    }
}

/// Uses the whole item and runs the health until its use is over: the
/// stimulator is then on the head and its clock at nought.
fn take(db: &Database, health: &mut Health, template: &str) {
    let m = Arc::new(Medicine::of(db, template).expect("the item"));
    assert!(m.buffs.is_some(), "{template} has a set of buffs");
    health.use_item(m.clone(), 1, m.full(), None, None).expect("used");
    for _ in 0..(60.0 / DT) as u32 {
        health.step(DT);
        if health.existing(Kind::MedEffect, None).is_none() {
            break;
        }
    }
    assert!(health.existing(Kind::MedEffect, None).is_none(), "the use ends");
    assert!(health.existing(Kind::Stimulator, Some(Part::Head)).is_some(), "the stimulator is on");
}

#[test]
fn every_set_an_item_names_is_in_the_install() {
    let Some(db) = database() else { return };
    let settings = HealthSettings::new(&db.config).expect("config.Health");
    let named: Vec<String> = db.items.values().filter_map(|item| Medicine::of(db, &item.id)).filter_map(|m| m.buffs.as_deref().map(str::to_owned)).collect();
    assert!(named.len() >= 40, "{} items name a set", named.len());
    for set in &named {
        assert!(settings.stimulator.sets.contains_key(set), "no set {set}");
    }
    assert!(settings.stimulator.loop_time > 0.0);
}

#[test]
fn propital_heals_raises_three_skills_then_shakes_the_hands() {
    let Some(db) = database() else { return };
    let mut health = health(db);
    health.apply_damage(Part::LeftArm, 20.0, trp_sim::health::DamageKind::Fall);
    let hurt = health.part(Part::LeftArm).current;
    take(db, &mut health, PROPITAL);
    assert!(health.skill_boosts().is_empty(), "a second's delay");
    run(&mut health, 1.1);
    let mut boosts = health.skill_boosts();
    boosts.sort_by_key(|(s, _)| s.index());
    let mut wanted = vec![(Skill::Metabolism, 20.0), (Skill::Health, 20.0), (Skill::Vitality, 20.0)];
    wanted.sort_by_key(|(s, _)| s.index());
    assert_eq!(boosts, wanted);
    // A point a second, the only part hurt taking it all.
    run(&mut health, 10.0);
    let healed = health.part(Part::LeftArm).current - hurt;
    assert!((healed - 10.0).abs() < 0.5, "healed {healed}");
    // Its side effects at 270 s: the tremor and the tunnel vision.
    assert!(health.existing(Kind::Tremor, None).is_none());
    run(&mut health, 270.0 - 11.1 + 0.5);
    assert!(health.existing(Kind::Tremor, None).is_some(), "the hands shake");
    assert!(health.existing(Kind::TunnelVision, None).is_some(), "the tunnel vision");
    // All over at 301 s: the stimulator gone, the skills as they were.
    run(&mut health, 32.0);
    assert!(health.existing(Kind::Stimulator, None).is_none(), "it ends");
    assert!(health.skill_boosts().is_empty());
}

#[test]
fn sj6_adds_to_the_stamina_and_its_second_dose_runs_on() {
    let Some(db) = database() else { return };
    let mut health = health(db);
    take(db, &mut health, SJ6);
    assert_eq!(health.stamina_buffs(), (0.0, 0.0));
    run(&mut health, 1.1);
    assert_eq!(health.stamina_buffs(), (30.0, 2.0));
    // A second at 100 s: the buffs at work get the whole 240 s again from
    // then (what was left and the new, at most the whole).
    run(&mut health, 99.0);
    take(db, &mut health, SJ6);
    assert_eq!(health.effects.iter().filter(|e| e.kind == Kind::Stimulator).count(), 1, "one stimulator, renewed");
    run(&mut health, 200.0);
    assert_eq!(health.stamina_buffs(), (30.0, 2.0), "still at work past the first dose's end");
}

#[test]
fn mule_raises_the_carrying_limits_by_half() {
    let Some(db) = database() else { return };
    let mut health = health(db);
    take(db, &mut health, MULE);
    assert_eq!(health.carry_modifier(), 1.0);
    run(&mut health, 1.1);
    assert!((health.carry_modifier() - 1.5).abs() < 1e-6, "{}", health.carry_modifier());
    // A tenth of a point a second away, until a part is left at nought.
    let before = health.common()[0];
    run(&mut health, 10.0);
    let lost = before - health.common()[0];
    assert!((lost - 1.0).abs() < 0.1, "lost {lost}");
}

#[test]
fn zagustin_stops_blood_loss_while_it_works() {
    let Some(db) = database() else { return };
    let mut health = health(db);
    health.do_bleed(true, Part::Stomach);
    run(&mut health, 1.0);
    assert!(health.existing(Kind::HeavyBleeding, None).is_some());
    take(db, &mut health, ZAGUSTIN);
    // No delay: the bleeding it finds ended at once (`RemoveBleedingEffects`),
    // and damage draws none while it works (`HasBloodLossBlockers`; a
    // bleeding put on by hand is not damage's, and opens).
    assert!(health.blood_loss_blocked());
    assert!(health.existing(Kind::HeavyBleeding, None).is_none(), "the bleeding ended");
    for _ in 0..10 {
        health.apply_damage(Part::RightLeg, 5.0, trp_sim::health::DamageKind::Bullet);
        health.apply_damage(Part::LeftLeg, 5.0, trp_sim::health::DamageKind::Bullet);
    }
    assert!(health.existing(Kind::LightBleeding, None).is_none() && health.existing(Kind::HeavyBleeding, None).is_none(), "no bleeding drawn");
    run(&mut health, 181.0);
    assert!(!health.blood_loss_blocked(), "over at 180 s");
}

#[test]
fn a_skill_s_summary_level_takes_the_buff_and_metabolism_shortens_side_effects() {
    let Some(db) = database() else { return };
    let mut levels = SkillLevels::default();
    levels.set_level(Skill::Strength, 45);
    levels.set_level(Skill::Endurance, 0);
    let boosts = [(Skill::Strength, 20.0), (Skill::Endurance, 20.0)];
    assert_eq!(levels.summary(Skill::Strength, &boosts), 60, "held at 60 above elite");
    assert_eq!(levels.summary(Skill::Endurance, &boosts), 20);
    assert_eq!(levels.summary(Skill::Endurance, &[(Skill::Endurance, -5.0)]), 0);
    assert!(levels.buffs_with(&boosts).strength_elite, "elite while it lasts");
    // Metabolism at 50: SJ1's side effects (200 s) last half as long.
    let mut health = health(db);
    levels.set_level(Skill::Metabolism, 50);
    health.skills = trp_sim::health::HealthSkills::of(&levels);
    take(db, &mut health, SJ1);
    let stim = health.existing(Kind::Stimulator, None).and_then(|e| e.stim.as_ref()).expect("its state");
    let energy = stim.buffs.iter().find(|b| b.personal.kind == trp_sim::health::BuffKind::EnergyRate).expect("the energy's");
    assert!((energy.personal.duration - 100.0).abs() < 1e-3, "{}", energy.personal.duration);
    let strength = stim.buffs.iter().find(|b| b.personal.skill.as_deref() == Some("Strength")).expect("Strength's");
    assert_eq!(strength.personal.duration, 180.0, "a buff's is left");
}
