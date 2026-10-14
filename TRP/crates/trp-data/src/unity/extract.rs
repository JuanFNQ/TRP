//! Typed views of the objects this project needs: meshes, textures,
//! materials and transform hierarchies. All values stay in Unity's
//! left-handed space; converting is the renderer's job.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, bail, Context, Result};

use super::assets::{Assets, ObjRef};
use super::class;
use super::value::Value;

// ---------------------------------------------------------------- meshes

#[derive(Debug, Clone, Default)]
pub struct MeshData {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tangents: Vec<[f32; 4]>,
    pub uv0: Vec<[f32; 2]>,
    pub weights: Vec<[f32; 4]>,
    pub joints: Vec<[u16; 4]>,
    /// One triangle list per submesh, one submesh per material slot.
    pub submeshes: Vec<Vec<u32>>,
    /// Inverse bind matrices, one per bone, each as rows `e00..e33`.
    pub bind_poses: Vec<[f32; 16]>,
}

const CHANNEL_POSITION: usize = 0;
const CHANNEL_NORMAL: usize = 1;
const CHANNEL_TANGENT: usize = 2;
const CHANNEL_UV0: usize = 4;
const CHANNEL_BLEND_WEIGHT: usize = 12;
const CHANNEL_BLEND_INDICES: usize = 13;

struct Channel {
    stream: usize,
    offset: usize,
    format: u8,
    dimension: usize,
}

fn format_size(format: u8) -> Result<usize> {
    Ok(match format {
        0 | 10 | 11 => 4,
        1 | 4 | 5 | 8 | 9 => 2,
        2 | 3 | 6 | 7 => 1,
        other => bail!("unknown vertex format {other}"),
    })
}

fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) as u32) << 31;
    let exp = ((h >> 10) & 0x1f) as u32;
    let mant = (h & 0x3ff) as u32;
    let bits = match exp {
        0 if mant == 0 => sign,
        0 => {
            let mut m = mant;
            let mut e = 113u32;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | (e << 23) | ((m & 0x3ff) << 13)
        }
        31 => sign | 0x7f80_0000 | (mant << 13),
        _ => sign | ((exp + 112) << 23) | (mant << 13),
    };
    f32::from_bits(bits)
}

fn component_f32(format: u8, b: &[u8]) -> f32 {
    match format {
        0 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        1 => half_to_f32(u16::from_le_bytes([b[0], b[1]])),
        2 => b[0] as f32 / 255.0,
        3 => (b[0] as i8 as f32 / 127.0).max(-1.0),
        4 => u16::from_le_bytes([b[0], b[1]]) as f32 / 65535.0,
        5 => (i16::from_le_bytes([b[0], b[1]]) as f32 / 32767.0).max(-1.0),
        6 => b[0] as f32,
        7 => b[0] as i8 as f32,
        8 => u16::from_le_bytes([b[0], b[1]]) as f32,
        9 => i16::from_le_bytes([b[0], b[1]]) as f32,
        10 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
        _ => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
    }
}

struct VertexBuffer<'a> {
    data: &'a [u8],
    count: usize,
    channels: Vec<Channel>,
    /// (offset, stride) per stream.
    streams: Vec<(usize, usize)>,
}

impl VertexBuffer<'_> {
    /// Reads channel `index` as `N` floats per vertex. Missing components are zero.
    fn read<const N: usize>(&self, index: usize) -> Result<Vec<[f32; N]>> {
        let Some(ch) = self.channels.get(index).filter(|c| c.dimension > 0) else {
            return Ok(Vec::new());
        };
        let size = format_size(ch.format)?;
        let (base, stride) = self.streams[ch.stream];
        let mut out = Vec::with_capacity(self.count);
        for v in 0..self.count {
            let start = base + v * stride + ch.offset;
            let Some(bytes) = self.data.get(start..start + size * ch.dimension) else {
                bail!("vertex {v} of channel {index} lies outside the vertex data");
            };
            let mut item = [0.0f32; N];
            for (c, slot) in item.iter_mut().enumerate().take(ch.dimension) {
                *slot = component_f32(ch.format, &bytes[c * size..]);
            }
            out.push(item);
        }
        Ok(out)
    }
}

pub fn read_mesh(assets: &Assets, obj: ObjRef) -> Result<MeshData> {
    let v = assets.read(obj)?;
    let name = v.req("m_Name")?.as_str().unwrap_or("").to_owned();
    if v.at("m_MeshCompression").and_then(Value::as_i64).unwrap_or(0) != 0 {
        bail!("mesh {name:?} uses packed compression, which is not supported");
    }

    let vd = v.req("m_VertexData")?;
    let count = vd.req("m_VertexCount")?.as_usize().unwrap_or(0);
    let mut channels = Vec::new();
    for c in vd.req("m_Channels")?.as_array().unwrap_or(&[]) {
        channels.push(Channel {
            stream: c.req("stream")?.as_usize().unwrap_or(0),
            offset: c.req("offset")?.as_usize().unwrap_or(0),
            format: c.req("format")?.as_i64().unwrap_or(0) as u8,
            // The upper bits of the byte carry unrelated flags.
            dimension: (c.req("dimension")?.as_i64().unwrap_or(0) & 0xf) as usize,
        });
    }
    // Streams are stored one after another, each starting on a 16-byte boundary.
    let stream_count = channels.iter().filter(|c| c.dimension > 0).map(|c| c.stream + 1).max().unwrap_or(0);
    let mut streams = Vec::with_capacity(stream_count);
    let mut offset = 0usize;
    for s in 0..stream_count {
        let mut stride = 0;
        for c in channels.iter().filter(|c| c.stream == s && c.dimension > 0) {
            stride += format_size(c.format)? * c.dimension;
        }
        streams.push((offset, stride));
        offset = (offset + count * stride + 15) & !15;
    }

    let inline = vd.req("m_DataSize")?.as_bytes().unwrap_or(&[]);
    let data: &[u8] = if !inline.is_empty() {
        inline
    } else {
        let stream = v.req("m_StreamData")?;
        let size = stream.req("size")?.as_usize().unwrap_or(0);
        if size == 0 {
            &[]
        } else {
            assets.resource(
                stream.req("path")?.as_str().unwrap_or(""),
                stream.req("offset")?.as_usize().unwrap_or(0),
                size,
            )?
        }
    };
    let vb = VertexBuffer { data, count, channels, streams };

    let index_bytes = v.req("m_IndexBuffer")?.as_bytes().unwrap_or(&[]);
    let wide = v.req("m_IndexFormat")?.as_i64().unwrap_or(0) == 1;
    let index_size = if wide { 4 } else { 2 };
    let mut submeshes = Vec::new();
    for sm in v.req("m_SubMeshes")?.as_array().unwrap_or(&[]) {
        let first = sm.req("firstByte")?.as_usize().unwrap_or(0);
        let index_count = sm.req("indexCount")?.as_usize().unwrap_or(0);
        let base_vertex = sm.req("baseVertex")?.as_i64().unwrap_or(0) as u32;
        let is_triangles = sm.req("topology")?.as_i64().unwrap_or(0) == 0;
        let mut indices = Vec::new();
        if is_triangles {
            let Some(bytes) = index_bytes.get(first..first + index_count * index_size) else {
                bail!("submesh of {name:?} runs past the index buffer");
            };
            indices.reserve(index_count);
            for chunk in bytes.chunks_exact(index_size) {
                let i = if wide {
                    u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                } else {
                    u16::from_le_bytes([chunk[0], chunk[1]]) as u32
                };
                indices.push(i + base_vertex);
            }
        }
        submeshes.push(indices);
    }

    let mut bind_poses = Vec::new();
    for m in v.req("m_BindPose")?.as_array().unwrap_or(&[]) {
        let mut rows = [0.0f32; 16];
        for (i, slot) in rows.iter_mut().enumerate() {
            let field = format!("e{}{}", i / 4, i % 4);
            *slot = m.req(&field)?.as_f32().unwrap_or(0.0);
        }
        bind_poses.push(rows);
    }

    let joints: Vec<[f32; 4]> = vb.read(CHANNEL_BLEND_INDICES)?;
    Ok(MeshData {
        positions: vb.read(CHANNEL_POSITION)?,
        normals: vb.read(CHANNEL_NORMAL)?,
        tangents: vb.read(CHANNEL_TANGENT)?,
        uv0: vb.read(CHANNEL_UV0)?,
        weights: vb.read(CHANNEL_BLEND_WEIGHT)?,
        joints: joints.iter().map(|j| j.map(|c| c as u16)).collect(),
        submeshes,
        bind_poses,
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats() {
        assert_eq!(half_to_f32(0x3c00), 1.0);
        assert_eq!(half_to_f32(0xc000), -2.0);
        assert_eq!(half_to_f32(0x0000), 0.0);
        assert_eq!(half_to_f32(0x0001), 2.0f32.powi(-24));
        assert_eq!(half_to_f32(0x7c00), f32::INFINITY);
    }

    fn node(name: &str, parent: Option<usize>, rotation: [f32; 4]) -> Node {
        Node {
            name: name.to_owned(),
            parent,
            translation: [0.0; 3],
            rotation,
            scale: [1.0; 3],
            active: true,
            renderer: None,
            scripts: Vec::new(),
            grip: None,
            placer: None,
            transform: ObjRef { file: 0, path_id: 0 },
        }
    }

    #[test]
    fn hierarchy_paths_search_and_attach() {
        let quarter_x = [-0.70710677, 0.0, 0.0, 0.70710677];
        let identity = [0.0, 0.0, 0.0, 1.0];
        let mut h = Hierarchy {
            nodes: vec![
                node("root", None, identity),
                node("Weapon_root", Some(0), quarter_x),
                node("weapon", Some(1), identity),
                node("mod_stock", Some(2), identity),
            ],
        };
        assert_eq!(h.path(0), "");
        assert_eq!(h.path(3), "Weapon_root/weapon/mod_stock");
        assert_eq!(h.rotation_from_root(3), quarter_x);

        // A part with a slot of the same name: the search prefers the nearer one.
        let part = Hierarchy { nodes: vec![node("stock", None, identity), node("mod_stock", Some(0), identity)] };
        let part_root = h.attach(3, part);
        assert_eq!(h.nodes[part_root].parent, Some(3));
        assert_eq!(h.find_below(0, "mod_stock"), Some(3));
        assert_eq!(h.find_below(part_root, "mod_stock"), Some(part_root + 1));
        assert_eq!(h.find_below(0, "missing"), None);
    }
}

// -------------------------------------------------------------- textures

#[derive(Debug, Clone)]
pub struct TextureData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Unity `TextureFormat` value.
    pub format: i32,
    pub mip_count: u32,
    pub srgb: bool,
    /// All mip levels, largest first, rows bottom to top.
    pub data: Vec<u8>,
}

pub fn read_texture(assets: &Assets, obj: ObjRef) -> Result<TextureData> {
    let v = assets.read(obj)?;
    let inline = v.req("image data")?.as_bytes().unwrap_or(&[]);
    let data = if !inline.is_empty() {
        inline.to_vec()
    } else {
        let stream = v.req("m_StreamData")?;
        assets
            .resource(
                stream.req("path")?.as_str().unwrap_or(""),
                stream.req("offset")?.as_usize().unwrap_or(0),
                stream.req("size")?.as_usize().unwrap_or(0),
            )?
            .to_vec()
    };
    Ok(TextureData {
        name: v.req("m_Name")?.as_str().unwrap_or("").to_owned(),
        width: v.req("m_Width")?.as_u32().unwrap_or(0),
        height: v.req("m_Height")?.as_u32().unwrap_or(0),
        format: v.req("m_TextureFormat")?.as_i64().unwrap_or(0) as i32,
        mip_count: v.req("m_MipCount")?.as_u32().unwrap_or(1).max(1),
        srgb: v.req("m_ColorSpace")?.as_i64().unwrap_or(0) == 1,
        data,
    })
}

// ------------------------------------------------------------- materials

#[derive(Debug, Clone, Default)]
pub struct MaterialData {
    pub name: String,
    pub textures: HashMap<String, ObjRef>,
    pub floats: HashMap<String, f32>,
    pub colors: HashMap<String, [f32; 4]>,
    pub keywords: Vec<String>,
}

pub fn read_material(assets: &Assets, obj: ObjRef) -> Result<MaterialData> {
    let v = assets.read(obj)?;
    let mut out = MaterialData {
        name: v.req("m_Name")?.as_str().unwrap_or("").to_owned(),
        ..Default::default()
    };
    for k in v.at("m_ValidKeywords").and_then(Value::as_array).unwrap_or(&[]) {
        out.keywords.extend(k.as_str().map(str::to_owned));
    }
    let props = v.req("m_SavedProperties")?;
    for pair in props.req("m_TexEnvs")?.as_array().unwrap_or(&[]) {
        let name = pair.req("first")?.as_str().unwrap_or("");
        let ptr = pair.req("second.m_Texture")?.as_pptr().context("texture reference")?;
        if let Some(tex) = assets.resolve_loaded(obj.file, ptr) {
            out.textures.insert(name.to_owned(), tex);
        }
    }
    for pair in props.req("m_Floats")?.as_array().unwrap_or(&[]) {
        if let (Some(name), Some(value)) = (pair.req("first")?.as_str(), pair.req("second")?.as_f32()) {
            out.floats.insert(name.to_owned(), value);
        }
    }
    for pair in props.req("m_Colors")?.as_array().unwrap_or(&[]) {
        let c = pair.req("second")?;
        let rgba = ["r", "g", "b", "a"].map(|k| c.get(k).and_then(Value::as_f32).unwrap_or(0.0));
        out.colors.insert(pair.req("first")?.as_str().unwrap_or("").to_owned(), rgba);
    }
    Ok(out)
}

// ----------------------------------------------------------- hierarchies

#[derive(Debug, Clone)]
pub struct Renderer {
    pub mesh: ObjRef,
    pub materials: Vec<Option<ObjRef>>,
    /// For skinned meshes: node index of each bone, in bind pose order.
    pub bones: Vec<Option<usize>>,
    pub skinned: bool,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub active: bool,
    pub renderer: Option<Renderer>,
    /// Class names of the scripts on this object.
    pub scripts: Vec<String>,
    /// Set on a part's marker for where a hand holds it.
    pub grip: Option<Grip>,
    /// Set on a part that says for itself where it goes on its bone.
    pub placer: Option<Placer>,
    pub transform: ObjRef,
}

/// Where a part goes on the bone it is put on, when that is not the
/// game's usual place (`ModPlacer`): its place, its turn (degrees about
/// each axis, as `Quaternion.Euler` takes them) and its size on the bone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placer {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

/// A part's marker for a hand: the palm goes to the marker's own place and
/// turn, and the fingers take the stored pose.
#[derive(Debug, Clone)]
pub struct Grip {
    pub left_hand: bool,
    /// 0 the ordinary grip, 1 the alternative one, 2 for an underbarrel weapon.
    pub kind: i32,
    /// Local rotations (`xyzw`) for the palm and every bone below it, in
    /// the order of the hand's own bones, palm first.
    pub fingers: Vec<[f32; 4]>,
}

/// A GameObject tree, parents before children. Node 0 is the root.
#[derive(Debug, Clone, Default)]
pub struct Hierarchy {
    pub nodes: Vec<Node>,
}

impl Hierarchy {
    /// Path of a node from the root, in the form animation bindings use.
    pub fn path(&self, mut index: usize) -> String {
        let mut parts = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            parts.push(self.nodes[index].name.as_str());
            index = parent;
        }
        parts.reverse();
        parts.join("/")
    }

    /// Rest-pose rotation of a node relative to the root node, as `xyzw`.
    pub fn rotation_from_root(&self, mut index: usize) -> [f32; 4] {
        let mut chain = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            chain.push(self.nodes[index].rotation);
            index = parent;
        }
        chain.iter().rev().fold([0.0, 0.0, 0.0, 1.0], |acc, q| quat_mul(acc, *q))
    }

    /// First node called `name` at or below `start`, nearest first.
    pub fn find_below(&self, start: usize, name: &str) -> Option<usize> {
        let mut depth_of = vec![usize::MAX; self.nodes.len()];
        depth_of[start] = 0;
        let mut best: Option<(usize, usize)> = None;
        for i in start..self.nodes.len() {
            if i != start {
                let Some(parent) = self.nodes[i].parent else { continue };
                if depth_of[parent] == usize::MAX {
                    continue;
                }
                depth_of[i] = depth_of[parent] + 1;
            }
            if self.nodes[i].name == name && best.map_or(true, |(d, _)| depth_of[i] < d) {
                best = Some((depth_of[i], i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Appends another hierarchy under `parent` and returns the index of its root.
    pub fn attach(&mut self, parent: usize, other: Hierarchy) -> usize {
        let base = self.nodes.len();
        for mut node in other.nodes {
            node.parent = Some(node.parent.map_or(parent, |p| p + base));
            if let Some(r) = &mut node.renderer {
                for bone in r.bones.iter_mut().flatten() {
                    *bone += base;
                }
            }
            self.nodes.push(node);
        }
        base
    }
}

/// Hamilton product of two `xyzw` quaternions: `b` applied first, then `a`.
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let ([ax, ay, az, aw], [bx, by, bz, bw]) = (a, b);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// Reads the tree below a GameObject. Renderers of lower detail levels are
/// dropped, so only the most detailed version of each part remains.
pub fn read_hierarchy(assets: &Assets, game_object: ObjRef) -> Result<Hierarchy> {
    let go = assets.read(game_object)?;
    let root_transform = components(assets, game_object, &go)
        .into_iter()
        .find(|c| assets.class_id(*c) == Some(class::TRANSFORM))
        .ok_or_else(|| anyhow!("game object has no transform"))?;

    let mut nodes = Vec::new();
    let mut node_of_transform: HashMap<ObjRef, usize> = HashMap::new();
    let mut pending_bones: Vec<(usize, Vec<Option<ObjRef>>)> = Vec::new();
    let mut hidden: HashSet<ObjRef> = HashSet::new();
    let mut renderer_objects: Vec<(usize, ObjRef)> = Vec::new();
    let mut stack = vec![(root_transform, None::<usize>)];

    while let Some((transform, parent)) = stack.pop() {
        let t = assets.read(transform)?;
        let go_ref = t
            .req("m_GameObject")?
            .as_pptr()
            .and_then(|p| assets.resolve_loaded(transform.file, p))
            .ok_or_else(|| anyhow!("transform without a game object"))?;
        let go = assets.read(go_ref)?;
        let index = nodes.len();
        node_of_transform.insert(transform, index);

        let mut mesh = None;
        let mut renderer = None;
        let mut scripts = Vec::new();
        let mut grip = None;
        let mut placer = None;
        for comp in components(assets, go_ref, &go) {
            match assets.class_id(comp) {
                Some(class::MESH_FILTER) => {
                    let c = assets.read(comp)?;
                    mesh = c.req("m_Mesh")?.as_pptr().and_then(|p| assets.resolve_loaded(comp.file, p));
                }
                Some(class::MESH_RENDERER) => {
                    let c = assets.read(comp)?;
                    if c.req("m_Enabled")?.as_bool().unwrap_or(true) {
                        renderer = Some((comp, materials(assets, comp, &c)?, None));
                    }
                }
                Some(class::SKINNED_MESH_RENDERER) => {
                    let c = assets.read(comp)?;
                    if c.req("m_Enabled")?.as_bool().unwrap_or(true) {
                        let skinned_mesh =
                            c.req("m_Mesh")?.as_pptr().and_then(|p| assets.resolve_loaded(comp.file, p));
                        let bones = c
                            .req("m_Bones")?
                            .as_array()
                            .unwrap_or(&[])
                            .iter()
                            .map(|b| b.as_pptr().and_then(|p| assets.resolve_loaded(comp.file, p)))
                            .collect();
                        pending_bones.push((index, bones));
                        renderer = Some((comp, materials(assets, comp, &c)?, skinned_mesh));
                    }
                }
                Some(class::LOD_GROUP) => {
                    let c = assets.read(comp)?;
                    let lods = c.req("m_LODs")?.as_array().unwrap_or(&[]);
                    let level = |lod: &Value| -> Vec<ObjRef> {
                        lod.at("renderers")
                            .and_then(Value::as_array)
                            .unwrap_or(&[])
                            .iter()
                            .filter_map(|r| r.get("renderer")?.as_pptr())
                            .filter_map(|p| assets.resolve_loaded(comp.file, p))
                            .collect()
                    };
                    let kept: HashSet<ObjRef> = lods.first().map(level).unwrap_or_default().into_iter().collect();
                    for lod in lods.iter().skip(1) {
                        hidden.extend(level(lod).into_iter().filter(|r| !kept.contains(r)));
                    }
                }
                Some(class::MONO_BEHAVIOUR) => {
                    let class_name = script_class(assets, comp);
                    if class_name.as_deref() == Some("GripPose") {
                        let c = assets.read(comp)?;
                        grip = Some(Grip {
                            left_hand: c.req("Hand")?.as_i64() == Some(0),
                            kind: c.req("GripType")?.as_i64().unwrap_or(0) as i32,
                            fingers: c
                                .req("Fingers")?
                                .as_array()
                                .unwrap_or(&[])
                                .iter()
                                .filter_map(Value::as_vec4)
                                .collect(),
                        });
                    }
                    if class_name.as_deref() == Some("ModPlacer") {
                        let c = assets.read(comp)?;
                        placer = Some(Placer {
                            position: c.req("ModPosition")?.as_vec3().context("ModPosition")?,
                            rotation: c.req("ModRotation")?.as_vec3().context("ModRotation")?,
                            scale: c.req("ModScale")?.as_vec3().context("ModScale")?,
                        });
                    }
                    scripts.extend(class_name);
                }
                _ => {}
            }
        }
        let renderer = renderer.and_then(|(comp, materials, skinned_mesh)| {
            let skinned = skinned_mesh.is_some();
            let mesh = skinned_mesh.or(mesh)?;
            renderer_objects.push((index, comp));
            Some(Renderer { mesh, materials, bones: Vec::new(), skinned })
        });

        nodes.push(Node {
            name: go.req("m_Name")?.as_str().unwrap_or("").to_owned(),
            parent,
            translation: t.req("m_LocalPosition")?.as_vec3().context("position")?,
            rotation: t.req("m_LocalRotation")?.as_vec4().context("rotation")?,
            scale: t.req("m_LocalScale")?.as_vec3().context("scale")?,
            active: go.req("m_IsActive")?.as_bool().unwrap_or(true),
            renderer,
            scripts,
            grip,
            placer,
            transform,
        });
        // Reversed so that children come off the stack in file order.
        for child in t.req("m_Children")?.as_array().unwrap_or(&[]).iter().rev() {
            if let Some(obj) = child.as_pptr().and_then(|p| assets.resolve_loaded(transform.file, p)) {
                stack.push((obj, Some(index)));
            }
        }
    }

    for (index, comp) in renderer_objects {
        if hidden.contains(&comp) {
            nodes[index].renderer = None;
        }
    }
    for (index, bones) in pending_bones {
        if let Some(r) = &mut nodes[index].renderer {
            r.bones = bones.iter().map(|b| b.and_then(|t| node_of_transform.get(&t).copied())).collect();
        }
    }
    Ok(Hierarchy { nodes })
}

fn components(assets: &Assets, owner: ObjRef, go: &Value) -> Vec<ObjRef> {
    go.get("m_Component")
        .and_then(Value::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(|c| c.get("component")?.as_pptr())
        .filter_map(|p| assets.resolve_loaded(owner.file, p))
        .collect()
}

fn materials(assets: &Assets, comp: ObjRef, renderer: &Value) -> Result<Vec<Option<ObjRef>>> {
    Ok(renderer
        .req("m_Materials")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .map(|m| m.as_pptr().and_then(|p| assets.resolve_loaded(comp.file, p)))
        .collect())
}

/// Class name of the script a MonoBehaviour instantiates.
pub fn script_class(assets: &Assets, behaviour: ObjRef) -> Option<String> {
    let v = assets.read(behaviour).ok()?;
    let script = assets.resolve_loaded(behaviour.file, v.get("m_Script")?.as_pptr()?)?;
    let script = assets.read(script).ok()?;
    script.get("m_ClassName")?.as_str().map(str::to_owned)
}

/// The prefab roots a bundle exports, read from its asset table.
pub fn exported_game_objects(assets: &Assets, file: usize) -> Result<Vec<(String, ObjRef)>> {
    let mut out = Vec::new();
    for bundle in assets.objects_of(file, class::ASSET_BUNDLE) {
        let v = assets.read(bundle)?;
        for pair in v.req("m_Container")?.as_array().unwrap_or(&[]) {
            let path = pair.req("first")?.as_str().unwrap_or("").to_owned();
            let Some(obj) = pair.req("second.asset")?.as_pptr().and_then(|p| assets.resolve_loaded(file, p))
            else {
                continue;
            };
            if assets.class_id(obj) == Some(class::GAME_OBJECT) {
                out.push((path, obj));
            }
        }
    }
    Ok(out)
}
