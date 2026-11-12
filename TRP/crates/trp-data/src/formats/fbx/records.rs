//! Parsing a binary FBX file into records and their properties.

use super::*;

/// One value of a record.
#[derive(Debug, Clone)]
pub enum Prop {
    Int(i64),
    Float(f64),
    Text(String),
    Raw(Vec<u8>),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
}

impl Prop {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Prop::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Prop::Float(v) => Some(*v),
            Prop::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Prop::Text(v) => Some(v),
            _ => None,
        }
    }
}

/// A record of the file.
#[derive(Debug, Clone, Default)]
pub struct Record {
    pub name: String,
    pub props: Vec<Prop>,
    pub children: Vec<Record>,
}

impl Record {
    pub fn child(&self, name: &str) -> Option<&Record> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Record> {
        self.children.iter().filter(move |c| c.name == name)
    }

    pub(super) fn floats(&self, name: &str) -> Option<&[f64]> {
        match self.child(name)?.props.first()? {
            Prop::Floats(v) => Some(v),
            _ => None,
        }
    }

    pub(super) fn ints(&self, name: &str) -> Option<&[i64]> {
        match self.child(name)?.props.first()? {
            Prop::Ints(v) => Some(v),
            _ => None,
        }
    }

    pub(super) fn text(&self, name: &str) -> Option<&str> {
        self.child(name)?.props.first()?.as_str()
    }

    /// A property of the record's `Properties70` list, by its name: the
    /// values after the four words that say what kind it is.
    pub fn property(&self, name: &str) -> Option<&[Prop]> {
        self.child("Properties70")?.all("P").find(|p| p.props.first().and_then(Prop::as_str) == Some(name)).map(|p| &p.props[4.min(p.props.len())..])
    }

    pub(super) fn property_vec3(&self, name: &str) -> Option<[f64; 3]> {
        let values = self.property(name)?;
        Some([values.first()?.as_f64()?, values.get(1)?.as_f64()?, values.get(2)?.as_f64()?])
    }
}

/// The number an object's record goes by in the file's connections.
pub(super) fn id_of(record: &Record) -> i64 {
    record.props.first().and_then(Prop::as_i64).unwrap_or(0)
}

/// An object's name: it is stored with its class after it, a zero and a
/// one between.
pub(super) fn name_of(record: &Record) -> String {
    record.props.get(1).and_then(Prop::as_str).map_or(String::new(), |name| name.split('\0').next().unwrap_or("").to_owned())
}

pub(super) struct Cursor<'a> {
    pub(super) data: &'a [u8],
    pub(super) at: usize,
    /// Whether the file's offsets are eight bytes wide (version 7500 on).
    wide: bool,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let bytes = self.data.get(self.at..self.at + count).ok_or_else(|| anyhow!("the file ends inside a record at {}", self.at))?;
        self.at += count;
        Ok(bytes)
    }

    pub(super) fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn offset(&mut self) -> Result<usize> {
        Ok(if self.wide { self.u64()? as usize } else { self.u32()? as usize })
    }

    /// An array's bytes, inflated if the file deflated them.
    pub(super) fn array(&mut self, width: usize) -> Result<Vec<u8>> {
        let (count, encoding, stored) = (self.u32()? as usize, self.u32()?, self.u32()? as usize);
        let bytes = self.take(stored)?;
        let bytes = match encoding {
            0 => bytes.to_vec(),
            1 => miniz_oxide::inflate::decompress_to_vec_zlib(bytes).map_err(|e| anyhow!("an array could not be inflated: {e}"))?,
            other => bail!("an array is stored in an unknown way ({other})"),
        };
        ensure!(bytes.len() == count * width, "an array of {count} values has {} bytes", bytes.len());
        Ok(bytes)
    }

    fn prop(&mut self) -> Result<Prop> {
        let kind = self.u8()?;
        let chunks = |bytes: Vec<u8>, width: usize| bytes.chunks_exact(width).map(<[u8]>::to_vec).collect::<Vec<_>>();
        Ok(match kind {
            b'C' => Prop::Int(self.u8()? as i64),
            b'Y' => Prop::Int(i16::from_le_bytes(self.take(2)?.try_into().unwrap()) as i64),
            b'I' => Prop::Int(i32::from_le_bytes(self.take(4)?.try_into().unwrap()) as i64),
            b'L' => Prop::Int(i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            b'F' => Prop::Float(f32::from_le_bytes(self.take(4)?.try_into().unwrap()) as f64),
            b'D' => Prop::Float(f64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            b'S' => {
                let length = self.u32()? as usize;
                Prop::Text(String::from_utf8_lossy(self.take(length)?).into_owned())
            }
            b'R' => {
                let length = self.u32()? as usize;
                Prop::Raw(self.take(length)?.to_vec())
            }
            b'b' => Prop::Ints(self.array(1)?.into_iter().map(i64::from).collect()),
            b'i' => Prop::Ints(chunks(self.array(4)?, 4).iter().map(|b| i32::from_le_bytes(b[..].try_into().unwrap()) as i64).collect()),
            b'l' => Prop::Ints(chunks(self.array(8)?, 8).iter().map(|b| i64::from_le_bytes(b[..].try_into().unwrap())).collect()),
            b'f' => Prop::Floats(chunks(self.array(4)?, 4).iter().map(|b| f32::from_le_bytes(b[..].try_into().unwrap()) as f64).collect()),
            b'd' => Prop::Floats(chunks(self.array(8)?, 8).iter().map(|b| f64::from_le_bytes(b[..].try_into().unwrap())).collect()),
            other => bail!("a value of an unknown kind ({:?}) at {}", other as char, self.at - 1),
        })
    }

    /// The next record, or none at the empty one that ends a list.
    pub(super) fn record(&mut self) -> Result<Option<Record>> {
        let (end, count, _length) = (self.offset()?, self.offset()?, self.offset()?);
        let name_length = self.u8()? as usize;
        let name = String::from_utf8_lossy(self.take(name_length)?).into_owned();
        if end == 0 {
            return Ok(None);
        }
        let props = (0..count).map(|_| self.prop()).collect::<Result<Vec<_>>>().with_context(|| format!("in the record {name}"))?;
        let mut children = Vec::new();
        while self.at < end {
            match self.record()? {
                Some(child) => children.push(child),
                None => break,
            }
        }
        ensure!(self.at <= end, "the record {name} runs past its end");
        self.at = end;
        Ok(Some(Record { name, props, children }))
    }
}

/// Reads a binary FBX file's records. The root has the file's top records.
pub fn parse(data: &[u8]) -> Result<Record> {
    ensure!(data.starts_with(b"Kaydara FBX Binary  \0"), "not a binary FBX file");
    let version = u32::from_le_bytes(data.get(23..27).ok_or_else(|| anyhow!("the file is too short"))?.try_into().unwrap());
    let mut cursor = Cursor { data, at: 27, wide: version >= 7500 };
    let mut root = Record { name: format!("FBX {version}"), ..Record::default() };
    while let Some(record) = cursor.record()? {
        root.children.push(record);
    }
    Ok(root)
}
