//! A recorded fall: its first impact, the ground and the drift.

use super::*;

/// A fall in the engine, a place a recorded frame.
pub struct Fall {
    /// Per frame (the first the start): its centre of mass, its turn and
    /// whether its body is asleep (none once taken away).
    pub frames: Vec<(V3, Q4, Option<bool>)>,
    /// Per frame, its velocity and spin (none once taken away).
    pub motion: Vec<(V3, V3)>,
    /// The frame on which the watch took its body away.
    pub done: Option<usize>,
}

/// The first frame on which the recorded thing met something: its
/// velocity changed by more than its fall's on that frame (gravity's and
/// the drag's, 0.5 m/s more), or its spin by more than 0.2 rad/s.
pub fn first_impact(thing: &Thrown, gravity: f32) -> Option<usize> {
    let moving: Vec<(usize, V3, V3, f32)> = thing.frames.iter().enumerate().filter_map(|(i, f)| f.moving.map(|(v, w, _)| (i, v, w, f.step.unwrap_or(0.0)))).collect();
    moving.windows(2).find_map(|pair| {
        let ((_, v0, w0, _), (i, v1, w1, step)) = (pair[0], pair[1]);
        let expected = [v0[0], v0[1] + gravity * step, v0[2]];
        (distance(v1, expected) > 0.5 || distance(w1, w0) > 0.2).then_some(i)
    })
}

/// The ground under where a thing set off, as recorded: a plane through
/// the point met, facing along its normal, on the layer met (flat at the
/// thrower's feet where nothing was met).
pub fn ground_of(thing: &Thrown) -> (V3, V3, String) {
    thing.ground.clone().unwrap_or_else(|| (thing.player_pos.unwrap_or([0.0; 3]), [0.0, 1.0, 0.0], "Terrain".to_owned()))
}

/// A thing's fall in the engine: its body as `loose::add` makes it of
/// `colliders` at the recorded start, on the recorded ground of
/// `ground_material` (the level's is not recorded), stepped as the game
/// stepped on each recorded frame, the game's watch kept.
pub fn fall(thing: &Thrown, colliders: Vec<Collider>, physics: &trp_data::setup::engine::PhysicsSettings, ground_material: Material) -> Fall {
    fall_on(thing, colliders, physics, ground_of(thing), ground_material, true)
}

/// The same on a ground given: through a point, facing along a normal, on
/// a layer; a floor of two triangles as a level's floor is (`mesh`), or
/// a plane (which PhysX never sweeps a body against).
pub fn fall_on(thing: &Thrown, colliders: Vec<Collider>, physics: &trp_data::setup::engine::PhysicsSettings, ground: (V3, V3, String), ground_material: Material, mesh: bool) -> Fall {
    let mut world = World::new(Settings::of(physics));
    let (point, normal, layer) = ground;
    let layer = physics.layer(&layer).unwrap_or(11);
    if mesh {
        world.add_floor(point, normal, ground_material, layer);
    } else {
        world.add_plane(point, normal, ground_material, layer);
    }
    let loot = physics.layer("Loot").unwrap_or(15);
    let is_swept = swept(&colliders);
    let (body, shift) = loose::add(&mut world, colliders, thing.kg, &thing.start, SWEPT_UNDER, loot);
    let centre = |world: &World| {
        let (at, turn) = world.pose(body);
        (add(at, rotate(turn, shift)), turn)
    };
    let (at, turn) = centre(&world);
    let mut frames = vec![(at, turn, Some(world.asleep(body)))];
    let mut motion = vec![world.velocity(body)];
    let mut watch = Watch::default();
    let mut done = None;
    let mut lying = None;
    for (index, frame) in thing.frames.iter().enumerate().skip(1) {
        if let Some((at, turn)) = lying {
            frames.push((at, turn, None));
            motion.push(([0.0; 3], [0.0; 3]));
            continue;
        }
        if let Some(step) = frame.step {
            world.step(step);
        }
        let (at, turn) = centre(&world);
        let asleep = world.asleep(body);
        frames.push((at, turn, Some(asleep)));
        motion.push(world.velocity(body));
        if watch.frame(&world, body, is_swept, frame.dt) {
            done = Some(index);
            lying = Some((at, turn));
        }
    }
    Fall { frames, motion, done }
}

/// The mean distance of the fall from the recorded, over the frames to
/// `until` seconds after the start (the start left out), and the most.
pub fn drift(thing: &Thrown, fall: &Fall, until: f32) -> (f32, f32) {
    let (mut sum, mut most, mut n) = (0.0, 0.0f32, 0);
    for (frame, (at, _, _)) in thing.frames.iter().zip(&fall.frames).skip(1) {
        if thing.since(frame) > until + 1e-4 {
            break;
        }
        let d = distance(frame.at, *at);
        sum += d;
        most = most.max(d);
        n += 1;
    }
    if n == 0 { (f32::NAN, f32::NAN) } else { (sum / n as f32, most) }
}

/// Where it ends, recorded and ours: the last recorded frame's place and
/// turn beside ours on that frame (the distance, and the turns' angle).
pub fn end(thing: &Thrown, fall: &Fall) -> Option<(f32, f32)> {
    let (frame, (at, turn, _)) = thing.frames.iter().zip(&fall.frames).last()?;
    Some((distance(frame.at, *at), angle(frame.turn, *turn)))
}
