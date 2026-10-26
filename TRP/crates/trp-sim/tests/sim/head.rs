//! The head's push out of walls (`docs/spec/movement.md`, "The head's push
//! out of walls"), with the install's prefab, movement and body animator: the
//! two capsules as the prefab has them, and a walk into a wall that the head
//! meets before the body's capsule does. These pass trivially, with a note,
//! where no install is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HeadCollidersData;
use trp_data::unity::Assets;
use trp_sim::collision::{Block, Site};
use trp_sim::movement::Capsule;
use trp_sim::{Input, Movement, World};

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

#[test]
fn the_head_s_two_capsules_are_the_prefab_s() {
    let Some(install) = crate::fixture::install() else { return };
    let head = HeadCollidersData::load(&mut Assets::new(install).expect("manifest")).expect("the head's colliders");
    let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-5);
    assert!((head.standing.radius - 0.22).abs() < 1e-6 && (head.standing.height - 0.7).abs() < 1e-6 && head.standing.direction == 1);
    assert!(close(head.standing.centre, [0.0, -0.2, -0.1]) && close(head.standing.position, [0.0, 0.15, 0.15]));
    assert!((head.lying.radius - 0.175).abs() < 1e-6 && (head.lying.height - 1.6).abs() < 1e-6 && head.lying.direction == 2);
    assert!(close(head.lying.centre, [0.0, 0.15, 0.0]));
    assert_eq!(head.lying_pivot, "Base HumanSpine2");
}

/// Level ground with a wall square to z, its face at `face`: the body's
/// capsule kept out as the game's controller keeps it (`Site`), the head's
/// pushed out of it.
struct Wall {
    site: Site,
}

impl Wall {
    fn at(face: f32) -> Self {
        Self { site: Site { ground: 0.0, blocks: vec![Block::square([-2.0, 0.0, face], [2.0, 2.5, face + 0.3])] } }
    }
}

impl World for Wall {
    fn ground_height(&self, x: f32, z: f32) -> f32 {
        self.site.height(x, z)
    }

    fn obstructed(&self, from: [f32; 3], to: [f32; 3], capsule: Capsule) -> [f32; 3] {
        self.site.settle(from, to, capsule, 0.366)
    }

    fn sets_feet(&self) -> bool {
        true
    }

    fn capsule_push(&self, ends: [[f32; 3]; 2], radius: f32) -> [f32; 3] {
        let mut push = [0.0; 3];
        for (way, depth) in self.site.blocks.iter().filter_map(|b| b.capsule_push(ends, radius)) {
            for k in 0..3 {
                push[k] += way[k] * (depth + trp_sim::movement::pom::MARGIN);
            }
        }
        push
    }
}

#[test]
fn a_lean_into_a_wall_is_pushed_off_by_the_head() {
    let Some(mut m) = movement() else { return };
    // Facing +z, a wall to the right, its face 0.4 m from the axis: the
    // body's capsule (0.366) clear of it, the head leaned into it.
    let world = Wall { site: Site { ground: 0.0, blocks: vec![Block::square([0.4, 0.0, -2.0], [0.7, 2.5, 2.0])] } };
    for _ in 0..60 {
        m.step(DT, &Input::default(), &world);
    }
    let mut most: f32 = 0.0;
    for _ in 0..240 {
        m.step(DT, &Input { lean: 1.0, ..Input::default() }, &world);
        most = most.max(m.head_push()[0].abs());
    }
    let capsule = m.head_capsule().expect("the head's capsule");
    let side = capsule.ends.iter().map(|end| end[0] + capsule.radius).fold(f32::MIN, f32::max);
    println!("leaned: the feet {:.3} m from the wall, the head {:.3} m; the largest push {most:.3} m", 0.4 - m.position[0], 0.4 - side);
    assert!(most > 0.01, "the head was pushed: {most}");
    assert!(m.position[0] < -0.01, "the body carried off the wall: {:?}", m.position);
    assert!(0.4 - side > -0.012, "the head not left in the wall: {}", 0.4 - side);
}

