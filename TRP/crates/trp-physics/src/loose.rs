//! Things let fall and thrown (`docs/spec/physics.md`, "Loose things"):
//! where the game sets one going and how (`GameWorld.ThrowItem`, the quick
//! reload's `DropMod`), the rigid body it gives it
//! (`LootItem.CreateLootWithRigidbody`), and when it takes the body away
//! again (`LootItem.IsRigidbodyDone`). The host makes the colliders from the
//! thing's model and steps the world.
//!
//! Everything in the game's axes, angles in degrees as the game keeps a
//! player's: yaw to the right, pitch downward.

use trp_data::unity::assets::ObjRef;
use trp_data::unity::extract::{ColliderShape, Hierarchy, NodeCollider};

use crate::*;

/// How a loose thing sets off: where its object is put and how it is
/// turned, and its velocity and spin (radians a second, about the world's
/// axes) as the game gives them to its rigid body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Start {
    pub at: V3,
    pub turn: Q4,
    pub velocity: V3,
    pub spin: V3,
}

/// The game's dice for a throw (`UnityEngine.Random`): a number in a range,
/// either kind. Its sequence is its own (the game's seed is not known).
#[derive(Debug, Clone)]
pub struct Dice(u64);

impl Dice {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        // xorshift64*.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// `Random.Range(float, float)`: from `low` to `high`.
    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * ((self.next() >> 40) as f32 / (1u64 << 24) as f32)
    }

    /// `Random.Range(int, int)`: from `low` up to but not `high`.
    pub fn whole(&mut self, low: i32, high: i32) -> i32 {
        low + (self.next() % (high - low).max(1) as u64) as i32
    }
}

/// Unity's `Mathf.Sign`: 1 for nought.
fn sign(x: f32) -> f32 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// The turn `Quaternion.Euler(x, y, z)` gives (degrees; z, then x, then y).
pub fn euler(x: f32, y: f32, z: f32) -> Q4 {
    let about = |axis: V3, degrees: f32| {
        let (s, c) = (degrees.to_radians() / 2.0).sin_cos();
        [axis[0] * s, axis[1] * s, axis[2] * s, c]
    };
    mul(mul(about([0.0, 1.0, 0.0], y), about([1.0, 0.0, 0.0], x)), about([0.0, 0.0, 1.0], z))
}

/// The character as a throw needs it: where it looks, where its capsule's
/// axis is at 0.65 of its height (`PlayerColliderPointOnCenterAxis(0.65)`:
/// the capsule's centre across, 0.65 of its height over the feet), its
/// velocity, the frame's time, and how its weapon's root is turned
/// (`PlayerBones.WeaponRoot`).
#[derive(Debug, Clone, Copy)]
pub struct Thrower {
    pub yaw: f32,
    pub pitch: f32,
    pub chest: V3,
    pub velocity: V3,
    pub dt: f32,
    pub weapon_root: Q4,
}

/// A thing put down from the inventory, or a backpack dropped
/// (`GameWorld.ThrowItem` with no direction given): thrown from 0.65 of
/// the capsule's height, where the character will be a frame on, at 2 m/s
/// along the look turned 45 degrees up (the pitch held between straight
/// up and 45 down), with half the character's own velocity; turned as the
/// weapon's root and then 90 degrees about its x, and spun at random up to
/// 3 about x and y and 2 one way or the other about z (the other way one
/// time in three: `Mathf.Sign` of a whole number from -1 to 1).
pub fn thrown(by: &Thrower, down: bool, dice: &mut Dice) -> Start {
    let along = if down { [0.0, -1.0, 0.0] } else { rotate(euler(by.pitch.clamp(-90.0, 45.0), by.yaw, 0.0), [0.0, 1.0, 1.0]) };
    let at = [by.chest[0] + by.velocity[0] * by.dt, by.chest[1] + by.velocity[1] * by.dt, by.chest[2] + by.velocity[2] * by.dt];
    let turn = mul(by.weapon_root, euler(90.0, 0.0, 0.0));
    let spin = [dice.range(-3.0, 3.0), dice.range(-3.0, 3.0), 2.0 * sign(dice.whole(-1, 2) as f32)];
    let velocity = [along[0] * 2.0 + by.velocity[0] / 2.0, along[1] * 2.0 + by.velocity[1] / 2.0, along[2] * 2.0 + by.velocity[2] / 2.0];
    Start { at, turn, velocity, spin }
}

/// The magazine a quick reload lets go (`DropMod` at `OnMagPuttedToRig`):
/// where the weapon's magazine is at that moment, falling at 1 cm/s and
/// nothing of the character's velocity, spun at random up to 3 about each
/// axis.
pub fn let_fall(at: V3, turn: Q4, dice: &mut Dice) -> Start {
    let spin = [dice.range(-3.0, 3.0), dice.range(-3.0, 3.0), dice.range(-3.0, 3.0)];
    Start { at, turn, velocity: [0.0, -0.01, 0.0], spin }
}

/// The volume of the box round a loose thing's colliders under which the
/// game sweeps it (`LootVolumeForHighQuallityPhysicsClient`, 999 m³ in the
/// install: every thing there is).
pub const SWEPT_UNDER: f32 = 999.0;

/// A loose thing's body as the game makes it: its mass its weight with all
/// in it (`TotalWeight`), a new `Rigidbody`'s drags (none, and 0.05 for the
/// spin), the engine's own depenetration, swept if the box round its
/// colliders is no bigger than `swept_under`, on the given layer (the
/// game's `Loot`). Made, it is put at `start` with its centre of mass
/// where its object's origin is (`SetShift`: the game moves its model's
/// parts back by the centre of mass and sets the object where the thing
/// was, so the thing as drawn and touched sits that much off), and set
/// going. Returns the body and the shift, in the thing's axes.
pub fn add(world: &mut World, colliders: Vec<Collider>, mass: f32, start: &Start, swept_under: f32, layer: u16) -> (Body, V3) {
    let box_volume = bounds(&colliders).map_or(0.0, |(low, high)| (high[0] - low[0]) * (high[1] - low[1]) * (high[2] - low[2]));
    let desc = BodyDesc {
        at: start.at,
        turn: start.turn,
        colliders,
        // Unity takes no mass under 1e-7.
        mass: mass.max(1e-7),
        drag: 0.0,
        angular_drag: 0.05,
        velocity: start.velocity,
        spin: start.spin,
        max_depenetration: world.settings().max_depenetration,
        continuous: box_volume <= swept_under,
        layer,
    };
    let body = world.add_body(&desc);
    let (shift, _) = world.mass_properties(body);
    let moved = rotate(start.turn, shift);
    world.set_pose(body, [start.at[0] - moved[0], start.at[1] - moved[1], start.at[2] - moved[2]], start.turn);
    (body, shift)
}

/// The box round colliders, in their body's axes (`SetupBoundingCollider`
/// without its least size, which only a trigger has), if any has one.
pub fn bounds(colliders: &[Collider]) -> Option<(V3, V3)> {
    let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
    let mut take = |p: V3| {
        for axis in 0..3 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    };
    for c in colliders {
        let corners: Vec<V3> = match &c.shape {
            Shape::Box { half } => (0..8).map(|i| [if i & 1 == 0 { -half[0] } else { half[0] }, if i & 2 == 0 { -half[1] } else { half[1] }, if i & 4 == 0 { -half[2] } else { half[2] }]).collect(),
            Shape::Sphere { radius } => (0..8).map(|i| [if i & 1 == 0 { -radius } else { *radius }, if i & 2 == 0 { -radius } else { *radius }, if i & 4 == 0 { -radius } else { *radius }]).collect(),
            Shape::Capsule { radius, height, direction } => {
                let mut half = [*radius; 3];
                half[(*direction).min(2) as usize] = height.max(2.0 * radius) / 2.0;
                (0..8).map(|i| [if i & 1 == 0 { -half[0] } else { half[0] }, if i & 2 == 0 { -half[1] } else { half[1] }, if i & 4 == 0 { -half[2] } else { half[2] }]).collect()
            }
            Shape::Convex { points, scale } => points.iter().map(|p| [p[0] * scale[0], p[1] * scale[1], p[2] * scale[2]]).collect(),
        };
        for corner in corners {
            let p = rotate(c.turn, corner);
            take([p[0] + c.at[0], p[1] + c.at[1], p[2] + c.at[2]]);
        }
    }
    (low[0] <= high[0]).then_some((low, high))
}

/// The game's watch on a loose thing's body (`LootItem.PhysicsStopWorker`):
/// every frame, the body is done once asleep; or, swept and slower than
/// 0.09 m/s (a squared speed of 0.008), once it has moved 7.5 s; or after
/// 15 s whatever it does. The game then takes the body away and the thing
/// lies where it is.
#[derive(Debug, Clone, Copy, Default)]
pub struct Watch {
    pub time: f32,
}

impl Watch {
    pub const MOST: f32 = 15.0;
    pub const MOST_SWEPT_SLOW: f32 = 7.5;
    pub const SLOW: f32 = 0.008;

    /// A frame of `dt` after the world was stepped: whether the body is
    /// done (checked before the frame's time is counted, as the game's).
    pub fn frame(&mut self, world: &World, body: Body, swept: bool, dt: f32) -> bool {
        let (velocity, _) = world.velocity(body);
        let squared = velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2];
        let done = world.asleep(body) || if swept && squared <= Self::SLOW { self.time >= Self::MOST_SWEPT_SLOW } else { self.time >= Self::MOST };
        self.time += dt;
        done
    }
}

/// A collider of a model's as the engine takes it (Unity's sizes times its
/// object's scale: a box's size and centre, a sphere's radius by the
/// largest, a capsule's height by its own axis's and its radius by the
/// largest across, a mesh's points by all three), its object standing at
/// `at`, turned `turn` and scaled `scale` in the body's axes. A convex
/// mesh's points come from `points`; a mesh that is not convex, or whose
/// points cannot be had, gives none (a moving body's never is: Unity will
/// not move one).
pub fn collider(c: &NodeCollider, at: V3, turn: Q4, scale: V3, points: &mut dyn FnMut(ObjRef) -> Option<std::sync::Arc<[V3]>>) -> Option<Collider> {
    let s = [scale[0].abs(), scale[1].abs(), scale[2].abs()];
    let (shape, centre) = match c.shape {
        ColliderShape::Box { centre, size } => (Shape::Box { half: [size[0] * s[0] / 2.0, size[1] * s[1] / 2.0, size[2] * s[2] / 2.0] }, centre),
        ColliderShape::Sphere { centre, radius } => (Shape::Sphere { radius: radius * s[0].max(s[1]).max(s[2]) }, centre),
        ColliderShape::Capsule { centre, radius, height, direction } => {
            let along = direction.min(2) as usize;
            let across = (0..3).filter(|axis| *axis != along).map(|axis| s[axis]).fold(0.0f32, f32::max);
            (Shape::Capsule { radius: radius * across, height: height * s[along], direction: along as u8 }, centre)
        }
        ColliderShape::Mesh { convex: true, mesh: Some(mesh) } => (Shape::Convex { points: points(mesh)?, scale }, [0.0; 3]),
        ColliderShape::Mesh { .. } => return None,
    };
    let centre = rotate(turn, [centre[0] * s[0], centre[1] * s[1], centre[2] * s[2]]);
    let material = c.material.map_or_else(Material::default, |m| {
        let combine = |mode: u32| match mode {
            1 => Combine::Minimum,
            2 => Combine::Multiply,
            3 => Combine::Maximum,
            _ => Combine::Average,
        };
        Material { dynamic_friction: m.dynamic_friction, static_friction: m.static_friction, bounciness: m.bounciness, friction_combine: combine(m.friction_combine), bounce_combine: combine(m.bounce_combine) }
    });
    Some(Collider { shape, at: [at[0] + centre[0], at[1] + centre[1], at[2] + centre[2]], turn, material })
}

/// The colliders of a thing's model as it lies loose
/// (`ObjectsFactory.CreateLootPrefab`, which switches every collider on but
/// a flare's), in its first object's axes, that object's own place, turn
/// and scale left out (the game sets them): those of objects that are on,
/// triggers left out (they push nothing; that Unity leaves them out of the
/// centre of mass too is `PROVISIONAL`).
pub fn colliders(hierarchy: &Hierarchy, points: &mut dyn FnMut(ObjRef) -> Option<std::sync::Arc<[V3]>>) -> Vec<Collider> {
    let mut placed: Vec<(V3, Q4, V3, bool)> = Vec::with_capacity(hierarchy.nodes.len());
    let mut out = Vec::new();
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let here = match node.parent.filter(|_| index > 0) {
            None => ([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3], node.active),
            Some(parent) => {
                let (at, turn, scale, on) = placed[parent];
                let offset = rotate(turn, [node.translation[0] * scale[0], node.translation[1] * scale[1], node.translation[2] * scale[2]]);
                let own = [scale[0] * node.scale[0], scale[1] * node.scale[1], scale[2] * node.scale[2]];
                ([at[0] + offset[0], at[1] + offset[1], at[2] + offset[2]], mul(turn, node.rotation), own, on && node.active)
            }
        };
        placed.push(here);
        let (at, turn, scale, on) = here;
        if !on || node.scripts.iter().any(|script| script == "SingleFlareController") {
            continue;
        }
        for c in node.colliders.iter().filter(|c| !c.trigger) {
            out.extend(collider(c, at, turn, scale, points));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_is_unitys() {
        // Pitched 45 down, (0, 1, 1) lies level ahead; yawed 90, ahead is +x.
        let v = rotate(euler(45.0, 0.0, 0.0), [0.0, 1.0, 1.0]);
        assert!(v[1].abs() < 1e-5 && (v[2] - 2f32.sqrt()).abs() < 1e-5, "{v:?}");
        let v = rotate(euler(0.0, 90.0, 0.0), [0.0, 0.0, 1.0]);
        assert!((v[0] - 1.0).abs() < 1e-5 && v[2].abs() < 1e-5, "{v:?}");
    }

    #[test]
    fn a_thing_thrown_falls_ahead_and_rests() {
        let mut world = World::new(Settings::game());
        world.add_ground(0.0, Material::default(), 11);
        let mut dice = Dice::new(7);
        let by = Thrower { yaw: 0.0, pitch: 0.0, chest: [0.0, 1.04, 0.0], velocity: [0.0; 3], dt: 1.0 / 60.0, weapon_root: [0.0, 0.0, 0.0, 1.0] };
        let start = thrown(&by, false, &mut dice);
        assert_eq!(start.velocity, [0.0, 2.0, 2.0]);
        let collider = Collider { shape: Shape::Box { half: [0.05, 0.1, 0.03] }, at: [0.0, -0.05, 0.0], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() };
        let (body, shift) = add(&mut world, vec![collider], 0.5, &start, SWEPT_UNDER, 15);
        assert!((shift[1] + 0.05).abs() < 1e-5, "{shift:?}");
        let mut watch = Watch::default();
        let mut frames = 0;
        while !watch.frame(&world, body, true, 1.0 / 60.0) {
            world.step(1.0 / 60.0);
            frames += 1;
            assert!(frames < 60 * 16, "never done");
        }
        let (at, _) = world.pose(body);
        assert!(at[2] > 0.5 && at[1] < 0.2, "{at:?} after {frames} frames");
    }
}
