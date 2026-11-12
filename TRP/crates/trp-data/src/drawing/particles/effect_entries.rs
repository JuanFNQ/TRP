//! The effects' pool entries, adapters, emitters, materials and shader queues.

use super::*;

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
/// it lays no deferred one), with what that one draws from the game's shared
/// tables as it lays a decal.
#[derive(Debug, Clone)]
pub struct MaterialEntry {
    pub name: String,
    pub materials: Vec<i32>,
    pub deferred_decal: bool,
    /// `UseDeferredDecals` as set (the older decal is laid where it is off
    /// or the hard settings' `DEFERRED_DECALS_ENABLED` is).
    pub use_deferred_decals: bool,
    pub old_decal: bool,
    pub old_decal_draws: Option<OldDecalDraws>,
}

impl MaterialEntry {
    /// What its older decal draws as a bullet's hit lays it (`Effect.Emit`:
    /// with a decal, not a knife's, where `UseDeferredDecals` is off or the
    /// deferred decals are not enabled), if it lays one.
    pub fn old_decal_laid(&self, deferred_enabled: bool) -> Option<OldDecalDraws> {
        if self.use_deferred_decals && deferred_enabled {
            return None;
        }
        self.old_decal_draws
    }
}

/// What the older decal system (`DecalSystem.Add`) draws from the game's
/// shared tables (`FastRndom`) for each decal it lays: a size (`Float`)
/// where its `SizeRandomRange` is at least the smallest float, then a way
/// across the normal (`VectorNormalized`, in `Decal.Calc`), then a tile
/// (the whole numbers' `Int(tiles)`) where its sheet has more than one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OldDecalDraws {
    pub size_random: bool,
    pub tiles: u32,
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
        let old_decal_draws = entry
            .get("Decal")
            .and_then(Value::as_pptr)
            .filter(|p| p.path_id != 0)
            .and_then(|p| assets.resolve_loaded(effects.file, p))
            .and_then(|decal| assets.read(decal).ok())
            .map(|d| {
                let whole = |name: &str| d.get(name).and_then(Value::as_i64).unwrap_or(1).max(1) as u32;
                OldDecalDraws { size_random: d.get("SizeRandomRange").and_then(Value::as_f32).unwrap_or(0.0) >= f32::from_bits(1), tiles: whole("TileSheetRows") * whole("TileSheetColumns") }
            });
        out.push(MaterialEntry {
            name: entry.get("Name").and_then(Value::as_str).unwrap_or("").to_owned(),
            materials,
            deferred_decal: entry.get("UseDeferredDecals").and_then(Value::as_i64).unwrap_or(0) != 0 && set("DeferredDecals"),
            use_deferred_decals: entry.get("UseDeferredDecals").and_then(Value::as_i64).unwrap_or(0) != 0,
            old_decal: set("Decal"),
            old_decal_draws,
        });
    }
    Ok(out)
}

/// Every entry's name in the effects' list, in its order: those of
/// materials and those that name none (a grenade's, a lamp's).
pub fn entry_names(assets: &mut Assets) -> Result<Vec<String>> {
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let all = assets.read(effects)?;
    Ok(all.req("EffectsArray")?.as_array().unwrap_or(&[]).iter().filter_map(|e| e.get("Name").and_then(Value::as_str).map(str::to_owned)).collect())
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
pub(super) fn shader_queue(shader: &Value) -> Option<i64> {
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

pub(super) fn float(v: &Value, name: &str) -> Result<f32> {
    v.req(name)?.as_f32().with_context(|| format!("{name} is not a number"))
}

pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
