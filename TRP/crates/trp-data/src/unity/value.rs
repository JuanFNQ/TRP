//! Dynamically typed object data, as described by a type tree.

use std::fmt::Write as _;
use std::sync::Arc;

use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub enum Value {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Str(String),
    /// Arrays of one-byte elements and `TypelessData`.
    Bytes(Vec<u8>),
    Floats(Vec<f32>),
    Ints(Vec<i32>),
    UInts(Vec<u32>),
    U16s(Vec<u16>),
    Array(Vec<Value>),
    Struct(Struct),
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub type_name: Arc<str>,
    pub fields: Vec<(Arc<str>, Value)>,
}

/// Reference to another object: `file_id` 0 is the same file, otherwise an
/// index into the file's externals, starting at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PPtr {
    pub file_id: i32,
    pub path_id: i64,
}

impl PPtr {
    pub fn is_null(&self) -> bool {
        self.path_id == 0
    }
}

impl Value {
    pub fn get(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Struct(s) => s.fields.iter().find(|(n, _)| &**n == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// A struct's fields by name, in their stored order; none for anything
    /// else.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &Value)> {
        let fields = match self {
            Value::Struct(s) => s.fields.as_slice(),
            _ => &[],
        };
        fields.iter().map(|(name, value)| (&**name, value))
    }

    /// Field lookup through a dotted path.
    pub fn at(&self, path: &str) -> Option<&Value> {
        path.split('.').try_fold(self, |v, name| v.get(name))
    }

    pub fn req(&self, path: &str) -> Result<&Value> {
        self.at(path).ok_or_else(|| anyhow!("missing field {path:?}"))
    }

    pub fn type_name(&self) -> &str {
        match self {
            Value::Struct(s) => &s.type_name,
            _ => "",
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::Int(v) => Some(v),
            Value::UInt(v) => Some(v as i64),
            Value::Bool(v) => Some(v as i64),
            _ => None,
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        self.as_i64().map(|v| v as u32)
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_i64().map(|v| v as usize)
    }

    pub fn as_bool(&self) -> Option<bool> {
        self.as_i64().map(|v| v != 0)
    }

    pub fn as_f32(&self) -> Option<f32> {
        match *self {
            Value::Float(v) => Some(v as f32),
            Value::Int(v) => Some(v as f32),
            Value::UInt(v) => Some(v as f32),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_floats(&self) -> Option<&[f32]> {
        match self {
            Value::Floats(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_uints(&self) -> Option<&[u32]> {
        match self {
            Value::UInts(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_ints(&self) -> Option<&[i32]> {
        match self {
            Value::Ints(v) => Some(v),
            _ => None,
        }
    }

    /// Number of elements if this is any kind of array.
    pub fn len(&self) -> Option<usize> {
        Some(match self {
            Value::Bytes(v) => v.len(),
            Value::Floats(v) => v.len(),
            Value::Ints(v) => v.len(),
            Value::UInts(v) => v.len(),
            Value::U16s(v) => v.len(),
            Value::Array(v) => v.len(),
            _ => return None,
        })
    }

    pub fn as_pptr(&self) -> Option<PPtr> {
        Some(PPtr {
            file_id: self.get("m_FileID")?.as_i64()? as i32,
            path_id: self.get("m_PathID")?.as_i64()?,
        })
    }

    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        Some([
            self.get("x")?.as_f32()?,
            self.get("y")?.as_f32()?,
            self.get("z")?.as_f32()?,
        ])
    }

    pub fn as_vec4(&self) -> Option<[f32; 4]> {
        Some([
            self.get("x")?.as_f32()?,
            self.get("y")?.as_f32()?,
            self.get("z")?.as_f32()?,
            self.get("w")?.as_f32()?,
        ])
    }

    /// Indented text form with long arrays cut short, for inspection.
    pub fn pretty(&self, max_depth: usize, max_items: usize) -> String {
        let mut out = String::new();
        self.write_pretty(&mut out, 0, max_depth, max_items);
        out
    }

    fn write_pretty(&self, out: &mut String, depth: usize, max_depth: usize, max_items: usize) {
        let pad = "  ".repeat(depth);
        match self {
            Value::Bool(v) => _ = write!(out, "{v}"),
            Value::Int(v) => _ = write!(out, "{v}"),
            Value::UInt(v) => _ = write!(out, "{v}"),
            Value::Float(v) => _ = write!(out, "{v}"),
            Value::Str(v) => _ = write!(out, "{v:?}"),
            Value::Bytes(v) => _ = write!(out, "<{} bytes>", v.len()),
            Value::Floats(v) => write_prims(out, "f32", v, max_items),
            Value::Ints(v) => write_prims(out, "i32", v, max_items),
            Value::UInts(v) => write_prims(out, "u32", v, max_items),
            Value::U16s(v) => write_prims(out, "u16", v, max_items),
            Value::Array(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                } else if depth >= max_depth {
                    _ = write!(out, "[{} items]", items.len());
                } else {
                    _ = write!(out, "[{} items]", items.len());
                    for (i, item) in items.iter().take(max_items).enumerate() {
                        _ = write!(out, "\n{pad}  [{i}] ");
                        item.write_pretty(out, depth + 1, max_depth, max_items);
                    }
                    if items.len() > max_items {
                        _ = write!(out, "\n{pad}  ...");
                    }
                }
            }
            Value::Struct(s) => {
                if let Some(p) = self.as_pptr().filter(|_| s.fields.len() == 2) {
                    _ = write!(out, "PPtr(file {}, path {})", p.file_id, p.path_id);
                } else if depth >= max_depth {
                    _ = write!(out, "{} {{..}}", s.type_name);
                } else {
                    _ = write!(out, "{}", s.type_name);
                    for (name, v) in &s.fields {
                        _ = write!(out, "\n{pad}  {name}: ");
                        v.write_pretty(out, depth + 1, max_depth, max_items);
                    }
                }
            }
        }
    }
}

fn write_prims<T: std::fmt::Debug>(out: &mut String, ty: &str, v: &[T], max_items: usize) {
    let shown = &v[..v.len().min(max_items)];
    let more = if v.len() > max_items { ", .." } else { "" };
    _ = write!(out, "{ty}[{}] {shown:?}{more}", v.len());
}
