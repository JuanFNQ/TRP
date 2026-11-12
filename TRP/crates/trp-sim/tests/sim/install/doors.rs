//! Kicking a door and the door's menu.

use super::*;

/// A door kicked (`docs/spec/interactions.md`, "A kick"), set beside the
/// two kicks recorded by hand (`local\traces\byhand3`): a
/// shut door that puts the character at its place and is thrown open, and
/// a locked one that puts it nowhere and holds. The figures in the
/// comments are the recordings'.
#[test]
fn a_door_is_kicked() {
    use trp_sim::held::things::{Hinge, Thing, ThingSettings, ThingState};
    use trp_sim::Acted;
    let Some(mut m) = movement(0.0) else { return };
    let hard = trp_data::setup::hard_settings::HardSettings::load(&crate::fixture::install().ok_or(()).unwrap()).expect("settings object");
    let settings = ThingSettings::new(&hard).expect("the things' settings");
    // The recorded character had Strength 40, which sets how fast a body
    // may still be moved once it stands again.
    m.settings.set_strength(40.0);
    const FRAME: f32 = 1.0 / 120.0;
    let still = Input { hands_idle: true, ..Input::default() };

    // What a kick did, frame by frame: the movement states entered, when
    // it landed, when the door changed, and where the character was.
    struct Seen {
        states: Vec<(State, f32)>,
        landed: Option<(bool, f32)>,
        door: Vec<(ThingState, f32)>,
        path: Vec<([f32; 3], [f32; 2], f32)>,
    }
    let run = |m: &mut Movement, door: &mut Thing, seconds: f32| -> Seen {
        let mut seen = Seen { states: Vec::new(), landed: None, door: Vec::new(), path: Vec::new() };
        let (mut state, mut stands) = (m.state, door.state());
        for frame in 0..(seconds / FRAME).round() as usize {
            m.step(FRAME, &still, &Flat);
            let at = frame as f32 * FRAME;
            for acted in m.take_acted() {
                if let Acted::Kicked { opens } = acted {
                    seen.landed = Some((opens, at));
                    if opens { door.kick_open() } else { door.fail_breach() }
                }
            }
            door.step(FRAME, &settings);
            if m.state != state {
                state = m.state;
                seen.states.push((state, at));
            }
            if door.state() != stands {
                stands = door.state();
                seen.door.push((stands, at));
            }
            seen.path.push((m.position, [m.yaw, m.pitch], m.body_yaw));
        }
        seen
    };

    // The shut door of the second recording: its hinge 1.383 m ahead to
    // the right of the character and 0.308 m further on, its leaf running
    // back along -z, opening away to +x; the character looks 9.8 degrees
    // down and 2.4 off square.
    run(&mut m, &mut Thing::container(None, None, 0.0, 0.0), 1.0);
    (m.yaw, m.pitch, m.body_yaw) = (92.35577, 9.800021, 93.2261353);
    let hinge = Hinge { at: [1.3834, 0.0, 0.308044434], axis: [0.0, 1.0, 0.0], forward: [0.0, 0.0, -1.0] };
    let mut door = Thing::door(hinge, 0.0, -124.44);
    let from = m.position;
    let breach = door.start_breach(from).expect("a door");
    println!("the kick is made from {:?}, aimed at {:?}; the door puts the character there {}, it will open {}", breach.position, breach.view_target, breach.snaps, breach.opens);
    assert!(breach.snaps && breach.opens);
    // Recorded: the character ends 1.3 m square off the leaf, 0.5 m along it.
    assert!((breach.position[0] - from[0] - 0.08345).abs() < 2e-4 && (breach.position[2] - from[2] + 0.19211).abs() < 2e-4, "{:?}", breach.position);
    assert_eq!(door.state(), ThingState::Interacting);
    assert!(m.kick(breach.opens, Some((breach.position, breach.view_target))));
    let seen = run(&mut m, &mut door, 3.0);
    println!("  states {:?}\n  landed {:?}\n  the door {:?}", seen.states, seen.landed, seen.door);
    let at = |state: State| seen.states.iter().find(|entry| entry.0 == state).map(|entry| entry.1).unwrap_or(f32::NAN);
    let (approach, kick, idle) = (at(State::Approach), at(State::Breach), at(State::Idle));
    // The approach begins on the key's frame and lasts 0.261 s; the kick's
    // state 1.202 s.
    assert_eq!(approach, 0.0);
    assert!((kick - approach - 0.261).abs() < 0.012, "the approach lasted {}", kick - approach);
    assert!((idle - kick - 1.202).abs() < 0.012, "the kick's state lasted {}", idle - kick);
    // It lands 0.603 s into its state, and the door is open 0.506 s later.
    let (opens, landed) = seen.landed.expect("the kick lands");
    assert!(opens && (landed - kick - 0.603).abs() < 0.012, "landed {} into the state", landed - kick);
    assert_eq!(seen.door.len(), 1);
    assert!(seen.door[0].0 == ThingState::Open && (seen.door[0].1 - landed - 0.506).abs() < 0.012, "{:?}", seen.door);
    assert!((door.angle() + 124.44).abs() < 1e-3 && door.broken());
    // The approach: where the character was six frames in and when it was
    // over, its view levelled and square to the door, its body with it.
    let frame = |after: f32| &seen.path[((approach + after) / FRAME).round() as usize];
    let (six, over) = (frame(6.0 * FRAME), frame(0.2));
    println!("  six frames into the approach at {:?} (recorded 0.03794, -0.08734); over at {:?}, looking {:?}, facing {}", six.0, over.0, over.1, over.2);
    assert!((six.0[0] - from[0] - 0.03794).abs() < 0.004 && (six.0[2] - from[2] + 0.08734).abs() < 0.008, "{:?}", six.0);
    assert!((over.0[0] - from[0] - 0.08345).abs() < 5e-4 && (over.0[2] - from[2] + 0.19211).abs() < 5e-4, "{:?}", over.0);
    assert!((over.1[0] - 89.99328).abs() < 0.05 && over.1[1].abs() < 1e-3 && (over.2 - 89.99328).abs() < 0.05, "{:?} {}", over.1, over.2);
    // The kick carries the character on through the doorway: 0.5905 m by
    // the time its state is left, 0.649 m when it has come to rest.
    let (left, rest) = (frame(idle - approach), seen.path.last().unwrap());
    println!("  moved {:.4} m when the state is left (recorded 0.5905), {:.4} m at rest (recorded 0.6489)", left.0[0] - over.0[0], rest.0[0] - over.0[0]);
    assert!((left.0[0] - over.0[0] - 0.5905).abs() < 0.01 && (rest.0[0] - over.0[0] - 0.6489).abs() < 0.01);
    assert!((rest.0[2] - over.0[2]).abs() < 0.005);
    assert!(!m.kicking());

    // The locked door of the first recording: it puts the character
    // nowhere, the kick fails, and the door is locked again 0.9 s into the
    // kick's state. The character steps in a quarter of a metre and back.
    let mut m = movement(0.0).unwrap();
    m.settings.set_strength(40.0);
    run(&mut m, &mut Thing::container(None, None, 0.0, 0.0), 1.0);
    (m.yaw, m.pitch, m.body_yaw) = (-13.4458761, 11.8589754, 0.30562);
    // Its leaf runs across ahead of the character and opens towards it.
    let hinge = Hinge { at: [0.322971344, 0.0, 1.26254272], axis: [0.0, 1.0, 0.0], forward: [-1.0, 0.0, 0.0] };
    let mut door = Thing::door(hinge, 0.0, -110.0);
    door.lock(None);
    let from = m.position;
    let breach = door.start_breach(from).expect("a door");
    println!("a locked door: the kick is made from {:?}; the door puts the character there {}, it will open {}", breach.position, breach.snaps, breach.opens);
    assert!(!breach.snaps && !breach.opens);
    assert!(m.kick(breach.opens, breach.snaps.then_some((breach.position, breach.view_target))));
    let seen = run(&mut m, &mut door, 3.0);
    println!("  states {:?}\n  landed {:?}\n  the door {:?}", seen.states, seen.landed, seen.door);
    let at = |state: State| seen.states.iter().find(|entry| entry.0 == state).map(|entry| entry.1).unwrap_or(f32::NAN);
    let (approach, kick, idle) = (at(State::Approach), at(State::Breach), at(State::Idle));
    assert!((kick - approach - 0.262).abs() < 0.012 && (idle - kick - 1.204).abs() < 0.012, "{} {}", kick - approach, idle - kick);
    let (opens, landed) = seen.landed.expect("the kick lands");
    assert!(!opens && (landed - kick - 0.603).abs() < 0.012);
    assert!(seen.door.len() == 1 && seen.door[0].0 == ThingState::Locked, "{:?}", seen.door);
    assert!((seen.door[0].1 - kick - 0.904).abs() < 0.012, "locked again {} into the state", seen.door[0].1 - kick);
    // The view and the body are left as they were through the approach.
    let frame = |at: f32| &seen.path[(at / FRAME).round() as usize];
    let began = frame(kick);
    assert!(began.0 == from && (began.1[0] + 13.4458761).abs() < 1e-3 && (began.1[1] - 11.8589754).abs() < 1e-3);
    // In 0.249 m, and back out to 0.2305 m behind where it stood as the
    // state is left, 0.324 m at rest; along the way the body faces.
    let furthest = seen.path.iter().map(|entry| entry.0[2]).fold(f32::MIN, f32::max);
    let (left, rest) = (frame(idle), seen.path.last().unwrap());
    println!("  in {:.4} m (recorded 0.2492), {:.4} m when the state is left (recorded -0.2305), {:.4} m at rest (recorded -0.3242)", furthest - from[2], left.0[2] - from[2], rest.0[2] - from[2]);
    assert!((furthest - from[2] - 0.2492).abs() < 0.012 && (left.0[2] - from[2] + 0.2305).abs() < 0.012 && (rest.0[2] - from[2] + 0.3242).abs() < 0.012);
    // The legs pay for a kick what they pay for a jump.
    println!("  the legs after the kick: {:.2} of {:.2}", m.stamina.legs.current, m.stamina.legs.capacity);
    assert!(m.stamina.legs.current < m.stamina.legs.capacity);
}

/// The menu for a door and its wheel (`AvailableInteractionState`).
#[test]
fn a_door_s_menu_has_the_kick() {
    use trp_sim::held::things::{Action, Hinge, Thing};
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let threshold = hard.float("DOOR_BREACH_THRESHOLD").expect("the kick's angle");
    println!("a kick is offered while the way to the door is under {threshold} along its leaf");
    // A door in a wall along x, hinged at the origin, opening towards -z.
    let mut door = Thing::door(Hinge { at: [0.0; 3], axis: [0.0, 1.0, 0.0], forward: [1.0, 0.0, 0.0] }, 0.0, 110.0);
    let square = [0.5, 0.0, 1.2];
    let mut menu = door.menu(square, false, false, threshold).expect("a menu");
    let names: Vec<&str> = menu.lines.iter().map(|(action, _)| action.name()).collect();
    assert_eq!(names, ["OpenDoor", "Breach", "Bang & clear", "Flash & clear", "Move in"]);
    assert_eq!(menu.lines.iter().map(|(_, disabled)| *disabled).collect::<Vec<_>>(), [false, false, true, true, true]);
    // The first line is chosen; the wheel goes round the two that can be.
    assert_eq!(menu.chosen(), Some(Action::Open));
    menu.scroll(true);
    assert_eq!(menu.chosen(), Some(Action::Breach));
    menu.scroll(true);
    assert_eq!(menu.chosen(), Some(Action::Open));
    menu.scroll(false);
    assert_eq!(menu.chosen(), Some(Action::Breach));
    // From far along the wall the kick is not to be chosen, nor lying.
    let along = [3.0, 0.0, 0.4];
    assert_eq!(door.menu(along, false, false, threshold).unwrap().lines[1], (Action::Breach, true));
    assert_eq!(door.menu(square, false, true, threshold).unwrap().lines[1], (Action::Breach, true));
    // It opens from the side it swings away from, and holds from the other.
    assert!(door.breach_succeeds(square) && !door.breach_succeeds([0.5, 0.0, -1.2]));
    // Locked: the kick alone, and the key's line first for who has it.
    door.lock(Some("a key".to_owned()));
    let locked = door.menu(square, false, false, threshold).unwrap();
    assert_eq!((locked.lines[0], locked.chosen()), ((Action::Breach, false), Some(Action::Breach)));
    let keyed = door.menu(square, true, false, threshold).unwrap();
    assert_eq!((keyed.lines.len(), keyed.chosen()), (5, Some(Action::Unlock)));
    // Locked and at a bad angle there is nothing to choose.
    let mut none = door.menu(along, false, false, threshold).unwrap();
    assert_eq!(none.chosen(), None);
    none.scroll(true);
    assert_eq!(none.chosen(), None);
    // Unlocked it is a shut door; open, there is only shutting it.
    door.unlock();
    assert_eq!(door.menu(square, true, false, threshold).unwrap().chosen(), Some(Action::Open));
    door.at_rest(true);
    assert_eq!(door.menu(square, false, false, threshold).unwrap().lines, [(Action::Close, false)]);
    // Being worked, nothing is offered.
    door.hold();
    assert!(door.menu(square, false, false, threshold).is_none());
}
