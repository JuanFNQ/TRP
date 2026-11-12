//! Tests of a poisoning as the install has it, from the blow to the cure.
//! After `docs/spec/health.md`, "An intoxication": the cultists' knife's blow, its toxin's buff, the
//! intoxication's loop, Immunity's cut and its elite's two rolls, the
//! antidote's buff and Perfotoran's cure. Passes with a note where no
//! install is configured.

use std::sync::Arc;

use trp_data::Database;
use trp_sim::condition::health::{Health, HealthSettings, HealthSkills, Kind, Medicine, Part, SideEffect, Stage};

/// `weapon_cultist_damask`: the cultists' knife, `Buffs_KultistsToxin`.
const DAMASK: &str = "5fc64ea372b0dd78d51159dc";
/// `x-TG-12_antidote`: `Buffs_Antidote`.
const ANTIDOTE: &str = "5fca138c2a7b221b2852a5c6";
const PERFOTORAN: &str = "637b6251104668754b72f8f9";

const DT: f32 = 1.0 / 60.0;

fn database() -> Option<&'static Database> {
    crate::fixture::install()?;
    Some(crate::fixture::database().expect("database"))
}

fn health(db: &Database, seed: u64, immunity: u32) -> Health {
    let mut health = Health::new(Arc::new(HealthSettings::new(&db.config).expect("config.Health")), seed);
    health.skills = HealthSkills::of_levels(|name| if name == "Immunity" { immunity } else { 0 });
    health
}

fn run(health: &mut Health, seconds: f32) {
    for _ in 0..(seconds / DT).round() as u32 {
        health.step(DT);
    }
}

fn knife(db: &Database) -> SideEffect {
    SideEffect::of(&db.items[DAMASK].props).expect("the damask names a set")
}

fn stage(health: &Health) -> Option<Stage> {
    health.effects.iter().find(|e| e.kind == Kind::Intoxication).map(|e| e.stage)
}

/// Uses the whole item and runs until its use is over.
fn take(db: &Database, health: &mut Health, template: &str) {
    let m = Arc::new(Medicine::of(db, template).expect("the item"));
    health.use_item(m.clone(), 1, m.full(), None, None).expect("used");
    for _ in 0..(60.0 / DT) as u32 {
        health.step(DT);
        if health.existing(Kind::MedEffect, None).is_none() {
            break;
        }
    }
    assert!(health.existing(Kind::MedEffect, None).is_none(), "the use ends");
}

#[test]
fn the_damask_carries_two_doses_of_the_cultists_toxin() {
    let Some(db) = database() else { return };
    let side = knife(db);
    assert_eq!((side.buffs.as_str(), side.value, side.max), ("Buffs_KultistsToxin", 2.0, 2.0));
    let settings = HealthSettings::new(&db.config).expect("config.Health");
    assert_eq!(settings.intoxication, [10.0, 15.0, 1.0, 3.0], "delay, residue, damage, loop");
    // Only a blow that took health poisons; two doses, then none.
    let mut health = health(db, 7, 0);
    let mut side = knife(db);
    assert!(!health.try_apply_side_effects(0.0, DAMASK, &mut side));
    assert!(health.try_apply_side_effects(10.0, DAMASK, &mut side));
    assert!(health.try_apply_side_effects(10.0, DAMASK, &mut side));
    assert!(side.empty());
    assert!(!health.try_apply_side_effects(10.0, DAMASK, &mut side));
}

#[test]
fn an_untrained_character_loses_a_point_a_part_every_three_seconds() {
    let Some(db) = database() else { return };
    let mut health = health(db, 7, 0);
    let mut side = knife(db);
    assert!(health.try_apply_side_effects(10.0, DAMASK, &mut side));
    // The toxin's buff waits 6 s on the stimulator's clock.
    run(&mut health, 5.9);
    assert_eq!(stage(&health), None);
    run(&mut health, 0.2);
    assert_eq!(stage(&health), Some(Stage::Added));
    assert!(health.existing(Kind::ImmunityPrevented, None).is_none(), "no roll below elite");
    // Then the intoxication's own 10 s.
    run(&mut health, 9.8);
    assert_eq!(stage(&health), Some(Stage::Added));
    run(&mut health, 0.2);
    assert_eq!(stage(&health), Some(Stage::Started));
    assert_eq!(health.effects.iter().find(|e| e.kind == Kind::Intoxication).map(|e| e.part), Some(Part::Stomach));
    let rate = health.rates()[0];
    assert!((rate + 7.0 / 3.0).abs() < 1e-4, "rate {rate}");
    // A pain on the stomach a second on.
    run(&mut health, 1.0);
    assert!(health.active(Kind::Pain, Some(Part::Stomach)).is_some());
    // Every 3 s a point from each of the seven parts.
    assert_eq!(health.common()[0], 440.0);
    run(&mut health, 2.0);
    assert_eq!(health.common()[0], 433.0);
    run(&mut health, 3.0);
    assert_eq!(health.common()[0], 426.0);
    // No medicine, no end: an hour on, every part at nought or dead.
    run(&mut health, 3600.0);
    assert!(!health.alive, "poisoned to death");
}

#[test]
fn immunity_halves_the_damage_at_level_fifty() {
    let Some(db) = database() else { return };
    let mut health = health(db, 7, 50);
    let mut side = knife(db);
    health.try_apply_side_effects(10.0, DAMASK, &mut side);
    run(&mut health, 16.1);
    assert_eq!(stage(&health), Some(Stage::Started), "level 50 rolls nothing");
    let rate = health.rates()[0];
    assert!((rate + 3.5 / 3.0).abs() < 1e-4, "rate {rate}");
    run(&mut health, 3.0);
    assert_eq!(health.common()[0], 436.5);
}

#[test]
fn immunitys_elite_keeps_off_ninety_nine_in_a_hundred_and_says_so() {
    let Some(db) = database() else { return };
    let tries = 600;
    let (mut poisoned, mut twice) = (0, 0);
    for seed in 0..tries {
        let mut health = health(db, seed, 51);
        let mut side = knife(db);
        health.try_apply_side_effects(10.0, DAMASK, &mut side);
        run(&mut health, 6.1);
        // The side effect's roll, and the poison's where that one let it
        // through: the notice put on by every roll, kept or not, a second
        // lengthening the first's 2 s of work by 2 (`AddWorkTime`).
        let notice = health.existing(Kind::ImmunityPrevented, Some(Part::Chest)).expect("the notice");
        assert_eq!(notice.stage, Stage::Started);
        if notice.work > 3.0 {
            twice += 1;
        }
        if stage(&health).is_some() {
            poisoned += 1;
        }
        // Its work, then 8 s of residue.
        run(&mut health, 4.1);
        assert!(health.effects.iter().any(|e| e.kind == Kind::ImmunityPrevented && e.stage == Stage::Residual));
        run(&mut health, 8.1);
        assert!(health.effects.iter().all(|e| e.kind != Kind::ImmunityPrevented));
    }
    // One in ten gets past the first roll (about 60 of 600), and one in a
    // hundred past both (0.1 × 0.1: about 6).
    eprintln!("Immunity elite: {twice} of {tries} rolled twice, {poisoned} poisoned");
    assert!((30..=100).contains(&twice), "{twice} of {tries} rolled twice");
    assert!(poisoned <= 20 && poisoned <= twice, "{poisoned} of {tries} poisoned");
}

#[test]
fn the_antidote_ends_an_intoxication_and_holds_off_the_next() {
    let Some(db) = database() else { return };
    let mut health = health(db, 7, 0);
    let mut side = knife(db);
    health.try_apply_side_effects(10.0, DAMASK, &mut side);
    run(&mut health, 17.0);
    assert_eq!(stage(&health), Some(Stage::Started));
    take(db, &mut health, ANTIDOTE);
    assert!(health.antidote_at_work() || stage(&health) == Some(Stage::Started), "its buff waits its delay");
    run(&mut health, 6.1);
    assert!(health.antidote_at_work());
    assert_eq!(stage(&health), Some(Stage::Residual));
    run(&mut health, 15.1);
    assert_eq!(stage(&health), None, "15 s of residue");
    // A blow while the antidote works: the toxin acts and nothing begins.
    let before = health.common()[0];
    health.try_apply_side_effects(10.0, DAMASK, &mut side);
    run(&mut health, 30.0);
    assert!(health.antidote_at_work());
    assert_eq!(stage(&health), None);
    assert!(health.common()[0] >= before);
}

#[test]
fn perfotoran_cures_it_as_its_use_ends() {
    let Some(db) = database() else { return };
    let mut health = health(db, 7, 0);
    let mut side = knife(db);
    health.try_apply_side_effects(10.0, DAMASK, &mut side);
    run(&mut health, 17.0);
    assert_eq!(stage(&health), Some(Stage::Started));
    take(db, &mut health, PERFOTORAN);
    assert_eq!(stage(&health), Some(Stage::Residual), "its cure, before its antidote's second");
}
