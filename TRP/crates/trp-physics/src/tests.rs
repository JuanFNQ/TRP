//! Tests of the physics world: sweeps against meshes, boxes and spheres, a body taken out, a box at rest.

use super::*;

/// A level's mesh collider (`Shape::Mesh`): a sphere swept down onto a
/// floor of two triangles meets it where it lies, its face up; swept up
/// from under it, the face's back, nothing (PhysX's sweeps take a
/// mesh's front faces only, as a particle's sweep asks).
#[test]
fn a_mesh_floor_meets_a_sweep_from_above_only() {
    let mut world = World::new(Settings::game());
    // Its corners in Unity's order: the cross product of the first two
    // edges points up.
    let points: std::sync::Arc<[V3]> = vec![[-1.0, 0.2, -1.0], [-1.0, 0.2, 1.0], [1.0, 0.2, 1.0], [1.0, 0.2, -1.0]].into();
    let triangles: std::sync::Arc<[[u32; 3]]> = vec![[0, 1, 2], [0, 2, 3]].into();
    world.add_static([0.0; 3], [0.0, 0.0, 0.0, 1.0], &[Collider { shape: Shape::Mesh { points, triangles, scale: [1.0; 3] }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }], 12);
    let (distance, normal) = world.sweep_sphere([0.1, 1.0, 0.1], [0.0, -1.0, 0.0], 2.0, 0.01).expect("the floor from above");
    assert!((distance - 0.79).abs() < 1e-3, "{distance}");
    assert!(normal[1] > 0.999, "{normal:?}");
    assert!(world.sweep_sphere([0.1, -1.0, 0.1], [0.0, 1.0, 0.0], 2.0, 0.01).is_none(), "its back from below");
}

/// A sweep says which still thing it met, by the order they were put
/// in; one that begins inside a box gives a depth below nought.
#[test]
fn a_sweep_says_what_it_met() {
    let mut world = World::new(Settings::game());
    let cube = |half: f32| [Collider { shape: Shape::Box { half: [half; 3] }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }];
    world.add_static([0.0, 0.0, 5.0], [0.0, 0.0, 0.0, 1.0], &cube(0.5), 18);
    world.add_static([0.0, 0.0, 2.0], [0.0, 0.0, 0.0, 1.0], &cube(0.5), 18);
    let (distance, normal, met) = world.sweep_sphere_met([0.0; 3], [0.0, 0.0, 1.0], 10.0, 0.01).expect("the nearer box");
    assert!((distance - 1.49).abs() < 1e-3 && normal[2] < -0.999, "{distance} {normal:?}");
    assert_eq!(met, Some(Met::Still(1)));
    let (inside, _, met) = world.sweep_sphere_met([0.0, 0.0, 1.9], [0.0, 0.0, 1.0], 0.05, 0.01).expect("from inside the nearer box");
    assert!(inside <= 0.0, "{inside}");
    assert_eq!(met, Some(Met::Still(1)));
}

/// A box swept with every touch asked for (Unity's `BoxCastNonAlloc`, a
/// knife's blade) is told the still things first and the moving bodies
/// after, whatever their distances: a kinematic body at 1 m comes after
/// a box at 3 m, where the nearest-first sweep meets the body. One that
/// begins inside a shape comes at nought. Each with its layer.
#[test]
fn a_box_sweep_is_told_the_still_things_before_the_bodies() {
    let mut world = World::new(Settings::game());
    let cube = |half: f32| [Collider { shape: Shape::Box { half: [half; 3] }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }];
    world.add_static([0.0, 0.0, 3.0], [0.0, 0.0, 0.0, 1.0], &cube(0.5), 12);
    let body = world.add_kinematic([0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0], &[Collider { shape: Shape::Sphere { radius: 0.2 }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }], 16);
    world.build_query_trees();
    let (half, turn, up) = ([0.05, 0.05, 0.05], [0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
    let touches = world.sweep_box_touches([0.0; 3], half, turn, up, 5.0);
    let told: Vec<(Option<Met>, u16)> = touches.iter().map(|t| (t.met, t.layer)).collect();
    assert_eq!(told, vec![(Some(Met::Still(0)), 12), (Some(Met::Body(body)), 16)]);
    assert!((touches[0].distance - 2.45).abs() < 1e-3 && (touches[1].distance - 0.75).abs() < 1e-3, "{touches:?}");
    let (nearest, ..) = world.sweep_box([0.0; 3], half, turn, up, 5.0).expect("the nearest");
    assert!((nearest - 0.75).abs() < 1e-3, "{nearest}");
    // The body moved (`set_pose`) behind the box, out of reach, and back
    // over the start: the box alone, then the body at nought.
    world.set_pose(body, [0.0, 0.0, 9.0], turn);
    assert_eq!(world.sweep_box_touches([0.0; 3], half, turn, up, 5.0).iter().map(|t| t.met).collect::<Vec<_>>(), vec![Some(Met::Still(0))]);
    world.set_pose(body, [0.0; 3], turn);
    let touches = world.sweep_box_touches([0.0; 3], half, turn, up, 5.0);
    assert_eq!(touches.len(), 2);
    assert_eq!((touches[1].met, touches[1].distance), (Some(Met::Body(body)), 0.0));
}

/// A swept box falling fast onto the floor (a mesh, as the game's) is
/// stopped by the sweep as it meets it: its velocity into the floor
/// taken away and its spin kept, as the game's quick-reload magazine's
/// was (`courses2`'s course 1, `docs/spec/physics.md`); onto a plane,
/// which PhysX never sweeps against, it is met only by the contact,
/// which turns it.
#[test]
fn a_swept_box_is_stopped_by_the_floor_as_the_games() {
    let first_impact = |floor: bool| {
        let mut world = World::new(Settings::game());
        if floor {
            world.add_ground(0.0, Material::default(), 11);
        } else {
            world.add_plane([0.0; 3], [0.0, 1.0, 0.0], Material::default(), 11);
        }
        // The game's magazine's box and its fall at the impact, the step
        // before it 4 cm above the floor.
        let body = world.add_body(&BodyDesc {
            at: [0.0, 0.2, 0.0],
            turn: crate::loose::euler(-40.0, 0.0, 15.0),
            colliders: vec![Collider { shape: Shape::Box { half: [0.0135, 0.0764, 0.1006] }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }],
            mass: 0.73,
            drag: 0.0,
            angular_drag: 0.05,
            velocity: [0.0, -4.8, 0.0],
            spin: [-0.15, 2.5, 2.7],
            max_depenetration: 10.0,
            continuous: true,
            layer: 15,
        });
        let dt = 1.0 / 120.0;
        let mut before = world.velocity(body);
        for _ in 0..40 {
            world.step(dt);
            let after = world.velocity(body);
            if (after.0[1] - (before.0[1] - 9.81 * dt)).abs() > 0.5 {
                return (before, after);
            }
            before = after;
        }
        panic!("never met the ground");
    };
    let spun = |(before, after): ((V3, V3), (V3, V3))| (0..3).map(|k| (after.1[k] - before.1[k]).abs()).fold(0.0f32, f32::max);
    let floor = first_impact(true);
    assert!(floor.1 .0[1].abs() < 0.1 && spun(floor) < 0.05, "onto the floor: {floor:?}");
    let plane = first_impact(false);
    assert!(spun(plane) > 1.0, "onto a plane: {plane:?}");
}

/// A body taken out takes its joints with it (`remove`): the weapon
/// hung on a corpse's pelvis by the strap's spring, taken, leaves the
/// pelvis falling free, its joint released.
#[test]
fn a_body_taken_out_takes_its_joints() {
    let mut world = World::new(Settings::game());
    let ball = |at: V3, mass: f32| BodyDesc {
        at,
        turn: [0.0, 0.0, 0.0, 1.0],
        colliders: vec![Collider { shape: Shape::Sphere { radius: 0.1 }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }],
        mass,
        drag: 0.0,
        angular_drag: 0.05,
        velocity: [0.0; 3],
        spin: [0.0; 3],
        max_depenetration: 10.0,
        continuous: false,
        layer: 0,
    };
    let pelvis = world.add_body(&ball([0.0, 10.0, 0.0], 10.0));
    let weapon = world.add_body(&ball([0.5, 10.0, 0.0], 1.0));
    world.spring_joint(weapon, [0.0; 3], pelvis, [0.5, 0.0, 0.0], 33.0, 3.0, [0.0, 0.0], true, 10.0);
    assert_eq!(world.joints.len(), 1);
    world.remove(weapon);
    assert!(world.joints.is_empty(), "the strap released with the weapon");
    let dt = 1.0 / 60.0;
    for _ in 0..30 {
        world.step(dt);
    }
    // Free fall for half a second: 4.905 m/s down.
    let (velocity, _) = world.velocity(pelvis);
    assert!((velocity[1] + 9.81 * 0.5).abs() < 0.05 && velocity[0].abs() < 1e-3, "{velocity:?}");
}

/// A box let fall on the ground comes to rest on it, half its height
/// up, and goes to sleep.
#[test]
fn a_box_falls_and_rests() {
    let mut world = World::new(Settings::game());
    world.add_ground(0.0, Material::default(), 0);
    let body = world.add_body(&BodyDesc {
        at: [0.0, 1.0, 0.0],
        turn: [0.0, 0.0, 0.0, 1.0],
        colliders: vec![Collider { shape: Shape::Box { half: [0.1, 0.05, 0.2] }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }],
        mass: 0.5,
        drag: 0.0,
        angular_drag: 0.05,
        velocity: [0.0; 3],
        spin: [0.0; 3],
        max_depenetration: 10.0,
        continuous: false,
        layer: 1,
    });
    for _ in 0..600 {
        world.step(1.0 / 120.0);
    }
    let (at, _) = world.pose(body);
    assert!((at[1] - 0.05).abs() < 0.01, "{at:?}");
    assert!(world.asleep(body));
    let (centre, inertia) = world.mass_properties(body);
    assert!(centre.iter().all(|c| c.abs() < 1e-5));
    // A box's own turning: m (b^2 + c^2) / 12 about each axis.
    assert!((inertia[0] - 0.5 * (0.01 + 0.16) / 12.0).abs() < 1e-5, "{inertia:?}");
}

/// A sphere swept down onto the ground and onto a box meets each where
/// it should, with the surface's normal; one swept clear meets nothing.
#[test]
fn a_swept_sphere_meets_the_ground_and_a_box() {
    let mut world = World::new(Settings::game());
    world.add_ground(0.0, Material::default(), 0);
    world.add_static([2.0, 0.5, 0.0], [0.0, 0.0, 0.0, 1.0], &[Collider { shape: Shape::Box { half: [0.5; 3] }, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }], 0);
    world.step(1.0 / 60.0);
    let (d, n) = world.sweep_sphere([0.0, 1.0, 0.0], [0.0, -1.0, 0.0], 2.0, 0.1).expect("the ground");
    assert!((d - 0.9).abs() < 1e-3 && (n[1] - 1.0).abs() < 1e-4, "{d} {n:?}");
    let (d, n) = world.sweep_sphere([0.0, 0.5, 0.0], [1.0, 0.0, 0.0], 3.0, 0.1).expect("the box's side");
    assert!((d - 1.4).abs() < 1e-3 && (n[0] + 1.0).abs() < 1e-4, "{d} {n:?}");
    assert!(world.sweep_sphere([0.0, 3.0, 0.0], [1.0, 0.0, 0.0], 3.0, 0.1).is_none());
}
