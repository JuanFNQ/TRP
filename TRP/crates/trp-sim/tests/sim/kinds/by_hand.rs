//! Rounds moved by hand from the inventory, the unload keys and chambers filled outright.

use super::*;

/// Steps a weapon until its logic idles again after an operation from the
/// inventory (at most `seconds`), then `after` seconds more; the states
/// entered and what was said, as `run`, and when the logic idled.
fn until_idle(w: &mut Firearm, seconds: f32, after: f32) -> (Vec<(String, f32)>, Vec<(FirearmEvent, f32)>, Option<f32>) {
    let name = |w: &Firearm| {
        let (state, _, next) = w.animator().state();
        next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
    };
    let (mut entered, mut said, mut last, mut idled) = (Vec::new(), Vec::new(), name(w), None);
    let mut at = 0.0;
    while at < seconds && idled.is_none_or(|idled| at < idled + after) {
        at += FRAME;
        said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_) | FirearmEvent::BodyAction(_) | FirearmEvent::LeftStance(_))).map(|e| (e, at)));
        let now = name(w);
        if now != last {
            entered.push((now.clone(), at));
            last = now;
        }
        if idled.is_none() && w.can_move_magazine() {
            idled = Some(at);
        }
    }
    (entered, said, idled)
}

fn said_at(said: &[(FirearmEvent, f32)], event: &FirearmEvent) -> Option<f32> {
    said.iter().find(|(said, _)| said == event).map(|(_, at)| *at)
}

fn entered_at(entered: &[(String, f32)], state: &str) -> Option<f32> {
    entered.iter().find(|(name, _)| name == state).map(|(_, at)| *at)
}

/// A round moved into a chamber of its own or out of one from the
/// inventory, the weapon in hand (more-weapons spec, "A round moved into a
/// chamber of its own, or out of one, from the inventory"): the hands' own
/// operations on each weapon's animator from the install, from its
/// inventory pose. The Rhino takes a round out (`DISCHARGE`, the round
/// gone at `MagShow`, 0.885 of 1.867 s), loads one into a chamber with a
/// case (`REMOVE SHELLS` first, the case gone unthrown) and one without
/// (`RELOAD IN` at once), and moves one from a chamber to another (the
/// discharge over at `MagShow`, the load straight after); the M32A1's drum
/// is turned back to its first chamber; the MP-43 takes a barrel's round
/// out (`RELOAD OUT`, at `DelAmmoChamber`) and loads one over a case
/// (`RELOAD THROW` first, every case gone unthrown), and gives a load up
/// when put away; the MP-18 does both through its own states. The rocket
/// launcher is refused both. Only the install's animators run by the
/// simulation: no recording has one.
#[test]
fn a_round_moved_into_a_chamber_of_its_own_or_out_from_the_inventory() {
    if let Some(mut w) = weapon_of(RHINO) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        for _ in 0..2 {
            press(&mut w, 2.0);
        }
        assert_eq!((shown(&w), w.magazine, w.camora), ("ccRRRR".to_owned(), Some(4), 2));
        w.set_inventory(true);
        run(&mut w, 2.0);
        assert!(w.in_inventory() && w.can_load_own_chamber() && w.can_unload_own_chamber());
        assert!(!w.unload_own_chamber(0, false), "chamber 0 holds a case, no round");
        assert!(!w.load_own_chamber(2), "chamber 2 holds a round");
        // A round out of chamber 3.
        assert!(w.unload_own_chamber(3, false));
        assert!(!w.can_load_own_chamber() && !w.can_unload_own_chamber(), "the hands are busy");
        let (entered, said, idled) = until_idle(&mut w, 10.0, 2.0);
        println!("  the Rhino's chamber 3 out: {}", told(&entered));
        assert_eq!(names(&entered), ["CLOSE INVENTORY", "RELOAD START", "DISCHARGE", "RELOAD END", "RELOAD INVENTORY END", "IN INVENTORY"]);
        let (into, out) = (entered_at(&entered, "DISCHARGE").unwrap(), said_at(&said, &FirearmEvent::Discharged).unwrap());
        println!("  DISCHARGE from {into:.3} s, the round out at {out:.3} s, idle at {:.3} s", idled.unwrap());
        assert!((out - into - 0.885 * 1.867).abs() < 0.15, "{into} {out}");
        assert!(idled.unwrap() > out, "over at the cylinder's shutting");
        assert_eq!((shown(&w), w.magazine, w.camora), ("ccR.RR".to_owned(), Some(3), 2));
        // A round into chamber 0, which holds a case: the case out first.
        assert!(w.load_own_chamber(0));
        let (entered, said, _) = until_idle(&mut w, 12.0, 2.0);
        println!("  the Rhino's chamber 0 loaded over its case: {}", told(&entered));
        assert_eq!(names(&entered), ["CLOSE INVENTORY", "RELOAD START", "REMOVE SHELLS", "RELOAD IN", "RELOAD END", "RELOAD INVENTORY END", "IN INVENTORY"]);
        assert!(said.iter().any(|(event, _)| *event == FirearmEvent::Chambered));
        assert!(!said.iter().any(|(event, _)| matches!(event, FirearmEvent::CaseThrown { .. })), "the case is taken away, unthrown");
        assert_eq!((shown(&w), w.magazine), ("RcR.RR".to_owned(), Some(4)));
        assert_eq!(w.in_chambers[0], trp_sim::weapons::firearm::Chamber { round: true, case: false });
        // Into chamber 3, which holds nothing.
        assert!(w.load_own_chamber(3));
        let (entered, _, _) = until_idle(&mut w, 12.0, 2.0);
        println!("  the Rhino's chamber 3 loaded: {}", told(&entered));
        assert_eq!(names(&entered), ["CLOSE INVENTORY", "RELOAD START", "RELOAD IN", "RELOAD END", "RELOAD INVENTORY END", "IN INVENTORY"]);
        assert_eq!((shown(&w), w.magazine), ("RcRRRR".to_owned(), Some(5)));
        // From chamber 4 into chamber 1 (a case there): out, and loaded
        // straight after, the cylinder shut once.
        assert!(w.unload_own_chamber(4, true));
        let (mut entered, _, idled) = until_idle(&mut w, 10.0, 0.0);
        assert!(idled.is_some() && w.animator().flag("LoadOne") && shown(&w) == "RcRR.R");
        assert!(w.load_own_chamber(1));
        let (more, _, _) = until_idle(&mut w, 12.0, 2.0);
        entered.extend(more);
        println!("  the Rhino's chamber 4 into 1: {}", told(&entered));
        assert_eq!(names(&entered), ["CLOSE INVENTORY", "RELOAD START", "DISCHARGE", "REMOVE SHELLS", "RELOAD IN", "RELOAD END", "RELOAD INVENTORY END", "IN INVENTORY"]);
        assert_eq!((shown(&w), w.magazine, w.camora), ("RRRR.R".to_owned(), Some(5), 2));
        // The screen shut, the weapon fires again from the chamber in turn.
        w.set_inventory(false);
        run(&mut w, 2.0);
        let (_, said) = press(&mut w, 2.0);
        assert_eq!((shots(&said), w.magazine), (1, Some(4)));
        // A put-away in the middle of a cylinder's load, before the round's
        // `AddAmmoInMag`: the round is in at once (`LoadCartridgeToCamora.
        // HideWeapon`: `SwitchToIdle`, then the put-away), and `CanReload`,
        // which only that mark takes away, keeps the looping `RELOAD IN`
        // going: the weapon never goes. The game's own code, read, not run
        // (its put-away answers no mark of the load's); the inventory cannot
        // ask for it (a busy weapon does not leave its slot), the slot keys
        // can with the screen shut.
        w.set_inventory(true);
        run(&mut w, 2.0);
        assert!(w.load_own_chamber(4));
        run(&mut w, 2.5);
        assert_eq!(w.animator().state().0, "Hands.RELOAD IN");
        w.hide(false);
        let (after, said) = run(&mut w, 10.0);
        assert!(said.iter().any(|(event, _)| *event == FirearmEvent::Chambered) && w.in_chambers[4].round);
        assert!(after.is_empty() && !said.iter().any(|(event, _)| *event == FirearmEvent::Hidden), "{}", told(&after));
    }
    if let Some(mut w) = weapon_of(MSGL) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        for _ in 0..2 {
            press(&mut w, 2.5);
        }
        assert_eq!(w.camora, 2);
        w.set_inventory(true);
        run(&mut w, 2.0);
        assert!(w.unload_own_chamber(4, false));
        let (entered, said, idled) = until_idle(&mut w, 20.0, 1.0);
        println!("  the M32A1's chamber 4 out: {}", told(&entered));
        assert!(idled.is_some() && said.iter().any(|(event, _)| *event == FirearmEvent::Discharged));
        assert_eq!((shown(&w), w.magazine, w.camora), ("ccRR.R".to_owned(), Some(3), 0), "the drum turned back to its first chamber");
    }
    if let Some(mut w) = weapon_of(MP43) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        w.reload_shells(40);
        run(&mut w, 12.0);
        press(&mut w, 2.0);
        assert_eq!((shown(&w), w.magazine), ("cR".to_owned(), Some(1)));
        w.set_inventory(true);
        run(&mut w, 2.0);
        assert!(w.can_load_chamber() && w.can_discharge(), "the inventory's rules: barrels taken and given up");
        assert!(w.unload_own_chamber(1, false));
        let (entered, said, idled) = until_idle(&mut w, 10.0, 2.0);
        println!("  the MP-43's barrel 1 out: {}", told(&entered));
        assert_eq!(names(&entered), ["INV TO RELOAD", "OPEN CHAMBER HANDS OFF", "RELOAD OUT", "RELOAD CLOSE CHAMBER", "RELOAD INVENTORY END", "IN INVENTORY"]);
        let (into, out) = (entered_at(&entered, "RELOAD OUT").unwrap(), said_at(&said, &FirearmEvent::Discharged).unwrap());
        println!("  RELOAD OUT from {into:.3} s, the round out at {out:.3} s, idle at {:.3} s", idled.unwrap());
        assert!(idled.unwrap() > out);
        assert_eq!((shown(&w), w.magazine), ("c.".to_owned(), Some(0)));
        // A round into barrel 1, the case in barrel 0: thrown out of sight
        // first (`RELOAD THROW`), every case gone, none thrown.
        assert!(w.load_own_chamber(1));
        let (entered, said, _) = until_idle(&mut w, 12.0, 2.0);
        println!("  the MP-43's barrel 1 loaded: {}", told(&entered));
        assert_eq!(names(&entered), ["INV TO RELOAD", "OPEN CHAMBER HANDS OFF", "RELOAD THROW", "RELOAD IN", "RELOAD CLOSE CHAMBER", "RELOAD INVENTORY END", "IN INVENTORY"]);
        assert!(!said.iter().any(|(event, _)| matches!(event, FirearmEvent::CaseThrown { .. })));
        assert_eq!((shown(&w), w.magazine), (".R".to_owned(), Some(1)));
        assert!(w.in_chambers.iter().all(|chamber| !chamber.case));
        // A load given up for a put-away: the round is not in.
        assert!(w.load_own_chamber(0));
        run(&mut w, 0.5);
        w.hide(false);
        let (_, said) = run(&mut w, 4.0);
        assert!(said.iter().any(|(event, _)| *event == FirearmEvent::ChamberGivenUp));
        assert!(!w.in_chambers[0].round && w.magazine == Some(1));
    }
    if let Some(mut w) = weapon_of(MP18) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        w.reload_shells(20);
        run(&mut w, 10.0);
        press(&mut w, 2.0);
        assert_eq!(shown(&w), "c");
        w.set_inventory(true);
        run(&mut w, 2.0);
        assert!(w.load_own_chamber(0));
        let (entered, said, _) = until_idle(&mut w, 12.0, 2.0);
        println!("  the MP-18 loaded over its case: {}", told(&entered));
        assert_eq!(names(&entered), ["RELOAD START INVENTORY", "RELOAD THROW", "RELOAD IN", "RELOAD INVENTORY END", "IN INVENTORY"]);
        assert!(!said.iter().any(|(event, _)| matches!(event, FirearmEvent::CaseThrown { .. })));
        assert_eq!((shown(&w), w.magazine), ("R".to_owned(), Some(1)));
        assert!(w.unload_own_chamber(0, false));
        let (entered, said, idled) = until_idle(&mut w, 10.0, 2.0);
        println!("  the MP-18 unloaded: {}", told(&entered));
        assert_eq!(names(&entered), ["DISCHARGE START INV", "DISCHARGE", "DISCHARGE END INV", "IN INVENTORY"]);
        let (into, out) = (entered_at(&entered, "DISCHARGE").unwrap(), said_at(&said, &FirearmEvent::Discharged).unwrap());
        assert!((out - into - 0.855 * 1.267 / 1.05).abs() < 0.15 && idled.unwrap() >= out, "{into} {out}");
        assert_eq!((shown(&w), w.magazine), (".".to_owned(), Some(0)));
    }
    if let Some(mut w) = weapon_of(RSHG2) {
        w.found();
        w.draw();
        run(&mut w, 5.0);
        w.set_inventory(true);
        run(&mut w, 2.0);
        assert!(!w.can_load_chamber() && !w.can_discharge() && !w.unload_own_chamber(0, false), "not built for the rocket launcher");
    }
}

const AKM: &str = "59d6088586f774275f37482f";

/// The magazine's and the chamber's unload keys (firing spec, "The
/// magazine's and the chamber's keys"; `FirearmHandsInputTranslator.
/// UnloadMag`, `UnloadChamber`). The AKM with a magazine in works its
/// action at the chamber's key (`RechamberOperation`: `RECHAMBER`,
/// `akm_misfire_0`, 1.667 s, entered from rest at once): the round thrown
/// out at `ShellEject` (0.495 of it) and the magazine's top fed at
/// `AddAmmoInChamber` (0.600), where the rack is over; from an empty
/// magazine it is over at the throw, the chamber left empty. With no
/// magazine the round is the inventory's to move; an empty chamber does
/// nothing. The Rhino's cylinder is turned on by a chamber
/// (`RollCylinderOperation`: `RELOAD START`, then `ROLL CYLINDER`, at whose
/// `MagHide`, 0.732 of 0.967 s, it turns). The RShG-2's rounds are not the
/// player's to take out. The magazine's key leaves the moving of the
/// magazine to the inventory only where one is in.
#[test]
fn the_unload_keys_rack_turn_or_hand_over() {
    use trp_sim::ChamberKey;
    let rack = |chamber: bool, rounds: Option<u32>| {
        let mut w = weapon_of(AKM)?;
        (w.chamber, w.magazine) = (chamber, rounds);
        w.start_drawn();
        run(&mut w, 2.0);
        let key = w.unload_chamber_key();
        let (entered, said, idled) = until_idle(&mut w, 4.0, 0.5);
        Some((w, key, entered, said, idled))
    };
    let Some((w, key, entered, said, idled)) = rack(true, Some(5)) else { return };
    println!("  the AKM racked with five in the magazine: {}; said {:?}", told(&entered), said.iter().map(|s| &s.0).collect::<Vec<_>>());
    assert_eq!(key, ChamberKey::Done);
    assert!(said_at(&said, &FirearmEvent::RackBegun { feeds: true }).is_some());
    let (into, thrown, idled) = (entered_at(&entered, "RECHAMBER").expect("RECHAMBER"), said_at(&said, &FirearmEvent::RoundRacked).expect("thrown"), idled.expect("at rest"));
    assert!((thrown - into - 0.495 * 1.667).abs() < 0.02, "entered {into}, thrown {thrown}");
    assert!((idled - into - 0.600 * 1.667).abs() < 0.02, "entered {into}, fed {idled}");
    assert_eq!((w.chamber, w.magazine), (true, Some(4)));
    // From an empty magazine: over at the throw, nothing fed.
    let (w, key, _, said, idled) = rack(true, Some(0)).expect("the AKM");
    assert_eq!(key, ChamberKey::Done);
    assert!(said_at(&said, &FirearmEvent::RackBegun { feeds: false }).is_some());
    assert_eq!(said_at(&said, &FirearmEvent::RoundRacked), idled);
    assert_eq!((w.chamber, w.magazine), (false, Some(0)));
    // No magazine: the inventory's move; an empty chamber: nothing.
    let (w, key, _, said, _) = rack(true, None).expect("the AKM");
    assert_eq!((key, w.chamber), (ChamberKey::ToRig, true));
    assert!(!said.iter().any(|(event, _)| matches!(event, FirearmEvent::RackBegun { .. } | FirearmEvent::RoundRacked)));
    let (w, key, _, said, _) = rack(false, Some(5)).expect("the AKM");
    assert_eq!((key, w.chamber, w.magazine), (ChamberKey::Done, false, Some(5)));
    assert!(!said.iter().any(|(event, _)| matches!(event, FirearmEvent::RackBegun { .. })));
    // The rack's other states (`probe paramuse Rechamber`): the PM's and the
    // PP-9's `RECHAMBER CATCH` from an empty magazine, the M3's by its fire
    // mode, its tube the magazine fed from. Each throws its round and is over.
    for (template, name) in [("5448bd6b4bdc2dfc2f8b4569", "PM"), ("57f4c844245977379d5c14d1", "PP-9"), ("6259b864ebedf17603599e88", "M3")] {
        for rounds in [3, 0] {
            let mut w = weapon_of(template).expect("the weapon");
            (w.chamber, w.magazine) = (true, Some(rounds));
            w.start_drawn();
            run(&mut w, 2.0);
            assert_eq!(w.unload_chamber_key(), ChamberKey::Done, "{name}");
            let (entered, said, idled) = until_idle(&mut w, 4.0, 0.5);
            println!("  the {name} racked with {rounds}: {}", told(&entered));
            assert!(idled.is_some() && said_at(&said, &FirearmEvent::RoundRacked).is_some(), "{name} with {rounds}: {}", told(&entered));
            assert_eq!((w.chamber, w.magazine), (rounds > 0, Some(rounds.saturating_sub(1))), "{name}");
        }
    }
    // The magazine's key: the inventory's move only with one in.
    let mut w = weapon_of(AKM).expect("the AKM");
    (w.chamber, w.magazine) = (true, Some(5));
    w.start_drawn();
    run(&mut w, 2.0);
    assert!(w.unload_magazine_key());
    w.magazine = None;
    assert!(!w.unload_magazine_key());
    // The Rhino's cylinder turned.
    let mut w = weapon_of(RHINO).expect("the Rhino");
    w.found();
    w.draw();
    run(&mut w, 3.0);
    let before = w.camora;
    assert_eq!(w.unload_chamber_key(), ChamberKey::Done);
    assert!(!w.can_move_magazine(), "the turn is under way");
    let (entered, _, idled) = until_idle(&mut w, 4.0, 2.0);
    println!("  the Rhino's cylinder turned: {}", told(&entered));
    assert_eq!(names(&entered), ["RELOAD START", "ROLL CYLINDER", "ROLL END", "IDLE"]);
    let (rolled, idled) = (entered_at(&entered, "ROLL CYLINDER").expect("ROLL CYLINDER"), idled.expect("at rest"));
    assert!((idled - rolled - 0.732 * 0.967).abs() < 0.03, "rolled {rolled}, turned {idled}");
    assert_eq!(w.camora, (before + 1) % 6);
    // The RShG-2: nothing at all.
    let mut w = weapon_of(RSHG2).expect("the RShG-2");
    assert!(!w.settings.can_unload);
    w.found();
    w.draw();
    run(&mut w, 5.0);
    assert_eq!(w.unload_chamber_key(), ChamberKey::Done);
    assert!(!w.step(FRAME).iter().any(|event| matches!(event, FirearmEvent::LeaveMount { .. })));
}

/// The debug panel's FILL over an empty chamber (`Firearm::chamber_outright`,
/// a test tool's, not the game's). The AKM fired
/// dry and the M4A1 with its bolt caught open by the last round: with the
/// chamber empty a press only clicks, as in the game; a round put in
/// outright at rest lets the M4A1's bolt go home, and the next press fires.
/// A round in already, or a revolver's cylinder, takes none.
#[test]
fn fill_chambers_a_round_outright_at_rest() {
    for (template, name, catches) in [(AKM, "AKM", false), ("5447a9cd4bdc2dbd208b4567", "M4A1", true)] {
        let Some(mut w) = weapon_of(template) else { return };
        (w.chamber, w.magazine) = (true, Some(0));
        w.start_drawn();
        run(&mut w, 2.0);
        w.set_fire_mode(FireMode::Single);
        run(&mut w, 1.0);
        let press = |w: &mut Firearm| {
            w.set_trigger(true, true);
            let (_, said) = run(w, 0.2);
            w.set_trigger(false, true);
            let (_, after) = run(w, 1.5);
            said.into_iter().chain(after).filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count()
        };
        assert_eq!(press(&mut w), 1, "{name}: the last round");
        assert_eq!((w.chamber, w.magazine, w.bolt_caught()), (false, Some(0), catches), "{name}");
        w.magazine = Some(30);
        assert_eq!(press(&mut w), 0, "{name}: an empty chamber clicks");
        assert!(w.chamber_outright(), "{name}");
        assert!(!w.chamber_outright(), "{name}: a round in already");
        assert!(w.chamber && !w.bolt_caught(), "{name}");
        run(&mut w, 0.5);
        assert_eq!(press(&mut w), 1, "{name}: the round put in");
        assert_eq!((w.chamber, w.magazine), (true, Some(29)), "{name}");
        println!("  the {name}: fired dry, a round put in outright, fired again; 29 left and one chambered");
    }
    let mut w = weapon_of(RHINO).expect("the Rhino");
    w.start_drawn();
    run(&mut w, 2.0);
    assert!(!w.chamber_outright(), "a cylinder's chambers are its own");
}

/// The reload key's last resort (firing spec, "A round into the chamber by
/// the reload key"; `LoadAmmoToChamber`) on the install's AKM, at rest with
/// its chamber empty: with no magazine in, the round is to be put in, and
/// the hands' load of one chambers it (`LOAD ONE`, `akm_ammo_in`, 3.767 s,
/// entered from `IDLE` at 0.08 of it; its `AddAmmoInChamber` at 0.885 comes
/// after its way back to `IDLE` begins at 0.79, so is given up then, by the
/// game's rule for a state being left: 2.675 s in); with an empty magazine in, that one
/// goes out first (the hands' `RELOAD OUT`), the logic idles with none in,
/// and the round then goes in the same way. With rounds in the magazine, a
/// round chambered already or none within reach, nothing; a cylinder's
/// chambers are its own.
/// A round loaded by hand is seen in the hand before it is in the chamber:
/// on the chamber's point from the clip's `MagShow` (`LoadCartridgeToChamber.
/// OnMagAppeared`), in the chamber at its `AddAmmoInChamber`.
#[test]
fn a_round_loaded_by_hand_is_seen_before_it_is_chambered() {
    let Some(mut w) = weapon_of(AKM) else { return };
    (w.chamber, w.magazine) = (false, None);
    w.start_drawn();
    run(&mut w, 2.0);
    assert!(!w.round_drawn_in_chamber(), "nothing chambered, nothing drawn");
    assert!(w.load_chamber());
    assert!(!w.round_drawn_in_chamber(), "not before the clip shows it");
    let (mut seen, mut chambered, mut at) = (None, None, 0.0);
    while at < 6.0 {
        at += FRAME;
        w.step(FRAME);
        if seen.is_none() && w.round_drawn_in_chamber() {
            seen = Some(at);
        }
        if chambered.is_none() && w.chamber {
            chambered = Some(at);
        }
    }
    let (seen, chambered) = (seen.expect("the round seen"), chambered.expect("the round chambered"));
    println!("  the round seen {seen:.3} s after the load began, chambered at {chambered:.3}");
    // (The AKM's `LOAD ONE`, 3.767 s: `MagShow` near its start, the chamber's mark late in it.)
    assert!(chambered - seen > 2.0, "in the hand for most of the clip: seen at {seen}, chambered at {chambered}");
    assert!(w.round_drawn_in_chamber(), "and drawn on once chambered");
}

#[test]
fn the_reload_key_s_last_resort_puts_a_round_into_the_chamber() {
    use trp_sim::ChamberByHand;
    let at_rest = |chamber: bool, rounds: Option<u32>| {
        let mut w = weapon_of(AKM)?;
        (w.chamber, w.magazine) = (chamber, rounds);
        w.start_drawn();
        run(&mut w, 2.0);
        Some(w)
    };
    let loads = |w: &mut Firearm, what: &str| {
        assert!(w.can_load_chamber() && w.load_chamber(), "{what}");
        let (entered, said, idled) = until_idle(w, 6.0, 0.2);
        println!("  {what}: {}", told(&entered));
        let into = entered_at(&entered, "LOAD ONE").expect("LOAD ONE");
        let chambered = said_at(&said, &FirearmEvent::Chambered).expect("chambered");
        assert!((chambered - into - (0.79 - 0.08) * 3.767).abs() < 0.02, "{what}: LOAD ONE at {into}, the round in at {chambered}");
        assert!(w.chamber && idled.is_some(), "{what}");
    };
    let Some(mut w) = at_rest(false, None) else { return };
    assert_eq!(w.chamber_by_hand(false), ChamberByHand::Nothing, "no round within reach");
    assert_eq!(w.chamber_by_hand(true), ChamberByHand::Load);
    loads(&mut w, "no magazine in");
    assert_eq!(w.chamber_by_hand(true), ChamberByHand::Nothing, "a round in already");
    // An empty magazine in: out to the rig first, then the round.
    let mut w = at_rest(false, Some(0)).expect("the AKM");
    assert_eq!(w.chamber_by_hand(true), ChamberByHand::MagazineOutFirst);
    assert!(!w.can_load_chamber(), "never past a magazine in");
    assert!(w.pull_out_magazine());
    let (entered, said, idled) = until_idle(&mut w, 4.0, 0.0);
    println!("  the empty magazine out: {}", told(&entered));
    assert!(said_at(&said, &FirearmEvent::MagazineMoved).is_some() && idled.is_some());
    assert_eq!(w.magazine, None);
    loads(&mut w, "after the empty magazine");
    // Rounds in the magazine: nothing; nor for a cylinder.
    assert_eq!(at_rest(false, Some(5)).expect("the AKM").chamber_by_hand(true), ChamberByHand::Nothing);
    let mut rhino = weapon_of(RHINO).expect("the Rhino");
    rhino.start_drawn();
    run(&mut rhino, 2.0);
    assert_eq!(rhino.chamber_by_hand(true), ChamberByHand::Nothing, "a cylinder's chambers are its own");
}
