//! Serialized file: a table of typed objects plus the type trees needed to
//! read them.

use std::collections::HashMap;

use anyhow::{bail, Result};

use super::reader::Reader;
use super::typetree::TypeTree;

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub class_id: i32,
    pub tree: TypeTree,
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectInfo {
    pub path_id: i64,
    /// Offset from the start of the serialized file.
    pub offset: usize,
    pub size: usize,
    pub type_index: usize,
    pub class_id: i32,
}

/// Parsed metadata of one serialized file. Object bytes stay in the caller's buffer.
#[derive(Debug, Clone)]
pub struct SerializedFile {
    pub version: u32,
    pub unity_version: String,
    pub big_endian: bool,
    pub has_type_trees: bool,
    pub types: Vec<TypeInfo>,
    pub objects: Vec<ObjectInfo>,
    /// Path names of the files `PPtr::file_id` values 1.. refer to.
    pub externals: Vec<String>,
    index: HashMap<i64, usize>,
}

impl SerializedFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::parse_head(data, data.len())
    }

    /// Where a file's objects begin, from its first 48 bytes: what of it
    /// `parse_head` needs.
    pub fn head_size(first: &[u8]) -> Result<usize> {
        let mut r = Reader::big_endian(first);
        r.skip(8)?;
        let version = r.u32()?;
        let data_offset = r.u32()? as usize;
        if version < 22 {
            return Ok(data_offset);
        }
        r.skip(4 + 4 + 8)?;
        Ok(r.i64()? as usize)
    }

    /// A file's table read from its head alone (`head_size` bytes of it),
    /// the file being so long: its objects read later by their ranges.
    pub fn parse_head(data: &[u8], file_len: usize) -> Result<Self> {
        let mut r = Reader::big_endian(data);
        let _metadata_size = r.u32()?;
        let _file_size = r.u32()?;
        let version = r.u32()?;
        let mut data_offset = r.u32()? as usize;
        if version < 9 {
            bail!("serialized file version {version} is older than supported");
        }
        let big_endian = r.u8()? != 0;
        r.skip(3)?;
        if version >= 22 {
            let _metadata_size = r.u32()?;
            let _file_size = r.i64()?;
            data_offset = r.i64()? as usize;
            r.skip(8)?;
        }
        r.big_endian = big_endian;

        let unity_version = r.cstr()?;
        let _target_platform = r.i32()?;
        let has_type_trees = if version >= 13 { r.u8()? != 0 } else { true };

        let type_count = r.i32()? as usize;
        let mut types = Vec::with_capacity(type_count);
        for _ in 0..type_count {
            types.push(read_type(&mut r, version, has_type_trees, false)?);
        }
        let big_ids = if (7..14).contains(&version) { r.i32()? != 0 } else { false };

        let object_count = r.i32()? as usize;
        let mut objects = Vec::with_capacity(object_count);
        for _ in 0..object_count {
            let path_id = if big_ids {
                r.i64()?
            } else if version < 14 {
                r.i32()? as i64
            } else {
                r.align(4);
                r.i64()?
            };
            let start = if version >= 22 { r.i64()? as usize } else { r.u32()? as usize };
            let size = r.u32()? as usize;
            let type_id = r.i32()?;
            let (type_index, class_id) = if version < 16 {
                let class_id = r.u16()? as i32;
                let index = types.iter().position(|t| t.class_id == type_id).unwrap_or(usize::MAX);
                (index, class_id)
            } else {
                let index = type_id as usize;
                let Some(ty) = types.get(index) else {
                    bail!("object {path_id} refers to missing type {type_id}");
                };
                (index, ty.class_id)
            };
            if version < 11 {
                r.skip(2)?;
            }
            if (11..17).contains(&version) {
                r.skip(2)?;
            }
            if version == 15 || version == 16 {
                r.skip(1)?;
            }
            let offset = data_offset + start;
            if offset + size > file_len {
                bail!("object {path_id} runs past the end of the file");
            }
            objects.push(ObjectInfo { path_id, offset, size, type_index, class_id });
        }

        if version >= 11 {
            let script_count = r.i32()? as usize;
            for _ in 0..script_count {
                r.skip(4)?;
                if version < 14 {
                    r.skip(4)?;
                } else {
                    r.align(4);
                    r.skip(8)?;
                }
            }
        }

        let external_count = r.i32()? as usize;
        let mut externals = Vec::with_capacity(external_count);
        for _ in 0..external_count {
            let _asset_path = r.cstr()?;
            r.skip(16 + 4)?; // guid, type
            externals.push(r.cstr()?);
        }

        let index = objects.iter().enumerate().map(|(i, o)| (o.path_id, i)).collect();
        Ok(Self { version, unity_version, big_endian, has_type_trees, types, objects, externals, index })
    }

    pub fn object(&self, path_id: i64) -> Option<&ObjectInfo> {
        self.index.get(&path_id).map(|&i| &self.objects[i])
    }
}

fn read_type(r: &mut Reader, version: u32, has_type_trees: bool, is_ref: bool) -> Result<TypeInfo> {
    let class_id = r.i32()?;
    if version >= 16 {
        r.skip(1)?; // stripped flag
    }
    let script_type_index = if version >= 17 { r.i16()? } else { -1 };
    if version >= 13 {
        let has_script_id = (is_ref && script_type_index >= 0)
            || (version < 16 && class_id < 0)
            || (version >= 16 && class_id == 114);
        if has_script_id {
            r.skip(16)?;
        }
        r.skip(16)?; // type hash
    }
    let mut tree = TypeTree::default();
    if has_type_trees {
        if version < 12 && version != 10 {
            bail!("pre-blob type trees are not supported");
        }
        tree = TypeTree::parse_blob(r, version)?;
        if version >= 21 {
            if is_ref {
                r.cstr()?;
                r.cstr()?;
                r.cstr()?;
            } else {
                let count = r.i32()? as usize;
                r.skip(count * 4)?;
            }
        }
    }
    Ok(TypeInfo { class_id, tree })
}
