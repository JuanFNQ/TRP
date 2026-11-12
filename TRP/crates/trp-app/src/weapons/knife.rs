//! A knife's blade.
//! After `docs/spec/grenades-and-knife.md`, "The knife's hands": its box on the rig
//! (`KnifeCollider`'s own `BoxCollider`). The blade is swept on the simulation's own frames
//! (`controller::step`): while a stroke's blade is live, before the hands' clips move on, from the
//! blade where the frame before left it, the first person's squeeze off (`trp_sim::BladeSweep`),
//! along the look by the stroke's reach, through the stroke's world (`StrokeWorld`): the range's
//! boxes and ground as the physics engine has them, and every living dummy's hit colliders on
//! bodies of their own that the engine moves only where they are put, as a character's ride its
//! bones' kinematic bodies in the game.
//!
//! **What it meets** is the game's (`KnifeCollider.ManualUpdateClient`,
//! `Physics.BoxCastNonAlloc`, read in the engine itself): every touch in
//! the engine's own order, the still things' before the bodies', each
//! tree's nearer child first, the first on the blade's layers taken
//! (`HitCollider`, `HighPolyCollider`, `TransparentCollider`, `Water`: not
//! `Terrain`, which the range's ground is on). So a box in reach takes the
//! blow before a body, even behind it. Hit once a stroke, the stroke
//! glancing off (`KnifeHands::hit`); a dummy's collider takes a `Blow`
//! (`dummy::blow_dummies`), the world's boxes nothing but the glance and,
//! on the deflecting layer, the stroke's extra stamina.

use trp_physics::{Body, Collider, Met, World};

/// The blade's box on a melee weapon's rig: its object (by its place in
/// the rig's hierarchy, which is the rig's bone) and the box's centre and
/// size there, in the game's axes.
#[derive(Debug, Clone)]
pub struct Blade {
    pub bone: usize,
    pub centre: [f32; 3],
    pub size: [f32; 3],
}

impl Blade {
    /// The object on a rig with the knife's collider script, and its box.
    pub fn find(hierarchy: &trp_data::unity::extract::Hierarchy) -> Option<Self> {
        hierarchy.nodes.iter().enumerate().find(|(_, node)| node.scripts.iter().any(|script| script == "KnifeCollider")).and_then(|(bone, node)| {
            node.colliders.iter().find_map(|collider| match collider.shape {
                trp_data::unity::extract::ColliderShape::Box { centre, size } => Some(Self { bone, centre, size }),
                _ => None,
            })
        })
    }
}

/// The layers a blade goes by (`Player.BaseKnifeController`'s
/// `_knifeMask`, `_deflectLayer`; `Player.SetupBodyPartCollider`): those it
/// meets, the one it glances off at a cost (`HighPolyCollider`), and the
/// one a living character's hit colliders are put on (`HitCollider`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layers {
    pub mask: u32,
    pub deflect: u16,
    pub hit: u16,
}

impl Layers {
    pub const MET: [&'static str; 4] = ["HitCollider", "HighPolyCollider", "TransparentCollider", "Water"];

    /// By the install's names (`probe layers`: 16, 12, 30 and 4).
    pub fn of(physics: &trp_data::setup::engine::PhysicsSettings) -> Self {
        let layer = |name: &str, otherwise: u16| physics.layer(name).unwrap_or(otherwise);
        let mask = Self::MET.iter().zip([16, 12, 30, 4]).fold(0u32, |mask, (name, otherwise)| mask | 1 << layer(name, otherwise));
        Self { mask, deflect: layer("HighPolyCollider", 12), hit: layer("HitCollider", 16) }
    }

    pub fn meets(&self, layer: u16) -> bool {
        layer < 32 && self.mask >> layer & 1 == 1
    }
}

/// A dummy's hit collider in the stroke's world: its dummy, its place in
/// that dummy's list, and its object's place (the game's axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub which: usize,
    pub index: usize,
    pub object: [f32; 3],
}

/// What a blade met: a still thing of the range (by the order it was put
/// in, on its layer), or a dummy's collider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Struck {
    World { still: Option<usize>, layer: u16 },
    Dummy(Target),
}

/// A blade's meeting: how far along its sweep, the point (`KnifeRaycastHit`
/// as `ProcessOverlap` takes it) and the normal, and what it met.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meeting {
    pub distance: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub struck: Struck,
}

/// The world a stroke is swept through: the range's still things, made
/// once a stroke, and the living dummies' hit colliders, put in again on
/// every frame the blade is live.
pub struct StrokeWorld {
    world: World,
    layers: Layers,
    /// Each still thing's object's place, in the order put in.
    stills: Vec<[f32; 3]>,
    bodies: Vec<(Body, Target)>,
}

impl StrokeWorld {
    /// The range as the engine has it (`loose::range_world`: the ground
    /// first, on `Terrain`, then each box on `HighPolyCollider`).
    pub fn of_range(physics: &trp_data::setup::engine::PhysicsSettings, range: &crate::world::range::Range) -> Self {
        let mut stills = vec![[0.0; 3]];
        stills.extend(range.boxes().into_iter().map(|(centre, ..)| [centre.x, centre.y, -centre.z]));
        Self::of(crate::world::loose::range_world(physics, range), Layers::of(physics), stills)
    }

    /// A world of still things already made, and their objects' places.
    pub fn of(mut world: World, layers: Layers, stills: Vec<[f32; 3]>) -> Self {
        world.build_query_trees();
        Self { world, layers, stills, bodies: Vec::new() }
    }

    /// The living dummies' hit colliders as they stand now (`dummy::
    /// Dummies::blade_targets`), each on a kinematic body of its own on the
    /// hit colliders' layer, in place of the last frame's; the query trees
    /// built again.
    pub fn set_targets(&mut self, targets: Vec<(Target, [f32; 3], [f32; 4], Collider)>) {
        for (body, _) in self.bodies.drain(..) {
            self.world.remove(body);
        }
        for (target, at, turn, collider) in targets {
            let body = self.world.add_kinematic(at, turn, &[collider], self.layers.hit);
            self.bodies.push((body, target));
        }
        self.world.build_query_trees();
    }

    /// What the blade's sweep meets (`ManualUpdateClient`): the engine's
    /// first touch on the blade's layers (`TOUCHES_KEPT` places, read in
    /// order; none here the character's own, whose colliders are not in
    /// the world: `PROVISIONAL`, the game's own can fill both places, and
    /// the moving bodies' tree is this small world's, not a level's). Where it began inside a shape, Unity gives the point as
    /// nought, and a point that near the world's origin (its square under
    /// 0.1) is taken as the met collider's object's place (`ProcessOverlap`).
    pub fn meet(&self, sweep: &trp_sim::BladeSweep) -> Option<Meeting> {
        let touches = self.world.sweep_box_touches(sweep.from, sweep.half, sweep.turn, sweep.way, sweep.reach);
        let touch = touches.iter().filter(|touch| self.layers.meets(touch.layer)).take(TOUCHES_KEPT).next()?;
        let (struck, object) = match touch.met {
            Some(Met::Body(body)) => {
                let target = self.bodies.iter().find(|(own, _)| *own == body).map(|(_, target)| *target)?;
                (Struck::Dummy(target), target.object)
            }
            Some(Met::Still(still)) => (Struck::World { still: Some(still), layer: touch.layer }, self.stills.get(still).copied().unwrap_or([0.0; 3])),
            None => (Struck::World { still: None, layer: touch.layer }, [0.0; 3]),
        };
        let given = if touch.distance <= 0.0 { [0.0; 3] } else { touch.point };
        let point = if given.iter().map(|x| x * x).sum::<f32>() < 0.1 { object } else { given };
        Some(Meeting { distance: touch.distance, point, normal: touch.normal, struck })
    }

    /// Whether the deflecting layer was met (`ProcessOverlap`: the knife's
    /// `DeflectionConsumption` spent too).
    pub fn deflects(&self, struck: &Struck) -> bool {
        matches!(struck, Struck::World { layer, .. } if *layer == self.layers.deflect)
    }
}

/// The stroke's world as the controller keeps it: a copy of the controller
/// (its start, kept for beginning again) begins with none, and makes one
/// at its next stroke.
#[derive(Default)]
pub struct StrokeSlot(pub Option<StrokeWorld>);

impl Clone for StrokeSlot {
    fn clone(&self) -> Self {
        Self(None)
    }
}

/// How many of the cast's touches the knife reads (`KnifeCollider.hits`,
/// two places): the first on its layers that is not the character's own
/// is taken. With none of the character's own in the world, the first.
pub const TOUCHES_KEPT: usize = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use trp_physics::{Material, Settings, Shape};

    fn layers() -> Layers {
        Layers { mask: 1 << 16 | 1 << 12 | 1 << 30 | 1 << 4, deflect: 12, hit: 16 }
    }

    fn collider(shape: Shape) -> Collider {
        Collider { shape, at: [0.0; 3], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() }
    }

    /// A stroke's world of a wall (a box on `HighPolyCollider`) and the
    /// ground (`Terrain`), and a dummy's chest (a sphere) before the wall.
    fn world(wall_at: f32) -> StrokeWorld {
        let mut world = World::new(Settings::game());
        world.add_ground(0.0, Material::default(), 11);
        world.add_static([0.0, 1.3, wall_at], [0.0, 0.0, 0.0, 1.0], &[collider(Shape::Box { half: [1.0, 1.0, 0.1] })], 12);
        let mut stroke = StrokeWorld::of(world, layers(), vec![[0.0; 3], [0.0, 1.3, wall_at]]);
        let chest = Target { which: 0, index: 3, object: [0.0, 1.3, 0.5] };
        stroke.set_targets(vec![(chest, [0.0, 1.3, 0.5], [0.0, 0.0, 0.0, 1.0], collider(Shape::Sphere { radius: 0.15 }))]);
        stroke
    }

    fn sweep(from: [f32; 3], way: [f32; 3], reach: f32) -> trp_sim::BladeSweep {
        trp_sim::BladeSweep { from, half: [0.0004, 0.054, 0.0635], turn: [0.0, 0.0, 0.0, 1.0], way, reach }
    }

    /// What the blade meets is the game's order (`BoxCastNonAlloc`: the
    /// still things before the bodies, the first on the blade's layers):
    /// with the wall out of its reach the blade meets the chest and goes no
    /// further; with the wall in reach behind the chest, the wall takes the
    /// blow, at a cost (its layer deflects). The ground, on `Terrain`, is
    /// not the blade's, so a stroke into it meets nothing; one begun inside
    /// the chest is met at nought, at its object's place.
    #[test]
    fn the_blade_meets_still_things_before_a_body_on_its_own_layers() {
        // The chest's front 0.35 m along; the wall 0.9 m along, past a 0.6 m
        // reach.
        let alone = world(1.0);
        let met = alone.meet(&sweep([0.0, 1.3, 0.0], [0.0, 0.0, 1.0], 0.6)).expect("the chest");
        assert_eq!(met.struck, Struck::Dummy(Target { which: 0, index: 3, object: [0.0, 1.3, 0.5] }));
        assert!((met.distance - (0.35 - 0.0635)).abs() < 1e-3, "{met:?}");
        assert!(!alone.deflects(&met.struck));
        // The wall 0.6 m along, within the reach: met first, though further.
        let walled = world(0.7);
        let met = walled.meet(&sweep([0.0, 1.3, 0.0], [0.0, 0.0, 1.0], 0.6)).expect("the wall");
        assert_eq!(met.struck, Struck::World { still: Some(1), layer: 12 });
        assert!(met.distance > 0.5 && walled.deflects(&met.struck), "{met:?}");
        // Down into the ground: not the blade's layer.
        assert_eq!(alone.meet(&sweep([2.0, 0.3, 0.0], [0.0, -1.0, 0.0], 0.6)), None);
        // Begun inside the chest: nought along, at the chest's object.
        let inside = alone.meet(&sweep([0.0, 1.3, 0.45], [0.0, 0.0, 1.0], 0.1)).expect("the chest from inside");
        assert_eq!((inside.distance, inside.point), (0.0, [0.0, 1.3, 0.5]));
        // The targets put in again, none: the chest is gone.
        let mut gone = world(1.0);
        gone.set_targets(Vec::new());
        assert_eq!(gone.meet(&sweep([0.0, 1.3, 0.0], [0.0, 0.0, 1.0], 0.6)), None);
    }
}

/// A blow on a dummy, for it to take (`dummy::blow_dummies`): which dummy,
/// which of its colliders, the stroke, the blow's damage (the exhausted
/// hands' cut and Strength's buff in it) and penetration, the knife's
/// stamina burn rate, the swing's way and the point (the game's axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blow {
    pub target: Target,
    pub stroke: trp_sim::Stroke,
    pub damage: f32,
    pub penetration: f32,
    pub burn_rate: f32,
    pub direction: [f32; 3],
    pub point: [f32; 3],
}
