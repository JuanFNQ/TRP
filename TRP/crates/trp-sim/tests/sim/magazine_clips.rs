//! A magazine's own clips (`magAnimationIndex`; firing spec, "Which
//! magazine"): the build's fitted magazine gives the settings' index; a
//! reload tells the animator the fitted one's as the old and the incoming
//! one's as the new, the new one's kept from its `MagIn` and on the next
//! draw. Those that need the install pass trivially, with a note, where
//! none is configured.

use trp_sim::weapon_animator::WeaponAnimatorData;
use trp_sim::{Firearm, FirearmEvent};

const AKM: &str = "59d6088586f774275f37482f";
/// The AKM with the 75-round drum (`5cbdc23eae9215001136a407`, index 5).
const AKM_DRUM: &str = "59d6088586f774275f37482f[mod_magazine=5cbdc23eae9215001136a407[]]";

#[test]
fn a_reload_plays_the_incoming_magazines_clips() {
    let Some(install) = crate::fixture::install() else {
        eprintln!("no install configured: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");

    // The build's magazine names the clips: the stock 30-round's 0, the
    // drum's 5.
    let drum = trp_data::weapon::assemble_key(&mut assets, db, AKM_DRUM).expect("the AKM with a drum");
    assert_eq!(drum.magazine_animation, Some(5));
    assert_eq!(trp_sim::firearm_settings(&assets, db, &hard, &drum).expect("its settings").magazine_variant, 5);
    let weapon = trp_data::weapon::assemble(&mut assets, db, AKM).expect("the AKM");
    assert_eq!(weapon.magazine_animation, Some(0));
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("its settings");
    assert_eq!(settings.magazine_variant, 0);

    const FRAME: f32 = 1.0 / 120.0;
    let run = |w: &mut Firearm, seconds: f32| -> Vec<FirearmEvent> { (0..(seconds / FRAME).round() as usize).flat_map(|_| w.step(FRAME)).collect() };
    let mut w = Firearm::new(settings, &data);
    w.carried();
    w.draw();
    run(&mut w, 2.0);
    assert!(w.is_idle());
    assert_eq!(w.animator().get("RelTypeOld"), 0.0, "drawn with its 30-round");

    // The drum goes in: the old type the 30-round's, the new the drum's.
    w.next_magazine_variant(5);
    w.reload_with(75);
    run(&mut w, FRAME);
    assert_eq!((w.animator().get("RelTypeOld"), w.animator().get("RelTypeNew")), (0.0, 5.0));
    let said = run(&mut w, 6.0);
    assert!(said.contains(&FirearmEvent::MagazineIn) && w.is_idle(), "the reload is over");
    assert_eq!(w.animator().get("RelTypeOld"), 5.0, "the drum's from its MagIn on");

    // Put away and drawn again: still the drum's.
    w.hide(false);
    run(&mut w, 2.0);
    w.draw();
    run(&mut w, 2.0);
    assert_eq!(w.animator().get("RelTypeOld"), 5.0, "drawn with the drum");

    // Another reload, a 30-round in: the old type the drum's, the new its
    // own.
    w.next_magazine_variant(0);
    w.reload_with(30);
    run(&mut w, FRAME);
    assert_eq!((w.animator().get("RelTypeOld"), w.animator().get("RelTypeNew")), (5.0, 0.0));
}
