//! A sight's ranges, zoom ring, reticle, lens nodes and lens shader.

use super::*;

/// A scope's camera data's factor on the levels of detail's bias, where it
/// turns its camera's controller on (`CameraLodBiasController`).
pub(super) fn lod_bias_factor(data: &Value) -> Option<f32> {
    let on = data.get("CameraLodBiasController").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0)))?;
    on.then(|| data.get("LodBiasFactor").and_then(Value::as_f32)).flatten()
}

/// What a part's `ScopePrefabCache` turns by the zeroing distance, where its
/// `CanChangeAngleByDistance` is on and it names its object.
pub(super) fn angle_by_range(assets: &Assets, hierarchy: &Hierarchy, root: usize, cache: ObjRef) -> Option<AngleByRange> {
    let value = assets.read(cache).ok()?;
    if !value.get("CanChangeAngleByDistance").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    let transform = value.get("WeaponScopeAxis").and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(cache.file, p))?;
    let axis = part_nodes(hierarchy, root).into_iter().find(|&i| hierarchy.nodes[i].transform == transform)?;
    let table = value
        .get("AngleByRange")
        .and_then(Value::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(|entry| Some((entry.get("Distance")?.as_f32()?, entry.get("Angle")?.as_f32()?)))
        .collect();
    Some(AngleByRange { axis, table })
}

/// What a part's `ScopeZoomHandler` turns, where it names its ring and both
/// pivots and they hang from one object (the Nightforce NXS's names none).
pub(super) fn zoom_ring(assets: &Assets, hierarchy: &Hierarchy, root: usize) -> Option<ZoomRing> {
    let n = &hierarchy.nodes[root];
    let obj = n.scripts.iter().zip(&n.script_objects).find(|(name, _)| name.as_str() == "ScopeZoomHandler").map(|(_, &obj)| obj)?;
    let value = assets.read(obj).ok()?;
    let own = part_nodes(hierarchy, root);
    let find = |name: &str| -> Option<usize> {
        let transform = value.get(name).and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(obj.file, p))?;
        own.iter().copied().find(|&i| hierarchy.nodes[i].transform == transform)
    };
    let (ring, start, end) = (find("ScopeSwitcher")?, find("StartPivot")?, find("EndPivot")?);
    let parent = hierarchy.nodes[ring].parent;
    if hierarchy.nodes[start].parent != parent || hierarchy.nodes[end].parent != parent {
        return None;
    }
    let inverse = value.get("InverseRotation").and_then(Value::as_bool).unwrap_or(false);
    Some(ZoomRing { node: ring, start: hierarchy.nodes[start].rotation, end: hierarchy.nodes[end].rotation, inverse })
}

/// A `ScopeReticle` and what it names.
pub(super) fn read_reticle(assets: &Assets, obj: ObjRef) -> Result<Option<ScopeReticle>> {
    let value = assets.read(obj)?;
    let resolve = |v: Option<&Value>| v.and_then(Value::as_pptr).filter(|p| !p.is_null()).and_then(|p| assets.resolve_loaded(obj.file, p));
    let Some(mesh_obj) = resolve(value.get("Mesh")) else { return Ok(None) };
    let mesh = crate::unity::extract::read_mesh(assets, mesh_obj)?;
    let color = resolve(value.get("Material")).and_then(|m| crate::unity::extract::read_material(assets, m).ok()).and_then(|m| m.colors.get("_Color").copied()).unwrap_or([1.0; 4]);
    let vec3 = |name: &str| value.get(name).and_then(Value::as_vec3).unwrap_or([0.0; 3]);
    Ok(Some(ScopeReticle {
        colors: if mesh.colors.len() == mesh.positions.len() { mesh.colors.clone() } else { vec![[1.0; 4]; mesh.positions.len()] },
        triangles: mesh.submeshes.first().cloned().unwrap_or_default(),
        shape: mesh.shape.clone(),
        positions: mesh.positions,
        position: vec3("Position"),
        rotation: vec3("Rotation"),
        scale: value.get("Scale").and_then(Value::as_f32).unwrap_or(1.0),
        color,
    }))
}

/// A part's own nodes: its root and all below it, depth first, but not into
/// another part's root.
pub(super) fn own_nodes(hierarchy: &Hierarchy, root: usize, others: &[usize]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        out.push(at);
        let kids: Vec<usize> = (0..hierarchy.nodes.len()).filter(|&i| hierarchy.nodes[i].parent == Some(at) && !others.contains(&i)).collect();
        stack.extend(kids.into_iter().rev());
    }
    out
}

/// What a material's saved properties say it is drawn by: the shaders'
/// own properties, each kept by one of the four (`PROVISIONAL`: by its
/// properties, not by its shader's name, which is in the 378 MB shaders
/// bundle the loader opens only when it must; a material keeps the
/// properties of a shader it once had, so a lens's are looked for first).
pub fn lens_shader(data: &crate::unity::extract::MaterialData) -> Option<&'static str> {
    if data.textures.contains_key("_MaskTex2") && data.colors.contains_key("_Scales") && data.colors.contains_key("_Shifts") {
        Some("CW FX/OpticSight")
    } else if data.colors.contains_key("_MarkShift") && data.floats.contains_key("_HDR") {
        Some("CW FX/Collimator")
    } else if data.floats.contains_key("_SwitchToSightMultiplier") {
        Some("Custom/OpticGlass")
    } else if data.colors.contains_key("_Offset") && !data.textures.contains_key("_MainTex") {
        Some("CW FX/BackLens")
    } else if ["_GlobalReflectionStrength", "_FresPow", "_FresAdd", "_Specular"].iter().all(|name| data.floats.contains_key(*name)) && !data.textures.contains_key("_MainTex") {
        // A red dot's window: the reflective glass's figures and no picture
        // of its own (the game's `Glass` shader keeps the same with a `_MainTex`).
        Some("EFT/CollimatorGlass")
    } else {
        None
    }
}

/// The nodes among `nodes` drawn by the sights' own shaders (`lens_shader`),
/// with what their material and mesh give.
pub(super) fn lens_nodes(assets: &Assets, hierarchy: &Hierarchy, nodes: &[usize]) -> Vec<LensNode> {
    let mut out = Vec::new();
    let slots = nodes.iter().filter_map(|&node| Some((node, hierarchy.nodes[node].renderer.as_ref()?))).flat_map(|(node, renderer)| renderer.materials.iter().enumerate().filter_map(move |(slot, m)| Some((node, renderer, slot, (*m)?))));
    for (node, renderer, slot, material) in slots {
        let Ok(data) = crate::unity::extract::read_material(assets, material) else { continue };
        let Some(name) = lens_shader(&data) else { continue };
        let color = |name: &str| data.colors.get(name).copied().unwrap_or([0.0; 4]);
        let float = |name: &str, or: f32| data.floats.get(name).copied().unwrap_or(or);
        let kind = match name {
            "CW FX/OpticSight" => {
                // The two rings' radii from the mesh: the rim is the
                // farthest vertex, the inner ring the farthest whose uv lies
                // within the rim's.
                let Ok(mesh) = crate::unity::extract::read_mesh(assets, renderer.mesh) else { continue };
                let radius = |p: &[f32; 3]| (p[0] * p[0] + p[2] * p[2]).sqrt();
                let outer = mesh.positions.iter().map(radius).fold(0.0f32, f32::max);
                let inner = mesh.positions.iter().zip(&mesh.uv0).filter(|(_, uv)| ((uv[0] - 0.5).powi(2) + (uv[1] - 0.5).powi(2)).sqrt() < 0.49).map(|(p, _)| radius(p)).fold(0.0f32, f32::max);
                let (scales, shifts, way) = (color("_Scales"), color("_Shifts"), color("_ShiftDirection"));
                LensKind::Optic(OpticLens {
                    scales: [scales[0], scales[1]],
                    shifts: [shifts[0], shifts[1]],
                    shift_direction: [way[0], way[1], way[2]],
                    normal_hideness: float("_NormalHideness", 1.0),
                    mark_lightness: float("_MarkLightness", 0.0),
                    inner_radius: inner,
                    outer_radius: outer,
                })
            }
            "CW FX/Collimator" => {
                let shift = color("_MarkShift");
                LensKind::Collimator(CollimatorLens { color: data.colors.get("_Color").copied().unwrap_or([1.0; 4]), hdr: float("_HDR", 1.0), mark_scale: float("_MarkScale", 1.0), mark_shift: [shift[0], shift[1], shift[2]] })
            }
            "Custom/OpticGlass" | "EFT/CollimatorGlass" => LensKind::Glass,
            "CW FX/BackLens" => LensKind::Back,
            _ => continue,
        };
        let pictures = ["_MarkTex", "_MaskTex", "_MaskTex2", "_NoiseTex", "_FadeTex"]
            .into_iter()
            .filter_map(|name| {
                let texture = crate::unity::extract::read_texture(assets, *data.textures.get(name)?).ok()?;
                Some((name.to_owned(), std::sync::Arc::new(texture)))
            })
            .collect();
        out.push(LensNode { node, slot, material, kind, pictures });
    }
    out
}

/// A node and every node below it, depth first in file order.
pub(super) fn part_nodes(hierarchy: &Hierarchy, root: usize) -> Vec<usize> {
    let mut out = vec![root];
    let mut at = 0;
    while at < out.len() {
        let here = out[at];
        let kids: Vec<usize> = (0..hierarchy.nodes.len()).filter(|&i| hierarchy.nodes[i].parent == Some(here)).collect();
        // Depth first: the children go right after their parent.
        for (k, kid) in kids.into_iter().enumerate() {
            out.insert(at + 1 + k, kid);
        }
        at += 1;
    }
    out
}
