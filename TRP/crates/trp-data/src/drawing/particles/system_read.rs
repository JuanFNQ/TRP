//! Reading a particle system, its renderer, material and built-in meshes.

use super::*;

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
pub(super) fn read_light(assets: &Assets, obj: ObjRef) -> Result<EffectLight> {
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

pub(super) fn vec3(v: &Value, name: &str) -> Result<[f32; 3]> {
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
        culling_mode: s.get("cullingMode").and_then(Value::as_i64).unwrap_or(0),
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
                    quality: m.get("quality").and_then(Value::as_i64).unwrap_or(0).clamp(0, 2),
                    voxel_size: m.get("voxelSize").and_then(Value::as_f32).unwrap_or(0.5).max(1e-4),
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
    Ok(Some(std::sync::Arc::new(builtin_mesh(assets, obj.file, p).context("the particle renderer's mesh")?)))
}

/// One of Unity's own meshes (the built-in sphere, 10207; the plane,
/// 10209) as an object of `file` names it: read from the install's copy of
/// the engine's resources by the tree of a mesh borrowed from that file,
/// or, where it keeps none (a prefab whose meshes are all in other
/// bundles: the menu player's), from any file loaded.
pub(crate) fn builtin_mesh(assets: &Assets, file: usize, p: crate::unity::PPtr) -> Result<MeshData> {
    let own = assets.file(file);
    let external = usize::try_from(p.file_id).ok().and_then(|n| n.checked_sub(1)).and_then(|n| own.meta.externals.get(n)).context("the mesh is in no file its object names")?;
    anyhow::ensure!(external == DEFAULT_RESOURCES.0, "the mesh is in {external}, which is not read");
    let tree_of = |file: &crate::unity::assets::AssetFile| file.meta.types.iter().find(|ty| ty.class_id == class::MESH && !ty.tree.is_empty()).map(|ty| ty.tree.clone());
    let tree = tree_of(own).or_else(|| assets.files().iter().find_map(tree_of)).context("no file loaded has a mesh's tree to read Unity's own by")?;
    let v = crate::interface::screen::build_object(&assets.install, DEFAULT_RESOURCES.1, p.path_id, class::MESH, &tree)?;
    mesh_of(&v, None)
}
