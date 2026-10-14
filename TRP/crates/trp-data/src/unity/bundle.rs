//! UnityFS archive: a header, a compressed block table and a directory of
//! files stored as one concatenated, block-compressed stream.

use std::ops::Range;
use std::path::Path;

use anyhow::{bail, Context, Result};

use super::reader::Reader;

const COMPRESSION_MASK: u32 = 0x3f;
const BLOCKS_INFO_AT_END: u32 = 0x80;
const BLOCK_INFO_PADDING: u32 = 0x200;

/// Set on directory entries that are serialized files rather than raw resources.
const NODE_SERIALIZED_FILE: u32 = 0x4;

#[derive(Debug, Clone)]
pub struct Node {
    pub path: String,
    pub flags: u32,
    pub range: Range<usize>,
}

impl Node {
    pub fn is_serialized_file(&self) -> bool {
        self.flags & NODE_SERIALIZED_FILE != 0
    }
}

/// A fully decompressed bundle.
pub struct Bundle {
    pub unity_revision: String,
    pub data: Vec<u8>,
    pub nodes: Vec<Node>,
}

impl Bundle {
    pub fn open(path: &Path) -> Result<Self> {
        let raw = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&raw).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn parse(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::big_endian(raw);
        let signature = r.cstr()?;
        if signature != "UnityFS" {
            bail!("not a UnityFS bundle (signature {signature:?})");
        }
        let version = r.u32()?;
        let _player_version = r.cstr()?;
        let unity_revision = r.cstr()?;
        let _size = r.i64()?;
        let compressed_info_size = r.u32()? as usize;
        let uncompressed_info_size = r.u32()? as usize;
        let flags = r.u32()?;
        if version >= 7 {
            r.align(16);
        }

        let info_raw = if flags & BLOCKS_INFO_AT_END != 0 {
            let start = raw
                .len()
                .checked_sub(compressed_info_size)
                .context("block table larger than file")?;
            &raw[start..]
        } else {
            r.bytes(compressed_info_size)?
        };
        let info = decompress(flags & COMPRESSION_MASK, info_raw, uncompressed_info_size)?;
        if flags & BLOCK_INFO_PADDING != 0 {
            r.align(16);
        }

        let mut ir = Reader::big_endian(&info);
        ir.skip(16)?; // hash of the uncompressed data
        let block_count = ir.i32()? as usize;
        let mut blocks = Vec::with_capacity(block_count);
        let mut total = 0usize;
        for _ in 0..block_count {
            let uncompressed = ir.u32()? as usize;
            let compressed = ir.u32()? as usize;
            let block_flags = ir.u16()? as u32;
            total += uncompressed;
            blocks.push((uncompressed, compressed, block_flags));
        }
        let node_count = ir.i32()? as usize;
        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let offset = ir.i64()? as usize;
            let size = ir.i64()? as usize;
            let node_flags = ir.u32()?;
            let path = ir.cstr()?;
            if offset + size > total {
                bail!("directory entry {path:?} runs past the end of the data");
            }
            nodes.push(Node { path, flags: node_flags, range: offset..offset + size });
        }

        let mut data = Vec::with_capacity(total);
        for (uncompressed, compressed, block_flags) in blocks {
            let src = r.bytes(compressed)?;
            let kind = block_flags & COMPRESSION_MASK;
            if kind == 0 {
                data.extend_from_slice(src);
            } else {
                data.extend_from_slice(&decompress(kind, src, uncompressed)?);
            }
        }
        Ok(Self { unity_revision, data, nodes })
    }

    pub fn file(&self, node: &Node) -> &[u8] {
        &self.data[node.range.clone()]
    }
}

fn decompress(kind: u32, src: &[u8], uncompressed_size: usize) -> Result<Vec<u8>> {
    match kind {
        0 => Ok(src.to_vec()),
        1 => {
            // LZMA: 5 property bytes, then the stream with no size field.
            use lzma_rs::decompress::{Options, UnpackedSize};
            let options = Options {
                unpacked_size: UnpackedSize::UseProvided(Some(uncompressed_size as u64)),
                memlimit: None,
                allow_incomplete: false,
            };
            let mut out = Vec::with_capacity(uncompressed_size);
            lzma_rs::lzma_decompress_with_options(&mut std::io::Cursor::new(src), &mut out, &options)
                .map_err(|e| anyhow::anyhow!("lzma: {e:?}"))?;
            Ok(out)
        }
        2 | 3 => {
            let mut out = vec![0u8; uncompressed_size];
            let n = lz4_flex::block::decompress_into(src, &mut out)
                .map_err(|e| anyhow::anyhow!("lz4: {e}"))?;
            if n != uncompressed_size {
                bail!("lz4 block decompressed to {n} bytes, expected {uncompressed_size}");
            }
            Ok(out)
        }
        other => bail!("unsupported bundle compression {other}"),
    }
}
