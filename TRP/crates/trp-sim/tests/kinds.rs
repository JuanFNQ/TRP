//! One weapon of each kind whose logic was written on 2026-10-13, run on
//! its own animator from the install: a bolt action whose magazine comes
//! out, a self-loader filled through its action, a revolver, a double
//! barrel and a launcher with a drum (`docs/spec/more-weapons.md`). These
//! pass trivially, with a note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test kinds -- --nocapture` for what
//! each does.

use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::{FireMode, Firearm, FirearmEvent};

const SV98: &str = "55801eed4bdc2d89578b4588";
const SKS: &str = "574d967124597745970e7c94";
const RHINO: &str = "61a4c8884f95bc3b2c5dc96f";
const MP43: &str = "5580223e4bdc2d1c128b457f";
const MSGL: &str = "6275303a9f372d6ea97f9ec7";
const FRAME: f32 = 1.0 / 120.0;

fn weapon_of(template: &str) -> Option<Firearm> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, template).expect("the weapon");
    let data = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    println!(
        "{}: {} rounds, modes {:?}, filled a round at a time {}, bolt worked by hand {}, catch {}",
        weapon.name, settings.magazine_capacity, settings.modes, settings.internal_magazine, settings.bolt_action, settings.bolt_catch
    );
    Some(Firearm::new(settings, &data))
}

/// Steps a weapon and says which states its animator entered and what it
/// said, each with its time.
fn run(w: &mut Firearm, seconds: f32) -> (Vec<(String, f32)>, Vec<(FirearmEvent, f32)>) {
    let name = |w: &Firearm| {
        let (state, _, next) = w.animator().state();
        next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
    };
    let (mut entered, mut said, mut last) = (Vec::new(), Vec::new(), name(w));
    for frame in 0..(seconds / FRAME).round() as usize {
        let at = (frame + 1) as f32 * FRAME;
        said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_) | FirearmEvent::BodyAction(_))).map(|e| (e, at)));
        let now = name(w);
        if now != last {
            entered.push((now.clone(), at));
            last = now;
        }
    }
    (entered, said)
}

fn names(entered: &[(String, f32)]) -> Vec<&str> {
    entered.iter().map(|entry| entry.0.as_str()).collect()
}

fn told(entered: &[(String, f32)]) -> String {
    entered.iter().map(|(state, at)| format!("{state} {at:.2}")).collect::<Vec<_>>().join(" > ")
}


fn shots(said: &[(FirearmEvent, f32)]) -> usize {
    said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count()
}

/// Presses the trigger for a moment and lets the weapon come to rest.
fn press(w: &mut Firearm, seconds: f32) -> (Vec<(String, f32)>, Vec<(FirearmEvent, f32)>) {
    w.set_trigger(true, true);
    let (mut entered, mut said) = run(w, 0.3);
    w.set_trigger(false, true);
    let (more, more_said) = run(w, seconds);
    entered.extend(more.into_iter().map(|(state, at)| (state, at + 0.3)));
    said.extend(more_said);
    (entered, said)
}

/// A bolt action whose magazine comes out (the SV-98): the Model 700's
/// firing and the AKM's reload on its own animator, with the chamber
/// opened first when it is empty and shut on a round at the end.
#[test]
fn a_bolt_action_with_a_magazine_that_comes_out() {
    let Some(mut w) = weapon_of(SV98) else { return };
    w.found();
    w.draw();
    let (entered, _) = run(&mut w, 5.0);
    println!("  drawn as found: {}; {:?} + {}", told(&entered), w.magazine, w.chamber as u8);
    assert_eq!(names(&entered), ["OUT TO IDLE ARM", "IDLE"]);
    assert_eq!((w.magazine, w.chamber), (Some(9), true));
    // A shot: the bolt is worked when the trigger is let go.
    let (entered, said) = press(&mut w, 4.0);
    println!("  a shot: {}", told(&entered));
    assert_eq!((names(&entered), shots(&said)), (vec!["FIRE", "FIRE RELOAD", "IDLE"], 1));
    assert_eq!((w.magazine, w.chamber), (Some(8), true));
    // A reload with a round chambered keeps it.
    w.reload();
    let (entered, _) = run(&mut w, 8.0);
    println!("  reloaded: {}", told(&entered));
    assert_eq!(names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
    assert_eq!((w.magazine, w.chamber), (Some(10), true));
    // From empty: the chamber is opened, and shut on the first round.
    (w.chamber, w.magazine) = (false, Some(0));
    w.reload();
    let (entered, _) = run(&mut w, 10.0);
    println!("  reloaded from empty: {}", told(&entered));
    assert_eq!(names(&entered).first(), Some(&"OPEN CHAMBER"));
    assert!(names(&entered).contains(&"RELOAD CLOSE CHAMBER") && w.is_idle());
    assert_eq!((w.magazine, w.chamber), (Some(9), true));
}

/// A self-loader filled through its action (the SKS): it fires as any
/// self-loader, and is filled a round at a time with its bolt open, which
/// throws the chambered round out first.
#[test]
fn a_self_loader_filled_through_its_action() {
    let Some(mut w) = weapon_of(SKS) else { return };
    w.found();
    w.draw();
    let (entered, _) = run(&mut w, 5.0);
    println!("  drawn as found: {}; {:?} + {}", told(&entered), w.magazine, w.chamber as u8);
    assert_eq!((w.magazine, w.chamber), (Some(9), true));
    let (entered, said) = press(&mut w, 2.0);
    println!("  a shot: {}", told(&entered));
    assert_eq!((names(&entered), shots(&said)), (vec!["FIRE", "IDLE"], 1));
    assert_eq!((w.magazine, w.chamber), (Some(8), true));
    w.reload_shells(40);
    let (entered, said) = run(&mut w, 8.0);
    println!("  filled: {}; said {:?}", told(&entered), said.iter().map(|s| &s.0).collect::<Vec<_>>());
    assert_eq!(names(&entered), ["RELOAD START INTERNAL", "RELOAD IN INTERNAL", "RELOAD END INTERNAL", "IDLE"]);
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::RoundEjected));
    assert_eq!((w.magazine, w.chamber), (Some(9), true));
}

/// A revolver (the Rhino 50DS): six in the cylinder and no chamber apart;
/// fired single action it is cocked again after each shot; the cases stay
/// in their chambers until a reload takes them out, one at a time, and a
/// round goes into each chamber freed; double action fires from a hammer
/// that is down.
#[test]
fn a_revolver_fires_from_its_cylinder() {
    let Some(mut w) = weapon_of(RHINO) else { return };
    assert_eq!(w.settings.cylinder, Some((6, false)));
    assert_eq!(w.settings.modes, [FireMode::Single, FireMode::DoubleAction]);
    w.found();
    w.draw();
    let (entered, _) = run(&mut w, 3.0);
    assert_eq!((names(&entered), w.magazine, w.chamber), (vec!["OUT TO IDLE", "IDLE"], Some(6), false));
    // Two shots, single action.
    for shot in 1..=2 {
        let (entered, said) = press(&mut w, 2.0);
        println!("  shot {shot}: {}; the cylinder at {}", told(&entered), w.camora);
        assert_eq!((names(&entered), shots(&said)), (vec!["FIRE", "FIRE ARM", "IDLE"], 1));
        assert_eq!((w.magazine, w.camora), (Some(6 - shot), shot as usize));
    }
    assert_eq!(w.in_chambers.iter().filter(|c| **c == trp_sim::firearm::Chamber::Shell).count(), 2);
    // A reload: both cases out, a round into each chamber.
    w.reload_shells(40);
    let (entered, said) = run(&mut w, 12.0);
    println!("  reloaded: {}; said {:?}", told(&entered), said.iter().map(|s| &s.0).collect::<Vec<_>>());
    assert_eq!(names(&entered), ["RELOAD START", "REMOVE SHELLS", "RELOAD IN", "RELOAD END", "IDLE"]);
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::RoundsTaken(2)));
    assert_eq!(w.magazine, Some(6));
    assert!(w.in_chambers.iter().all(|c| *c == trp_sim::firearm::Chamber::Round) && w.is_idle());
    // Double action: the next shot is from the cocked hammer still, and
    // leaves it down; the one after turns the cylinder itself.
    assert!(w.set_fire_mode(FireMode::DoubleAction));
    run(&mut w, 1.0);
    let (first, said) = press(&mut w, 2.0);
    assert_eq!(shots(&said), 1);
    assert!(w.hammer_closed);
    let (second, said) = press(&mut w, 2.0);
    println!("  double action: {} | {}", told(&first), told(&second));
    assert_eq!(shots(&said), 1);
    assert!(names(&second).contains(&"DOUBLE_ACTION_FIRE"), "{second:?}");
    assert_eq!(w.magazine, Some(4));
    // Emptied, the hammer falls on spent cases: no shot.
    for _ in 0..4 {
        press(&mut w, 2.0);
    }
    assert_eq!(w.magazine, Some(0));
    let (entered, said) = press(&mut w, 2.0);
    println!("  on a spent cylinder: {}", told(&entered));
    assert_eq!(shots(&said), 0);
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::DryFire));
    // An empty cylinder throws all its cases out at once.
    w.reload_shells(40);
    let (entered, _) = run(&mut w, 20.0);
    println!("  reloaded from empty: {}", told(&entered));
    assert_eq!(w.magazine, Some(6));
    assert!(w.is_idle());
}

/// Barrels loaded one by one (the MP-43): found, they are empty; a press
/// fires the first that is loaded, and both together in the mode for it;
/// a reload throws the cases out and puts a round in each barrel that has
/// none.
#[test]
fn barrels_are_loaded_one_by_one() {
    let Some(mut w) = weapon_of(MP43) else { return };
    assert_eq!((w.settings.barrels, w.settings.cylinder), (2, None));
    assert_eq!(w.settings.modes, [FireMode::Single, FireMode::Doublet]);
    w.found();
    w.draw();
    let (entered, _) = run(&mut w, 3.0);
    println!("  drawn as found: {}; {:?}", told(&entered), w.magazine);
    assert_eq!(w.magazine, Some(0));
    let (_, said) = press(&mut w, 1.0);
    assert!(shots(&said) == 0 && said.iter().any(|(event, _)| *event == FirearmEvent::DryFire));
    w.reload_shells(40);
    let (entered, said) = run(&mut w, 12.0);
    println!("  loaded: {}; said {:?}", told(&entered), said.iter().map(|s| &s.0).collect::<Vec<_>>());
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::RoundsTaken(2)));
    assert!(w.magazine == Some(2) && w.is_idle());
    // One barrel, then the other.
    for left in [1, 0] {
        let (entered, said) = press(&mut w, 2.0);
        println!("  a shot: {}", told(&entered));
        assert_eq!((shots(&said), w.magazine), (1, Some(left)));
    }
    w.reload_shells(1);
    let (entered, _) = run(&mut w, 12.0);
    println!("  one shell in reach: {}", told(&entered));
    assert_eq!(w.magazine, Some(1));
    w.reload_shells(40);
    run(&mut w, 12.0);
    assert_eq!(w.magazine, Some(2));
    // Both at once.
    assert!(w.set_fire_mode(FireMode::Doublet));
    run(&mut w, 1.5);
    let (entered, said) = press(&mut w, 2.0);
    println!("  both barrels: {}", told(&entered));
    assert_eq!((shots(&said), w.magazine), (2, Some(0)));
}

/// A launcher with a drum (the M32A1): a cylinder a spring turns, fired
/// chamber after chamber up to its last, and loaded from its first again.
#[test]
fn a_launcher_fires_from_a_spring_driven_drum() {
    let Some(mut w) = weapon_of(MSGL) else { return };
    assert_eq!(w.settings.cylinder, Some((6, true)));
    w.found();
    w.draw();
    run(&mut w, 3.0);
    assert_eq!(w.magazine, Some(6));
    for shot in 1..=6usize {
        let (entered, said) = press(&mut w, 2.5);
        if shot == 1 {
            println!("  a shot: {}", told(&entered));
        }
        assert_eq!((shots(&said), w.magazine), (1, Some(6 - shot as u32)));
        assert_eq!(w.camora, shot.min(5));
    }
    let (_, said) = press(&mut w, 2.0);
    assert_eq!(shots(&said), 0);
    w.reload_shells(40);
    let (entered, said) = run(&mut w, 30.0);
    println!("  reloaded from empty: {}; said {:?}", told(&entered), said.iter().map(|s| &s.0).collect::<Vec<_>>());
    assert_eq!((w.magazine, w.camora), (Some(6), 0));
    assert!(w.is_idle());
}

const MP18: &str = "61f7c9e189e6fb1a5e3ea78d";
const FN40GL: &str = "5e81ebcd8e146c7080625e15";
const RSHG2: &str = "676bf44c5539167c3603e869";

/// One barrel loaded alone (the MP-18 and the FN40GL): found empty,
/// loaded; fired, its case thrown out as the next goes in; loaded again
/// over a live round, which comes out (more-weapons spec, "One barrel
/// loaded alone, and the rocket launcher").
#[test]
fn one_barrel_is_loaded_alone_whatever_it_holds() {
    for template in [MP18, FN40GL] {
        let Some(mut w) = weapon_of(template) else { return };
        assert_eq!((w.settings.barrels, w.settings.rocket_launcher), (1, false));
        w.found();
        w.draw();
        run(&mut w, 3.0);
        assert_eq!(w.magazine, Some(0));
        let reload = |w: &mut Firearm, what: &str| {
            w.reload_shells(20);
            let (entered, said) = run(w, 10.0);
            let said: Vec<&FirearmEvent> = said.iter().map(|s| &s.0).collect();
            println!("  {what}: {}; said {said:?}", told(&entered));
            assert!(w.is_idle() && w.magazine == Some(1), "{what}");
            (entered, said.into_iter().cloned().collect::<Vec<_>>())
        };
        let (_, said) = reload(&mut w, "from empty");
        assert!(said.contains(&FirearmEvent::RoundsTaken(1)) && !said.contains(&FirearmEvent::RoundUnloaded));
        let (_, fired) = press(&mut w, 2.0);
        assert_eq!((shots(&fired), w.magazine), (1, Some(0)));
        let (_, said) = reload(&mut w, "over a case");
        assert!(!said.contains(&FirearmEvent::RoundUnloaded));
        let (_, said) = reload(&mut w, "over a live round");
        assert!(said.contains(&FirearmEvent::RoundUnloaded), "the live round comes out");
        // With nothing in reach the key does nothing.
        w.reload_shells(0);
        assert!(w.is_idle());
    }
}

/// The rocket launcher (the RShG-2): found with its rocket; fired from
/// the sights only, by its own firing; empty after, and idle.
#[test]
fn the_rocket_launcher_fires_from_the_sights() {
    let Some(mut w) = weapon_of(RSHG2) else { return };
    assert!(w.settings.rocket_launcher);
    w.found();
    w.draw();
    let (entered, _) = run(&mut w, 5.0);
    println!("  drawn as found: {}; {:?}", told(&entered), w.magazine);
    assert!(w.is_idle() && w.magazine == Some(1));
    // The sights up: its own clip's mark has the animator ready to fire.
    w.set_aim(true, true);
    let (entered, _) = run(&mut w, 1.0);
    println!("  the sights up: {}; IsAimingFloat {}", told(&entered), w.animator().get("IsAimingFloat"));
    assert!(w.animator().get("IsAimingFloat") > 0.9);
    let (entered, said) = press(&mut w, 3.0);
    println!("  fired: {}", told(&entered));
    assert_eq!((shots(&said), w.magazine), (1, Some(0)));
    assert!(names(&entered).contains(&"FIRE") && !w.aiming() && w.is_idle());
}
