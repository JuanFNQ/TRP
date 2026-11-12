//! Reading a prefab's objects, meshes, files and trees from a build.

use super::*;

/// What reads a prefab (`Scene::load_prefab`): its file, the scene made,
/// and the files its scripts are named in.
pub(super) struct PrefabReader<'a> {
    pub(super) install: Install,
    pub(super) build: &'a mut BuildFile,
    pub(super) scene: Scene,
    pub(super) outside: HashMap<i32, Option<BuildFile>>,
}

impl PrefabReader<'_> {
    pub(super) fn object(&mut self, id: i64) -> Result<(i32, Vec<u8>)> {
        let object = *self.build.meta.object(id).with_context(|| format!("no object {id}"))?;
        Ok((object.class_id, self.build.bytes(object.offset, object.size)?))
    }

    /// A script's class: the name of the script object it points to, in
    /// one of the file's outside files.
    pub(super) fn class(&mut self, script: &Pointer) -> Option<String> {
        if script.file <= 0 {
            return None;
        }
        if !self.outside.contains_key(&script.file) {
            let name = self.build.meta.externals.get(script.file as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned());
            let opened = name.and_then(|name| BuildFile::open(&self.install, &name).ok());
            self.outside.insert(script.file, opened);
        }
        let file = self.outside.get_mut(&script.file)?.as_mut()?;
        let object = *file.meta.object(script.path).filter(|object| object.class_id == class::MONO_SCRIPT)?;
        let big_endian = file.meta.big_endian;
        let bytes = file.bytes(object.offset, object.size).ok()?;
        string(&mut Reader { data: &bytes, pos: 0, big_endian }).ok()
    }

    /// A game object, its components and its children, as a node of the
    /// scene under `parent`.
    pub(super) fn visit(&mut self, go: i64, parent: Option<usize>) -> Result<usize> {
        let big_endian = self.build.meta.big_endian;
        let (_, bytes) = self.object(go)?;
        let mut r = Reader { data: &bytes, pos: 0, big_endian };
        let count = r.i32()?.max(0) as usize;
        let components: Vec<Pointer> = (0..count).map(|_| pointer(&mut r)).collect::<Result<_>>()?;
        r.skip(4)?; // layer
        let name = string(&mut r)?;
        r.skip(2)?; // tag
        let active = r.u8()? != 0;
        let index = self.scene.nodes.len();
        self.scene.objects.insert(go, index);
        self.scene.nodes.push(Node { name, active, parent, children: Vec::new(), layout: None, scripts: Vec::new() });
        let mut children = Vec::new();
        for component in components.iter().filter(|component| component.file == 0) {
            self.scene.components.insert(component.path, index);
            let (class_id, bytes) = self.object(component.path)?;
            // Every part's class and bytes (a camera's, a light's figures),
            // as a scene file's are kept.
            self.scene.component_classes.insert(component.path, class_id);
            let at = self.scene.data.len();
            self.scene.data.extend_from_slice(&bytes);
            self.scene.spans.insert(component.path, (at, bytes.len()));
            let mut r = Reader { data: &bytes, pos: 0, big_endian };
            match class_id {
                class::TRANSFORM | RECT_TRANSFORM => {
                    pointer(&mut r)?;
                    let rotation = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
                    let turn = z_turn(rotation);
                    let position = [r.f32()?, r.f32()?, r.f32()?];
                    let whole = [r.f32()?, r.f32()?, r.f32()?];
                    let scale = [whole[0], whole[1]];
                    self.scene.locals.insert(index, Local { rotation, position, scale: whole });
                    let count = r.i32()?.max(0) as usize;
                    children = (0..count).map(|_| pointer(&mut r).map(|child| child.path)).collect::<Result<_>>()?;
                    pointer(&mut r)?; // father
                    self.scene.transforms.insert(component.path, index);
                    if class_id == RECT_TRANSFORM {
                        let (anchor_min, anchor_max, offset, size_delta, pivot) = (pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?);
                        self.scene.nodes[index].layout = Some(Layout { anchor_min, anchor_max, offset, size_delta, pivot, scale, turn });
                    }
                }
                class::MONO_BEHAVIOUR => {
                    pointer(&mut r)?;
                    let enabled = flag(&mut r)?;
                    let script = pointer(&mut r)?;
                    string(&mut r)?;
                    let start = self.scene.data.len();
                    self.scene.data.extend_from_slice(&bytes[r.pos..]);
                    self.scene.behaviours.push(Behaviour { id: component.path, node: index, enabled, fields: (start, bytes.len() - r.pos) });
                    if let Some(class) = self.class(&script) {
                        self.scene.classes.insert(component.path, class.clone());
                        self.scene.nodes[index].scripts.push(class);
                    }
                }
                _ => {}
            }
        }
        // Each child transform's own game object, in the transform's order.
        for child in children {
            let (_, bytes) = self.object(child)?;
            let owner = pointer(&mut Reader { data: &bytes, pos: 0, big_endian })?;
            let node = self.visit(owner.path, Some(index))?;
            self.scene.nodes[index].children.push(node);
        }
        Ok(index)
    }
}

/// One object of one of the build's own files by its number: its class and
/// its bytes as kept (`probe object`).
pub fn object_bytes(install: &Install, file: &str, id: i64) -> Result<(i32, Vec<u8>)> {
    let mut build = BuildFile::open(install, file)?;
    let object = *build.meta.object(id).with_context(|| format!("{file} has no object {id}"))?;
    Ok((object.class_id, build.bytes(object.offset, object.size)?))
}

/// A plain mesh of one of the build's own files (no type tree): its
/// vertices' places, in the engine's axes, and its triangles.
#[derive(Debug, Clone, Default)]
pub struct BuildMesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<u32>,
    /// Its normals and first texture coordinates, where it has them as
    /// floats (else empty).
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
}

/// The engine's class number of a mesh, and of a mesh filter.
const MESH: i32 = 43;

pub const MESH_FILTER: i32 = 33;

/// A mesh of the build's own files by its number, read by the engine's
/// order of a mesh's fields (its name, its parts each with its bounds, the
/// blend shapes, the bones, the flags, the index buffer, the vertex data):
/// a mesh without blend shapes or bones, its places of the first stream
/// and floats, as a level's sky's are; its normals and first texture
/// coordinates with them where they are floats too.
pub fn build_mesh(install: &Install, file: &str, id: i64) -> Result<BuildMesh> {
    let (class, bytes) = object_bytes(install, file, id)?;
    if class != MESH {
        bail!("{file} {id} is of class {class}, not a mesh");
    }
    let mut r = Reader::new(&bytes);
    let name = string(&mut r)?;
    // The parts: where each starts, how many indices, its topology, its
    // base and first vertex and count, its bounds (6 floats).
    let parts = r.u32()?;
    for _ in 0..parts {
        let topology = {
            r.skip(8)?;
            r.i32()?
        };
        if topology != 0 {
            bail!("{name}: a part of topology {topology}, not triangles");
        }
        r.skip(12 + 24)?;
    }
    // Blend shapes (their vertices, shapes, channels, weights), bind
    // poses, bones' names, the root bone, the bones' bounds, the variable
    // weights: all to be empty.
    for what in ["blend shape vertices", "blend shapes", "blend channels", "blend weights", "bind poses", "bone names"] {
        if r.u32()? != 0 {
            bail!("{name} has {what}");
        }
    }
    r.skip(4)?;
    for what in ["bones' bounds", "variable weights"] {
        if r.u32()? != 0 {
            bail!("{name} has {what}");
        }
    }
    let compression = r.u8()?;
    if compression != 0 {
        bail!("{name} is compressed");
    }
    r.skip(3)?;
    let wide = r.i32()? == 1;
    let size = r.u32()? as usize;
    let index_bytes = r.bytes(size)?;
    r.align(4);
    let triangles = if wide {
        index_bytes.chunks_exact(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
    } else {
        index_bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]]) as u32).collect()
    };
    let count = r.u32()? as usize;
    let channels = r.u32()? as usize;
    let mut stride = 0usize;
    let mut place = None;
    let (mut normal, mut uv) = (None, None);
    for n in 0..channels {
        let (stream, offset, format, dimension) = (r.u8()?, r.u8()?, r.u8()?, r.u8()? & 0x0f);
        if dimension == 0 {
            continue;
        }
        if stream != 0 {
            bail!("{name}: a channel in stream {stream}");
        }
        // Formats 0 float, 1 half; 1, 2, 3 bytes, 4 to 11 two or four.
        let width = match format {
            0 | 10 | 11 => 4,
            1 | 4 | 5 | 8 | 9 => 2,
            _ => 1,
        };
        stride = stride.max(offset as usize + width * dimension as usize);
        if n == 0 {
            if format != 0 || dimension != 3 {
                bail!("{name}: places of format {format} and {dimension} figures");
            }
            place = Some(offset as usize);
        }
        // The engine's channels: 1 the normals, 4 the first coordinates.
        if n == 1 && format == 0 && dimension == 3 {
            normal = Some(offset as usize);
        }
        if n == 4 && format == 0 && dimension == 2 {
            uv = Some(offset as usize);
        }
    }
    let place = place.with_context(|| format!("{name} has no places"))?;
    let size = r.u32()? as usize;
    let data = r.bytes(size)?;
    // A stream's vertices are padded to a multiple of four bytes.
    let stride = if count > 0 { data.len() / count } else { stride };
    let float = |v: usize, at: usize, k: usize| {
        let at = v * stride + at + 4 * k;
        f32::from_le_bytes(data[at..at + 4].try_into().expect("four bytes"))
    };
    let positions = (0..count).map(|v| [float(v, place, 0), float(v, place, 1), float(v, place, 2)]).collect();
    let normals = normal.map_or_else(Vec::new, |at| (0..count).map(|v| [float(v, at, 0), float(v, at, 1), float(v, at, 2)]).collect());
    let uvs = uv.map_or_else(Vec::new, |at| (0..count).map(|v| [float(v, at, 0), float(v, at, 1)]).collect());
    Ok(BuildMesh { name, positions, triangles, normals, uvs })
}

/// One of the build's own files, its table read from its head and its
/// objects by their ranges, as wanted: `resources.assets` is some 300 MB.
pub(crate) struct BuildFile {
    pub(super) file: std::fs::File,
    pub(crate) meta: SerializedFile,
}

impl BuildFile {
    pub(crate) fn open(install: &Install, name: &str) -> Result<Self> {
        use std::io::Read;
        let path = install.root.join("EscapeFromTarkov_Data").join(name);
        let mut file = std::fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let len = file.metadata()?.len() as usize;
        let mut first = [0u8; 48];
        file.read_exact(&mut first)?;
        let mut head = vec![0; SerializedFile::head_size(&first)?.min(len)];
        let mut file = std::fs::File::open(&path)?;
        file.read_exact(&mut head)?;
        let meta = SerializedFile::parse_head(&head, len).with_context(|| name.to_owned())?;
        Ok(Self { file, meta })
    }

    pub(crate) fn bytes(&mut self, offset: usize, size: usize) -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        self.file.seek(SeekFrom::Start(offset as u64))?;
        let mut bytes = vec![0; size];
        self.file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

/// One object of the build's own files (`Resources/unity default
/// resources`, which keeps no trees) read by a tree of its class borrowed
/// from a bundle.
pub fn build_object(install: &Install, name: &str, path_id: i64, class: i32, tree: &crate::unity::typetree::TypeTree) -> Result<crate::unity::Value> {
    let mut file = BuildFile::open(install, name)?;
    let object = *file.meta.object(path_id).with_context(|| format!("no object {path_id} in {name}"))?;
    anyhow::ensure!(object.class_id == class, "object {path_id} in {name} is of class {}, not {class}", object.class_id);
    let big_endian = file.meta.big_endian;
    let bytes = file.bytes(object.offset, object.size)?;
    let mut r = Reader { data: &bytes, pos: 0, big_endian };
    tree.read(&mut r).with_context(|| format!("object {path_id} of {name}"))
}

/// The tree of a class as a bundle of the install's has it, to read the
/// build's own files by (`ScenePictures::read_by`).
pub fn tree_of(install: &Install, key: &str, class: i32) -> Result<crate::unity::typetree::TypeTree> {
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(key)?;
    files
        .iter()
        .find_map(|&file| assets.file(file).meta.types.iter().find(|ty| ty.class_id == class && !ty.tree.is_empty()).map(|ty| ty.tree.clone()))
        .with_context(|| format!("{key} has no object of class {class} with its tree"))
}
