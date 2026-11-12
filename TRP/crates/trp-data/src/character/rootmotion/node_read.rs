//! Reading the table's nodes from its binary text asset.

use super::*;

pub(super) fn text_asset_bytes(value: &Value) -> Result<&[u8]> {
    match value.req("m_Script")? {
        Value::Bytes(b) => Ok(b),
        // Valid UTF-8 is kept as text, byte for byte.
        Value::Str(s) => Ok(s.as_bytes()),
        _ => bail!("table data has an unexpected type"),
    }
}

pub(super) fn read_node(r: &mut Bin) -> Result<Node> {
    let kind = r.u8()?;
    let name = r.string()?;
    let looping = r.u8()? != 0;
    let mut node = match kind {
        1 => {
            let _name_again = r.string()?;
            let time_scale = r.f32()?;
            let length = r.f32()?;
            let _clip_index = r.i32()?;
            let velocity = [r.f32()?, r.f32()?, r.f32()?];
            let stores_rotation = r.u8()? != 0;
            let uses_curves = r.u8()? != 0;
            if stores_rotation || uses_curves {
                let curves = r.i16()?;
                for _ in 0..curves {
                    let keys = r.i32()? as usize;
                    // Wrap modes, then per key six floats and a weighted mode.
                    r.skip(4 + keys * (6 * 4 + 2))?;
                }
            }
            Node::Clip { name, looping, length, time_scale, velocity, uses_curves }
        }
        2 => {
            // Zero is the game's "none" and does not occur in the table.
            let blend = match r.u8()? {
                1 => BlendType::Simple1D,
                2 => BlendType::FreeformDirectional2D,
                3 => BlendType::FreeformCartesian2D,
                4 => BlendType::SimpleDirectional2D,
                5 => BlendType::Direct,
                other => bail!("unknown blend type {other}"),
            };
            let param_x = r.u32()?;
            let param_y = r.u32()?;
            let _child_count = r.i32()?;
            let thresholds = (0..r.i32()?).map(|_| r.f32()).collect::<Result<Vec<_>>>()?;
            let positions = (0..r.i32()?).map(|_| Ok([r.f32()?, r.f32()?])).collect::<Result<Vec<_>>>()?;
            let direct_params = (0..r.i32()?).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
            let steps_x = r.i32()? as usize;
            let steps_y = r.i32()? as usize;
            let grid_children = r.i32()? as usize;
            let _stores_rotation = r.u8()?;
            let count = steps_x * steps_y * grid_children;
            if count * 4 > r.data.len() - r.pos {
                bail!("weight grid of {name} runs past the end of the table");
            }
            let weights = (0..count).map(|_| r.f32()).collect::<Result<Vec<_>>>()?;
            Node::Tree {
                name,
                blend,
                param_x,
                param_y,
                thresholds,
                positions,
                direct_params,
                steps_x,
                steps_y,
                weights,
                children: Vec::new(),
            }
        }
        other => bail!("unknown node type {other}"),
    };
    let child_count = r.i32()?;
    let mut read_children = Vec::new();
    for _ in 0..child_count {
        read_children.push(read_node(r)?);
    }
    if let Node::Tree { children, .. } = &mut node {
        *children = read_children;
    }
    Ok(node)
}

/// Little-endian reader for the .NET `BinaryWriter` layout.
pub(super) struct Bin<'a> {
    pub(super) data: &'a [u8],
    pub(super) pos: usize,
}

impl Bin<'_> {
    pub(super) fn take(&mut self, n: usize) -> Result<&[u8]> {
        if n > self.data.len() - self.pos {
            bail!("root motion table is truncated at {}", self.pos);
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    pub(super) fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    /// Length-prefixed string; the length is a 7-bit variable-length integer.
    pub(super) fn string(&mut self) -> Result<String> {
        let mut len = 0usize;
        let mut shift = 0;
        loop {
            let byte = self.u8()?;
            len |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }
}
