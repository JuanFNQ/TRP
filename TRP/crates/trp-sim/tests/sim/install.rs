//! Movement measured with the real install's data. These pass trivially,
//! with a note, on a machine where no install is configured.
//!
//! Run with `cargo test -p trp-sim -- --nocapture` to see the numbers.

use trp_data::rootmotion::Node;
use trp_data::unity::Assets;
use trp_data::Database;
use trp_sim::{Flat, Input, Movement, State};

const DT: f32 = 1.0 / 60.0;

fn movement(weight: f32) -> Option<Movement> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, weight).expect("movement settings");
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    // The stock AKM is five inventory cells wide.
    Some(Movement::new(settings, trp_sim::Body::new(&body, 5.0)))
}

fn run(m: &mut Movement, seconds: f32, input: Input) {
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, &input, &Flat);
    }
}

fn walk(dir: [f32; 2]) -> Input {
    Input { move_dir: dir, ..Input::default() }
}

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
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let base = m.settings.clone();
    m.set_weight_limits(&hard, &db.config, trp_sim::settings::WeightLimits::BASE).expect("untrained limits");
    assert_eq!((m.settings.inertia, m.settings.overweight), (base.inertia, base.overweight), "untrained: nothing changes");
    let mut levels = trp_sim::skills::SkillLevels::default();
    levels.set_level(trp_sim::skills::Skill::Strength, 51);
    let limits = trp_sim::settings::WeightLimits::of(&levels.buffs());
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
    fn visit(table: &trp_data::rootmotion::RootMotionTable, node: &Node, worst: &mut f32, count: &mut usize) {
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

/// Where the first-person view sits comes from the body's animation: the
/// ribcage's height standing and crouched, its bob with each stride and
/// its shift in a lean.
#[test]
fn the_view_hangs_from_the_bodys_ribcage() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, Input::default());
    let standing = m.body.ribcage();
    // The camera sits 0.04 m above; the recorded eye is at 1.508 m.
    println!("standing: ribcage {standing:?}");
    assert!((standing[1] - 1.468).abs() < 0.02, "{standing:?}");

    // A full lean to the right carries it 0.3 m aside and 0.06 m down.
    run(&mut m, 1.5, Input { lean: 1.0, ..Input::default() });
    let leaning = m.body.ribcage();
    println!("leaning right: {:+.3} m aside, {:+.3} m in height", leaning[0] - standing[0], leaning[1] - standing[1]);
    assert!((leaning[0] - standing[0] - 0.30).abs() < 0.05, "{leaning:?}");
    assert!((standing[1] - leaning[1] - 0.06).abs() < 0.03, "{leaning:?}");
    run(&mut m, 1.5, Input::default());

    // Fully crouched the recorded eye is 0.6 m lower.
    m.step(DT, &Input { pose_delta: -1.0, ..Input::default() }, &Flat);
    run(&mut m, 3.0, Input::default());
    let crouched = m.body.ribcage();
    println!("crouched: {:.3} m lower", standing[1] - crouched[1]);
    assert!((standing[1] - crouched[1] - 0.60).abs() < 0.06, "{crouched:?}");

    // Walking, it rises and falls with the stride.
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 1.0, walk([0.0, 1.0]));
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for _ in 0..120 {
        m.step(DT, &walk([0.0, 1.0]), &Flat);
        let height = m.body.ribcage()[1];
        (low, high) = (low.min(height), high.max(height));
    }
    println!("walking: the ribcage moves between {low:.3} and {high:.3} m");
    assert!(high - low > 0.01 && high - low < 0.1, "{low} to {high}");
    assert!(high < standing[1], "walking is lower than standing: {high}");
}

/// While the body sprints its own weapon bone takes the first-person
/// weapon over entirely, and hands it back when the sprint ends.
#[test]
fn the_sprint_carries_the_weapon() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, walk([0.0, 1.0]));
    assert!(m.carried().is_none(), "a walk does not carry the weapon");
    run(&mut m, 2.0, Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() });
    assert_eq!(m.state, State::Sprint);
    let carried = m.carried().expect("a sprint carries the weapon");
    println!("sprinting: the weapon is carried at {:?}, turned {:?}", carried.position, carried.rotation);
    assert_eq!(carried.authority, 1.0);
    // Below the ribcage, and turned well away from the view.
    assert!(carried.position[1] < -0.05, "{:?}", carried.position);
    assert!(carried.rotation[3].abs() < 0.95, "{:?}", carried.rotation);
    // The hand-over fades in and out with the animator's blends.
    run(&mut m, 0.15, walk([0.0, 1.0]));
    let fading = m.carried().map_or(0.0, |c| c.authority);
    assert!(fading > 0.0 && fading < 1.0, "{fading}");
    run(&mut m, 1.0, walk([0.0, 1.0]));
    assert!(m.carried().is_none());
}

/// A lying body is laid to the ground, and its roll is limited the way
/// the game limits it: ground a touch lower on the left gives the true
/// small roll, ground a touch lower on the right a full 8 degrees the
/// wrong way. Getting up clears it.
#[test]
fn a_lying_body_is_laid_to_the_ground() {
    use trp_sim::math::quat_to_euler;
    /// Even ground rising `0` metres a metre to the right (the world's x).
    struct Tilted(f32);
    impl trp_sim::World for Tilted {
        fn ground_height(&self, x: f32, _z: f32) -> f32 {
            self.0 * x
        }
        fn ground_normal(&self, _x: f32, _z: f32) -> [f32; 3] {
            let len = (self.0 * self.0 + 1.0).sqrt();
            [-self.0 / len, 1.0 / len, 0.0]
        }
    }
    let lie = |slope: f32| -> Option<(f32, f32)> {
        let (mut m, ground) = (movement(0.0)?, Tilted(slope));
        m.step(DT, &Input { prone: true, ..Input::default() }, &ground);
        for _ in 0..150 {
            m.step(DT, &Input::default(), &ground);
        }
        assert_eq!(m.state, State::ProneIdle);
        let lying = quat_to_euler(m.body_tilt)[2];
        m.step(DT, &Input { prone: true, ..Input::default() }, &ground);
        for _ in 0..150 {
            m.step(DT, &Input::default(), &ground);
        }
        Some((lying, quat_to_euler(m.body_tilt)[2]))
    };
    // A thousandth is 0.057 degrees.
    let (Some(level), Some(right_higher), Some(right_lower)) = (lie(0.0), lie(0.001), lie(-0.001)) else { return };
    println!("roll lying on level ground {:.3}, right side higher {:.3}, right side lower {:.3} degrees", level.0, right_higher.0, right_lower.0);
    assert!(level.0.abs() < 1e-3, "{level:?}");
    assert!((right_higher.0 - 0.0573).abs() < 0.005, "{right_higher:?}");
    assert!((right_lower.0 - 8.0).abs() < 1e-3, "{right_lower:?}");
    for (_, up_again) in [level, right_higher, right_lower] {
        assert!(up_again.abs() < 1e-4, "{up_again}");
    }
}

/// Crawling, the body's clips take both hands, and the weapon goes to the
/// right one; lying still and sprinting it does not.
#[test]
fn a_crawl_takes_the_weapon_in_hand() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, walk([0.0, 1.0]));
    run(&mut m, 2.0, Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() });
    assert_eq!(m.state, State::Sprint);
    assert!(m.carried().is_some() && m.held().is_none(), "a sprint carries the weapon at the body's weapon bone");
    let Some(mut m) = movement(0.0) else { return };
    m.step(DT, &Input { prone: true, ..Input::default() }, &Flat);
    run(&mut m, 2.5, Input::default());
    assert!(m.held().is_none(), "lying still the hands are on the weapon");
    run(&mut m, 1.0, walk([0.0, 1.0]));
    assert_eq!(m.state, State::ProneMove);
    let held = m.held().expect("a crawl has the weapon in the right hand");
    println!("crawling: the right palm is at {:?} from the ribcage", held.position);
    assert_eq!(held.weight, 0.0);
    assert!(held.position.iter().all(|c| c.abs() < 0.8), "{:?}", held.position);
}

/// Lying down, crawling and getting up: the animator takes the character
/// down through a clip of its own, the crawl goes at its own pace whatever
/// the speed wheel says, and getting up returns to the stance held before.
#[test]
fn lying_down_crawling_and_getting_up() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, Input::default());
    let standing = m.body.ribcage()[1];

    // Which states the character passes through, and when each begins.
    fn follow(m: &mut Movement, seconds: f32, input: Input, log: &mut Vec<(f32, State)>, clock: &mut f32) {
        for _ in 0..(seconds / DT).round() as usize {
            m.step(DT, &input, &Flat);
            *clock += DT;
            if log.last().is_none_or(|(_, state)| *state != m.state) {
                log.push((*clock, m.state));
            }
        }
    }
    let (mut log, mut clock) = (vec![(0.0, m.state)], 0.0);

    m.step(DT, &Input { prone: true, ..Input::default() }, &Flat);
    follow(&mut m, 2.5, Input::default(), &mut log, &mut clock);
    let lying = m.body.ribcage();
    println!("lying down from standing: {log:?}");
    println!("lying still: ribcage {lying:?} (standing {standing:.3}), stance {}, pitch range {:?}", m.pose_level, m.pitch_range());
    assert_eq!(m.state, State::ProneIdle);
    assert!(lying[1] < 0.5, "{lying:?}");
    assert_eq!(m.pose_level, 0.0);

    // The view pitches 16 degrees up and 25 down, and turning hard it
    // leads the body by 35 degrees at most, 25 once the body has caught up.
    m.step(DT, &Input { look_delta: [0.0, -60.0], free_look: true, ..Input::default() }, &Flat);
    let up = m.pitch;
    m.step(DT, &Input { look_delta: [0.0, 100.0], free_look: true, ..Input::default() }, &Flat);
    let down = m.pitch;
    m.step(DT, &Input { look_delta: [0.0, -down], free_look: true, ..Input::default() }, &Flat);
    let mut lead = 0.0f32;
    for _ in 0..30 {
        m.step(DT, &Input { look_delta: [12.0, 0.0], free_look: true, ..Input::default() }, &Flat);
        lead = lead.max((m.yaw - m.body_yaw).abs());
    }
    let turned = m.yaw;
    run(&mut m, 0.5, Input::default());
    println!(
        "lying still: pitch {up} to {down}; turning 720 degrees a second for half a second turns the view {turned:.0} degrees, leading the body by up to {lead:.1} and by {:.1} at rest",
        m.yaw - m.body_yaw
    );
    assert_eq!([up, down], [-16.0, 25.0]);
    assert!(lead <= 35.0 + 1e-3 && lead > 25.0, "{lead}");
    assert!((m.yaw - m.body_yaw - 25.0).abs() < 0.5, "{}", m.yaw - m.body_yaw);
    // Turn back to where the walk test expects the body.
    let back = -m.yaw;
    for _ in 0..40 {
        m.step(DT, &Input { look_delta: [back / 40.0, 0.0], free_look: true, ..Input::default() }, &Flat);
    }
    run(&mut m, 0.5, Input::default());

    // Crawling forward, then with the wheel at the bottom.
    let before = m.position;
    follow(&mut m, 3.0, walk([0.0, 1.0]), &mut log, &mut clock);
    assert_eq!(m.state, State::ProneMove);
    let pace = m.ground_speed();
    let crawled = m.position[2] - before[2];
    for _ in 0..20 {
        m.step(DT, &Input { speed_delta: -m.settings.delta_speed, ..walk([0.0, 1.0]) }, &Flat);
    }
    follow(&mut m, 2.0, walk([0.0, 1.0]), &mut log, &mut clock);
    println!("crawling: {pace:.2} m/s, {crawled:.2} m in 3 s; with the wheel at the bottom {:.2} m/s", m.ground_speed());
    // The install's root motion table has the crawl at 0.5 m/s forward,
    // 0.4 backward and 0.275 sideways.
    assert!((pace - 0.5).abs() < 0.01, "{pace}");
    assert!((m.ground_speed() - pace).abs() < 0.01, "the wheel does not change the crawl");
    for (keys, table) in [([0.0, -1.0], 0.4), ([1.0, 0.0], 0.275)] {
        follow(&mut m, 1.0, Input::default(), &mut log, &mut clock);
        follow(&mut m, 2.5, walk(keys), &mut log, &mut clock);
        println!("crawling {keys:?}: {:.3} m/s", m.ground_speed());
        assert!((m.ground_speed() - table).abs() < 0.01, "{keys:?}: {}", m.ground_speed());
    }

    // Stopping, then getting up.
    follow(&mut m, 1.0, Input::default(), &mut log, &mut clock);
    assert_eq!(m.state, State::ProneIdle);
    m.step(DT, &Input { prone: true, ..Input::default() }, &Flat);
    follow(&mut m, 3.0, Input::default(), &mut log, &mut clock);
    println!("and up again: {log:?}");
    println!("standing again: ribcage {:.3}, stance {}, pitch range {:?}", m.body.ribcage()[1], m.pose_level, m.pitch_range());
    assert_eq!(m.state, State::Idle);
    assert_eq!(m.pose_level, 1.0);
    assert!((m.body.ribcage()[1] - standing).abs() < 0.02);
    assert_eq!(m.pitch_range(), [-90.0, 90.0]);
}

/// Out of a sprint the character dives: the clip carries it on, and it
/// ends crawling if the key is still held. The crouch key, lying still,
/// gets it up into a crouch.
#[test]
fn a_sprint_dives_into_prone() {
    let Some(mut m) = movement(0.0) else { return };
    let sprint = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    run(&mut m, 0.5, walk([0.0, 1.0]));
    run(&mut m, 2.5, sprint);
    assert_eq!(m.state, State::Sprint);
    let from = m.position[2];
    m.step(DT, &Input { prone: true, ..sprint }, &Flat);
    let mut dived = None;
    for _ in 0..120 {
        m.step(DT, &walk([0.0, 1.0]), &Flat);
        if dived.is_none() && m.state != State::ToProne && m.state != State::Sprint {
            dived = Some(m.position[2] - from);
        }
    }
    println!("diving out of a sprint: {:.2} m further on before the crawl begins, then {:?}", dived.unwrap_or(f32::NAN), m.state);
    assert_eq!(m.state, State::ProneMove);
    // The dive's clip covers 2.2 m in its second, and gives way to the
    // crawl half way through.
    assert!(dived.is_some_and(|d| d > 0.8 && d < 2.4), "{dived:?}");

    run(&mut m, 1.0, Input::default());
    assert_eq!(m.state, State::ProneIdle);
    m.step(DT, &Input { crouch: true, ..Input::default() }, &Flat);
    run(&mut m, 3.0, Input::default());
    println!("the crouch key, lying still: {:?} at stance {}", m.state, m.pose_level);
    assert_eq!(m.state, State::Idle);
    assert_eq!(m.pose_level, 0.0);
}

/// On frames of even length the game's whole-millisecond clock for the
/// player makes a jump's travel uneven (8, 8 and 9 ms of it a frame at 120
/// a second). The place a host is given to draw has that taken out.
#[test]
fn a_jump_is_drawn_as_evenly_as_a_walk() {
    let Some(mut m) = movement(0.0) else { return };
    // The largest change of level travel from one frame to the next, as a
    // share of the travel, over the frames in the air.
    let (mut moved, mut drawn) = (Vec::new(), Vec::new());
    let (mut at, mut shown) = (m.position[2], m.drawn_position()[2]);
    for frame in 0..600 {
        m.frame(1.0 / 120.0, &Input { jump: frame == 300, ..walk([0.0, 1.0]) }, &Flat);
        if m.state == State::Jump && frame > 300 {
            moved.push(m.position[2] - at);
            drawn.push(m.drawn_position()[2] - shown);
        }
        (at, shown) = (m.position[2], m.drawn_position()[2]);
    }
    let ripple = |travel: &[f32]| travel.windows(2).map(|w| ((w[1] - w[0]) / w[0]).abs()).fold(0.0f32, f32::max);
    println!(
        "a running jump, {} frames in the air: travel a frame changes by up to {:.1}% as moved, {:.2}% as drawn",
        moved.len(),
        100.0 * ripple(&moved),
        100.0 * ripple(&drawn)
    );
    assert!(moved.len() > 60);
    assert!(ripple(&moved) > 0.1, "{}", ripple(&moved));
    assert!(ripple(&drawn) < 0.01, "{}", ripple(&drawn));
    // On the ground the two are one.
    assert_eq!(m.drawn_position(), m.position);
}

/// The movement state slows the view's turning and limits how far one
/// frame may turn it: half the rate in a sprint, a third in a jump from
/// standing and lying down, and none at all through a reload lying down.
#[test]
fn the_movement_state_slows_the_views_turning() {
    let Some(mut m) = movement(0.0) else { return };
    // How far one step turns the view when asked for `ask` degrees.
    fn turned(m: &mut Movement, ask: f32, held: Input) -> f32 {
        let before = m.yaw;
        m.step(DT, &Input { look_delta: [ask, 0.0], ..held }, &Flat);
        m.yaw - before
    }
    let still = Input::default();
    assert!((turned(&mut m, 10.0, still) - 10.0).abs() < 1e-4);

    let sprint = Input { sprint: true, ..walk([0.0, 1.0]) };
    run(&mut m, 0.5, walk([0.0, 1.0]));
    assert!((turned(&mut m, 10.0, walk([0.0, 1.0])) - 10.0).abs() < 1e-4, "walking turns freely");
    run(&mut m, 2.0, sprint);
    assert_eq!(m.state, State::Sprint);
    let (sprinting, far) = (turned(&mut m, 10.0, sprint), turned(&mut m, 60.0, sprint));
    run(&mut m, 2.0, still);

    m.step(DT, &Input { jump: true, ..still }, &Flat);
    run(&mut m, 0.3, still);
    assert_eq!(m.state, State::Jump);
    let jumping = turned(&mut m, 9.0, still);
    run(&mut m, 2.0, still);

    m.step(DT, &Input { prone: true, ..still }, &Flat);
    run(&mut m, 2.5, still);
    assert_eq!(m.state, State::ProneIdle);
    let (lying, lying_far) = (turned(&mut m, 9.0, still), turned(&mut m, 60.0, still));
    let reloading = Input { reloading: true, ..still };
    run(&mut m, 0.5, reloading);
    let in_reload = turned(&mut m, 9.0, reloading);
    run(&mut m, 0.5, still);
    let after = turned(&mut m, 9.0, still);
    println!(
        "asked for 10 degrees in a step: sprinting {sprinting:.2} (60 gives {far:.2}); for 9: in a standing jump {jumping:.2}, lying {lying:.2} (60 gives {lying_far:.2}), in a reload lying {in_reload:.2}, after it {after:.2}"
    );
    assert!((sprinting - 5.0).abs() < 1e-3 && (far - 10.0).abs() < 1e-3, "{sprinting} {far}");
    assert!((jumping - 2.97).abs() < 1e-3, "{jumping}");
    assert!((lying - 2.97).abs() < 1e-3 && (lying_far - 5.0).abs() < 1e-3, "{lying} {lying_far}");
    assert_eq!(in_reload, 0.0);
    assert!((after - 2.97).abs() < 1e-3, "{after}");
}

/// A host that draws more often than the simulation's frames come shows
/// the mouse's travel no frame has had yet. Shown as the next frame will
/// let it through, the view turns evenly through a jump, where only a third
/// gets through; shown whole it would step forward and back.
#[test]
fn the_view_shown_between_frames_turns_evenly() {
    for host_rate in [144.0f32, 240.0, 360.0] {
        let Some(mut m) = movement(0.0) else { return };
        let mut pacer = trp_sim::Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
        // The mouse turns the view a steady degree per drawn frame.
        let (mut pending, mut jumped) = (0.0f32, false);
        let (mut shown, mut whole) = (Vec::new(), Vec::new());
        for host_frame in 0..(host_rate as usize * 2) {
            pending += 1.0;
            let due = pacer.pass(1.0 / host_rate);
            let share = pending / due.max(1) as f32;
            for _ in 0..due {
                pacer.frame();
                let jump = host_frame as f32 / host_rate > 0.5 && !std::mem::replace(&mut jumped, true);
                m.frame(pacer.frame_length(), &Input { look_delta: [share, 0.0], jump, ..Input::default() }, &Flat);
                pending = 0.0;
            }
            if m.state == State::Jump {
                shown.push(m.look_ahead([pending, 0.0])[0]);
                whole.push(m.yaw + pending);
            }
        }
        let steps = |view: &[f32]| -> (f32, f32) {
            let steps: Vec<f32> = view.windows(2).map(|w| w[1] - w[0]).collect();
            (steps.iter().copied().fold(f32::MAX, f32::min), steps.iter().copied().fold(f32::MIN, f32::max))
        };
        let ((least, most), (least_whole, most_whole)) = (steps(&shown), steps(&whole));
        println!(
            "a standing jump drawn at {host_rate} a second, a degree of mouse a frame: the view shown turns {least:.3} to {most:.3} degrees a frame; with the travel shown whole, {least_whole:.3} to {most_whole:.3}"
        );
        assert!(shown.len() > 40, "{}", shown.len());
        assert!((least - 0.33).abs() < 1e-3 && (most - 0.33).abs() < 1e-3, "{least} {most}");
        assert!(least_whole < 0.0, "{least_whole}");
    }
}

/// In a jump the body turns after the view with a lag and carries the
/// weapon, so the weapon and arms are drawn by the angle between the two.
/// Two things made that angle change unevenly from frame to frame, and the
/// weapon shake against a view that turned smoothly: the mouse's travel
/// reaching the simulation's frames in uneven lots wherever the host's
/// rate and the simulation's do not divide, and the game's own turning of
/// the body by a clock in whole milliseconds. The first is shared out by
/// time; the second is taken out of what is drawn.
#[test]
fn the_body_follows_the_view_evenly_in_a_jump() {
    // How far the angle from body to view moves in a frame, set against
    // the mean of the frames either side of it: the largest such gap, in
    // degrees, while the body is turning after the view. A smooth run-in
    // has next to none; a ripple shows whole.
    let ripple = |host_rate: f32, by_time: bool, drawn: bool| -> Option<(f32, f32, usize)> {
        let mut m = movement(0.0)?;
        let mut pacer = trp_sim::Pacer::new(120.0, 1.0 / 60.0, 1.0 / 12.0);
        // The mouse turns at a steady 360 degrees a second.
        let (mut held, mut jumped) = (0.0f32, false);
        let mut angles = Vec::new();
        for host_frame in 0..(host_rate as usize * 2) {
            held += 360.0 / host_rate;
            let due = pacer.pass(1.0 / host_rate);
            for _ in 0..due {
                let frame = pacer.frame();
                // Given whole, the frame that comes due takes all there is.
                let turn = if by_time { held * frame.input_share } else { held };
                held -= turn;
                let jump = host_frame as f32 / host_rate > 0.5 && !std::mem::replace(&mut jumped, true);
                m.frame(frame.dt, &Input { look_delta: [turn, 0.0], jump, ..Input::default() }, &Flat);
                if m.state == State::Jump {
                    let body = if drawn { m.drawn_body_yaw() } else { m.body_yaw };
                    angles.push(trp_sim::math::delta_angle(body, m.yaw));
                }
            }
        }
        // The body stops turning some frames before the state ends; the
        // first hundred frames are all turning.
        let moved: Vec<f32> = angles.windows(2).take(100).map(|w| w[1] - w[0]).collect();
        let ripple = moved.windows(3).map(|w| (w[1] - (w[0] + w[2]) / 2.0).abs()).fold(0.0f32, f32::max);
        let widest = angles.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        Some((ripple, widest, angles.len()))
    };
    for host_rate in [144.0f32, 165.0, 240.0] {
        let (Some((before, widest, frames)), Some((games, _, _)), Some((now, _, _))) =
            (ripple(host_rate, false, false), ripple(host_rate, true, false), ripple(host_rate, true, true))
        else {
            return;
        };
        println!(
            "a standing jump drawn at {host_rate} a second, the mouse at 360 degrees a second, the view up to {widest:.0} degrees ahead of the body over {frames} frames: the angle between them ripples by {before:.3} degrees a frame with the mouse's travel given whole, {games:.3} shared by time (the game's own, from its clock), {now:.3} as drawn"
        );
        assert!(frames > 110, "{frames}");
        assert!(games > 0.05, "{host_rate}: {games}");
        assert!(now < 0.02, "{host_rate}: {now}");
    }
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

/// A reload in hand holds a lying body still: it ends a crawl, and the
/// movement keys do nothing until the magazine is in. The prone key is
/// taken at any time, as the offline player takes it: it gets a standing
/// character down in a reload, and a lying one up once the reload lets go.
#[test]
fn a_reload_holds_a_lying_body_still() {
    let Some(mut m) = movement(0.0) else { return };
    let reloading = |input: Input| Input { reloading: true, ..input };

    // Standing: the key takes the character down with the reload in hand.
    m.step(DT, &reloading(Input { prone: true, ..Input::default() }), &Flat);
    run(&mut m, 1.5, reloading(Input::default()));
    assert_eq!(m.state, State::ProneIdle);
    assert!(m.prone);
    run(&mut m, 1.0, walk([0.0, 1.0]));
    assert_eq!(m.state, State::ProneMove);

    // The reload ends the crawl with the key still held.
    let from = m.position[2];
    let mut stopped = None;
    for step in 0..180 {
        m.step(DT, &reloading(walk([0.0, 1.0])), &Flat);
        if stopped.is_none() && m.state == State::ProneIdle {
            stopped = Some(step);
        }
    }
    let crept = m.position[2] - from;
    println!("a reload in a crawl: lying still {stopped:?} steps after it began, {crept:.3} m further on; then {:.3} m/s", m.ground_speed());
    assert_eq!(stopped, Some(0), "the crawl gives way as the blend begins");
    assert_eq!(m.state, State::ProneIdle);
    assert!(crept < 0.15, "{crept}");
    assert!(m.ground_speed() < 0.01, "{}", m.ground_speed());

    // With the magazine in, the key held starts the crawl again.
    run(&mut m, 1.0, walk([0.0, 1.0]));
    assert_eq!(m.state, State::ProneMove);
    run(&mut m, 1.0, Input::default());
    assert_eq!(m.state, State::ProneIdle);

    // The prone key in a reload lying down is taken, and the body stays
    // down until the reload lets go of it; then it gets up by itself.
    run(&mut m, 0.5, reloading(Input::default()));
    m.step(DT, &reloading(Input { prone: true, ..Input::default() }), &Flat);
    run(&mut m, 2.0, reloading(Input::default()));
    assert_eq!(m.state, State::ProneIdle);
    assert!(!m.prone);
    run(&mut m, 3.0, Input::default());
    assert_eq!(m.state, State::Idle);

    // The player of an online raid refuses the key both ways instead.
    m.settings.prone_key_refused_in_reload = true;
    m.step(DT, &reloading(Input { prone: true, ..Input::default() }), &Flat);
    run(&mut m, 1.5, reloading(Input::default()));
    assert_eq!(m.state, State::Idle);
    assert!(!m.prone);
    m.step(DT, &Input { prone: true, ..Input::default() }, &Flat);
    run(&mut m, 2.5, Input::default());
    assert_eq!(m.state, State::ProneIdle);
    m.step(DT, &reloading(Input { prone: true, ..Input::default() }), &Flat);
    run(&mut m, 2.0, reloading(Input::default()));
    assert_eq!(m.state, State::ProneIdle);
    assert!(m.prone);
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

/// Standing, the body lets the view lead it by up to 45 degrees. Past
/// that the animator plays a turn on the spot, whose clip swings the body
/// a quarter turn.
#[test]
fn a_turn_past_the_limit_turns_the_body_on_the_spot() {
    let Some(mut m) = movement(0.0) else { return };
    run(&mut m, 0.5, Input::default());
    // 40 degrees: the body stays where it is.
    for _ in 0..20 {
        m.step(DT, &Input { look_delta: [2.0, 0.0], free_look: true, ..Input::default() }, &Flat);
    }
    run(&mut m, 1.0, Input::default());
    println!("view turned 40 degrees: the body at {:.1}, state {:?}", m.body_yaw, m.body.state_names().0);
    assert!(m.body_yaw.abs() < 0.01, "{}", m.body_yaw);

    // 30 more: past the limit.
    let mut log = Vec::new();
    for frame in 0..90 {
        let turn = if frame < 15 { 2.0 } else { 0.0 };
        m.step(DT, &Input { look_delta: [turn, 0.0], free_look: true, ..Input::default() }, &Flat);
        if frame % 6 == 5 {
            log.push(format!("{:.0}", m.body_yaw));
        }
    }
    let (root, _) = (m.body.root_rotation(), ());
    println!(
        "view turned to 70 degrees: the body at {} by tenths of a second; ends at {:.1} in {:?}, the root bone turned {:.1} degrees in the pose",
        log.join(" "),
        m.body_yaw,
        m.body.state_names().0,
        2.0 * root[1].atan2(root[3]).to_degrees()
    );
    assert_eq!(m.state, State::Idle);
    assert!(m.body.state_names().0.ends_with("Idle_Aim"));
    // The clip turns the body a quarter turn in its first 0.4 s, on top
    // of the little the body had followed the view by then, so the body
    // ends ahead of the view. Its root bone keeps none of the turn.
    assert!(m.body_yaw > 90.0 && m.body_yaw < 95.0, "{}", m.body_yaw);
    assert!((m.yaw - 70.0).abs() < 1e-3 && (m.body_yaw - m.yaw).abs() < 45.0);
    assert!(root[1].abs() < 1e-3, "{root:?}");
}

/// A view flicked one way and then further back the other, as a mouse
/// does it: the second turn on the spot cuts into the first, and the view
/// comes to rest while it still plays, with the body drawn after it at the
/// limit. The game takes the angle between them with 360 added to each
/// bearing, which rounds it, and its body's turning is finer than a
/// bearing, so the angle comes to read the limit exactly and no third
/// turn follows (movement spec, "Looking"; eleventh recording, 59).
#[test]
fn a_body_held_at_the_limit_does_not_turn_again() {
    // From the lane's bearing, from the eleventh recording's, and after
    // the view has been round eight times, where a bearing in single
    // precision is at its coarsest.
    for bearing in [0.0, 20.565_096, 167.833_98, 2900.565] {
        let Some(mut m) = movement(0.0) else { return };
        (m.yaw, m.body_yaw) = (bearing, bearing);
        m.note_rotation();
        run(&mut m, 0.5, Input::default());
        let mut turns = Vec::new();
        let mut last = String::new();
        // At the rate the simulation is run at, where the body takes a
        // quarter of what it is over the limit each frame.
        const FRAME: f32 = 1.0 / 120.0;
        for frame in 0..600 {
            // 84 degrees to the left in a fifth of a second, a pause as
            // long, 168 to the right in two fifths: 420 degrees a second.
            let turn = match frame {
                0..24 => -3.5,
                48..96 => 3.5,
                _ => 0.0,
            };
            m.step(FRAME, &Input { look_delta: [turn, 0.0], free_look: true, ..Input::default() }, &Flat);
            let (playing, next) = m.body.state_names();
            let entering = next.unwrap_or(playing).to_string();
            if entering != last && entering.contains("Turn") {
                turns.push((frame, entering.clone()));
            }
            last = entering;
        }
        let [angle, _] = m.noted_angles();
        println!(
            "from a bearing of {bearing}, a flick left and a longer one back: turns begun {turns:?}; the view ends at {:.3}, the body at {:.5}, the angle between them reads {angle:?} in {:?}",
            m.yaw,
            m.body_yaw,
            m.body.state_names().0
        );
        assert_eq!(turns.len(), 2, "{turns:?}");
        assert!(turns[0].1.contains("Left") && turns[1].1.contains("Right"), "{turns:?}");
        assert!(m.body.state_names().0.ends_with("Idle_Aim"));
        assert_eq!(angle, m.settings.trunk_rotation_limit, "the body is held at the limit to the last place");
    }
}

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
    use trp_sim::script::{Clock, Scenario};
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

/// A starting character's equipment, read from the install and put
/// together (inventory spec): every entry of the database's list finds
/// its slot or grid, everything in a grid lies where the grid's own rules
/// allow it, and the whole has a weight. Nothing here is compared with
/// the game.
#[test]
fn a_starting_character_carries_its_equipment() {
    use trp_sim::inventory::{Equipment, SLOTS};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let empty = Equipment::empty(&db).expect("the equipment's template");
    assert_eq!(empty.item.kind.slots.iter().map(|slot| slot.name.as_str()).collect::<std::collections::BTreeSet<_>>(), SLOTS.into_iter().collect());
    assert_eq!(empty.carried_weight(false), 0.0);
    for side in ["usec", "bear"] {
        let loadout = trp_data::inventory::Loadout::load(&install, "Standard", side).expect("the starting character");
        let (equipment, left_out) = Equipment::of_loadout(&db, &loadout).expect("its equipment");
        assert!(left_out.is_empty(), "{left_out:?}");
        println!("Standard/{side}: {} entries, {:.3} kg carried ({:.3} at elite Strength)", loadout.items.len(), equipment.carried_weight(false), equipment.carried_weight(true));
        for slot in SLOTS {
            let Some(item) = equipment.worn(slot) else { continue };
            let [width, height] = item.cells();
            let grids: Vec<String> = item.kind.grids.iter().zip(&item.grids).map(|(grid, held)| format!("{}x{} holding {}", grid.cells[0], grid.cells[1], held.len())).collect();
            println!("  {slot}: {} ({width}x{height} cells, {:.3} kg){}", item.kind.name, item.weight(), if grids.is_empty() { String::new() } else { format!("; grids {}", grids.join(", ")) });
            // What the game put in a grid passes the grid's own rules,
            // each thing against the others.
            for (index, held) in item.grids.iter().enumerate() {
                let mut holder = item.clone();
                holder.grids[index].clear();
                for (place, inside) in held {
                    assert_eq!(holder.put(index, inside.clone(), *place), Ok(()), "{} in {}", inside.kind.name, item.kind.name);
                }
            }
        }
        assert!(equipment.worn("FirstPrimaryWeapon").is_some_and(|rifle| rifle.cells()[0] >= 4 && rifle.weight() > 2.0));
        assert!(equipment.carried_weight(false) > 10.0 && equipment.carried_weight(true) < equipment.carried_weight(false));
        assert!(!equipment.drawn().is_empty());
    }
}

/// The inventory screen's equipment panel as the install's own scene lays
/// it out (inventory spec, "Where the views sit"): a view for each of the
/// ten slots the game's panel shows, three to a row, none over another,
/// with the two long weapons' views wider than the rest.
#[test]
fn the_equipment_panel_is_the_installs_own() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let panel = trp_data::screen::GearPanel::load(&install).expect("the equipment panel");
    println!("the equipment panel, from {}: {} x {}", panel.file, panel.size[0], panel.size[1]);
    for (name, [left, top, width, height]) in &panel.slots {
        println!("  {name}: left {left} top {top}, {width} x {height}");
    }
    let rows = [["Earpiece", "Headwear", "FaceCover"], ["ArmBand", "ArmorVest", "Eyewear"]];
    let weapons = [["FirstPrimaryWeapon", "Holster"], ["SecondPrimaryWeapon", "Scabbard"]];
    let rect = |slot: &str| panel.slot(slot).unwrap_or_else(|| panic!("no view for {slot}"));
    let mut below = 0.0;
    for row in rows.iter().map(|row| row.to_vec()).chain(weapons.iter().map(|row| row.to_vec())) {
        // Left to right along the row, on one line, under the row before.
        for pair in row.windows(2) {
            let (a, b) = (rect(pair[0]), rect(pair[1]));
            assert!(a[0] + a[2] <= b[0] && a[1] == b[1], "{pair:?}");
        }
        assert!(rect(row[0])[1] >= below, "{row:?} lies over the row above");
        below = row.iter().map(|slot| rect(slot)[1] + rect(slot)[3]).fold(0.0, f32::max);
    }
    assert!(below <= panel.size[1] && rect("FaceCover")[0] + rect("FaceCover")[2] <= panel.size[0]);
    for [long, short] in weapons {
        assert!(rect(long)[2] > 2.0 * rect(short)[2], "{long} is the wide one");
    }
}

/// What the app's crate holds goes onto a bare character by the model's
/// own moves (inventory spec, "Moving things"): the stock rifle onto the
/// sling, a starting vest and backpack into their slots with all that is
/// in them, and the carried weight is then the three's. A magazine out of
/// the vest goes into the backpack, and the pockets stay where they are.
#[test]
fn a_crate_is_emptied_onto_a_bare_character() {
    use trp_data::inventory::Loadout;
    use trp_sim::inventory::{apply_item, move_item, Applied, Equipment, Item, Kind, Refusal, Rules, Spot};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let rules = Rules::of(&db);
    println!("backpacks lie in one another {} deep at most", rules.backpacks_deep);
    assert!(rules.in_raid && rules.backpacks_deep > 0);
    // The airdrop crate, the stock AKM, and a BEAR's starting vest and
    // backpack with the magazines and rounds changed for the AKM's own.
    let mut loot = Item::new(std::sync::Arc::new(Kind::of(&db, "6223349b3136504a544d1608").expect("the crate")));
    let (rifle, _) = Loadout::of_preset(&db, "59d6088586f774275f37482f").and_then(|list| Item::of_list(&db, &list)).expect("the rifle");
    assert!(!rifle.missing_vital(), "the stock rifle is whole");
    let mut kit = Loadout::load(&install, "Standard", "bear").expect("the starting character");
    let changed = kit.fit_to(&db, "59d6088586f774275f37482f").expect("the rifle's own magazine and round");
    let (mut outfit, left_out) = Equipment::of_loadout(&db, &kit).expect("its equipment");
    assert!(left_out.is_empty(), "{left_out:?}");
    // Every magazine and every round in the kit is now one the rifle takes.
    let rifle_kind = db.items.get("59d6088586f774275f37482f").expect("the rifle");
    let magazine_slot = rifle_kind.slots().into_iter().find(|slot| slot.name == "mod_magazine").expect("a magazine slot");
    let (mut magazines, mut rounds) = (0, 0);
    for item in &kit.items {
        let classes = db.class_names(&item.template);
        if classes.iter().any(|class| class == "Magazine") {
            assert!(magazine_slot.takes.contains(&item.template), "{}", classes[0]);
            magazines += 1;
        }
        if classes.iter().any(|class| class == "Ammo") {
            assert!(classes[0].contains("762x39"), "{}", classes[0]);
            rounds += item.count;
        }
    }
    println!("the kit: {} magazines and {} lots of rounds changed; {magazines} magazines the rifle takes and {rounds} rounds of its own in all", changed[0], changed[1]);
    assert!(changed[0] > 0 && changed[1] > 0);
    let (vest, pack, pockets) = (outfit.take_off("TacticalVest").expect("a vest"), outfit.take_off("Backpack").expect("a backpack"), outfit.take_off("Pockets").expect("pockets"));
    let weights = rifle.weight() + vest.weight() + pack.weight();
    let (rifle_id, vest_id, pack_id, pockets_id) = (rifle.id, vest.id, pack.id, pockets.id);
    for thing in [rifle, vest, pack] {
        let (name, cells) = (thing.kind.name.clone(), thing.cells());
        loot.stow(thing).unwrap_or_else(|_| panic!("the crate has no room for {name} ({cells:?})"));
    }
    let mut worn = Equipment::empty(&db).expect("the equipment's template").item;
    let pockets_slot = worn.slot_index("Pockets").expect("a slot for pockets");
    worn.slots[pockets_slot] = Some(pockets);
    let (worn_id, loot_id) = (worn.id, loot.id);
    let slot = |name: &str| Spot::Slot(worn.slot_index(name).expect("the slot"));
    let (sling, back, vest_slot, pack_slot, headwear) = (slot("FirstPrimaryWeapon"), slot("SecondPrimaryWeapon"), slot("TacticalVest"), slot("Backpack"), slot("Headwear"));
    let mut roots = [&mut worn, &mut loot];
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, headwear, &rules), Err(Refusal::Filter));
    assert_eq!(move_item(&mut roots, vest_id, worn_id, sling, &rules), Err(Refusal::Filter));
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, sling, &rules), Ok(()));
    assert_eq!(move_item(&mut roots, vest_id, worn_id, vest_slot, &rules), Ok(()));
    assert_eq!(move_item(&mut roots, pack_id, worn_id, pack_slot, &rules), Ok(()));
    assert!(roots[1].contents().next().is_none(), "the crate is empty");
    let carried = |roots: &[&mut Item]| ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "TacticalVest", "Backpack"].iter().filter_map(|slot| roots[0].in_slot(slot)).map(Item::weight).sum::<f32>();
    println!("carried from the crate: {:.3} kg", carried(&roots));
    assert!((carried(&roots) - weights).abs() < 1e-4);
    // From the sling to the back and again.
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, back, &rules), Ok(()));
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, sling, &rules), Ok(()));
    // Something out of the vest, dropped on the backpack, goes into it.
    let magazine = roots[0].find(vest_id).and_then(|vest| vest.grids.iter().flatten().next()).map(|(_, item)| item.id).expect("something in the vest");
    assert!(matches!(apply_item(&mut roots, magazine, pack_id, &rules), Ok(Applied::Put(Spot::Cell(..)))));
    assert_eq!(roots[0].holder_of(magazine).map(|(holder, _)| holder), Some(pack_id));
    assert!((carried(&roots) - weights).abs() < 1e-4);
    // The pockets are not taken off in a raid.
    let cell = Spot::Cell(0, trp_sim::inventory::Place { x: 0, y: 0, turned: false });
    assert_eq!(move_item(&mut roots, pockets_id, loot_id, cell, &rules), Err(Refusal::KeptOn("Pockets".into())));
}

/// The moves nobody drags (inventory spec, "Clicks, keys and windows"),
/// on the install's own items: the crate's three things are put on by
/// their kind, a magazine is sent to the crate and back to the vest, a
/// pick-up finds the place the game's order has for the thing, and a
/// quick-use key takes medicine in the pockets and nothing in a backpack.
#[test]
fn things_are_sent_put_on_and_bound_by_the_install_s_classes() {
    use trp_data::inventory::Loadout;
    use trp_sim::inventory::{bindable, can_pick_up, equip, is, loot_order, move_item, pick_up, quick_find, take_out, Applied, Bound, Equipment, Item, Kind, Order, Refusal, Rules, Spot};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let rules = Rules::of(&db);
    let mut loot = Item::new(std::sync::Arc::new(Kind::of(&db, "6223349b3136504a544d1608").expect("the crate")));
    let (rifle, _) = Loadout::of_preset(&db, "59d6088586f774275f37482f").and_then(|list| Item::of_list(&db, &list)).expect("the rifle");
    let mut kit = Loadout::load(&install, "Standard", "bear").expect("the starting character");
    kit.fit_to(&db, "59d6088586f774275f37482f").expect("the rifle's own magazine and round");
    let (mut outfit, _) = Equipment::of_loadout(&db, &kit).expect("its equipment");
    let (vest, pack, pockets) = (outfit.take_off("TacticalVest").expect("a vest"), outfit.take_off("Backpack").expect("a backpack"), outfit.take_off("Pockets").expect("pockets"));
    let (rifle_id, vest_id, pack_id, pockets_id) = (rifle.id, vest.id, pack.id, pockets.id);
    for thing in [rifle, vest, pack] {
        loot.stow(thing).expect("room in the crate");
    }
    let mut worn = Equipment::empty(&db).expect("the equipment's template").item;
    let pockets_slot = worn.slot_index("Pockets").expect("a slot for pockets");
    worn.slots[pockets_slot] = Some(pockets);
    let (worn_id, loot_id) = (worn.id, loot.id);
    let slot = |name: &str| worn.slot_index(name).expect("the slot");
    let (sling, back, vest_slot, pack_slot) = (slot("FirstPrimaryWeapon"), slot("SecondPrimaryWeapon"), slot("TacticalVest"), slot("Backpack"));
    let mut roots = [&mut worn, &mut loot];
    // Ctrl and a click on the rifle with nothing worn to hold it: the
    // character has no place, so it goes to the next room in the crate.
    let sent = quick_find(&mut roots, rifle_id, &[worn_id], Order::OTHER_SIDE, &rules);
    println!("the rifle sent from the crate to a bare character: {sent:?}");
    assert!(matches!(sent, Ok((into, Applied::Put(Spot::Cell(..)))) if into == loot_id));
    // Alt and a click: each onto the slot of its kind.
    assert_eq!(equip(&mut roots, rifle_id, &rules), Ok(sling));
    assert_eq!(equip(&mut roots, vest_id, &rules), Ok(vest_slot));
    assert_eq!(equip(&mut roots, pack_id, &rules), Ok(pack_slot));
    assert_eq!(equip(&mut roots, pack_id, &rules), Err(Refusal::AlreadyThere));
    assert!(roots[1].contents().next().is_none(), "the crate is empty");
    // A second rifle goes onto the back, a third nowhere.
    let another = || Loadout::of_preset(&db, "59d6088586f774275f37482f").and_then(|list| Item::of_list(&db, &list)).expect("the rifle").0;
    let (second, third) = (another(), another());
    let (second_id, third_id) = (second.id, third.id);
    roots[1].stow(second).unwrap();
    roots[1].stow(third).unwrap();
    assert_eq!(equip(&mut roots, second_id, &rules), Ok(back));
    assert_eq!(equip(&mut roots, third_id, &rules), Err(Refusal::NoRoom));
    // What the kit holds, by the game's classes.
    let all = |roots: &[&mut Item], holder: Id_| roots[0].find(holder).map(|holder| holder.grids.iter().flatten().map(|(_, item)| item.id).collect::<Vec<_>>()).unwrap_or_default();
    let of = |roots: &[&mut Item], class: &str| {
        let among = [vest_id, pack_id, pockets_id].into_iter().flat_map(|holder| all(roots, holder));
        among.into_iter().find(|id| roots[0].find(*id).is_some_and(|item| is(item, class)))
    };
    let magazine = of(&roots, "Magazine").expect("a magazine in the kit");
    let rounds = of(&roots, "Ammo").expect("rounds in the kit");
    let kit_id = of(&roots, "Meds").expect("medicine in the kit");
    let food = of(&roots, "FoodDrink").expect("food or drink in the kit");
    for id in [magazine, rounds, kit_id, food] {
        let item = roots[0].find(id).unwrap();
        let lies = roots[0].holder_of(id).and_then(|(holder, _)| roots[0].find(holder)).map(|holder| holder.kind.name.clone()).unwrap_or_default();
        println!("  {} (in {lies}) is looked for room in {:?}", item.kind.name, loot_order(item));
    }
    // Wherever the kit had them: the magazine in the vest and the
    // medicine in the backpack, to begin with.
    for (id, holder) in [(magazine, vest_id), (kit_id, pack_id)] {
        if roots[0].holder_of(id).map(|(holder, _)| holder) != Some(holder) {
            let thing = roots[0].find(id).unwrap();
            let room = (0..roots[0].find(holder).unwrap().kind.grids.len()).find_map(|grid| roots[0].find(holder)?.find_room(grid, thing).map(|place| Spot::Cell(grid, place)));
            assert_eq!(move_item(&mut roots, id, holder, room.expect("room for it"), &rules), Ok(()));
        }
    }
    assert_eq!(loot_order(roots[0].find(magazine).unwrap())[0], "TacticalVest");
    assert_eq!(loot_order(roots[0].find(kit_id).unwrap())[0], "Backpack");
    // The magazine to the crate (the crate is looked in first while it is
    // open) and back: to the vest, which is where magazines go.
    let sent = quick_find(&mut roots, magazine, &[loot_id, worn_id], Order::OTHER_SIDE, &rules);
    assert!(matches!(sent, Ok((into, Applied::Put(_))) if into == loot_id), "{sent:?}");
    let back_again = quick_find(&mut roots, magazine, &[worn_id], Order::OTHER_SIDE, &rules);
    assert!(matches!(back_again, Ok((into, Applied::Put(_))) if into == vest_id), "{back_again:?}");
    // The rounds in the backpack, sent with nothing open: the vest is
    // looked in before the backpack they lie in.
    let sent = quick_find(&mut roots, rounds, &[worn_id], Order::OTHER_SIDE, &rules);
    println!("rounds sent with nothing open: {sent:?}");
    assert!(sent.is_ok());
    // Put down and taken up again: a magazine goes back to the vest.
    let dropped = take_out(&mut roots, magazine, &rules).expect("a magazine is put down");
    assert!(can_pick_up(roots[0], &dropped, &rules));
    let (taken, left) = pick_up(roots[0], dropped, &rules);
    assert!(matches!(taken, Ok((into, Applied::Put(_))) if into == vest_id) && left.is_none(), "{taken:?}");
    // The quick-use keys: medicine in the backpack is not bound; in the
    // pockets it is, and a magazine never.
    let mut bound = Bound::default();
    assert!(!bindable(roots[0], kit_id) && !bound.bind(roots[0], kit_id, 0));
    let pocket = (0..4).find_map(|grid| roots[0].find(pockets_id)?.find_room(grid, roots[0].find(kit_id)?).map(|place| Spot::Cell(grid, place))).expect("a free pocket");
    assert_eq!(move_item(&mut roots, kit_id, pockets_id, pocket, &rules), Ok(()));
    assert!(bound.bind(roots[0], kit_id, 0) && !bound.bind(roots[0], magazine, 1));
    // The rifle on the sling is not bound to a key of these: its slot has
    // a key of its own.
    assert!(!bindable(roots[0], rifle_id));
    assert_eq!(bound.key_of(kit_id), Some(0));
}

type Id_ = trp_sim::inventory::Id;

/// The equipment list a recording's header carries from recorder 0.5.0 on
/// is read back and set beside the model (inventory spec, "Compared with
/// the running game"). No recording has one yet: the list here is made up
/// from a starting character in the recorder's own form, with the model's
/// figures where the game's would stand, so that it agrees with itself;
/// one figure is then spoilt, which has to show.
#[test]
fn a_recorded_equipment_list_is_read_back() {
    use trp_data::inventory::Loadout;
    use trp_sim::inventory::Equipment;
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let loadout = Loadout::load(&install, "Standard", "usec").expect("the starting character");
    let (equipment, _) = Equipment::of_loadout(&db, &loadout).expect("its equipment");
    // The recorder lists the equipment first and then what is in it, each
    // with what it is in, where, and the game's cells and weight for it.
    let mut entries: Vec<serde_json::Value> = Vec::new();
    let mut waiting = vec![(&equipment.item, None::<(String, String, Option<trp_sim::inventory::Place>)>)];
    while let Some((item, lies)) = waiting.pop() {
        let id = format!("item{}", item.id);
        let mut entry = serde_json::json!({ "id": id, "tpl": item.kind.template, "n": item.count, "w": item.cells()[0], "h": item.cells()[1], "kg": item.weight() });
        if let Some((holder, at, place)) = lies {
            entry["in"] = holder.into();
            entry["at"] = at.into();
            if let Some(place) = place {
                (entry["x"], entry["y"], entry["r"]) = (place.x.into(), place.y.into(), (place.turned as u32).into());
            }
        }
        entries.push(entry);
        for (slot, held) in item.kind.slots.iter().zip(&item.slots) {
            waiting.extend(held.as_ref().map(|held| (held, Some((id.clone(), slot.name.clone(), None)))));
        }
        for (grid, held) in item.kind.grids.iter().zip(&item.grids) {
            waiting.extend(held.iter().map(|(place, held)| (held, Some((id.clone(), grid.name.clone(), Some(*place))))));
        }
        waiting.extend(item.loose.iter().map(|held| (held, Some((id.clone(), "cartridges".to_owned(), None)))));
    }
    let list = serde_json::Value::Array(entries.clone());
    let (read, compared, left_out) = Equipment::of_recording(&db, &list).expect("the list");
    println!("{} entries read back, {} items compared, {:.3} kg carried", entries.len(), compared.len(), read.carried_weight(false));
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(compared.len(), entries.len() - 1);
    assert!(compared.iter().all(|item| item.agrees()));
    assert!((read.carried_weight(false) - equipment.carried_weight(false)).abs() < 1e-4);
    // A weight the game would have summed otherwise is found.
    entries[1]["kg"] = (entries[1]["kg"].as_f64().unwrap() + 0.25).into();
    let (_, compared, _) = Equipment::of_recording(&db, &serde_json::Value::Array(entries)).expect("the list");
    assert_eq!(compared.iter().filter(|item| !item.agrees()).count(), 1);
}

/// The sizes the game gave everything the character carried in a
/// recording with the Benelli M3's stock folded (`courses1`, the
/// Five-seveN's course, 2026-10-15: the M3 on the back 4 cells by 2, its
/// stock 1 by 2), set beside the model's with that stock folded
/// (inventory spec, "Grids"). The recording says nothing of a fold (the
/// recorder writes it from 0.10.2): only the stock's own size shows it,
/// and the weapon's is what this measures. Unfolded, the model has the
/// two at 5 and 2, as it did before 0.47.0.
#[test]
fn a_folded_stock_sizes_its_weapon_as_the_game_did() {
    use trp_sim::inventory::Equipment;
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses1/4-five-seven/five-seven.jsonl");
    let (Ok(_install), Ok(text)) = (crate::fixture::install().ok_or(()), std::fs::read_to_string(path)) else {
        eprintln!("skipped: no install or no recording");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let header: serde_json::Value = serde_json::from_str(text.lines().next().expect("a header")).expect("the header");
    let mut list = header["equipment"].clone();
    const STOCK: &str = "6259c3387d6aab70bc23a18d";
    let off = |list: &serde_json::Value| {
        let (_, compared, left_out) = Equipment::of_recording(&db, list).expect("the list");
        assert!(left_out.is_empty(), "{left_out:?}");
        compared.into_iter().filter(|item| !item.agrees()).map(|item| (item.name, item.cells)).collect::<Vec<_>>()
    };
    let unfolded = off(&list);
    assert_eq!(unfolded.len(), 2, "{unfolded:?}");
    assert!(unfolded.iter().all(|(_, cells)| cells[1][0] == cells[0][0] + 1));
    for entry in list.as_array_mut().expect("a list").iter_mut().filter(|entry| entry["tpl"] == STOCK) {
        entry["fold"] = 1.into();
    }
    assert_eq!(off(&list), Vec::new());
}

/// Level ground that ends in a drop: `height` metres down from `edge`
/// metres ahead on.
struct Ledge {
    edge: f32,
    height: f32,
}

impl trp_sim::World for Ledge {
    fn ground_height(&self, _x: f32, z: f32) -> f32 {
        if z < self.edge { 0.0 } else { -self.height }
    }
}

/// The game has no falling state: a character that walks off a ledge goes
/// on walking, with gravity on it from the frame the ground check finds
/// nothing, its motion dying away from a quarter second in, the sights
/// due down from half a second, and the fall's cap on the speed when it
/// lands (movement spec, "Falling").
#[test]
fn a_walk_off_a_ledge_falls_and_lands() {
    let Some(mut m) = movement(0.0) else { return };
    let ledge = Ledge { edge: 6.0, height: 2.5 };
    let walking = walk([0.0, 1.0]);
    let frame = 1.0 / 120.0;
    let (mut left, mut landed, mut sights_due) = (None, None, None);
    let (mut pace_off, mut cap_before) = (0.0, 0.0);
    let mut states = Vec::new();
    for index in 0..(8.0 / frame) as usize {
        let (was_grounded, before) = (m.grounded, m.position);
        m.step(frame, &walking, &ledge);
        let time = index as f32 * frame;
        if left.is_none() && was_grounded && !m.grounded {
            (left, pace_off, cap_before) = (Some((time, before)), m.ground_speed(), m.clamped_speed());
        }
        if let (Some((since, _)), None) = (left, landed) {
            if !states.contains(&m.state) {
                states.push(m.state);
            }
            if sights_due.is_none() && m.long_fall() {
                sights_due = Some((time - since, -m.position[1]));
            }
            if m.grounded {
                landed = Some((time, m.position));
                break;
            }
        }
    }
    let ((left_at, from), (landed_at, to)) = (left.expect("it never left the ground"), landed.expect("it never landed"));
    let (air, forward, drop) = (landed_at - left_at, to[2] - from[2], from[1] - to[1]);
    println!(
        "walked off at {pace_off:.2} m/s, {:.2} m out; in the air {air:.3} s, {forward:.2} m forward and {drop:.2} m down; states in the air {states:?}",
        from[2]
    );
    let (due_after, dropped) = sights_due.expect("the fall was long enough for the sights");
    println!("the sights are due down {due_after:.3} s into the fall, {dropped:.2} m down");
    // A drop of 2.5 m takes 0.71 s; the character had been riding up to
    // 8 cm over the ground.
    assert!(states == [State::Move], "{states:?}");
    assert!((0.68..0.76).contains(&air), "{air}");
    assert!((2.4..2.6).contains(&drop), "{drop}");
    // A quarter second at the walk's pace, then less and less.
    let unslowed = pace_off * air;
    assert!(forward > pace_off * 0.25 && forward < 0.8 * unslowed, "{forward} against {unslowed}");
    assert!((0.5..0.52).contains(&due_after), "{due_after}");

    // The landing caps the selected speed for a while.
    for _ in 0..3 {
        m.step(frame, &walking, &ledge);
    }
    let capped = m.clamped_speed();
    for _ in 0..(4.0 / frame) as usize {
        m.step(frame, &walking, &ledge);
    }
    println!("the speed setting allowed: {cap_before:.3} before, {capped:.3} on landing, {:.3} four seconds on", m.clamped_speed());
    assert!(capped < cap_before - 0.05, "{capped} against {cap_before}");
    assert!((m.clamped_speed() - cap_before).abs() < 1e-3);
    assert!(m.grounded && m.freefall_time == 0.0 && !m.long_fall());
}

/// A sprint off the range's ledge, 6 m up, held on after the landing: the
/// landing caps the speed to nought for a while, and the sprint's own limit
/// under it is the game's `Mathf.Clamp` with its bounds crossed (0.1, the
/// lower, wins), which Rust's clamp would stop the program on (it did, in
/// the app, 2026-10-15).
#[test]
fn a_sprint_off_a_high_ledge_lands_and_goes_on() {
    let Some(mut m) = movement(0.0) else { return };
    let ledge = Ledge { edge: 12.0, height: 6.0 };
    let frame = 1.0 / 120.0;
    // A walk first: a sprint pressed before moving is ignored.
    let walking = walk([0.0, 1.0]);
    for _ in 0..(0.5 / frame) as usize {
        m.step(frame, &walking, &ledge);
    }
    let sprinting = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    let (mut lowest, mut landed, mut sprinted_on) = (f32::MAX, None, 0);
    for _ in 0..(8.0 / frame) as usize {
        m.step(frame, &sprinting, &ledge);
        if landed.is_none() && m.grounded && m.position[1] < -5.9 {
            landed = Some(m.state);
        }
        if landed.is_some() {
            lowest = lowest.min(m.speed_limit());
            sprinted_on += (m.state == State::Sprint) as usize;
        }
    }
    println!("landed in {landed:?}; the speed allowed after it at least {lowest:.3}; {sprinted_on} frames of sprint after it; now {:?} at {:.2} m/s", m.state, m.ground_speed());
    assert_eq!(landed, Some(State::Sprint));
    assert!(lowest < 0.1, "{lowest}");
}

/// The legs as the body's animator poses them, for a host that draws the
/// body: a foot's path through a walk has no jumps in it from one frame to
/// the next. (Whether it is the game's path is for a recording with the
/// leg bones to say: recorder 0.4.4.)
#[test]
fn a_foot_moves_evenly_through_a_walk() {
    use trp_sim::math::{quat_mul, quat_rotate};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement settings");
    let chain = ["Root_Joint", "Base HumanPelvis", "Base HumanLThigh1", "Base HumanLThigh2", "Base HumanLCalf", "Base HumanLFoot"];
    let paths: Vec<String> = (1..=chain.len()).map(|n| chain[..n].join("/")).collect();
    let body = trp_sim::body::BodyData::load_with(&mut assets, &paths).expect("body animator");
    let mut m = Movement::new(settings, trp_sim::Body::new(&body, 5.0));
    let foot = |m: &Movement| -> [f32; 3] {
        let bones = m.body.drawn_bones();
        let (at, _) = bones.iter().fold(([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0]), |(at, turn), (position, rotation)| {
            let shifted = quat_rotate(turn, *position);
            (std::array::from_fn(|k| at[k] + shifted[k]), quat_mul(turn, *rotation))
        });
        at
    };
    let frame = 1.0 / 120.0;
    let run = |m: &mut Movement, seconds: f32, input: Input| -> Vec<[f32; 3]> {
        (0..(seconds / frame) as usize)
            .map(|_| {
                m.step(frame, &input, &Flat);
                foot(m)
            })
            .collect()
    };
    let report = |name: &str, path: &[[f32; 3]]| -> (f32, f32) {
        let steps: Vec<f32> = path.windows(2).map(|w| (0..3).map(|k| (w[1][k] - w[0][k]).powi(2)).sum::<f32>().sqrt()).collect();
        let changes: Vec<f32> = steps.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let mean = steps.iter().sum::<f32>() / steps.len() as f32;
        let most = steps.iter().cloned().fold(0.0, f32::max);
        let jump = changes.iter().cloned().fold(0.0, f32::max);
        let reach = |k: usize| {
            let (low, high) = path.iter().fold((f32::MAX, f32::MIN), |(low, high), p| (low.min(p[k]), high.max(p[k])));
            high - low
        };
        println!(
            "{name}: the left foot moves {:.2} mm a frame on average, {:.2} at most; the most that changes from one frame to the next is {:.2} mm; it ranges {:.0} mm sideways, {:.0} up and {:.0} along",
            mean * 1000.0, most * 1000.0, jump * 1000.0, reach(0) * 1000.0, reach(1) * 1000.0, reach(2) * 1000.0
        );
        (most, jump)
    };
    let still = run(&mut m, 3.0, Input::default());
    let (most_still, _) = report("standing", &still);
    run(&mut m, 2.0, walk([0.0, 1.0]));
    let walking = run(&mut m, 3.0, walk([0.0, 1.0]));
    let (most, jump) = report("walking", &walking);
    // Standing, the foot hardly moves; walking, no frame's move is out of
    // line with its neighbours'.
    assert!(most_still < 0.002, "{most_still}");
    assert!(jump < 0.5 * most, "{jump} against {most}");
}

/// The floor under a walk's speed comes from a curve the game keeps once
/// for the whole raid, keyed by whichever character's carried weight
/// changed last (movement spec, "The floor's curve is shared"). A crouched
/// walk goes at the floor, so its pace follows that key; an upright walk
/// only starts from it.
#[test]
fn a_crouched_walk_goes_at_the_shared_floor() {
    let pace = |keyed_by: Option<f32>, crouched: bool| -> Option<(f32, f32, f32)> {
        // An untrained character carrying 9 kg.
        let mut m = movement(9.0)?;
        if let Some(inertia) = keyed_by {
            m.settings.key_acceleration_range(inertia);
        }
        if crouched {
            m.step(DT, &Input { pose_delta: -1.0, ..Input::default() }, &Flat);
            run(&mut m, 1.5, Input::default());
        }
        let from = m.position;
        run(&mut m, 4.0, walk([0.0, 1.0]));
        let gone = ((m.position[0] - from[0]).powi(2) + (m.position[2] - from[2]).powi(2)).sqrt();
        Some((m.smoothed_speed, gone, m.settings.inertia))
    };
    let Some((own, own_gone, inertia)) = pace(None, true) else { return };
    let (laden, laden_gone, _) = pace(Some(1.0), true).unwrap();
    let (same, _, _) = pace(Some(inertia), true).unwrap();
    println!(
        "crouched, speed parameter and metres in 4 s: {own:.4} and {own_gone:.3} by the character's own key (inertia {inertia:.3}), {laden:.4} and {laden_gone:.3} by a fully laden character's"
    );
    assert!((same - own).abs() < 1e-6, "keyed by its own inertia: {same} against {own}");
    assert!(laden < own - 0.005 && laden_gone < own_gone - 0.02, "{laden} {own} {laden_gone} {own_gone}");
    let (upright, upright_gone, _) = pace(None, false).unwrap();
    let (upright_laden, upright_laden_gone, _) = pace(Some(1.0), false).unwrap();
    println!(
        "upright: {upright:.4} and {upright_gone:.3} by its own key, {upright_laden:.4} and {upright_laden_gone:.3} by a fully laden character's"
    );
    assert!((upright - upright_laden).abs() < 1e-4 && (upright_gone - upright_laden_gone).abs() < 0.005);

    // And the key is read back from the curve's value it gives.
    let Some(mut m) = movement(9.0) else { return };
    m.settings.key_acceleration_range(0.456);
    let value = m.settings.movement_acceleration_range.evaluate(m.settings.walk_inertia);
    let found = m.settings.acceleration_key_giving(value, 1e-4).expect("a key gives it");
    assert!((found - 0.456).abs() < 1e-3, "{found}");
}

/// The body's animation turns and bends the upper body to the view by
/// angles noted before the frame's own turn of the view, so the ribcage,
/// which the eye hangs on, goes round the feet a frame behind the view and
/// is at rest a frame after the view is (recorded: the tenth recording's
/// scenario 11, 1.6 mm sideways at 84 degrees a second on the frame the
/// view starts and on the frame it stops). `ribcage_posed_for` gives a
/// host the ribcage for whatever view it shows.
#[test]
fn the_ribcage_trails_the_view_by_a_frame() {
    let Some(mut m) = movement(0.0) else { return };
    let frame = 1.0 / 120.0;
    // Looking 80 degrees down, where the ribcage is far ahead of the feet.
    m.pitch = 80.0;
    m.note_rotation();
    for _ in 0..240 {
        m.frame(frame, &Input::default(), &Flat);
    }
    // Which way the ribcage lies from the feet, in the world, in degrees.
    let bearing = |m: &Movement, ribcage: [f32; 3]| ribcage[0].atan2(ribcage[2]).to_degrees() + m.body_yaw;
    let reach = |r: [f32; 3]| (r[0] * r[0] + r[2] * r[2]).sqrt();

    // Asked for the view the pose was made for, it is the ribcage as posed.
    let (as_posed, asked) = (m.ribcage(), m.ribcage_posed_for(m.posed_for()));
    let apart = (0..3).map(|k| (as_posed[k] - asked[k]).abs()).fold(0.0, f32::max);
    assert!(apart < 1e-5, "{apart}");

    // The view turns 300 degrees a second for ten frames: 2.5 degrees a
    // frame, 25 in all, short of what turns the feet.
    let turn = Input { look_delta: [2.5, 0.0], ..Input::default() };
    let mut trails = Vec::new();
    for _ in 0..10 {
        m.frame(frame, &turn, &Flat);
        // How far short of where the view as it stands would have it.
        trails.push(bearing(&m, m.ribcage_posed_for([m.yaw, m.pitch])) - bearing(&m, m.ribcage()));
    }
    let turning = trails[3..].iter().sum::<f32>() / (trails.len() - 3) as f32;
    m.frame(frame, &Input::default(), &Flat);
    let stopped = bearing(&m, m.ribcage_posed_for([m.yaw, m.pitch])) - bearing(&m, m.ribcage());
    println!(
        "looking 80 degrees down with the ribcage {:.2} m ahead of the feet, the view turning 2.5 degrees a frame: the ribcage trails it by {turning:.2} degrees ({:.0} mm); a frame after the view stops, by {stopped:.2}",
        reach(m.ribcage()),
        reach(m.ribcage()) * turning.to_radians() * 1000.0
    );
    assert_eq!(m.state, State::Idle);
    assert!((turning - 2.5).abs() < 0.3, "{turning}");
    assert!(stopped.abs() < 0.3, "{stopped}");
}

/// Level ground with boxes on it, each endlessly wide and standing from
/// `near` to `far` metres ahead and from `low` to `high`, answering as a
/// host's world does: a line sees a box only from outside, a box's top is
/// ground, and the character is kept its capsule's reach from a box's
/// faces unless its top is a step up at most or lower than the capsule's
/// underside.
struct Yard(Vec<[f32; 4]>);

impl trp_sim::World for Yard {
    fn ground_height(&self, _x: f32, z: f32) -> f32 {
        self.0.iter().filter(|b| z >= b[0] && z <= b[1]).map(|b| b[3]).fold(0.0, f32::max)
    }

    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let mut nearest: Option<f32> = (from[1] >= 0.0 && to[1] < 0.0).then(|| from[1] / (from[1] - to[1]));
        for [near, far, low, high] in &self.0 {
            let (mut enter, mut leave, mut misses) = (0.0f32, 1.0f32, false);
            for (k, (a, b)) in [(2, (*near, *far)), (1, (*low, *high))] {
                let (start, travel) = (from[k], to[k] - from[k]);
                if travel.abs() < 1e-9 {
                    misses |= start <= a || start >= b;
                    continue;
                }
                let (t0, t1) = ((a - start) / travel, (b - start) / travel);
                enter = enter.max(t0.min(t1));
                leave = leave.min(t0.max(t1));
            }
            if !misses && enter <= leave && enter > 0.0 {
                nearest = Some(nearest.map_or(enter, |n| n.min(enter)));
            }
        }
        nearest.map(|share| share * length)
    }

    fn terrain(&self, at: [f32; 3]) -> bool {
        at[1] <= 1e-3
    }

    /// The character is kept out of the boxes as the game's own controller
    /// keeps it (movement spec, "The character's own collision").
    fn obstructed(&self, from: [f32; 3], to: [f32; 3], capsule: trp_sim::movement::Capsule) -> [f32; 3] {
        let settled = self.course().settle([from[2], from[1]], [to[2], to[1]], capsule, CAPSULE_RADIUS);
        [to[0], settled[1], settled[0]]
    }

    fn sets_feet(&self) -> bool {
        true
    }

    fn held(&self, at: [f32; 3], capsule: trp_sim::movement::Capsule) -> bool {
        self.course().held([at[2], at[1]], capsule, CAPSULE_RADIUS)
    }
}

/// The game's capsule, from the player prefab.
const CAPSULE_RADIUS: f32 = 0.366;

impl Yard {
    /// The yard as a line of ground: its boxes, which stand on the ground
    /// one after another, and the ground between them.
    fn course(&self) -> trp_sim::collision::Course {
        use trp_sim::collision::Tread;
        let mut boxes: Vec<[f32; 4]> = self.0.clone();
        boxes.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let (mut treads, mut at) = (Vec::new(), f32::MIN);
        for [near, far, _, high] in boxes {
            treads.push(Tread::level(at, near, 0.0));
            treads.push(Tread::level(near, far, high));
            at = far;
        }
        treads.push(Tread::level(at, f32::MAX, 0.0));
        trp_sim::collision::Course { treads }
    }
}

/// Runs a frame at a time at the recordings' rate until `done` says so or
/// `seconds` have passed; gives back the states passed through, in order,
/// and how long it took.
fn until(m: &mut Movement, world: &dyn trp_sim::World, input: Input, seconds: f32, mut done: impl FnMut(&Movement) -> bool) -> (Vec<State>, f32) {
    let frame = 1.0 / 120.0;
    let mut states = vec![m.state];
    for index in 0..(seconds / frame) as usize {
        m.step(frame, &input, world);
        if states.last() != Some(&m.state) {
            states.push(m.state);
        }
        if done(m) {
            return (states, (index + 1) as f32 * frame);
        }
    }
    (states, seconds)
}

/// A vault and a climb sound their marks (footsteps spec, "The vault's
/// sounds"): the moves' states pass their four marks in the order and at
/// the spacing the body's states have them (a climb's grab before its
/// start), and each asks the surface's element at the obstacle's height,
/// with the move's kit bank at the grab and at the end. The game's own
/// sound is not compared here: the `courses2` vault recording is set
/// beside the rules in the spec.
#[test]
fn a_vault_and_a_climb_sound_their_marks() {
    use trp_sim::steps::{BankOf, Play};
    let held = Input { move_dir: [0.0, 1.0], vault: true, ..Input::default() };
    let frame = 1.0 / 120.0;
    let gather = |m: &mut Movement, world: &dyn trp_sim::World| -> Vec<(f32, Play)> {
        let mut asked = Vec::new();
        let mut clock = 0.0;
        for _ in 0..(5.0 / frame) as usize {
            m.step(frame, &held, world);
            clock += frame;
            asked.extend(m.sounds.take().into_iter().filter(|play| matches!(play.bank, BankOf::Vault { .. } | BankOf::VaultGear(_))).map(|play| (clock, play)));
        }
        asked
    };
    let marks = |asked: &[(f32, Play)]| -> Vec<(usize, Option<usize>)> {
        asked.iter()
            .map(|(_, play)| match play.bank {
                BankOf::Vault { kind, mark, .. } => (kind, Some(mark)),
                BankOf::VaultGear(kind) => (kind, None),
                _ => unreachable!(),
            })
            .collect()
    };

    // The wall of the vault above: start, the kit and the grab, the
    // slide, the kit and the end, at 0.10, 0.15, 0.30 and 0.55 of the
    // state at its pace.
    let Some(mut m) = movement(0.0) else { return };
    let wall = Yard(vec![[0.5, 0.85, 0.0, 1.0]]);
    let asked = gather(&mut m, &wall);
    println!("the vault's marks: {:?}", asked.iter().map(|(t, play)| format!("{t:.3} {:?} {:.3}", play.bank, play.volume)).collect::<Vec<_>>());
    assert_eq!(marks(&asked), [(0, Some(0)), (0, None), (0, Some(1)), (0, Some(2)), (0, None), (0, Some(3))], "{asked:?}");
    let (figures, pace, _) = m.vault_parameters();
    let height = figures[0];
    for (_, play) in &asked {
        match play.bank {
            BankOf::Vault { surface, .. } => {
                assert_eq!(surface, trp_sim::steps::CONCRETE);
                assert!((play.volume - height).abs() < 1e-5, "the height goes as the volume: {} against {height}", play.volume);
            }
            _ => assert!((play.volume - (0.1 + 0.9 * height.clamp(0.0, 1.0))).abs() < 1e-5, "the kit's share: {}", play.volume),
        }
        assert_eq!(play.blend, 0.0);
    }
    // The state's length as played is its clip's at this height (the
    // blend tree's `Jump_over_100` here) at the pace: taken from the
    // start-to-end gap, 0.45 of it, and the other marks set against that.
    let gap = |a: usize, b: usize| asked[b].0 - asked[a].0;
    let state_length = gap(0, 5) / 0.45;
    println!("the vault's state plays {state_length:.3} s at a pace of {pace:.3}");
    assert!((0.9..2.0).contains(&state_length), "{state_length}");
    assert!((gap(0, 2) - 0.05 * state_length).abs() < 2.5 * frame, "start to grab {:.3} against {:.3}", gap(0, 2), 0.05 * state_length);
    assert!((gap(0, 3) - 0.20 * state_length).abs() < 2.5 * frame, "start to slide {:.3} against {:.3}", gap(0, 3), 0.20 * state_length);
    assert_eq!(gap(1, 2), 0.0, "the kit and the grab on one frame");
    assert_eq!(gap(4, 5), 0.0, "the kit and the end on one frame");

    // The box of the climb: the kit and the grab first (0.25), then the
    // start (0.30), the slide (0.35), the kit and the end (0.50).
    let Some(mut m) = movement(0.0) else { return };
    let crate_ = Yard(vec![[0.5, 2.5, 0.0, 1.0]]);
    let asked = gather(&mut m, &crate_);
    println!("the climb's marks: {:?}", asked.iter().map(|(t, play)| format!("{t:.3} {:?} {:.3}", play.bank, play.volume)).collect::<Vec<_>>());
    assert_eq!(marks(&asked), [(2, None), (2, Some(1)), (2, Some(0)), (2, Some(2)), (2, None), (2, Some(3))], "{asked:?}");
    let (_, pace, _) = m.vault_parameters();
    let gap = |a: usize, b: usize| asked[b].0 - asked[a].0;
    let state_length = gap(1, 5) / 0.25;
    println!("the climb's state plays {state_length:.3} s at a pace of {pace:.3}");
    assert!((0.9..2.5).contains(&state_length), "{state_length}");
    assert!((gap(1, 2) - 0.05 * state_length).abs() < 2.5 * frame, "grab to start {:.3} against {:.3}", gap(1, 2), 0.05 * state_length);
    assert!((gap(1, 3) - 0.10 * state_length).abs() < 2.5 * frame, "grab to slide {:.3} against {:.3}", gap(1, 3), 0.10 * state_length);
    assert_eq!(gap(0, 1), 0.0, "the kit and the grab on one frame");
    assert_eq!(gap(4, 5), 0.0, "the kit and the end on one frame");
}

/// A wall is vaulted and a box climbed, by the game's own clips (vaulting
/// spec). Nothing here is compared with the game: no recording has one.
#[test]
fn a_wall_is_vaulted_and_a_box_climbed() {
    let Some(mut m) = movement(0.0) else { return };
    let held = Input { move_dir: [0.0, 1.0], vault: true, ..Input::default() };
    let rest = |m: &Movement| !m.is_vaulting && matches!(m.state, State::Idle | State::Move);

    // A wall a metre high and 0.35 m thick, its face half a metre ahead:
    // from standing, with forward and the key held.
    let wall = Yard(vec![[0.5, 0.85, 0.0, 1.0]]);
    let (legs, arms) = (m.stamina.legs.current, m.stamina.arms.current);
    let mut highest = 0.0f32;
    let (states, _) = until(&mut m, &wall, held, 1.0, |m| m.state == State::Vault);
    assert_eq!(states.last(), Some(&State::Vault), "{states:?}; measured {:?}", m.vaulting.obstacle);
    let started = m.position;
    println!(
        "the wall as measured: {:?}; the arms pay {:.1}, the legs {:.1}",
        m.vaulting.obstacle,
        arms - m.stamina.arms.current,
        legs - m.stamina.legs.current
    );
    assert!((arms - m.stamina.arms.current - 5.0).abs() < 0.05, "a hand is taken over 0.65 m");
    assert_eq!(legs, m.stamina.legs.current, "standing and unburdened, the legs pay nothing");
    assert!(!m.can_press_trigger() && m.take_aim_taken(), "no trigger, and the sights are taken down");
    let capsule = m.capsule();
    println!("the capsule in the vault: {:.3} to {:.3} m over the feet", capsule.underside, capsule.top);
    assert!(capsule.underside > 1.0, "clear of the wall's top: {capsule:?}");
    // The view does not answer the mouse while the vault's state lasts.
    let (yaw, pitch) = (m.yaw, m.pitch);
    let turning = Input { look_delta: [2.0, 1.0], ..held };
    let mut turned_in_vault = 0.0f32;
    let (states, took) = until(&mut m, &wall, turning, 4.0, |m| {
        highest = highest.max(m.position[1]);
        if m.state == State::Vault {
            turned_in_vault = (m.yaw - yaw).abs().max((m.pitch - pitch).abs());
        }
        rest(m)
    });
    println!(
        "the vault: {took:.2} s through {states:?}; {:.2} m forward, the feet up to {highest:.3} m; ends at {:.2} m, {:.3} m up",
        m.position[2] - started[2],
        m.position[2],
        m.position[1]
    );
    assert_eq!(states[..2], [State::Vault, State::VaultLanding], "{states:?}");
    assert!(m.position[2] > 0.85 + 0.3, "beyond the wall: {:?}", m.position);
    assert!(m.position[1].abs() < 0.09 && highest < 0.3, "the feet hardly leave the ground's level: {highest}");
    assert_eq!(turned_in_vault, 0.0, "the view is the move's own");
    assert!((m.yaw - yaw).abs() > 1.0, "and free again after it: {}", m.yaw - yaw);
    assert!(m.can_press_trigger());

    // A box a metre high and two deep: climbed, onto its top.
    let Some(mut m) = movement(0.0) else { return };
    let crate_ = Yard(vec![[0.5, 2.5, 0.0, 1.0]]);
    let arms = m.stamina.arms.current;
    let (states, _) = until(&mut m, &crate_, held, 1.0, |m| m.state == State::Climb);
    assert_eq!(states.last(), Some(&State::Climb), "{states:?}; measured {:?}", m.vaulting.obstacle);
    assert!((arms - m.stamina.arms.current - 8.0).abs() < 0.05, "one hand: {}", arms - m.stamina.arms.current);
    let from = m.position;
    let (states, took) = until(&mut m, &crate_, held, 4.0, rest);
    println!(
        "the climb: {took:.2} s through {states:?}; {:.2} m forward and {:.3} m up, to {:.2} m short of the box's face",
        m.position[2] - from[2],
        m.position[1] - from[1],
        0.5 - m.position[2]
    );
    // It comes to rest on the edge, and walks on from there.
    assert!((m.position[1] - 1.0).abs() < 0.1 && m.position[2] > 0.5 - 0.366, "on the box's edge: {:?}", m.position);
    until(&mut m, &crate_, held, 0.5, |_| false);
    // (Within the ground check of its top: nothing pulls a character down
    // that is.)
    assert!((m.position[1] - 1.0).abs() < 0.08 && m.position[2] > 0.5 && m.grounded, "on the box: {:?}", m.position);

    // A low kerb of a wall, 0.4 m high: vaulted with the hands on the
    // weapon, the view free, and at no cost.
    let Some(mut m) = movement(0.0) else { return };
    let low = Yard(vec![[0.5, 0.8, 0.0, 0.4]]);
    let (legs, arms) = (m.stamina.legs.current, m.stamina.arms.current);
    let (states, _) = until(&mut m, &low, held, 1.0, |m| m.state == State::Vault);
    assert_eq!(states.last(), Some(&State::Vault), "{states:?}; measured {:?}", m.vaulting.obstacle);
    assert!(m.can_press_trigger() && !m.take_aim_taken());
    assert_eq!((legs, arms), (m.stamina.legs.current, m.stamina.arms.current));
    let yaw = m.yaw;
    m.step(1.0 / 120.0, &Input { look_delta: [2.0, 0.0], ..held }, &low);
    assert!((m.yaw - yaw - 2.0).abs() < 1e-3, "the view turns: {}", m.yaw - yaw);
    let (states, took) = until(&mut m, &low, held, 4.0, rest);
    println!("the low vault: {took:.2} s through {states:?}; ends {:.2} m ahead, {:.3} m up", m.position[2], m.position[1]);
    assert!(m.position[2] > 0.8, "{:?}", m.position);
}

/// Medicine in hand, or a grenade's throw held: a wall that needs a hand is
/// not vaulted, and with no move measured before, nothing is (vaulting
/// spec, "What a try does").
#[test]
fn hands_full_refuse_a_vault_that_needs_a_hand() {
    let Some(mut m) = movement(0.0) else { return };
    let held = Input { move_dir: [0.0, 1.0], vault: true, hands_limit_vault: true, ..Input::default() };
    let wall = Yard(vec![[0.5, 0.85, 0.0, 1.0]]);
    let (states, _) = until(&mut m, &wall, held, 1.5, |m| m.state == State::Vault);
    assert!(!states.contains(&State::Vault), "{states:?}");
    // The hands free again: over it.
    let free = Input { hands_limit_vault: false, ..held };
    let (states, _) = until(&mut m, &wall, free, 1.5, |m| m.state == State::Vault);
    assert_eq!(states.last(), Some(&State::Vault), "{states:?}");
}

/// A sprint at a wall with the key held goes over it and on, and the key
/// held past its time does not jump when let go.
#[test]
fn a_sprint_vaults_and_a_long_hold_does_not_jump() {
    let Some(mut m) = movement(0.0) else { return };
    let frame = 1.0 / 120.0;
    // Up to a sprint on open ground, then a wall 0.9 m high across the way.
    let open = Yard(vec![]);
    until(&mut m, &open, walk([0.0, 1.0]), 0.5, |_| false);
    let sprint = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    let (states, _) = until(&mut m, &open, sprint, 4.0, |m| m.state == State::Sprint);
    assert_eq!(states.last(), Some(&State::Sprint));
    until(&mut m, &open, sprint, 1.5, |_| false);
    let wall = Yard(vec![[m.position[2] + 3.0, m.position[2] + 3.3, 0.0, 0.9]]);
    let far = m.position[2] + 3.3;
    let keyed = Input { vault: true, ..sprint };
    let (states, took) = until(&mut m, &wall, keyed, 3.0, |m| m.state == State::Vault);
    assert_eq!(states.last(), Some(&State::Vault), "{states:?}; measured {:?}", m.vaulting.obstacle);
    println!(
        "a sprint meets the wall {took:.2} s on, {:.2} m short of it at {:.2} m/s: {:?}",
        far - 0.3 - m.position[2],
        m.ground_speed(),
        m.vaulting.obstacle
    );
    let (states, took) = until(&mut m, &wall, sprint, 4.0, |m| m.state == State::Sprint);
    println!("the sprint's vault: {took:.2} s through {states:?}; {:.2} m past the wall", m.position[2] - far);
    assert_eq!(states.last(), Some(&State::Sprint), "the sprint is taken up again: {states:?}");
    assert!(m.position[2] > far && !m.is_vaulting);

    // The key alone: a tap jumps when it is let go, a long hold does not.
    let Some(mut m) = movement(0.0) else { return };
    let (key, release) = (Input { vault: true, ..Input::default() }, Input { jump: true, ..Input::default() });
    for _ in 0..12 {
        m.step(frame, &key, &Flat);
    }
    assert_eq!(m.state, State::Idle, "held, the key does not jump");
    m.step(frame, &release, &Flat);
    let (states, _) = until(&mut m, &Flat, Input::default(), 0.2, |m| m.state == State::Jump);
    assert_eq!(states.last(), Some(&State::Jump), "a tap of a tenth of a second jumps on release");
    let Some(mut m) = movement(0.0) else { return };
    for _ in 0..90 {
        m.step(frame, &key, &Flat);
    }
    m.step(frame, &release, &Flat);
    let (states, _) = until(&mut m, &Flat, Input::default(), 0.3, |m| m.state == State::Jump);
    assert_eq!(states, [State::Idle], "held three quarters of a second, it does not");
}

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

/// The game's empty hands are read from the install like a weapon: a rig
/// with both arms and the hand markers the arms are solved onto, clips of
/// its own, an animator whose resting state blends two of them by how far
/// down the view points, and settings for the weapon motion that hold no
/// weapon's figures.
#[test]
fn empty_hands_are_read_like_a_weapon() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let hands = trp_data::weapon::assemble_prefab(&mut assets, trp_data::weapon::EMPTY_HANDS).expect("the empty hands' prefab");
    let has = |path: &str| (0..hands.hierarchy.nodes.len()).any(|i| hands.hierarchy.path(i) == path);
    for bone in [
        "Base HumanLCollarbone/Base HumanLUpperarm",
        "Base HumanRCollarbone/Base HumanRUpperarm",
        "Camera_animated",
        "Weapon_root/Weapon_root_anim/weapon/weapon_L_IK_marker",
        "Weapon_root/Weapon_root_anim/weapon/weapon_R_IK_marker",
        "Weapon_root/Weapon_root_anim/weapon/Bend_Goal_Left",
    ] {
        assert!(has(bone), "the rig has no {bone}");
    }
    assert!(hands.hierarchy.nodes.iter().all(|node| node.renderer.is_none()), "the rig draws nothing of its own");

    let table = trp_data::states::StateTable::load(&assets, &hands).expect("the empty hands' animator");
    let state = |name: &str| table.find(name).unwrap_or_else(|| panic!("no state {name}"));
    let rest = state("Hands.IDLE");
    assert_eq!(rest.blend_parameter.as_deref(), Some("Aim_angle"));
    assert_eq!(rest.thresholds, [0.0, 90.0]);
    let names: Vec<&str> = rest.clips.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["empty_hands_idle", "empty_hands_idle_down"]);
    assert!(state("Hands.SPAWN").transition_to("Hands.OUT TO IDLE").is_some());
    assert!(table.find("Hands.IN INVENTORY").is_none(), "the empty hands have no pose for an open inventory");
    let appear = state("Hands.OUT TO IDLE");
    let length = trp_data::states::clip_length(&assets, appear.clips[0].1).expect("clip");
    println!(
        "empty hands: {} bones, {} clips; they appear in {length:.3} s and are there {:.3} s in",
        hands.hierarchy.nodes.len(),
        hands.clips(&assets).expect("clips").len(),
        appear.event_time("WeapIn").unwrap_or(f32::NAN) * length
    );

    let data = trp_data::player::HandsData::load(&mut assets).expect("the player prefab's tuning");
    let prefab_center = data.rotation_center;
    let settings = trp_sim::empty_hands_settings(data, &db, &hard, &engine, &hands, trp_sim::ViewSettings::default()).expect("settings");
    assert_eq!((settings.weapon_weight, settings.weapon_length), (0.0, 0.0));
    assert_eq!(settings.rotation_center, prefab_center);
    println!("the prefab's own swing point {prefab_center:?}, position spring damping {}", settings.hand_damping);
    // The motion runs with them: a few seconds standing still stay finite.
    let mut motion = trp_sim::Hands::new(settings);
    for _ in 0..300 {
        motion.advance(DT, &trp_sim::HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..Default::default() });
    }
    let (at, turn) = motion.weapon_transform();
    assert!(at.iter().chain(turn.iter()).all(|v| v.is_finite()), "{at:?} {turn:?}");
}

/// The body's animator is told of an open inventory as it is of a reload:
/// lying down, the view does not turn while it is open.
#[test]
fn an_open_inventory_holds_a_lying_body_as_a_reload_does() {
    let Some(mut m) = movement(0.0) else { return };
    m.step(DT, &Input { prone: true, ..Input::default() }, &Flat);
    run(&mut m, 2.5, Input::default());
    assert_eq!(m.state, State::ProneIdle);
    let open = Input { inventory: true, ..Input::default() };
    run(&mut m, 0.5, open);
    let before = m.yaw;
    m.step(DT, &Input { look_delta: [9.0, 0.0], ..open }, &Flat);
    assert_eq!(m.yaw, before);
    run(&mut m, 0.5, Input::default());
    m.step(DT, &Input { look_delta: [9.0, 0.0], ..Input::default() }, &Flat);
    assert!(m.yaw > before);
}

/// Told that the hands are empty, the body's animator plays the sprint's
/// own clip for empty hands in the layer that has the arms: both hands are
/// then the body's, shoulder to palm, and the left hand's fingers, which go
/// with the hand, are closed (body-animator spec, "The kind of thing in
/// hand"). With the rifle the hands stay the weapon's. The figure the
/// trees go by follows the new kind over most of a second. (A firearm one
/// cell wide is given the same clip, so telling the animator the width the
/// game's empty hands tell it had the same pose before the kind was told.)
#[test]
fn empty_hands_are_fists_in_a_sprint() {
    use trp_sim::body::hands_kind;
    let (Some(mut rifle), Some(mut empty)) = (movement(0.0), movement(0.0)) else { return };
    // The game's empty hands tell the animator a width of one cell.
    empty.body.set_weapon_width(1.0);
    empty.body.set_hands_kind(hands_kind::EMPTY_HANDS);
    empty.step(DT, &Input::default(), &Flat);
    let first = empty.body.hands_kind();
    assert!((first - 4.0 * 7.0 * DT).abs() < 1e-5, "one frame on: {first}");
    let mut frames = 1;
    while empty.body.hands_kind() != hands_kind::EMPTY_HANDS {
        empty.step(DT, &Input::default(), &Flat);
        frames += 1;
        assert!(frames < 600);
    }
    let sprint = Input { sprint: true, ..walk([0.0, 1.0]) };
    for m in [&mut rifle, &mut empty] {
        run(m, 0.5, walk([0.0, 1.0]));
        run(m, 3.0, sprint);
        assert_eq!(m.state, State::Sprint);
    }
    // How far a joint is bent from straight, in degrees.
    let bend = |q: [f32; 4]| 2.0 * q[3].abs().min(1.0).acos().to_degrees();
    // The index finger's three joints follow the thumb's.
    let index = |m: &Movement| {
        let fingers = m.arm_pose(false).fingers.expect("the left hand's fingers");
        [bend(fingers[3]), bend(fingers[4]), bend(fingers[5])]
    };
    let hands = |m: &Movement| [m.arm_pose(false).hand, m.arm_pose(true).hand];
    println!(
        "the animator's figure for empty hands: {first:.3} after a frame, 4 after {frames} frames ({:.2} s)",
        frames as f32 * DT
    );
    println!(
        "sprinting, the body's share of the hands (left, right): a rifle {:?}, empty hands {:?}",
        hands(&rifle),
        hands(&empty)
    );
    println!("the left index finger's joints, degrees: a rifle {:.0?}, empty hands {:.0?}", index(&rifle), index(&empty));
    assert_eq!(hands(&rifle), [0.0, 0.0]);
    assert_eq!(hands(&empty), [1.0, 1.0]);
    assert!(index(&empty)[1] > 100.0 && index(&rifle)[1] < 90.0);
    // Standing again, the hands are the rig's own once more.
    run(&mut empty, 2.0, Input::default());
    assert_eq!(empty.state, State::Idle);
    assert_eq!(hands(&empty), [0.0, 0.0]);
}

/// The weapon's controller on the engine, by the game's own calls for a
/// weapon being drawn (firing spec, "The weapon's animator as a state
/// machine"): `SPAWN`, the draw, rest; then the inventory's flag, by which
/// the hands go down, are held and come back.
#[test]
fn the_weapons_animator_runs_as_a_state_machine() {
    use trp_sim::weapon_animator::{WeaponAnimator, WeaponAnimatorData};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    println!("{} clips; FIRE's marks: {:?}", data.clip_count(), data.marks_of("Hands.FIRE"));
    let mut a = WeaponAnimator::new(&data);
    assert_eq!(a.state().0, "Hands.SPAWN");
    // As `SpawnOperation` and `SetAnimatorValues` leave it for a loaded rifle.
    a.set_flag("Active", true);
    a.set("AmmoInChamber", 1.0);
    a.set("AmmoInMag", 30.0);
    a.set_flag("MagInWeapon", true);
    const FRAME: f32 = 1.0 / 120.0;
    let mut log: Vec<(f32, String)> = Vec::new();
    let mut last = String::new();
    let mut t = 0.0;
    let mut run = |a: &mut WeaponAnimator, seconds: f32, log: &mut Vec<(f32, String)>| {
        for _ in 0..(seconds / FRAME).round() as usize {
            a.update(FRAME);
            t += FRAME;
            let (state, _, next) = a.state();
            let now = format!("{state}{}", next.map_or(String::new(), |next| format!(" -> {next}")));
            if now != last {
                log.push((t, now.clone()));
                last = now;
            }
            for mark in a.deliver() {
                log.push((t, format!("mark {} {}", mark.name, mark.text)));
            }
        }
    };
    run(&mut a, 1.5, &mut log);
    assert_eq!(a.state().0, "Hands.IDLE");
    a.set_flag("Inventory", true);
    run(&mut a, 1.5, &mut log);
    assert_eq!(a.state().0, "Hands.IN INVENTORY");
    a.set_flag("Inventory", false);
    run(&mut a, 1.5, &mut log);
    assert_eq!(a.state().0, "Hands.IDLE");
    for (at, what) in &log {
        println!("{at:6.3} s  {what}");
    }
    let at = |what: &str| log.iter().find(|(_, entry)| entry == what).map(|(at, _)| *at);
    // The draw is 0.967 s long and hands over to rest at 0.98 of it.
    let rest = at("Hands.OUT TO IDLE -> Hands.IDLE").expect("the draw blends into rest");
    assert!((rest - 0.967 * 0.98).abs() < 0.02, "{rest}");
    assert!(at("mark WeapIn ").is_some() && at("mark IdleStart ").is_some());
    // The way down: 0.333 s of `OPEN INVENTORY`, then the held state.
    let down = at("Hands.IDLE -> Hands.OPEN INVENTORY").expect("the hands go down");
    let held = at("Hands.OPEN INVENTORY -> Hands.IN INVENTORY").expect("and are held");
    assert!((held - down - 0.333).abs() < 0.03, "{}", held - down);
}

/// The weapon's logic going by the weapon's own animator, with the
/// install's AKM (firing spec, "The weapon goes by its animator"): the
/// draw and the put-away as the animator's states, a reload that ends in
/// the hold of an open inventory without the hands going back to the
/// weapon, from a chambered round and from an empty chamber, a reload
/// asked for in a shot, a weapon drawn with its chamber empty; and the
/// empty hands on their own animator.
#[test]
fn the_weapon_goes_by_its_animator() {
    use trp_sim::weapon_animator::WeaponAnimatorData;
    use trp_sim::{EmptyHands, Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    const FRAME: f32 = 1.0 / 120.0;
    // Every state the animator enters over some seconds, with when, and
    // whatever the weapon says of itself.
    let run = |w: &mut Firearm, seconds: f32| -> (Vec<(String, f32)>, Vec<(FirearmEvent, f32)>) {
        let name = |w: &Firearm| {
            let (state, _, next) = w.animator().state();
            next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
        };
        let (mut entered, mut said, mut last) = (Vec::new(), Vec::new(), name(w));
        for frame in 0..(seconds / FRAME).round() as usize {
            let at = (frame + 1) as f32 * FRAME;
            said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_))).map(|e| (e, at)));
            let now = name(w);
            if now != last {
                entered.push((now.clone(), at));
                last = now;
            }
        }
        (entered, said)
    };
    let names = |entered: &[(String, f32)]| entered.iter().map(|entry| entry.0.clone()).collect::<Vec<_>>();
    let when = |said: &[(FirearmEvent, f32)], what: &FirearmEvent| said.iter().find(|(event, _)| event == what).map(|(_, at)| *at);

    // Drawn, at rest, put away.
    let mut w = Firearm::new(settings.clone(), &data);
    w.draw();
    assert!(w.hands_busy() && w.occupied());
    let (entered, said) = run(&mut w, 1.5);
    let appeared = when(&said, &FirearmEvent::Appeared).expect("the weapon appears");
    println!("drawn: {entered:?}; appeared {appeared:.3} s after the draw was asked for");
    assert_eq!(names(&entered), ["OUT TO IDLE", "IDLE"]);
    // The draw's state is 0.967 s long and gives way at 0.98 of it.
    assert!((appeared - 0.967 * 0.98).abs() < 0.03, "{appeared}");
    assert!(w.is_idle() && !w.hands_busy());
    w.hide(false);
    let (entered, said) = run(&mut w, 1.5);
    let hidden = when(&said, &FirearmEvent::Hidden).expect("the weapon goes");
    println!("put away: {entered:?}; hidden {hidden:.3} s after it was asked for");
    assert_eq!(names(&entered), ["IDLE TO OUT", "SPAWN"]);
    // Its mark sits 0.968 of the way through a state of 0.833 s.
    assert!((hidden - 0.833 * 0.968).abs() < 0.05, "{hidden}");
    assert!(!w.in_hand());

    // A reload with a round chambered, the inventory opened 0.3 s into it:
    // noted until the magazine is out, and then the reload ends in the
    // hold, by the animator's own way there.
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    w.reload();
    let (mut entered, _) = run(&mut w, 0.3);
    w.set_inventory(true);
    assert!(!w.animator().flag("Inventory"));
    let (more, said) = run(&mut w, 5.0);
    entered.extend(more.into_iter().map(|(name, at)| (name, at + 0.3)));
    println!("a reload with the inventory open: {entered:?}");
    assert_eq!(names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD INVENTORY END", "IN INVENTORY"]);
    assert!(when(&said, &FirearmEvent::MagazineIn).is_some() && w.in_inventory() && !w.reloading());
    assert_eq!((w.magazine, w.chamber), (Some(30), true));
    // Shut, the hands come back.
    w.set_inventory(false);
    let (entered, _) = run(&mut w, 1.5);
    assert_eq!(names(&entered), ["CLOSE INVENTORY", "IDLE"]);
    assert!(w.is_idle());

    // The same from an empty chamber: through the chambering, and from
    // there to the hold.
    let mut w = Firearm::new(settings.clone(), &data);
    (w.chamber, w.magazine) = (false, Some(0));
    w.start_drawn();
    w.reload();
    let (mut entered, _) = run(&mut w, 0.3);
    w.set_inventory(true);
    let (more, _) = run(&mut w, 7.0);
    entered.extend(more.into_iter().map(|(name, at)| (name, at + 0.3)));
    println!("a reload from empty with the inventory open: {entered:?}");
    assert_eq!(names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD CHARGE END", "RELOAD INVENTORY END", "IN INVENTORY"]);
    assert_eq!((w.magazine, w.chamber), (Some(29), true));

    // A reload asked for in a single shot follows the shot's own wait.
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    w.set_trigger(true, true);
    w.step(FRAME);
    w.reload();
    let mut waited = FRAME;
    while !w.reloading() && waited < 1.0 {
        w.step(FRAME);
        waited += FRAME;
    }
    println!("a reload asked for in a shot begins {waited:.3} s after the press (the wait between single shots is {:.3} s)", 60.0 / settings.single_fire_rate);
    assert!((waited - 60.0 / settings.single_fire_rate).abs() < 0.03, "{waited}");

    // Drawn with rounds in the magazine and none chambered, the weapon
    // racks its bolt: the animator's other draw.
    let mut w = Firearm::new(settings.clone(), &data);
    (w.chamber, w.magazine) = (false, Some(7));
    w.draw();
    let (entered, said) = run(&mut w, 2.5);
    let appeared = when(&said, &FirearmEvent::Appeared).expect("the weapon appears");
    println!("drawn with an empty chamber: {entered:?}; appeared at {appeared:.3} s");
    assert_eq!(names(&entered), ["OUT TO IDLE ARM", "IDLE"]);
    assert_eq!((w.magazine, w.chamber), (Some(6), true));
    assert_eq!(w.animator().get("AmmoInChamber"), 1.0);

    // The empty hands: there 0.034 s into their state for appearing.
    let empty = trp_data::weapon::assemble_prefab(&mut assets, trp_data::weapon::EMPTY_HANDS).expect("the empty hands");
    let data = WeaponAnimatorData::load(&assets, &empty).expect("their controller");
    let mut hands = EmptyHands::new(&data);
    hands.draw();
    assert!(hands.hands_busy());
    let mut appeared = None;
    for frame in 0..120 {
        if hands.step(FRAME, 0.0).contains(&FirearmEvent::Appeared) {
            appeared.get_or_insert((frame + 1) as f32 * FRAME);
        }
    }
    let appeared = appeared.expect("the hands appear");
    println!("empty hands: there {appeared:.3} s after they were asked for; at rest in {}", hands.animator().state().0);
    assert!(appeared < 0.06 && !hands.hands_busy());
    assert_eq!(hands.animator().state().0, "Hands.IDLE");
    // Looking down, the resting hands are the other of their two clips.
    let level = hands.animator().pose().to_vec();
    for _ in 0..30 {
        hands.step(FRAME, 90.0);
    }
    let down = hands.animator().pose();
    let moved = level.iter().zip(down).filter(|(a, b)| (0..3).any(|k| (a.0[k] - b.0[k]).abs() > 1e-4)).count();
    println!("{moved} of their {} bones are elsewhere with the view straight down", level.len());
    assert!(moved > 0);
    hands.hide();
    assert!(!hands.in_hand() && hands.hands_busy());
}

/// A switch on the head flipped by the key (inventory spec, "A switch on the
/// head"): its gesture on the body's own animator, set beside course 3 of
/// `courses2`, and refused with the thing in hand not at rest.
#[test]
fn a_switch_on_the_head_is_flipped_by_a_gesture() {
    use trp_sim::Interaction;
    let Some(mut m) = movement(0.0) else { return };
    const FRAME: f32 = 1.0 / 120.0;
    let still = Input { hands_idle: true, ..Input::default() };
    for _ in 0..120 {
        m.step(FRAME, &still, &Flat);
    }
    // How long the gesture's flag stays up, as the recording counts it.
    let lasts = |m: &mut Movement| -> f32 {
        let mut frames = 1;
        while m.interaction().is_some() && frames < 600 {
            m.step(FRAME, &still, &Flat);
            frames += 1;
        }
        (frames - 1) as f32 * FRAME
    };
    // Recorded (`courses2`, `3-menu-worn`, the key): the face shield's
    // gesture up 1.342 s, down 1.342 s.
    for gesture in [Interaction::FaceshieldOnGear, Interaction::FaceshieldOffGear, Interaction::NightVisionOnGear, Interaction::NightVisionOffGear] {
        assert!(m.set_interaction(gesture));
        assert_eq!(m.interaction(), Some(gesture));
        let took = lasts(&mut m);
        println!("{gesture:?}: {took:.3} s");
        if matches!(gesture, Interaction::FaceshieldOnGear | Interaction::FaceshieldOffGear) {
            assert!((took - 1.342).abs() <= 2.0 * FRAME, "{gesture:?}: {took}");
        }
        for _ in 0..30 {
            m.step(FRAME, &still, &Flat);
        }
    }
    // With the thing in hand not at rest (as under the open inventory),
    // refused.
    let busy = Input { hands_idle: false, ..Input::default() };
    m.step(FRAME, &busy, &Flat);
    assert!(!m.set_interaction(Interaction::FaceshieldOnGear));
}

/// The hands' gestures on the world, on the body's own animator
/// (`docs/spec/interactions.md`): a door worked, a container searched and
/// shut, a thing taken up, a thing unlocked, and the body's pose for an
/// open inventory. The states, their lengths and the curves that hand the
/// left arm over to the body are the install's; how long each gesture's
/// flag is up, when a searched container is shown and how long a pick-up
/// holds the character are set beside the recordings made by hand on
/// 2026-10-12 (`local\traces\byhand1`, at 120 frames a second like this).
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
    // The other gestures recorded on 2026-10-12 (byhand2): an open door
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
    let turned = trp_sim::math::delta_angle(yaw, m.yaw);
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

/// A container searched lying down (recorded on 2026-10-12, byhand2,
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

/// The M3 Super 90 on the install's own controller (`docs/spec/shotgun.md`):
/// a pumped shot and a self-loading one, the tube filled a shell at a time
/// and cut short, what the reload key is refused, the bolt caught open on
/// the last shell and what that does to the fire mode, the first shell
/// into an empty weapon in each mode, the look into the chamber, and the
/// draw that works the action. The sixteenth recording has the shots, the
/// fillings, the mode changes and the looks frame for frame; what is only
/// here is said where it is tried.
#[test]
fn the_m3_is_pumped_and_filled_a_shell_at_a_time() {
    use trp_sim::weapon_animator::WeaponAnimatorData;
    use trp_sim::{FireMode, Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, "6259b864ebedf17603599e88").expect("the M3 Super 90");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    println!(
        "the M3: modes {:?}, a tube of {}, {} a minute for single shots, bolt catch {}, no mode change on it {}, filled a shell at a time {}",
        settings.modes, settings.magazine_capacity, settings.single_fire_rate, settings.bolt_catch, settings.no_fire_mode_on_bolt_catch, settings.internal_magazine
    );
    assert_eq!(settings.modes, [FireMode::Single, FireMode::SemiAuto]);
    assert_eq!(settings.magazine_capacity, 5);
    assert!(settings.internal_magazine && settings.bolt_catch && settings.no_fire_mode_on_bolt_catch);

    const FRAME: f32 = 1.0 / 120.0;
    let run = |w: &mut Firearm, seconds: f32| -> (Vec<(String, f32)>, Vec<(FirearmEvent, f32)>) {
        let name = |w: &Firearm| {
            let (state, _, next) = w.animator().state();
            next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
        };
        let (mut entered, mut said, mut last) = (Vec::new(), Vec::new(), name(w));
        for frame in 0..(seconds / FRAME).round() as usize {
            let at = (frame + 1) as f32 * FRAME;
            said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_))).map(|e| (e, at)));
            let now = name(w);
            if now != last {
                entered.push((now.clone(), at));
                last = now;
            }
        }
        (entered, said)
    };
    let names = |entered: &[(String, f32)]| entered.iter().map(|entry| entry.0.clone()).collect::<Vec<_>>();
    let shots = |said: &[(FirearmEvent, f32)]| said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count();
    let taken = |said: &[(FirearmEvent, f32)]| said.iter().filter_map(|(event, _)| if let FirearmEvent::RoundsTaken(n) = event { Some(*n) } else { None }).sum::<u32>();
    let tap = |w: &mut Firearm| {
        w.set_trigger(true, true);
        w.step(FRAME);
        w.set_trigger(false, true);
    };
    let at_rest = |tube: u32, chamber: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.magazine, w.chamber) = (Some(tube), chamber);
        w.start_drawn();
        w
    };

    // A pumped shot: the fire state from a fifth in, the action worked by
    // the same state, and a press in the middle of it lost.
    let mut w = at_rest(5, true);
    tap(&mut w);
    assert_eq!(w.animator().state().0, "Hands.FIRE");
    let (_, early) = run(&mut w, 0.25);
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (entered, said) = run(&mut w, 1.5);
    println!("a pumped shot, and a press 0.26 s into it: {entered:?}; {} shot(s)", shots(&early) + shots(&said));
    assert_eq!(shots(&early) + shots(&said), 1, "the weapon queues no second shot");
    assert_eq!((w.magazine, w.chamber), (Some(4), true));
    assert!(w.is_idle());

    // The trigger held: one shot, as in the game (recorded: 72).
    let mut w = at_rest(5, true);
    w.set_trigger(true, true);
    let (_, said) = run(&mut w, 2.0);
    assert_eq!(shots(&said), 1);

    // Topped up with a shell chambered: two shells into a tube of three,
    // the count the weapon's only when the filling is over.
    let mut w = at_rest(3, true);
    w.reload_shells(40);
    assert!(w.reloading());
    let (entered, said) = run(&mut w, 1.5);
    assert_eq!(w.magazine, Some(3), "the tube's count is the weapon's when the filling ends");
    let (more, said_more) = run(&mut w, 4.0);
    println!("topped up from 3 + 1: {entered:?} then {more:?}");
    assert_eq!(names(&entered), ["RELOAD START", "RELOAD IN"]);
    assert_eq!(names(&more), ["RELOAD END", "IDLE"]);
    assert_eq!((w.magazine, w.chamber, taken(&said) + taken(&said_more)), (Some(5), true, 2));
    assert!(!w.reloading() && w.is_idle());

    // A full tube under an empty chamber: the key does nothing (from the
    // code alone: no recording has it).
    let mut w = at_rest(5, false);
    w.reload_shells(40);
    assert!(!w.reloading(), "a full tube is not filled, whatever the chamber");
    // And with nothing within reach it does nothing either.
    let mut w = at_rest(2, true);
    w.reload_shells(0);
    assert!(!w.reloading());
    // With one shell within reach, one goes in.
    w.reload_shells(1);
    run(&mut w, 4.0);
    assert_eq!(w.magazine, Some(3));

    // Cut short by the trigger: the shell on its way in still goes in, and
    // nothing fires.
    let mut w = at_rest(0, true);
    w.reload_shells(40);
    run(&mut w, 2.0);
    tap(&mut w);
    let (entered, said) = run(&mut w, 3.0);
    println!("cut short 2 s in: {entered:?}, {:?} in the tube", w.magazine);
    assert_eq!(shots(&said), 0);
    assert!(w.magazine.is_some_and(|tube| tube >= 1 && tube < 5), "{:?}", w.magazine);
    assert!(w.is_idle());

    // From empty, pumped: the first shell goes into the chamber by a state
    // of its own, and the weapon has it from the key on.
    let mut w = at_rest(0, false);
    w.reload_shells(40);
    assert!(w.chamber, "an empty chamber is given its round as the filling begins");
    let (entered, said) = run(&mut w, 10.0);
    println!("from empty, pumped: {entered:?}");
    assert_eq!(names(&entered).first().map(String::as_str), Some("PA RELOAD FIRST"));
    assert_eq!((w.magazine, w.chamber, taken(&said)), (Some(5), true, 6), "one for the chamber as it began, five into the tube");

    // The self-loading mode: a change that takes its time, a shot by its
    // own state, and the bolt caught open on the last shell.
    let mut w = at_rest(1, true);
    assert!(w.set_fire_mode(FireMode::SemiAuto));
    let (entered, _) = run(&mut w, 2.0);
    println!("to the self-loading mode: {entered:?}");
    assert_eq!(names(&entered), ["TO SA", "IDLE"]);
    tap(&mut w);
    assert_eq!(w.animator().state().0, "Hands.SA FIRE", "the self-loading shot has a state of its own");
    let (entered, said) = run(&mut w, 0.6);
    assert_eq!((names(&entered), shots(&said)), (vec!["IDLE".to_owned()], 1));
    assert!(!w.bolt_caught(), "a shell was left to load");
    tap(&mut w);
    let (_, said) = run(&mut w, 0.6);
    assert_eq!(shots(&said), 1);
    assert!(w.bolt_caught() && !w.chamber && w.magazine == Some(0), "the last shell leaves the bolt caught open");
    // The mode is not changed while it is (the item's own flag).
    assert!(!w.set_fire_mode(FireMode::Single));
    assert_eq!(w.mode(), FireMode::SemiAuto);
    // The first shell goes in past the open bolt, which is let go.
    w.reload_shells(40);
    let (entered, _) = run(&mut w, 10.0);
    println!("from empty, self-loading, the bolt caught: {entered:?}");
    assert_eq!(names(&entered).first().map(String::as_str), Some("SA AMMO IN FIRST CATCH"));
    assert!(!w.bolt_caught() && w.chamber && w.magazine == Some(5));
    assert!(w.set_fire_mode(FireMode::Single));
    let (entered, _) = run(&mut w, 2.0);
    assert_eq!(names(&entered), ["TO PA", "IDLE"]);

    // The look into the chamber, by the mode; not with the trigger held.
    let mut w = at_rest(5, true);
    w.check_chamber();
    assert!(w.checking());
    let (entered, _) = run(&mut w, 3.0);
    assert_eq!(names(&entered), ["PA CHECK CHAMBER", "IDLE"]);
    assert!(!w.checking());

    // A weapon with shells in its tube and none chambered works its action
    // as it is drawn (the user, 2026-10-12: a weapon found in a container;
    // `SpawnOperation`), by the state of its mode.
    let mut w = Firearm::new(settings.clone(), &data);
    (w.magazine, w.chamber) = (Some(5), false);
    w.draw();
    let (entered, said) = run(&mut w, 2.5);
    let appeared = said.iter().find(|(event, _)| *event == FirearmEvent::Appeared).map(|(_, at)| *at).expect("it appears");
    println!("drawn with an empty chamber: {entered:?}; appeared {appeared:.3} s after the draw was asked for");
    assert_eq!(names(&entered), ["PA OUT TO IDLE ARM", "IDLE"]);
    assert_eq!((w.magazine, w.chamber), (Some(4), true));
    let mut w = Firearm::new(settings.clone(), &data);
    (w.magazine, w.chamber) = (Some(5), false);
    w.start_in_mode(FireMode::SemiAuto);
    w.draw();
    let (entered, _) = run(&mut w, 2.5);
    assert_eq!(names(&entered), ["SA OUT TO IDLE ARM", "IDLE"]);
}

/// What a weapon of the install is to the simulation: its build, its
/// animator and its logic's settings.
fn weapon_of(
    template: &str,
) -> Option<(trp_data::weapon::Weapon, trp_sim::weapon_animator::WeaponAnimatorData, trp_sim::FirearmSettings, &'static Database, Assets)> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, template).expect("the weapon");
    let data = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    Some((weapon, data, settings, db, assets))
}

/// Steps a weapon's logic at 120 frames a second and says which of its
/// animator's states were entered and what it said, each with its time.
fn run_weapon(w: &mut trp_sim::Firearm, seconds: f32) -> (Vec<(String, f32)>, Vec<(trp_sim::FirearmEvent, f32)>) {
    use trp_sim::{Firearm, FirearmEvent};
    const FRAME: f32 = 1.0 / 120.0;
    let name = |w: &Firearm| {
        let (state, _, next) = w.animator().state();
        next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
    };
    let (mut entered, mut said, mut last) = (Vec::new(), Vec::new(), name(w));
    for frame in 0..(seconds / FRAME).round() as usize {
        let at = (frame + 1) as f32 * FRAME;
        said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_))).map(|e| (e, at)));
        let now = name(w);
        if now != last {
            entered.push((now.clone(), at));
            last = now;
        }
    }
    (entered, said)
}

fn state_names(entered: &[(String, f32)]) -> Vec<&str> {
    entered.iter().map(|entry| entry.0.as_str()).collect()
}

/// The M3's stock folds and unfolds by its animator's own two states
/// (`docs/spec/shotgun.md`, "The stock"). From the game's code and the
/// install; no recording has a fold.
#[test]
fn the_m3_s_stock_folds() {
    use trp_sim::{Firearm, FirearmEvent};
    let Some((weapon, data, settings, db, _)) = weapon_of("6259b864ebedf17603599e88") else { return };
    let fold = weapon.fold.expect("the stock preset's stock folds");
    println!(
        "the M3: {} cells wide and {} folded; the parts' recoil {:+.2} and {:+.2} folded; held closer {} and {} folded",
        weapon.cell_width, fold.cell_width, weapon.recoil_delta, fold.recoil_delta, weapon.held_closer, fold.held_closer
    );
    assert!(settings.foldable);
    assert_eq!((weapon.cell_width, fold.cell_width), (5, 4));
    // The stock's own share, 21 hundredths, is what the fold takes away.
    assert!((fold.recoil_delta - weapon.recoil_delta - 0.21).abs() < 1e-4, "{} {}", fold.recoil_delta, weapon.recoil_delta);
    assert!(!weapon.held_closer && fold.held_closer);
    // The AKM's stock does not fold.
    let akm = trp_data::weapon::assemble(&mut Assets::new(crate::fixture::install().ok_or(()).unwrap()).unwrap(), &db, "59d6088586f774275f37482f").unwrap();
    assert!(akm.fold.is_none() && !akm.held_closer);

    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    assert!(!w.stock_folded());
    // Not in a look at the weapon.
    w.check_magazine();
    assert!(!w.fold_stock());
    run_weapon(&mut w, 3.0);
    assert!(w.is_idle());

    assert!(w.fold_stock());
    assert!(w.occupied() && w.hands_busy());
    // The trigger does nothing meanwhile.
    w.set_trigger(true, true);
    let (entered, said) = run_weapon(&mut w, 2.0);
    w.set_trigger(false, true);
    println!("folding: {entered:?} {said:?}");
    assert_eq!(state_names(&entered), ["STOCK FOLD", "IDLE"]);
    // The stock stands folded 0.694 of the way through the state's 1.333 s.
    let folded = said.iter().find(|(event, _)| *event == FirearmEvent::Folded(true)).expect("folded").1;
    assert!((folded - 0.925).abs() < 0.03, "{folded}");
    assert!((entered[1].1 - 1.34).abs() < 0.03, "{:?}", entered);
    assert!(w.stock_folded() && w.is_idle() && !w.hands_busy());
    assert!(!said.iter().any(|(event, _)| matches!(event, FirearmEvent::Shot { .. })));
    assert!(w.animator().flag("StockFolded"));

    // It shoots folded, and unfolds again; put away in the unfolding, it
    // goes when the stock is out.
    w.set_trigger(true, true);
    let (_, said) = run_weapon(&mut w, 1.0);
    w.set_trigger(false, true);
    assert_eq!(said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count(), 1);
    assert!(w.fold_stock());
    w.hide(false);
    let (entered, said) = run_weapon(&mut w, 3.0);
    println!("unfolding, put away meanwhile: {entered:?} {said:?}");
    let unfolded = said.iter().find(|(event, _)| *event == FirearmEvent::Folded(false)).expect("unfolded").1;
    assert!(unfolded < 0.1, "{unfolded}");
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::Hidden));
    assert!(!w.stock_folded() && !w.in_hand());

    // A weapon put away folded is drawn folded.
    let mut w = Firearm::new(settings, &data);
    w.start_folded(true);
    w.draw();
    run_weapon(&mut w, 2.0);
    assert!(w.is_idle() && w.stock_folded() && w.animator().flag("StockFolded"));
}

/// The Remington Model 700: the bolt worked when the trigger is let go,
/// and its magazine filled a round at a time with the bolt open
/// (`docs/spec/bolt-action.md`). From the game's code and the install; no
/// recording has the weapon.
#[test]
fn the_model_700_s_bolt_is_worked_by_hand() {
    use trp_sim::{FireMode, Firearm, FirearmEvent};
    let Some((weapon, data, settings, ..)) = weapon_of("5bfea6e90db834001b7347f3") else { return };
    println!(
        "the Model 700: {:.2} kg, {} cells, modes {:?}, a magazine of {}, bolt action {}, filled with the bolt open {}, {} a minute",
        weapon.total_weight, weapon.cell_width, settings.modes, settings.magazine_capacity, settings.bolt_action, settings.open_bolt_reload, settings.single_fire_rate
    );
    assert_eq!(settings.modes, [FireMode::Single]);
    assert_eq!(settings.magazine_capacity, 5);
    assert!(settings.bolt_action && settings.internal_magazine && settings.open_bolt_reload && !settings.foldable);
    const FRAME: f32 = 1.0 / 120.0;
    let at_rest = |magazine: u32, chamber: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.magazine, w.chamber) = (Some(magazine), chamber);
        w.start_drawn();
        w
    };
    let shots = |said: &[(FirearmEvent, f32)]| said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count();

    // The trigger held: the shot, and the bolt left alone for as long as
    // it is held.
    let mut w = at_rest(5, true);
    w.set_trigger(true, true);
    let (entered, said) = run_weapon(&mut w, 2.0);
    println!("held: {entered:?}");
    assert_eq!(shots(&said), 1);
    assert_eq!(w.animator().state().0, "Hands.FIRE");
    assert!(w.occupied() && state_names(&entered) == ["FIRE"]);
    // Let go: the bolt is worked, 1.767 s, and the next round is in.
    w.set_trigger(false, true);
    let (entered, said) = run_weapon(&mut w, 2.5);
    println!("let go: {entered:?} {said:?}");
    assert_eq!(state_names(&entered), ["FIRE RELOAD", "IDLE"]);
    assert!((entered[1].1 - entered[0].1 - 1.767).abs() < 0.03, "{entered:?}");
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::ShellEject));
    assert!(w.is_idle() && w.chamber && w.magazine == Some(4));

    // A tap: the bolt follows the shot at once (the fire state is 0.167 s,
    // entered a fifth in).
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (entered, said) = run_weapon(&mut w, 2.5);
    println!("a tap: {entered:?}");
    assert_eq!((shots(&said), state_names(&entered)), (1, vec!["FIRE RELOAD", "IDLE"]));
    assert!((entered[0].1 - 0.13).abs() < 0.03, "{entered:?}");
    // A second press while the bolt is worked is lost.
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    run_weapon(&mut w, 0.5);
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (_, said) = run_weapon(&mut w, 2.5);
    assert_eq!(shots(&said), 0);
    assert!(w.is_idle() && w.chamber && w.magazine == Some(2));

    // The magazine filled: the bolt opened, which throws the chambered
    // round out, a round a loop, the bolt shut on the first of them.
    let mut w = at_rest(2, true);
    w.reload_shells(20);
    assert!(w.reloading() && !w.chamber, "the chambered round has left the weapon at the key");
    let (entered, said) = run_weapon(&mut w, 9.0);
    println!("filled: {entered:?}\n  {said:?}");
    assert_eq!(state_names(&entered), ["RELOAD START", "RELOAD IN", "RELOAD CLOSE CHAMBER", "IDLE"]);
    assert_eq!(said.iter().filter(|(event, _)| *event == FirearmEvent::RoundEjected).count(), 1);
    let taken: u32 = said.iter().filter_map(|(event, _)| if let FirearmEvent::RoundsTaken(n) = event { Some(*n) } else { None }).sum();
    assert_eq!(taken, 3);
    // Five went into the magazine and the bolt took the first of them.
    assert!(w.is_idle() && w.chamber && w.magazine == Some(4), "{:?} {}", w.magazine, w.chamber);
    assert!(!w.bolt_caught());
    // Three loops of 1.533 s between the opening (1.167 s) and the closing.
    let loops = entered[2].1 - entered[1].1;
    assert!((loops - 3.0 * 1.533).abs() < 0.1, "{loops}");

    // A full magazine is not filled, whatever the chamber; and the round
    // chambered is then kept.
    let mut w = at_rest(5, true);
    w.reload_shells(20);
    assert!(!w.reloading() && w.chamber);

    // Cut short by the trigger: not before the first round is in, and then
    // the round on its way still goes in, the bolt is shut, nothing fires.
    let mut w = at_rest(0, false);
    w.reload_shells(20);
    run_weapon(&mut w, 1.0);
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (_, early) = run_weapon(&mut w, 1.5);
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (entered, said) = run_weapon(&mut w, 6.0);
    println!("cut short: {entered:?}");
    assert_eq!(shots(&early) + shots(&said), 0);
    assert!(w.is_idle() && w.chamber);
    assert_eq!(w.magazine, Some(1), "two went in and the bolt took one");

    // Found with rounds in the magazine and none chambered, the draw works
    // the bolt.
    let mut w = Firearm::new(settings.clone(), &data);
    (w.magazine, w.chamber) = (Some(5), false);
    w.draw();
    let (entered, said) = run_weapon(&mut w, 3.0);
    println!("first draw: {entered:?}");
    assert_eq!(state_names(&entered), ["OUT TO IDLE ARM", "IDLE"]);
    let appeared = said.iter().find(|(event, _)| *event == FirearmEvent::Appeared).expect("appeared").1;
    assert!((appeared - 2.2).abs() < 0.06, "{appeared}");
    assert!(w.chamber && w.magazine == Some(4));
}

/// The FN Five-seveN: a pistol with a magazine and a slide that locks
/// back on the last round (`docs/spec/pistol.md`). From the game's code
/// and the install; no recording has the weapon.
#[test]
fn the_five_seven_s_slide_locks_back() {
    use trp_sim::{FireMode, Firearm, FirearmEvent};
    let Some((weapon, data, settings, db, _)) = weapon_of("5d3eb3b0a4b93615055e84d2") else { return };
    println!(
        "the Five-seveN: {:.2} kg, {} cells, modes {:?}, a magazine of {}, bolt catch {}, a second shot queued {}, {} a minute",
        weapon.total_weight, weapon.cell_width, settings.modes, settings.magazine_capacity, settings.bolt_catch, settings.can_queue_second_shot, settings.single_fire_rate
    );
    assert_eq!(settings.modes, [FireMode::Single]);
    assert_eq!(settings.magazine_capacity, 20);
    assert!(settings.bolt_catch && settings.can_queue_second_shot && !settings.internal_magazine && !settings.bolt_action);
    assert!(db.is_a(db.item(&weapon.template).unwrap(), "Pistol"));
    const FRAME: f32 = 1.0 / 120.0;
    let at_rest = |magazine: u32, chamber: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.magazine, w.chamber) = (Some(magazine), chamber);
        w.start_drawn();
        w
    };
    let shots = |said: &[(FirearmEvent, f32)]| said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count();
    let tap = |w: &mut Firearm| {
        w.set_trigger(true, true);
        w.step(FRAME);
        w.set_trigger(false, true);
    };

    // One shot a press, however long it is held; a press during the shot
    // is remembered and fired after it.
    let mut w = at_rest(20, true);
    w.set_trigger(true, true);
    let (_, said) = run_weapon(&mut w, 1.0);
    w.set_trigger(false, true);
    assert_eq!(shots(&said), 1);
    tap(&mut w);
    let (_, first) = run_weapon(&mut w, 0.05);
    tap(&mut w);
    let (_, said) = run_weapon(&mut w, 1.0);
    assert_eq!((shots(&first), shots(&said)), (1, 1), "the one in hand and the one queued");
    assert!(w.is_idle() && w.magazine == Some(17) && !w.bolt_caught());

    // The last round: the slide stays back, and a press on it plays the
    // dry press of a caught slide.
    let mut w = at_rest(0, true);
    tap(&mut w);
    let (entered, said) = run_weapon(&mut w, 1.0);
    println!("the last round: {entered:?}");
    assert_eq!(shots(&said), 1);
    assert!(w.is_idle() && !w.chamber && w.bolt_caught());
    w.set_trigger(true, true);
    let (entered, _) = run_weapon(&mut w, 0.5);
    w.set_trigger(false, true);
    assert_eq!(state_names(&entered), ["DRY FIRE CATCH"]);
    run_weapon(&mut w, 1.0);

    // Reloaded from there: the magazine changed, the slide let go on the
    // first round of the new one.
    w.reload();
    let (entered, said) = run_weapon(&mut w, 8.0);
    println!("reloaded with the slide back: {entered:?}");
    assert_eq!(state_names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD CATCH END TO IDLE", "IDLE"]);
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::MagazineIn));
    assert!(w.is_idle() && w.chamber && w.magazine == Some(19) && !w.bolt_caught());
    let whole = entered.last().unwrap().1;
    println!("  at rest {whole:.2} s after the key");

    // With a round chambered the magazine is only changed.
    let mut w = at_rest(3, true);
    w.reload();
    let (entered, _) = run_weapon(&mut w, 8.0);
    println!("reloaded with a round chambered: {entered:?}");
    assert_eq!(state_names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
    assert!(w.is_idle() && w.chamber && w.magazine == Some(20) && !w.bolt_caught());

    // Put away with the slide back and drawn again, the slide is forward.
    let mut w = at_rest(0, true);
    tap(&mut w);
    run_weapon(&mut w, 1.0);
    assert!(w.bolt_caught());
    w.hide(false);
    run_weapon(&mut w, 2.0);
    w.draw();
    let (entered, _) = run_weapon(&mut w, 2.0);
    assert_eq!(state_names(&entered), ["OUT TO IDLE", "IDLE"]);
    assert!(w.is_idle() && !w.bolt_caught());

    // Found with a full magazine and none chambered: the draw racks it.
    let mut w = Firearm::new(settings, &data);
    (w.magazine, w.chamber) = (Some(20), false);
    w.draw();
    let (entered, said) = run_weapon(&mut w, 3.0);
    assert_eq!(state_names(&entered), ["OUT TO IDLE ARM", "IDLE"]);
    let appeared = said.iter().find(|(event, _)| *event == FirearmEvent::Appeared).expect("appeared").1;
    println!("first draw: appeared {appeared:.3} s after it was asked for");
    assert!(w.chamber && w.magazine == Some(19));
}

/// A door kicked (`docs/spec/interactions.md`, "A kick"), set beside the
/// two kicks recorded by hand on 2026-10-12 (`local\traces\byhand3`): a
/// shut door that puts the character at its place and is thrown open, and
/// a locked one that puts it nowhere and holds. The figures in the
/// comments are the recordings'.
#[test]
fn a_door_is_kicked() {
    use trp_sim::things::{Hinge, Thing, ThingSettings, ThingState};
    use trp_sim::Acted;
    let Some(mut m) = movement(0.0) else { return };
    let hard = trp_data::hard_settings::HardSettings::load(&crate::fixture::install().ok_or(()).unwrap()).expect("settings object");
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
    use trp_sim::things::{Action, Hinge, Thing};
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
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

/// Every weapon the app builds (`trp-app`'s `WEAPONS`): its motion and its
/// logic are read from the install, a crate of ten cells by thirty holds
/// the four with a vest and a backpack, and put on, each goes to the slot
/// of its kind: the pistol to the holster.
#[test]
fn the_four_weapons_are_built_and_find_their_slots() {
    use trp_data::inventory::Loadout;
    use trp_sim::inventory::{equip, Equipment, Item, Kind, Rules};
    use std::sync::Arc;
    const WEAPONS: [(&str, &str); 4] = [
        ("59d6088586f774275f37482f", "FirstPrimaryWeapon"),
        ("6259b864ebedf17603599e88", "SecondPrimaryWeapon"),
        ("5d3eb3b0a4b93615055e84d2", "Holster"),
        ("5bfea6e90db834001b7347f3", ""),
    ];
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let engine = trp_data::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let data = trp_data::player::HandsData::load(&mut assets).expect("weapon motion data");
    let rules = Rules::of(&db);
    let mut crate_kind = Kind::of(&db, "6223349b3136504a544d1608").expect("the airdrop crate");
    crate_kind.grids[0].cells = [10, 30];
    let mut loot = Item::new(Arc::new(crate_kind));
    let mut worn = Equipment::empty(&db).expect("the equipment's template").item;
    for (template, slot) in WEAPONS {
        let weapon = trp_data::weapon::assemble(&mut assets, &db, template).expect("the weapon");
        let hands = trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &weapon, trp_sim::ViewSettings::default()).expect("its motion settings");
        let logic = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its logic's settings");
        trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its animator");
        println!(
            "{}: {} cells, {:.2} kg, ergonomics {}, a pistol {}, held closer {}, line {:.3} m, magazine {}",
            weapon.name, weapon.cell_width, weapon.total_weight, weapon.total_ergonomics, hands.pistol, hands.held_closer, hands.weapon_length, logic.magazine_capacity
        );
        assert_eq!(hands.pistol, template == "5d3eb3b0a4b93615055e84d2");
        assert!(hands.weapon_length > 0.15 && logic.magazine_capacity > 0);
        let (item, left) = Loadout::of_preset(&db, template).and_then(|list| Item::of_list(&db, &list)).expect("its preset as an item");
        assert!(left.is_empty(), "{left:?}");
        let id = item.id;
        assert!(loot.stow(item).is_ok(), "the crate has no room for {}", weapon.name);
        // Put on with Alt and a click: the first slot of its kind that is free.
        let mut roots = [&mut worn, &mut loot];
        match equip(&mut roots, id, &rules) {
            Ok(into) => assert_eq!(roots[0].kind.slots[into].name, slot, "{}", weapon.name),
            // The third long weapon has no slot left.
            Err(refusal) => assert!(slot.is_empty(), "{}: {refusal:?}", weapon.name),
        }
    }
    let hangs: Vec<&str> = trp_sim::inventory::HAND_SLOTS.into_iter().filter(|slot| worn.in_slot(slot).is_some()).collect();
    assert_eq!(hangs, ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster"]);
    // The game's own words for a door's menu are in the install.
    let texts = trp_data::inventory::texts(&assets.install, "en", &["OpenDoor", "CloseDoor", "Breach", "Bang & clear", "Flash & clear", "Move in", "Search", "Unlock", "Take"]).expect("the texts");
    println!("the menu's words: {texts:?}");
    assert_eq!(texts.len(), 9);
    assert_eq!(texts["opendoor"].to_uppercase(), "OPEN");
}

/// The character's skill with a class of weapon speeds its reloading and
/// its drawing: the animator's own states go by two of its parameters,
/// which the game sets from the skill's level (`docs/spec/firing.md`,
/// "The skill's two speeds"). From the game's code and the install; the
/// recordings were all made by a character with no such skill.
#[test]
fn a_weapon_skill_speeds_the_reload_and_the_draw() {
    use trp_sim::firearm::weapon_skill_speeds;
    use trp_sim::{Firearm, FirearmEvent};
    assert_eq!(weapon_skill_speeds(0), (1.0, 1.0));
    let (reload, draw) = weapon_skill_speeds(50);
    assert!((reload - 1.2).abs() < 1e-6 && (draw - 1.4).abs() < 1e-6);
    for (template, name) in [("5d3eb3b0a4b93615055e84d2", "the Five-seveN"), ("5bfea6e90db834001b7347f3", "the Model 700"), ("59d6088586f774275f37482f", "the AKM")] {
        let Some((_, data, settings, ..)) = weapon_of(template) else { return };
        let times = |level: u32| {
            let (reload_speed, draw_speed) = weapon_skill_speeds(level);
            let mut w = Firearm::new(trp_sim::FirearmSettings { reload_speed, draw_speed, ..settings.clone() }, &data);
            w.draw();
            let (_, said) = run_weapon(&mut w, 4.0);
            let drawn = said.iter().find(|(event, _)| *event == FirearmEvent::Appeared).expect("appeared").1;
            if settings.internal_magazine {
                w.magazine = Some(settings.magazine_capacity - 1);
                w.reload_shells(10);
            } else {
                w.reload();
            }
            let (entered, _) = run_weapon(&mut w, 12.0);
            assert!(w.is_idle(), "{name}: {entered:?}");
            (drawn, entered.last().expect("states").1)
        };
        let (plain, skilled) = (times(0), times(50));
        println!(
            "{name}: drawn in {:.2} s and {:.2} s at level 50 ({:.2} times as fast); reloaded in {:.2} s and {:.2} s ({:.2} times)",
            plain.0, skilled.0, plain.0 / skilled.0, plain.1, skilled.1, plain.1 / skilled.1
        );
        // Faster, and by no more than the parameters' own factors.
        assert!(skilled.0 < plain.0 && plain.0 / skilled.0 < 1.42, "{plain:?} {skilled:?}");
        assert!(skilled.1 < plain.1 && plain.1 / skilled.1 < 1.22, "{plain:?} {skilled:?}");
    }
}

/// The eye is lined up with the iron sights, not with the weapon's own
/// aim bone (`docs/spec/weapon-handling.md`, "The sights"): the rear
/// sight's aim bone, moved onto the line through the rear and the front
/// sight. On the AKM that is a millimetre above the weapon's own bone; on
/// the M3 and the Five-seveN it is centimetres from it. A build with no
/// sight at all aims by the weapon's own bone.
#[test]
fn the_eye_is_lined_up_with_the_iron_sights() {
    use trp_sim::rig::{SightNodes, WeaponRig};
    for (template, name, off) in [
        ("59d6088586f774275f37482f", "the AKM", 0.0011),
        ("6259b864ebedf17603599e88", "the M3", 0.0133),
        ("5d3eb3b0a4b93615055e84d2", "the Five-seveN", 0.0120),
    ] {
        let Some((weapon, ..)) = weapon_of(template) else { return };
        let nodes = SightNodes::of_weapon(&weapon).expect("its sights");
        let rig = WeaponRig::new(&weapon.hierarchy);
        let h = &weapon.hierarchy;
        assert_eq!(h.nodes[nodes.bone].name, "mod_aim_camera", "{name}");
        let line = nodes.line(&rig).expect("a rear and a front sight");
        let bone = nodes.bone_on_weapon(&rig);
        let own = (0..h.nodes.len()).find(|&i| h.nodes[i].name == "aim_camera").map(|own| SightNodes { bone: own, ..nodes }.bone_on_weapon(&rig)).expect("the weapon's own aim bone");
        // Across and up, the eye against the weapon's own bone.
        let gap = ((bone[0] - own[0]).powi(2) + (bone[2] + line.shift[1] - own[2]).powi(2)).sqrt();
        println!("{name}: the sights' line is pitched {:.4} degrees, the eye {:.2} mm up from the sight's bone and {:.1} mm from the weapon's own", line.angle, line.shift[1] * 1000.0, gap * 1000.0);
        assert!(line.angle.abs() < 0.1 && line.shift[0].abs() < 1e-5, "{line:?}");
        assert!((gap - off).abs() < 0.0006, "{name}: {gap}");
        assert!(nodes.muzzle.is_some());
    }
    // The Model 700's stock preset has no sight.
    let Some((weapon, ..)) = weapon_of("5bfea6e90db834001b7347f3") else { return };
    let nodes = SightNodes::of_weapon(&weapon).expect("an aim bone");
    assert_eq!(weapon.hierarchy.nodes[nodes.bone].name, "aim_camera");
    assert!(nodes.line.is_none());
}

/// The way the eye looks along a sight's aim bone (`SightNodes::
/// line_on_weapon`), along which the eye's target slides in automatic fire
/// (`docs/spec/weapon-handling.md`, "Camera"): the bone's -z, or -y for the
/// weapon's own `aim_camera`. On the stock weapons read it runs along the
/// barrel, the weapon's own -y: the iron sights' bone, an optic's, a
/// variable scope's and a rifle with no sight at all.
#[test]
fn a_sight_s_line_runs_along_the_barrel() {
    use trp_sim::rig::{SightNodes, WeaponRig};
    for (template, name, own) in [
        ("59d6088586f774275f37482f", "the AKM", false),
        ("661cec09b2c6356b4d0c7a36", "the M60E6", false),
        ("65290f395ae2ae97b80fdf2d", "the SPEAR", false),
        ("5bfea6e90db834001b7347f3", "the Model 700", true),
    ] {
        let Some((weapon, ..)) = weapon_of(template) else { return };
        let nodes = SightNodes::of_weapon(&weapon).expect("its sights");
        let rig = WeaponRig::new(&weapon.hierarchy);
        let line = nodes.line_on_weapon(&rig);
        let off = (-line[1]).clamp(-1.0, 1.0).acos().to_degrees();
        println!("{name}: {} ({}), its line {:.4} {:.4} {:.4}, {off:.3} degrees off the barrel", weapon.hierarchy.nodes[nodes.bone].name, if nodes.own { "-y" } else { "-z" }, line[0], line[1], line[2]);
        assert_eq!(nodes.own, own, "{name}");
        assert!((line.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 1e-4);
        assert!(off < 0.5, "{name}: {off}");
    }
}

/// A part goes on its bone as the game puts it there (`docs/spec/
/// hands-on-weapon.md`, "A part on its bone"): a quarter turn about x from
/// the bone, or where the part's own `ModPlacer` says. The M3's stock and
/// forend say so for themselves and its pistol grip, which hangs on the
/// stock, does not: so the grip is behind the trigger and below it, in the
/// right hand, where a rule that stood every part upright had it ahead of
/// the trigger guard. And with the rule every weapon's idle clip begins in
/// the build's own rest pose, the parts' roots included.
#[test]
fn a_part_goes_on_its_bone_by_the_game_s_rule() {
    use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
    use trp_data::unity::extract::read_mesh;
    use trp_sim::rig::WeaponRig;
    let quarter = [0.5f32.sqrt(), 0.0, 0.0, 0.5f32.sqrt()];
    let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(&b).map(|(a, b)| a * b).sum::<f32>().abs() > 0.999_99;
    let Some((weapon, _, _, _, assets)) = weapon_of("6259b864ebedf17603599e88") else { return };
    let h = &weapon.hierarchy;
    let named = |name: &str| (0..h.nodes.len()).find(|&i| h.nodes[i].name == name).unwrap_or_else(|| panic!("no {name}"));
    let (stock, forend, grip) = (
        named("stock_m3_benelli_5_position_telescoping_stock"),
        named("handguard_m3_benelli_forend_std"),
        named("pistolgrip_m3_benelli_telescopic_stock_handle"),
    );
    for part in [stock, forend] {
        let placer = h.nodes[part].placer.expect("its own place on the bone");
        assert_eq!((placer.rotation, placer.scale), ([0.0; 3], [1.0; 3]), "{}", h.nodes[part].name);
        assert!(near(h.nodes[part].rotation, [0.0, 0.0, 0.0, 1.0]));
    }
    assert!(h.nodes[grip].placer.is_none());
    assert!(near(h.nodes[grip].rotation, quarter), "{:?}", h.nodes[grip].rotation);
    // The grip's middle in the weapon's own bone (the barrel along -y, up
    // z), against the trigger's pivot.
    let rig = WeaponRig::new(h);
    let (origin, turn) = rig.placed(rig.bone("Weapon_root/Weapon_root_anim/weapon").expect("the weapon's own bone"));
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    let on_weapon = |at: [f32; 3]| trp_sim::math::quat_rotate(back, std::array::from_fn(|k| at[k] - origin[k]));
    let mesh_node = (0..h.nodes.len()).find(|&i| h.nodes[i].parent == Some(grip) && h.nodes[i].renderer.is_some()).expect("the grip's mesh");
    let mesh = read_mesh(&assets, h.nodes[mesh_node].renderer.as_ref().unwrap().mesh).expect("mesh");
    let (at, turn) = rig.placed(mesh_node);
    let trigger = on_weapon(rig.placed(named("weapon_trigger")).0);
    // How far behind the trigger's pivot its nearest and its farthest
    // point are, and how far below the pivot its lowest.
    let (mut nearest, mut farthest, mut lowest) = (f32::MAX, f32::MIN, f32::MIN);
    for vertex in &mesh.positions {
        let there = trp_sim::math::quat_rotate(turn, *vertex);
        let there = on_weapon(std::array::from_fn(|k| at[k] + there[k]));
        nearest = nearest.min(there[1] - trigger[1]);
        farthest = farthest.max(there[1] - trigger[1]);
        lowest = lowest.max(trigger[2] - there[2]);
    }
    println!("the M3's pistol grip: from {:.1} to {:.1} cm behind the trigger's pivot, down to {:.1} cm below it", nearest * 100.0, farthest * 100.0, lowest * 100.0);
    assert!(nearest > 0.0 && farthest < 0.15, "{nearest} {farthest}");
    assert!((0.05..0.15).contains(&lowest), "{lowest}");
    // Every weapon built: no bone its idle clip turns is turned otherwise
    // at rest, and no part without a place of its own is anything but a
    // quarter turn from its bone.
    for (template, idle) in [
        ("59d6088586f774275f37482f", "akm_idle_armed"),
        ("6259b864ebedf17603599e88", "m3s90_idle_armed"),
        ("5d3eb3b0a4b93615055e84d2", "fiveseven_idle"),
        ("5bfea6e90db834001b7347f3", "m700_idle_armed"),
    ] {
        let Some((weapon, _, _, _, assets)) = weapon_of(template) else { return };
        let h = &weapon.hierarchy;
        let nodes: std::collections::HashMap<u32, usize> = (0..h.nodes.len()).map(|i| (path_hash(&h.path(i)), i)).collect();
        let clips = weapon.clips(&assets).expect("clips");
        let clip = Clip::read(&assets, clips.iter().find(|(name, _)| name == idle).unwrap_or_else(|| panic!("no {idle}")).1).expect("the clip");
        let mut values = Vec::new();
        clip.sample(0.0, &mut values);
        let mut turned = 0;
        for track in clip.tracks.iter().filter(|track| track.property == Property::Euler) {
            let Some(&node) = nodes.get(&track.path) else { continue };
            let there = euler_to_quat([values[track.curve], values[track.curve + 1], values[track.curve + 2]]);
            let dot: f32 = there.iter().zip(&h.nodes[node].rotation).map(|(a, b)| a * b).sum();
            let off = 2.0 * dot.abs().min(1.0).acos().to_degrees();
            assert!(off < 1.0, "{idle}: {} is {off} degrees from where it rests", h.path(node));
            turned += 1;
        }
        // A part's root: what hangs on a bone named for a slot and is not
        // one of the prefab's own bones of that kind.
        let parts: Vec<usize> = (0..h.nodes.len())
            .filter(|&i| h.nodes[i].parent.is_some_and(|parent| h.nodes[parent].name.starts_with("mod_")) && !h.nodes[i].name.starts_with("mod_"))
            .filter(|&i| h.nodes[i].scripts.iter().any(|script| script == "PreviewPivot"))
            .collect();
        let placed = parts.iter().filter(|&&i| h.nodes[i].placer.is_some()).count();
        for &part in parts.iter().filter(|&&i| h.nodes[i].placer.is_none()) {
            assert!(near(h.nodes[part].rotation, quarter), "{}", h.path(part));
        }
        println!("{idle}: {turned} bones turned by the clip rest as it has them; {} parts, {placed} with a place of their own", parts.len());
        assert!(parts.len() >= 3, "{}", parts.len());
    }
}

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
    let controls = trp_sim::bindings::install_controls(&install);
    if controls.keys.is_empty() {
        return;
    }
    let named: Vec<&str> = controls.axes.iter().map(|axis| axis.name.as_str()).collect();
    assert!(named.contains(&"MoveX") && named.contains(&"LeanX") && !named.contains(&"TurnX"), "{named:?}");
    for name in ["ReloadWeapon", "Walk", "DropBackpack", "ForceAutoWeaponMode", "CheckFireMode", "ExamineWeapon"] {
        assert!(controls.group(name).is_some(), "{name}");
    }
}

/// The Sturmgewehr 44 of `custom/stg44.json`: a weapon that is not the
/// install's, whose model lies in `local/` (`docs/spec/custom-weapons.md`).
const STG44: &str = "66f0a5740000000000000044";
/// Its donor, the FN SCAR-L, and the AKM it takes its round and recoil from.
const SCAR_L: &str = "6184055050224f204c1da540";
const AKM: &str = "59d6088586f774275f37482f";

/// A weapon of one's own is an item of the database like any other, and
/// is assembled by the game's rules on its donor's rig: what it weighs,
/// what it holds and how it kicks come from its template, its muzzle and
/// its sights from bones as a weapon's parts would bring them, and what
/// is drawn is the model's own parts, each the right way out. Passes
/// with a note where the model is not there (it is never in the
/// repository).
#[test]
fn a_weapon_of_ones_own_is_built_on_its_donors_rig() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    if !db.custom.contains_key(STG44) {
        eprintln!("skipped: the StG 44 is not built here ({:?})", db.custom_notes);
        return;
    }
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let engine = trp_data::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let data = trp_data::player::HandsData::load(&mut assets).expect("weapon motion data");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, STG44).expect("the StG 44");
    let akm = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("the AKM");
    let custom = weapon.custom.as_ref().expect("its own parts");

    // The template: 4.6 kg with its empty magazine, and 31 rounds of the
    // AKM's; five cells; a fixed stock; no catch for the bolt.
    let template = db.item(STG44).expect("its template");
    let round = db.item(db.item(AKM).unwrap().props["defAmmo"].as_str().unwrap()).unwrap().prop_f32("Weight").unwrap();
    assert!((weapon.total_weight - (4.6 + 31.0 * round)).abs() < 1e-3, "{}", weapon.total_weight);
    assert_eq!((weapon.magazine_capacity, weapon.cell_width, weapon.held_closer, weapon.fold), (Some(30), 5, false, None));
    assert_eq!(template.props["ammoCaliber"], db.item(AKM).unwrap().props["ammoCaliber"]);
    let logic = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its logic's settings");
    assert_eq!((logic.bolt_catch, logic.foldable, logic.fire_rate, logic.magazine_capacity), (false, false, 540.0, 30));
    assert_eq!(logic.modes, [trp_sim::FireMode::Single, trp_sim::FireMode::FullAuto]);
    // It kicks and handles as the AKM's stock build does: that build's
    // own figures, its parts' shares in them (below).
    assert!((weapon.total_ergonomics - akm.total_ergonomics).abs() < 1e-4, "{} {}", weapon.total_ergonomics, akm.total_ergonomics);
    let kick = |weapon: &trp_data::weapon::Weapon, template: &str| db.item(template).unwrap().prop_f32("RecoilForceUp").unwrap() * (1.0 + weapon.recoil_delta);
    assert!((kick(&weapon, STG44) - kick(&akm, AKM)).abs() < 1e-3);
    let view = trp_sim::ViewSettings::default();
    let (ours, theirs) = (
        trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &weapon, view).expect("its motion"),
        trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &akm, view).expect("the AKM's motion"),
    );
    for (mine, its) in [(ours.recoil.rotation_strength, theirs.recoil.rotation_strength), (ours.recoil.position_strength, theirs.recoil.position_strength)] {
        assert!((mine[0] - its[0]).abs() < 1e-4 && (mine[1] - its[1]).abs() < 1e-4, "{mine:?} {its:?}");
    }
    println!(
        "the StG 44: {:.2} kg loaded, ergonomics {}, {} rounds a minute, the line that feels for obstacles {:.3} m (the AKM's {:.3})",
        weapon.total_weight, weapon.total_ergonomics, logic.fire_rate, ours.weapon_length, theirs.weapon_length
    );

    // The muzzle and the sights are bones of its own, as a weapon's parts
    // bring them: the model's muzzle 88.5 cm ahead of the weapon's bone,
    // and a line through the notch and the post that is all but level.
    assert!((ours.weapon_length - 0.885).abs() < 0.005, "{}", ours.weapon_length);
    let sights = trp_sim::rig::SightNodes::find(&weapon.hierarchy).expect("its sights");
    let rig = trp_sim::rig::WeaponRig::new(&weapon.hierarchy);
    let line = sights.line(&rig).expect("the line through its two sights");
    assert_eq!(weapon.hierarchy.nodes[sights.bone].name, "mod_aim_camera");
    assert!(line.angle.abs() < 0.2 && line.shift[1].abs() < 0.002, "{line:?}");

    // Every part is drawn the right way out: a triangle's face, by the
    // order of its corners, looks the way its corners' normals do. (The
    // model's axes are mirrored into the rig's, which turns every
    // triangle over unless two corners change places.)
    assert_eq!(custom.parts.len(), 12);
    for part in &custom.parts {
        let mesh = &part.mesh;
        let (mut agree, mut count) = (0usize, 0usize);
        for triangle in mesh.submeshes.iter().flat_map(|indices| indices.chunks_exact(3)) {
            let [a, b, c] = [0, 1, 2].map(|k| mesh.positions[triangle[k] as usize]);
            let (u, v): ([f32; 3], [f32; 3]) = (std::array::from_fn(|k| b[k] - a[k]), std::array::from_fn(|k| c[k] - a[k]));
            let face = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let normal: [f32; 3] = std::array::from_fn(|k| triangle.iter().map(|&i| mesh.normals[i as usize][k]).sum());
            let along: f32 = (0..3).map(|k| face[k] * normal[k]).sum();
            if face.iter().map(|f| f * f).sum::<f32>() > 1e-16 {
                count += 1;
                agree += (along > 0.0) as usize;
            }
        }
        assert!(agree as f32 > 0.98 * count as f32, "{}: {agree} of {count} triangles face their normals' way", mesh.name);
        assert_eq!(mesh.tangents.len(), mesh.positions.len());
    }

    // Its muzzle is the SKS's (its file's `muzzle_from`): that barrel's
    // one jet and its smoke, below its own fireport where they are from
    // the SKS's, half a centimetre ahead along the barrel (-y).
    let h = &weapon.hierarchy;
    let muzzle = weapon.muzzle.as_ref().expect("its muzzle effects");
    assert_eq!((muzzle.jets.len(), muzzle.smokes.len()), (1, 1));
    let jet = &h.nodes[muzzle.jets[0].node];
    assert_eq!(jet.parent.map(|parent| h.nodes[parent].name.as_str()), Some("fireport"));
    assert!(jet.translation[0].abs() < 1e-3 && (jet.translation[1] + 0.005).abs() < 1e-3 && jet.translation[2].abs() < 1e-3, "{:?}", jet.translation);
    assert_eq!(muzzle.smokes[0].node, muzzle.jets[0].node);

    // Its magazine's object is its donor's magazine's prefab, the first
    // under the slot as the game hangs it (what a reload shows and hides,
    // and what is let go): below it the points its rounds are drawn at and
    // the model's magazine, nothing of the donor's drawn and no hand going
    // by it. Every object comes after its parent.
    let slot = h.find_below(0, "mod_magazine").expect("its magazine's slot");
    let magazine = (0..h.nodes.len()).find(|&i| h.nodes[i].parent == Some(slot)).expect("a magazine");
    assert!(h.nodes[magazine].name.starts_with("mag_"), "{}", h.nodes[magazine].name);
    assert!(h.nodes.iter().enumerate().all(|(i, node)| node.parent.is_none_or(|parent| parent < i)));
    let under = |mut node: usize| loop {
        match h.nodes[node].parent {
            Some(parent) if parent == magazine => return true,
            Some(parent) => node = parent,
            None => return false,
        }
    };
    let points: Vec<usize> = (0..h.nodes.len()).filter(|&i| h.nodes[i].name.starts_with("patron_") && h.nodes[i].name != "patron_in_weapon").collect();
    assert_eq!(points.len(), 5);
    assert!(points.iter().all(|&point| under(point)));
    let part = custom.parts.iter().find(|part| part.mesh.name == "Magazine").expect("the model's magazine");
    assert_eq!(h.nodes[part.node].parent, Some(magazine));
    let hung = (0..h.nodes.len()).filter(|&i| under(i)).collect::<Vec<_>>();
    assert!(hung.iter().all(|&i| h.nodes[i].renderer.is_none() && h.nodes[i].grip.is_none()));

    // Let go or in its picture, the magazine is that object with what is
    // below it: the model's magazine drawn, its five points.
    let (tree, own) = trp_data::custom::magazine_model(&weapon).expect("its magazine as a thing");
    assert_eq!(tree.nodes[0].name, h.nodes[magazine].name);
    assert_eq!(tree.nodes.len(), hung.len() + 1);
    assert_eq!((own.parts.len(), own.materials.len()), (1, 1));
    assert_eq!(own.materials[0].name, "Magazine");
    assert_eq!(tree.nodes[own.parts[0].node].name, "Magazine");
    assert_eq!(tree.nodes.iter().filter(|node| node.name.starts_with("patron_")).count(), 5);
}

/// The StG 44 is worked by its donor's animator, and by nothing else:
/// its states, their times and their marks are the SCAR-L's as that
/// weapon would have them with no catch for its bolt. What differs is
/// where the bones are in the pose given out (`trp_data::custom::
/// PoseEdits`): the left hand rests on this rifle's handguard, stays on
/// its magazine wherever that is carried, and takes its cocking handle
/// where this rifle has it; the right hand's marker, which the
/// simulation goes by, is the donor's own.
#[test]
fn a_weapon_of_ones_own_is_worked_by_its_donors_animator() {
    use trp_sim::math::{quat_mul, quat_rotate};
    use trp_sim::rig::WeaponRig;
    use trp_sim::Firearm;
    const FRAME: f32 = 1.0 / 120.0;
    let built = crate::fixture::install().ok_or(()).ok().and_then(|install| Database::load(&install).ok()).is_some_and(|db| db.custom.contains_key(STG44));
    if !built {
        eprintln!("skipped: no install, or the StG 44 is not built here");
        return;
    }
    let Some((ours, ours_data, ours_settings, ..)) = weapon_of(STG44) else { return };
    let Some((theirs, theirs_data, mut theirs_settings, ..)) = weapon_of(SCAR_L) else { return };
    // The donor as a weapon with no catch, at the same rate of fire and
    // with the same round, its magazine's clips the StG's (the 40-round's:
    // `custom/stg44.json`'s `magAnimationIndex`).
    theirs_settings.bolt_catch = false;
    theirs_settings.fire_rate = ours_settings.fire_rate;
    theirs_settings.shot_strength = ours_settings.shot_strength;
    assert_eq!(ours_settings.magazine_variant, 3, "the StG 44's magazine plays the SCAR-L's 40-round's clips");
    theirs_settings.magazine_variant = ours_settings.magazine_variant;
    assert_eq!(ours_data.state_names(), theirs_data.state_names());

    let bone = |weapon: &trp_data::weapon::Weapon, name: &str| {
        let path = format!("Weapon_root/Weapon_root_anim/weapon{name}");
        (0..weapon.hierarchy.nodes.len()).find(|&i| weapon.hierarchy.path(i) == path).unwrap_or_else(|| panic!("no bone {name}"))
    };
    // A bone on the weapon's own bone: centimetres ahead, up and to the right.
    let on_weapon = |rig: &WeaponRig, weapon: usize, bone: usize| -> ([f32; 3], [f32; 4]) {
        let ((at, turn), (there, turned)) = (rig.placed(weapon), rig.placed(bone));
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let p = quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]));
        ([-p[1] * 100.0, p[2] * 100.0, p[0] * 100.0], quat_mul(back, turned))
    };
    let apart = |a: [f32; 3], b: [f32; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt();

    struct Side<'a> {
        weapon: &'a trp_data::weapon::Weapon,
        rig: WeaponRig,
        logic: Firearm,
    }
    let side = |weapon, settings: &trp_sim::FirearmSettings, data| Side { weapon, rig: WeaponRig::new(&weapon.hierarchy), logic: Firearm::new(settings.clone(), data) };
    let (mut a, mut b) = (side(&ours, &ours_settings, &ours_data), side(&theirs, &theirs_settings, &theirs_data));
    // What is read off both rigs each frame.
    struct Read {
        state: String,
        left: [f32; 3],
        right: [f32; 3],
        magazine: [f32; 3],
        handle: [f32; 3],
    }
    let read = |s: &mut Side| -> Read {
        let moved = s.rig.moved_bone();
        s.rig.pose(s.logic.animator().pose(), moved);
        let weapon = bone(s.weapon, "");
        let at = |name: &str| on_weapon(&s.rig, weapon, bone(s.weapon, name)).0;
        let (state, through, next) = s.logic.animator().state();
        Read {
            state: format!("{state} {through:.4} {next:?}"),
            left: at("/weapon_L_IK_marker"),
            right: at("/weapon_R_IK_marker"),
            magazine: at("/mod_magazine"),
            handle: at("/weapon_bolt/mod_charge"),
        }
    };
    // How many frames were set side by side, on how many of them the
    // hand had the magazine and how far from the donor's hold at worst,
    // and where the palm was from the donor's at the handle's fullest
    // pull.
    let tally = std::cell::RefCell::new((0usize, 0usize, 0.0f32, None::<([f32; 3], f32)>));
    let step = |a: &mut Side, b: &mut Side, seconds: f32| {
        let mut tally = tally.borrow_mut();
        let (frames, on_magazine, worst_on_magazine, pulled) = &mut *tally;
        for _ in 0..(seconds / FRAME).round() as usize {
            // The same things said by both, frame for frame.
            assert_eq!(a.logic.step(FRAME), b.logic.step(FRAME));
            let (ours, theirs) = (read(a), read(b));
            assert_eq!(ours.state, theirs.state);
            *frames += 1;
            // The right hand's marker is the donor's own.
            assert!(apart(ours.right, theirs.right) < 1e-3, "{:?} {:?}", ours.right, theirs.right);
            // The handle travels the donor's path.
            assert!(apart(ours.handle, theirs.handle) < 1e-3);
            // While the donor's hand has its magazine (the palm's marker
            // within 3 cm of where it holds one), ours is as far from
            // ours.
            let rest = [40.8f32, -0.8, 0.0];
            let holding = [[-8.4f32, -20.7, -8.6], [-7.8, -23.3, -7.1]].iter().any(|palm| apart(theirs.left, std::array::from_fn(|k| theirs.magazine[k] + palm[k])) < 3.0);
            if holding && apart(theirs.magazine, rest) > 1.0 {
                let (mine, donor): ([f32; 3], [f32; 3]) = (std::array::from_fn(|k| ours.left[k] - ours.magazine[k]), std::array::from_fn(|k| theirs.left[k] - theirs.magazine[k]));
                *on_magazine += 1;
                *worst_on_magazine = worst_on_magazine.max(apart(mine, donor));
            }
            // And at the handle's fullest pull, where the hand is from
            // the donor's.
            let travel = 47.6 - theirs.handle[0];
            if pulled.is_none_or(|(_, most)| travel > most) {
                *pulled = Some((std::array::from_fn(|k| ours.left[k] - theirs.left[k]), travel));
            }
        }
    };

    // At rest in the hands: the left hand on the handguard, 5.6 cm ahead
    // of where the donor has it, 2 cm higher and 0.9 cm nearer the bore.
    for s in [&mut a, &mut b] {
        s.logic.start_drawn();
    }
    step(&mut a, &mut b, 0.5);
    let (ours_at, theirs_at) = (read(&mut a), read(&mut b));
    let rested: [f32; 3] = std::array::from_fn(|k| ours_at.left[k] - theirs_at.left[k]);
    assert!(apart(rested, [5.6, 2.0, 0.9]) < 0.02, "{rested:?}");
    // Its magazine's bone is at this rifle's magazine well.
    let moved: [f32; 3] = std::array::from_fn(|k| ours_at.magazine[k] - theirs_at.magazine[k]);
    assert!(apart(moved, [5.30, 0.48, 0.0]) < 0.05, "{moved:?}");

    // A reload from empty: the magazine changed, the handle pulled.
    for s in [&mut a, &mut b] {
        (s.logic.chamber, s.logic.magazine) = (false, Some(0));
        s.logic.reload();
    }
    step(&mut a, &mut b, 5.0);
    assert!(a.logic.is_idle() && a.logic.chamber && a.logic.magazine == Some(29));
    let (frames, on_magazine, worst_on_magazine, pulled) = *tally.borrow();
    let (shift, travel) = pulled.expect("the handle was pulled");
    println!(
        "the StG 44's reload from empty: {frames} frames in the donor's states; the hand on the magazine for {on_magazine} of them, within {worst_on_magazine:.2} cm of the donor's hold; at the handle's full pull ({travel:.1} cm) the palm is {shift:.2?} cm from the donor's"
    );
    assert!(on_magazine > 40 && worst_on_magazine < 1.0, "{on_magazine} {worst_on_magazine}");
    assert!((travel - 10.6).abs() < 0.2 && apart(shift, [-6.9, 0.0, 1.1]) < 0.3, "{travel} {shift:?}");

    // A shot, a switch to automatic, a look at the magazine and one into
    // the chamber: all the donor's, frame for frame.
    for s in [&mut a, &mut b] {
        s.logic.set_trigger(true, true);
    }
    step(&mut a, &mut b, 0.1);
    for s in [&mut a, &mut b] {
        s.logic.set_trigger(false, true);
    }
    step(&mut a, &mut b, 0.5);
    assert_eq!(a.logic.magazine, Some(28));
    // The button that is this rifle's selector turns in place with the
    // donor's: it is round, and its middle stays where it is.
    let button = |s: &mut Side| {
        let moved = s.rig.moved_bone();
        s.rig.pose(s.logic.animator().pose(), moved);
        let (weapon, node) = (bone(s.weapon, ""), bone(s.weapon, "/mod_trigger_group/mod_selector"));
        on_weapon(&s.rig, weapon, node)
    };
    let switch = |a: &mut Side, b: &mut Side| {
        a.logic.next_fire_mode();
        b.logic.next_fire_mode();
        step(a, b, 2.0);
        button(a)
    };
    let automatic = switch(&mut a, &mut b);
    assert_eq!(a.logic.mode(), trp_sim::FireMode::FullAuto);
    // The donor's selector turns about the axis across the weapon (45
    // degrees between its two modes, by its clips), and this rifle's
    // selector is a round button across the weapon: its bone is at the
    // button's own middle, so the turn leaves the button where it is.
    let custom = ours.custom.as_ref().expect("its own parts");
    for part in custom.parts.iter().filter(|part| part.mesh.name.starts_with("Body_FiremodeButton")) {
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &part.mesh.positions {
            (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
        }
        // Ahead and up, in centimetres, as the bone is given.
        let middle = [-(low[1] + high[1]) * 50.0, (low[2] + high[2]) * 50.0];
        assert!((middle[0] - automatic.0[0]).abs() < 0.06 && (middle[1] - automatic.0[1]).abs() < 0.06, "{} {middle:?} {:?}", part.mesh.name, automatic.0);
    }
    assert!(apart(automatic.0, [26.52, -1.67, 0.0]) < 0.05 && quat_mul(automatic.1, [0.0, 0.0, 0.0, 1.0])[1].abs() < 1e-3, "{automatic:?}");
    for check in [0, 1] {
        for s in [&mut a, &mut b] {
            if check == 0 {
                s.logic.check_magazine();
            } else {
                s.logic.check_chamber();
            }
        }
        step(&mut a, &mut b, 4.0);
        assert!(a.logic.is_idle());
    }
    // Held, it fires at its own rate: 540 rounds a minute.
    for s in [&mut a, &mut b] {
        s.logic.set_trigger(true, true);
    }
    step(&mut a, &mut b, 1.0);
    let fired = 28 - a.logic.magazine.unwrap_or(0);
    println!("held for a second on automatic it fired {fired} rounds");
    assert!((8..=10).contains(&fired), "{fired}");
}

/// The AKM's magazine taken out and put in from the inventory (firing spec,
/// "A magazine taken out or put in from the inventory"), against course 3
/// of `courses2`: the hands take them only while the logic idles.
#[test]
fn the_akm_s_magazine_is_moved_by_the_hands_from_the_inventory() {
    use trp_sim::weapon_animator::WeaponAnimatorData;
    use trp_sim::{Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    const FRAME: f32 = 1.0 / 120.0;
    // When each of the events wanted came, from the operation's start,
    // until the move is made.
    let run = |w: &mut Firearm, wanted: &[FirearmEvent]| {
        let (mut said, mut at) = (Vec::new(), 0.0);
        while at < 4.0 {
            let events = w.step(FRAME);
            at += FRAME;
            let done = events.contains(&FirearmEvent::MagazineMoved);
            said.extend(events.into_iter().filter(|event| wanted.contains(event)).map(|event| (event, at)));
            if done {
                break;
            }
        }
        said
    };
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    w.set_inventory(true);
    for _ in 0..240 {
        w.step(FRAME);
    }
    assert!(w.in_inventory() && w.can_move_magazine());
    // Out: the magazine out of the weapon's animation 0.885 s after the
    // operation began, the operation over at 1.435 s (course 3,
    // `3-menu-weapon`, the rifle in hand unloaded from the menu).
    assert!(w.pull_out_magazine());
    assert!(!w.can_move_magazine() && !w.pull_out_magazine() && !w.insert_magazine(30));
    let said = run(&mut w, &[FirearmEvent::MagazineOut, FirearmEvent::MagazineMoved]);
    eprintln!("out: {said:?}");
    let when = |said: &[(FirearmEvent, f32)], what: FirearmEvent| said.iter().find(|(event, _)| *event == what).map(|(_, at)| *at).expect("said");
    assert!((when(&said, FirearmEvent::MagazineOut) - 0.885).abs() <= 2.0 * FRAME);
    assert!((when(&said, FirearmEvent::MagazineMoved) - 1.435).abs() <= 2.0 * FRAME);
    assert_eq!((w.magazine, w.chamber), (None, true));
    assert!(w.can_move_magazine() && !w.pull_out_magazine());
    for _ in 0..120 {
        w.step(FRAME);
    }
    assert!(w.in_inventory(), "back down in its inventory pose");
    // In, with a round chambered: in at its mark, and over there.
    assert!(w.insert_magazine(30));
    // Until its `MagShow` the magazine going in is out of sight (a build of
    // another magazine may take over unseen); from it, in sight to the end
    // (the game's model shows the new magazine from there).
    assert!(w.magazine_out_of_sight() && !w.magazine_shown_going_in());
    let mut shown = false;
    for _ in 0..480 {
        if w.step(FRAME).contains(&FirearmEvent::MagazineShown) {
            shown = true;
            break;
        }
        assert!(w.magazine_out_of_sight() && !w.magazine_shown_going_in());
    }
    assert!(shown && !w.magazine_out_of_sight() && w.magazine_shown_going_in(), "in sight from its MagShow");
    let said = run(&mut w, &[FirearmEvent::MagazineIn, FirearmEvent::MagazineMoved]);
    assert!(!w.magazine_shown_going_in(), "over with the operation");
    eprintln!("in: {said:?}");
    assert_eq!(when(&said, FirearmEvent::MagazineIn), when(&said, FirearmEvent::MagazineMoved));
    assert_eq!((w.magazine, w.chamber), (Some(30), true));
    // With the chamber empty (the weapon fired dry) a round is chambered
    // from it, and the move is made then.
    let mut w = Firearm::new(settings.clone(), &data);
    w.magazine = Some(1);
    w.start_drawn();
    for _ in 0..3 {
        w.set_trigger(true, true);
        for _ in 0..30 {
            w.step(FRAME);
        }
        w.set_trigger(false, true);
        for _ in 0..90 {
            w.step(FRAME);
        }
    }
    assert_eq!((w.magazine, w.chamber), (Some(0), false));
    w.set_inventory(true);
    for _ in 0..240 {
        w.step(FRAME);
    }
    assert!(w.pull_out_magazine());
    run(&mut w, &[]);
    for _ in 0..120 {
        w.step(FRAME);
    }
    assert!(w.insert_magazine(30));
    let said = run(&mut w, &[FirearmEvent::MagazineIn, FirearmEvent::MagazineMoved]);
    eprintln!("in, chambering: {said:?}");
    assert!(when(&said, FirearmEvent::MagazineMoved) > when(&said, FirearmEvent::MagazineIn));
    assert_eq!((w.magazine, w.chamber), (Some(29), true));
}

/// A round put into the AKM's empty chamber from the inventory (firing
/// spec, "A round into the chamber from the inventory"): refused while a
/// magazine is in or the chamber is full; with neither, the hands play
/// their load of one from the inventory pose, the round in at the state's
/// mark (0.885 of 3.767 s), the logic idle then and back down in the
/// inventory pose. A put-away meanwhile gives it up at once.
#[test]
fn a_round_is_put_into_the_akm_s_chamber_from_the_inventory() {
    use trp_sim::weapon_animator::WeaponAnimatorData;
    use trp_sim::{Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    assert!(settings.chamber_load);
    const FRAME: f32 = 1.0 / 120.0;
    let settle = |w: &mut Firearm, seconds: f32| {
        for _ in 0..(seconds / FRAME) as usize {
            w.step(FRAME);
        }
    };
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    w.set_inventory(true);
    settle(&mut w, 2.0);
    // A magazine in, or a round chambered already: refused.
    assert!(w.chamber && !w.can_load_chamber() && !w.load_chamber());
    w.chamber = false;
    assert!(!w.can_load_chamber(), "a magazine is in");
    assert!(w.pull_out_magazine());
    settle(&mut w, 2.5);
    assert_eq!(w.magazine, None);
    assert!(w.can_load_chamber() && w.load_chamber());
    let (mut at, mut entered, mut chambered) = (0.0, None, None);
    while at < 6.0 && chambered.is_none() {
        let events = w.step(FRAME);
        at += FRAME;
        let (state, _, next) = w.animator().state();
        if entered.is_none() && next.unwrap_or(state).ends_with("LOAD ONE") {
            entered = Some(at);
        }
        if events.contains(&FirearmEvent::Chambered) {
            chambered = Some(at);
        }
    }
    let (entered, chambered) = (entered.expect("LOAD ONE played"), chambered.expect("chambered"));
    eprintln!("LOAD ONE from {entered:.3} s, the round in at {chambered:.3} s");
    assert!((chambered - entered - 0.885 * 3.767).abs() < 0.15, "{entered} {chambered}");
    assert!(w.chamber && w.can_move_magazine() && !w.can_load_chamber());
    settle(&mut w, 1.5);
    assert!(w.in_inventory(), "back down in its inventory pose");
    // Given up by a put-away.
    w.chamber = false;
    w.set_inventory(false);
    settle(&mut w, 1.5);
    assert!(w.load_chamber());
    settle(&mut w, 0.2);
    w.hide(false);
    let mut gave_up = false;
    for _ in 0..240 {
        gave_up |= w.step(FRAME).contains(&FirearmEvent::ChamberGivenUp);
    }
    assert!(gave_up && !w.chamber);
}

/// The chambered round taken out of the AKM from the inventory (firing
/// spec, "The chambered round taken out from the inventory";
/// `DischargeOperation`): refused with a magazine in; with it out, the
/// hands play `DISCHARGE` (`akm_ammo_out`, 2.6 s) from the inventory pose
/// and the chamber is empty at its `MagHide` mark (0.695 of it), for good:
/// nothing chambers it again (the user's report of 2026-10-25, a PS round
/// put back in the chamber however often it was taken out).
#[test]
fn the_akm_s_chambered_round_is_taken_out_from_the_inventory() {
    use trp_sim::weapon_animator::WeaponAnimatorData;
    use trp_sim::{Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    const FRAME: f32 = 1.0 / 120.0;
    let settle = |w: &mut Firearm, seconds: f32| {
        for _ in 0..(seconds / FRAME) as usize {
            w.step(FRAME);
        }
    };
    let mut w = Firearm::new(settings, &data);
    w.start_drawn();
    w.set_inventory(true);
    settle(&mut w, 2.0);
    assert!(w.chamber && !w.can_discharge() && !w.discharge(), "a magazine is in");
    assert!(w.pull_out_magazine());
    settle(&mut w, 2.5);
    assert_eq!(w.magazine, None);
    assert!(w.chamber && w.can_discharge() && w.discharge());
    assert!(!w.can_discharge() && !w.can_load_chamber(), "the hands are busy");
    let (mut at, mut entered, mut emptied) = (0.0, None, None);
    while at < 6.0 && emptied.is_none() {
        let events = w.step(FRAME);
        at += FRAME;
        let (state, _, next) = w.animator().state();
        if entered.is_none() && next.unwrap_or(state).ends_with("DISCHARGE") {
            entered = Some(at);
        }
        if events.contains(&FirearmEvent::Discharged) {
            emptied = Some(at);
        }
        assert!(w.chamber == emptied.is_none(), "the round is in until the mark");
    }
    let (entered, emptied) = (entered.expect("DISCHARGE played"), emptied.expect("emptied"));
    eprintln!("DISCHARGE from {entered:.3} s, the chamber empty at {emptied:.3} s");
    assert!((emptied - entered - 0.695 * 2.6).abs() < 0.15, "{entered} {emptied}");
    assert!(!w.chamber && w.can_move_magazine() && !w.can_discharge() && w.can_load_chamber());
    settle(&mut w, 3.0);
    assert!(!w.chamber && w.in_inventory(), "empty, and back down in its inventory pose");
}

/// The quick reload on the install's AKM (firing spec, "The quick
/// reload"): `RELOAD OUT ALL` in place of `RELOAD START` and `RELOAD OUT`,
/// then the plain reload's states; set beside the plain one, with a round
/// chambered and from an empty chamber. Only the install's controller run
/// by the simulation: no recording has a quick reload.
#[test]
fn the_quick_reload_throws_the_akm_s_magazine_out() {
    use trp_sim::weapon_animator::WeaponAnimatorData;
    use trp_sim::{Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    assert!(settings.fast_reload, "the AKM's template allows it (isFastReload)");
    const FRAME: f32 = 1.0 / 120.0;
    // The states entered and the marks' events, each with when, until
    // the weapon is back at rest.
    let reload = |quick: bool, chambered: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.chamber, w.magazine) = (chambered, Some(if chambered { 12 } else { 0 }));
        w.start_drawn();
        if quick { w.quick_reload_with(30) } else { w.reload_with(30) }
        let name = |w: &Firearm| {
            let (state, _, next) = w.animator().state();
            next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
        };
        let (mut entered, mut said, mut last, mut at) = (Vec::new(), Vec::new(), String::new(), 0.0);
        while at < 10.0 {
            said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_))).map(|e| (e, at)));
            at += FRAME;
            let now = name(&w);
            if now != last {
                entered.push((now.clone(), at));
                last = now;
            }
            if w.is_idle() && at > 0.5 {
                break;
            }
        }
        let when = |what: &FirearmEvent| said.iter().find(|(event, _)| event == what).map(|(_, at)| *at).expect("said");
        let names: Vec<String> = entered.iter().map(|entry| entry.0.clone()).collect();
        let swap = said.iter().find_map(|(event, _)| matches!(event, FirearmEvent::MagazineSwap { .. }).then(|| event.clone()));
        (names, [when(&FirearmEvent::MagazineOut), when(&FirearmEvent::MagazineHidden), when(&FirearmEvent::MagazineIn), at], swap, (w.magazine, w.chamber))
    };
    for chambered in [true, false] {
        let (plain, plain_at, _, plain_after) = reload(false, chambered);
        let (quick, quick_at, swap, quick_after) = reload(true, chambered);
        println!("chambered {chambered}: plain {plain:?}, out {:.3} hidden {:.3} in {:.3} at rest {:.3}", plain_at[0], plain_at[1], plain_at[2], plain_at[3]);
        println!("chambered {chambered}: quick {quick:?}, out {:.3} hidden {:.3} in {:.3} at rest {:.3}", quick_at[0], quick_at[1], quick_at[2], quick_at[3]);
        assert_eq!(quick[0], "RELOAD OUT ALL");
        assert_eq!(&quick[1..], &plain[2..], "from the magazine going in the two are one");
        assert_eq!(swap, Some(FirearmEvent::MagazineSwap { left: Some(if chambered { 12 } else { 0 }), quick: true }));
        assert_eq!(quick_after, plain_after);
        // `RELOAD OUT ALL`'s marks are 0.486 and 0.602 of a 1.0 s clip.
        assert!((quick_at[0] - 0.486).abs() < 0.03 && (quick_at[1] - 0.602).abs() < 0.03, "{quick_at:?}");
        // From the magazine out on, the quick one is the plain one moved
        // earlier: the plain one leaves `RELOAD OUT` at 0.78 of 1.43 s
        // after 0.13 s of `RELOAD START`; the quick one `RELOAD OUT ALL`
        // at 0.9 of 1.0 s.
        let earlier = plain_at[2] - quick_at[2];
        assert!((earlier - (0.133 + 1.117 - 0.9)).abs() < 0.03, "{earlier}");
        assert!((plain_at[3] - quick_at[3] - earlier).abs() < 0.03);
    }
}

/// Reweighing a character (`Movement::set_weight`, done whenever what is
/// carried changes) takes so long: the whole and the root motion table's
/// copy, which it makes. Printed, not held to a figure.
#[test]
fn reweighing_is_timed() {
    let Some(mut m) = movement(10.0) else { return };
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let began = std::time::Instant::now();
    for step in 0..20 {
        m.set_weight(&hard, &db.config, 10.0 + step as f32).expect("reweighed");
    }
    let whole = began.elapsed().as_secs_f32() * 1000.0 / 20.0;
    let began = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(m.settings.table.clone());
    }
    let table = began.elapsed().as_secs_f32() * 1000.0 / 20.0;
    println!("reweighing: {whole:.2} ms a time, of which the table's copy {table:.2} ms");
}

/// What the inventory screen lights together, by the install's filters
/// (inventory spec, "The box under the mouse, and what goes with it"),
/// either way round: the AKM with a spare of its own magazine and its own
/// round, and that magazine with that round; the M4A1 likewise with its
/// own; neither weapon with the other's magazine or round, nor with the
/// other.
#[test]
fn a_weapon_goes_with_its_own_magazines_and_rounds_and_not_another_s() {
    use std::sync::Arc;
    use trp_sim::inventory::{go_together, Item, Kind, MAGAZINE_SLOT};
    let Ok(_install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let weapon = |template: &str| Item::of_list(&db, &trp_data::inventory::Loadout::of_preset(&db, template).expect("its preset")).expect("put together").0;
    // A spare of the magazine on it, and a stack of its own round.
    let own = |weapon: &Item| {
        let magazine = weapon.in_slot(MAGAZINE_SLOT).expect("a magazine").kind.clone();
        let round = db.items[&weapon.kind.template].props.get("defAmmo").and_then(|round| round.as_str()).expect("a round of its own");
        (Item::new(magazine), Item::new(Arc::new(Kind::of(&db, round).expect("the round"))))
    };
    let (akm, m4) = (weapon("59d6088586f774275f37482f"), weapon("5447a9cd4bdc2dbd208b4567"));
    let ((akm_magazine, akm_round), (m4_magazine, m4_round)) = (own(&akm), own(&m4));
    for (a, b) in [(&akm, &akm_magazine), (&akm, &akm_round), (&akm_magazine, &akm_round), (&m4, &m4_magazine), (&m4, &m4_round), (&m4_magazine, &m4_round)] {
        assert!(go_together(a, b) && go_together(b, a), "{} and {}", a.kind.name, b.kind.name);
    }
    for (a, b) in [(&akm, &m4_magazine), (&akm, &m4_round), (&m4, &akm_magazine), (&m4, &akm_round), (&akm_magazine, &m4_round), (&akm, &m4)] {
        assert!(!go_together(a, b) && !go_together(b, a), "{} and {}", a.kind.name, b.kind.name);
    }
}

/// A magazine's points for its rounds as its model has them, found breadth
/// first (the 6L10: five at its lips and one at its foot), its template's
/// visible ranges (1 to 3), and the AKM's own magazine's points under its
/// slot in the weapon as built.
#[test]
fn a_magazine_s_points_for_rounds_and_its_visible_ranges_are_read() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let key = db.items["5a01c29586f77474660c694c"].prefab_key().expect("the 6L10's model").to_owned();
    let (model, _) = trp_data::icon::model_of(&mut assets, &key).expect("the 6L10");
    let points = trp_sim::magazine_view::points(model.nodes.iter().map(|node| (node.parent, node.name.as_str())), 0);
    let numbers: Vec<u32> = points.iter().map(|&(_, number)| number).collect();
    println!("the 6L10's points: {numbers:?}");
    assert_eq!(numbers, [1, 2, 3, 4, 5, 30]);
    let kind = trp_sim::inventory::Kind::of(&db, "5a01c29586f77474660c694c").expect("its kind");
    assert_eq!(kind.visible_rounds, vec![[1, 3]]);
    assert!(!kind.hide_entrails);
    // A rifle has no visible ranges of its own.
    assert!(trp_sim::inventory::Kind::of(&db, "59d6088586f774275f37482f").expect("the AKM").visible_rounds.is_empty());
    let akm = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let slot = akm.hierarchy.nodes.iter().position(|node| node.name == "mod_magazine").expect("its magazine's slot");
    let magazine = akm.hierarchy.nodes.iter().position(|node| node.parent == Some(slot)).expect("its magazine");
    let points = trp_sim::magazine_view::points(akm.hierarchy.nodes.iter().map(|node| (node.parent, node.name.as_str())), magazine);
    println!("the AKM's own magazine's points: {:?}", points.iter().map(|&(_, number)| number).collect::<Vec<_>>());
    assert_eq!(points.len(), 6);
}

/// The player prefab's force reaction (`ForceReact`): what a blast's shake
/// pushes into the springs, read.
#[test]
fn the_force_reaction_is_read() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let mut assets = Assets::new(install).expect("manifest");
    let data = trp_data::player::HandsData::load(&mut assets).expect("the player prefab's tuning");
    let force = &data.force_react;
    println!("force reaction: intensity {}, noise {} {}, a hit's push shorter by up to {}, vals {:?}; mask {}", force.intensity, force.noise_xy, force.noise_z, force.delta_random, force.vals, data.mask);
    println!("camera rotation spring {:?}, camera position {:?}, hands rotation {:?}", data.camera_rotation, data.camera_position, data.hands_rotation);
    assert!(!force.vals.is_empty());
    assert!(data.mask & 8 != 0, "the prefab's mask has the force reaction");
}
