//! Slopes and sliding (`docs/spec/movement.md`, "Slopes and sliding"), and
//! the sprint's slide, with the install's movement and body animator: ground
//! too steep to stand on carries the character down it, ground that is not
//! is stood on, a walk down a slope keeps to it, and a crouch in a sprint
//! is the crouched way out of it. These pass trivially, with a note, where
//! no install is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::unity::Assets;
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

/// A plane through the origin falling toward +x at `degrees`.
struct Plane(f32);

impl World for Plane {
    fn ground_height(&self, x: f32, _z: f32) -> f32 {
        -self.0.to_radians().tan() * x
    }

    fn ground_normal(&self, _x: f32, _z: f32) -> [f32; 3] {
        let (sin, cos) = self.0.to_radians().sin_cos();
        [sin, cos, 0.0]
    }
}

fn run(m: &mut Movement, seconds: f32, input: &Input, world: &dyn World) {
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, input, world);
    }
}

#[test]
fn ground_too_steep_to_stand_on_carries_the_character_down_it() {
    let Some(mut m) = movement() else { return };
    let world = Plane(60.0);
    run(&mut m, 0.5, &Input::default(), &world);
    let before = m.position;
    run(&mut m, 0.5, &Input::default(), &world);
    let gone: [f32; 3] = std::array::from_fn(|k| m.position[k] - before[k]);
    // Down the slope, on it.
    assert!(gone[0] > 0.1 && gone[1] < 0.0, "{gone:?}");
    let ground = world.ground_height(m.position[0], m.position[2]);
    assert!((m.position[1] - ground).abs() < 1e-3, "on the ground: {} against {ground}", m.position[1]);
    // The pull is 0.4 g down the slope, 3.9 m/s; standing's hard cap of an
    // untrained character holds the level part to 0.5 m/s.
    let level = (gone[0] * gone[0] + gone[2] * gone[2]).sqrt() / 0.5;
    assert!((level - 0.5).abs() < 0.01, "level pace {level}");
    assert!(m.slope[1] > 0.0, "the pull points up the slope: {:?}", m.slope);
}

#[test]
fn ground_under_the_threshold_is_stood_on() {
    let Some(mut m) = movement() else { return };
    let world = Plane(40.0);
    run(&mut m, 0.5, &Input::default(), &world);
    let before = m.position;
    run(&mut m, 1.0, &Input::default(), &world);
    assert_eq!(m.slope, [0.0; 3]);
    assert!((m.position[0] - before[0]).abs() < 1e-4, "{:?} from {before:?}", m.position);
}

#[test]
fn a_walk_down_a_slope_keeps_to_it() {
    let Some(mut m) = movement() else { return };
    // Facing +x, down the slope.
    let world = Plane(20.0);
    let turn = Input { look_delta: [90.0, 0.0], ..Input::default() };
    m.step(DT, &turn, &world);
    run(&mut m, 0.5, &Input::default(), &world);
    let walk = Input { move_dir: [0.0, 1.0], ..Input::default() };
    let mut highest: f32 = 0.0;
    for _ in 0..240 {
        m.step(DT, &walk, &world);
        highest = highest.max(m.position[1] - world.ground_height(m.position[0], m.position[2]));
        assert!(m.grounded);
    }
    assert!(m.position[0] > 1.0, "walked down: {:?}", m.position);
    // Laid along the ground, the feet never leave it.
    assert!(highest < 1e-3, "rode {highest} m over the ground");
}

#[test]
fn a_crouch_in_a_sprint_is_the_slide() {
    let Some(mut m) = movement() else { return };
    let world = trp_sim::Flat;
    // Walking first: a sprint key held before forward is not taken.
    run(&mut m, 0.5, &Input { move_dir: [0.0, 1.0], ..Input::default() }, &world);
    let sprint = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    run(&mut m, 2.0, &sprint, &world);
    assert_eq!(m.state, State::Sprint);
    m.step(DT, &Input { crouch: true, ..sprint }, &world);
    // A full crouch at once, the smoothed stance with it.
    assert_eq!((m.pose_level, m.smoothed_pose_level), (0.0, 0.0));
    // Forward and sprint still held: the sprint is let go of, and the body
    // goes out of it crouched.
    let mut left = None;
    for frame in 0..120 {
        m.step(DT, &sprint, &world);
        if m.state != State::Sprint {
            left = Some((frame, m.state, m.sprinting, m.pose_level));
            break;
        }
    }
    let (frame, state, sprinting, pose) = left.expect("out of the sprint");
    assert_eq!((state, sprinting, pose), (State::SprintStop, false, 0.0), "frame {frame}");
}
