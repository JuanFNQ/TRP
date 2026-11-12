//! Tests of the doors and containers the hands work: their waits, swings and sounds.
use super::*;
use trp_data::formats::curve::Key;

/// The settings object's `DoorCurve` as the install has it (read with
/// `probe hardsettings DoorCurve`), and its `DelayToOpenContainer`.
fn settings() -> ThingSettings {
    let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
    let curve = Curve {
        keys: vec![key(0.0, 0.0, 1.11), key(0.568, 0.546, 0.61), key(1.718, 0.976, 0.10), key(2.5, 1.0, 0.01)],
    };
    ThingSettings { curve, answers_after: 0.55 }
}

/// A door upright in a wall that runs along x, hinged at the origin,
/// pointing along +x when shut and turning about straight up, so that
/// it opens towards -z.
fn door() -> Thing {
    Thing::door(Hinge { at: [0.0; 3], axis: [0.0, 1.0, 0.0], forward: [1.0, 0.0, 0.0] }, 0.0, 110.0)
}

const DT: f32 = 1.0 / 120.0;

/// Frames until the thing is no longer being worked, stepping it. The
/// key's own frame has been stepped before.
fn frames_until_done(thing: &mut Thing, settings: &ThingSettings) -> usize {
    (1..2000).find(|_| {
        thing.step(DT, settings);
        thing.state() != ThingState::Interacting
    })
    .unwrap()
}

/// A door is offered to be opened shut and to be shut open, and nothing
/// while it is being worked; it waits half a second, swings for the
/// curve's 2.5 s, and is open on the frame 3 s after the key.
#[test]
fn a_door_waits_half_a_second_and_swings_for_the_curve() {
    let settings = settings();
    let mut door = door();
    assert_eq!(door.action(), Some(Action::Open));
    door.work(true, [0.5, 0.0, 1.0], &settings);
    assert_eq!((door.state(), door.action()), (ThingState::Interacting, None));
    // The key's own frame, and 59 more: still at its shut angle.
    for _ in 0..60 {
        door.step(DT, &settings);
    }
    assert_eq!(door.angle(), 0.0);
    // Half a second after the key the swing's first frame puts it
    // where the curve begins, and the next one on by a frame of it.
    door.step(DT, &settings);
    assert_eq!(door.angle(), 0.0);
    door.step(DT, &settings);
    let first = 110.0 * settings.curve.evaluate(DT);
    assert!((door.angle() - first).abs() < 1e-3, "{} against {first}", door.angle());
    // Recorded: open 360 frames after the key at 120 a second.
    let frames = 61 + frames_until_done(&mut door, &settings);
    assert_eq!((door.state(), door.angle(), door.action()), (ThingState::Open, 110.0, Some(Action::Close)));
    assert!(frames.abs_diff(360) <= 1, "{frames} frames");
}

/// A container waits the install's 0.55 s and swings half as fast
/// again: in its new state 2.22 s after the key, as recorded.
#[test]
fn a_container_waits_and_swings_faster() {
    let settings = settings();
    let mut crate_ = Thing::container(Some(Interaction::ContainerHingeUpOpen), Some(Interaction::ContainerHingeUpClose), 0.0, -30.0);
    assert_eq!(crate_.action(), Some(Action::Search));
    assert_eq!(crate_.work(true, [0.0; 3], &settings), Some(Interaction::ContainerHingeUpOpen));
    crate_.step(DT, &settings);
    // Recorded: open 266 frames after the key at 120 a second.
    let frames = frames_until_done(&mut crate_, &settings);
    assert!(frames.abs_diff(266) <= 1, "{frames} frames");
    assert_eq!((crate_.state(), crate_.angle()), (ThingState::Open, -30.0));
    assert_eq!(crate_.gesture([0.0; 3]), Some(Interaction::ContainerHingeUpClose));
    // Told to shut, as its screen does: it stays open through the wait
    // and the swing, and is shut 266 frames later.
    crate_.tell(false, &settings);
    crate_.step(DT, &settings);
    let shut = (1..2000usize).find(|_| {
        crate_.step(DT, &settings);
        crate_.state() == ThingState::Shut
    });
    assert!(shut.unwrap().abs_diff(266) <= 1, "{shut:?} frames");
    // A drawer comes out and goes back in by the same curve.
    let mut drawer = Thing::container(None, None, 0.0, 0.0);
    drawer.work(true, [0.0; 3], &settings);
    for _ in 0..150 {
        drawer.step(DT, &settings);
    }
    assert!(drawer.out() > 0.5 && drawer.out() < 1.0, "{}", drawer.out());
}

/// A container shut by its screen while the character asks it to shut
/// as well: the second order is refused and changes nothing; the
/// container is shut when the first is done (recorded: a drawer and a
/// dead Scav, both shut 2.22 s after their screens).
#[test]
fn a_second_order_to_shut_changes_nothing() {
    let settings = settings();
    let mut drawer = Thing::container(Some(Interaction::ContainerOpenPull), Some(Interaction::DoorPushForward), 0.0, 0.0);
    drawer.work(true, [0.0; 3], &settings);
    drawer.step(DT, &settings);
    frames_until_done(&mut drawer, &settings);
    // Its screen shuts: told to shut, with no hand.
    drawer.tell(false, &settings);
    drawer.step(DT, &settings);
    for _ in 0..83 {
        drawer.step(DT, &settings);
    }
    // Still open, and offered to be shut: the key is pressed on it. It
    // is being worked from then on, and the order is refused.
    assert_eq!(drawer.action(), Some(Action::Close));
    assert_eq!(drawer.work(false, [0.0; 3], &settings), Some(Interaction::DoorPushForward));
    assert_eq!(drawer.state(), ThingState::Interacting);
    let shut = (1..2000usize).find(|_| {
        drawer.step(DT, &settings);
        drawer.state() == ThingState::Shut
    });
    assert!((83 + shut.unwrap()).abs_diff(266) <= 1, "{shut:?} frames");
    assert!(!drawer.busy());
}

/// The door's gesture by where the character stands: shut, pushed from
/// the side it swings away from, pulled from the other with the hinge
/// to one hand or the other; open, pushed shut or pulled back.
#[test]
fn a_door_s_gesture_goes_by_the_side() {
    let settings = settings();
    // It opens towards -z: from +z it swings away.
    let mut door = door();
    assert_eq!(door.door_gesture([0.5, 0.0, 1.0]), 27);
    assert_eq!(door.door_gesture([0.5, 0.0, -1.0]), 24);
    // The same door hinged at its other end, pointing along -x.
    let mirrored = Thing::door(Hinge { at: [1.0, 0.0, 0.0], axis: [0.0, -1.0, 0.0], forward: [-1.0, 0.0, 0.0] }, 0.0, 110.0);
    assert_eq!(mirrored.door_gesture([0.5, 0.0, 1.0]), 27);
    assert_eq!(mirrored.door_gesture([0.5, 0.0, -1.0]), 26);
    // Open, it points along -z from its hinge (+x turned 110 degrees
    // about straight up); the shut line it will swing back to is +x.
    door.work(true, [0.5, 0.0, 1.0], &settings);
    door.step(DT, &settings);
    frames_until_done(&mut door, &settings);
    assert_eq!(door.state(), ThingState::Open);
    // From the side it will swing away from (the far side of the open
    // door from the wall's line): pushed shut. From the side it will
    // swing towards: pulled back.
    let open_direction = turn([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 110.0);
    let beyond = [open_direction[0] * 0.5 - 0.8, 0.0, open_direction[2] * 0.5];
    let between = [open_direction[0] * 0.5 + 0.8, 0.0, open_direction[2] * 0.5];
    let (a, b) = (door.door_gesture(beyond), door.door_gesture(between));
    assert!([a, b].contains(&28) && [a, b].iter().any(|g| *g == 23 || *g == 25), "{a} {b}");
    // Being worked, its number is the game's 28 (recorded so).
    door.work(false, [0.0; 3], &settings);
    assert_eq!(door.door_gesture([0.5, 0.0, 1.0]), 28);
}

#[test]
fn the_shortest_turn() {
    assert_eq!(delta_angle(0.0, 110.0), 110.0);
    assert_eq!(delta_angle(110.0, 0.0), -110.0);
    assert_eq!(delta_angle(0.0, -30.0), -30.0);
    assert_eq!(delta_angle(350.0, 10.0), 20.0);
}

/// Steps the thing until it has sounded something, and says what and
/// on which frame (the key's own frame has been stepped before).
fn frames_until_sound(thing: &mut Thing, settings: &ThingSettings) -> (usize, Vec<ThingSound>) {
    for frame in 1..2000 {
        thing.step(DT, settings);
        let sounds = thing.take_sounds();
        if !sounds.is_empty() {
            return (frame, sounds);
        }
    }
    panic!("nothing sounded")
}

/// A door's opening sounds the open clip and the squeak together on
/// the first frame of its swing, half a second after the key; its
/// shutting the squeak then, and the shut clip the frame the curve's
/// time passes its end (no shift): the frame before it stands still.
#[test]
fn a_door_sounds_its_opening_and_its_shutting() {
    let settings = settings();
    let mut door = door();
    door.work(true, [0.5, 0.0, 1.0], &settings);
    door.step(DT, &settings);
    assert!(door.take_sounds().is_empty(), "nothing on the key's frame");
    let (frame, sounds) = frames_until_sound(&mut door, &settings);
    assert_eq!(sounds, vec![ThingSound::Swing { opens: true, squeaks: true }]);
    assert_eq!(frame, (DOOR_WAITS / DT).round() as usize, "the first frame at or past the wait");
    let done = frames_until_done(&mut door, &settings);
    assert!(door.take_sounds().is_empty(), "an opening sounds nothing more");
    assert_eq!(door.state(), ThingState::Open);
    // Shut it: the squeak at the swing's start, the shut clip at its end.
    door.work(false, [0.5, 0.0, 1.0], &settings);
    door.step(DT, &settings);
    let (frame, sounds) = frames_until_sound(&mut door, &settings);
    assert_eq!(sounds, vec![ThingSound::Swing { opens: false, squeaks: true }]);
    assert_eq!(frame, (DOOR_WAITS / DT).round() as usize);
    let (frame, sounds) = frames_until_sound(&mut door, &settings);
    assert_eq!(sounds, vec![ThingSound::Shut]);
    let swing_frames = (2.5 / DT).round() as usize;
    assert!((swing_frames - 1..=swing_frames).contains(&frame), "the frame the time passes 2.5 s, counted from the swing's own first: {frame}");
    frames_until_done(&mut door, &settings);
    assert_eq!(door.state(), ThingState::Shut);
    assert!(door.take_sounds().is_empty());
    let _ = done;
}

/// A shift moves the shut clip earlier by that many seconds of the
/// curve: a door's by the seconds themselves, a container's by two
/// thirds of them (it runs the curve at 1.5).
#[test]
fn the_shut_shift() {
    let settings = settings();
    let mut door = door();
    door.shut_shift = 1.0;
    door.at_rest(true);
    door.work(false, [0.5, 0.0, 1.0], &settings);
    door.step(DT, &settings);
    let (swing, _) = frames_until_sound(&mut door, &settings);
    let (frame, sounds) = frames_until_sound(&mut door, &settings);
    assert_eq!(sounds, vec![ThingSound::Shut]);
    let expected = (DOOR_WAITS / DT).round() as usize + (1.5 / DT).round() as usize;
    assert!((expected - 1..=expected).contains(&(swing + frame)), "{}", swing + frame);
    let mut lid = Thing::container(None, None, 0.0, -30.0);
    lid.shut_shift = 1.0;
    lid.at_rest(true);
    lid.tell(false, &settings);
    lid.step(DT, &settings);
    // The wait runs out and the swing's first frame runs at once.
    let (frame, sounds) = frames_until_sound(&mut lid, &settings);
    assert_eq!(sounds, vec![ThingSound::Swing { opens: false, squeaks: true }]);
    assert_eq!(frame, (0.55 / DT).round() as usize);
    let (frame, sounds) = frames_until_sound(&mut lid, &settings);
    assert_eq!(sounds, vec![ThingSound::Shut]);
    // (2.5 - 1) / 1.5 = 1 s of the swing, the frame the time passes it.
    assert!(((1.0 / DT).round() as usize - 1..=(1.0 / DT).round() as usize).contains(&frame), "{frame}");
}

/// A container's opening sounds on the frame its wait runs out, which
/// is its swing's first; its shutting without a shift sounds the shut
/// clip as the time passes the end, 1.67 s in.
#[test]
fn a_container_sounds_as_its_wait_runs_out() {
    let settings = settings();
    let mut lid = Thing::container(None, None, 0.0, -30.0);
    lid.work(true, [0.0; 3], &settings);
    lid.step(DT, &settings);
    let (frame, sounds) = frames_until_sound(&mut lid, &settings);
    assert_eq!(sounds, vec![ThingSound::Swing { opens: true, squeaks: true }]);
    assert_eq!(frame, (0.55 / DT).round() as usize);
    frames_until_done(&mut lid, &settings);
    assert!(lid.take_sounds().is_empty());
    lid.tell(false, &settings);
    lid.step(DT, &settings);
    let (_, sounds) = frames_until_sound(&mut lid, &settings);
    assert_eq!(sounds, vec![ThingSound::Swing { opens: false, squeaks: true }]);
    let (frame, sounds) = frames_until_sound(&mut lid, &settings);
    assert_eq!(sounds, vec![ThingSound::Shut]);
    let expected = (2.5 / 1.5 / DT).round() as usize;
    assert!((expected - 1..=expected).contains(&frame), "{frame}");
    frames_until_done(&mut lid, &settings);
    assert_eq!(lid.state(), ThingState::Shut);
}

/// A kick that opens the door sounds its breach clip and the swing's
/// squeak (no open clip: the swing is a breach, not an opening) on the
/// frame it lands; one that fails sounds the hit clip.
#[test]
fn a_kick_sounds_its_breach_or_its_hit() {
    let settings = settings();
    let mut kicked = door();
    kicked.start_breach([0.5, 0.0, -1.0]);
    kicked.kick_open();
    assert_eq!(kicked.take_sounds(), vec![ThingSound::Breach, ThingSound::Swing { opens: false, squeaks: true }]);
    frames_until_done(&mut kicked, &settings);
    assert!(kicked.take_sounds().is_empty(), "a kick's swing sounds nothing more");
    let mut held = door();
    held.lock(Some("key".to_owned()));
    held.start_breach([0.5, 0.0, 1.0]);
    held.fail_breach();
    assert_eq!(held.take_sounds(), vec![ThingSound::Hit]);
    // A locked door thrown open squeaks not: its fallback is locked.
    let mut locked = door();
    locked.lock(Some("key".to_owned()));
    locked.start_breach([0.5, 0.0, -1.0]);
    locked.kick_open();
    assert_eq!(locked.take_sounds(), vec![ThingSound::Breach, ThingSound::Swing { opens: false, squeaks: false }]);
}
