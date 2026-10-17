use trp_physics::*;

fn body(world: &mut World, shape: Shape) -> Body {
    world.add_body(&BodyDesc {
        at: [0.0, 1.0, 0.0],
        turn: [0.0, 0.0, 0.0, 1.0],
        colliders: vec![Collider { shape, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }],
        mass: 1.0,
        drag: 0.0,
        angular_drag: 0.05,
        velocity: [0.0; 3],
        spin: [0.0; 3],
        max_depenetration: 4.0,
        continuous: false,
        layer: 23,
    })
}

/// Every kind of shape is made and stepped; a convex hull of a box's
/// corners (and a point inside, which the hull leaves out) falls and rests
/// on the ground as the box would, its centre of mass at the box's.
#[test]
fn every_shape() {
    let mut world = World::new(Settings::game());
    world.set_layers_meet(23, 23, true);
    world.add_ground(0.0, Material::default(), 11);
    body(&mut world, Shape::Sphere { radius: 0.1 });
    body(&mut world, Shape::Capsule { radius: 0.05, height: 0.3, direction: 1 });
    body(&mut world, Shape::Box { half: [0.1, 0.1, 0.1] });
    let mut points: Vec<V3> = (0..8).map(|i| [if i & 1 == 0 { -0.1 } else { 0.1 }, if i & 2 == 0 { 0.0 } else { 0.2 }, if i & 4 == 0 { -0.1 } else { 0.1 }]).collect();
    points.push([0.0, 0.1, 0.0]);
    // In a world of its own, the others not in its way.
    let mut world = World::new(Settings::game());
    world.add_ground(0.0, Material::default(), 11);
    let hull = body(&mut world, Shape::Convex { points: points.into(), scale: [1.0, 0.5, 1.0] });
    let (centre, _) = world.mass_properties(hull);
    assert!(centre[0].abs() < 1e-4 && (centre[1] - 0.05).abs() < 1e-4 && centre[2].abs() < 1e-4, "{centre:?}");
    for _ in 0..240 {
        world.step(1.0 / 120.0);
    }
    let (at, _) = world.pose(hull);
    // The hull's underside, its origin, on the ground.
    assert!(at[1].abs() < 0.02, "{at:?}");
}
