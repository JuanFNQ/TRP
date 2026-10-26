//! A weapon's muzzle flash (firing spec, "The muzzle flash"; `MuzzleManager`,
//! `MuzzleJet` and the jets' material): which of the weapon's objects are
//! its muzzle effects, its manager's figures and curves, its jets'
//! particles, and its material's figures and picture.

use anyhow::{Context, Result};

use crate::curve::Curve;
use crate::unity::extract::{read_texture, Hierarchy, TextureData};
use crate::unity::{Assets, ObjRef, Value};

/// The scripts that make an object one of the weapon's muzzle effects
/// (`MuzzleEffect` and those made from it).
pub const MUZZLE_EFFECTS: [&str; 4] = ["MuzzleEffect", "MuzzleJet", "MuzzleParticlePivot", "MuzzleFume"];

/// One particle of a jet (`MuzzleJet.Particle`): its place along the jet,
/// its size, its shift across (two ways) and along.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JetParticle {
    pub position: f32,
    pub size: f32,
    pub shift: [f32; 2],
    pub axis_shift: f32,
}

/// A jet (`MuzzleJet`): its object, its particles, its bounds
/// (`JetBounds`) and its chance.
#[derive(Debug, Clone, PartialEq)]
pub struct Jet {
    pub node: usize,
    pub particles: Vec<JetParticle>,
    pub bounds: [f32; 3],
    pub chance: f32,
}

/// The jets' material (`MuzzleManager.JetMaterial`, shader
/// `Custom/Additive`): its picture and figures.
#[derive(Debug, Clone)]
pub struct JetMaterial {
    pub texture: TextureData,
    pub tint: [f32; 4],
    pub inv_window: f32,
    pub randomize_phase: f32,
    pub hdr: f32,
    pub z_shift: f32,
    /// Its render queue (`m_CustomRenderQueue`; −1 the shader's own): the
    /// engine draws a lower queue first.
    pub render_queue: i64,
}

/// A gun smoke's trail (`MuzzleSmoke`): its object and figures.
#[derive(Debug, Clone)]
pub struct SmokeData {
    pub node: usize,
    pub smoke_end: f32,
    pub brake_distance: f32,
    pub drag: f32,
    pub gravity: f32,
    pub smoke_velocity: f32,
    pub turbulence_density: f32,
    pub turbulence_intensity: f32,
    pub diffusion_by_velocity: f32,
    pub speed_multiplier: f32,
    pub speed_turbulence_density: Curve,
    pub speed_turbulence_strength: Curve,
    pub speed_smoke_strength: Curve,
    pub speed_start_diffusion: Curve,
    pub smoke: Curve,
    pub smoke_strength: f32,
    pub smoke_length: f32,
    pub smoke_length_randomness: f32,
    pub increasing_by_shot: f32,
    pub drop_time: f32,
    pub material: SmokeMaterial,
}

/// A smoke's material (shader `Custom/MuzzleSmoke`).
#[derive(Debug, Clone)]
pub struct SmokeMaterial {
    pub texture: TextureData,
    pub width: f32,
    pub uv_y_scale: f32,
    pub diffusion_strength: f32,
    pub start_fade: f32,
    pub end_fade: f32,
    pub end: f32,
    pub inv_fade: f32,
    pub alpha: f32,
}

/// What a weapon's muzzle flash is drawn with.
#[derive(Debug, Clone)]
pub struct MuzzleData {
    /// How long a shot's flash runs (`ShotLength`), and its two curves over
    /// that time (`MoveCurve`, `JetLightCurve`).
    pub shot_length: f32,
    pub move_curve: Curve,
    pub light_curve: Curve,
    /// The picture's cells across and down (`AtlasXCount`, `AtlasYCount`).
    pub atlas: [u32; 2],
    /// The object the jets' mesh hangs from (`MeshParentName`, the root
    /// where there is none of that name).
    pub mesh_parent: usize,
    pub material: JetMaterial,
    pub jets: Vec<Jet>,
    /// Its gun smokes (`MuzzleSmoke`s on the same objects).
    pub smokes: Vec<SmokeData>,
    /// Its fumes (`MuzzleFume`s on the same objects, `extractor_smoke`'s
    /// among them).
    pub fumes: Vec<FumeData>,
    /// Its sparks (`MuzzleSparks` on the same objects).
    pub sparks: Vec<SparksData>,
    /// Its heat haze (`HeatHazeEmitter`s on the same objects).
    pub hazes: Vec<HazeData>,
    /// Its particle pivots (`MuzzleParticlePivot`s on the same objects):
    /// each one's object and kind (`_pivot`: 0 the front, 1 the back), where
    /// the shared blast of that kind plays at a shot (firing spec, "A
    /// launcher's blasts").
    pub pivots: Vec<(usize, i64)>,
}

/// A muzzle's heat haze (`HeatHazeEmitter`, firing spec, "The muzzle's heat
/// haze"): its object, how many puffs, their lifetimes and sizes (each
/// between two), how fast and how straight they go, the delay after the
/// shot and the chance of a shot having it.
#[derive(Debug, Clone)]
pub struct HazeData {
    pub node: usize,
    pub lifetime: [f32; 2],
    pub size: [f32; 2],
    pub count: i32,
    pub velocity_factor: f32,
    pub angle: f32,
    pub delay: f32,
    pub chance: f32,
}

/// A muzzle's sparks (`MuzzleSparks`, firing spec, "The muzzle's
/// sparks"): its object, where they begin, how many a shot, and the tables
/// of 128 their figures are drawn from (stored with it: `Values`).
#[derive(Debug, Clone)]
pub struct SparksData {
    pub node: usize,
    pub start_pos: f32,
    pub emitter_radius: f32,
    pub conus_size: f32,
    pub count_min: i32,
    pub count_range: i32,
    pub lifetime: Vec<f32>,
    pub speed: Vec<f32>,
    pub gravity: Vec<f32>,
    /// Already as the game keeps it (`LimitDrag`).
    pub drag: Vec<f32>,
    pub emission: Vec<u8>,
    pub size: Vec<u8>,
    pub turbulence: Vec<u8>,
    pub frequency: Vec<u8>,
    /// Added to the time each is born at.
    pub emission_time_shift: f32,
}

/// A muzzle's fume (`MuzzleFume`, firing spec, "The muzzle's fumes"): its
/// object, where its puffs begin, how many a shot, and their speeds, sizes,
/// lifetimes and colours over a draw.
#[derive(Debug, Clone)]
pub struct FumeData {
    pub node: usize,
    pub start_pos: f32,
    pub emitter_radius: f32,
    pub conus_size: f32,
    pub sizes: Curve,
    pub speeds: Curve,
    pub lifetimes: Curve,
    pub sizes_rnd: f32,
    pub size: f32,
    pub speed: f32,
    pub lifetime: f32,
    pub color: crate::particles::Gradient,
    pub count_min: i32,
    pub count_range: i32,
}

/// The muzzle effects' objects (`UpdateJetsAndFumes`): of those with a
/// `MuzzleEffect`, the deepest under the root by their count of parents,
/// and any named `extractor_smoke`.
pub fn effect_objects(hierarchy: &Hierarchy) -> Vec<usize> {
    let depth = |mut node: usize| {
        let mut depth = 0;
        while let Some(parent) = hierarchy.nodes[node].parent {
            (depth, node) = (depth + 1, parent);
        }
        depth
    };
    let effects: Vec<usize> = (0..hierarchy.nodes.len()).filter(|&i| hierarchy.nodes[i].scripts.iter().any(|script| MUZZLE_EFFECTS.contains(&script.as_str()))).collect();
    let smoke = |node: &usize| hierarchy.nodes[*node].name == "extractor_smoke";
    let deepest = effects.iter().filter(|node| !smoke(node)).map(|&node| depth(node)).max();
    let mut chosen: Vec<usize> = effects.iter().copied().filter(|node| !smoke(node) && Some(depth(*node)) == deepest).collect();
    chosen.extend(effects.iter().copied().filter(smoke));
    chosen
}

/// The scripts of a class on an object, read.
fn scripts_of(assets: &Assets, hierarchy: &Hierarchy, node: usize, class: &str) -> Result<Vec<Value>> {
    let n = &hierarchy.nodes[node];
    n.scripts.iter().zip(&n.script_objects).filter(|(name, _)| name.as_str() == class).map(|(_, &obj)| assets.read(obj)).collect()
}

fn float(v: &Value, name: &str) -> Result<f32> {
    v.req(name)?.as_f32().with_context(|| format!("{name} is not a number"))
}

/// Reads a weapon's muzzle flash: its `MuzzleManager` from the weapon's
/// own bundle's files (on its container, above the weapon's object), the
/// rest from its put-together tree; none where it has no manager or no
/// jets' material.
pub fn read(assets: &Assets, files: &[usize], hierarchy: &Hierarchy) -> Result<Option<MuzzleData>> {
    let Some(manager_obj) = files.iter().flat_map(|&file| assets.objects_of(file, crate::unity::class::MONO_BEHAVIOUR)).find(|&obj| crate::unity::extract::script_class(assets, obj).as_deref() == Some("MuzzleManager")) else {
        return Ok(None);
    };
    let manager = assets.read(manager_obj)?;
    let Some(material_obj) = manager.req("JetMaterial")?.as_pptr().and_then(|p| assets.resolve_loaded(manager_obj.file, p)) else {
        return Ok(None);
    };
    let material = read_material(assets, material_obj)?;
    let mesh_name = manager.get("MeshParentName").and_then(Value::as_str).unwrap_or("weapon");
    let mesh_parent = hierarchy.find_below(0, mesh_name).unwrap_or(0);
    let mut jets = Vec::new();
    let mut smokes = Vec::new();
    let mut fumes = Vec::new();
    let mut sparks = Vec::new();
    let mut hazes = Vec::new();
    let mut pivots = Vec::new();
    for node in effect_objects(hierarchy) {
        for h in scripts_of(assets, hierarchy, node, "HeatHazeEmitter").unwrap_or_default() {
            let pair = |name: &str| -> Option<[f32; 2]> {
                let v = h.get(name)?;
                Some([v.get("x")?.as_f32()?, v.get("y")?.as_f32()?])
            };
            let number = |name: &str| h.get(name).and_then(Value::as_f32);
            let (Some(lifetime), Some(size), Some(count), Some(velocity_factor), Some(angle), Some(delay), Some(chance)) =
                (pair("LifetimeDelta"), pair("Size"), h.get("Count").and_then(Value::as_i64), number("VelocityFactor"), number("Angle"), number("DelayInSec"), number("Chance"))
            else {
                continue;
            };
            hazes.push(HazeData { node, lifetime, size, count: count as i32, velocity_factor, angle, delay, chance });
        }
        for p in scripts_of(assets, hierarchy, node, "MuzzleParticlePivot").unwrap_or_default() {
            pivots.push((node, p.get("_pivot").and_then(Value::as_i64).unwrap_or(0)));
        }
        // A sparks' component that cannot be read is left out, not the
        // weapon (its tables are kept with it, 128 each).
        for s in scripts_of(assets, hierarchy, node, "MuzzleSparks").unwrap_or_default() {
            let floats = |name: &str| -> Result<Vec<f32>> {
                let values = s.req(name)?.req("Values")?;
                let all = match values.as_floats() {
                    Some(f) => f.to_vec(),
                    None => values.as_array().unwrap_or(&[]).iter().filter_map(Value::as_f32).collect(),
                };
                anyhow::ensure!(!all.is_empty(), "{name} has no values");
                Ok(all)
            };
            let bytes = |name: &str| -> Result<Vec<u8>> {
                let values = s.req(name)?.req("Values")?;
                let all = match values.as_bytes() {
                    Some(b) => b.to_vec(),
                    None => values.as_array().unwrap_or(&[]).iter().filter_map(|v| v.as_i64().map(|n| n as u8)).collect(),
                };
                anyhow::ensure!(!all.is_empty(), "{name} has no values");
                Ok(all)
            };
            let whole = |name: &str| s.req(name)?.as_i64().with_context(|| format!("{name} is not a number")).map(|n| n as i32);
            let read = || -> Result<SparksData> {
                Ok(SparksData {
                    node,
                    start_pos: float(&s, "StartPos")?,
                    emitter_radius: float(&s, "EmitterRadius")?,
                    conus_size: float(&s, "ConusSize")?,
                    count_min: whole("CountMin")?,
                    count_range: whole("CountRange")?,
                    lifetime: floats("LifetimeCurve")?,
                    speed: floats("SpeedCurve")?,
                    gravity: floats("GravityCurve")?,
                    drag: floats("DragCurve")?,
                    emission: bytes("EmissionCurve")?,
                    size: bytes("SizeCurve")?,
                    turbulence: bytes("TurbulenceAmplitudeCurve")?,
                    frequency: bytes("TurbulenceFrequencyCurve")?,
                    emission_time_shift: float(&s, "EmissionTimeShift")?,
                })
            };
            if let Ok(read) = read() {
                sparks.push(read);
            }
        }
        let n = &hierarchy.nodes[node];
        for (_, &obj) in n.scripts.iter().zip(&n.script_objects).filter(|(name, _)| name.as_str() == "MuzzleSmoke") {
            smokes.push(read_smoke(assets, obj, node)?);
        }
        for fume in scripts_of(assets, hierarchy, node, "MuzzleFume")? {
            let curve = |name: &str| crate::player::curve(fume.req(name)?);
            let whole = |name: &str| fume.req(name)?.as_i64().with_context(|| format!("{name} is not a number")).map(|n| n as i32);
            fumes.push(FumeData {
                node,
                start_pos: float(&fume, "StartPos")?,
                emitter_radius: float(&fume, "EmitterRadius")?,
                conus_size: float(&fume, "ConusSize")?,
                sizes: curve("Sizes")?,
                speeds: curve("Speeds")?,
                lifetimes: curve("LifeTimes")?,
                sizes_rnd: float(&fume, "SizesRnd")?,
                size: float(&fume, "Size")?,
                speed: float(&fume, "Speed")?,
                lifetime: float(&fume, "LifeTime")?,
                color: crate::particles::Gradient::read(fume.req("Color")?)?,
                count_min: whole("CountMin")?,
                count_range: whole("CountRange")?,
            });
        }
        for jet in scripts_of(assets, hierarchy, node, "MuzzleJet")? {
            let particles = jet
                .req("Particles")?
                .as_array()
                .unwrap_or(&[])
                .iter()
                .map(|p| -> Result<JetParticle> {
                    let shift = p.req("RandomShift")?;
                    Ok(JetParticle { position: float(p, "Position")?, size: float(p, "Size")?, shift: [float(shift, "x")?, float(shift, "y")?], axis_shift: float(p, "AxisShift")? })
                })
                .collect::<Result<Vec<_>>>()?;
            let bounds = jet.req("JetBounds")?.as_vec3().context("JetBounds")?;
            jets.push(Jet { node, particles, bounds, chance: float(&jet, "Chance")? });
        }
    }
    Ok(Some(MuzzleData {
        shot_length: float(&manager, "ShotLength")?,
        move_curve: crate::player::curve(manager.req("MoveCurve")?)?,
        light_curve: crate::player::curve(manager.req("JetLightCurve")?)?,
        atlas: [manager.req("AtlasXCount")?.as_i64().unwrap_or(1).max(1) as u32, manager.req("AtlasYCount")?.as_i64().unwrap_or(1).max(1) as u32],
        mesh_parent,
        material,
        jets,
        smokes,
        fumes,
        sparks,
        hazes,
        pivots,
    }))
}

/// A `MuzzleSmoke`'s figures and its material.
fn read_smoke(assets: &Assets, obj: ObjRef, node: usize) -> Result<SmokeData> {
    let s = assets.read(obj)?;
    let curve = |name: &str| crate::player::curve(s.req(name)?);
    let material_obj = s.req("Material")?.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p)).context("the smoke has no material")?;
    let material = assets.read(material_obj)?;
    let props = material.req("m_SavedProperties")?;
    let floats = props.get("m_Floats").and_then(Value::as_array).map(<[Value]>::to_vec).unwrap_or_default();
    let number = |name: &str| floats.iter().find(|pair| pair.get("first").and_then(Value::as_str) == Some(name)).and_then(|pair| pair.get("second")?.as_f32()).with_context(|| format!("the smoke's material has no {name}"));
    let texture = props
        .get("m_TexEnvs")
        .and_then(Value::as_array)
        .and_then(|envs| envs.iter().find(|pair| pair.get("first").and_then(Value::as_str) == Some("_MainTex")))
        .and_then(|pair| pair.get("second")?.get("m_Texture")?.as_pptr())
        .and_then(|p| assets.resolve_loaded(material_obj.file, p))
        .context("the smoke's material has no picture")?;
    Ok(SmokeData {
        node,
        smoke_end: float(&s, "SmokeEnd")?,
        brake_distance: float(&s, "BrakeDistance")?,
        drag: float(&s, "DragValue")?,
        gravity: float(&s, "Gravity")?,
        smoke_velocity: float(&s, "SmokeVelocity")?,
        turbulence_density: float(&s, "TurbulenceDensity")?,
        turbulence_intensity: float(&s, "TurbulenceIntensity")?,
        diffusion_by_velocity: float(&s, "SmokeDiffusionBySmokeVelocity")?,
        speed_multiplier: float(&s, "MuzzleSpeedMultiplier")?,
        speed_turbulence_density: curve("SpeedTurbulenceDensity")?,
        speed_turbulence_strength: curve("SpeedTurbulenceStrength")?,
        speed_smoke_strength: curve("SpeedSmokeStrength")?,
        speed_start_diffusion: curve("SpeedStartDiffusion")?,
        smoke: curve("Smoke")?,
        smoke_strength: float(&s, "SmokeStrength")?,
        smoke_length: float(&s, "SmokeLength")?,
        smoke_length_randomness: float(&s, "SmokeLengthRandomness")?,
        increasing_by_shot: float(&s, "SmokeIncreasingByShot")?,
        drop_time: float(&s, "ShotFactorDropTime")?,
        material: SmokeMaterial {
            texture: read_texture(assets, texture)?,
            width: number("_Width")?,
            uv_y_scale: number("_UVYScale")?,
            diffusion_strength: number("_DiffusionStrength")?,
            start_fade: number("_StartFade")?,
            end_fade: number("_EndFade")?,
            end: number("_End")?,
            inv_fade: number("_InvFade")?,
            alpha: number("_Alpha")?,
        },
    })
}

/// The jets' material: its picture (`_MainTex`) and its figures.
fn read_material(assets: &Assets, obj: ObjRef) -> Result<JetMaterial> {
    let material = assets.read(obj)?;
    let props = material.req("m_SavedProperties")?;
    let pairs = |name: &str| props.get(name).and_then(Value::as_array).map(<[Value]>::to_vec).unwrap_or_default();
    let floats = pairs("m_Floats");
    let number = |name: &str| floats.iter().find(|pair| pair.get("first").and_then(Value::as_str) == Some(name)).and_then(|pair| pair.get("second")?.as_f32()).with_context(|| format!("the jets' material has no {name}"));
    let colors = pairs("m_Colors");
    let tint = colors
        .iter()
        .find(|pair| pair.get("first").and_then(Value::as_str) == Some("_TintColor"))
        .and_then(|pair| {
            let c = pair.get("second")?;
            Some([c.get("r")?.as_f32()?, c.get("g")?.as_f32()?, c.get("b")?.as_f32()?, c.get("a")?.as_f32()?])
        })
        .context("the jets' material has no _TintColor")?;
    let texture = pairs("m_TexEnvs")
        .iter()
        .find(|pair| pair.get("first").and_then(Value::as_str) == Some("_MainTex"))
        .and_then(|pair| pair.get("second")?.get("m_Texture")?.as_pptr())
        .and_then(|p| assets.resolve_loaded(obj.file, p))
        .context("the jets' material has no picture")?;
    Ok(JetMaterial {
        texture: read_texture(assets, texture)?,
        tint,
        inv_window: number("_InvWindowSize")?,
        randomize_phase: number("_RandomizePhase")?,
        hdr: number("_HDR")?,
        z_shift: number("_ZShift")?,
        render_queue: material.get("m_CustomRenderQueue").and_then(Value::as_i64).unwrap_or(-1),
    })
}
