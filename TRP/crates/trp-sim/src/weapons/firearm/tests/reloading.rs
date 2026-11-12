//! Tests of reloads: mounts, chambered rounds, sprints, the quick reload and the old magazine.

use super::*;

/// The reload's key leaves a mount whatever follows
/// (`FirearmHandsInputTranslator.Reload`): a magazine filled a round at
/// a time and full is not filled, and the mount is left all the same;
/// with nothing to fill it from, the same; a weapon used once hears
/// nothing (as the M700 must not stay on its bipod through the key).
#[test]
fn the_reload_key_leaves_a_mount_whatever_follows() {
    let leaves = |w: &mut Firearm| w.step(DT).contains(&Event::LeaveMount { at_once: false });
    let mut w = Firearm::new(FirearmSettings { internal_magazine: true, ..settings() }, &animator());
    w.start_drawn();
    w.reload_shells(20);
    assert!(leaves(&mut w) && w.is_idle());
    w.magazine = Some(0);
    w.reload_shells(0);
    assert!(leaves(&mut w) && w.is_idle());
    let mut w = Firearm::new(FirearmSettings { one_off: true, ..settings() }, &animator());
    w.start_drawn();
    w.reload_key();
    assert!(!leaves(&mut w));
}

#[test]
fn reloading_with_a_round_chambered_keeps_it() {
    let mut w = rifle();
    w.set_trigger(true, true);
    run(&mut w, 0.5);
    w.set_trigger(false, true);
    w.reload();
    let all = states(&mut w, 6.0, 0.0);
    assert!(w.chamber);
    assert_eq!(w.magazine, Some(30));
    assert_eq!(names(&all), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
    // Each state gives way where the animator's way to the next one
    // opens: start 0.3 of 0.444 s, out 0.78 of 1 s, in 0.9 of 1.2 s,
    // end 0.98 of 0.457 s.
    let rest = all.last().unwrap().1 - all[0].1;
    assert!((rest - 2.44).abs() < 5.0 * DT, "{rest}");
}

#[test]
fn nothing_fires_while_reloading() {
    let mut w = rifle();
    w.reload();
    w.set_trigger(true, true);
    assert_eq!(shots(&run(&mut w, 1.0)), 0);
}

/// A reload asked for in a shot follows it.
#[test]
fn a_reload_asked_for_in_a_shot_follows_it() {
    let mut w = rifle();
    w.set_trigger(true, true);
    w.step(DT);
    w.reload();
    assert!(!w.reloading());
    let wait = run_until(&mut w, |w| w.reloading());
    // The wait between single shots, 60 / 450 s.
    assert!((wait - 0.133).abs() < 3.0 * DT, "{wait}");
    run_until(&mut w, |w| w.is_idle());
    assert_eq!(w.magazine, Some(30));
}

/// A press made where the movement state gives the hands no say (a
/// sprint, a crawl) is dropped, and holding the trigger until the state
/// ends fires nothing. Automatic fire already running carries on.
#[test]
fn a_press_in_a_sprint_or_a_crawl_is_dropped() {
    let mut w = rifle();
    w.set_trigger(true, false);
    let events = run(&mut w, 0.5);
    assert!(shots(&events) == 0 && w.is_idle() && w.magazine == Some(30));
    // The state ends with the trigger still held.
    w.set_trigger(true, true);
    assert_eq!(shots(&run(&mut w, 0.5)), 0);
    // Let go and pressed again.
    w.set_trigger(false, true);
    w.set_trigger(true, true);
    assert_eq!(shots(&run(&mut w, 0.5)), 1);
    w.set_trigger(false, true);

    w.next_fire_mode();
    settle(&mut w);
    w.set_trigger(true, true);
    run(&mut w, 0.3);
    // The crawl begins with the trigger held.
    w.set_trigger(true, false);
    assert!(shots(&run(&mut w, 0.5)) >= 4);
    w.set_trigger(false, false);
    run(&mut w, 0.5);
    assert!(w.is_idle());
    // A fresh press in the crawl does nothing.
    w.set_trigger(true, false);
    assert_eq!(shots(&run(&mut w, 0.5)), 0);
}

#[test]
fn a_reload_is_over_when_the_magazine_is_in_and_plays_on() {
    // Start left at 0.133 s, out at 0.78 s, and the magazine in 0.707
    // of the way through the 1.2 s that follow.
    let mut w = rifle();
    w.reload();
    assert!(w.reloading());
    let over = run_until(&mut w, |w| !w.reloading());
    assert!((over - 1.76).abs() < 4.0 * DT, "{over}");
    assert_eq!(w.magazine, Some(30));
    // The hands are still coming back: the weapon is not at rest and
    // a press of the trigger is still dropped.
    assert!(!w.is_idle() && w.hands_busy());
    w.set_trigger(true, true);
    let rest = run_until(&mut w, |w| w.is_idle());
    assert!(rest > 0.5, "{rest}");
    assert_eq!(w.magazine, Some(30));
    assert!(!w.hands_busy());
    // Back at rest the trigger is heard again.
    w.set_trigger(false, true);
    w.set_trigger(true, true);
    assert_eq!(shots(&run(&mut w, 0.5)), 1);
}

#[test]
fn a_reload_from_an_empty_chamber_is_over_when_the_round_is_chambered() {
    let mut w = rifle();
    (w.chamber, w.magazine) = (false, Some(0));
    w.start_drawn();
    w.reload();
    run_until(&mut w, |w| w.magazine == Some(30));
    assert!(w.reloading() && !w.chamber);
    run_until(&mut w, |w| w.chamber);
    assert!(!w.reloading() && !w.is_idle());
    assert_eq!(w.magazine, Some(29));
}

#[test]
fn an_empty_magazine_going_in_ends_the_reload_with_nothing_chambered() {
    let mut w = rifle();
    (w.chamber, w.magazine) = (false, Some(0));
    w.start_drawn();
    w.reload_with(0);
    let all = states(&mut w, 10.0, 0.0);
    assert_eq!(names(&all), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
    assert!(w.is_idle() && !w.chamber);
}

/// The quick reload throws the magazine out by a state of its own
/// (`RELOAD OUT ALL`) and is the plain reload from there; the host is
/// told of the exchange as it begins and of the old magazine leaving
/// the hand.
#[test]
fn the_quick_reload_throws_the_magazine_out_and_goes_on_as_the_plain_one() {
    let mut w = rifle();
    w.set_trigger(true, true);
    run(&mut w, 0.5);
    w.set_trigger(false, true);
    let mut again = rifle();
    again.magazine = Some(29);
    w.quick_reload_with(30);
    assert!(w.reloading());
    let all = states(&mut w, 6.0, 0.0);
    assert_eq!(names(&all), ["RELOAD OUT ALL", "RELOAD IN", "RELOAD END", "IDLE"]);
    assert!(w.chamber && w.magazine == Some(30));
    // Out all 0.9 of 1 s, in 0.9 of 1.2 s, end 0.98 of 0.457 s.
    let rest = all.last().unwrap().1 - all[0].1;
    assert!((rest - 2.43).abs() < 5.0 * DT, "{rest}");
    again.quick_reload_with(30);
    let swap = again.step(DT);
    assert!(swap.contains(&Event::MagazineSwap { left: Some(29), quick: true }), "{swap:?}");
}

#[test]
fn the_old_magazine_leaves_the_hand_once_in_either_reload() {
    for quick in [false, true] {
        let mut w = rifle();
        if quick { w.quick_reload_with(30) } else { w.reload_with(30) }
        let mut said = w.step(DT);
        while !w.is_idle() {
            said.extend(w.step(DT));
            assert!(said.len() < 10_000);
        }
        let hidden = said.iter().filter(|e| **e == Event::MagazineHidden).count();
        let swaps: Vec<&Event> = said.iter().filter(|e| matches!(e, Event::MagazineSwap { .. })).collect();
        assert_eq!(hidden, 1, "{quick}");
        assert_eq!(swaps, [&Event::MagazineSwap { left: Some(30), quick }]);
        // It leaves after the magazine is out and before the new one is in.
        let at = |wanted: &Event| said.iter().position(|e| e == wanted).unwrap();
        assert!(at(&Event::MagazineOut) < at(&Event::MagazineHidden) && at(&Event::MagazineHidden) < at(&Event::MagazineIn));
    }
}

/// The key is heard only with the animator at rest, and is the plain
/// reload where the weapon does not allow the quick one or has no
/// magazine fitted.
#[test]
fn the_quick_reload_is_refused_away_from_rest_and_plain_where_not_allowed() {
    let swapped = |events: &[Event]| events.iter().any(|e| matches!(e, Event::MagazineSwap { .. }));
    let mut w = rifle();
    w.reload();
    w.step(DT);
    w.quick_reload_with(30);
    assert!(!swapped(&w.step(DT)));
    // Not allowed by the weapon.
    let mut settings = settings();
    settings.fast_reload = false;
    let mut w = Firearm::new(settings, &animator());
    w.start_drawn();
    w.quick_reload_with(30);
    assert!(w.step(DT).contains(&Event::MagazineSwap { left: Some(30), quick: false }));
    assert_eq!(names(&states(&mut w, 1.0, 0.0))[0], "RELOAD OUT");
    // No magazine fitted.
    let mut w = rifle();
    w.magazine = None;
    w.quick_reload_with(30);
    assert!(w.step(DT).contains(&Event::MagazineSwap { left: None, quick: false }));
}
