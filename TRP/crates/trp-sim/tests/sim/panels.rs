//! What the battle screen's ammunition panel is told by the hands
//! (inventory spec, "What the ammunition's panel says"; `ShowAmmoDetails`,
//! `ShowFireMode`): a magazine's count in words by the look's level, and
//! the way of firing named by the fire mode key, the look at the selector
//! and the force to automatic. The words pass without an install; the
//! AKM's events pass trivially, with a note, where none is configured.

use trp_data::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::inventory::{ammo_words, AmmoWords as W};
use trp_sim::{Firearm, FirearmEvent, FireMode};

#[test]
fn a_magazine_s_count_is_said_by_the_look_s_level() {
    // One short of full is full; none is empty, at every level.
    assert_eq!(ammo_words(29, 30, 0, false), W::Full);
    assert_eq!(ammo_words(0, 30, 2, false), W::Empty);
    // Level 0: one of four by (int)(3.33 count / max + 0.5).
    assert_eq!(ammo_words(1, 30, 0, false), W::AlmostEmpty);
    assert_eq!(ammo_words(5, 30, 0, false), W::LessThanHalf);
    assert_eq!(ammo_words(15, 30, 0, false), W::AboutHalf);
    assert_eq!(ammo_words(25, 30, 0, false), W::AlmostFull);
    // Level 1: to the nearest five, the count itself on a magazine of ten
    // or fewer, "fewer than 5" below five.
    assert_eq!(ammo_words(17, 30, 1, false), W::About(15));
    assert_eq!(ammo_words(18, 30, 1, false), W::About(20));
    assert_eq!(ammo_words(3, 30, 1, false), W::FewerThanFive);
    assert_eq!(ammo_words(7, 10, 1, false), W::Count(7));
    // Level 2 on: the count.
    assert_eq!(ammo_words(17, 30, 2, false), W::Count(17));
    // A cylinder: full only when full, else the count.
    assert_eq!(ammo_words(5, 6, 0, true), W::Count(5));
    assert_eq!(ammo_words(6, 6, 0, true), W::Full);
    assert_eq!(W::About(15).key(), ("Approx.", Some(15)));
}

fn akm() -> Option<Firearm> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database()?;
    let hard = HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, db, "59d6088586f774275f37482f").expect("the AKM");
    let animator = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its animator");
    let mut firearm = Firearm::new(trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("its settings"), &animator);
    firearm.start_drawn();
    Some(firearm)
}

#[test]
fn the_hands_tell_the_panel_the_way_of_firing_and_the_magazine() {
    let Some(mut akm) = akm() else { return };
    let shown = |events: &[FirearmEvent]| events.iter().filter_map(|event| if let FirearmEvent::FireModeShown(mode) = event { Some(*mode) } else { None }).collect::<Vec<_>>();
    // At rest: the look at the selector names the mode before it begins.
    let mode = akm.mode();
    assert!(akm.check_fire_mode());
    let events = akm.step(1.0 / 60.0);
    assert_eq!(shown(&events), vec![mode]);
    for _ in 0..600 {
        akm.step(1.0 / 60.0);
    }
    // The fire mode key: named, and changed.
    akm.next_fire_mode();
    let events = akm.step(1.0 / 60.0);
    assert_eq!(shown(&events).len(), 1);
    assert_ne!(akm.mode(), mode);
    // The force to automatic where it is so already: nothing.
    if akm.mode() == FireMode::FullAuto {
        assert!(!akm.force_auto_fire_mode());
        assert!(shown(&akm.step(1.0 / 60.0)).is_empty());
    }
    for _ in 0..600 {
        akm.step(1.0 / 60.0);
    }
    // The look at the magazine begins at rest and says so.
    assert!(akm.check_magazine());
    assert!(akm.step(1.0 / 60.0).contains(&FirearmEvent::MagazineChecked));
    // Not again in the middle of it.
    assert!(!akm.check_magazine());
}
