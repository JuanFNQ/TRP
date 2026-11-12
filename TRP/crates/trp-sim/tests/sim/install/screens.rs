//! Screens and the inventory taking the keys, and the lean keys.

use super::*;

/// With a screen open the character is handed nothing: a walk with the
/// key still down runs out exactly as one with the key let go, and the
/// view does not turn. When the screen shuts the key held is heard again.
#[test]
fn a_screen_takes_the_keys_from_a_walk() {
    let (Some(mut let_go), Some(mut screened)) = (movement(0.0), movement(0.0)) else { return };
    run(&mut let_go, 2.0, walk([0.0, 1.0]));
    run(&mut screened, 2.0, walk([0.0, 1.0]));
    let (from, yaw) = (screened.position[2], screened.yaw);
    let held = Input { screen: true, inventory: true, look_delta: [3.0, 1.0], ..walk([0.0, 1.0]) };
    let mut apart = 0.0f32;
    for _ in 0..90 {
        let_go.step(DT, &Input::default(), &Flat);
        screened.step(DT, &held, &Flat);
        apart = apart.max((let_go.position[2] - screened.position[2]).abs());
    }
    println!(
        "a walk with the inventory open: {:.3} m on and {:?} after 1.5 s; {apart:.6} m from one with the key let go",
        screened.position[2] - from,
        screened.state
    );
    assert_eq!(apart, 0.0);
    assert_eq!(screened.state, State::Idle);
    assert!(screened.ground_speed() < 0.01);
    assert_eq!(screened.yaw, yaw, "the view is not turned");
    // The key is still down when the screen shuts.
    run(&mut screened, 1.0, walk([0.0, 1.0]));
    assert_eq!(screened.state, State::Move);
}

/// A sprint ends the same way: the frame after the screen opens it is
/// handed no forward, and goes through its stop into standing.
#[test]
fn a_screen_ends_a_sprint() {
    let (Some(mut let_go), Some(mut screened)) = (movement(0.0), movement(0.0)) else { return };
    let sprint = Input { sprint: true, ..walk([0.0, 1.0]) };
    for m in [&mut let_go, &mut screened] {
        run(m, 0.5, walk([0.0, 1.0]));
        run(m, 3.0, sprint);
        assert_eq!(m.state, State::Sprint);
    }
    let from = screened.position[2];
    let mut apart = 0.0f32;
    for _ in 0..120 {
        // The same keys let go, but for the sprint key: its release would
        // be a command of its own, which a screen swallows.
        let_go.step(DT, &Input { sprint: true, ..Input::default() }, &Flat);
        screened.step(DT, &Input { screen: true, inventory: true, ..sprint }, &Flat);
        apart = apart.max((let_go.position[2] - screened.position[2]).abs());
    }
    println!(
        "a sprint with the inventory open: {:.3} m on and {:?} after 2 s; {apart:.6} m from one with forward let go",
        screened.position[2] - from,
        screened.state
    );
    assert_eq!(apart, 0.0);
    assert_eq!(screened.state, State::Idle);
    assert!(screened.ground_speed() < 0.01);
}

/// A key that changes under an open screen is never heard of. A step
/// aside asked for while the screen is open is not made when it shuts with
/// the key still down, and a jump pressed under it does not happen.
#[test]
fn keys_pressed_under_a_screen_are_lost() {
    let Some(mut m) = movement(0.0) else { return };
    let screen = Input { screen: true, ..Input::default() };
    run(&mut m, 0.2, screen);
    run(&mut m, 0.5, Input { step: 1, jump: true, prone: true, crouch: true, pose_delta: -1.0, lean: 1.0, ..screen });
    assert_eq!((m.state, m.step, m.prone), (State::Idle, 0, false));
    assert_eq!((m.pose_level, m.tilt), (1.0, 0.0));
    // The screen shuts with the step key still down: no press was heard.
    run(&mut m, 0.5, Input { step: 1, ..Input::default() });
    assert_eq!((m.state, m.step), (State::Idle, 0));
    // Let go and pressed again, it steps.
    run(&mut m, 0.1, Input::default());
    run(&mut m, 0.5, Input { step: 1, ..Input::default() });
    assert_eq!(m.state, State::Sidestep);
}

/// A lean held when the screen opens stays: its key coming up is not
/// heard. It stays when the screen has shut with the key up, too, since
/// nothing holds a lean to its key; the key pressed again straightens the
/// character, as a press toward the side already leaned to does.
#[test]
fn a_lean_is_kept_under_a_screen() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 1.0, Input { lean: 1.0, ..Input::default() });
    let leaning = m.tilt;
    assert!(leaning > 0.0);
    run(&mut m, 1.0, Input { screen: true, ..Input::default() });
    assert_eq!(m.tilt, leaning);
    run(&mut m, 1.0, Input::default());
    assert_eq!(m.tilt, leaning, "the screen has shut and the lean stands");
    m.step(DT, &Input { lean: 1.0, ..Input::default() }, &Flat);
    assert_eq!(m.tilt, 0.0, "the key pressed again straightens");
    // Let go, nothing more: there is no lean to that side to take back.
    run(&mut m, 0.5, Input::default());
    assert_eq!(m.tilt, 0.0);
}

/// The lean keys are presses and releases (movement spec, "Lean"). A
/// second key pressed while the first is held leans the other way, and
/// the first let go after that does nothing. A lean a sprint put to zero
/// is not taken up again while its key stays down, and neither is one a
/// jump put to zero; in a sprint and in a jump a press is not heard at all.
#[test]
fn the_lean_keys_are_presses_and_releases() {
    let Some(mut m) = movement(0.0) else { return };
    let (left, right, both) = ([true, false], [false, true], [true, true]);
    let keys = |lean_keys: [bool; 2]| Input { lean_keys, ..Input::default() };
    run(&mut m, 0.5, keys(right));
    assert_eq!(m.tilt, 5.0);
    // The left key too: the lean goes over to the left.
    run(&mut m, 0.5, keys(both));
    assert_eq!(m.tilt, -5.0);
    // The right key let go: the character does not lean right, so nothing.
    run(&mut m, 0.5, keys(left));
    assert_eq!(m.tilt, -5.0);
    // The left key let go: straight.
    run(&mut m, 0.5, keys([false, false]));
    assert_eq!(m.tilt, 0.0);

    // A sprint with the lean key held down throughout.
    run(&mut m, 1.0, keys(right));
    assert_eq!(m.tilt, 5.0);
    let walking = Input { move_dir: [0.0, 1.0], ..keys(right) };
    run(&mut m, 0.5, walking);
    run(&mut m, 2.0, Input { sprint: true, ..walking });
    assert_eq!((m.state, m.tilt), (State::Sprint, 0.0));
    // A press of the other key in the sprint is not heard.
    run(&mut m, 0.2, Input { sprint: true, lean_keys: both, ..walking });
    assert_eq!(m.tilt, 0.0);
    // Standing again with the key still down: no lean comes back.
    run(&mut m, 2.0, keys(both));
    assert_eq!((m.state, m.tilt), (State::Idle, 0.0));
    run(&mut m, 0.5, keys(right));
    assert_eq!(m.tilt, 0.0, "a key let go with no lean to its side does nothing");
    // Let go and pressed again, it leans.
    run(&mut m, 0.1, keys([false, false]));
    run(&mut m, 0.5, keys(right));
    assert_eq!(m.tilt, 5.0);
    println!("the lean keys: a second key leans the other way, and a sprint's zero stays with the key down");
}

/// The inventory opening switches a held vault key off, and since every
/// change of the key switches it over, letting that key go afterwards
/// switches it on: with the key up the hold's time then runs, and a jump
/// is refused as after a long hold. Read from the game's code
/// (`Player.Vaulting`, `DisableVaulting`); no recording has it.
#[test]
fn the_inventory_switches_a_held_vault_key_off() {
    let Some(mut m) = movement(0.0) else { return };
    let frame = 1.0 / 120.0;
    let key = Input { vault: true, ..Input::default() };
    for _ in 0..12 {
        m.step(frame, &key, &Flat);
    }
    // Opened and shut again with the key down throughout.
    for _ in 0..60 {
        m.step(frame, &Input { screen: true, inventory: true, ..key }, &Flat);
    }
    for _ in 0..12 {
        m.step(frame, &key, &Flat);
    }
    // Switched off, a release of the key jumps however long it was down.
    let Some(mut tapped) = movement(0.0) else { return };
    for _ in 0..12 {
        tapped.step(frame, &key, &Flat);
    }
    for _ in 0..200 {
        tapped.step(frame, &Input { screen: true, inventory: true, ..key }, &Flat);
    }
    tapped.step(frame, &Input { jump: true, ..Input::default() }, &Flat);
    let (states, _) = until(&mut tapped, &Flat, Input::default(), 0.2, |m| m.state == State::Jump);
    assert_eq!(states.last(), Some(&State::Jump), "the hold's time was cleared when the inventory opened");
    // And the key's coming up has switched it on: a second later a jump is refused.
    m.step(frame, &Input { jump: true, ..Input::default() }, &Flat);
    let (states, _) = until(&mut m, &Flat, Input::default(), 3.0, |_| false);
    assert!(states.contains(&State::Jump), "the release itself still jumps: {states:?}");
    assert_eq!(m.state, State::Idle);
    m.step(frame, &Input { jump: true, ..Input::default() }, &Flat);
    let (states, _) = until(&mut m, &Flat, Input::default(), 0.3, |m| m.state == State::Jump);
    assert_eq!(states, [State::Idle], "with the key up its try is on and its time long run out");
}
