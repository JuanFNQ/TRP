//! A weapon with no chamber (`FirearmSettings::no_chamber`; more-weapons
//! spec, "A weapon with no chamber"): the nine of the install's that have
//! none have the flag and the AKM has not; the M60E6 holds its box's
//! rounds and no more, fires them straight off it with the trigger held,
//! and a reload chambers nothing. Those that need the install pass
//! trivially, with a note, where none is configured.

use trp_sim::weapon_animator::WeaponAnimatorData;
use trp_sim::{Firearm, FirearmEvent};

const AKM: &str = "59d6088586f774275f37482f";
const M60E6_FDE: &str = "661cec09b2c6356b4d0c7a36";
/// The PPSh-41, the RPD and RPDN, the M60E4, the two M60E6, the Uzi, the
/// two AA-12s: `Chambers` empty with a detachable magazine.
const NO_CHAMBER: [&str; 9] = [
    "5ea03f7400685063ec28bfa8",
    "6513ef33e06849f06c0957ca",
    "65268d8ecb944ff1e90ea385",
    "65fb023261d5829b2d090755",
    "661ceb1b9311543c7104149b",
    M60E6_FDE,
    "66992b349950f5f4cd06029f",
    "66ffa9b66e19cc902401c5e8",
    "67124dcfa3541f2a1f0e788b",
];

#[test]
fn the_weapons_with_no_chamber_are_the_games() {
    let Some(install) = crate::fixture::install() else {
        eprintln!("no install configured: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    for template in NO_CHAMBER.iter().chain([&AKM]) {
        let weapon = trp_data::weapon::assemble(&mut assets, db, template).expect("the weapon");
        let settings = trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("its settings");
        assert_eq!(settings.no_chamber, *template != AKM, "{} ({template})", weapon.name);
    }
}

#[test]
fn the_m60e6_fires_straight_off_its_box() {
    let Some(install) = crate::fixture::install() else {
        eprintln!("no install configured: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, db, M60E6_FDE).expect("the M60E6");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("its settings");
    let capacity = settings.magazine_capacity;
    const FRAME: f32 = 1.0 / 120.0;
    let run = |w: &mut Firearm, seconds: f32| -> Vec<FirearmEvent> { (0..(seconds / FRAME).round() as usize).flat_map(|_| w.step(FRAME)).collect() };
    let shots = |said: &[FirearmEvent]| said.iter().filter(|e| matches!(e, FirearmEvent::Shot { .. })).count() as u32;

    // Carried in hand: its box full, nothing in a chamber, nothing taken at
    // the draw.
    let mut w = Firearm::new(settings, &data);
    w.carried();
    assert!(!w.chamber && w.magazine == Some(capacity), "{:?} {}", w.magazine, w.chamber);
    w.draw();
    let said = run(&mut w, 2.0);
    let (state, time, next) = w.animator().state();
    assert!(w.is_idle(), "drawn: {state} {time} {next:?}; said {said:?}");
    assert_eq!(w.magazine, Some(capacity), "the draw takes no round");
    assert!(w.loaded() && !w.chamber, "ready to fire off its box");

    // Held for a second: each shot one round off the box, none in a
    // chamber; let go, the run ends.
    w.set_trigger(true, true);
    let said = run(&mut w, 1.0);
    w.set_trigger(false, true);
    let said: Vec<FirearmEvent> = said.into_iter().chain(run(&mut w, 1.0)).collect();
    let fired = shots(&said);
    assert!(fired >= 8, "about 550 a minute: {fired} in a second");
    assert_eq!(w.magazine, Some(capacity - fired), "every shot off the box");
    assert!(!w.chamber);

    // Down to its last rounds: held, it fires every one and stops dry.
    w.magazine = Some(3);
    w.set_trigger(true, true);
    let said = run(&mut w, 2.0);
    w.set_trigger(false, true);
    run(&mut w, 1.0);
    assert_eq!(shots(&said), 3, "the box's last three, and no more");
    assert_eq!(w.magazine, Some(0));
    assert!(!w.loaded());

    // A full box put in (the M60E6's goes in late: `MagIn` at 0.91 of
    // its 4.5 s clip): nothing to chamber, the box's count all of it.
    w.reload();
    let said = run(&mut w, 15.0);
    let (state, time, next) = w.animator().state();
    assert!(w.is_idle(), "the reload is over: {state} {time} {next:?}; said {said:?}");
    assert_eq!(w.magazine, Some(capacity));
    assert!(!w.chamber, "a reload chambers nothing");
}
