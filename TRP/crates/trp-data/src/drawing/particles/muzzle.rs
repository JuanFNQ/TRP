//! The muzzle's fume, heat, sparks, tracers' light and blasts.

use super::*;

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

/// The shared system a hot part's shimmer is emitted into
/// (`Effects.MuzzleEffect.Heat`, object `HeatEffect`; malfunctions spec,
/// "The shimmer over a hot barrel"): drawn, as the muzzles' haze, into the
/// camera's distortion.
pub fn muzzle_heat_system(assets: &mut Assets) -> Result<ParticleSystemData> {
    muzzle_system(assets, "Heat")
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

/// The tracers' renderer (`TracersLight`, on a child of the effects'
/// `Effects`; ballistics spec, "A tracer"): its figures and curves, and its
/// mesh's material: the shader's name, the picture and `_Brightness`.
#[derive(Debug, Clone)]
pub struct TracersLightData {
    /// `_maxFlyingTime`: no tracer past it.
    pub max_flying_time: f32,
    /// `_sizeTimeCurve`, `_colorAlphaCurve`: over the share of that time.
    pub size_time: Curve,
    pub color_alpha: Curve,
    /// `_grenadeFlyingMaxTime`, `_grenadeSizeCoef`.
    pub grenade_flying_max_time: f32,
    pub grenade_size: f32,
    /// `_speedModifier`, `_grenadeSpeedModifier`: a tracer's length over its
    /// bullet's speed.
    pub speed_modifier: f32,
    pub grenade_speed_modifier: f32,
    pub shader: String,
    pub texture: TextureData,
    pub brightness: f32,
}

/// The tracers' renderer, read from the effects' bundle.
pub fn tracers_light(assets: &mut Assets) -> Result<TracersLightData> {
    assets.load("shaders")?;
    let files = assets.load(EFFECTS)?;
    let script = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("TracersLight"))
        .context("the effects' bundle has no TracersLight")?;
    let s = assets.read(script)?;
    let number = |name: &str| -> Result<f32> { s.req(name)?.as_f32().with_context(|| format!("the tracers' {name}")) };
    // Its mesh's object's renderer, and that one's material.
    let filter = s.req("_tracersMF")?.as_pptr().and_then(|p| assets.resolve_loaded(script.file, p)).context("the tracers have no mesh filter")?;
    let game_object = assets.read(filter)?.req("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(filter.file, p)).context("the tracers' mesh has no game object")?;
    let mesh_renderer = assets
        .read(game_object)?
        .req("m_Component")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .filter_map(|pair| pair.get("component")?.as_pptr())
        .filter_map(|p| assets.resolve_loaded(game_object.file, p))
        .find(|&component| assets.info(component).is_some_and(|info| info.class_id == class::MESH_RENDERER))
        .context("the tracers have no mesh renderer")?;
    let material_obj = assets
        .read(mesh_renderer)?
        .req("m_Materials")?
        .as_array()
        .and_then(|all| all.first())
        .and_then(Value::as_pptr)
        .and_then(|p| assets.resolve_loaded(mesh_renderer.file, p))
        .context("the tracers' renderer has no material")?;
    let m = read_material(assets, material_obj)?;
    let shader = assets
        .read(material_obj)?
        .req("m_Shader")?
        .as_pptr()
        .and_then(|p| assets.resolve_loaded(material_obj.file, p))
        .and_then(|shader| assets.read(shader).ok())
        .and_then(|shader| shader.get("m_ParsedForm")?.get("m_Name")?.as_str().map(str::to_owned))
        .unwrap_or_default();
    let texture = m.textures.get("_MainTex").copied().context("the tracers' material has no picture")?;
    Ok(TracersLightData {
        max_flying_time: number("_maxFlyingTime")?,
        size_time: crate::character::player::curve(s.req("_sizeTimeCurve")?)?,
        color_alpha: crate::character::player::curve(s.req("_colorAlphaCurve")?)?,
        grenade_flying_max_time: number("_grenadeFlyingMaxTime")?,
        grenade_size: number("_grenadeSizeCoef")?,
        speed_modifier: number("_speedModifier")?,
        grenade_speed_modifier: number("_grenadeSpeedModifier")?,
        shader,
        texture: read_texture(assets, texture)?,
        // The shader's own default where the material has none.
        brightness: m.floats.get("_Brightness").copied().unwrap_or(1.0),
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
