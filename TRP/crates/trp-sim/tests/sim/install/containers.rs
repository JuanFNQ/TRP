//! The hands working a container, examining from the world and searching.

use super::*;

/// The hands' gestures on the world, on the body's own animator
/// (`docs/spec/interactions.md`): a door worked, a container searched and
/// shut, a thing taken up, a thing unlocked, and the body's pose for an
/// open inventory. The states, their lengths and the curves that hand the
/// left arm over to the body are the install's; how long each gesture's
/// flag is up, when a searched container is shown and how long a pick-up
/// holds the character are set beside the recordings made by hand
/// (`local\traces\byhand1`, at 120 frames a second like this).
#[test]
fn the_hands_work_a_container_and_take_a_thing() {
    use trp_sim::{Acted, Interaction, WorldInteraction};
    let Some(mut m) = movement(0.0) else { return };
    const FRAME: f32 = 1.0 / 120.0;
    let still = Input { hands_idle: true, ..Input::default() };
    let gesture = |m: &Movement| {
        let (state, _, next) = m.body.interaction_state().expect("the interactions' layer");
        next.unwrap_or(state).trim_start_matches("InteractionsLayer.").to_owned()
    };
    // Steps for some seconds, and says when each thing came: every state
    // the layer entered, what the character's work came to, and the most
    // of the left hand that was the body's.
    let run = |m: &mut Movement, seconds: f32, input: Input| -> (Vec<(String, f32)>, Vec<(Acted, f32)>, f32) {
        let (mut entered, mut acted, mut most, mut last) = (Vec::new(), Vec::new(), 0.0f32, gesture(m));
        for frame in 0..(seconds / FRAME).round() as usize {
            m.step(FRAME, &input, &Flat);
            let at = (frame + 1) as f32 * FRAME;
            acted.extend(m.take_acted().into_iter().map(|what| (what, at)));
            most = most.max(m.arm_pose(false).hand);
            let now = gesture(m);
            if now != last {
                entered.push((now.clone(), at));
                last = now;
            }
        }
        (entered, acted, most)
    };
    // How long a gesture's flag stays up, as a recording shows it: from
    // the first frame that has it up (the one the key is in) to the first
    // that has it down. Asked for after `already` frames of it.
    let lasts = |m: &mut Movement, input: Input, already: usize| -> f32 {
        let mut frames = already;
        while m.interaction().is_some() && frames < 600 {
            m.step(FRAME, &input, &Flat);
            frames += 1;
        }
        (frames - 1) as f32 * FRAME
    };
    run(&mut m, 1.0, still);
    assert_eq!(gesture(&m), "Idle");
    assert!(m.arm_pose(false).hand < 0.01, "at rest the left hand is the weapon's");
    let walking = Input { move_dir: [0.0, 1.0], ..still };

    // A door pulled: the hand makes the door's gesture and the character
    // is held to nothing. Recorded: the flag up for 1.737 s (twice, for
    // two of the doors' gestures), the character walking off meanwhile.
    let stance = m.pose_level;
    let from = m.position;
    assert!(m.work(Interaction::PullHingeLeft));
    assert!(!m.acting() && m.interaction() == Some(Interaction::PullHingeLeft));
    let (entered, acted, most) = run(&mut m, 1.0, walking);
    let gone = ((m.position[0] - from[0]).powi(2) + (m.position[2] - from[2]).powi(2)).sqrt();
    let up = lasts(&mut m, walking, 120);
    println!("a door pulled: the layer {entered:?}; the flag up {up:.3} s (recorded 1.737); the character went {gone:.2} m in its first second; the body has {most:.2} of the left hand at the most");
    assert_eq!(entered[0].0, "17 Door Pull Hinge Left");
    assert!(entered[0].1 < 0.03, "the gesture begins with the key");
    assert!(acted.is_empty() && gone > 0.3 && most > 0.99);
    assert!((m.pose_level - stance).abs() < 1e-4, "nothing is crouched to");
    assert!((up - 1.737).abs() < 0.005, "{up}");
    assert!(m.work(Interaction::DoorPushForward));
    let up = lasts(&mut m, still, 0);
    println!("a door pushed: the flag up {up:.3} s (recorded 1.737)");
    assert!((up - 1.737).abs() < 0.005, "{up}");
    // The other gestures recorded (byhand2): an open door
    // pushed shut, an open door pulled back (also a dead Scav's opening),
    // and a container's or a trunk's default two.
    for (gesture, recorded) in [
        (Interaction::PushHingeLeft, 1.738),
        (Interaction::DoorPullBackward, 1.939),
        (Interaction::ContainerOpenDefault, 1.563),
        (Interaction::ContainerCloseDefault, 1.563),
    ] {
        run(&mut m, 0.5, still);
        assert!(m.work(gesture));
        let up = lasts(&mut m, still, 0);
        println!("{gesture:?}: the flag up {up:.3} s (recorded {recorded})");
        assert!((up - recorded).abs() < 0.009, "{gesture:?}: {up}");
    }

    // A container searched: shown when the game's wait is over (the
    // install's 0.55 s; recorded, the screen was up 0.550 to 0.553 s after
    // the key ten times), and the screen opening stops the gesture. The
    // first frame stepped is the key's own.
    run(&mut m, 1.0, still);
    let crate_at = [m.position[0], m.position[1] + 0.5, m.position[2] + 0.8];
    assert!(m.search(Interaction::ContainerHingeUpOpen, crate_at));
    assert!(m.searching() && !m.acting());
    let (entered, acted, most) = run(&mut m, 0.56, still);
    println!("a container searched: the layer {entered:?}; {acted:?}; the body has {most:.2} of the left hand by then");
    assert_eq!(entered[0].0, "29 Container Hinge Up Open");
    assert_eq!(acted.iter().map(|entry| entry.0).collect::<Vec<_>>(), [Acted::Searched]);
    assert!((acted[0].1 - FRAME - 0.55).abs() < 0.001, "{acted:?}");
    assert!(!m.searching() && m.interaction().is_some());
    let open = Input { inventory: true, screen: true, ..still };
    let (entered, _, _) = run(&mut m, 0.1, open);
    assert!(m.interaction().is_none(), "the screen opening stops the gesture");
    assert_eq!(entered.last().map(|entry| entry.0.as_str()), Some("Idle"), "{entered:?}");
    run(&mut m, 1.0, still);
    // Shut again with the hand. Recorded: the flag up for 1.771 s.
    assert!(m.work(Interaction::ContainerHingeUpClose));
    let up = lasts(&mut m, still, 0);
    println!("a container shut: the flag up {up:.3} s (recorded 1.771)");
    assert!((up - 1.771).abs() < 0.009, "{up}");
    // Walked away from in the wait, it is not shown.
    assert!(m.search(Interaction::ContainerOpenDefault, [m.position[0], m.position[1], m.position[2] - 0.8]));
    let (_, acted, _) = run(&mut m, 0.6, walking);
    assert_eq!(acted.iter().map(|entry| entry.0).collect::<Vec<_>>(), [Acted::LeftUnsearched], "{acted:?}");
    run(&mut m, 2.0, still);

    // A thing unlocked (the game's other way to a thing, for a key, a
    // card or a kick): crouched to, worked for 1.6 s, and done with the
    // frame after where it neither places the character nor has a grip;
    // no step is taken meanwhile. Compared with nothing.
    let from = m.position;
    let gone = |m: &Movement| ((m.position[0] - from[0]).powi(2) + (m.position[2] - from[2]).powi(2)).sqrt();
    assert!(m.unlock(WorldInteraction { interaction: Interaction::ContainerOpenDefault, snap: false, height: 0.5 }));
    assert!(m.acting() && !m.unlock(WorldInteraction { interaction: Interaction::ContainerOpenDefault, snap: false, height: 0.5 }));
    let (mut entered, mut acted, most) = run(&mut m, 1.5, walking);
    let held = gone(&m);
    let (more, after, _) = run(&mut m, 1.5, walking);
    entered.extend(more.into_iter().map(|(name, at)| (name, at + 1.5)));
    acted.extend(after.into_iter().map(|(what, at)| (what, at + 1.5)));
    println!("a thing unlocked: the layer {entered:?}; {acted:?}; the body has {most:.2} of the left hand at the most");
    println!("with the walk key held through it the character went {held:.3} m in the first 1.5 s and {:.2} m by 3 s", gone(&m));
    assert!(held < 0.001 && gone(&m) > 1.0);
    assert_eq!(entered.iter().map(|entry| entry.0.as_str()).collect::<Vec<_>>(), ["27_Container_Open_Default".replace('_', " "), "Idle".to_owned()]);
    // The clip is 1.733 s long and gives way at three quarters of it.
    assert!((entered[1].1 - entered[0].1 - 1.733 * 0.75).abs() < 0.03, "{entered:?}");
    assert!(most > 0.99);
    assert_eq!(acted.iter().map(|entry| entry.0).collect::<Vec<_>>(), [Acted::Worked, Acted::Done]);
    assert!((acted[0].1 - 1.6).abs() < 0.02 && acted[1].1 - acted[0].1 < 0.02, "{acted:?}");
    assert!(m.pose_level < 0.05 && stance > 0.9, "a thing half a metre up is crouched to");
    assert!(!m.acting());
    m.let_go(&Flat);
    run(&mut m, 2.0, still);
    assert!(m.arm_pose(false).hand < 0.01);
    assert!(m.pose_level > 0.9, "and the stance is taken again when the thing is let go");
    // One that places the character lasts its three seconds.
    assert!(m.unlock(WorldInteraction { interaction: Interaction::DoorKeyOpen, snap: true, height: 1.2 }));
    let (entered, acted, _) = run(&mut m, 4.0, still);
    println!("a lock that places the character: the layer {entered:?}; {acted:?}");
    assert!((acted[0].1 - 1.6).abs() < 0.02 && (acted[1].1 - 3.0).abs() < 0.02, "{acted:?}");
    assert_eq!(entered[0].0, "22 Door Key Open");

    // A thing taken up: only standing still; the view is held; taken
    // 0.6 s in. Recorded: the state for it lasted 0.614 s, the gesture's
    // flag was up 1.466 s, and with the view 74 degrees down the stance
    // stood at a third for the while.
    run(&mut m, 0.5, Input { move_dir: [0.0, 1.0], ..still });
    assert!(!m.pick_up(), "not out of a walk");
    run(&mut m, 1.5, still);
    assert!(m.pick_up());
    let yaw = m.yaw;
    let (entered, acted, most) = run(&mut m, 1.0, Input { look_delta: [0.1, 0.0], ..still });
    let up = lasts(&mut m, Input { look_delta: [0.1, 0.0], ..still }, 120);
    let turned = trp_sim::base::math::delta_angle(yaw, m.yaw);
    println!("a thing taken up: the layer {entered:?}; {acted:?}; the flag up {up:.3} s (recorded 1.466); the body has {most:.2} of the left hand at the most");
    assert_eq!(acted.iter().map(|entry| entry.0).collect::<Vec<_>>(), [Acted::PickedUp]);
    assert!((acted[0].1 - 0.614).abs() < 0.02, "{acted:?}");
    assert_eq!(entered[0].0, "8 Take");
    assert!(most > 0.99);
    assert!((up - 1.466).abs() < 0.009, "{up}");
    // The view stood still for the 0.6 s and turned a tenth of a degree a
    // frame after.
    assert!((turned - (up - 0.6) * 12.0).abs() < 0.5, "{turned}");
    // Looked at from above, it is gone down to.
    run(&mut m, 1.0, still);
    run(&mut m, 0.5, Input { look_delta: [0.0, 1.3], ..still });
    assert!(m.pitch > 70.0, "{}", m.pitch);
    assert!(m.pick_up());
    let (_, acted, _) = run(&mut m, 0.5, still);
    println!("taken up with the view {:.0} degrees down: the stance {:.2} half a second in (recorded 0.33)", m.pitch, m.pose_level);
    assert!(acted.is_empty() && (m.pose_level - 1.0 / 3.0).abs() < 0.05, "{}", m.pose_level);
    run(&mut m, 2.0, still);
    assert!(m.pose_level > 0.95, "and stood up from again");
    run(&mut m, 0.6, Input { look_delta: [0.0, -1.3], ..still });

    // The body's own pose for an open inventory, once the hands are down.
    run(&mut m, 1.0, still);
    let open = Input { inventory: true, screen: true, ..still };
    let (entered, _, most) = run(&mut m, 0.5, open);
    assert!(entered.is_empty() && most < 0.01, "not before the hands have gone down: {entered:?}");
    let (entered, _, most) = run(&mut m, 2.0, Input { hands_in_inventory: true, hands_idle: false, ..open });
    println!("the inventory open: the layer {entered:?}; the body has {most:.2} of the left hand at the most");
    // The layer's resting state has two ways for it, and the one into the
    // looping pose comes first in the controller's list, so the state
    // named for the start of it is never entered.
    assert_eq!(entered.iter().map(|entry| entry.0.as_str()).collect::<Vec<_>>(), ["Inventory"]);
    assert!(most > 0.99);
    let (entered, _, _) = run(&mut m, 2.0, still);
    println!("and shut: {entered:?}");
    assert_eq!(entered.iter().map(|entry| entry.0.as_str()).collect::<Vec<_>>(), ["Inventory End", "Idle"]);
    assert!(m.arm_pose(false).hand < 0.01);
}

/// A thing examined where it lies (`courses10`'s `examine-world-loot`: the
/// state `Pickup` from the key, 2.829 s, the examine begun on the first
/// frame past 0.6 s, the state `Idle` again on the frame the examine ended,
/// 1.177 s in): the pick-up's state, its action 0.6 s in, held until the
/// examine is said to be over and left with the next step; never told, it
/// lasts its 5 s (`PickUpState.ManualAnimatorMoveUpdate`).
#[test]
fn an_examine_from_the_world_holds_the_pick_up() {
    use trp_sim::Acted;
    let Some(mut m) = movement(0.0) else { return };
    const FRAME: f32 = 1.0 / 120.0;
    let still = Input { hands_idle: true, ..Input::default() };
    for _ in 0..120 {
        m.step(FRAME, &still, &Flat);
    }
    assert!(m.pick_up_to_examine());
    assert!(!m.pick_up() && !m.pick_up_to_examine(), "one at a time");
    let mut began = None;
    for frame in 1..=140 {
        m.step(FRAME, &still, &Flat);
        if m.take_acted().contains(&Acted::PickedUp) {
            assert!(began.replace(frame as f32 * FRAME).is_none(), "its action once");
        }
    }
    let began = began.expect("the examine begun");
    println!("an examine from the world: begun {began:.4} s in");
    assert!(began > 0.6 && began <= 0.6 + FRAME + 1e-4, "{began}");
    assert!(m.acting(), "held past its action");
    m.examined();
    assert!(m.acting(), "until the next step");
    m.step(FRAME, &still, &Flat);
    assert!(!m.acting());
    // Never told: 5 s.
    for _ in 0..120 {
        m.step(FRAME, &still, &Flat);
    }
    assert!(m.pick_up_to_examine());
    let mut frames = 0;
    while m.acting() && frames < 1000 {
        m.step(FRAME, &still, &Flat);
        frames += 1;
    }
    println!("never told: {:.3} s", frames as f32 * FRAME);
    assert!((frames as f32 * FRAME - 5.0).abs() <= FRAME + 1e-3, "{frames}");
    // A take is as before: over with its action.
    for _ in 0..120 {
        m.step(FRAME, &still, &Flat);
    }
    assert!(m.pick_up());
    for _ in 0..80 {
        m.step(FRAME, &still, &Flat);
    }
    assert!(!m.acting() && m.take_acted().contains(&Acted::PickedUp));
}

/// A container searched lying down (recorded in byhand2,
/// `Opening Crate While Prone`): on the key the character is no longer
/// lying and its stance is at its lowest; the getting up lasted 0.834 s
/// and it then stood at stance 0, the gesture having begun with the key.
#[test]
fn a_search_lying_down_gets_the_character_up() {
    use trp_sim::Interaction;
    let Some(mut m) = movement(0.0) else { return };
    const FRAME: f32 = 1.0 / 120.0;
    let still = Input { hands_idle: true, ..Input::default() };
    m.step(FRAME, &Input { prone: true, ..still }, &Flat);
    for _ in 0..240 {
        m.step(FRAME, &still, &Flat);
    }
    assert_eq!(m.state, State::ProneIdle);
    assert!(m.search(Interaction::ContainerHingeUpOpen, [m.position[0], 0.3, m.position[2] + 0.9]));
    let (mut up_from, mut up_to) = (None, None);
    for frame in 0..240 {
        m.step(FRAME, &still, &Flat);
        let at = frame as f32 * FRAME;
        if m.state == State::FromProne && up_from.is_none() {
            up_from = Some(at);
        }
        if m.state == State::Idle && up_from.is_some() && up_to.is_none() {
            up_to = Some(at);
        }
    }
    let (from, to) = (up_from.expect("getting up"), up_to.expect("standing"));
    println!("searched lying down: getting up from {from:.3} s to {to:.3} s after the key, {:.3} s (recorded 0.834); the stance {:.2} (recorded 0)", to - from, m.pose_level);
    assert!(from < 0.02, "it gets up on the key");
    assert!(((to - from) - 0.834).abs() < 0.02, "{}", to - from);
    assert!(m.pose_level < 0.01);
}
