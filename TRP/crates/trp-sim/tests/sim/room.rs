//! Room to move (`docs/spec/movement.md`, "Room to move"), with the install's
//! movement and body animator: the step aside, the roll, lying down,
//! standing up and the crawl refused where something is in the way or the
//! ground is not there, in a world of level ground, upright boxes and a
//! drop. These pass trivially, with a note, where no install is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::collision::Block;
use trp_sim::math::quat_rotate;
use trp_sim::{Input, Movement, State, World};

const DT: f32 = 1.0 / 120.0;

fn movement() -> Option<Movement> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = HardSettings::load(&assets.install).expect("settings object");
    let engine = EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement settings");
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    Some(Movement::new(settings, trp_sim::Body::new(&body, 5.0)))
}

/// Level ground at nought, falling to a metre down past `drop` along x if
/// given, with upright boxes on or over it.
#[derive(Default)]
struct Room {
    boxes: Vec<Block>,
    drop: Option<f32>,
}

impl World for Room {
    fn ground_height(&self, x: f32, _z: f32) -> f32 {
        if self.drop.is_some_and(|edge| x > edge) {
            -1.0
        } else {
            0.0
        }
    }

    fn box_overlaps(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4]) -> bool {
        let forward = quat_rotate(turn, [0.0, 0.0, 1.0]);
        let yaw = forward[0].atan2(forward[2]).to_degrees();
        centre[1] - half[1] < self.ground_height(centre[0], centre[2]) || self.boxes.iter().any(|b| b.upright_box_overlaps(centre, half, yaw))
    }

    fn sphere_cast(&self, from: [f32; 3], radius: f32, direction: [f32; 3], length: f32) -> Option<f32> {
        let touches = |along: f32| {
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * along);
            at[1] - radius < self.ground_height(at[0], at[2]) || self.boxes.iter().any(|b| b.sphere_overlaps(at, radius))
        };
        (1..=130).map(|i| length * i as f32 / 130.0).find(|along| touches(*along))
    }
}

fn run(m: &mut Movement, seconds: f32, input: &Input, world: &dyn World) {
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, input, world);
    }
}

/// A wall square to x, its near face at `x`, two metres high and long.
fn wall_at_x(x: f32) -> Block {
    Block::square([x, 0.0, -1.0], [x + 0.2, 2.0, 1.0])
}

#[test]
fn a_step_aside_into_a_wall_or_over_an_edge_is_refused() {
    let Some(mut m) = movement() else { return };
    // Facing +z, right is +x: a wall 0.6 m to the right.
    let world = Room { boxes: vec![wall_at_x(0.6)], drop: None };
    run(&mut m, 0.5, &Input::default(), &world);
    run(&mut m, 0.3, &Input { step: 1, ..Input::default() }, &world);
    assert_eq!((m.step, m.state), (0, State::Idle), "into the wall");
    run(&mut m, 0.3, &Input::default(), &world);
    run(&mut m, 0.3, &Input { step: -1, ..Input::default() }, &world);
    assert_eq!((m.step, m.state), (-1, State::Sidestep), "away from it");

    // A metre's drop 0.5 m to the right.
    let Some(mut m) = movement() else { return };
    let world = Room { boxes: vec![], drop: Some(0.5) };
    run(&mut m, 0.5, &Input::default(), &world);
    run(&mut m, 0.3, &Input { step: 1, ..Input::default() }, &world);
    assert_eq!((m.step, m.state), (0, State::Idle), "over the edge");
}

#[test]
fn lying_down_needs_length_and_level_ground() {
    let Some(mut m) = movement() else { return };
    // A wall 0.6 m ahead: not the half metre and more the body needs.
    let world = Room { boxes: vec![Block::square([-1.0, 0.0, 0.6], [1.0, 2.0, 0.8])], drop: None };
    run(&mut m, 0.5, &Input::default(), &world);
    m.step(DT, &Input { prone: true, ..Input::default() }, &world);
    run(&mut m, 1.0, &Input::default(), &world);
    assert!(!m.prone && m.state == State::Idle, "{:?}", m.state);
    // In the open it lies down.
    let open = Room::default();
    m.step(DT, &Input { prone: true, ..Input::default() }, &open);
    run(&mut m, 2.0, &Input::default(), &open);
    assert!(m.prone && m.state == State::ProneIdle, "{:?}", m.state);
}

#[test]
fn standing_up_needs_headroom() {
    let Some(mut m) = movement() else { return };
    let open = Room::default();
    run(&mut m, 0.5, &Input::default(), &open);
    m.step(DT, &Input { prone: true, ..Input::default() }, &open);
    run(&mut m, 2.0, &Input::default(), &open);
    assert_eq!(m.state, State::ProneIdle);
    // A ceiling at a metre: no room even to crouch, the character lies on.
    let low = Room { boxes: vec![Block::square([-2.0, 1.0, -2.0], [2.0, 1.3, 2.0])], drop: None };
    m.step(DT, &Input { prone: true, ..Input::default() }, &low);
    run(&mut m, 1.0, &Input::default(), &low);
    assert!(m.prone, "lies on under a metre");
    // At 1.4 m: room to crouch, not to stand; up into a crouch.
    let middling = Room { boxes: vec![Block::square([-2.0, 1.4, -2.0], [2.0, 1.7, 2.0])], drop: None };
    m.step(DT, &Input { prone: true, ..Input::default() }, &middling);
    run(&mut m, 2.0, &Input::default(), &middling);
    assert!(!m.prone && m.pose_level == 0.0, "up crouched: prone {} stance {}", m.prone, m.pose_level);
    // And the crouch key does not stand it up there.
    m.step(DT, &Input { crouch: true, ..Input::default() }, &middling);
    assert_eq!(m.pose_level, 0.0);
    // In the open it does.
    m.step(DT, &Input { crouch: true, ..Input::default() }, &open);
    assert_eq!(m.pose_level, 1.0);
}

#[test]
fn a_roll_into_a_wall_is_refused() {
    let Some(mut m) = movement() else { return };
    let open = Room::default();
    run(&mut m, 0.5, &Input::default(), &open);
    m.step(DT, &Input { prone: true, ..Input::default() }, &open);
    run(&mut m, 2.0, &Input::default(), &open);
    assert_eq!(m.state, State::ProneIdle);
    // A wall 0.5 m to the right, low enough to be in the roll's box.
    let world = Room { boxes: vec![wall_at_x(0.5)], drop: None };
    run(&mut m, 1.0, &Input { step: 1, ..Input::default() }, &world);
    assert_eq!((m.step, m.state), (0, State::ProneIdle), "into the wall");
    run(&mut m, 0.2, &Input::default(), &world);
    run(&mut m, 0.5, &Input { step: -1, ..Input::default() }, &world);
    assert_eq!(m.state, State::Roll, "away from it");
}

#[test]
fn a_crawl_toward_an_edge_is_turned_back() {
    let Some(mut m) = movement() else { return };
    // Facing +x, the ground drops away 1.2 m ahead.
    let world = Room { boxes: vec![], drop: Some(1.2) };
    m.step(DT, &Input { look_delta: [90.0, 0.0], ..Input::default() }, &world);
    run(&mut m, 0.5, &Input::default(), &world);
    m.step(DT, &Input { prone: true, ..Input::default() }, &world);
    run(&mut m, 2.0, &Input::default(), &world);
    assert_eq!(m.state, State::ProneIdle);
    let crawl = Input { move_dir: [0.0, 1.0], ..Input::default() };
    let mut furthest: f32 = 0.0;
    for _ in 0..1200 {
        m.step(DT, &crawl, &world);
        furthest = furthest.max(m.position[0]);
    }
    // The body needs steady ground 0.6 m past the capsule's side ahead.
    assert!(furthest > 0.2 && furthest < 1.2, "crawled to {furthest}");
    println!("crawled to {furthest:.3} m of the 1.2 m to the edge");
    assert!(m.position[1].abs() < 1e-3, "still on the top: {:?}", m.position);
}
