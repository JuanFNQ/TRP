//! Emit parameters, particles, drawn output, the world and placement.

use super::*;

/// What a particle is born with (`EmitParams`): the caller's values; the
/// system's start values for none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmitParams {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub size: f32,
    pub lifetime: f32,
    /// Its colour as bytes (`Color32`), red, green, blue, alpha.
    pub color: [u8; 4],
    /// Its turn in degrees.
    pub rotation: f32,
    /// Its own seed, which its draws for the modules come from.
    pub seed: u32,
}

/// A particle alive, in the system's space (the world's, or its own).
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    /// Its size and colour as born.
    pub size: f32,
    pub color: [f32; 4],
    /// Its turn, degrees.
    pub rotation: f32,
    pub lifetime: f32,
    pub age: f32,
    pub seed: u32,
    /// Its trail's points (`TrailModule`), oldest first, each with its age
    /// when dropped, in the system's space.
    pub trail: Vec<([f32; 3], f32)>,
}

impl Particle {
    /// Its share of life, from 0 to 1.
    pub fn share(&self) -> f32 {
        if self.lifetime > f32::EPSILON { (self.age / self.lifetime).clamp(0.0, 1.0) } else { 1.0 }
    }

    /// Turning the other way (`randomizeRotationDirection`).
    pub(super) fn flipped(&self, data: &ParticleSystemData) -> bool {
        engine_draw(self.seed, FLIP_ID, 0) < data.flip_rotation
    }
}

/// What a particle is drawn as, in the world: its place, its velocity, its
/// size, its turn in degrees, its colour, its picture's place in a sheet
/// (`[u0, v0, u1, v1]`, the whole picture without one), and the seconds of
/// life it has left (what a renderer sorting by age goes by).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drawn {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub size: f32,
    pub rotation: f32,
    pub color: [f32; 4],
    pub sheet: [f32; 4],
    pub remaining: f32,
}

/// The world a system's particles meet (the collision module's world): the
/// first surface a sphere meets going from `origin` along the unit way
/// `dir` within `distance`, its distance along the way (below nought where
/// the sphere starts inside a surface, by how far) and its normal. The app
/// asks the game's physics engine (`trp_physics::World::sweep_sphere`).
pub trait World {
    fn sweep_sphere(&self, origin: [f32; 3], dir: [f32; 3], distance: f32, radius: f32) -> Option<(f32, [f32; 3])>;

    /// The same among the still shapes alone (the sweep of medium and low
    /// quality); the same as `sweep_sphere` for a world with nothing moving.
    fn sweep_sphere_still(&self, origin: [f32; 3], dir: [f32; 3], distance: f32, radius: f32) -> Option<(f32, [f32; 3])> {
        self.sweep_sphere(origin, dir, distance, radius)
    }
}

/// A particle's light this frame (the lights module): where it is, its
/// colour (the light's, tinted by the particle's where asked), its range
/// and its intensity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleLight {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub range: f32,
    pub intensity: f32,
}

/// Where a system is in the world: its object's place, turn and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Placement {
    fn default() -> Self {
        Self { position: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] }
    }
}

impl Placement {
    pub(super) fn point(&self, local: [f32; 3]) -> [f32; 3] {
        let scaled = std::array::from_fn(|k| local[k] * self.scale[k]);
        add(self.position, rotate(self.rotation, scaled))
    }

    pub(super) fn way(&self, local: [f32; 3]) -> [f32; 3] {
        rotate(self.rotation, local)
    }

    pub(super) fn way_back(&self, world: [f32; 3]) -> [f32; 3] {
        rotate(conjugate(self.rotation), world)
    }

    pub(super) fn point_back(&self, world: [f32; 3]) -> [f32; 3] {
        let local = self.way_back(sub(world, self.position));
        std::array::from_fn(|k| if self.scale[k].abs() > 1e-9 { local[k] / self.scale[k] } else { local[k] })
    }
}

/// The draws of the modules whose constants in the engine are not read (the
/// gravity modifier, a force drawn each frame, the texture sheet, the
/// collision, the lights and the trails), by the particle's seed.
/// `PROVISIONAL`: a hash of the seed and the module's number, from 0 to 1,
/// where the engine's are `engine_draw` with each module's constant.
pub(super) fn draw(seed: u32, module: u32) -> f32 {
    let mut n = seed ^ module.wrapping_mul(0x9e37_79b9);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^= n >> 16;
    (n >> 8) as f32 / (1u32 << 24) as f32
}

/// The engine's own draw for one of a particle's modules: Unity's generator
/// seeded with the particle's seed plus the module's constant (`RandN::
/// RandN`, each next word the last times 1812433253 plus one, as
/// `Random.InitState`), its `n`th number between 0 and 1 (the low 23 bits
/// times the engine's constant, as `Random.value`). Read with Ghidra from
/// each module's update (`VelocityModule::Update`, `RotationModule::
/// UpdateProcedural`, `SizeModule::Update`, `ColorModule::Update`,
/// `ForceModule::Update`, `ClampVelocityModule::Update`, `RotationBySpeed
/// Module::Update`): the constants below.
pub(super) fn engine_draw(seed: u32, module: u32, n: usize) -> f32 {
    let mut random = crate::base::unity_random::UnityRandom::new(seed.wrapping_add(module) as i32);
    let mut value = random.value();
    for _ in 0..n {
        value = random.value();
    }
    value
}
