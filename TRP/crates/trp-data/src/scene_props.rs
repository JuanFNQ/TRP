//! Things laid in a level's scene, to be drawn elsewhere: an object of a
//! scene and all under it that is on, with what each draws (its meshes,
//! their materials and those materials' pictures), read from the build's
//! own files beside the scene. Those keep no type trees: a mesh and a
//! material are read by the trees of the same classes in a bundle of the
//! install's, which the same engine wrote (`screen::tree_of`), a picture by
//! the scene pictures' reader (`screen::ScenePictures`). The first is the
//! hideout's workbench (the user, 2026-10-20: "Let's add a workbench using
//! the workbench model"; workbench spec).
//!
//! **Static batching.** A level's meshes that never move are joined by the
//! engine as the build is made into one mesh of many parts, their corners
//! already where they lie in the world (`StaticBatchUtility`); a renderer
//! so joined names the joined mesh and its own run of parts
//! (`m_StaticBatchInfo`). Its parts are taken out of the joined mesh here,
//! each with only the corners it uses, and brought into the axes of the
//! object read (`Props::root`): drawn there, not on their own objects.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::install::Install;
use crate::screen::{scene_paths, tree_of, Scene, ScenePictures, MESH_FILTER};
use crate::unity::extract::{mesh_streamed, MeshData, TextureData};
use crate::unity::reader::Reader;
use crate::unity::{class, Value};

/// The bundle the mesh's and the material's trees are borrowed from: the
/// effects', which has both.
const TREES: &str = crate::sound::EFFECTS;

/// An object read and all under it that is on.
#[derive(Debug, Default)]
pub struct Props {
    /// Every object, the one read first, each after its parent; its own
    /// place, turn and scale under its parent (the first's: none).
    pub nodes: Vec<PropNode>,
    pub meshes: Vec<MeshData>,
    pub materials: Vec<PropMaterial>,
    pub pictures: Vec<TextureData>,
    /// Where the object read lies in its scene and how it is turned (its
    /// scale taken as one).
    pub root: ([f32; 3], [f32; 4]),
    /// What could not be read, with why.
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PropNode {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// What it draws: a mesh (one part of a mesh: one material) and its
    /// material, by their places in `Props`.
    pub drawn: Vec<(usize, usize)>,
    /// Its meshes are in the axes of the object read, not its own (taken
    /// out of a statically batched mesh).
    pub in_root: bool,
    /// Its box colliders that are on, in its own axes: each one's middle
    /// and size (`BoxCollider.m_Center`, `m_Size`), and whether it is a
    /// trigger.
    pub boxes: Vec<([f32; 3], [f32; 3], bool)>,
}

/// A material as the app draws the game's: its colour picture, its normal
/// map and its gloss (`_MainTex`, `_BumpMap`, `_SpecMap`, by their places
/// in `Props::pictures`), its tint (`_Color`), the queue it is drawn in
/// (`m_CustomRenderQueue`: −1 its shader's), its keywords and its floats.
#[derive(Debug, Clone, Default)]
pub struct PropMaterial {
    pub name: String,
    pub base: Option<usize>,
    pub normal: Option<usize>,
    pub gloss: Option<usize>,
    pub color: [f32; 4],
    pub queue: i32,
    pub keywords: Vec<String>,
    pub floats: HashMap<String, f32>,
}

/// Reads the object at `path` (its names from the scene's top, joined by
/// `/`) of the scene whose path in the build ends with `scene`, and all
/// under it that is on, skipping an object with a script of a class in
/// `skip`, or whose name begins with one of `left_out` (and all under it);
/// with `keep` named, only those objects (their paths under the one read)
/// and all under them, and the objects on the way to them.
pub fn read(install: &Install, scene: &str, path: &str, skip: &[&str], left_out: &[&str], keep: &[&str]) -> Result<Props> {
    let index = scene_paths(install)?.iter().position(|p| p.ends_with(scene)).with_context(|| format!("no scene {scene} in the build"))?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true).with_context(|| file.clone())?;
    let top = (0..scene.nodes.len()).find(|&node| scene.path(node) == path).with_context(|| format!("no object {path} in {file}"))?;
    let mesh_tree = tree_of(install, TREES, class::MESH).context("a mesh's tree")?;
    let material_tree = tree_of(install, TREES, class::MATERIAL).context("a material's tree")?;
    let mut pictures = ScenePictures::new(install)?;
    let folder = install.root.join("EscapeFromTarkov_Data");
    let stream = |name: &str, path: &str, offset: usize, size: usize| -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let beside = folder.join(path.rsplit('/').next().unwrap_or(path));
        let mut file = std::fs::File::open(&beside).with_context(|| format!("{name}: {}", beside.display()))?;
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut data = vec![0; size];
        file.read_exact(&mut data).with_context(|| format!("{name}: {}", beside.display()))?;
        Ok(data)
    };
    let mut props = Props { root: (scene.world_position(top), scene.world_rotation(top)), ..Default::default() };
    // Levels of detail: the renderers of every level but the nearest, under
    // any group below the object read, are not drawn (the range sees the
    // bench from near; `PROVISIONAL`: the game shows a farther level by the
    // bench's height on the screen).
    let mut farther: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for node in (0..scene.nodes.len()).filter(|&node| scene.path(node).starts_with(path)) {
        if let Some(bytes) = scene.component(node, LOD_GROUP) {
            match lod_renderers(bytes) {
                Ok(levels) => farther.extend(levels.into_iter().skip(1).flatten().filter(|pointer| pointer.0 == 0).map(|pointer| pointer.1)),
                Err(e) => props.skipped.push(format!("{}'s levels of detail: {e:#}", scene.path(node))),
            }
        }
    }
    // Meshes read whole by their file and number (a joined mesh is many
    // objects' own), materials and pictures by theirs.
    let mut whole: HashMap<(String, i64), MeshData> = HashMap::new();
    let mut materials: HashMap<(String, i64), usize> = HashMap::new();
    let mut pictures_read: HashMap<(String, i64), Option<usize>> = HashMap::new();
    let file_of = |pointer: (i32, i64)| -> Result<String> {
        if pointer.0 == 0 {
            return Ok(file.clone());
        }
        let listed = scene.externals.get(pointer.0 as usize - 1).with_context(|| format!("no file {} among {file}'s", pointer.0))?;
        Ok(listed.rsplit('/').next().unwrap_or(listed).to_owned())
    };
    // What is kept, where only some is: each kept object's path under the
    // one read, and the paths on the way to it.
    let top_path = scene.path(top);
    let kept = |node: usize| -> bool {
        if keep.is_empty() || node == top {
            return true;
        }
        let under = scene.path(node);
        let under = under.strip_prefix(&top_path).map(|rest| rest.trim_start_matches('/')).unwrap_or("");
        keep.iter().any(|wanted| under == *wanted || under.starts_with(&format!("{wanted}/")) || wanted.starts_with(&format!("{under}/")))
    };
    let mut stack = vec![(top, None)];
    while let Some((node, parent)) = stack.pop() {
        let own = &scene.nodes[node];
        if !own.active || !kept(node) || skip.iter().any(|class| scene.behaviour(node, class).is_some()) || left_out.iter().any(|name| own.name.starts_with(name)) {
            continue;
        }
        let local = scene.local(node).unwrap_or(crate::screen::Local { rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3], scale: [1.0; 3] });
        let at = props.nodes.len();
        let (translation, rotation) = if parent.is_none() { ([0.0; 3], [0.0, 0.0, 0.0, 1.0]) } else { (local.position, local.rotation) };
        let boxes = scene.component(node, BOX_COLLIDER).and_then(box_collider).into_iter().collect();
        props.nodes.push(PropNode { name: own.name.clone(), parent, translation, rotation, scale: if parent.is_none() { [1.0; 3] } else { local.scale }, drawn: Vec::new(), in_root: false, boxes });
        for &child in own.children.iter().rev() {
            stack.push((child, Some(at)));
        }
        let (Some(filter), Some(renderer)) = (scene.component(node, MESH_FILTER), scene.component(node, class::MESH_RENDERER)) else { continue };
        let what = scene.path(node);
        let made = (|| -> Result<()> {
            let renderer = Renderer::read(renderer).context("its renderer")?;
            // Off, one that casts only a shadow, one of a farther level of
            // detail: not drawn.
            let farther_level = scene.component_id(node, class::MESH_RENDERER).is_some_and(|id| farther.contains(&id));
            if !renderer.enabled || renderer.shadow_only || farther_level {
                return Ok(());
            }
            let mut r = Reader::new(filter.get(12..).context("a short mesh filter")?);
            let mesh = (r.i32()?, r.i64()?);
            let mesh_key = (file_of(mesh)?, mesh.1);
            if !whole.contains_key(&mesh_key) {
                let value = pictures.read_by(&mesh_key.0, mesh_key.1, class::MESH, &mesh_tree)?;
                whole.insert(mesh_key.clone(), mesh_streamed(&value, &stream)?);
            }
            let data = &whole[&mesh_key];
            // A joined mesh's run of parts, in the world's axes; else the
            // mesh's own parts.
            let joined = renderer.batch_count > 0;
            let parts: Vec<usize> = if joined { (renderer.batch_first as usize..(renderer.batch_first + renderer.batch_count) as usize).collect() } else { (0..data.submeshes.len()).collect() };
            for (n, part) in parts.into_iter().enumerate() {
                let Some(material) = renderer.materials.get(n).or(renderer.materials.last()).copied() else { continue };
                let Some(indices) = data.submeshes.get(part) else { bail!("part {part} of a mesh of {}", data.submeshes.len()) };
                let mut mesh = part_of(data, indices);
                if joined {
                    to_root(&mut mesh, props.root);
                }
                let material_key = (file_of(material)?, material.1);
                let material = match materials.get(&material_key) {
                    Some(&at) => at,
                    None => {
                        let value = pictures.read_by(&material_key.0, material_key.1, class::MATERIAL, &material_tree)?;
                        let mut picture = |name: &str| -> Option<usize> {
                            let pointer = texture_pointer(&value, name)?;
                            let picture_file = pictures.file_name(&material_key.0, pointer.0).ok()?;
                            let key = (picture_file, pointer.1);
                            if let Some(read) = pictures_read.get(&key) {
                                return *read;
                            }
                            let read = match pictures.texture(&key.0, key.1) {
                                Ok(texture) => {
                                    props.pictures.push(texture);
                                    Some(props.pictures.len() - 1)
                                }
                                Err(e) => {
                                    props.skipped.push(format!("{name} of {}: {e:#}", value.get("m_Name").and_then(Value::as_str).unwrap_or("?")));
                                    None
                                }
                            };
                            pictures_read.insert(key, read);
                            read
                        };
                        let (base, normal, gloss) = (picture("_MainTex"), picture("_BumpMap"), picture("_SpecMap"));
                        props.materials.push(material_of(&value, base, normal, gloss));
                        materials.insert(material_key, props.materials.len() - 1);
                        props.materials.len() - 1
                    }
                };
                props.meshes.push(mesh);
                props.nodes[at].drawn.push((props.meshes.len() - 1, material));
            }
            props.nodes[at].in_root = joined;
            Ok(())
        })();
        if let Err(e) = made {
            props.skipped.push(format!("{what}: {e:#}"));
        }
    }
    Ok(props)
}

impl Props {
    /// A point in an object's own axes in the axes of the object read: its
    /// place, scale and turn under each object above it.
    pub fn in_root(&self, mut node: usize, mut point: [f32; 3]) -> [f32; 3] {
        loop {
            let own = &self.nodes[node];
            let Some(parent) = own.parent else { return point };
            let scaled = [point[0] * own.scale[0], point[1] * own.scale[1], point[2] * own.scale[2]];
            let turned = crate::level_light::rotate(own.rotation, scaled);
            point = [turned[0] + own.translation[0], turned[1] + own.translation[1], turned[2] + own.translation[2]];
            node = parent;
        }
    }

    /// The box every drawn corner lies in, in the axes of the object read
    /// (lowest and highest), and those of each object that draws.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for (node, own) in self.nodes.iter().enumerate() {
            for &(mesh, _) in &own.drawn {
                for p in &self.meshes[mesh].positions {
                    let at = if own.in_root { *p } else { self.in_root(node, *p) };
                    for k in 0..3 {
                        low[k] = low[k].min(at[k]);
                        high[k] = high[k].max(at[k]);
                    }
                }
            }
        }
        (low, high)
    }
}

/// What the app goes by of a mesh renderer (the engine's order of its
/// fields, as `level_light` reads it): whether it is on, its materials,
/// and its run of a joined mesh's parts.
struct Renderer {
    enabled: bool,
    /// It casts a shadow and draws nothing (`m_CastShadows` 3,
    /// `ShadowCastingMode.ShadowsOnly`).
    shadow_only: bool,
    materials: Vec<(i32, i64)>,
    batch_first: u16,
    batch_count: u16,
}

impl Renderer {
    fn read(bytes: &[u8]) -> Result<Self> {
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
const LOD_GROUP: i32 = 205;
const BOX_COLLIDER: i32 = 65;

/// A box collider that is on (the engine's order of its fields: its
/// object, its physics material, the layers it includes and excludes,
/// their priority, whether it is a trigger and whether it gives contacts,
/// padded, whether it is on, padded; its size and its middle): its middle,
/// size and whether it is a trigger.
fn box_collider(bytes: &[u8]) -> Option<([f32; 3], [f32; 3], bool)> {
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

/// A group of levels of detail's renderers, nearest level first (the
/// engine's order of its fields: its object, its reference point, its
/// size, its fade mode, two flags, then each level's height on the screen,
/// fade width and renderers).
fn lod_renderers(bytes: &[u8]) -> Result<Vec<Vec<(i32, i64)>>> {
    let mut r = Reader::new(bytes);
    r.skip(12 + 12 + 4 + 4)?;
    r.skip(4)?;
    let levels = r.i32()?;
    anyhow::ensure!((0..16).contains(&levels), "{levels} levels");
    let mut out = Vec::new();
    for _ in 0..levels {
        r.skip(8)?;
        let count = r.i32()?;
        anyhow::ensure!((0..256).contains(&count), "{count} renderers");
        let mut renderers = Vec::new();
        for _ in 0..count {
            renderers.push((r.i32()?, r.i64()?));
        }
        out.push(renderers);
    }
    Ok(out)
}

/// One part of a mesh as a mesh of its own, with only the corners it uses.
fn part_of(data: &MeshData, indices: &[u32]) -> MeshData {
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
fn to_root(mesh: &mut MeshData, (at, turn): ([f32; 3], [f32; 4])) {
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    for p in &mut mesh.positions {
        *p = crate::level_light::rotate(back, [p[0] - at[0], p[1] - at[1], p[2] - at[2]]);
    }
    for n in &mut mesh.normals {
        *n = crate::level_light::rotate(back, *n);
    }
    for t in &mut mesh.tangents {
        let v = crate::level_light::rotate(back, [t[0], t[1], t[2]]);
        *t = [v[0], v[1], v[2], t[3]];
    }
}

/// The picture a material names for one of its textures (its file's
/// number among the material's file's and its own), if it names one.
fn texture_pointer(material: &Value, name: &str) -> Option<(i32, i64)> {
    let envs = material.get("m_SavedProperties")?.get("m_TexEnvs")?.as_array()?;
    let pair = envs.iter().find(|pair| pair.get("first").and_then(Value::as_str) == Some(name))?;
    let pointer = pair.get("second")?.get("m_Texture")?.as_pptr()?;
    (pointer.path_id != 0).then_some((pointer.file_id, pointer.path_id))
}

fn material_of(value: &Value, base: Option<usize>, normal: Option<usize>, gloss: Option<usize>) -> PropMaterial {
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
