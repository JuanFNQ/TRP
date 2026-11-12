//! Reading a mesh: its channels, vertex buffers and streamed data.

use super::*;

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
    /// The vertices' colours, where the mesh has them, 0 to 1.
    pub colors: Vec<[f32; 4]>,
    /// Its first blend shape's full frame, each vertex's move, where it has
    /// one (a variable scope's reticle: optics spec, "The lens").
    pub shape: Option<Vec<[f32; 3]>>,
}

const CHANNEL_POSITION: usize = 0;

const CHANNEL_NORMAL: usize = 1;

const CHANNEL_TANGENT: usize = 2;

const CHANNEL_COLOR: usize = 3;

const CHANNEL_UV0: usize = 4;

const CHANNEL_BLEND_WEIGHT: usize = 12;

const CHANNEL_BLEND_INDICES: usize = 13;

pub(super) struct Channel {
    pub(super) stream: usize,
    pub(super) offset: usize,
    pub(super) format: u8,
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

pub(super) fn half_to_f32(h: u16) -> f32 {
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

pub(super) struct VertexBuffer<'a> {
    pub(super) data: &'a [u8],
    count: usize,
    channels: Vec<Channel>,
    /// (offset, stride) per stream.
    streams: Vec<(usize, usize)>,
}

impl VertexBuffer<'_> {
    /// Reads channel `index` as `N` floats per vertex. Missing components are zero.
    pub(super) fn read<const N: usize>(&self, index: usize) -> Result<Vec<[f32; N]>> {
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
    mesh_of(&v, Some(assets))
}

/// A mesh from its object as read (by its own tree or one borrowed); its
/// vertices streamed from a resource file where it keeps them so, which
/// needs the assets.
pub fn mesh_of(v: &Value, assets: Option<&Assets>) -> Result<MeshData> {
    mesh_streamed(v, &|name: &str, path: &str, offset: usize, size: usize| match assets {
        Some(assets) => Ok(assets.resource(path, offset, size)?.to_vec()),
        None => bail!("mesh {name:?} streams its vertices from a resource, which needs its bundle"),
    })
}

/// A mesh from its object as read, its vertices streamed where it keeps
/// them so by `stream` (the mesh's name, the resource's path, the offset
/// and the size): from a bundle's resource, or a file of the build's own
/// beside a scene (`scene_props`).
pub fn mesh_streamed(v: &Value, stream: &dyn Fn(&str, &str, usize, usize) -> Result<Vec<u8>>) -> Result<MeshData> {
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
    let streamed;
    let data: &[u8] = if !inline.is_empty() {
        inline
    } else {
        let streamed_data = v.req("m_StreamData")?;
        let size = streamed_data.req("size")?.as_usize().unwrap_or(0);
        if size == 0 {
            &[]
        } else {
            streamed = stream(&name, streamed_data.req("path")?.as_str().unwrap_or(""), streamed_data.req("offset")?.as_usize().unwrap_or(0), size)?;
            &streamed
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
        // Triangles (0), or quads (2: `MeshTopology.Quads`, each drawn as
        // two triangles on its first corner, the RGO's parts); lines and
        // points are not drawn.
        let topology = sm.req("topology")?.as_i64().unwrap_or(0);
        let mut indices = Vec::new();
        if topology == 0 || topology == 2 {
            let Some(bytes) = index_bytes.get(first..first + index_count * index_size) else {
                bail!("submesh of {name:?} runs past the index buffer");
            };
            let read: Vec<u32> = bytes
                .chunks_exact(index_size)
                .map(|chunk| if wide { u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) } else { u16::from_le_bytes([chunk[0], chunk[1]]) as u32 } + base_vertex)
                .collect();
            if topology == 0 {
                indices = read;
            } else {
                indices.reserve(read.len() / 4 * 6);
                for [a, b, c, d] in read.chunks_exact(4).map(|quad| [quad[0], quad[1], quad[2], quad[3]]) {
                    indices.extend([a, b, c, a, c, d]);
                }
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
    // Bones with no weights: one bone a vertex, all of it on that bone (a
    // vertex's weights add up to one). The F-1's tripwire peg is kept so:
    // a bone's index for each vertex, one bind pose, and no weights.
    let mut weights: Vec<[f32; 4]> = vb.read(CHANNEL_BLEND_WEIGHT)?;
    if weights.is_empty() && !joints.is_empty() {
        weights = vec![[1.0, 0.0, 0.0, 0.0]; joints.len()];
    }
    // The first blend shape's full frame, as each vertex's move.
    let shape = v.get("m_Shapes").and_then(|shapes| {
        let channel = shapes.get("channels")?.as_array()?.first()?;
        let frame = channel.get("frameIndex")?.as_usize()? + channel.get("frameCount")?.as_usize()?.checked_sub(1)?;
        let piece = shapes.get("shapes")?.as_array()?.get(frame)?;
        let (first, n) = (piece.get("firstVertex")?.as_usize()?, piece.get("vertexCount")?.as_usize()?);
        let moves = shapes.get("vertices")?.as_array()?;
        let mut out = vec![[0.0f32; 3]; count];
        for m in moves.get(first..first + n)? {
            let index = m.get("index")?.as_usize()?;
            if let Some(slot) = out.get_mut(index) {
                *slot = m.get("vertex")?.as_vec3()?;
            }
        }
        Some(out)
    });
    Ok(MeshData {
        positions: vb.read(CHANNEL_POSITION)?,
        normals: vb.read(CHANNEL_NORMAL)?,
        tangents: vb.read(CHANNEL_TANGENT)?,
        uv0: vb.read(CHANNEL_UV0)?,
        colors: vb.read(CHANNEL_COLOR)?,
        shape,
        weights,
        joints: joints.iter().map(|j| j.map(|c| c as u16)).collect(),
        submeshes,
        bind_poses,
        name,
    })
}
