//! The walk bob rewound, the lame walk's pitch and the first step's tempo.

use super::*;

/// The bob's clocks (weapon-handling spec, "Walking"): a new rig starts
/// each curve at its time shift (the camera's pitch half a cycle on), and
/// a walk at once plays on from there; a change of the movement state into
/// one other than the walk (a sprint begun standing) rewinds the current
/// preset's curves and the load's to 0, not to the time shift, and the
/// walk after it starts from 0; leaving the walk rewinds them unbidden;
/// the other preset's are left where they were.
#[test]
fn a_change_of_state_but_into_the_walk_rewinds_the_bob() {
    let (Some((mut drawn, _)), Some((mut stopped, _))) = (rig(), rig()) else { return };
    let shifts: Vec<f32> = drawn.settings.data.walk_normal.iter().map(|e| e.time_shift).collect();
    let load_shifts: Vec<f32> = drawn.settings.data.walk_overweight.iter().map(|e| e.time_shift).collect();
    assert_eq!(drawn.walk_clocks().0, shifts.as_slice());
    assert_eq!(drawn.walk_clocks().1, load_shifts.as_slice());
    assert!(shifts.contains(&0.5), "the camera's pitch starts half a cycle on: {shifts:?}");
    let (step, tempo) = (drawn.settings.fixed_timestep, drawn.step_frequency);
    let walk = HandsInput { speed: 1.0, walking: true, idle: false, ..still() };
    let at = |hands: &Hands, from: &[f32]| hands.walk_clocks().0.iter().zip(from).all(|(clock, start)| (clock - (start + step * tempo)).abs() < 1e-6);
    // Walked at once: on from the time shifts.
    drawn.advance(step, &walk);
    assert!(at(&drawn, &shifts), "{:?}", drawn.walk_clocks().0);
    // A sprint begun standing: a change of state, though not out of the walk.
    stopped.advance(step, &HandsInput { walk_stop: true, sprinting: true, idle: false, ..still() });
    let (now, load) = stopped.walk_clocks();
    assert!(now.iter().chain(load).all(|c| *c == 0.0), "{now:?} {load:?}");
    stopped.advance(step, &walk);
    assert!(at(&stopped, &vec![0.0; shifts.len()]), "{:?}", stopped.walk_clocks().0);
    // Leaving the walk, with no word of a change.
    drawn.advance(step, &still());
    assert!(drawn.walk_clocks().0.iter().all(|c| *c == 0.0));
    // Only the preset in play: the lame one walked a step, then a stop with
    // the normal one in play leaves it.
    let Some((mut hands, _)) = rig() else { return };
    let lame = trp_sim::condition::health::Conditions { left_leg: true, ..Default::default() };
    let lame_shifts: Vec<f32> = hands.settings.data.walk_lame.iter().map(|e| e.time_shift).collect();
    hands.set_health(lame);
    hands.advance(step, &walk);
    hands.set_health(trp_sim::condition::health::Conditions::default());
    hands.advance(step, &HandsInput { walk_stop: true, ..still() });
    assert!(hands.walk_clocks().0.iter().all(|c| *c == 0.0));
    hands.set_health(lame);
    assert!(at(&hands, &lame_shifts), "the lame curves rewound too: {:?}", hands.walk_clocks().0);
}

/// Each curve of the bob goes on past its keys by its own wraps
/// (weapon-handling spec, "Walking"; `probe player-scripts WalkPreset`):
/// the lame walk's second, the hands' pitch (keys 0 to 0.25), runs back and
/// forth, at 0.30 as at 0.20 and back at 0 at half a cycle; every other
/// curve of the four presets and the breath's start over, or is flat and
/// held. And the hands play it so: a lame walk whose pitch curve repeated
/// instead is the same until the curve's last key and apart after it.
#[test]
fn the_lame_walk_s_pitch_runs_back_and_forth() {
    use trp_data::formats::curve::Wrap;
    let (Some((mut played, _)), Some((mut repeated, _))) = (rig(), rig()) else { return };
    let data = &played.settings.data;
    let pitch = &data.walk_lame[1];
    assert_eq!((pitch.before, pitch.after), (Wrap::Clamp, Wrap::PingPong));
    assert_eq!(pitch.curve.keys.last().map(|k| k.time), Some(0.25));
    assert!((pitch.at(0.30) - pitch.at(0.20)).abs() < 1e-6 && pitch.at(0.5).abs() < 1e-4, "{} {} {}", pitch.at(0.30), pitch.at(0.20), pitch.at(0.5));
    assert!((pitch.at(0.30) - pitch.curve.evaluate(0.05)).abs() > 0.01, "a repeat would be at 0.05's");
    let presets = [&data.walk_normal, &data.walk_lame, &data.walk_overweight, &data.walk_overweight_lame, &data.breath.curves];
    for (preset, entry) in presets.iter().enumerate().flat_map(|(p, list)| list.iter().enumerate().map(move |(i, e)| ((p, i), e))) {
        let flat = entry.curve.keys.windows(2).all(|k| k[0].value == k[1].value);
        if preset != (1, 1) {
            assert!(entry.after == Wrap::Repeat || flat, "{preset:?}: {:?}", entry.after);
        }
    }
    let lame = trp_sim::condition::health::Conditions { right_leg: true, ..Default::default() };
    repeated.settings.data.walk_lame[1].after = Wrap::Repeat;
    let step = played.settings.fixed_timestep;
    let walk = HandsInput { speed: 1.0, walking: true, idle: false, ..still() };
    for hands in [&mut played, &mut repeated] {
        hands.set_health(lame);
        hands.step_frequency = 1.0;
    }
    let pitch = |hands: &Hands| hands.hands_rotation.current[0];
    let mut apart_before = 0.0f32;
    while played.walk_clocks().0[1] + step < 0.25 {
        played.advance(step, &walk);
        repeated.advance(step, &walk);
        apart_before = apart_before.max((pitch(&played) - pitch(&repeated)).abs());
    }
    assert_eq!(apart_before, 0.0);
    let mut apart_after = 0.0f32;
    while played.walk_clocks().0[1] < 0.5 {
        played.advance(step, &walk);
        repeated.advance(step, &walk);
        apart_after = apart_after.max((pitch(&played) - pitch(&repeated)).abs());
    }
    assert!(apart_after > 0.1, "{apart_after}");
}

/// The movement tells the bob of every change of state but into the walk,
/// once (`Movement::take_walk_stop`): none standing or walking on, one on
/// stopping, one on a sprint and one on a jump begun standing. And the
/// character's first footstep comes long after a step that never was, so
/// it leaves the bob's tempo as it stood; the next retunes it to its gap
/// (`0.5 / clamp(gap, 0.7 - speed / 2, 1.2)`).
#[test]
fn the_movement_tells_the_bob_to_stop_and_the_first_step_leaves_its_tempo() {
    let Some((mut hands, mut movement)) = rig() else { return };
    let run = |movement: &mut Movement, hands: &mut Hands, input: Input, frames: usize, changes: &mut Vec<(State, State, bool)>, gaps: &mut Vec<[f32; 4]>| {
        for _ in 0..frames {
            let before = movement.state;
            movement.step(DT, &input, &Flat);
            let walk_stop = movement.take_walk_stop();
            if movement.state != before || walk_stop {
                changes.push((before, movement.state, walk_stop));
            }
            let footstep = movement.take_footstep();
            let tempo = hands.step_frequency;
            hands.advance(
                DT,
                &HandsInput {
                    speed: movement.smoothed_speed,
                    walking: movement.state == State::Move,
                    walk_stop,
                    sprinting: movement.state == State::Sprint,
                    idle: movement.state == State::Idle,
                    footstep,
                    ..still()
                },
            );
            if let Some(gap) = footstep {
                gaps.push([gap, tempo, hands.step_frequency, movement.smoothed_speed]);
            }
        }
    };
    let (mut changes, mut gaps) = (Vec::new(), Vec::new());
    let forward = Input { move_dir: [0.0, 1.0], ..Input::default() };
    run(&mut movement, &mut hands, Input::default(), 30, &mut changes, &mut gaps);
    assert!(changes.is_empty(), "standing: {changes:?}");
    run(&mut movement, &mut hands, forward, 150, &mut changes, &mut gaps);
    assert_eq!(changes, vec![(State::Idle, State::Move, false)], "walking off");
    // The first step: none before it.
    let [first, before, after, _] = gaps[0];
    assert!(first > 1.2 && after == before, "the first step's gap {first} retuned {before} to {after}");
    let [gap, _, after, speed] = gaps[1];
    assert!((0.2..1.2).contains(&gap) && (after - 0.5 / gap.clamp(0.7 - speed / 2.0, 1.2)).abs() < 1e-5, "{gap}: {after}");
    changes.clear();
    run(&mut movement, &mut hands, Input::default(), 90, &mut changes, &mut gaps);
    assert!(changes.first().is_some_and(|c| c.0 == State::Move && c.2), "stopping: {changes:?}");
    changes.clear();
    run(&mut movement, &mut hands, forward, 5, &mut changes, &mut gaps);
    run(&mut movement, &mut hands, Input { sprint: true, ..forward }, 60, &mut changes, &mut gaps);
    run(&mut movement, &mut hands, Input::default(), 120, &mut changes, &mut gaps);
    run(&mut movement, &mut hands, Input { jump: true, ..Input::default() }, 1, &mut changes, &mut gaps);
    run(&mut movement, &mut hands, Input::default(), 120, &mut changes, &mut gaps);
    println!("the changes of state: {changes:?}");
    for (from, to, told) in &changes {
        assert_eq!(*told, *to != State::Move, "{from:?} to {to:?}");
    }
    assert!(changes.iter().any(|c| c.1 == State::Sprint), "a sprint: {changes:?}");
    assert!(changes.iter().any(|c| c.0 == State::Idle && c.1 == State::Jump), "a jump begun standing: {changes:?}");
}
