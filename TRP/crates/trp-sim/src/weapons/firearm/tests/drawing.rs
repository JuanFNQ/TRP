//! Tests of the hands going down for the inventory and drawing and putting away.

use super::*;

/// The inventory opening takes the hands down by the animator's own
/// states and holds them there; shutting brings them back. The sights
/// are down, and the weapon's logic idles throughout.
#[test]
fn the_hands_go_down_for_an_open_inventory_and_come_back() {
    let mut w = rifle();
    w.set_aim(true, true);
    w.set_inventory(true);
    assert!(!w.aiming());
    let down = states(&mut w, 2.0, 0.0);
    assert_eq!(names(&down), ["OPEN INVENTORY", "IN INVENTORY"]);
    // The held state takes over halfway through the way down.
    assert!((down[1].1 - 0.333).abs() < 3.0 * DT, "{}", down[1].1);
    assert!(w.in_inventory() && !w.is_idle() && !w.occupied());
    w.set_inventory(false);
    let up = states(&mut w, 2.0, 0.0);
    assert_eq!(names(&up), ["CLOSE INVENTORY", "IDLE"]);
    assert!((up[1].1 - up[0].1 - 0.64).abs() < 3.0 * DT, "{:?}", up);
    assert!(w.is_idle() && !w.in_inventory());
}

/// Shut again at once, the hands still go down first: the way down has
/// one way out, into the held state, and the way back opens only when
/// the blend into that is over.
#[test]
fn an_inventory_shut_at_once_still_takes_the_hands_down() {
    let mut w = rifle();
    w.set_inventory(true);
    w.step(DT);
    w.set_inventory(false);
    let all = states(&mut w, 3.0, DT);
    assert_eq!(names(&all), ["IN INVENTORY", "CLOSE INVENTORY", "IDLE"]);
    // Halfway down (0.333 s), then the blend into the held state (0.25 s).
    assert!((all[1].1 - 0.583).abs() < 3.0 * DT, "{:?}", all);
    assert!(w.is_idle());
}

/// Opened in a reload, the reload does not bring the hands back to
/// the weapon first: its last state gives way to the animator's own
/// way from a reload into the held state.
#[test]
fn a_reload_ends_in_the_inventory_s_hold_when_it_is_open() {
    let mut w = rifle();
    w.reload();
    run(&mut w, 0.1);
    w.set_inventory(true);
    let all = states(&mut w, 6.0, 0.1);
    assert_eq!(names(&all), ["RELOAD OUT", "RELOAD IN", "RELOAD INVENTORY END", "IN INVENTORY"]);
    assert_eq!(w.magazine, Some(30));
    assert!(w.in_inventory() && !w.reloading());
    // And from an empty chamber, through the chambering.
    let mut w = rifle();
    (w.chamber, w.magazine) = (false, Some(0));
    w.start_drawn();
    w.reload();
    run(&mut w, 0.1);
    w.set_inventory(true);
    let all = states(&mut w, 8.0, 0.1);
    assert_eq!(names(&all), ["RELOAD OUT", "RELOAD IN", "RELOAD CHARGE END", "RELOAD INVENTORY END", "IN INVENTORY"]);
    assert!(w.chamber && w.magazine == Some(29));
}

/// The inventory opened before the magazine is out is only noted: the
/// animator hears of it when the magazine comes out.
#[test]
fn a_reload_tells_the_animator_of_the_inventory_once_the_magazine_is_out() {
    let mut w = rifle();
    w.reload();
    w.step(DT);
    w.set_inventory(true);
    assert!(!w.animator().flag("Inventory"));
    run_until(&mut w, |w| w.magazine.is_none());
    assert!(w.animator().flag("Inventory"));
}

/// A weapon is drawn by its animator's own states, appears at the
/// draw's last mark, and is put away the same way.
#[test]
fn a_weapon_is_drawn_and_put_away_by_its_animator() {
    let mut w = Firearm::new(settings(), &animator());
    assert!(!w.in_hand());
    // Out of the hands its keys do nothing.
    w.set_trigger(true, true);
    w.reload();
    assert_eq!(shots(&run(&mut w, 0.5)), 0);
    w.set_trigger(false, true);

    w.draw();
    assert!(w.in_hand() && w.hands_busy() && w.occupied());
    // The inventory opening in a draw is not heard by the draw.
    w.set_inventory(true);
    let mut events = Vec::new();
    let appeared = run_until(&mut w, |w| !w.occupied());
    events.extend(run(&mut w, 0.2));
    assert!((appeared - 0.95).abs() < 4.0 * DT, "{appeared}");
    assert!(w.is_idle() && !w.hands_busy() && !w.in_inventory());
    // What has appeared is told of the screen afresh, and then the
    // hands go down for it like any other weapon's.
    w.set_inventory(true);
    run(&mut w, 1.0);
    assert!(w.in_inventory());
    w.set_inventory(false);
    settle(&mut w);
    assert!(w.is_idle());

    w.hide(false);
    assert!(w.occupied() && w.hands_busy());
    for _ in 0..120 {
        events.extend(w.step(DT));
    }
    assert!(events.contains(&Event::Hidden), "{events:?}");
    assert!(!w.in_hand());
}

/// A build left at rest in hand for another (the app's builds) is put
/// away unseen, and taken in hand again it is drawn as any weapon is,
/// its hands free once it has appeared (left at rest, its draw would
/// never begin).
#[test]
fn a_build_left_in_hand_is_drawn_again() {
    let mut w = rifle();
    w.draw();
    assert!(w.is_idle() && !w.hands_busy(), "at rest, a draw is not begun");
    w.put_away_unseen();
    assert!(!w.in_hand() && !w.hands_busy());
    w.magazine = Some(30);
    w.draw();
    assert!(w.in_hand() && w.hands_busy() && w.occupied());
    run_until(&mut w, |w| !w.occupied());
    run(&mut w, 0.2);
    assert!(w.is_idle() && !w.hands_busy());
}

/// Drawn with rounds in the magazine and none chambered, a weapon
/// chambers one.
#[test]
fn a_draw_chambers_a_round() {
    let mut w = Firearm::new(settings(), &animator());
    (w.chamber, w.magazine) = (false, Some(12));
    w.draw();
    assert!(w.chamber && w.magazine == Some(11));
}

/// A draw made to rack for the look of it tells the animator an empty
/// chamber and a round to feed, and moves no round; a draw that racks
/// of itself is left as it is.
#[test]
fn a_racked_draw_moves_no_round() {
    let mut w = Firearm::new(settings(), &animator());
    (w.chamber, w.magazine) = (true, Some(12));
    w.draw();
    w.rack_drawn();
    assert_eq!((w.animator.get("AmmoInChamber"), w.animator.get("AmmoInMag")), (0.0, 12.0));
    run_until(&mut w, |w| !w.occupied());
    run(&mut w, 0.2);
    assert!(w.is_idle(), "{:?}", w.operation);
    assert_eq!((w.chamber, w.magazine), (true, Some(12)));
    // An empty magazine is drawn racking all the same, and still holds
    // nothing.
    let mut w = Firearm::new(settings(), &animator());
    (w.chamber, w.magazine) = (true, Some(0));
    w.draw();
    w.rack_drawn();
    assert_eq!(w.animator.get("AmmoInMag"), 1.0);
    run_until(&mut w, |w| !w.occupied());
    assert!(w.chamber && w.magazine == Some(0));
    // Drawn with none chambered, it chambers one of itself, once.
    let mut w = Firearm::new(settings(), &animator());
    (w.chamber, w.magazine) = (false, Some(12));
    w.draw();
    w.rack_drawn();
    assert!(w.chamber && w.magazine == Some(11));
}

/// Put away while the hands are down for the inventory, the weapon
/// goes from there.
#[test]
fn a_weapon_is_put_away_from_the_inventory_s_hold() {
    let mut w = rifle();
    w.set_inventory(true);
    run(&mut w, 1.5);
    w.hide(false);
    let all = states(&mut w, 2.0, 0.0);
    assert_eq!(names(&all), ["IDLE TO OUT", "SPAWN"]);
    assert!(!w.in_hand());
}
