//! Each kind of action: bolts, self-loaders, revolvers, barrels and the launcher's drum.

use super::*;

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
        assert_eq!(hammers(&said), [false], "single action: cocked at every shot");
    }
    assert_eq!(w.in_chambers.iter().filter(|c| c.case && !c.round).count(), 2);
    // A reload: both cases out, a round into each chamber.
    w.reload_shells(40);
    let (entered, said) = run(&mut w, 12.0);
    println!("  reloaded: {}; said {:?}", told(&entered), said.iter().map(|s| &s.0).collect::<Vec<_>>());
    assert_eq!(names(&entered), ["RELOAD START", "REMOVE SHELLS", "RELOAD IN", "RELOAD END", "IDLE"]);
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::RoundsTaken(2)));
    assert_eq!(w.magazine, Some(6));
    assert!(w.in_chambers.iter().all(|c| c.round && !c.case) && w.is_idle());
    // Double action: the next shot is from the cocked hammer still, and
    // leaves it down; the one after turns the cylinder itself.
    assert!(w.set_fire_mode(FireMode::DoubleAction));
    run(&mut w, 1.0);
    // The first is made with the hammer still cocked (no double action's
    // penalty), the second with it down (`CylinderHammerClosed` at `Shot`).
    let (first, said) = press(&mut w, 2.0);
    assert_eq!(shots(&said), 1);
    assert_eq!(hammers(&said), [false]);
    assert!(w.hammer_closed);
    let (second, said) = press(&mut w, 2.0);
    println!("  double action: {} | {}", told(&first), told(&second));
    assert_eq!(shots(&said), 1);
    assert_eq!(hammers(&said), [true]);
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

/// The Rhino as its recording found it
/// (`local\traces\rhino-s160`): five rounds, its first chamber empty, the
/// cylinder at its third and the hammer down, in double action. The game
/// fired the fourth, fifth and sixth chambers, its hammer fell on the
/// empty first (`DRY FIRE DOUBLE ACTION`, no shot), then it fired the
/// second and third: the animator goes by the weapon's own chambers'
/// count, which a revolver's is 0 whatever its cylinder holds, so a pull on
/// an empty chamber is a click though rounds are left in others.
#[test]
fn a_revolver_clicks_on_its_empty_chamber_with_rounds_left_in_others() {
    let Some(mut w) = weapon_of(RHINO) else { return };
    w.found();
    w.draw();
    run(&mut w, 3.0);
    assert!(w.set_fire_mode(FireMode::DoubleAction));
    run(&mut w, 2.0);
    assert!(w.hold_rounds(&[false, true, true, true, true, true], &[], false));
    (w.camora, w.hammer_closed) = (2, true);
    let mut seen = Vec::new();
    for pull in 1..=6 {
        let (entered, said) = press(&mut w, 2.0);
        println!("  pull {pull}: {}; the cylinder at {}", told(&entered), w.camora);
        seen.push((names(&entered).first().map(|name| name.to_string()).unwrap_or_default(), shots(&said)));
    }
    let fire = || ("DOUBLE_ACTION_FIRE".to_owned(), 1);
    assert_eq!(seen, [fire(), fire(), fire(), ("DRY FIRE DOUBLE ACTION".to_owned(), 0), fire(), fire()]);
    assert_eq!(w.magazine, Some(0));
}

/// A shot from a hammer left down takes the weapon's double action
/// penalty (`DoubleActionAccuracyPenalty`): the install's Rhino 0.7 and
/// no stock; its MTs-255-12 2.5, which its wooden stock cuts to 0.3 of
/// (`DoubleActionAccuracyPenaltyMult`, the one part of the install below 1).
#[test]
fn a_double_action_penalty_is_the_weapon_s_and_its_stock_s() {
    let Some(db) = crate::fixture::database() else { return };
    let figures = |template: &str| {
        let preset = db.default_preset(template).expect("its preset");
        let parts: Vec<&str> = preset.items.iter().map(|part| part.template.as_str()).filter(|part| *part != template).collect();
        trp_sim::combat::ballistics::WeaponFigures::of(db, template, &parts).expect("its figures")
    };
    let rhino = figures(RHINO);
    println!("  Rhino: {} times {}, centre of impact {}", rhino.double_action_penalty, rhino.stock_double_action, rhino.centre_of_impact);
    assert_eq!((rhino.double_action_penalty, rhino.stock_double_action), (0.7, 1.0));
    assert!((rhino.double_action(true, 0.0) - 0.7).abs() < 1e-6 && rhino.double_action(false, 0.0) == 0.0);
    let mts = figures(MTS255);
    println!("  MTs-255-12: {} times {}, centre of impact {}", mts.double_action_penalty, mts.stock_double_action, mts.centre_of_impact);
    assert_eq!((mts.double_action_penalty, mts.stock_double_action), (2.5, 0.3));
    // Revolver 50 takes half off before the stock's share.
    assert!((mts.double_action(true, 0.5) - 2.5 * 0.5 * 0.3).abs() < 1e-6);
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
