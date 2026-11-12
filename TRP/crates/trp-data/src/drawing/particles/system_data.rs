//! A particle system's modules: shape, emission, sheet, forces, renderer, trails.

use super::*;

/// The speed's limit over life (`ClampVelocityModule`, one limit for all
/// axes): the limit, the share of the speed above it taken off, the drag
/// and what multiplies it.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitVelocity {
    pub magnitude: MinMaxCurve,
    pub dampen: f32,
    pub drag: MinMaxCurve,
    pub drag_by_size: bool,
    pub drag_by_velocity: bool,
}

/// Where a particle is born and which way it goes (`ShapeModule`), in the
/// system's own axes moved by the shape's own place, turn and size.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// `type`: 0 a sphere, 2 a hemisphere, 4 a cone's base, 5 a box, 10 a
    /// circle (`ParticleSystemShapeType`).
    pub kind: i64,
    pub radius: f32,
    /// The share of the radius, from the edge in, a particle may be born
    /// in (1 the whole, 0 the edge alone).
    pub radius_thickness: f32,
    /// A cone's angle, degrees; its length.
    pub angle: f32,
    pub length: f32,
    /// The arc, degrees.
    pub arc: f32,
    pub position: [f32; 3],
    /// Euler angles, degrees.
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub align_to_direction: bool,
    pub random_direction: f32,
    pub spherical_direction: f32,
    pub random_position: f32,
}

/// A burst (`ParticleSystemEmissionBurst`): at a time of the system's,
/// so many, so many times, so far apart, with a chance.
#[derive(Debug, Clone, PartialEq)]
pub struct Burst {
    pub time: f32,
    pub count: MinMaxCurve,
    pub cycles: i64,
    pub interval: f32,
    pub probability: f32,
}

/// How many are born (`EmissionModule`): a rate a second, a rate a metre
/// moved, and bursts.
#[derive(Debug, Clone, PartialEq)]
pub struct Emission {
    pub rate_over_time: MinMaxCurve,
    pub rate_over_distance: MinMaxCurve,
    pub bursts: Vec<Burst>,
}

/// A picture of tiles played over a particle's life (`UVModule`, a grid).
#[derive(Debug, Clone, PartialEq)]
pub struct TextureSheet {
    /// `mode`: 0 a grid.
    pub mode: i64,
    pub tiles: [u32; 2],
    /// `animationType`: 0 the whole sheet, 1 a single row.
    pub single_row: bool,
    /// `rowMode`: 0 a row by `rowIndex`, 1 a random row.
    pub random_row: bool,
    pub row_index: u32,
    /// The frame as a share of the sheet (or of the row), over life.
    pub frame_over_time: MinMaxCurve,
    pub start_frame: MinMaxCurve,
    pub cycles: f32,
    /// `timeMode`: 0 over life (the rest not read).
    pub time_mode: i64,
}

/// A velocity over life (`VelocityModule`, its straight part), metres a
/// second, in the world's axes or the system's: added to a particle's own
/// velocity each step and not kept.
#[derive(Debug, Clone, PartialEq)]
pub struct VelocityOverLifetime {
    pub x: MinMaxCurve,
    pub y: MinMaxCurve,
    pub z: MinMaxCurve,
    pub world_space: bool,
}

/// A force over life (`ForceModule`), metres a second a second, in the
/// world's axes or the system's.
#[derive(Debug, Clone, PartialEq)]
pub struct Force {
    pub x: MinMaxCurve,
    pub y: MinMaxCurve,
    pub z: MinMaxCurve,
    pub world_space: bool,
    pub randomize_per_frame: bool,
}

/// A turn by speed (`RotationBySpeedModule`, one axis): its angular speed
/// (radians a second) by where the particle's speed lies in `range`.
#[derive(Debug, Clone, PartialEq)]
pub struct RotationBySpeed {
    pub curve: MinMaxCurve,
    pub range: [f32; 2],
}

/// How a system's particles are drawn (`ParticleSystemRenderer`).
#[derive(Debug, Clone)]
pub struct ParticleRenderer {
    /// `m_RenderMode`: 0 a billboard, 1 stretched along its velocity, 4 a
    /// mesh, 5 none.
    pub render_mode: i64,
    /// A stretched billboard's length: its velocity's share
    /// (`m_VelocityScale`), its size's (`m_LengthScale`), the camera's
    /// velocity's (`m_CameraVelocityScale`).
    pub velocity_scale: f32,
    pub length_scale: f32,
    pub camera_velocity_scale: f32,
    /// Its place among the scene's transparent things (`m_SortingFudge`).
    pub sorting_fudge: f32,
    /// `m_RenderAlignment`: 0 the view.
    pub alignment: i64,
    /// `m_SortMode`: 0 none.
    pub sort_mode: i64,
    /// The least and most size on the screen, as shares of the view.
    pub min_size: f32,
    pub max_size: f32,
    pub material: ParticleMaterial,
    /// The trails' material (`m_Materials[1]`), where it has one.
    pub trail_material: Option<ParticleMaterial>,
    /// Whether the renderer is on (`m_Enabled`): one off draws nothing.
    pub enabled: bool,
    /// A mesh's particles' mesh (render mode 4; `m_Mesh`, the first of up
    /// to four: the others not read).
    pub mesh: Option<std::sync::Arc<MeshData>>,
    /// How a billboard's normals lean (`m_NormalDirection`): 1 straight to
    /// the camera, 0 out from the particle's middle.
    pub normal_direction: f32,
}

/// A particle system's material: its shader's name, its pictures, figures
/// and colours by their names.
#[derive(Debug, Clone)]
pub struct ParticleMaterial {
    pub name: String,
    pub shader: String,
    pub textures: Vec<(String, TextureData)>,
    pub floats: Vec<(String, f32)>,
    pub colors: Vec<(String, [f32; 4])>,
    /// Each picture's scale and offset (`m_Scale`, `m_Offset`: the
    /// shader's `_ST`).
    pub scales: Vec<(String, [f32; 4])>,
    /// Its keywords on (`m_ValidKeywords`).
    pub keywords: Vec<String>,
    /// Its render queue: its own (`m_CustomRenderQueue`), or else its
    /// shader's (`Queue` tag: `Transparent` 3000, `Geometry` 2000 and so
    /// on, with any `+n`); the engine draws a lower one first.
    pub render_queue: i64,
}

impl ParticleMaterial {
    pub fn scale(&self, name: &str) -> Option<[f32; 4]> {
        self.scales.iter().find(|(n, _)| n == name).map(|(_, s)| *s)
    }

    pub fn texture(&self, name: &str) -> Option<&TextureData> {
        self.textures.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }

    pub fn float(&self, name: &str) -> Option<f32> {
        self.floats.iter().find(|(n, _)| n == name).map(|(_, f)| *f)
    }

    pub fn color(&self, name: &str) -> Option<[f32; 4]> {
        self.colors.iter().find(|(n, _)| n == name).map(|(_, c)| *c)
    }
}

/// A particle system's figures as the run goes by them.
#[derive(Debug, Clone)]
pub struct ParticleSystemData {
    pub name: String,
    /// Simulated in the world's space (`moveWithTransform` 1).
    pub world_space: bool,
    pub max_particles: u32,
    /// Its own playing: how long a cycle lasts (`lengthInSec`), whether it
    /// starts again, the delay before it starts.
    pub duration: f32,
    pub looping: bool,
    pub start_delay: MinMaxCurve,
    /// The share of particles turning the other way
    /// (`randomizeRotationDirection`).
    pub flip_rotation: f32,
    /// What it does while its renderer is not seen (`cullingMode`): 0 left
    /// to the engine (a looping system paused, another simulated), 1 paused
    /// and caught up when seen again, 2 paused, 3 always simulated.
    pub culling_mode: i64,
    pub shape: Option<Shape>,
    pub emission: Option<Emission>,
    pub texture_sheet: Option<TextureSheet>,
    pub force: Option<Force>,
    /// Its velocity over life (`VelocityModule`), where only its straight
    /// part is on (its orbits, radial and speed modifier left at nought and
    /// one); otherwise it stays among the modules not built.
    pub velocity_over_lifetime: Option<VelocityOverLifetime>,
    pub rotation_by_speed: Option<RotationBySpeed>,
    pub start_lifetime: MinMaxCurve,
    pub start_speed: MinMaxCurve,
    pub start_color: MinMaxGradient,
    pub start_size: MinMaxCurve,
    pub start_rotation: MinMaxCurve,
    pub gravity_modifier: MinMaxCurve,
    pub size_over_lifetime: Option<MinMaxCurve>,
    pub rotation_over_lifetime: Option<MinMaxCurve>,
    pub color_over_lifetime: Option<MinMaxGradient>,
    pub limit_velocity: Option<LimitVelocity>,
    /// The systems it sets off (`SubModule`, where on): each emits its own
    /// emission at its parent's particle's birth, collision or death.
    pub sub_emitters: Vec<SubEmitter>,
    /// Its particles' meeting with the world (`CollisionModule`, where on).
    pub collision: Option<Collision>,
    /// Its particles' lights (`LightsModule`, where on).
    pub lights: Option<ParticleLights>,
    /// Its particles' trails (`TrailModule`), where on.
    pub trails: Option<Trails>,
    /// Modules that are on and not built (left out: particles spec, "Not
    /// built").
    pub unbuilt: Vec<String>,
    pub renderer: ParticleRenderer,
}

/// A system another sets off (`SubEmitterData`): which (the system's
/// object), when (`type`: 0 a particle's birth, 1 its collision, 2 its
/// death, 3 a trigger, 4 by hand), what it takes of the particle
/// (`properties`: 1 its colour, 2 its size, 4 its turn, 8 its lifetime, 16
/// the parent's duration) and its chance.
#[derive(Debug, Clone)]
pub struct SubEmitter {
    pub system: Option<ObjRef>,
    pub kind: i64,
    pub inherit: i64,
    pub probability: f32,
}

/// A system's particles meeting the world (`CollisionModule`): with the
/// world's colliders (`type` 1) or with planes (0); what a meeting does to
/// a particle (the speed it keeps, `m_Dampen`; the share of its normal
/// speed it bounces back with, `m_Bounce`; the share of its lifetime it
/// loses, `m_EnergyLossOnCollision`), the speeds outside which a meeting
/// kills it, its sphere's size as a share of its own, the query's quality
/// (0 high: each particle swept each frame; 1 medium and 2 low: a share of
/// the frame's rays and a cache of the planes met, by cubes of
/// `voxel_size`), the layers it meets.
#[derive(Debug, Clone)]
pub struct Collision {
    pub world: bool,
    pub dampen: MinMaxCurve,
    pub bounce: MinMaxCurve,
    pub lifetime_loss: MinMaxCurve,
    pub min_kill_speed: f32,
    pub max_kill_speed: f32,
    pub radius_scale: f32,
    pub quality: i64,
    /// The cache's cube, metres across (`voxelSize`; four times that
    /// tall), held at 0.0001 or more as the engine loads it.
    pub voxel_size: f32,
    pub collides_with: u32,
}

/// A system's particles' lights (`LightsModule`): the share of particles
/// given one, the light copied (its figures), whether they are given in
/// turn or at random, whether the particle's colour tints it, its size
/// scales its range, its alpha its intensity, the multipliers over a
/// particle's life, and the most at once.
#[derive(Debug, Clone)]
pub struct ParticleLights {
    pub ratio: f32,
    pub light: Option<EffectLight>,
    pub random_distribution: bool,
    pub use_color: bool,
    pub size_affects_range: bool,
    pub alpha_affects_intensity: bool,
    pub range_curve: MinMaxCurve,
    pub intensity_curve: MinMaxCurve,
    pub max_lights: u32,
}

/// The modules this work runs, and those whose being on changes nothing
/// in the range (external forces need wind zones or force fields). Of the
/// sub-emitters, those set off by a collision are run (the app's effects:
/// `explosion_view`); the others are said so where read.
/// A system's trails (`TrailModule`; particles spec, "Trails"): its mode
/// (0 each particle's own), the share of particles given one, its length
/// in time as a share of the particle's lifetime, the least move that adds
/// a point, how its picture is laid along it (0 stretched over it), whether
/// its points stay in the world, die with the particle, are as wide as the
/// particle is big, live as long as it does, take its colour, and its
/// colour over the particle's life, its width and colour along the trail.
#[derive(Debug, Clone)]
pub struct Trails {
    pub mode: i64,
    pub ratio: f32,
    pub lifetime: MinMaxCurve,
    pub min_vertex_distance: f32,
    pub texture_mode: i64,
    pub world_space: bool,
    pub die_with_particles: bool,
    pub size_affects_width: bool,
    pub size_affects_lifetime: bool,
    pub inherit_particle_color: bool,
    pub color_over_lifetime: MinMaxGradient,
    pub width_over_trail: MinMaxCurve,
    pub color_over_trail: MinMaxGradient,
}
