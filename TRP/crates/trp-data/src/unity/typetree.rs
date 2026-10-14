//! Type trees: the per-class field layout a serialized file carries, and the
//! generic reader that turns object bytes into a [`Value`] by following one.

use std::sync::{Arc, OnceLock};

use anyhow::{bail, Result};

use super::reader::Reader;
use super::value::{Struct, Value};

/// Set on nodes whose data is followed by padding to a 4-byte boundary.
const ALIGN_BYTES: u32 = 0x4000;

#[derive(Debug, Clone)]
pub struct Node {
    pub type_name: Arc<str>,
    pub name: Arc<str>,
    pub level: u8,
    pub meta_flag: u32,
    /// Index one past the last node of this node's subtree.
    pub end: usize,
}

#[derive(Debug, Clone, Default)]
pub struct TypeTree {
    pub nodes: Vec<Node>,
}

impl TypeTree {
    /// Parses the blob form used by serialized file versions 10 and 12+.
    pub fn parse_blob(r: &mut Reader, file_version: u32) -> Result<Self> {
        let node_count = r.i32()? as usize;
        let string_size = r.i32()? as usize;
        let node_size = if file_version >= 19 { 32 } else { 24 };
        let node_bytes = r.bytes(node_count * node_size)?;
        let strings = r.bytes(string_size)?;

        let mut nodes = Vec::with_capacity(node_count);
        let mut nr = Reader { data: node_bytes, pos: 0, big_endian: r.big_endian };
        for _ in 0..node_count {
            let _version = nr.u16()?;
            let level = nr.u8()?;
            let _type_flags = nr.u8()?;
            let type_offset = nr.u32()?;
            let name_offset = nr.u32()?;
            let _byte_size = nr.i32()?;
            let _index = nr.i32()?;
            let meta_flag = nr.u32()?;
            if file_version >= 19 {
                nr.skip(8)?; // ref type hash
            }
            nodes.push(Node {
                type_name: string_at(strings, type_offset)?,
                name: string_at(strings, name_offset)?,
                level,
                meta_flag,
                end: 0,
            });
        }
        Ok(Self::from_nodes(nodes))
    }

    /// Builds a tree from nodes in file order, working out where each
    /// node's subtree ends from the levels.
    pub fn from_nodes(mut nodes: Vec<Node>) -> Self {
        for i in 0..nodes.len() {
            let level = nodes[i].level;
            let end = nodes[i + 1..]
                .iter()
                .position(|n| n.level <= level)
                .map_or(nodes.len(), |p| i + 1 + p);
            nodes[i].end = end;
        }
        Self { nodes }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Reads one object laid out by this tree.
    pub fn read(&self, r: &mut Reader) -> Result<Value> {
        if self.nodes.is_empty() {
            bail!("type has no type tree");
        }
        self.read_node(0, r)
    }

    fn read_node(&self, i: usize, r: &mut Reader) -> Result<Value> {
        let node = &self.nodes[i];
        let mut align = node.meta_flag & ALIGN_BYTES != 0;
        let value = match &*node.type_name {
            "SInt8" => Value::Int(r.i8()? as i64),
            "UInt8" | "char" => Value::UInt(r.u8()? as u64),
            "bool" => Value::Bool(r.u8()? != 0),
            "SInt16" | "short" => Value::Int(r.i16()? as i64),
            "UInt16" | "unsigned short" => Value::UInt(r.u16()? as u64),
            "SInt32" | "int" => Value::Int(r.i32()? as i64),
            "UInt32" | "unsigned int" | "Type*" => Value::UInt(r.u32()? as u64),
            "SInt64" | "long long" => Value::Int(r.i64()?),
            "UInt64" | "unsigned long long" | "FileSize" => Value::UInt(r.u64()?),
            "float" => Value::Float(r.f32()? as f64),
            "double" => Value::Float(r.f64()?),
            "string" => {
                let len = r.i32()? as usize;
                let bytes = r.bytes(len)?;
                align = true;
                // Text assets keep binary data in a string field. Anything
                // that is not valid text stays as bytes, so nothing is lost.
                match String::from_utf8(bytes.to_vec()) {
                    Ok(text) => Value::Str(text),
                    Err(e) => Value::Bytes(e.into_bytes()),
                }
            }
            "TypelessData" => {
                let len = r.i32()? as usize;
                Value::Bytes(r.bytes(len)?.to_vec())
            }
            "ManagedReferencesRegistry" => bail!("managed references are not supported"),
            _ => {
                let wraps_array = i + 1 < node.end && &*self.nodes[i + 1].type_name == "Array";
                if wraps_array {
                    // vector, map, set and friends: Array { size, data }.
                    if self.nodes[i + 1].meta_flag & ALIGN_BYTES != 0 {
                        align = true;
                    }
                    let len = r.i32()? as usize;
                    self.read_array(i + 3, len, r)?
                } else {
                    let mut fields = Vec::new();
                    let mut child = i + 1;
                    while child < node.end {
                        fields.push((self.nodes[child].name.clone(), self.read_node(child, r)?));
                        child = self.nodes[child].end;
                    }
                    Value::Struct(Struct { type_name: node.type_name.clone(), fields })
                }
            }
        };
        if align {
            r.align(4);
        }
        Ok(value)
    }

    fn read_array(&self, elem: usize, len: usize, r: &mut Reader) -> Result<Value> {
        let Some(node) = self.nodes.get(elem) else {
            bail!("array without an element type");
        };
        if len > r.remaining() {
            bail!("array of {len} elements at {} cannot fit in the object", r.pos);
        }
        let is_leaf = node.end == elem + 1 && node.meta_flag & ALIGN_BYTES == 0;
        if is_leaf {
            // Typed fast paths: vertex data, sample arrays and index lists are
            // large, and one enum per element would be wasteful.
            match &*node.type_name {
                "UInt8" | "char" | "SInt8" | "bool" => {
                    return Ok(Value::Bytes(r.bytes(len)?.to_vec()));
                }
                "float" => {
                    let mut v = Vec::with_capacity(len);
                    for _ in 0..len {
                        v.push(r.f32()?);
                    }
                    return Ok(Value::Floats(v));
                }
                "SInt32" | "int" => {
                    let mut v = Vec::with_capacity(len);
                    for _ in 0..len {
                        v.push(r.i32()?);
                    }
                    return Ok(Value::Ints(v));
                }
                "UInt32" | "unsigned int" => {
                    let mut v = Vec::with_capacity(len);
                    for _ in 0..len {
                        v.push(r.u32()?);
                    }
                    return Ok(Value::UInts(v));
                }
                "UInt16" | "unsigned short" => {
                    let mut v = Vec::with_capacity(len);
                    for _ in 0..len {
                        v.push(r.u16()?);
                    }
                    return Ok(Value::U16s(v));
                }
                _ => {}
            }
        }
        let mut items = Vec::with_capacity(len.min(1 << 16));
        for _ in 0..len {
            items.push(self.read_node(elem, r)?);
        }
        Ok(Value::Array(items))
    }
}

/// Type and field names are offsets into the tree's own string block, or,
/// with the top bit set, into a table built into every Unity version.
fn string_at(local: &[u8], offset: u32) -> Result<Arc<str>> {
    let (table, offset) = if offset & 0x8000_0000 != 0 {
        (common_strings(), (offset & 0x7fff_ffff) as usize)
    } else {
        (local, offset as usize)
    };
    let Some(rest) = table.get(offset..) else {
        bail!("type tree string offset {offset} out of range");
    };
    let len = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
    Ok(Arc::from(String::from_utf8_lossy(&rest[..len]).as_ref()))
}

fn common_strings() -> &'static [u8] {
    static TABLE: OnceLock<Vec<u8>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = Vec::new();
        for s in COMMON_STRINGS {
            table.extend_from_slice(s.as_bytes());
            table.push(0);
        }
        table
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(type_name: &str, name: &str, level: u8, meta_flag: u32) -> Node {
        Node { type_name: type_name.into(), name: name.into(), level, meta_flag, end: 0 }
    }

    #[test]
    fn reads_strings_arrays_and_padding() {
        let tree = TypeTree::from_nodes(vec![
            node("Thing", "Base", 0, 0),
            node("string", "m_Name", 1, 0),
            node("Array", "Array", 2, ALIGN_BYTES),
            node("int", "size", 3, 0),
            node("char", "data", 3, 0),
            node("UInt8", "flag", 1, ALIGN_BYTES),
            node("vector", "samples", 1, 0),
            node("Array", "Array", 2, 0),
            node("int", "size", 3, 0),
            node("float", "data", 3, 0),
            node("PPtr<Object>", "target", 1, 0),
            node("int", "m_FileID", 2, 0),
            node("SInt64", "m_PathID", 2, 0),
        ]);
        let mut bytes = Vec::new();
        bytes.extend(3i32.to_le_bytes());
        bytes.extend(b"abc\0"); // one byte of padding after the text
        bytes.extend([7, 0, 0, 0]); // flag, then padding
        bytes.extend(2i32.to_le_bytes());
        bytes.extend(1.5f32.to_le_bytes());
        bytes.extend((-2.0f32).to_le_bytes());
        bytes.extend(1i32.to_le_bytes());
        bytes.extend((-42i64).to_le_bytes());

        let mut r = Reader::new(&bytes);
        let v = tree.read(&mut r).unwrap();
        assert_eq!(r.pos, bytes.len());
        assert_eq!(v.type_name(), "Thing");
        assert_eq!(v.get("m_Name").unwrap().as_str(), Some("abc"));
        assert_eq!(v.get("flag").unwrap().as_i64(), Some(7));
        assert_eq!(v.get("samples").unwrap().as_floats(), Some(&[1.5, -2.0][..]));
        let ptr = v.get("target").unwrap().as_pptr().unwrap();
        assert_eq!((ptr.file_id, ptr.path_id), (1, -42));
    }

    #[test]
    fn rejects_arrays_longer_than_the_data() {
        let tree = TypeTree::from_nodes(vec![
            node("Thing", "Base", 0, 0),
            node("vector", "items", 1, 0),
            node("Array", "Array", 2, 0),
            node("int", "size", 3, 0),
            node("float", "data", 3, 0),
        ]);
        let bytes = 1_000_000i32.to_le_bytes();
        assert!(tree.read(&mut Reader::new(&bytes)).is_err());
    }
}

/// Order matters: offsets are positions in the NUL-joined concatenation.
const COMMON_STRINGS: &[&str] = &[
    "AABB", "AnimationClip", "AnimationCurve", "AnimationState", "Array", "Base", "BitField",
    "bitset", "bool", "char", "ColorRGBA", "Component", "data", "deque", "double",
    "dynamic_array", "FastPropertyName", "first", "float", "Font", "GameObject", "Generic Mono",
    "GradientNEW", "GUID", "GUIStyle", "int", "list", "long long", "map", "Matrix4x4f", "MdFour",
    "MonoBehaviour", "MonoScript", "m_ByteSize", "m_Curve", "m_EditorClassIdentifier",
    "m_EditorHideFlags", "m_Enabled", "m_ExtensionPtr", "m_GameObject", "m_Index", "m_IsArray",
    "m_IsStatic", "m_MetaFlag", "m_Name", "m_ObjectHideFlags", "m_PrefabInternal",
    "m_PrefabParentObject", "m_Script", "m_StaticEditorFlags", "m_Type", "m_Version", "Object",
    "pair", "PPtr<Component>", "PPtr<GameObject>", "PPtr<Material>", "PPtr<MonoBehaviour>",
    "PPtr<MonoScript>", "PPtr<Object>", "PPtr<Prefab>", "PPtr<Sprite>", "PPtr<TextAsset>",
    "PPtr<Texture>", "PPtr<Texture2D>", "PPtr<Transform>", "Prefab", "Quaternionf", "Rectf",
    "RectInt", "RectOffset", "second", "set", "short", "size", "SInt16", "SInt32", "SInt64",
    "SInt8", "staticvector", "string", "TextAsset", "TextMesh", "Texture", "Texture2D",
    "Transform", "TypelessData", "UInt16", "UInt32", "UInt64", "UInt8", "unsigned int",
    "unsigned long long", "unsigned short", "vector", "Vector2f", "Vector3f", "Vector4f",
    "m_ScriptingClassIdentifier", "Gradient", "Type*", "int2_storage", "int3_storage",
    "BoundsInt", "m_CorrespondingSourceObject", "m_PrefabInstance", "m_PrefabAsset", "FileSize",
    "Hash128",
];
