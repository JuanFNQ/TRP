//! A set of loaded bundles with cross-file reference resolution.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};

use super::bundle::Bundle;
use super::reader::Reader;
use super::serialized::{ObjectInfo, SerializedFile};
use super::value::{PPtr, Value};
use crate::install::{Install, Manifest};

/// Shared bundles every asset depends on. They are large and are only opened
/// when a reference actually lands in one.
const DEFERRED: &[&str] = &["shaders", "cubemaps"];

/// An object in a loaded file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjRef {
    pub file: usize,
    pub path_id: i64,
}

pub struct AssetFile {
    /// Lower-cased name of the file inside its bundle, which is how other
    /// files refer to it.
    pub name: String,
    pub bundle_key: String,
    pub meta: SerializedFile,
    bundle: Arc<Bundle>,
    range: Range<usize>,
}

impl AssetFile {
    fn bytes(&self) -> &[u8] {
        &self.bundle.data[self.range.clone()]
    }
}

pub struct Assets {
    pub install: Install,
    pub manifest: Manifest,
    files: Vec<AssetFile>,
    file_by_name: HashMap<String, usize>,
    resources: HashMap<String, (Arc<Bundle>, Range<usize>)>,
    loaded: HashSet<String>,
}

impl Assets {
    pub fn new(install: Install) -> Result<Self> {
        let manifest = install.manifest()?;
        Ok(Self {
            install,
            manifest,
            files: Vec::new(),
            file_by_name: HashMap::new(),
            resources: HashMap::new(),
            loaded: HashSet::new(),
        })
    }

    /// Loads a bundle by manifest key together with its dependencies.
    /// Returns the indices of the serialized files in the bundle itself.
    pub fn load(&mut self, key: &str) -> Result<Vec<usize>> {
        let files = self.load_one(key)?;
        let mut pending: Vec<String> = self.manifest.dependencies(key).to_vec();
        while let Some(dep) = pending.pop() {
            if self.loaded.contains(&dep) || DEFERRED.contains(&dep.as_str()) {
                continue;
            }
            self.load_one(&dep)?;
            pending.extend(self.manifest.dependencies(&dep).iter().cloned());
        }
        Ok(files)
    }

    fn load_one(&mut self, key: &str) -> Result<Vec<usize>> {
        if self.loaded.contains(key) {
            return Ok(self
                .files
                .iter()
                .enumerate()
                .filter(|(_, f)| f.bundle_key == key)
                .map(|(i, _)| i)
                .collect());
        }
        let bundle = Arc::new(Bundle::open(&self.install.bundle_path(key))?);
        self.loaded.insert(key.to_owned());
        let mut indices = Vec::new();
        for node in &bundle.nodes {
            let name = node.path.to_ascii_lowercase();
            if node.is_serialized_file() {
                let meta = SerializedFile::parse(bundle.file(node))
                    .with_context(|| format!("{key}: {}", node.path))?;
                let index = self.files.len();
                self.file_by_name.insert(name.clone(), index);
                self.files.push(AssetFile {
                    name,
                    bundle_key: key.to_owned(),
                    meta,
                    bundle: bundle.clone(),
                    range: node.range.clone(),
                });
                indices.push(index);
            } else {
                self.resources.insert(name, (bundle.clone(), node.range.clone()));
            }
        }
        Ok(indices)
    }

    pub fn files(&self) -> &[AssetFile] {
        &self.files
    }

    pub fn file(&self, index: usize) -> &AssetFile {
        &self.files[index]
    }

    pub fn info(&self, obj: ObjRef) -> Option<&ObjectInfo> {
        self.files.get(obj.file)?.meta.object(obj.path_id)
    }

    pub fn class_id(&self, obj: ObjRef) -> Option<i32> {
        self.info(obj).map(|o| o.class_id)
    }

    /// All objects of one class in one file.
    pub fn objects_of(&self, file: usize, class_id: i32) -> impl Iterator<Item = ObjRef> + '_ {
        self.files[file]
            .meta
            .objects
            .iter()
            .filter(move |o| o.class_id == class_id)
            .map(move |o| ObjRef { file, path_id: o.path_id })
    }

    pub fn read(&self, obj: ObjRef) -> Result<Value> {
        let file = self.files.get(obj.file).ok_or_else(|| anyhow!("no file {}", obj.file))?;
        let info = file
            .meta
            .object(obj.path_id)
            .ok_or_else(|| anyhow!("no object {} in {}", obj.path_id, file.name))?;
        let ty = &file.meta.types[info.type_index];
        let bytes = &file.bytes()[info.offset..info.offset + info.size];
        let mut r = Reader { data: bytes, pos: 0, big_endian: file.meta.big_endian };
        ty.tree
            .read(&mut r)
            .with_context(|| format!("object {} (class {}) in {}", obj.path_id, info.class_id, file.name))
    }

    /// Name of an object whose first field is `m_Name`, without reading the
    /// rest of it. Most asset classes are laid out that way.
    pub fn name_of(&self, obj: ObjRef) -> Option<String> {
        let file = self.files.get(obj.file)?;
        let info = file.meta.object(obj.path_id)?;
        let first = file.meta.types[info.type_index].tree.nodes.get(1)?;
        if &*first.name != "m_Name" || &*first.type_name != "string" {
            return None;
        }
        let bytes = &file.bytes()[info.offset..info.offset + info.size];
        let mut r = Reader { data: bytes, pos: 0, big_endian: file.meta.big_endian };
        let len = r.i32().ok()? as usize;
        Some(String::from_utf8_lossy(r.bytes(len).ok()?).into_owned())
    }

    /// Follows a reference found in an object of `from_file`. Opens a
    /// deferred bundle if that is where the reference leads.
    pub fn resolve(&mut self, from_file: usize, pptr: PPtr) -> Result<Option<ObjRef>> {
        if pptr.is_null() {
            return Ok(None);
        }
        if pptr.file_id == 0 {
            return Ok(Some(ObjRef { file: from_file, path_id: pptr.path_id }));
        }
        let external = self.files[from_file]
            .meta
            .externals
            .get(pptr.file_id as usize - 1)
            .ok_or_else(|| anyhow!("file id {} has no external", pptr.file_id))?;
        let name = file_name(external);
        if !self.file_by_name.contains_key(&name) {
            let key = self.files[from_file].bundle_key.clone();
            for dep in self.manifest.dependencies(&key).to_vec() {
                if DEFERRED.contains(&dep.as_str()) && !self.loaded.contains(&dep) {
                    self.load_one(&dep)?;
                    if self.file_by_name.contains_key(&name) {
                        break;
                    }
                }
            }
        }
        Ok(self
            .file_by_name
            .get(&name)
            .map(|&file| ObjRef { file, path_id: pptr.path_id }))
    }

    /// Reference resolution that never opens another bundle.
    pub fn resolve_loaded(&self, from_file: usize, pptr: PPtr) -> Option<ObjRef> {
        if pptr.is_null() {
            return None;
        }
        if pptr.file_id == 0 {
            return Some(ObjRef { file: from_file, path_id: pptr.path_id });
        }
        let external = self.files[from_file].meta.externals.get(pptr.file_id as usize - 1)?;
        let file = *self.file_by_name.get(&file_name(external))?;
        Some(ObjRef { file, path_id: pptr.path_id })
    }

    /// Bytes of a streamed resource (`m_StreamData`), such as texture or
    /// vertex data kept beside the serialized file.
    pub fn resource(&self, path: &str, offset: usize, size: usize) -> Result<&[u8]> {
        let name = file_name(path);
        let (bundle, range) = self
            .resources
            .get(&name)
            .ok_or_else(|| anyhow!("resource {path:?} is not loaded"))?;
        let data = &bundle.data[range.clone()];
        if offset + size > data.len() {
            bail!("resource read {offset}+{size} runs past the end of {path:?}");
        }
        Ok(&data[offset..offset + size])
    }

    /// Where the same bytes are in their bundle's file: to be read again
    /// from it after the reader has gone (`ResourcePlace::read`).
    pub fn resource_place(&self, path: &str, offset: usize, size: usize) -> Result<ResourcePlace> {
        let name = file_name(path);
        let (bundle, range) = self
            .resources
            .get(&name)
            .ok_or_else(|| anyhow!("resource {path:?} is not loaded"))?;
        if offset + size > range.len() {
            bail!("resource read {offset}+{size} runs past the end of {path:?}");
        }
        let file = bundle.path.clone().ok_or_else(|| anyhow!("resource {path:?} is in a bundle read from no file"))?;
        Ok(ResourcePlace { file, range: range.start + offset..range.start + offset + size })
    }
}

/// Bytes of a bundle's, by its file and their place in its data: a
/// texture's levels, read again from the install when they are wanted.
#[derive(Debug, Clone)]
pub struct ResourcePlace {
    file: std::path::PathBuf,
    range: Range<usize>,
}

impl ResourcePlace {
    /// The bytes, read from the bundle's file, only the blocks they are in
    /// decompressed (`Bundle::read_range`).
    pub fn read(&self) -> Result<Vec<u8>> {
        Bundle::read_range(&self.file, self.range.clone())
    }
}

/// `archive:/CAB-abc/CAB-abc.resS` and `Library/unity default resources`
/// both resolve by their last path component.
fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase()
}
