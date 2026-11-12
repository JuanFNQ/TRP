//! Walking speed, sprint, jump, lean, inertia and landing against the install's tables.

use super::*;

/// Strength's lift and its inertia step (`PhysicalBase.UpdateWeightLimits`):
/// an untrained character's limits are the settings' own; at elite
/// Strength the same weight is lighter on every share (the overweight
/// limits 1.3 times as far, the inertia's 1.275 kg further).
#[test]
fn strength_raises_the_weight_limits() {
    let Some(mut m) = movement(40.0) else {
        eprintln!("no install: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let install = crate::fixture::install().expect("install");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let base = m.settings.clone();
    m.set_weight_limits(&hard, &db.config, trp_sim::base::settings::WeightLimits::BASE).expect("untrained limits");
    assert_eq!((m.settings.inertia, m.settings.overweight), (base.inertia, base.overweight), "untrained: nothing changes");
    let mut levels = trp_sim::condition::skills::SkillLevels::default();
    levels.set_level(trp_sim::condition::skills::Skill::Strength, 51);
    let limits = trp_sim::base::settings::WeightLimits::of(&levels.buffs());
    assert!(limits.strength_level == 51.0 && (limits.carry - 1.3).abs() < 1e-6, "{limits:?}");
    m.set_weight_limits(&hard, &db.config, limits).expect("elite limits");
    assert!(m.settings.inertia < base.inertia && m.settings.overweight < base.overweight, "{} {} against {} {}", m.settings.inertia, m.settings.overweight, base.inertia, base.overweight);
    assert!(m.stamina.settings.overweight <= base.stamina.overweight);
    // The weight's limits stay as the weight changes.
    m.set_weight(&hard, &db.config, 50.0).expect("reweighed");
    assert_eq!(m.settings.limits, limits);
}

/// The blend weights are computed directly instead of read from the stored
/// grid. On the grid's own points the two must agree.
#[test]
fn computed_blend_weights_match_the_stored_grid() {
    let Some(m) = movement(0.0) else { return };
    let table = &m.settings.table;
    fn visit(table: &trp_data::character::rootmotion::RootMotionTable, node: &Node, worst: &mut f32, count: &mut usize) {
        if let Some(mismatch) = table.grid_mismatch(node) {
            *worst = worst.max(mismatch);
            *count += 1;
        }
        if let Node::Tree { children, .. } = node {
            for child in children {
                visit(table, child, worst, count);
            }
        }
    }
    let (mut worst, mut count) = (0.0f32, 0usize);
    for state in [m.settings.move_state, m.settings.sprint_state] {
        visit(table, &table.states[&state].1, &mut worst, &mut count);
    }
    println!("freeform Cartesian trees checked: {count}, largest weight difference {worst:.6}");
    assert!(count >= 2);
    assert!(worst < 1e-3, "computed weights differ from the game's by {worst}");
}

#[test]
fn walking_reaches_the_table_speed_and_stops() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 3.0, walk([0.0, 1.0]));
    let speed = m.ground_speed();
    println!(
        "walk: {speed:.3} m/s at speed parameter {:.3} (wheel maximum {:.3}), state {:?}",
        m.smoothed_speed, m.settings.max_speed, m.state
    );
    assert_eq!(m.state, State::Move);
    assert!(m.velocity[2] > 0.0 && m.velocity[0].abs() < 1e-3, "{:?}", m.velocity);
    // Between the table's walk (2.50) and run (4.03) clips.
    assert!((2.5..3.3).contains(&speed), "{speed}");

    let travelled = m.position[2];
    run(&mut m, 1.5, Input::default());
    println!("stop: {:.3} m/s after 1.5 s, coasted {:.3} m, state {:?}", m.ground_speed(), m.position[2] - travelled, m.state);
    assert_eq!(m.state, State::Idle);
    assert!(m.ground_speed() < 0.01);
}

#[test]
fn directions_and_stance_change_the_speed() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 3.0, walk([1.0, 0.0]));
    println!("strafe right: velocity ({:.3}, {:.3})", m.velocity[0], m.velocity[2]);
    assert!(m.velocity[0] > 2.0 && m.velocity[2].abs() < 0.05);

    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 3.0, walk([0.0, -1.0]));
    println!("backward: velocity ({:.3}, {:.3})", m.velocity[0], m.velocity[2]);
    assert!(m.velocity[2] < -2.0);

    let Some(mut m) = movement(0.0) else { return };
    m.step(DT, &Input { pose_delta: -1.0, ..Input::default() }, &Flat);
    run(&mut m, 4.0, walk([0.0, 1.0]));
    println!("crouched: {:.3} m/s at stance {:.2}", m.ground_speed(), m.smoothed_pose_level);
    assert!(m.smoothed_pose_level < 0.01);
    // Between the table's crouch (1.38) and crouch-run (2.05) clips.
    assert!((1.3..2.1).contains(&m.ground_speed()), "{}", m.ground_speed());

    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.1, Input { speed_delta: -1.0, ..Input::default() });
    run(&mut m, 3.0, walk([0.0, 1.0]));
    println!("slowest wheel setting: {:.3} m/s", m.ground_speed());
    assert!((0.7..0.9).contains(&m.ground_speed()), "{}", m.ground_speed());
}

/// The dummies' settings made from the player's read
/// (`movement_settings_as_read`: a character carrying 40 kg at elite
/// Strength's limits, its settings then made for nothing carried) move a
/// character as settings read afresh for nothing carried do, frame for
/// frame: walking, sprinting, crouching, a jump and prone.
#[test]
fn movement_settings_made_from_anothers_read_move_as_a_fresh_read() {
    let Some(install) = crate::fixture::install() else {
        eprintln!("no install: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::setup::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let body = trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator");
    let mut read = trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 40.0).expect("movement settings");
    let mut levels = trp_sim::condition::skills::SkillLevels::default();
    levels.set_level(trp_sim::condition::skills::Skill::Strength, 51);
    read = read.relimited(&hard, &db.config, 40.0, trp_sim::base::settings::WeightLimits::of(&levels.buffs())).expect("elite limits");
    read.set_strength(51.0);
    let made = trp_sim::movement_settings_as_read(&read, &db, &hard, &engine, 0.0).expect("made from the read");
    let fresh = trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("read afresh");
    assert!(std::sync::Arc::ptr_eq(&made.table, &read.table), "the table shared, not read again");
    let mut m = [made, fresh].map(|settings| Movement::new(settings, trp_sim::Body::new(&body, 5.0)));
    let inputs = [
        (1.0, walk([0.0, 1.0])),
        (2.0, Input { move_dir: [0.3, 1.0], sprint: true, ..Input::default() }),
        (1.0, Input { move_dir: [0.0, 1.0], crouch: true, ..Input::default() }),
        (0.2, Input { move_dir: [0.0, 1.0], jump: true, ..Input::default() }),
        (1.5, walk([-1.0, 0.0])),
        (2.0, Input { prone: true, ..Input::default() }),
        (1.0, walk([0.0, -1.0])),
    ];
    let mut frame = 0;
    for (seconds, input) in inputs {
        for _ in 0..(seconds / DT).round() as usize {
            for m in &mut m {
                m.step(DT, &input, &Flat);
            }
            frame += 1;
            assert_eq!((m[0].position, m[0].velocity, m[0].state), (m[1].position, m[1].velocity, m[1].state), "frame {frame}");
            let pools = |m: &Movement| [m.stamina.legs.level(), m.stamina.arms.level(), m.stamina.oxygen.level()].map(f32::to_bits);
            assert_eq!(pools(&m[0]), pools(&m[1]), "frame {frame}");
        }
    }
    assert!(m[0].position[2].abs() > 3.0, "it went somewhere: {:?}", m[0].position);
}

#[test]
fn sprinting_winds_up_past_walking_speed() {
    // Sprint pressed before moving is ignored, as in the recorded game:
    // the character walks until the key is pressed again.
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, Input { sprint: true, ..Input::default() });
    run(&mut m, 2.0, Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() });
    println!("sprint held before moving: {:.2} m/s, state {:?}", m.ground_speed(), m.state);
    assert_eq!(m.state, State::Move);

    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, walk([0.0, 1.0]));
    let input = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    let mut entered_at = None;
    for frame in 0..(4.0 / DT) as usize {
        m.step(DT, &input, &Flat);
        if entered_at.is_none() && m.state == State::Sprint {
            entered_at = Some(frame as f32 * DT);
        }
    }
    println!(
        "sprint: entered after {:.2} s, {:.3} m/s at sprint parameter {:.2}",
        entered_at.unwrap_or(f32::NAN),
        m.ground_speed(),
        m.sprint_speed
    );
    assert_eq!(m.state, State::Sprint);
    assert!(m.ground_speed() > 5.0, "{}", m.ground_speed());

    run(&mut m, 1.0, walk([0.0, 1.0]));
    println!("after releasing sprint: {:.3} m/s, state {:?}", m.ground_speed(), m.state);
    assert_eq!(m.state, State::Move);
    assert!(m.ground_speed() < 3.5);
}

#[test]
fn a_standing_jump_follows_the_lift_curve() {
    let Some(mut m) = movement(0.0) else { return };
    let lift = m.settings.lift_velocity_by_speed.evaluate(0.0);
    let expected = lift * lift / (2.0 * -m.settings.gravity);
    m.step(DT, &Input { jump: true, ..Input::default() }, &Flat);
    assert_eq!(m.state, State::Jump);
    let (mut apex, mut airtime, mut frames) = (0.0f32, 0.0f32, 0);
    while m.state == State::Jump && frames < 600 {
        m.step(DT, &Input::default(), &Flat);
        apex = apex.max(m.position[1]);
        if m.position[1] > 0.0 {
            airtime += DT;
        }
        frames += 1;
    }
    println!(
        "standing jump: apex {apex:.3} m (lift {lift:.2} m/s gives {expected:.3}), {airtime:.2} s in the air, {:.2} s in the state",
        frames as f32 * DT
    );
    assert_eq!(m.state, State::Idle);
    assert!((apex - expected).abs() < 0.03, "apex {apex}, expected {expected}");
}

#[test]
fn lean_and_stance_settle_on_their_targets() {
    let Some(mut m) = movement(0.0) else { return };
    let mut time_to_full = None;
    for frame in 0..180 {
        m.step(DT, &Input { lean: 1.0, ..Input::default() }, &Flat);
        if time_to_full.is_none() && m.smoothed_tilt >= 5.0 {
            time_to_full = Some((frame + 1) as f32 * DT);
        }
    }
    println!("lean right: full after {:.2} s", time_to_full.unwrap_or(f32::NAN));
    assert_eq!(m.smoothed_tilt, 5.0);
    run(&mut m, 3.0, Input::default());
    assert_eq!(m.smoothed_tilt, 0.0);
}

#[test]
fn the_same_input_gives_the_same_result() {
    let (Some(mut a), Some(mut b)) = (movement(0.0), movement(0.0)) else { return };
    let script = |frame: usize| Input {
        move_dir: [((frame / 40) % 3) as f32 - 1.0, 1.0],
        look_delta: [0.7, 0.0],
        sprint: frame > 120,
        jump: frame == 200,
        ..Input::default()
    };
    for frame in 0..400 {
        a.step(DT, &script(frame), &Flat);
        b.step(DT, &script(frame), &Flat);
    }
    assert_eq!(a.position, b.position);
    assert_eq!(a.body_yaw, b.body_yaw);
}

#[test]
fn carried_weight_adds_inertia() {
    let (Some(mut light), Some(mut heavy)) = (movement(0.0), movement(40.0)) else { return };
    println!("inertia at 0 kg: {:.3}, at 40 kg: {:.3}", light.settings.inertia, heavy.settings.inertia);
    assert!(heavy.settings.inertia > light.settings.inertia);
    // The speed parameter follows the selected speed while moving, more
    // slowly under load. Each starts from zero, as after the wheel has
    // been turned up from the bottom. Weight also lowers the top speed, so
    // each is timed to 90% of its own.
    let time_to = |m: &mut Movement| {
        m.smoothed_speed = 0.0;
        let goal = 0.9 * m.clamped_speed();
        let mut frames = 0;
        while m.smoothed_speed < goal && frames < 600 {
            m.step(DT, &walk([0.0, 1.0]), &Flat);
            frames += 1;
        }
        assert!(frames < 600, "never reached {goal}");
        frames as f32 * DT
    };
    let (t_light, t_heavy) = (time_to(&mut light), time_to(&mut heavy));
    println!(
        "time to 90% of walking speed: {t_light:.2} s light, {t_heavy:.2} s at 40 kg (top speed parameter {:.3} against {:.3})",
        heavy.clamped_speed(),
        light.clamped_speed()
    );
    assert!(t_heavy > t_light);
}

/// A walk does not start at full pace: the animator blends from standing
/// into walking, and the movement state changes when that blend starts.
#[test]
fn a_walk_starts_through_the_animators_blend() {
    let Some(mut m) = movement(0.0) else { return };
    let mut speeds = Vec::new();
    for _ in 0..30 {
        m.step(DT, &walk([0.0, 1.0]), &Flat);
        speeds.push(m.ground_speed());
    }
    println!(
        "from standing: {:.2} m/s after 0.05 s, {:.2} after 0.1 s, {:.2} after 0.2 s, {:.2} after 0.5 s",
        speeds[2], speeds[5], speeds[11], speeds[29]
    );
    assert_eq!(m.state, State::Move);
    // The recorded game is at 0.65, 1.6 and 3.0 m/s at those moments, for
    // a character who walks 6% faster than this untrained one.
    assert!(speeds[2] < 1.2, "{}", speeds[2]);
    assert!(speeds[5] > 0.8 && speeds[5] < 2.4, "{}", speeds[5]);
    assert!(speeds[11] > 2.4, "{}", speeds[11]);
    assert!(speeds.windows(2).take(10).all(|pair| pair[1] >= pair[0] - 0.05), "{speeds:?}");
}

/// Reversing direction goes through standing: the walk blends out and
/// starts again the other way.
#[test]
fn reversing_goes_through_standing() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 2.0, walk([0.0, 1.0]));
    let (mut stood, mut slowest) = (None, f32::MAX);
    for frame in 0..60 {
        m.step(DT, &walk([0.0, -1.0]), &Flat);
        if stood.is_none() && m.state == State::Idle {
            stood = Some(frame as f32 * DT);
        }
        slowest = slowest.min(m.ground_speed());
    }
    println!(
        "reversing: standing {:.2} s after the keys changed, slowest {slowest:.2} m/s, {:.2} m/s backward after 1 s",
        stood.unwrap_or(f32::NAN),
        -m.velocity[2]
    );
    // The recorded game stands 0.1 s after the reversal and is walking
    // again 0.15 s later.
    assert!(stood.is_some_and(|t| t < 0.25), "{stood:?}");
    assert!(slowest < 1.0, "{slowest}");
    assert_eq!(m.state, State::Move);
    assert!(m.velocity[2] < -2.5, "{:?}", m.velocity);
}

/// The character controller holds each movement state to a top speed of
/// its own, set by Strength. Standing it is low, so a walk that stops does
/// not coast: it drops to the cap as soon as the standing state begins.
#[test]
fn a_stop_is_held_to_the_standing_cap() {
    for (strength, cap) in [(0.0, 0.5), (40.0, 0.5 + 40.0 / 60.0)] {
        let Some(mut m) = movement(0.0) else { return };
        m.settings.set_strength(strength);
        let mut m = Movement::new(m.settings, m.body);
        run(&mut m, 2.0, walk([0.0, 1.0]));
        let walking = m.ground_speed();
        let mut standing = Vec::new();
        for _ in 0..40 {
            m.step(DT, &Input::default(), &Flat);
            if m.state == State::Idle {
                standing.push(m.ground_speed());
            }
        }
        println!(
            "Strength {strength}: walking {walking:.2} m/s; once standing {:.2} m/s, then {:.2} after 0.1 s and {:.2} after 0.2 s",
            standing[0], standing[6], standing[12]
        );
        assert!(walking > 2.5, "{walking}");
        // The recorded character, at Strength 40, stops at 1.17 m/s.
        assert!((standing[0] - cap).abs() < 0.01, "{} against {cap}", standing[0]);
        assert!(standing.iter().all(|&v| v <= cap + 0.001), "{standing:?}");
    }
}

/// A landing caps the selected speed for a while, and entering a sprint
/// puts the selected speed back to the wheel's setting.
#[test]
fn a_landing_slows_the_walk_for_a_moment() {
    let Some(mut m) = movement(7.5) else { return };
    m.settings.set_strength(40.0);
    let mut m = Movement::new(m.settings, m.body);
    let top = m.settings.max_speed;
    run(&mut m, 1.5, walk([0.0, 1.0]));
    m.step(DT, &Input { jump: true, ..walk([0.0, 1.0]) }, &Flat);
    let (mut capped, mut seconds, mut lowest) = (f32::MAX, 0.0, f32::MAX);
    for _ in 0..180 {
        m.step(DT, &walk([0.0, 1.0]), &Flat);
        if m.clamped_speed() < top - 0.001 {
            capped = capped.min(m.clamped_speed());
            seconds += DT;
        }
        lowest = lowest.min(m.smoothed_speed);
    }
    println!(
        "after a running jump: selected speed held to {capped:.3} of {top:.3} for {seconds:.2} s; the pace setting dips to {lowest:.3}"
    );
    // The recorded game, with this weight and Strength: 0.456, for 0.43 s
    // after a standing jump and 0.82 s after a running one, whose fall of
    // 1.1 m from its top keeps the cap on for longer than the landing does.
    assert!((capped - 0.456).abs() < 0.002, "{capped}");
    assert!((seconds - 0.80).abs() < 0.05, "{seconds}");
    assert!(lowest < top - 0.02 && m.smoothed_speed > lowest, "{lowest}");
    assert_eq!(m.state, State::Move);

    // The wind-up before a sprint raises the selected speed to 1; the
    // sprint starting puts it back.
    let sprint = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    let mut raised = 0.0f32;
    for _ in 0..120 {
        m.step(DT, &sprint, &Flat);
        if m.state == State::Move {
            raised = raised.max(m.character_speed);
        }
    }
    println!("into a sprint: selected speed raised to {raised:.3}, then back to {:.3}", m.character_speed);
    assert_eq!(m.state, State::Sprint);
    assert!(raised > top + 0.1, "{raised}");
    assert!((m.character_speed - top).abs() < 1e-4, "{}", m.character_speed);
}

/// Sprinting, the body carries the weapon. With the hands busy on it (a
/// reload) the body lets most of it go, so that it comes up into view,
/// and takes it back when they are done.
#[test]
fn a_reload_in_a_sprint_brings_the_weapon_up() {
    let Some(mut m) = movement(0.0) else { return };
    let sprint = Input { sprint: true, ..walk([0.0, 1.0]) };
    let busy = Input { hands_busy: true, ..sprint };
    let hold = |m: &Movement| m.carried().map_or(0.0, |c| c.authority);
    run(&mut m, 0.5, walk([0.0, 1.0]));
    run(&mut m, 2.5, sprint);
    assert_eq!(m.state, State::Sprint);
    let sprinting = hold(&m);
    let mut course = Vec::new();
    for step in 0..180 {
        m.step(DT, if step < 120 { &busy } else { &sprint }, &Flat);
        if step % 6 == 5 {
            course.push(hold(&m));
        }
    }
    println!("the body's hold on the weapon in a sprint {sprinting:.2}; every tenth of a second through two seconds of busy hands and one after: {course:.2?}");
    assert_eq!(m.state, State::Sprint, "the sprint goes on");
    assert!((sprinting - 1.0).abs() < 0.01, "{sprinting}");
    // Down to the busy pose's 0.35 within half a second, held there, and
    // back to the sprint's own within a third of a second of the end.
    assert!((course[5] - 0.35).abs() < 0.02 && (course[19] - 0.35).abs() < 0.02, "{course:?}");
    assert!(course[2] < sprinting && course[2] > 0.35, "{course:?}");
    assert!((course[23] - 1.0).abs() < 0.02, "{course:?}");
}

/// A walk begins with what the last walk's direction smoothing was still
/// doing when that walk ended: after a walk forward, the next one's
/// forward direction dips below zero before it rises, and it covers less
/// ground in its first half second than a first walk does.
#[test]
fn a_walk_starts_with_what_the_last_one_left() {
    // The recorded character: 7.5 kg carried, Strength 40, 120 frames a second.
    let Some(m) = movement(7.5) else { return };
    let mut settings = m.settings;
    settings.set_strength(40.0);
    let mut m = Movement::new(settings, m.body);
    let frame = 1.0 / 120.0;
    let start = |m: &mut Movement| {
        let from = m.position[2];
        let mut direction = Vec::new();
        for _ in 0..60 {
            m.step(frame, &walk([0.0, 1.0]), &Flat);
            if m.state == State::Move {
                direction.push(m.movement_direction[1]);
            }
        }
        (direction, m.position[2] - from)
    };
    let (first, first_ground) = start(&mut m);
    for _ in 0..360 {
        m.step(frame, &walk([0.0, 1.0]), &Flat);
    }
    for _ in 0..120 {
        m.step(frame, &Input::default(), &Flat);
    }
    assert_eq!(m.state, State::Idle);
    let leftover = m.walk_leftover();
    let (second, second_ground) = start(&mut m);
    let at = |d: &[f32]| [d[0], d[1], d[2], d[4], d[8], d[12], d[24]];
    println!("the first walk's direction, frames 0, 1, 2, 4, 8, 12, 24: {:.3?}; {first_ground:.3} m in half a second", at(&first));
    println!("the second's, after {:.2} per second was left: {:.3?}; {second_ground:.3} m", leftover[1], at(&second));
    // The recording, at those frames of a walk that followed another:
    // -0.022, -0.043, -0.047, -0.021, 0.110, 0.277, 0.690; and 4.38 to
    // 4.40 per second left.
    assert!((leftover[1] + 4.39).abs() < 0.05, "{leftover:?}");
    for (ours, recorded) in at(&second).iter().zip([-0.022, -0.043, -0.047, -0.021, 0.110, 0.277, 0.690]) {
        assert!((ours - recorded).abs() < 0.012, "{ours} against {recorded}");
    }
    assert!(first[0] == 0.01 && first.iter().all(|&d| d >= 0.0), "{first:?}");
    assert!(second_ground < first_ground - 0.01, "{second_ground} against {first_ground}");
}
