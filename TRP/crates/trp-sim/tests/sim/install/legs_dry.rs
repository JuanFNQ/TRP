//! Sprints that run the legs dry.

use super::*;

/// Stamina through the movement code, on the install's figures (stamina
/// spec): a sprint drains the legs four a second and ends by itself when
/// they are spent; spent legs neither sprint nor jump; a jump costs
/// fourteen; and the sights held up drain the arms by the weapon's
/// ergonomic weight.
#[test]
fn a_sprint_runs_the_legs_dry_and_ends() {
    let Some(mut m) = movement(0.0) else { return };
    let full = m.stamina.legs.capacity;
    run(&mut m, 0.5, Input::default());
    // Forward first, then the sprint key, held.
    let held = Input { sprint: true, ..walk([0.0, 1.0]) };
    run(&mut m, 0.5, walk([0.0, 1.0]));
    let mut time = 0.0;
    let (mut began, mut ended, mut at_five) = (None, None, 0.0);
    while time < 40.0 && ended.is_none() {
        m.step(DT, &held, &Flat);
        time += DT;
        if m.sprinting && began.is_none() {
            began = Some(time);
        }
        if began.is_some_and(|began| (time - began - 5.0).abs() < DT / 2.0) {
            at_five = m.stamina.legs.current;
        }
        if began.is_some() && !m.sprinting {
            ended = Some(time);
        }
    }
    let (began, ended) = (began.expect("the sprint begins"), ended.expect("the sprint ends by itself"));
    println!(
        "legs of {full}: a sprint has them at {at_five:.1} after 5 s and ends by itself after {:.1} s with {:.2} left, in {:?}",
        ended - began,
        m.stamina.legs.current,
        m.state
    );
    assert!((full - at_five - 20.0).abs() < 0.5, "four a second: {at_five}");
    assert!((ended - began - full / 4.0).abs() < 0.5, "{}", ended - began);
    assert!(m.stamina.legs.current < 0.1);
    // Spent legs do not jump, and the sprint key does nothing.
    run(&mut m, 0.2, walk([0.0, 1.0]));
    m.step(DT, &Input { jump: true, ..walk([0.0, 1.0]) }, &Flat);
    run(&mut m, 0.3, walk([0.0, 1.0]));
    assert_ne!(m.state, State::Jump, "no jump on spent legs");
    run(&mut m, 0.5, held);
    assert!(!m.sprinting, "no sprint on spent legs");
    // Rested, a jump costs its fourteen.
    run(&mut m, 30.0, Input::default());
    assert_eq!(m.stamina.legs.current, full);
    m.step(DT, &Input { jump: true, ..Input::default() }, &Flat);
    run(&mut m, 0.3, Input::default());
    println!("a jump leaves {:.1} in the legs", m.stamina.legs.current);
    assert!((full - m.stamina.legs.current - 14.0).abs() < 0.01, "{}", m.stamina.legs.current);
    // The sights up cost the arms by the weapon's ergonomic weight.
    m.stamina.aim_mass = 2.88;
    run(&mut m, 5.0, Input { aiming: true, ..Input::default() });
    let spent = m.stamina.arms.capacity - m.stamina.arms.current;
    println!("five seconds of aiming a weapon of 2.88 kg ergonomic weight cost the arms {spent:.2}");
    assert!((spent - 2.88f32.sqrt() * 1.1 * 5.0).abs() < 0.1, "{spent}");
}

/// The recorder's scenario for spent legs (`61-legs-dry`, recorder 0.4.9),
/// played through the simulation as the recorder plays it, its clock held
/// at each gate by the simulation's own legs. Whatever the character's
/// skills and thirst: the jumps take the legs under 15, a jump asked then
/// is refused, the sprint begins on legs of 16 to 23 and runs them dry
/// inside the recording ground, and the jump, the stance and the prone
/// key are then used on spent legs. Nothing here is compared with the
/// game: it is the scenario that is being tried, before it is recorded.
#[test]
fn the_legs_dry_scenario_runs_the_legs_dry() {
    use trp_sim::recordings::script::{Clock, Scenario};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../hosts/spt-oracle/scenarios/61-legs-dry.json");
    // The recorder's scenarios are not in the public copy (`docs/manual.md`, "Publishing").
    if !path.exists() {
        eprintln!("skipped: no recorder's scenarios here");
        return;
    }
    let scenario = Scenario::load(&path).expect("the scenario");
    assert_eq!(scenario.gates.len(), 3);
    let frame = 1.0 / 120.0;
    for (name, endurance, strength, energy, hydration) in [
        ("untrained, fed and watered", 0.0, 0.0, 1.0, 1.0),
        ("Endurance 40 and Strength 40", 40.0, 40.0, 1.0, 1.0),
        ("the same, 67% fed and 70% watered", 40.0, 40.0, 0.67, 0.70),
    ] {
        let Some(mut m) = movement(0.0) else { return };
        m.stamina.settings.set_skills(endurance, strength);
        (m.stamina.settings.energy, m.stamina.settings.hydration) = (energy, hydration);
        m.stamina.resettle();
        m.stamina.legs.current = m.stamina.legs.capacity;
        let full = m.stamina.legs.capacity;

        let mut clock = Clock::default();
        let (mut before, mut real, mut passed) = (0.0f32, 0.0f32, 0usize);
        // What is looked for on the way.
        let (mut jumps, mut was) = (0, m.state);
        let (mut opened, mut shut_at) = (Vec::new(), None::<f32>);
        let (mut sprint_began, mut sprint_ended) = (None, None);
        let (mut furthest, mut least) = (0.0f32, f32::MAX);
        let mut jumped_after = [false; 2];
        let mut pose = Vec::new();
        let mut lying = Vec::new();
        while !clock.done(&scenario) && real < 200.0 {
            let legs = m.stamina.legs.current;
            let (time, pressing) = clock.frame(&scenario, frame, legs);
            // A gate is shut while the clock stands at it.
            let at_gate = scenario.gates.get(passed).filter(|gate| gate.at == time);
            match (at_gate, shut_at) {
                (Some(_), None) => shut_at = Some(real),
                (None, Some(since)) => {
                    opened.push((real - since, legs));
                    shut_at = None;
                    passed += 1;
                }
                _ => {}
            }
            // The movement keys reach the game a frame late.
            let key = |name: &str| scenario.held(name, before) as i32 as f32;
            let mut input = Input { move_dir: [key("d") - key("a"), key("w") - key("s")], sprint: scenario.held("shift", time), ..Input::default() };
            let gate_key = pressing.and_then(|gate| gate.press.as_ref()).map(|(key, _)| key.as_str());
            for key in scenario.pressed(before, time).chain(gate_key) {
                match key {
                    "jump" => input.jump = true,
                    "crouch" => input.crouch = true,
                    "prone" => input.prone = true,
                    other => panic!("the test does not press {other}"),
                }
            }
            m.step(frame, &input, &Flat);
            real += frame;
            if m.state == State::Jump && was != State::Jump {
                jumps += 1;
                jumped_after[0] |= (6.2..6.6).contains(&time);
                jumped_after[1] |= time >= 10.3;
            }
            was = m.state;
            if m.sprinting && sprint_began.is_none() {
                sprint_began = Some((time, real, legs, m.position[2]));
            }
            if sprint_began.is_some() && !m.sprinting && sprint_ended.is_none() {
                sprint_ended = Some((time, real, m.stamina.legs.current, m.position[2], scenario.held("shift", time)));
            }
            furthest = furthest.max(m.position[2]);
            least = least.min(m.position[2]);
            if time >= 10.6 {
                pose.push((time, m.smoothed_pose_level, m.stamina.legs.current));
                lying.push((time, m.state));
            }
            before = time;
        }
        let (sprint_at, sprint_real, sprint_legs, sprint_from) = sprint_began.expect("the sprint begins");
        let (ended_at, ended_real, ended_legs, ended_place, key_held) = sprint_ended.expect("the sprint ends");
        // The crouch key comes again a second after it sent the stance
        // down: how far down that got, and how far back up it is when the
        // prone key comes two seconds later.
        let stance_at = |at: f32| pose.iter().find(|(time, ..)| *time >= at).map_or(f32::NAN, |(_, level, _)| *level);
        let (down, up) = (stance_at(11.6), stance_at(13.6));
        let state_span = |from: f32, state: State| {
            let mut in_it = lying.iter().filter(|(time, s)| *time >= from && *s == state).map(|(time, _)| *time);
            let first = in_it.next();
            first.map_or(f32::NAN, |first| in_it.take_while(|time| *time < from + 4.0).last().unwrap_or(first) - first)
        };
        let legs_at = |at: f32| pose.iter().find(|(time, ..)| *time >= at).map_or(f32::NAN, |(_, _, legs)| *legs);
        println!(
            "{name} (legs of {full:.0}): {jumps} jumps, the gates shut for {:.1}, {:.1} and {:.1} s and open on legs of {:.1}, {:.1} and {:.2}; \
             the sprint begins at {sprint_at:.2} on legs of {sprint_legs:.1}, {:.1} m behind the spot, and ends at {ended_at:.2} ({:.1} s of it) on {ended_legs:.2}, {ended_place:.1} m ahead; \
             furthest {furthest:.1} m ahead, {:.1} m behind; {real:.0} s in all; the stance comes down to {down:.2} in a second and is back at {up:.2} two seconds on (legs {:.1} and {:.1}), \
             going down takes {:.2} s (legs {:.1}) and getting up {:.2} s (legs {:.1})",
            opened[0].0,
            opened[1].0,
            opened[2].0,
            opened[0].1,
            opened[1].1,
            opened[2].1,
            -sprint_from,
            ended_real - sprint_real,
            -least,
            legs_at(10.6),
            legs_at(11.6),
            state_span(13.6, State::ToProne),
            legs_at(13.6),
            state_span(15.3, State::FromProne),
            legs_at(15.3),
        );
        assert_eq!(opened.len(), 3, "{opened:?}");
        assert!(opened[0].1 < 15.0 && opened[0].0 < 59.0, "the jumps take the legs under 15: {opened:?}");
        assert!(!jumped_after[0], "no jump on legs under 15");
        assert!((16.0..17.0).contains(&opened[1].1) && opened[1].0 < 19.0, "{opened:?}");
        assert!((16.0..23.0).contains(&sprint_legs), "{sprint_legs}");
        assert!(ended_legs < 0.1 && key_held, "the sprint ends by itself on spent legs: {ended_legs}");
        assert!(opened[2].1 < 0.05 && opened[2].0 < 11.0, "{opened:?}");
        assert!(!jumped_after[1], "no jump on spent legs");
        assert!(furthest < 24.0 && least > -9.0, "inside the recording ground: {furthest} ahead, {least} behind");
        assert!(legs_at(10.6) < 15.0 && legs_at(11.6) < 15.0 && legs_at(13.6) < 15.0, "the stance and the prone key are used on exhausted legs");
        assert!(down < 0.5 && up > down, "the stance went down and came back: {down}, {up}");
    }
}
