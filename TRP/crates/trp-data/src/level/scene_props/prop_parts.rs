//! A prop's renderers, box colliders, levels of detail and materials.

use super::*;

/// What the app goes by of a mesh renderer (the engine's order of its
/// fields, as `level_light` reads it): whether it is on, its materials,
/// and its run of a joined mesh's parts.
pub(super) struct Renderer {
    pub(super) enabled: bool,
    /// It casts a shadow and draws nothing (`m_CastShadows` 3,
    /// `ShadowCastingMode.ShadowsOnly`).
    pub(super) shadow_only: bool,
    pub(super) materials: Vec<(i32, i64)>,
    pub(super) batch_first: u16,
    pub(super) batch_count: u16,
}

impl Renderer {
    pub(super) fn read(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        // Its object; then on, cast shadows, receive shadows, dynamic
        // occlusion, static shadow caster, motion vectors, light probe
        // usage, reflection probe usage, ray tracing mode, ray trace
        // procedural: ten bytes, padded; the rendering layer mask, renderer
        // priority, lightmap index and its dynamic one, the lightmap's
        // scale and offset twice.
        r.skip(12)?;
        let enabled = r.u8()? != 0;
        let shadow_only = r.u8()? == 3;
        r.skip(10)?;
        r.skip(4 + 4 + 2 + 2 + 16 + 16)?;
        let count = r.i32()?;
        anyhow::ensure!((0..256).contains(&count), "{count} materials");
        let mut materials = Vec::with_capacity(count as usize);
        for _ in 0..count {
            materials.push((r.i32()?, r.i64()?));
        }
        let batch_first = r.u16()?;
        let batch_count = r.u16()?;
        Ok(Self { enabled, shadow_only, materials, batch_first, batch_count })
    }
}

/// The engine's class number of a group of levels of detail, and of a box
/// collider.
pub(super) const LOD_GROUP: i32 = 205;

pub(super) const BOX_COLLIDER: i32 = 65;

/// A box collider that is on (the engine's order of its fields: its
/// object, its physics material, the layers it includes and excludes,
/// their priority, whether it is a trigger and whether it gives contacts,
/// padded, whether it is on, padded; its size and its middle): its middle,
/// size and whether it is a trigger.
pub(super) fn box_collider(bytes: &[u8]) -> Option<([f32; 3], [f32; 3], bool)> {
    let mut r = Reader::new(bytes);
    r.skip(12 + 12 + 4 + 4 + 4).ok()?;
    let trigger = r.u8().ok()? != 0;
    r.skip(3).ok()?;
    let enabled = r.u8().ok()? != 0;
    r.skip(3).ok()?;
    let size = [r.f32().ok()?, r.f32().ok()?, r.f32().ok()?];
    let centre = [r.f32().ok()?, r.f32().ok()?, r.f32().ok()?];
    enabled.then_some((centre, size, trigger))
}

/// A group of levels of detail as a scene keeps it (the engine's order of
/// its fields: its object, its reference point, its size, its fade mode,
/// two flags padded, then each level's height on the screen, fade width and
/// renderers), nearest level first; its renderers by their files and
/// numbers in the scene (file 0 its own), its object left out (the scene
/// object it is on places it).
pub(super) fn lod_group(bytes: &[u8]) -> Result<crate::drawing::lod::LodGroup> {
    let mut r = Reader::new(bytes);
    r.skip(12)?;
    let reference = [r.f32()?, r.f32()?, r.f32()?];
    let size = r.f32()?;
    let fade_mode = r.i32()? as i64;
    let animate_cross_fading = r.u8()? != 0;
    r.skip(3)?;
    let count = r.i32()?;
    anyhow::ensure!((0..16).contains(&count), "{count} levels");
    let mut levels = Vec::new();
    for _ in 0..count {
        let (height, fade_width) = (r.f32()?, r.f32()?);
        let many = r.i32()?;
        anyhow::ensure!((0..256).contains(&many), "{many} renderers");
        let mut renderers = Vec::new();
        for _ in 0..many {
            let (file, path_id) = (r.i32()?, r.i64()?);
            renderers.push(crate::unity::ObjRef { file: file as usize, path_id });
        }
        levels.push(crate::drawing::lod::LodLevel { height, fade_width, renderers });
    }
    Ok(crate::drawing::lod::LodGroup { id: None, object: None, size, reference, fade_mode, animate_cross_fading, levels, deviation: Vec::new() })
}

/// One part of a mesh as a mesh of its own, with only the corners it uses.
pub(super) fn part_of(data: &MeshData, indices: &[u32]) -> MeshData {
    let mut remap: HashMap<u32, u32> = HashMap::new();
    let mut used: Vec<usize> = Vec::new();
    let mut out = Vec::with_capacity(indices.len());
    for &i in indices {
        if i as usize >= data.positions.len() {
            continue;
        }
        let next = used.len() as u32;
        let at = *remap.entry(i).or_insert_with(|| {
            used.push(i as usize);
            next
        });
        out.push(at);
    }
    // A triangle cut short by a corner past the end is dropped whole.
    out.truncate(out.len() / 3 * 3);
    let take = |list: &[_]| if list.len() == data.positions.len() { used.iter().map(|&i| list[i]).collect() } else { Vec::new() };
    MeshData {
        name: data.name.clone(),
        positions: take(&data.positions),
        normals: take(&data.normals),
        tangents: if data.tangents.len() == data.positions.len() { used.iter().map(|&i| data.tangents[i]).collect() } else { Vec::new() },
        uv0: if data.uv0.len() == data.positions.len() { used.iter().map(|&i| data.uv0[i]).collect() } else { Vec::new() },
        weights: Vec::new(),
        joints: Vec::new(),
        submeshes: vec![out],
        bind_poses: Vec::new(),
        ..MeshData::default()
    }
}

/// A mesh in the world's axes brought into the axes of the object read
/// (at `root`, turned so).
pub(super) fn to_root(mesh: &mut MeshData, (at, turn): ([f32; 3], [f32; 4])) {
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    for p in &mut mesh.positions {
        *p = crate::level::level_light::rotate(back, [p[0] - at[0], p[1] - at[1], p[2] - at[2]]);
    }
    for n in &mut mesh.normals {
        *n = crate::level::level_light::rotate(back, *n);
    }
    for t in &mut mesh.tangents {
        let v = crate::level::level_light::rotate(back, [t[0], t[1], t[2]]);
        *t = [v[0], v[1], v[2], t[3]];
    }
}

/// The picture a material names for one of its textures (its file's
/// number among the material's file's and its own), if it names one.
pub(super) fn texture_pointer(material: &Value, name: &str) -> Option<(i32, i64)> {
    let envs = material.get("m_SavedProperties")?.get("m_TexEnvs")?.as_array()?;
    let pair = envs.iter().find(|pair| pair.get("first").and_then(Value::as_str) == Some(name))?;
    let pointer = pair.get("second")?.get("m_Texture")?.as_pptr()?;
    (pointer.path_id != 0).then_some((pointer.file_id, pointer.path_id))
}

pub(super) fn material_of(value: &Value, base: Option<usize>, normal: Option<usize>, gloss: Option<usize>) -> PropMaterial {
    let properties = value.get("m_SavedProperties");
    let list = |name: &str| properties.and_then(|p| p.get(name)).and_then(Value::as_array).unwrap_or(&[]);
    let floats = list("m_Floats").iter().filter_map(|pair| Some((pair.get("first")?.as_str()?.to_owned(), pair.get("second")?.as_f32()?))).collect();
    let color = list("m_Colors")
        .iter()
        .find(|pair| pair.get("first").and_then(Value::as_str) == Some("_Color"))
        .and_then(|pair| pair.get("second"))
        .and_then(|c| Some([c.get("r")?.as_f32()?, c.get("g")?.as_f32()?, c.get("b")?.as_f32()?, c.get("a")?.as_f32()?]))
        .unwrap_or([1.0; 4]);
    let keywords = value.get("m_ValidKeywords").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(|k| k.as_str().map(str::to_owned)).collect();
    PropMaterial {
        name: value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
        base,
        normal,
        gloss,
        color,
        queue: value.get("m_CustomRenderQueue").and_then(Value::as_i64).unwrap_or(-1) as i32,
        keywords,
        floats,
    }
}
