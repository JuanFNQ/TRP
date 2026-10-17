//! A corpse as the game makes one (`Corpse.CreateCorpse`, `CorpseRagdoll`;
//! health spec, "Death"; `docs/spec/physics.md`): the player prefab's
//! bodies, colliders and joints (`trp_data::ragdoll`) on the bones where
//! they stand, all on the `Deadbody` layer, each body set going with the
//! character's velocity held to 2 m/s, pushed out of what it is in at most
//! `CorpseMaxDepenetrationVelocity`; each joint the game's character joint
//! with its mass scale (the connected body's mass over its own), its
//! projection on and its preprocessing off, its limits counted from the
//! skeleton's rest (as the recordings show). Measured against four
//! recorded deaths by `bin/corpse`.

use std::collections::HashMap;

use trp_data::ragdoll::{ColliderData, RagdollData, Shape as Rshape};

use crate::{inverse, mul, rotate, Body, BodyDesc, CharacterJointDesc, Collider, Material, Shape, World, Q4, V3};

/// The game's: a corpse's bodies start no faster than this
/// (`CorpseRagdoll.Start`; straight up, a metre a second).
pub const MOST_SPEED: f32 = 2.0;
/// Unity's projection of a character joint, which `CorpseRagdoll` switches
/// on: its default distance and angle (the prefab's spawners leave them
/// at Unity's defaults, `PROVISIONAL` until recorder 0.9.0's set-up is read).
pub const PROJECTION: [f32; 2] = [0.1, 180.0];

/// A collider of the prefab's in its body's axes.
pub fn collider(c: &ColliderData) -> Collider {
    let (at, turn) = c.local.unwrap_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0]));
    let (shape, centre) = match c.shape {
        Rshape::Box { centre, size } => (Shape::Box { half: [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0] }, centre),
        Rshape::Sphere { centre, radius } => (Shape::Sphere { radius }, centre),
        Rshape::Capsule { centre, radius, height, direction } => (Shape::Capsule { radius, height, direction: direction as u8 }, centre),
    };
    let moved = rotate(turn, centre);
    Collider { shape, at: [at[0] + moved[0], at[1] + moved[1], at[2] + moved[2]], turn, material: Material::default() }
}

/// The velocity a corpse's bodies start with.
pub fn start_velocity(v: V3) -> V3 {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n <= 1e-9 {
        return [0.0; 3];
    }
    let (x, y, z) = (v[0] / n, v[1] / n, v[2] / n);
    if x.abs() < 1e-6 && z.abs() < 1e-6 && y > 0.0 {
        return [0.0, 1.0, 0.0];
    }
    if n > MOST_SPEED { [x * MOST_SPEED, y * MOST_SPEED, z * MOST_SPEED] } else { v }
}

/// The corpse's bodies and joints in a world, the bodies where `poses`
/// has their bones (in the game's axes); a bone it lacks has no body. The
/// bodies by bone.
pub fn build(world: &mut World, data: &RagdollData, poses: &HashMap<String, (V3, Q4)>, velocity: V3, depenetration: f32, layer: u16) -> HashMap<String, Body> {
    let speed = start_velocity(velocity);
    let mut bodies: HashMap<String, Body> = HashMap::new();
    let mut masses: HashMap<&str, f32> = HashMap::new();
    for b in &data.bodies {
        let Some((at, turn)) = poses.get(&b.bone) else { continue };
        let body = world.add_body(&BodyDesc {
            at: *at,
            turn: *turn,
            colliders: b.colliders.iter().map(collider).collect(),
            mass: b.mass,
            drag: b.drag,
            angular_drag: b.angular_drag,
            velocity: speed,
            spin: [0.0; 3],
            max_depenetration: depenetration,
            continuous: false,
            layer,
        });
        bodies.insert(b.bone.clone(), body);
        masses.insert(b.bone.as_str(), b.mass);
    }
    let rests: HashMap<&str, (V3, Q4)> = data.bodies.iter().map(|b| (b.bone.as_str(), b.rest)).collect();
    for j in &data.joints {
        let (Some(own), Some(connected)) = (bodies.get(&j.bone), bodies.get(&j.connected)) else { continue };
        let ((own_at, own_turn), (con_at, con_turn)) = (rests[j.bone.as_str()], rests[j.connected.as_str()]);
        let back = inverse(con_turn);
        let rest = (rotate(back, [own_at[0] - con_at[0], own_at[1] - con_at[1], own_at[2] - con_at[2]]), mul(back, own_turn));
        world.character_joint(&CharacterJointDesc {
            own: *own,
            connected: *connected,
            anchor: j.anchor,
            axis: j.axis,
            swing_axis: j.swing_axis,
            low_twist: j.low_twist.limit,
            high_twist: j.high_twist.limit,
            swing1: j.swing1.limit,
            swing2: j.swing2.limit,
            bounciness: [j.low_twist.bounciness, j.high_twist.bounciness, j.swing1.bounciness, j.swing2.bounciness],
            mass_scale: masses[j.connected.as_str()] / masses[j.bone.as_str()],
            connected_mass_scale: 1.0,
            projection: Some(PROJECTION),
            preprocessing: false,
            collision: j.enable_collision,
            rest: Some(rest),
        });
    }
    bodies
}

/// The game's own step of its physics (`PhysicsExtensions.Simulation.
/// SmoothSimulate`): once a drawn frame, by the average of the last 600
/// frames' times (a ring the game has kept full long before a raid; here
/// the average of those there are), at most `MaxSmoothPhysicsDeltaTime`.
#[derive(Debug, Clone)]
pub struct Stepper {
    ring: Vec<f32>,
    next: usize,
    most: f32,
}

impl Stepper {
    pub const SAMPLES: usize = 600;

    pub fn new(most: f32) -> Self {
        Self { ring: Vec::with_capacity(Self::SAMPLES), next: 0, most }
    }

    /// A drawn frame's time, and what the physics steps by on it.
    pub fn frame(&mut self, dt: f32) -> f32 {
        if self.ring.len() < Self::SAMPLES {
            self.ring.push(dt);
        } else {
            self.ring[self.next] = dt;
            self.next = (self.next + 1) % Self::SAMPLES;
        }
        let average = self.ring.iter().sum::<f32>() / self.ring.len() as f32;
        average.min(self.most)
    }
}
