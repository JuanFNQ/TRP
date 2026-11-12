//! Tests of fire modes, rates, running dry, magazine checks and the aim.

use super::*;

#[test]
fn a_single_shot_fires_once_however_long_the_trigger_is_held() {
    let mut w = rifle();
    w.set_trigger(true, true);
    // The round goes at the press; the shot is made a frame on.
    assert_eq!(w.magazine, Some(29));
    let (first, second) = (w.step(DT), w.step(DT));
    assert_eq!((shots(&first), shots(&second)), (0, 1));
    let mut events = run(&mut w, 1.0);
    assert_eq!(shots(&events), 0);
    events.extend(second);
    assert!(w.chamber && w.is_idle());
    assert!(events.contains(&Event::ShellEject));
}

#[test]
fn single_shots_are_limited_by_their_own_rate() {
    let mut w = rifle();
    // Tapping every other step, far faster than the weapon allows.
    let mut fired = 0;
    for step in 0..120 {
        w.set_trigger(step % 2 == 0, true);
        fired += shots(&w.step(DT));
    }
    // Two seconds at 450 a minute is 15 shots.
    assert!((14..=16).contains(&fired), "{fired}");
}

#[test]
fn automatic_fire_keeps_the_rate_and_stops_on_release() {
    let mut w = rifle();
    w.next_fire_mode();
    settle(&mut w);
    assert_eq!(w.mode(), FireMode::FullAuto);
    w.set_trigger(true, true);
    let events = run(&mut w, 1.0);
    // The first shot at once, then one every tenth of a second.
    assert!((10..=11).contains(&shots(&events)), "{}", shots(&events));
    assert!(w.auto_fire());
    w.set_trigger(false, true);
    let after = run(&mut w, 0.5);
    assert!(shots(&after) <= 1);
    assert!(w.is_idle());
    assert!(after.contains(&Event::AutoFire(false)));
}

#[test]
fn a_tap_in_automatic_still_fires_one_round() {
    let mut w = rifle();
    w.next_fire_mode();
    settle(&mut w);
    w.set_trigger(true, true);
    w.set_trigger(false, true);
    assert_eq!(shots(&run(&mut w, 0.5)), 1);
}

#[test]
fn the_weapon_runs_dry_and_a_reload_fills_it() {
    let mut w = rifle();
    w.next_fire_mode();
    settle(&mut w);
    w.set_trigger(true, true);
    let events = run(&mut w, 5.0);
    // Thirty in the magazine and one in the chamber.
    assert_eq!(shots(&events), 31);
    assert!(!w.chamber && w.magazine == Some(0));
    assert!(events.contains(&Event::DryFire));
    w.set_trigger(false, true);
    settle(&mut w);

    w.reload();
    let events = run(&mut w, 6.0);
    assert!(events.contains(&Event::MagazineOut) && events.contains(&Event::MagazineIn));
    // The empty chamber took the first round of the new magazine.
    assert!(w.chamber);
    assert_eq!(w.magazine, Some(29));
    assert!(w.is_idle());
}

#[test]
fn checking_the_magazine_lowers_the_sights_and_holds_them_down() {
    let mut w = rifle();
    w.set_aim(true, true);
    w.check_magazine();
    assert!(w.checking() && !w.aiming() && w.hands_busy());
    w.set_aim(true, true);
    assert!(!w.aiming(), "the key is dead through the check");
    let took = settle(&mut w);
    assert!((took - 2.0).abs() < 4.0 * DT, "{took}");
    assert!(!w.aiming(), "and they do not come back by themselves");
    assert!(!w.hands_busy() && !w.checking());
    w.set_aim(true, true);
    assert!(w.aiming());
    // Switching the fire mode leaves them be: the weapon's logic
    // idles through it, and the key works.
    w.next_fire_mode();
    w.step(DT);
    assert!(!w.is_idle() && !w.occupied() && !w.checking() && w.aiming());
    w.set_aim(false, true);
    w.set_aim(true, true);
    assert!(w.aiming());
}

/// A press of the trigger while the fire mode is being switched fires,
/// in the mode just switched to (the fifteenth recording, scenario
/// 64). The reload key is taken too, and the reload begins when the
/// switch has played out.
#[test]
fn a_press_in_a_fire_mode_switch_fires() {
    let mut w = rifle();
    w.next_fire_mode();
    w.step(DT);
    assert_eq!(w.animator().state().0, "Hands.FIREMODE");
    run(&mut w, 0.3);
    assert_eq!(w.animator().state().0, "Hands.FIREMODE");
    w.set_trigger(true, true);
    let burst = run(&mut w, 0.2);
    w.set_trigger(false, true);
    assert!(shots(&burst) >= 2, "automatic from the key on: {}", shots(&burst));
    settle(&mut w);

    w.next_fire_mode();
    w.step(DT);
    w.reload();
    assert!(w.reloading());
    let all = states(&mut w, 6.0, 0.0);
    assert_eq!(names(&all), ["IDLE", "RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
}

#[test]
fn the_aim_key_waits_for_the_magazine_not_for_the_animation() {
    let mut w = rifle();
    w.set_aim(true, true);
    w.reload();
    // Below mastering 2 the reload lowers the sights and the key does
    // nothing while it is in hand.
    assert!(!w.aiming());
    w.set_aim(true, true);
    assert!(!w.aiming());
    run_until(&mut w, |w| !w.reloading());
    // With the magazine in the key works, and so does what lowers the
    // sights from outside, though the hands are still coming back.
    assert!(!w.is_idle());
    w.set_aim(true, true);
    assert!(w.aiming());
    w.lower_aim();
    assert!(!w.aiming());
}

/// The aim's call that moves the sights says so, up or down, for the
/// sounds (`SetAim`); a call that leaves them as they are, or what
/// lowers them otherwise, says nothing.
#[test]
fn the_sights_moved_by_the_aim_s_call_are_told() {
    let mut w = rifle();
    let aimed = |w: &mut Firearm| -> Vec<bool> {
        w.step(0.0).into_iter().filter_map(|e| if let Event::Aimed(up) = e { Some(up) } else { None }).collect()
    };
    aimed(&mut w);
    w.set_aim(true, true);
    assert_eq!(aimed(&mut w), [true]);
    w.set_aim(true, true);
    assert_eq!(aimed(&mut w), Vec::<bool>::new());
    w.set_aim(false, true);
    assert_eq!(aimed(&mut w), [false]);
    w.set_aim(true, true);
    w.lower_aim();
    assert_eq!(aimed(&mut w), [true]);
    assert!(!w.aiming());
}

/// A weapon taken in hand has the fire mode its item keeps
/// (`FireModeComponent.FireMode`): set with nothing shown and no switch
/// played, the first where the item keeps none, and left alone where the
/// weapon has no such mode.
#[test]
fn the_fire_mode_is_as_the_item_keeps_it() {
    let mut w = rifle();
    let first = w.mode();
    w.next_fire_mode();
    settle(&mut w);
    let second = w.mode();
    assert_ne!(first, second);
    assert!(w.start_fire_mode(None), "another weapon of the build, never held");
    assert_eq!(w.mode(), first);
    assert!(w.start_fire_mode(Some(second)));
    assert_eq!(w.mode(), second);
    let events = run(&mut w, 0.1);
    assert_ne!(w.animator().state().0, "Hands.FIREMODE", "no switch is played");
    assert!(!events.iter().any(|event| matches!(event, Event::FireMode(_) | Event::FireModeShown(_))), "nothing is shown");
    assert!(!w.start_fire_mode(Some(second)), "as it is already");
    assert!(!w.start_fire_mode(Some(FireMode::Doublet)) && w.mode() == second, "a mode the weapon has not");
}
