//! Tests of what the hands and the health give the skills, as the install's weapons and items run.
//! After `docs/spec/skills.md`, "How a skill is earned", "Earning":
//! a reload's, a chamber's and Mag Drills' actions counted as the game's
//! operations raise them (`FirearmEvent::Earn`), and Metabolism by an item
//! used, by the point or once an item with simple XP gain. Passes with a
//! note where no install is configured.

use std::sync::Arc;

use trp_data::unity::Assets;
use trp_sim::condition::earning::{earn, metabolism, EarnRules, Session};
use trp_sim::weapons::firearm::Earn;
use trp_sim::condition::health::{Event, Health, HealthSettings, Kind, Medicine};
use trp_sim::condition::skills::{Skill, SkillLevels};
use trp_sim::{Firearm, FirearmEvent};

const M3: &str = "6259b864ebedf17603599e88";
const MODEL_700: &str = "5bfea6e90db834001b7347f3";
const RHINO: &str = "61a4c8884f95bc3b2c5dc96f";
const MP43: &str = "5580223e4bdc2d1c128b457f";
const AKM: &str = "59d6088586f774275f37482f";
const MRE: &str = "590c5f0d86f77413997acfab";
const FRAME: f32 = 1.0 / 120.0;

fn weapon_of(template: &str) -> Option<Firearm> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, template).expect("the weapon");
    let data = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    Some(Firearm::new(settings, &data))
}

/// What the weapon said over so many seconds.
fn run(w: &mut Firearm, seconds: f32) -> Vec<FirearmEvent> {
    (0..(seconds / FRAME).round() as usize).flat_map(|_| w.step(FRAME)).collect()
}

/// The reload's, the chamber's and Mag Drills' actions said.
fn earned(said: &[FirearmEvent]) -> [usize; 3] {
    let count = |earn: Earn| said.iter().filter(|event| **event == FirearmEvent::Earn(earn)).count();
    [count(Earn::Reload), count(Earn::Chamber), count(Earn::RoundLoaded)]
}

fn at_rest(template: &str, magazine: u32, chamber: bool) -> Option<Firearm> {
    let mut w = weapon_of(template)?;
    (w.magazine, w.chamber) = (Some(magazine), chamber);
    w.start_drawn();
    run(&mut w, 1.0);
    Some(w)
}

/// A tube filled with the bolt shut (the M3): the reload's action as it
/// begins and once for each round committed, the empty chamber's round a
/// chamber action, each round into the tube Mag Drills' (`BaseReloadOperation.
/// Start`, `ReloadInternalMagOperation`'s `OnAddAmmoInChamber`, `AddAmmoToMag`,
/// `CommitReloadWithAmmo`).
#[test]
fn a_tube_s_filling_earns_at_its_start_by_the_round_and_for_its_chamber() {
    let Some(mut w) = at_rest(M3, 2, false) else { return };
    w.reload_shells(3);
    let said = run(&mut w, 10.0);
    assert!(w.is_idle() && w.chamber && w.magazine == Some(4), "{:?} {}", w.magazine, w.chamber);
    assert_eq!(earned(&said), [3, 1, 2], "{said:?}");
    // With a round chambered already, no chamber action.
    let mut w = at_rest(M3, 2, true).expect("the M3");
    w.reload_shells(3);
    let said = run(&mut w, 10.0);
    assert!(w.is_idle() && w.magazine == Some(5));
    assert_eq!(earned(&said), [4, 0, 3]);
}

/// A magazine filled with the bolt open (the Model 700): the same, and the
/// bolt shut on the filled magazine chambers its first round
/// (`ReloadInternalMagWithOpenBoltOperation.CommitReloadWithAmmo`).
#[test]
fn a_filling_with_the_bolt_open_earns_its_chamber_as_the_bolt_shuts() {
    let Some(mut w) = at_rest(MODEL_700, 2, true) else { return };
    w.reload_shells(20);
    let said = run(&mut w, 9.0);
    assert!(w.is_idle() && w.chamber && w.magazine == Some(4), "{:?} {}", w.magazine, w.chamber);
    assert_eq!(earned(&said), [4, 1, 3]);
}

/// A cylinder's reload: the action as it begins and as it ends with a
/// round loaded (`ReloadCylinderMagOperation.EndReload`), no Mag Drills;
/// barrels' reload: once, as it begins.
#[test]
fn a_cylinder_s_reload_earns_twice_and_barrels_once() {
    let Some(mut w) = weapon_of(RHINO) else { return };
    w.found();
    w.draw();
    run(&mut w, 3.0);
    for _ in 0..2 {
        w.set_trigger(true, true);
        run(&mut w, 0.3);
        w.set_trigger(false, true);
        run(&mut w, 2.0);
    }
    assert_eq!(w.magazine, Some(4));
    w.reload_shells(40);
    let said = run(&mut w, 12.0);
    assert!(w.is_idle() && w.magazine == Some(6));
    assert_eq!(earned(&said), [2, 0, 0]);

    let mut w = weapon_of(MP43).expect("the MP-43");
    w.found();
    w.draw();
    run(&mut w, 3.0);
    assert_eq!(w.magazine, Some(0));
    w.reload_shells(40);
    let said = run(&mut w, 12.0);
    assert!(w.is_idle() && w.magazine == Some(2));
    assert_eq!(earned(&said), [1, 0, 0]);
}

/// The chambered round taken out and one put in from the inventory (the
/// AKM, its magazine out): a chamber action as each begins (`Idling.Execute`),
/// and Mag Drills' as the round goes in (`LoadCartridgeToChamber.
/// OnAddAmmoInChamber`).
#[test]
fn a_round_out_of_the_chamber_or_into_it_earns_the_chamber_action() {
    let Some(mut w) = weapon_of(AKM) else { return };
    w.start_drawn();
    w.set_inventory(true);
    run(&mut w, 2.0);
    assert!(w.pull_out_magazine());
    run(&mut w, 2.5);
    assert!(w.magazine.is_none() && w.chamber && w.discharge());
    let said = run(&mut w, 6.0);
    assert!(!w.chamber && said.contains(&FirearmEvent::Discharged));
    assert_eq!(earned(&said), [0, 1, 0]);
    assert!(w.load_chamber());
    let said = run(&mut w, 6.0);
    assert!(w.chamber && said.contains(&FirearmEvent::Chambered));
    assert_eq!(earned(&said), [0, 1, 1]);
}

/// A cylinder's round moved from one chamber into another by the inventory
/// is one operation in the game, one chamber action; one taken out alone
/// is one too.
#[test]
fn a_round_moved_between_a_cylinder_s_chambers_is_one_chamber_action() {
    let Some(mut w) = weapon_of(RHINO) else { return };
    w.found();
    w.draw();
    run(&mut w, 3.0);
    for _ in 0..2 {
        w.set_trigger(true, true);
        run(&mut w, 0.3);
        w.set_trigger(false, true);
        run(&mut w, 2.0);
    }
    w.set_inventory(true);
    run(&mut w, 2.0);
    assert!(w.unload_own_chamber(4, true));
    let mut said = run(&mut w, 6.0);
    assert!(w.load_own_chamber(1));
    said.extend(run(&mut w, 8.0));
    assert!(w.in_chambers[1].round && !w.in_chambers[4].round);
    assert_eq!(earned(&said), [0, 1, 0]);
    assert!(w.unload_own_chamber(3, false));
    let said = run(&mut w, 6.0);
    assert_eq!(earned(&said), [0, 1, 0]);
}

/// Metabolism by an MRE used to its end, energy low: by the game's rule
/// in a raid its 70 energy, eight points a point through the fatigue, are
/// worth about 12.6 points; with simple XP gain the use is eight, and the
/// MRE's energy rate after it (0.1 a second for 300 s) adds nothing; a
/// food or drink that raises both is sixteen.
#[test]
fn metabolism_by_the_point_or_once_an_item() {
    let Some(_) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let rules = EarnRules::read(&db.config);
    let settings = Arc::new(HealthSettings::new(&db.config).expect("config.Health"));
    // Uses an item to its end with energy and hydration low, then runs the
    // health `after` seconds more: what it told in the use, and after.
    let used = |template: &str, after: f32| -> (Vec<Event>, Vec<Event>) {
        let mut health = Health::new(settings.clone(), 7);
        (health.energy[0], health.hydration[0]) = (10.0, 10.0);
        let m = Arc::new(Medicine::of(db, template).expect("the item"));
        health.use_item(m.clone(), 1, m.full(), None, None).expect("used");
        let mut during = Vec::new();
        for _ in 0..(60.0 / FRAME) as u32 {
            health.step(FRAME);
            during.extend(health.take_events());
            if health.existing(Kind::MedEffect, None).is_none() {
                break;
            }
        }
        let mut later = Vec::new();
        for _ in 0..(after / FRAME) as u32 {
            health.step(FRAME);
            later.extend(health.take_events());
        }
        (during, later)
    };
    let worth = |simple: bool, events: &[Event]| {
        let (mut levels, mut session) = (SkillLevels::default(), Session::new(&rules));
        for event in events {
            for act in metabolism(event, simple) {
                earn(&mut levels, &mut session, &rules, act, 0.0, simple, |_| false);
            }
        }
        levels.progress(Skill::Metabolism)
    };
    let (during, later) = used(MRE, 310.0);
    let energy: f32 = during.iter().map(|e| if let Event::EnergyGained(n) = e { *n } else { 0.0 }).sum();
    let rate: f32 = later.iter().map(|e| if let Event::EnergyGained(n) = e { *n } else { 0.0 }).sum();
    let fed: Vec<&Event> = during.iter().filter(|e| matches!(e, Event::Fed { .. })).collect();
    println!("the MRE: {energy:.2} energy in its use, {rate:.2} by its rate after; told {fed:?}");
    assert!((energy - 70.0).abs() < 0.5 && (rate - 30.0).abs() < 1.5, "{energy} {rate}");
    assert_eq!(fed.len(), 1);
    // 12.6 points: level 0's ten at 10 each, the rest at 5.
    let fatigued = worth(false, &during);
    println!("  by the point through the fatigue: progress {fatigued:.1}; with simple XP gain {:.1}", worth(true, &during));
    assert!((fatigued - 113.0).abs() < 1.5, "{fatigued}");
    assert!((worth(true, &during) - 80.0).abs() < 1e-3, "eight points");
    assert_eq!(worth(true, &later), 0.0, "the rate's gains alone give nothing");
    assert!(worth(false, &later) > 0.0, "by the game's rule they do");
    // A food or drink that raises both.
    let both = db
        .items
        .values()
        .filter_map(|item| Medicine::of(db, &item.id))
        .find(|m| m.food() && m.energy > 0.0 && m.hydration > 0.0)
        .expect("a food or drink that raises both");
    let (during, _) = used(&both.template, 0.0);
    println!("  {} (energy {}, hydration {}): progress {:.1} with simple XP gain", both.template, both.energy, both.hydration, worth(true, &during));
    assert!((worth(true, &during) - 130.0).abs() < 1e-3, "sixteen points: level 0's ten at 10 each, six at 5");
}
