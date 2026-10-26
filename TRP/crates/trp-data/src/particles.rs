//! Unity's particle system as the install stores it (particles spec): a
//! system's figures and modules, its renderer and its material, and the
//! values that vary over a particle's life (`MinMaxCurve`,
//! `MinMaxGradient`, `Gradient`). Read here; run by `trp_sim::particles`.

use anyhow::{Context, Result};

use crate::curve::Curve;
use crate::unity::extract::{mesh_of, read_material, read_mesh, read_texture, script_class, MeshData, TextureData};
use crate::unity::{class, Assets, ObjRef, Value};

/// The effects' bundle, where the shared systems are (`Effects`).
pub const EFFECTS: &str = "assets/systems/effects/particlesystems/effects.bundle";

/// A value that can vary (`MinMaxCurve`): a constant, a curve over the
/// share of life times `scalar`, between two curves, or between two
/// constants; where it is between two, each particle has its own draw.
#[derive(Debug, Clone, PartialEq)]
pub struct MinMaxCurve {
    /// `minMaxState`: 0 a constant, 1 a curve, 2 two curves, 3 two
    /// constants.
    pub mode: u8,
    pub scalar: f32,
    pub min_scalar: f32,
    pub max_curve: Curve,
    pub min_curve: Curve,
}

impl MinMaxCurve {
    pub fn constant(value: f32) -> Self {
        Self { mode: 0, scalar: value, min_scalar: 0.0, max_curve: Curve::default(), min_curve: Curve::default() }
    }

    pub fn read(v: &Value) -> Result<Self> {
        Ok(Self {
            mode: v.req("minMaxState")?.as_i64().context("minMaxState")? as u8,
            scalar: float(v, "scalar")?,
            min_scalar: float(v, "minScalar")?,
            max_curve: crate::player::curve(v.req("maxCurve")?)?,
            min_curve: crate::player::curve(v.req("minCurve")?)?,
        })
    }

    /// At a share of life `t`, with a particle's own draw from 0 to 1.
    pub fn evaluate(&self, t: f32, draw: f32) -> f32 {
        match self.mode {
            1 => self.max_curve.evaluate(t) * self.scalar,
            2 => lerp(self.min_curve.evaluate(t), self.max_curve.evaluate(t), draw) * self.scalar,
            3 => lerp(self.min_scalar, self.scalar, draw),
            _ => self.scalar,
        }
    }
}

/// A gradient (`Gradient`): its colour keys and its alpha keys apart, each
/// at its time, blended between or stepped.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Gradient {
    pub colors: Vec<(f32, [f32; 3])>,
    pub alphas: Vec<(f32, f32)>,
    pub stepped: bool,
}

impl Gradient {
    pub fn read(v: &Value) -> Result<Self> {
        let count = |name: &str| v.get(name).and_then(Value::as_i64).unwrap_or(0).clamp(0, 8) as usize;
        let key = |k: usize| -> Result<[f32; 4]> {
            let c = v.req(&format!("key{k}"))?;
            Ok([float(c, "r")?, float(c, "g")?, float(c, "b")?, float(c, "a")?])
        };
        let time = |name: &str| -> Result<f32> { Ok(v.req(name)?.as_i64().context("a key's time")? as f32 / 65535.0) };
        let colors = (0..count("m_NumColorKeys")).map(|k| Ok((time(&format!("ctime{k}"))?, { let c = key(k)?; [c[0], c[1], c[2]] }))).collect::<Result<_>>()?;
        let alphas = (0..count("m_NumAlphaKeys")).map(|k| Ok((time(&format!("atime{k}"))?, key(k)?[3]))).collect::<Result<_>>()?;
        Ok(Self { colors, alphas, stepped: v.get("m_Mode").and_then(Value::as_i64) == Some(1) })
    }

    /// Its colour and alpha at `t`: before the first key the first's,
    /// after the last the last's.
    pub fn evaluate(&self, t: f32) -> [f32; 4] {
        let rgb = keyed(&self.colors, t, self.stepped, [1.0; 3], |a, b, s| std::array::from_fn(|i| lerp(a[i], b[i], s)));
        let a = keyed(&self.alphas, t, self.stepped, 1.0, lerp);
        [rgb[0], rgb[1], rgb[2], a]
    }
}

/// A key list's value at `t`.
fn keyed<T: Copy>(keys: &[(f32, T)], t: f32, stepped: bool, none: T, blend: impl Fn(T, T, f32) -> T) -> T {
    let Some(first) = keys.first() else { return none };
    if t <= first.0 {
        return first.1;
    }
    for pair in keys.windows(2) {
        let ((t0, a), (t1, b)) = (pair[0], pair[1]);
        if t <= t1 {
            if stepped {
                return b;
            }
            let span = t1 - t0;
            return if span > f32::EPSILON { blend(a, b, (t - t0) / span) } else { b };
        }
    }
    keys[keys.len() - 1].1
}

/// A colour that can vary (`MinMaxGradient`): a colour, a gradient, between
/// two colours, between two gradients, or a random colour of a gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct MinMaxGradient {
    /// `minMaxState`: 0 a colour, 1 a gradient, 2 two colours, 3 two
    /// gradients, 4 a random colour of the gradient.
    pub mode: u8,
    pub min_color: [f32; 4],
    pub max_color: [f32; 4],
    pub min_gradient: Gradient,
    pub max_gradient: Gradient,
}

impl MinMaxGradient {
    /// One colour throughout (mode 0).
    pub fn constant(color: [f32; 4]) -> Self {
        Self { mode: 0, min_color: color, max_color: color, min_gradient: Gradient::default(), max_gradient: Gradient::default() }
    }

    pub fn read(v: &Value) -> Result<Self> {
        let color = |name: &str| -> Result<[f32; 4]> {
            let c = v.req(name)?;
            Ok([float(c, "r")?, float(c, "g")?, float(c, "b")?, float(c, "a")?])
        };
        Ok(Self {
            mode: v.req("minMaxState")?.as_i64().context("minMaxState")? as u8,
            min_color: color("minColor")?,
            max_color: color("maxColor")?,
            min_gradient: Gradient::read(v.req("minGradient")?)?,
            max_gradient: Gradient::read(v.req("maxGradient")?)?,
        })
    }

    /// At a share of life `t`, with a particle's own draw.
    pub fn evaluate(&self, t: f32, draw: f32) -> [f32; 4] {
        match self.mode {
            1 => self.max_gradient.evaluate(t),
            2 => std::array::from_fn(|i| lerp(self.min_color[i], self.max_color[i], draw)),
            3 => {
                let (a, b) = (self.min_gradient.evaluate(t), self.max_gradient.evaluate(t));
                std::array::from_fn(|i| lerp(a[i], b[i], draw))
            }
            4 => self.max_gradient.evaluate(draw),
            _ => self.max_color,
        }
    }
}

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
/// (0 high: each particle swept each frame), the layers it meets.
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

const BUILT: [&str; 15] = [
    "TrailModule",
    "InitialModule",
    "SizeModule",
    "RotationModule",
    "ColorModule",
    "ClampVelocityModule",
    "ExternalForcesModule",
    "ShapeModule",
    "EmissionModule",
    "UVModule",
    "ForceModule",
    "RotationBySpeedModule",
    "CollisionModule",
    "SubModule",
    "LightsModule",
];

/// A light of the effects' (`Light`) by its object.
fn read_light(assets: &Assets, obj: ObjRef) -> Result<EffectLight> {
    let l = assets.read(obj)?;
    let c = l.req("m_Color")?;
    let name = l.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(obj.file, p)).and_then(|go| assets.read(go).ok()?.get("m_Name")?.as_str().map(str::to_owned)).unwrap_or_default();
    Ok(EffectLight {
        name,
        position: [0.0; 3],
        kind: l.get("m_Type").and_then(Value::as_i64).unwrap_or(2),
        color: [float(c, "r")?, float(c, "g")?, float(c, "b")?, float(c, "a")?],
        intensity: float(&l, "m_Intensity")?,
        range: float(&l, "m_Range")?,
        enabled: l.get("m_Enabled").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0))).unwrap_or(true),
        driver: None,
    })
}

fn vec3(v: &Value, name: &str) -> Result<[f32; 3]> {
    let v = v.req(name)?;
    Ok([float(v, "x")?, float(v, "y")?, float(v, "z")?])
}

fn read_shape(m: &Value) -> Result<Shape> {
    let multi = |name: &str| -> Result<f32> { float(m.req(name)?, "value") };
    Ok(Shape {
        kind: m.req("type")?.as_i64().context("type")?,
        radius: multi("radius")?,
        radius_thickness: float(m, "radiusThickness")?,
        angle: float(m, "angle")?,
        length: float(m, "length")?,
        arc: multi("arc")?,
        position: vec3(m, "m_Position")?,
        rotation: vec3(m, "m_Rotation")?,
        scale: vec3(m, "m_Scale")?,
        align_to_direction: m.get("alignToDirection").and_then(Value::as_bool).unwrap_or(false),
        random_direction: float(m, "randomDirectionAmount")?,
        spherical_direction: float(m, "sphericalDirectionAmount")?,
        random_position: float(m, "randomPositionAmount")?,
    })
}

fn read_emission(m: &Value) -> Result<Emission> {
    let bursts = m
        .get("m_Bursts")
        .and_then(Value::as_array)
        .unwrap_or(&[])
        .iter()
        .map(|b| -> Result<Burst> {
            Ok(Burst {
                time: float(b, "time")?,
                count: MinMaxCurve::read(b.req("countCurve")?)?,
                cycles: b.req("cycleCount")?.as_i64().context("cycleCount")?,
                interval: float(b, "repeatInterval")?,
                probability: float(b, "probability")?,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Emission { rate_over_time: MinMaxCurve::read(m.req("rateOverTime")?)?, rate_over_distance: MinMaxCurve::read(m.req("rateOverDistance")?)?, bursts })
}

fn read_texture_sheet(m: &Value) -> Result<TextureSheet> {
    let whole = |name: &str| m.get(name).and_then(Value::as_i64).unwrap_or(0);
    Ok(TextureSheet {
        mode: whole("mode"),
        tiles: [whole("tilesX").max(1) as u32, whole("tilesY").max(1) as u32],
        single_row: whole("animationType") == 1,
        random_row: whole("rowMode") == 1,
        row_index: whole("rowIndex").max(0) as u32,
        frame_over_time: MinMaxCurve::read(m.req("frameOverTime")?)?,
        start_frame: MinMaxCurve::read(m.req("startFrame")?)?,
        cycles: float(m, "cycles")?,
        time_mode: whole("timeMode"),
    })
}

/// Reads a particle system and its game object's renderer.
pub fn read_system(assets: &Assets, obj: ObjRef) -> Result<ParticleSystemData> {
    let s = assets.read(obj)?;
    let game_object = s.req("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p)).context("the system has no game object")?;
    let go = assets.read(game_object)?;
    let name = go.get("m_Name").and_then(Value::as_str).unwrap_or("").to_owned();
    let renderer_obj = go
        .req("m_Component")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .filter_map(|pair| pair.get("component")?.as_pptr())
        .filter_map(|p| assets.resolve_loaded(game_object.file, p))
        .find(|&component| assets.info(component).is_some_and(|info| info.class_id == class::PARTICLE_SYSTEM_RENDERER))
        .with_context(|| format!("{name} has no particle renderer"))?;
    let module = |name: &str| -> Option<&Value> { s.get(name).filter(|m| m.get("enabled").and_then(Value::as_bool) == Some(true)) };
    let main = s.req("InitialModule")?;
    let curve = |m: &Value, name: &str| MinMaxCurve::read(m.req(name)?);
    let mut unbuilt: Vec<String> = s
        .entries()
        .filter(|(key, value)| key.ends_with("Module") && value.get("enabled").and_then(Value::as_bool) == Some(true) && !BUILT.contains(key))
        .map(|(key, _)| key.to_owned())
        .collect();
    // The velocity over life, built where only its straight part is on.
    let velocity_over_lifetime = match module("VelocityModule") {
        Some(m) => {
            let quiet = |name: &str, value: f32| m.get(name).and_then(|c| MinMaxCurve::read(c).ok()).is_none_or(|c| c.mode == 0 && c.scalar == value);
            let straight = ["orbitalX", "orbitalY", "orbitalZ", "orbitalOffsetX", "orbitalOffsetY", "orbitalOffsetZ", "radial"].iter().all(|n| quiet(n, 0.0)) && quiet("speedModifier", 1.0);
            if straight {
                unbuilt.retain(|u| u != "VelocityModule");
                Some(VelocityOverLifetime { x: curve(m, "x")?, y: curve(m, "y")?, z: curve(m, "z")?, world_space: m.get("inWorldSpace").and_then(Value::as_bool).unwrap_or(false) })
            } else {
                None
            }
        }
        None => None,
    };
    let limit_velocity = module("ClampVelocityModule")
        .map(|m| -> Result<LimitVelocity> {
            Ok(LimitVelocity {
                magnitude: curve(m, "magnitude")?,
                dampen: float(m, "dampen")?,
                drag: curve(m, "drag")?,
                drag_by_size: m.get("multiplyDragByParticleSize").and_then(Value::as_bool).unwrap_or(false),
                drag_by_velocity: m.get("multiplyDragByParticleVelocity").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .transpose()?;
    Ok(ParticleSystemData {
        name,
        world_space: s.get("moveWithTransform").and_then(Value::as_i64) == Some(1),
        max_particles: main.req("maxNumParticles")?.as_i64().context("maxNumParticles")?.max(0) as u32,
        duration: float(&s, "lengthInSec")?,
        looping: s.get("looping").and_then(Value::as_bool).unwrap_or(false),
        start_delay: MinMaxCurve::read(s.req("startDelay")?)?,
        flip_rotation: main.get("randomizeRotationDirection").and_then(Value::as_f32).unwrap_or(0.0),
        shape: module("ShapeModule").map(read_shape).transpose()?,
        emission: module("EmissionModule").map(read_emission).transpose()?,
        texture_sheet: module("UVModule").map(read_texture_sheet).transpose()?,
        velocity_over_lifetime,
        force: module("ForceModule")
            .map(|m| -> Result<Force> {
                Ok(Force {
                    x: curve(m, "x")?,
                    y: curve(m, "y")?,
                    z: curve(m, "z")?,
                    world_space: m.get("inWorldSpace").and_then(Value::as_bool).unwrap_or(false),
                    randomize_per_frame: m.get("randomizePerFrame").and_then(Value::as_bool).unwrap_or(false),
                })
            })
            .transpose()?,
        rotation_by_speed: module("RotationBySpeedModule").map(|m| -> Result<RotationBySpeed> { Ok(RotationBySpeed { curve: curve(m, "curve")?, range: { let r = m.req("range")?; [float(r, "x")?, float(r, "y")?] } }) }).transpose()?,
        start_lifetime: curve(main, "startLifetime")?,
        start_speed: curve(main, "startSpeed")?,
        start_color: MinMaxGradient::read(main.req("startColor")?)?,
        start_size: curve(main, "startSize")?,
        start_rotation: curve(main, "startRotation")?,
        gravity_modifier: curve(main, "gravityModifier")?,
        size_over_lifetime: module("SizeModule").map(|m| curve(m, "curve")).transpose()?,
        rotation_over_lifetime: module("RotationModule").map(|m| curve(m, "curve")).transpose()?,
        color_over_lifetime: module("ColorModule").map(|m| MinMaxGradient::read(m.req("gradient")?)).transpose()?,
        limit_velocity,
        collision: module("CollisionModule")
            .map(|m| -> Result<Collision> {
                Ok(Collision {
                    world: m.get("type").and_then(Value::as_i64) == Some(1),
                    dampen: curve(m, "m_Dampen")?,
                    bounce: curve(m, "m_Bounce")?,
                    lifetime_loss: curve(m, "m_EnergyLossOnCollision")?,
                    min_kill_speed: float(m, "minKillSpeed")?,
                    max_kill_speed: float(m, "maxKillSpeed")?,
                    radius_scale: float(m, "radiusScale")?,
                    quality: m.get("quality").and_then(Value::as_i64).unwrap_or(0),
                    collides_with: m.get("collidesWith").and_then(|b| b.get("m_Bits")).and_then(Value::as_i64).unwrap_or(-1) as u32,
                })
            })
            .transpose()?,
        lights: module("LightsModule")
            .map(|m| -> Result<ParticleLights> {
                let flag = |name: &str| m.get(name).and_then(Value::as_bool).unwrap_or(false);
                let light = m.get("light").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(obj.file, p)).map(|light| read_light(assets, light)).transpose()?;
                Ok(ParticleLights {
                    ratio: float(m, "ratio")?,
                    light,
                    random_distribution: flag("randomDistribution"),
                    use_color: flag("color"),
                    size_affects_range: flag("range"),
                    alpha_affects_intensity: flag("intensity"),
                    range_curve: curve(m, "rangeCurve")?,
                    intensity_curve: curve(m, "intensityCurve")?,
                    max_lights: m.get("maxLights").and_then(Value::as_i64).unwrap_or(20).max(0) as u32,
                })
            })
            .transpose()?,
        trails: module("TrailModule")
            .map(|m| -> Result<Trails> {
                let flag = |name: &str| m.get(name).and_then(Value::as_bool).unwrap_or(false);
                Ok(Trails {
                    mode: m.get("mode").and_then(Value::as_i64).unwrap_or(0),
                    ratio: float(m, "ratio")?,
                    lifetime: curve(m, "lifetime")?,
                    min_vertex_distance: float(m, "minVertexDistance")?,
                    texture_mode: m.get("textureMode").and_then(Value::as_i64).unwrap_or(0),
                    world_space: flag("worldSpace"),
                    die_with_particles: flag("dieWithParticles"),
                    size_affects_width: flag("sizeAffectsWidth"),
                    size_affects_lifetime: flag("sizeAffectsLifetime"),
                    inherit_particle_color: flag("inheritParticleColor"),
                    color_over_lifetime: MinMaxGradient::read(m.req("colorOverLifetime")?)?,
                    width_over_trail: curve(m, "widthOverTrail")?,
                    color_over_trail: MinMaxGradient::read(m.req("colorOverTrail")?)?,
                })
            })
            .transpose()?,
        sub_emitters: module("SubModule")
            .and_then(|m| m.get("subEmitters")?.as_array())
            .unwrap_or(&[])
            .iter()
            .map(|e| SubEmitter {
                system: e.get("emitter").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(obj.file, p)),
                kind: e.get("type").and_then(Value::as_i64).unwrap_or(0),
                inherit: e.get("properties").and_then(Value::as_i64).unwrap_or(0),
                probability: e.get("emitProbability").and_then(Value::as_f32).unwrap_or(1.0),
            })
            .collect(),
        unbuilt,
        renderer: read_renderer(assets, renderer_obj)?,
    })
}

fn read_renderer(assets: &Assets, obj: ObjRef) -> Result<ParticleRenderer> {
    let r = assets.read(obj)?;
    let number = |name: &str| r.get(name).and_then(Value::as_i64).unwrap_or(0);
    let materials: Vec<Option<ObjRef>> = r.req("m_Materials")?.as_array().unwrap_or(&[]).iter().map(|m| m.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p))).collect();
    // On or off (a flag the bundles keep as a boolean).
    let enabled = r.get("m_Enabled").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0))).unwrap_or(true);
    // A renderer that is off may have no material (a muzzle blast's root).
    let material = match materials.first().copied().flatten() {
        Some(m) => read_particle_material(assets, m)?,
        None if !enabled => ParticleMaterial { name: String::new(), shader: String::new(), textures: Vec::new(), floats: Vec::new(), colors: Vec::new(), scales: Vec::new(), keywords: Vec::new(), render_queue: 2000 },
        None => anyhow::bail!("the particle renderer has no material"),
    };
    let trail_material = materials.get(1).copied().flatten().map(|m| read_particle_material(assets, m)).transpose()?;
    let real = |name: &str| r.get(name).and_then(Value::as_f32).unwrap_or(0.0);
    Ok(ParticleRenderer {
        render_mode: number("m_RenderMode"),
        velocity_scale: real("m_VelocityScale"),
        length_scale: real("m_LengthScale"),
        camera_velocity_scale: real("m_CameraVelocityScale"),
        sorting_fudge: real("m_SortingFudge"),
        alignment: number("m_RenderAlignment"),
        sort_mode: number("m_SortMode"),
        min_size: r.get("m_MinParticleSize").and_then(Value::as_f32).unwrap_or(0.0),
        max_size: r.get("m_MaxParticleSize").and_then(Value::as_f32).unwrap_or(f32::MAX),
        material,
        trail_material,
        enabled,
        mesh: if number("m_RenderMode") == 4 { renderer_mesh(assets, obj, r.req("m_Mesh")?)? } else { None },
        normal_direction: r.get("m_NormalDirection").and_then(Value::as_f32).unwrap_or(1.0),
    })
}

/// A particle renderer's material: its shader's name and queue, its
/// pictures, figures and colours by their names, its pictures' places.
fn read_particle_material(assets: &Assets, material_obj: ObjRef) -> Result<ParticleMaterial> {
    let data = read_material(assets, material_obj)?;
    let material = assets.read(material_obj)?;
    let shader_value = material.req("m_Shader")?.as_pptr().and_then(|p| assets.resolve_loaded(material_obj.file, p)).and_then(|shader| assets.read(shader).ok());
    let shader = shader_value.as_ref().and_then(|shader| shader.get("m_ParsedForm")?.get("m_Name")?.as_str().map(str::to_owned)).unwrap_or_default();
    let own_queue = material.get("m_CustomRenderQueue").and_then(Value::as_i64).unwrap_or(-1);
    let render_queue = if own_queue >= 0 { own_queue } else { shader_value.as_ref().and_then(shader_queue).unwrap_or(2000) };
    let mut textures = Vec::new();
    let mut names: Vec<_> = data.textures.iter().collect();
    names.sort_by(|a, b| a.0.cmp(b.0));
    for (name, texture) in names {
        textures.push((name.clone(), read_texture(assets, *texture).with_context(|| format!("{}'s {name}", data.name))?));
    }
    let mut floats: Vec<_> = data.floats.iter().map(|(n, f)| (n.clone(), *f)).collect();
    floats.sort_by(|a, b| a.0.cmp(&b.0));
    let mut colors: Vec<_> = data.colors.iter().map(|(n, c)| (n.clone(), *c)).collect();
    colors.sort_by(|a, b| a.0.cmp(&b.0));
    let scales = material
        .req("m_SavedProperties")?
        .get("m_TexEnvs")
        .and_then(Value::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(|pair| {
            let name = pair.get("first")?.as_str()?.to_owned();
            let env = pair.get("second")?;
            let (s, o) = (env.get("m_Scale")?, env.get("m_Offset")?);
            Some((name, [s.get("x")?.as_f32()?, s.get("y")?.as_f32()?, o.get("x")?.as_f32()?, o.get("y")?.as_f32()?]))
        })
        .collect();
    Ok(ParticleMaterial { name: data.name, shader, textures, floats, colors, scales, keywords: data.keywords, render_queue })
}

/// The file of the build Unity's own meshes are in, as a bundle names it
/// among its externals, and where it lies in the install.
const DEFAULT_RESOURCES: (&str, &str) = ("Library/unity default resources", "Resources/unity default resources");

/// A mesh renderer's mesh (`m_Mesh`): one of the bundle's, or one of
/// Unity's own (the built-in sphere, 10207), read from the install's copy
/// of the engine's resources by the tree of a mesh borrowed from the
/// bundle the renderer is in (that file keeps no trees).
fn renderer_mesh(assets: &Assets, obj: ObjRef, pointer: &Value) -> Result<Option<std::sync::Arc<MeshData>>> {
    let Some(p) = pointer.as_pptr() else { return Ok(None) };
    if p.path_id == 0 {
        return Ok(None);
    }
    if let Some(mesh) = assets.resolve_loaded(obj.file, p) {
        return Ok(Some(std::sync::Arc::new(read_mesh(assets, mesh)?)));
    }
    let file = assets.file(obj.file);
    let external = usize::try_from(p.file_id).ok().and_then(|n| n.checked_sub(1)).and_then(|n| file.meta.externals.get(n)).context("the particle renderer's mesh is in no file it names")?;
    anyhow::ensure!(external == DEFAULT_RESOURCES.0, "the particle renderer's mesh is in {external}, which is not read");
    let tree = file.meta.types.iter().find(|ty| ty.class_id == class::MESH && !ty.tree.is_empty()).map(|ty| ty.tree.clone()).context("the particle renderer's bundle has no mesh's tree to read Unity's own by")?;
    let v = crate::screen::build_object(&assets.install, DEFAULT_RESOURCES.1, p.path_id, class::MESH, &tree)?;
    Ok(Some(std::sync::Arc::new(mesh_of(&v, None)?)))
}

/// One of the muzzles' shared systems (`Effects.MuzzleEffect`'s field
/// `field`).
fn muzzle_system(assets: &mut Assets, field: &str) -> Result<ParticleSystemData> {
    // Its material's shader is in the shaders' bundle, which the effects'
    // does not bring.
    assets.load("shaders")?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let system = assets
        .read(effects)?
        .req("MuzzleEffect")?
        .req(field)?
        .as_pptr()
        .and_then(|p| assets.resolve_loaded(effects.file, p))
        .with_context(|| format!("the effects have no muzzle {field}"))?;
    read_system(assets, system)
}

/// The shared system the muzzles' fumes are emitted into
/// (`Effects.MuzzleEffect.Fume`).
pub fn muzzle_fume_system(assets: &mut Assets) -> Result<ParticleSystemData> {
    muzzle_system(assets, "Fume")
}

/// The shared system the muzzles' heat haze is emitted into
/// (`Effects.MuzzleEffect.HeatHaze`; firing spec, "The muzzle's heat
/// haze"): drawn not on the frame but into the camera's distortion
/// (`DistortRenderer`, `DistortCameraFX`).
pub fn muzzle_heat_haze_system(assets: &mut Assets) -> Result<ParticleSystemData> {
    muzzle_system(assets, "HeatHaze")
}

/// The game's own renderer all the muzzles' sparks are emitted into
/// (`SimpleSparksRenderer`, `Effects.MuzzleEffect.Spark`; firing spec, "The
/// muzzle's sparks"): how many it holds, how many segments a streak has,
/// and its material's picture and figures (shader
/// `Custom/SimpleParticleMoveAdd`).
#[derive(Debug, Clone)]
pub struct SparksRendererData {
    pub segments: u32,
    pub count: u32,
    pub shader: String,
    pub texture: TextureData,
    /// `_Size`, `_FadeOut`.
    pub size: f32,
    pub fade_out: f32,
    /// `_TurbulenceFrequency0`, `_TurbulenceAmplitude0`,
    /// `_TurbulenceFrequency1`, `_TurbulenceAmplitude1`, where the material
    /// has `TURBULENCE` on.
    pub turbulence: Option<[f32; 4]>,
    pub tint: [f32; 4],
}

/// The shared sparks' renderer, read from the effects' bundle.
pub fn muzzle_sparks_renderer(assets: &mut Assets) -> Result<SparksRendererData> {
    assets.load("shaders")?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let renderer = assets
        .read(effects)?
        .req("MuzzleEffect")?
        .req("Spark")?
        .as_pptr()
        .and_then(|p| assets.resolve_loaded(effects.file, p))
        .context("the effects have no muzzle sparks")?;
    let r = assets.read(renderer)?;
    let whole = |name: &str| -> Result<u32> { Ok(r.req(name)?.as_i64().with_context(|| format!("{name}"))?.max(0) as u32) };
    let (segments, count) = (whole("Segments")?.max(2), whole("Count")?.max(1));
    // As `OnValidate` holds them: a mesh of 16-bit indices.
    let count = count.min(65535 / ((segments + 1) << 1));
    let game_object = r.req("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(renderer.file, p)).context("the sparks have no game object")?;
    let mesh_renderer = assets
        .read(game_object)?
        .req("m_Component")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .filter_map(|pair| pair.get("component")?.as_pptr())
        .filter_map(|p| assets.resolve_loaded(game_object.file, p))
        .find(|&component| assets.info(component).is_some_and(|info| info.class_id == class::MESH_RENDERER))
        .context("the sparks have no mesh renderer")?;
    let material_obj = assets
        .read(mesh_renderer)?
        .req("m_Materials")?
        .as_array()
        .and_then(|all| all.first())
        .and_then(Value::as_pptr)
        .and_then(|p| assets.resolve_loaded(mesh_renderer.file, p))
        .context("the sparks' renderer has no material")?;
    let m = read_material(assets, material_obj)?;
    let shader = assets
        .read(material_obj)?
        .req("m_Shader")?
        .as_pptr()
        .and_then(|p| assets.resolve_loaded(material_obj.file, p))
        .and_then(|shader| assets.read(shader).ok())
        .and_then(|shader| shader.get("m_ParsedForm")?.get("m_Name")?.as_str().map(str::to_owned))
        .unwrap_or_default();
    let number = |name: &str| m.floats.get(name).copied().with_context(|| format!("the sparks' material has no {name}"));
    let texture = m.textures.get("_MainTex").copied().context("the sparks' material has no picture")?;
    let turbulence = if m.keywords.iter().any(|k| k == "TURBULENCE") {
        Some([number("_TurbulenceFrequency0")?, number("_TurbulenceAmplitude0")?, number("_TurbulenceFrequency1")?, number("_TurbulenceAmplitude1")?])
    } else {
        None
    };
    Ok(SparksRendererData {
        segments,
        count,
        shader,
        texture: read_texture(assets, texture)?,
        size: number("_Size")?,
        fade_out: number("_FadeOut")?,
        turbulence,
        tint: m.colors.get("_TintColor").copied().unwrap_or([1.0; 4]),
    })
}

/// One of an effect's systems, where it is from the effect's root: its
/// place, turn and size there, and how deep it hangs; the system itself,
/// and whether another of the effect's sets it off (a sub-emitter, which
/// playing the effect does not play: it emits only when set off).
#[derive(Debug, Clone)]
pub struct EffectSystem {
    pub data: ParticleSystemData,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub depth: usize,
    pub system: ObjRef,
    pub sub_emitter: bool,
}

/// One of an effect's lights (`Light`), where it is from the root: its
/// kind, colour, intensity and range, and whether it is on.
#[derive(Debug, Clone)]
pub struct EffectLight {
    pub name: String,
    pub position: [f32; 3],
    pub kind: i64,
    pub color: [f32; 4],
    pub intensity: f32,
    pub range: f32,
    pub enabled: bool,
    /// A script that drives its intensity by a system's playing
    /// (`MuzzleParticleLight`), where it has one.
    pub driver: Option<LightDriver>,
}

/// `MuzzleParticleLight`: the light's intensity eased toward the higher of
/// its two while its system plays, toward the lower after
/// (`Mathf.SmoothDamp`, its time the frame's times `time`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightDriver {
    pub system: ObjRef,
    pub min_max: [f32; 2],
    pub time: f32,
}

/// The muzzles' shared blasts (`Effects.MuzzleEffect.CommonSystems`, each a
/// `MuzzleParticleContainer`): each one's pivot (0 the front, 1 the back:
/// `EMuzzleParticlePivot`) and its effect (its root system and every one
/// below it, played together where a weapon's pivot of that kind is,
/// stopped and played again at each shot).
pub fn muzzle_blasts(assets: &mut Assets) -> Result<Vec<(i64, ParticleEffect)>> {
    assets.load("shaders")?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let containers: Vec<ObjRef> = assets
        .read(effects)?
        .req("MuzzleEffect")?
        .req("CommonSystems")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .filter_map(Value::as_pptr)
        .filter_map(|p| assets.resolve_loaded(effects.file, p))
        .collect();
    let mut out = Vec::new();
    for container in containers {
        let c = assets.read(container)?;
        let pivot = c.req("_pivot")?.as_i64().unwrap_or(0);
        let go = c.req("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(container.file, p)).context("a muzzle blast has no game object")?;
        let root = assets
            .read(go)?
            .req("m_Component")?
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter_map(|pair| pair.get("component")?.as_pptr())
            .filter_map(|p| assets.resolve_loaded(go.file, p))
            .find(|&component| assets.info(component).is_some_and(|info| info.class_id == class::PARTICLE_SYSTEM))
            .context("a muzzle blast has no particle system")?;
        out.push((pivot, read_effect(assets, root)?));
    }
    Ok(out)
}

/// A whole effect (a system and every one below it, played together:
/// `Play(withChildren: true)`), and its lights; how many copies of it its
/// mediator plays in turn (one where none).
#[derive(Debug, Clone)]
pub struct ParticleEffect {
    pub name: String,
    pub systems: Vec<EffectSystem>,
    pub lights: Vec<EffectLight>,
    pub copies: usize,
    /// The effects' list's entry lays a deferred decal (`UseDeferredDecals`,
    /// with the effects' decal renderer set); a grenade's is the renderer's
    /// grenade decal (`deferred_grenade_decal`).
    pub deferred_decal: bool,
}

/// One of the deferred decal renderer's decals (`DeferredDecalRenderer`,
/// `SingleDecal`; grenades spec, "The scorch"): its sizes drawn between,
/// whether it is turned at random about its normal, its sheet's rows and
/// columns, the renderer's box height along the normal for it and the most
/// static decals at once, and its material (`DecalMaterial`): its shader's
/// name, its picture (`_MainTex`), `_Color`, `_SpecularColor` and
/// `_NormalPower`.
#[derive(Debug, Clone)]
pub struct DeferredDecal {
    pub sizes: [f32; 2],
    pub randomize_rotation: bool,
    pub rows: u32,
    pub columns: u32,
    pub projector_height: f32,
    pub max_decals: u32,
    pub shader: String,
    pub picture: TextureData,
    pub color: [f32; 4],
    pub specular_color: [f32; 4],
    pub normal_power: f32,
}

/// The effects' deferred decal renderer (`DeferredDecals`), its figures read.
fn decal_renderer(assets: &mut Assets) -> Result<(ObjRef, Value)> {
    assets.load("shaders")?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let renderer = assets.read(effects)?.req("DeferredDecals")?.as_pptr().and_then(|p| assets.resolve_loaded(effects.file, p)).context("the effects have no deferred decal renderer")?;
    let r = assets.read(renderer)?;
    Ok((renderer, r))
}

/// The effects' decal renderer's grenade decal (`_grenadeDecal`, with
/// `_decalProjectorHeightForGrenade` and `_maxDecals`), read from the
/// effects' bundle.
pub fn deferred_grenade_decal(assets: &mut Assets) -> Result<DeferredDecal> {
    let (renderer, r) = decal_renderer(assets)?;
    let height = float(&r, "_decalProjectorHeightForGrenade")?;
    single_decal(assets, renderer, &r, r.req("_grenadeDecal")?, height)
}

/// The bullets' decals of the effects' decal renderer (`_decals`, each by
/// the materials it is for, `DecalMaterialType`; `_genericDecal` for any
/// other), with `_decalProjectorHeight` and `_maxDecals`
/// (`DeferredDecalRenderer.GetSingleDecal`; ballistics spec, "What a hit
/// shows and sounds").
#[derive(Debug, Clone)]
pub struct BulletDecals {
    pub by_material: Vec<(Vec<i32>, DeferredDecal)>,
    pub generic: DeferredDecal,
}

pub fn deferred_bullet_decals(assets: &mut Assets) -> Result<BulletDecals> {
    let (renderer, r) = decal_renderer(assets)?;
    let height = float(&r, "_decalProjectorHeight")?;
    let mut by_material = Vec::new();
    for decal in r.req("_decals")?.as_array().unwrap_or(&[]) {
        let materials = decal.req("DecalMaterialType")?;
        let materials: Vec<i32> = match materials.as_ints() {
            Some(list) => list.iter().map(|&m| m as i32).collect(),
            None => materials.as_array().unwrap_or(&[]).iter().filter_map(|m| m.as_i64().map(|m| m as i32)).collect(),
        };
        by_material.push((materials, single_decal(assets, renderer, &r, decal, height)?));
    }
    let generic = single_decal(assets, renderer, &r, r.req("_genericDecal")?, height)?;
    Ok(BulletDecals { by_material, generic })
}

/// One of the renderer's decals (`SingleDecal`).
fn single_decal(assets: &mut Assets, renderer: ObjRef, r: &Value, decal: &Value, projector_height: f32) -> Result<DeferredDecal> {
    let size = decal.req("DecalSize")?;
    let material_obj = decal.req("DecalMaterial")?.as_pptr().and_then(|p| assets.resolve_loaded(renderer.file, p)).context("the decal has no material")?;
    let m = read_material(assets, material_obj)?;
    let shader = assets
        .read(material_obj)?
        .req("m_Shader")?
        .as_pptr()
        .and_then(|p| assets.resolve_loaded(material_obj.file, p))
        .and_then(|shader| assets.read(shader).ok())
        .and_then(|shader| shader.get("m_ParsedForm")?.get("m_Name")?.as_str().map(str::to_owned))
        .unwrap_or_default();
    let picture = m.textures.get("_MainTex").copied().context("the decal's material has no picture")?;
    let whole = |v: &Value, name: &str| -> Result<u32> { Ok(v.req(name)?.as_i64().with_context(|| format!("{name}"))?.max(1) as u32) };
    Ok(DeferredDecal {
        sizes: [float(size, "x")?, float(size, "y")?],
        randomize_rotation: decal.req("RandomizeRotation")?.as_i64().unwrap_or(0) != 0,
        rows: whole(decal, "TileSheetRows")?,
        columns: whole(decal, "TileSheetColumns")?,
        projector_height,
        max_decals: whole(r, "_maxDecals")?,
        shader,
        picture: read_texture(assets, picture)?,
        color: m.colors.get("_Color").copied().unwrap_or([1.0; 4]),
        specular_color: m.colors.get("_SpecularColor").copied().unwrap_or([0.0; 4]),
        normal_power: m.floats.get("_NormalPower").copied().unwrap_or(0.0),
    })
}

fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|c| 2.0 * c);
    let c = cross(u, t);
    [v[0] + q[3] * t[0] + c[0], v[1] + q[3] * t[1] + c[1], v[2] + q[3] * t[2] + c[2]]
}

/// Reads an effect from its root system, down its objects' children.
pub fn read_effect(assets: &Assets, root: ObjRef) -> Result<ParticleEffect> {
    let game_object = |obj: ObjRef| -> Option<ObjRef> { assets.read(obj).ok()?.get("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p)) };
    let components = |go: ObjRef| -> Vec<ObjRef> {
        assets
            .read(go)
            .ok()
            .and_then(|g| g.get("m_Component")?.as_array().map(|all| all.iter().filter_map(|pair| pair.get("component")?.as_pptr()).filter_map(|p| assets.resolve_loaded(go.file, p)).collect()))
            .unwrap_or_default()
    };
    let of_class = |go: ObjRef, class_id: i32| components(go).into_iter().find(|&c| assets.info(c).is_some_and(|info| info.class_id == class_id));
    let root_go = game_object(root).context("the effect's root has no game object")?;
    let name = assets.read(root_go)?.get("m_Name").and_then(Value::as_str).unwrap_or("").to_owned();
    let mut effect = ParticleEffect { name, systems: Vec::new(), lights: Vec::new(), copies: 1, deferred_decal: false };
    // Each object with its place, turn and size from the root (the root's
    // own being where it is played).
    let mut stack = vec![(root_go, [0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0], [1.0f32; 3], 0usize)];
    while let Some((go, at, turn, size, depth)) = stack.pop() {
        if let Some(system) = of_class(go, class::PARTICLE_SYSTEM) {
            effect.systems.push(EffectSystem { data: read_system(assets, system)?, position: at, rotation: turn, scale: size, depth, system, sub_emitter: false });
        }
        if let Some(light) = of_class(go, class::LIGHT) {
            // A script beside it that drives it (`MuzzleParticleLight`).
            let driver = components(go).into_iter().filter(|&c| script_class(assets, c).as_deref() == Some("MuzzleParticleLight")).find_map(|c| {
                let d = assets.read(c).ok()?;
                let system = d.get("_particleSystem")?.as_pptr().and_then(|p| assets.resolve_loaded(c.file, p))?;
                let mm = d.get("_minMaxIntensity")?;
                Some(LightDriver { system, min_max: [mm.get("x")?.as_f32()?, mm.get("y")?.as_f32()?], time: d.get("_time")?.as_f32()? })
            });
            effect.lights.push(EffectLight { position: at, driver, ..read_light(assets, light)? });
        }
        let Some(transform) = of_class(go, class::TRANSFORM) else { continue };
        let t = assets.read(transform)?;
        let children: Vec<ObjRef> = t.get("m_Children").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_pptr).filter_map(|p| assets.resolve_loaded(transform.file, p)).collect();
        for child in children.into_iter().rev() {
            let c = assets.read(child)?;
            let q = c.req("m_LocalRotation")?;
            let local_turn = [float(q, "x")?, float(q, "y")?, float(q, "z")?, float(q, "w")?];
            let (local_at, local_size) = (vec3(&c, "m_LocalPosition")?, vec3(&c, "m_LocalScale")?);
            let scaled = [local_at[0] * size[0], local_at[1] * size[1], local_at[2] * size[2]];
            let moved = rotate(turn, scaled);
            let child_at = [at[0] + moved[0], at[1] + moved[1], at[2] + moved[2]];
            let child_turn = crate::unity::extract::quat_mul(turn, local_turn);
            let child_size = [size[0] * local_size[0], size[1] * local_size[1], size[2] * local_size[2]];
            if let Some(child_go) = game_object(child) {
                stack.push((child_go, child_at, child_turn, child_size, depth + 1));
            }
        }
    }
    let set_off: Vec<ObjRef> = effect.systems.iter().flat_map(|s| s.data.sub_emitters.iter().filter_map(|e| e.system)).collect();
    for s in &mut effect.systems {
        s.sub_emitter = set_off.contains(&s.system);
    }
    Ok(effect)
}

/// An effect of the effects' list by its name (`Effects.EffectsArray`, as
/// `EmitGrenade` finds it), from its mediator's first system
/// (`BasicParticleSystemMediator`: copies played in turn, for effects at
/// once; the copies are alike, by their names `(1)` and on, and their
/// systems).
pub fn named_effect(assets: &mut Assets, name: &str) -> Result<ParticleEffect> {
    assets.load("shaders")?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let all = assets.read(effects)?;
    let entry = all
        .req("EffectsArray")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .find(|e| e.get("Name").and_then(Value::as_str) == Some(name))
        .with_context(|| format!("no effect named {name}"))?
        .clone();
    let mediator = entry.req("BasicParticleSystemMediator")?.as_pptr().and_then(|p| assets.resolve_loaded(effects.file, p)).with_context(|| format!("{name} has no particle mediator"))?;
    let copies = assets.read(mediator)?.req("_particleSystems")?.as_array().map(<[Value]>::to_vec).unwrap_or_default();
    let first = copies.first().and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(mediator.file, p)).with_context(|| format!("{name}'s mediator has no system"))?;
    let mut effect = read_effect(assets, first)?;
    effect.copies = copies.len();
    effect.deferred_decal = entry.get("UseDeferredDecals").and_then(Value::as_i64).unwrap_or(0) != 0 && entry.get("DeferredDecals").and_then(Value::as_pptr).is_some_and(|p| p.path_id != 0);
    Ok(effect)
}

/// A value drawn as `min` plus a share of `range` (`RundomRange`: drawn
/// only where the range is above nought, else `min`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RandomRange {
    pub min: f32,
    pub range: f32,
}

/// An older emitter onto a particle system (`ParticleSystemAdapter`): the
/// ranges each particle's lifetime, speed and size are drawn from, its turn
/// (drawn from the game's table of turns, or fixed), its colour, and the
/// system it emits into (shared by every effect that names it).
#[derive(Debug, Clone)]
pub struct SystemAdapter {
    pub lifetime: RandomRange,
    pub speed: RandomRange,
    pub size: RandomRange,
    pub rotation_random: bool,
    pub rotation: f32,
    pub color: [u8; 4],
    /// The adapter itself (its draws are its own, whichever effect names
    /// it) and the system it emits into.
    pub adapter: ObjRef,
    pub system: ObjRef,
    pub data: ParticleSystemData,
}

/// An older emitter onto the shared sparks' renderer
/// (`SimpleSparksAdapter`): its tables of 128 (each its curve times its
/// value, kept with it), one place drawn for each spark, and its time's
/// shift.
#[derive(Debug, Clone)]
pub struct SparksAdapter {
    pub lifetime: Vec<f32>,
    pub speed: Vec<f32>,
    pub gravity: Vec<f32>,
    pub drag: Vec<f32>,
    pub emission: Vec<u8>,
    pub size: Vec<u8>,
    pub turbulence: Vec<u8>,
    pub frequency: Vec<u8>,
    pub time_shift: f32,
}

/// What an older emitter emits into.
#[derive(Debug, Clone)]
pub enum EmitterKind {
    System(SystemAdapter),
    Sparks(SparksAdapter),
}

/// One of an effect's older emitters (`Effect.ParticleSys`): the camera's
/// distances it plays within, how its particles' ways are drawn
/// (`RandomType`: 0 the normal, 1 the normal plus a random unit way, 2 a
/// way over half a sphere, 3 a way across from 0.3 m up, 4 the normal plus
/// a way over half a sphere made a unit, 5 within 60° of up), how many
/// (`MinCount`, and a draw below `RandomCountRange` added where that is at
/// least one), the scale of each way's axes (`RandomScale` where
/// `UseRandomScale`), and what it emits into.
#[derive(Debug, Clone)]
pub struct OlderEmitter {
    pub distance: [f32; 2],
    pub way: i64,
    pub min_count: i64,
    pub count_range: i64,
    pub random_scale: Option<[f32; 3]>,
    pub kind: EmitterKind,
}

/// An effect's light from the pool (`Effect.Light`, `LightPool.Add`): its
/// colour, the camera's distance it is lit within, its range, intensity
/// and time, its shadows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoolLight {
    pub color: [f32; 4],
    pub max_distance: f32,
    pub range: f32,
    pub intensity: f32,
    pub time: f32,
    pub shadows: bool,
}

/// An entry of the effects' list beyond its mediator (`Effects.Effect`):
/// its older emitters, how far along the normal they play
/// (`ParticlesShift`), and its pool's light.
#[derive(Debug, Clone)]
pub struct EffectEntry {
    pub name: String,
    pub emitters: Vec<OlderEmitter>,
    pub particles_shift: f32,
    pub light: Option<PoolLight>,
}

/// An entry of the effects' list that names materials (`Effects.
/// EffectsArray`, `MaterialTypes`; a material's effect is the first entry
/// naming it, `InitDictionaryAndNames`): its name, its materials, whether it
/// lays one of the deferred renderer's decals (`UseDeferredDecals`, the
/// renderer set) and whether the older decal system's (`Decal`, played where
/// it lays no deferred one).
#[derive(Debug, Clone)]
pub struct MaterialEntry {
    pub name: String,
    pub materials: Vec<i32>,
    pub deferred_decal: bool,
    pub old_decal: bool,
}

pub fn material_entries(assets: &mut Assets) -> Result<Vec<MaterialEntry>> {
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let all = assets.read(effects)?;
    let mut out = Vec::new();
    for entry in all.req("EffectsArray")?.as_array().unwrap_or(&[]) {
        let list = entry.req("MaterialTypes")?;
        let materials: Vec<i32> = match list.as_ints() {
            Some(m) => m.to_vec(),
            None => list.as_array().unwrap_or(&[]).iter().filter_map(|m| m.as_i64().map(|m| m as i32)).collect(),
        };
        if materials.is_empty() {
            continue;
        }
        let set = |field: &str| entry.get(field).and_then(Value::as_pptr).is_some_and(|p| p.path_id != 0);
        out.push(MaterialEntry {
            name: entry.get("Name").and_then(Value::as_str).unwrap_or("").to_owned(),
            materials,
            deferred_decal: entry.get("UseDeferredDecals").and_then(Value::as_i64).unwrap_or(0) != 0 && set("DeferredDecals"),
            old_decal: set("Decal"),
        });
    }
    Ok(out)
}

/// An entry of the effects' list by its name: its older emitters (each
/// adapter, and the system or sparks it emits into) and its light.
pub fn effect_entry(assets: &mut Assets, name: &str) -> Result<EffectEntry> {
    assets.load("shaders")?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let all = assets.read(effects)?;
    let entry = all
        .req("EffectsArray")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .find(|e| e.get("Name").and_then(Value::as_str) == Some(name))
        .with_context(|| format!("no effect named {name}"))?
        .clone();
    let mut emitters = Vec::new();
    for sys in entry.req("Particles")?.as_array().unwrap_or(&[]) {
        let Some(adapter) = sys.req("Particle")?.as_pptr().and_then(|p| assets.resolve_loaded(effects.file, p)) else { continue };
        let a = assets.read(adapter)?;
        let kind = match script_class(assets, adapter).as_deref() {
            Some("ParticleSystemAdapter") => {
                let range = |name: &str| -> Result<RandomRange> {
                    let r = a.req(name)?;
                    Ok(RandomRange { min: float(r, "Min")?, range: float(r, "Range")? })
                };
                let system = a
                    .req("ParticleSystemObject")?
                    .as_pptr()
                    .and_then(|p| assets.resolve_loaded(adapter.file, p))
                    .with_context(|| format!("{name}'s adapter has no particle system"))?;
                let rgba = a.req("Color")?.req("rgba")?.as_i64().context("the adapter's colour")? as u32;
                EmitterKind::System(SystemAdapter {
                    lifetime: range("Lifetime")?,
                    speed: range("Speed")?,
                    size: range("Size")?,
                    rotation_random: a.req("RotationRandom")?.as_i64().unwrap_or(0) != 0,
                    rotation: float(&a, "Rotation")?,
                    color: rgba.to_le_bytes(),
                    adapter,
                    system,
                    data: read_system(assets, system)?,
                })
            }
            Some("SimpleSparksAdapter") => {
                let floats = |field: &str| -> Result<Vec<f32>> {
                    let values = a.req(field)?.req("Values")?;
                    let all = match values.as_floats() {
                        Some(f) => f.to_vec(),
                        None => values.as_array().unwrap_or(&[]).iter().filter_map(Value::as_f32).collect(),
                    };
                    anyhow::ensure!(!all.is_empty(), "{field} has no values");
                    Ok(all)
                };
                let bytes = |field: &str| -> Result<Vec<u8>> {
                    let values = a.req(field)?.req("Values")?;
                    let all = match values.as_bytes() {
                        Some(b) => b.to_vec(),
                        None => values.as_array().unwrap_or(&[]).iter().filter_map(|v| v.as_i64().map(|n| n as u8)).collect(),
                    };
                    anyhow::ensure!(!all.is_empty(), "{field} has no values");
                    Ok(all)
                };
                EmitterKind::Sparks(SparksAdapter {
                    lifetime: floats("LifetimeCurve")?,
                    speed: floats("SpeedCurve")?,
                    gravity: floats("GravityCurve")?,
                    drag: floats("DragCurve")?,
                    emission: bytes("EmissionCurve")?,
                    size: bytes("SizeCurve")?,
                    turbulence: bytes("TurbulenceAmplitudeCurve")?,
                    frequency: bytes("TurbulenceFrequencyCurve")?,
                    time_shift: float(&a, "EmissionTimeShift")?,
                })
            }
            other => anyhow::bail!("{name}'s emitter of kind {other:?} is not read"),
        };
        let distance = sys.req("Distance")?;
        let scale = vec3(sys, "RandomScale")?;
        emitters.push(OlderEmitter {
            distance: [float(distance, "x")?, float(distance, "y")?],
            way: sys.req("RandomType")?.as_i64().unwrap_or(0),
            min_count: sys.req("MinCount")?.as_i64().unwrap_or(0),
            count_range: sys.req("RandomCountRange")?.as_i64().unwrap_or(0),
            random_scale: (sys.req("UseRandomScale")?.as_i64().unwrap_or(0) != 0).then_some(scale),
            kind,
        });
    }
    let light = (entry.req("Light")?.as_i64().unwrap_or(0) != 0)
        .then(|| -> Result<PoolLight> {
            let c = entry.req("LightColor")?;
            Ok(PoolLight {
                color: [float(c, "r")?, float(c, "g")?, float(c, "b")?, float(c, "a")?],
                max_distance: float(&entry, "LightMaxDist")?,
                range: float(&entry, "LightRange")?,
                intensity: float(&entry, "LightIntensity")?,
                time: float(&entry, "LightTime")?,
                shadows: entry.req("WithShadows")?.as_i64().unwrap_or(0) != 0,
            })
        })
        .transpose()?;
    Ok(EffectEntry { name: name.to_owned(), emitters, particles_shift: float(&entry, "ParticlesShift")?, light })
}

/// A shader's queue by its `Queue` tag (its subshader's or a pass's, the
/// first found): `Background` 1000, `Geometry` 2000, `AlphaTest` 2450,
/// `Transparent` 3000, `Overlay` 4000, with any `+n` or `-n`.
fn shader_queue(shader: &Value) -> Option<i64> {
    fn find(v: &Value) -> Option<String> {
        if let Some(first) = v.get("first").and_then(Value::as_str) {
            if first.eq_ignore_ascii_case("queue") {
                return v.get("second").and_then(Value::as_str).map(str::to_owned);
            }
        }
        if let Some(all) = v.as_array() {
            return all.iter().find_map(find);
        }
        v.entries().find_map(|(_, child)| find(child))
    }
    let tag = find(shader.get("m_ParsedForm")?)?;
    let (name, shift) = match tag.find(['+', '-']) {
        Some(at) => (&tag[..at], tag[at..].parse::<i64>().unwrap_or(0)),
        None => (tag.as_str(), 0),
    };
    let base = match name.trim().to_ascii_lowercase().as_str() {
        "background" => 1000,
        "geometry" => 2000,
        "alphatest" => 2450,
        "transparent" => 3000,
        "overlay" => 4000,
        _ => return None,
    };
    Some(base + shift)
}

fn float(v: &Value, name: &str) -> Result<f32> {
    v.req(name)?.as_f32().with_context(|| format!("{name} is not a number"))
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
