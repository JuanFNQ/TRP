//! Tests of rounds a magazine takes but the chamber does not.
//! After the firing spec, "Rounds the chamber does not take": the install's AKM takes .366 TKM rounds into its magazines
//! but not into its chamber, and the game feeds a chamber only from a
//! magazine whose top round it takes (`Magazine.IsAmmoCompatible`). Passes
//! trivially, with a note, where no install is configured.

use std::sync::Arc;

use trp_sim::items::inventory::{self, Item, Kind};
use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
use trp_sim::{Firearm, FirearmEvent};

const AKM: &str = "59d6088586f774275f37482f";
/// The AKM's own 30-round magazine (`mag_ak_izhmash_6L10_762x39_30`).
const MAGAZINE: &str = "5a01c29586f77474660c694c";
/// 7.62x39mm PS gzh and .366 TKM AP-M.
const PS: &str = "5656d7c34bdc2d9d198b4587";
const AP_M: &str = "5f0596629e22f464da6bbdd9";

#[test]
fn a_magazine_of_rounds_the_chamber_refuses_feeds_none() {
    let Some(install) = crate::fixture::install() else {
        eprintln!("no install configured: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");

    // The install's filters: the magazine takes both rounds, the chamber the
    // 7.62 alone; the count the chamber refuses is from the magazine's foot.
    let kind = |template: &str| Arc::new(Kind::of(db, template).expect(template));
    let (ps, ap) = (kind(PS), kind(AP_M));
    let mut akm = Item::new(kind(AKM));
    assert_eq!((inventory::chamber_takes(&akm, &ps), inventory::chamber_takes(&akm, &ap)), (Some(true), Some(false)));
    let mut magazine = Item::new(kind(MAGAZINE));
    assert!(inventory::takes_round(&magazine, &ps) && inventory::takes_round(&magazine, &ap), "the magazine takes both");
    let slot = akm.slot_index(inventory::MAGAZINE_SLOT).expect("its magazine's slot");
    inventory::fill_magazine(&mut magazine, ap.clone(), 30);
    akm.slots[slot] = Some(magazine.clone());
    assert_eq!((inventory::fitted_unfit_rounds(&akm), inventory::ammo_incompatible(&akm)), (30, true));
    // Two 7.62 over 28 .366.
    inventory::fill_magazine(&mut magazine, ap.clone(), 28);
    magazine.loose.push(Item { count: 2, ..Item::new(ps.clone()) });
    akm.slots[slot] = Some(magazine);
    let mixed = inventory::fitted_unfit_rounds(&akm);
    assert_eq!(mixed, 28);
    assert!(!inventory::ammo_incompatible(&akm), "a 7.62 on top");

    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, db, AKM).expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("its settings");
    const FRAME: f32 = 1.0 / 120.0;
    let run = |w: &mut Firearm, seconds: f32| -> Vec<FirearmEvent> { (0..(seconds / FRAME).round() as usize).flat_map(|_| w.step(FRAME)).collect() };
    let shots = |said: &[FirearmEvent]| said.iter().filter(|e| matches!(e, FirearmEvent::Shot { .. })).count();
    // Five presses, whatever the fire mode: what they said, and the hands'
    // states passed through.
    let presses = |w: &mut Firearm| {
        let mut said = Vec::new();
        for _ in 0..5 {
            w.set_trigger(true, true);
            said.extend(run(w, 0.4));
            w.set_trigger(false, true);
            said.extend(run(w, 0.4));
        }
        said
    };
    let states = |w: &mut Firearm, seconds: f32| {
        let mut seen: Vec<String> = Vec::new();
        for _ in 0..(seconds / FRAME).round() as usize {
            w.step(FRAME);
            let state = w.animator().state().0.to_owned();
            if seen.last() != Some(&state) {
                seen.push(state);
            }
        }
        seen
    };

    // In hand with a round chambered over a magazine of .366: that round
    // fires, nothing follows, every press after clicks, and each press says
    // why.
    let mut w = Firearm::new(settings, &data);
    w.carried();
    w.set_magazine_unfit(30);
    w.draw();
    run(&mut w, 2.0);
    assert!(w.is_idle() && w.chamber && !w.ammo_compatible());
    let said = presses(&mut w);
    assert_eq!(shots(&said), 1, "the chambered round alone");
    assert_eq!((w.chamber, w.magazine), (false, Some(30)), "nothing fed");
    assert!(said.contains(&FirearmEvent::DryFire), "the presses after click");
    assert_eq!(said.iter().filter(|e| **e == FirearmEvent::IncompatibleAmmo).count(), 5, "said at each press");

    // A reload brings two 7.62 over 28 .366: the first chambered, the second
    // fed after the first shot, then nothing.
    w.next_magazine_unfit(mixed);
    w.reload_with(30);
    run(&mut w, 6.0);
    assert!(w.is_idle() && w.chamber && w.ammo_compatible(), "the 7.62 chambered");
    assert_eq!(w.magazine, Some(29));
    let said = presses(&mut w);
    assert_eq!(shots(&said), 2, "the two 7.62 rounds and no more");
    assert_eq!((w.chamber, w.magazine), (false, Some(28)));
    assert!(said.contains(&FirearmEvent::IncompatibleAmmo), "said once the .366 is on top");

    // A reload brings 30 .366: the animator is told, the reload ends without
    // the charge, nothing is chambered.
    w.next_magazine_unfit(30);
    w.reload_with(30);
    let seen = states(&mut w, 6.0);
    assert!(w.animator().flag("IncompatibleAmmo"), "the animator told");
    assert!(w.is_idle() && !w.chamber && w.magazine == Some(30), "nothing chambered: {seen:?}");
    assert!(seen.iter().any(|s| s.ends_with("RELOAD END")) && !seen.iter().any(|s| s.contains("CHARGE")), "{seen:?}");

    // Put away and drawn again: the draw racks nothing.
    w.hide(false);
    run(&mut w, 2.0);
    w.draw();
    let seen = states(&mut w, 2.0);
    assert!(w.is_idle() && !w.chamber && w.magazine == Some(30), "{seen:?}");
    assert!(!seen.iter().any(|s| s.contains("IDLE ARM")), "{seen:?}");

    // A magazine of 7.62 put in by a reload: compatible again, chambered.
    w.next_magazine_unfit(0);
    w.reload_with(30);
    run(&mut w, 6.0);
    assert!(!w.animator().flag("IncompatibleAmmo"));
    assert!(w.is_idle() && w.chamber && w.magazine == Some(29));
}
