//! Lying down, crawling, getting up and a reload holding a lying body still.

use super::*;

/// A lying body is laid to the ground, and its roll is limited the way
/// the game limits it: ground a touch lower on the left gives the true
/// small roll, ground a touch lower on the right a full 8 degrees the
/// wrong way. Getting up clears it.
#[test]
fn a_lying_body_is_laid_to_the_ground() {
    use trp_sim::base::math::quat_to_euler;
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
