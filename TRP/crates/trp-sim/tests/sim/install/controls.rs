//! Commands, slow leans, looking a weapon over, and the mouse bindings.

use super::*;

/// A host that hears its keys as the game's input manager gives them out
/// hands the simulation commands, not held keys (settings spec, "What the
/// bindings do"). A sprint bound as a press is one command a press: on
/// with the first and off with the second, where a held key's coming up
/// would have ended it. A lean bound so leans on one press and
/// straightens on the next.
#[test]
fn commands_do_what_the_held_keys_do() {
    let Some(mut m) = movement(0.0) else { return };
    let forward = walk([0.0, 1.0]);
    run(&mut m, 0.5, forward);
    m.step(DT, &Input { sprint_toggle: true, ..forward }, &Flat);
    run(&mut m, 2.0, forward);
    assert_eq!(m.state, State::Sprint, "a press starts the sprint and nothing ends it");
    m.step(DT, &Input { sprint_toggle: true, ..forward }, &Flat);
    run(&mut m, 2.0, forward);
    assert_eq!(m.state, State::Move, "the next press ends it");
    // The same by a key held and let go.
    m.step(DT, &Input { sprint_toggle: true, ..forward }, &Flat);
    run(&mut m, 1.0, forward);
    assert_eq!(m.state, State::Sprint);
    m.step(DT, &Input { sprint_end: true, ..forward }, &Flat);
    run(&mut m, 2.0, forward);
    assert_eq!(m.state, State::Move);

    let Some(mut m) = movement(0.0) else { return };
    let idle = Input::default();
    m.step(DT, &Input { lean_toggle: [false, true], ..idle }, &Flat);
    run(&mut m, 1.0, idle);
    assert!(m.tilt > 0.0 && m.smoothed_tilt > 1.0, "leans right and stays: {} {}", m.tilt, m.smoothed_tilt);
    m.step(DT, &Input { lean_toggle: [false, true], ..idle }, &Flat);
    run(&mut m, 1.0, idle);
    assert_eq!(m.tilt, 0.0);
    // A held binding's coming up straightens only a lean to its own side.
    m.step(DT, &Input { lean_toggle: [true, false], ..idle }, &Flat);
    m.step(DT, &Input { lean_end: [false, true], ..idle }, &Flat);
    assert!(m.tilt < 0.0);
    m.step(DT, &Input { lean_end: [true, false], ..idle }, &Flat);
    assert_eq!(m.tilt, 0.0);
}

/// The slow lean, the walk key and a backpack's drop
/// (`docs/spec/movement.md`, "The slow lean" and "The walk key";
/// `docs/spec/interactions.md`, "A backpack dropped"). From the game's
/// code and the install; no recording has any of the three.
#[test]
fn a_lean_is_made_slowly_and_the_walk_key_puts_the_speed_aside() {
    use trp_sim::Acted;
    const FRAME: f32 = 1.0 / 120.0;
    let Some(mut m) = movement(0.0) else { return };
    let idle = Input::default();
    // The axis pushed right: a tenth of a degree a frame, whatever the
    // frame's length, and the body's lean close behind it.
    for _ in 0..20 {
        m.step(FRAME, &Input { slow_lean: 1.0, ..idle }, &Flat);
    }
    assert!((m.tilt - 2.0).abs() < 1e-4, "twenty frames are two degrees: {}", m.tilt);
    assert!(m.smoothed_tilt > 1.0 && m.smoothed_tilt < m.tilt, "the body follows: {}", m.smoothed_tilt);
    // Let go, the lean stays where it was left, and the body comes to it.
    run(&mut m, 1.0, idle);
    assert!((m.tilt - 2.0).abs() < 1e-4 && (m.smoothed_tilt - 2.0).abs() < 1e-3, "{} {}", m.tilt, m.smoothed_tilt);
    // The lean key of that side straightens a character that leans its
    // way, as it does a lean of its own.
    m.step(FRAME, &Input { lean_toggle: [false, true], ..idle }, &Flat);
    assert_eq!(m.tilt, 0.0);
    run(&mut m, 1.0, idle);
    assert!(m.smoothed_tilt.abs() < 1e-3, "{}", m.smoothed_tilt);
    // While the axis is pushed the lean keys do nothing: the frame after
    // a slow lean neither leans nor straightens.
    m.step(FRAME, &Input { slow_lean: -1.0, ..idle }, &Flat);
    m.step(FRAME, &Input { slow_lean: -1.0, lean_toggle: [false, true], ..idle }, &Flat);
    assert!((m.tilt + 0.2).abs() < 1e-4, "the key was not heard: {}", m.tilt);
    // It stops at the lean's limit.
    for _ in 0..80 {
        m.step(FRAME, &Input { slow_lean: -1.0, ..idle }, &Flat);
    }
    assert_eq!(m.tilt, -5.0);

    // The walk key: the speed put to none and what it was put aside; the
    // next press takes it up again. A walk goes on at the floor's pace.
    let Some(mut m) = movement(0.0) else { return };
    let forward = walk([0.0, 1.0]);
    run(&mut m, 2.0, forward);
    let (full, pace) = (m.character_speed, m.ground_speed());
    m.step(DT, &Input { walk_toggle: true, ..forward }, &Flat);
    assert_eq!(m.character_speed, 0.0);
    run(&mut m, 2.0, forward);
    let slow = m.ground_speed();
    println!("walking {pace:.3} m/s; with the walk key {slow:.3} m/s");
    assert!(slow > 0.1 && slow < pace * 0.6, "{slow} against {pace}");
    m.step(DT, &Input { walk_toggle: true, ..forward }, &Flat);
    assert_eq!(m.character_speed, full);
    // Bound to be held, the key coming up takes the speed up again too,
    // and does nothing to a speed that is not at none.
    m.step(DT, &Input { walk_toggle: true, ..forward }, &Flat);
    m.step(DT, &Input { walk_end: true, ..forward }, &Flat);
    assert_eq!(m.character_speed, full);
    m.step(DT, &Input { walk_end: true, ..forward }, &Flat);
    assert_eq!(m.character_speed, full);

    // A backpack's drop: its gesture plays on the body's animator, and
    // the mark for the pack leaving the shoulders comes a quarter of the
    // way through its state.
    let Some(mut m) = movement(0.0) else { return };
    let still = Input { hands_idle: true, ..Input::default() };
    run(&mut m, 0.5, still);
    assert!(m.can_drop_backpack());
    assert!(m.drop_backpack());
    assert!(!m.can_drop_backpack(), "not while a gesture plays");
    let (mut dropped, mut over) = (None, None);
    for frame in 0..(3.0 / FRAME) as usize {
        m.step(FRAME, &still, &Flat);
        let at = (frame + 1) as f32 * FRAME;
        if m.take_acted().contains(&Acted::BackpackDropped) {
            dropped.get_or_insert(at);
        }
        if m.interaction().is_none() {
            over.get_or_insert(at);
        }
    }
    let (dropped, over) = (dropped.expect("the pack is dropped"), over.expect("the gesture ends"));
    println!("a backpack dropped: off the shoulders {dropped:.3} s after the key, the gesture over at {over:.3} s");
    // A quarter of a clip of 1.333 s.
    assert!((dropped - 0.333).abs() < 0.03, "{dropped}");
    assert!(over > 1.1 && over < 1.45, "{over}");
    assert!(m.can_drop_backpack());
    // Not in a sprint.
    let forward = Input { hands_idle: true, ..walk([0.0, 1.0]) };
    run(&mut m, 0.5, forward);
    m.step(DT, &Input { sprint_toggle: true, ..forward }, &Flat);
    run(&mut m, 1.5, forward);
    assert_eq!(m.state, State::Sprint);
    assert!(!m.can_drop_backpack() && !m.drop_backpack());
}

/// The look at the selector, the look all over and the fullest fire mode
/// (`docs/spec/firing.md`, "Three more keys"), on the install's own
/// animator for the AKM and the M3. From the game's code; no recording has
/// any of the three.
#[test]
fn a_weapon_is_looked_over_and_put_on_automatic() {
    use trp_sim::{Firearm, FireMode};
    let Some((_, data, settings, ..)) = weapon_of("59d6088586f774275f37482f") else { return };
    let rested = || {
        let mut w = Firearm::new(settings.clone(), &data);
        w.start_drawn();
        w
    };
    // The look at the selector: its own state, and the weapon idles again.
    let mut w = rested();
    assert!(w.check_fire_mode());
    assert!(w.checking() && !w.check_fire_mode(), "one look at a time");
    let (entered, _) = run_weapon(&mut w, 2.0);
    println!("the AKM's selector looked at: {entered:?}");
    assert_eq!(state_names(&entered), ["FIREMODE CHECK", "IDLE"]);
    assert!(!w.checking());
    // It puts the sights down; the look all over is refused with them up.
    let mut w = rested();
    w.set_aim(true, true);
    assert!(!w.examine() && w.aiming());
    assert!(w.check_fire_mode() && !w.aiming());
    run_weapon(&mut w, 2.0);
    // The look all over, left to play out.
    assert!(w.examine());
    let (entered, _) = run_weapon(&mut w, 4.5);
    println!("the AKM looked over: {entered:?}");
    assert_eq!(state_names(&entered), ["LOOK", "IDLE"]);
    assert!((entered[1].1 - entered[0].1 - 3.333).abs() < 0.05, "the clip's own length: {entered:?}");
    // The trigger ends it at once, and the weapon fires.
    let mut w = rested();
    let rounds = w.magazine;
    assert!(w.examine());
    run_weapon(&mut w, 1.0);
    w.set_trigger(true, true);
    assert!(!w.checking());
    let (entered, _) = run_weapon(&mut w, 0.5);
    w.set_trigger(false, true);
    println!("the trigger in a look all over: {entered:?}");
    assert_eq!(state_names(&entered).first().copied(), Some("FIRE"));
    assert!(w.magazine < rounds, "a shot was fired: {:?} of {rounds:?}", w.magazine);

    // The fullest way of firing: the AKM's automatic, from single; a
    // second asking changes nothing.
    let mut w = rested();
    assert_eq!(w.mode(), FireMode::Single);
    assert!(w.force_auto_fire_mode());
    assert_eq!(w.mode(), FireMode::FullAuto);
    run_weapon(&mut w, 2.0);
    assert!(!w.force_auto_fire_mode());
    assert_eq!(w.mode(), FireMode::FullAuto);

    // A backpack's drop is the weapon's own operation: only from rest,
    // taking no trigger, and over when the pack is off.
    let mut w = rested();
    w.check_magazine();
    assert!(!w.drop_backpack(), "not in a look at the weapon");
    let (entered, _) = run_weapon(&mut w, 8.0);
    println!("the magazine looked at: {entered:?}; idle {}, checking {}", w.is_idle(), w.checking());
    w.set_aim(true, true);
    assert!(w.drop_backpack() && w.dropping_backpack() && !w.aiming());
    let rounds = w.magazine;
    w.set_trigger(true, true);
    run_weapon(&mut w, 0.5);
    w.set_trigger(false, true);
    assert_eq!(w.magazine, rounds, "no shot through the drop");
    assert!(!w.examine() && !w.check_fire_mode());
    w.backpack_dropped();
    assert!(!w.dropping_backpack() && w.examine());

    // The M3 has two ways of firing and neither is automatic or a burst:
    // it stays as it is, and its selector can be looked at.
    let Some((_, data, settings, ..)) = weapon_of("6259b864ebedf17603599e88") else { return };
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    let mode = w.mode();
    assert!(!w.force_auto_fire_mode());
    assert_eq!(w.mode(), mode);
    assert!(w.examine());
    let (entered, _) = run_weapon(&mut w, 6.0);
    println!("the M3 looked over: {entered:?}");
    assert_eq!(state_names(&entered).last().copied(), Some("IDLE"));
}

/// The mouse's turn for a count and the wheel's notch go by figures of the
/// install's (`docs/spec/settings.md`, "The mouse"): the build's own input
/// axes are read by the engine's fixed layout for one, and the bindings'
/// own factors from the install's settings.
#[test]
fn the_install_s_mouse_axes_and_bindings_are_read() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let axes = install.engine_axes().expect("the build's input settings");
    let axis = |name: &str| axes.iter().find(|axis| axis.name == name).unwrap_or_else(|| panic!("no axis named {name}"));
    let (across, up, wheel) = (axis("Mouse X"), axis("Mouse Y"), axis("Mouse Scroll"));
    let by = install.axis_sensitivity("TurnX");
    println!("{} axes; Mouse X {} a count, the wheel {} a notch; the turn's binding multiplies by {by:?}", axes.len(), across.sensitivity, wheel.sensitivity);
    // The mouse's movement, its first two axes, at the same figure.
    assert_eq!((across.kind, across.axis, up.kind, up.axis), (1, 0, 1, 1));
    assert!(across.sensitivity > 0.0 && across.sensitivity == up.sensitivity);
    // The wheel is the mouse's third axis, and a notch of it reads one:
    // which is what makes one notch in a frame a press and two none.
    assert_eq!((wheel.kind, wheel.axis, wheel.sensitivity), (1, 2, 1.0));
    assert!(by.is_none_or(|by| by > 0.0));
    // The install's bindings as the input manager takes them: the keys'
    // axes and not the mouse's, and the bindings the night's keys go by.
    let controls = trp_sim::base::bindings::install_controls(&install);
    if controls.keys.is_empty() {
        return;
    }
    let named: Vec<&str> = controls.axes.iter().map(|axis| axis.name.as_str()).collect();
    assert!(named.contains(&"MoveX") && named.contains(&"LeanX") && !named.contains(&"TurnX"), "{named:?}");
    for name in ["ReloadWeapon", "Walk", "DropBackpack", "ForceAutoWeaponMode", "CheckFireMode", "ExamineWeapon"] {
        assert!(controls.group(name).is_some(), "{name}");
    }
}
