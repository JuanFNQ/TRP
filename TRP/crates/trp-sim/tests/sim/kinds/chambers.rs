//! Rounds and cases shown in chambers of their own, and the rocket launcher.

use super::*;

pub(super) const MP18: &str = "61f7c9e189e6fb1a5e3ea78d";

const FN40GL: &str = "5e81ebcd8e146c7080625e15";

pub(super) const RSHG2: &str = "676bf44c5539167c3603e869";

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

/// What the model holds at each chamber of its own, one letter a chamber:
/// `R` a round at its point, `c` a case at its port, `.` nothing.
pub(super) fn shown(w: &Firearm) -> String {
    w.chambers_shown()
        .iter()
        .map(|s| match (s.round, s.case) {
            (true, true) => 'B',
            (true, false) => 'R',
            (false, true) => 'c',
            (false, false) => '.',
        })
        .collect()
}

/// The cases thrown and the reload's rounds shown, by chamber.
pub(super) fn thrown_and_loaded(said: &[(FirearmEvent, f32)]) -> (Vec<(usize, Option<f32>)>, Vec<(usize, usize)>) {
    let thrown = said.iter().filter_map(|(event, _)| if let FirearmEvent::CaseThrown { chamber, share } = event { Some((*chamber, *share)) } else { None }).collect();
    let loaded = said.iter().filter_map(|(event, _)| if let FirearmEvent::RoundShown { chamber, nth } = event { Some((*chamber, *nth)) } else { None }).collect();
    (thrown, loaded)
}

/// The rounds and cases a weapon's model holds in chambers of its own, at
/// the game's moments (more-weapons spec, "The rounds and cases seen in
/// chambers of their own"): a round goes to its port as a case at the
/// shot; the Rhino's reload throws its cases one by one, or all at once
/// straight down with no live round left, and shows each round as it goes
/// in; the MP-43's throws both at its eject and shows the new rounds at
/// `MagShow`; the MP-18's throws its case with the player's whole velocity.
/// Barrels are refused the magazine check (they have no magazine).
#[test]
fn the_rounds_and_cases_seen_in_chambers_of_their_own() {
    if let Some(mut w) = weapon_of(RHINO) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        assert_eq!(shown(&w), "RRRRRR");
        for _ in 0..2 {
            press(&mut w, 2.0);
        }
        assert_eq!(shown(&w), "ccRRRR");
        w.reload_shells(40);
        let (_, said) = run(&mut w, 12.0);
        assert_eq!(thrown_and_loaded(&said), (vec![(0, Some(0.33)), (1, Some(0.33))], vec![(0, 0), (1, 1)]));
        assert_eq!(shown(&w), "RRRRRR");
        for _ in 0..6 {
            press(&mut w, 2.0);
        }
        assert_eq!(shown(&w), "cccccc");
        w.reload_shells(40);
        let (_, said) = run(&mut w, 20.0);
        let (thrown, loaded) = thrown_and_loaded(&said);
        println!("  the Rhino emptied: thrown {thrown:?}, loaded {loaded:?}");
        assert_eq!(thrown, (0..6).map(|chamber| (chamber, None)).collect::<Vec<_>>());
        assert_eq!(loaded.iter().map(|(_, nth)| *nth).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
        assert_eq!(shown(&w), "RRRRRR");
        assert!(w.check_magazine(), "a revolver's cylinder is its magazine");
        // The hands made from an item that holds rounds beside cases (put
        // in while it was not in hand): the draw takes the case of each
        // loaded chamber numbered below the count of rounds, and of no other.
        assert!(w.hold_rounds(&[true, true, false, true, false, false], &[true, true, true, true, false, false], true));
        assert_eq!((shown(&w), w.magazine), ("RRcB..".to_owned(), Some(3)));
        assert_eq!(w.in_chambers.iter().map(|c| c.case).collect::<Vec<_>>(), [false, false, true, true, false, false]);
    }
    if let Some(mut w) = weapon_of(MP43) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        assert_eq!(shown(&w), "..");
        assert!(!w.check_magazine(), "barrels have no magazine to look at");
        w.reload_shells(40);
        let (_, said) = run(&mut w, 12.0);
        assert_eq!(thrown_and_loaded(&said), (vec![], vec![(0, 0), (1, 1)]));
        assert_eq!(shown(&w), "RR");
        press(&mut w, 2.0);
        assert_eq!(shown(&w), "cR");
        press(&mut w, 2.0);
        assert_eq!(shown(&w), "cc");
        w.reload_shells(40);
        let (_, said) = run(&mut w, 12.0);
        assert_eq!(thrown_and_loaded(&said), (vec![(0, Some(0.33)), (1, Some(0.33))], vec![(0, 0), (1, 1)]));
        assert_eq!(shown(&w), "RR");
        // The case is the weapon's at the shot's mark, not at the press.
        let (_, said) = press(&mut w, 2.0);
        assert!(said.iter().any(|(event, _)| *event == FirearmEvent::CaseLeft { chamber: 0 }));
        assert!(w.in_chambers[0].case && !w.in_chambers[0].round);
        // The item's rounds moved by the inventory, the weapon in hand: a
        // barrel left without its round keeps its case, one given a round
        // loses its case, unthrown.
        assert!(w.hold_rounds(&[false, false], &[], false));
        assert_eq!((shown(&w), w.magazine), ("c.".to_owned(), Some(0)));
        assert!(w.hold_rounds(&[true, true], &[], false) && !w.hold_rounds(&[true], &[], false));
        assert_eq!((shown(&w), w.magazine), ("RR".to_owned(), Some(2)));
        assert!(w.in_chambers.iter().all(|c| !c.case));
        // The hands made from an item with a round and a case in the first
        // barrel and a case alone in the second: barrels in number lose
        // the case of each loaded one as they are drawn.
        assert!(w.hold_rounds(&[true, false], &[true, true], true));
        assert_eq!((shown(&w), w.in_chambers[0].case, w.in_chambers[1].case), ("Rc".to_owned(), false, true));
    }
    if let Some(mut w) = weapon_of(MP18) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        assert!(!w.check_magazine());
        w.reload_shells(20);
        run(&mut w, 10.0);
        assert_eq!(shown(&w), "R");
        press(&mut w, 2.0);
        assert_eq!(shown(&w), "c");
        w.reload_shells(20);
        let (_, said) = run(&mut w, 10.0);
        assert_eq!(thrown_and_loaded(&said), (vec![(0, Some(1.0))], vec![(0, 0)]));
        assert_eq!(shown(&w), "R");
        // Over a live round: it leaves its point unthrown, the new one shows.
        w.reload_shells(20);
        let (_, said) = run(&mut w, 10.0);
        assert_eq!(thrown_and_loaded(&said), (vec![], vec![(0, 0)]));
        assert_eq!(shown(&w), "R");
        // A single barrel keeps its case beside a round as it is drawn (the
        // draw's clearing is for two barrels or more: `IsMultiBarrel`); the
        // shot leaves its own case in that one's place.
        assert!(w.hold_rounds(&[true], &[true], true));
        assert_eq!(shown(&w), "B");
        let (_, said) = press(&mut w, 2.0);
        assert!(said.iter().any(|(event, _)| *event == FirearmEvent::CaseLeft { chamber: 0 }));
        assert_eq!((shown(&w), w.in_chambers[0]), ("c".to_owned(), trp_sim::weapons::firearm::Chamber { round: false, case: true }));
        // A reload over a live round beside a case: the weapon's case goes
        // as it begins, the model's at `RemoveShell`, neither thrown.
        assert!(w.hold_rounds(&[true], &[true], true));
        w.reload_shells(20);
        let (_, said) = run(&mut w, 10.0);
        assert!(said.iter().any(|(event, _)| *event == FirearmEvent::RoundUnloaded));
        assert_eq!((thrown_and_loaded(&said), shown(&w)), ((vec![], vec![(0, 0)]), "R".to_owned()));
    }
}

/// The rocket launcher (the RShG-2): found with its rocket, shown at its
/// point; fired from the sights only, by its own firing; empty after, and
/// idle, its rocket's model at the port and no case on the weapon (more
/// weapons spec, "The rocket in its tube").
#[test]
fn the_rocket_launcher_fires_from_the_sights() {
    let Some(mut w) = weapon_of(RSHG2) else { return };
    assert!(w.settings.rocket_launcher);
    w.found();
    w.draw();
    let (entered, _) = run(&mut w, 5.0);
    println!("  drawn as found: {}; {:?}", told(&entered), w.magazine);
    assert!(w.is_idle() && w.magazine == Some(1));
    assert_eq!(shown(&w), "R");
    // The sights up: its own clip's mark has the animator ready to fire.
    w.set_aim(true, true);
    let (entered, _) = run(&mut w, 1.0);
    println!("  the sights up: {}; IsAimingFloat {}", told(&entered), w.animator().get("IsAimingFloat"));
    assert!(w.animator().get("IsAimingFloat") > 0.9);
    let (entered, said) = press(&mut w, 3.0);
    println!("  fired: {}", told(&entered));
    assert_eq!((shots(&said), w.magazine), (1, Some(0)));
    assert!(names(&entered).contains(&"FIRE") && !w.aiming() && w.is_idle());
    // The rocket's model at the port; the weapon holds no case for it.
    assert_eq!(shown(&w), "c");
    assert!(events(&said).iter().any(|event| matches!(event, FirearmEvent::CaseLeft { chamber: 0 })));
    assert!(w.in_chambers.iter().all(|chamber| !chamber.case && !chamber.round));
    // Its hands made again from the item (no rocket, no case): nothing.
    assert!(w.hold_rounds(&[false], &[false], true));
    assert_eq!(shown(&w), ".");
}

/// The events said, without the sounds' and the body's.
pub(super) fn events(said: &[(FirearmEvent, f32)]) -> Vec<&FirearmEvent> {
    said.iter().map(|(event, _)| event).collect()
}

pub(super) fn dropped(said: &[(FirearmEvent, f32)]) -> Vec<usize> {
    said.iter().filter_map(|(event, _)| if let FirearmEvent::RoundDropped { chamber } = event { Some(*chamber) } else { None }).collect()
}
