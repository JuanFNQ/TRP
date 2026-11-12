//! The deferred decals of grenades, bullets and blood.

use super::*;

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

/// The decal renderer's two decals of blood (body hits spec, "What a body's
/// hit shows"): the blood on a wall behind a body's hit (`_environmentBlood`,
/// `EmitBloodOnEnvironment`) and the drops under a bleeding character
/// (`_bleedingDecal`, `EmitBleeding`), each laid as a still thing's hole is,
/// with `_decalProjectorHeight` and `_maxDecals`.
/// With them, the time between a bleeding character's drops, drawn between
/// the two (the effects' commutator's `_minMaxBleedingSpawnDelta`, on the
/// same object as `Effects`).
#[derive(Debug, Clone)]
pub struct BloodDecals {
    pub environment: DeferredDecal,
    pub bleeding: DeferredDecal,
    pub bleeding_spawn_delta: [f32; 2],
}

pub fn deferred_blood_decals(assets: &mut Assets) -> Result<BloodDecals> {
    let (renderer, r) = decal_renderer(assets)?;
    let height = float(&r, "_decalProjectorHeight")?;
    let environment = single_decal(assets, renderer, &r, r.req("_environmentBlood")?, height).context("_environmentBlood")?;
    let bleeding = single_decal(assets, renderer, &r, r.req("_bleedingDecal")?, height).context("_bleedingDecal")?;
    let files = assets.load(EFFECTS)?;
    let commutator = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("EffectsCommutator"))
        .context("the effects' bundle has no EffectsCommutator")?;
    let c = assets.read(commutator)?;
    let delta = c.req("_minMaxBleedingSpawnDelta")?;
    Ok(BloodDecals { environment, bleeding, bleeding_spawn_delta: [float(delta, "x")?, float(delta, "y")?] })
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

pub(super) fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|c| 2.0 * c);
    let c = cross(u, t);
    [v[0] + q[3] * t[0] + c[0], v[1] + q[3] * t[1] + c[1], v[2] + q[3] * t[2] + c[2]]
}
