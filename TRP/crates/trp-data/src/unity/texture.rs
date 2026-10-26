//! Texture formats: where each mip level sits in the data, and decoding of
//! the block-compressed formats the weapons use, and of Unity's crunched
//! ones (`texture2ddecoder`'s port of Unity's crunch).

use anyhow::{bail, Result};

use super::extract::TextureData;

/// Unity `TextureFormat` values.
pub mod format {
    pub const ALPHA8: i32 = 1;
    pub const RGB24: i32 = 3;
    pub const RGBA32: i32 = 4;
    pub const ARGB32: i32 = 5;
    pub const DXT1: i32 = 10;
    pub const DXT5: i32 = 12;
    pub const BGRA32: i32 = 14;
    pub const BC6H: i32 = 24;
    pub const BC7: i32 = 25;
    pub const BC4: i32 = 26;
    pub const BC5: i32 = 27;
    pub const DXT1_CRUNCHED: i32 = 28;
    pub const DXT5_CRUNCHED: i32 = 29;
    pub const R8: i32 = 63;
}

#[derive(Debug, Clone, Copy)]
pub struct Mip {
    pub offset: usize,
    pub size: usize,
    pub width: u32,
    pub height: u32,
}

/// Bytes per 4x4 block for compressed formats, bytes per pixel otherwise.
fn unit_size(fmt: i32) -> Result<(usize, bool)> {
    Ok(match fmt {
        format::DXT1 | format::BC4 => (8, true),
        format::DXT5 | format::BC5 | format::BC6H | format::BC7 => (16, true),
        format::ALPHA8 | format::R8 => (1, false),
        format::RGB24 => (3, false),
        format::RGBA32 | format::ARGB32 | format::BGRA32 => (4, false),
        other => bail!("unsupported texture format {other}"),
    })
}

/// Location of every mip level in `tex.data`. Stops early if the data is short.
/// A crunched picture is one piece, its first level alone decoded from it.
pub fn mips(tex: &TextureData) -> Result<Vec<Mip>> {
    if crunched(tex.format) {
        return Ok(vec![Mip { offset: 0, size: tex.data.len(), width: tex.width.max(1), height: tex.height.max(1) }]);
    }
    let (unit, blocks) = unit_size(tex.format)?;
    let mut out = Vec::new();
    let (mut w, mut h, mut offset) = (tex.width.max(1), tex.height.max(1), 0usize);
    for _ in 0..tex.mip_count {
        let size = if blocks {
            (w as usize).div_ceil(4) * (h as usize).div_ceil(4) * unit
        } else {
            w as usize * h as usize * unit
        };
        if offset + size > tex.data.len() {
            break;
        }
        out.push(Mip { offset, size, width: w, height: h });
        offset += size;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    if out.is_empty() {
        bail!("texture {:?} has no complete mip level", tex.name);
    }
    Ok(out)
}

/// A format Unity crunches (`DXT1Crunched`, `DXT5Crunched`).
pub fn crunched(fmt: i32) -> bool {
    matches!(fmt, format::DXT1_CRUNCHED | format::DXT5_CRUNCHED)
}

/// Decodes one mip level to RGBA8.
pub fn decode_rgba(tex: &TextureData, mip: Mip) -> Result<Vec<u8>> {
    let src = &tex.data[mip.offset..mip.offset + mip.size];
    let (w, h) = (mip.width as usize, mip.height as usize);
    let mut out = vec![255u8; w * h * 4];
    match tex.format {
        format::DXT1_CRUNCHED | format::DXT5_CRUNCHED => {
            // Each pixel as the decoder packs it: blue, green, red, alpha.
            let mut pixels = vec![0u32; w * h];
            texture2ddecoder::decode_unity_crunch(src, w, h, &mut pixels).map_err(|e| anyhow::anyhow!("texture {:?}: {e}", tex.name))?;
            for (dst, pixel) in out.chunks_exact_mut(4).zip(&pixels) {
                let [b, g, r, a] = pixel.to_le_bytes();
                dst.copy_from_slice(&[r, g, b, a]);
            }
        }
        format::RGBA32 => out.copy_from_slice(src),
        // A mask of alpha alone (the scopes' shadows, `opticsightsmasks`):
        // white, by its alpha.
        format::ALPHA8 => {
            for (dst, &a) in out.chunks_exact_mut(4).zip(src) {
                dst[3] = a;
            }
        }
        format::ARGB32 => {
            for (dst, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                dst.copy_from_slice(&[s[1], s[2], s[3], s[0]]);
            }
        }
        format::BGRA32 => {
            for (dst, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                dst.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
            }
        }
        format::RGB24 => {
            for (dst, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(3)) {
                dst[..3].copy_from_slice(s);
            }
        }
        format::DXT1 | format::DXT5 => {
            let dxt5 = tex.format == format::DXT5;
            let block_size = if dxt5 { 16 } else { 8 };
            let blocks_w = w.div_ceil(4);
            for (index, block) in src.chunks_exact(block_size).enumerate() {
                let (bx, by) = (index % blocks_w * 4, index / blocks_w * 4);
                let (alpha, color) = if dxt5 { block.split_at(8) } else { (&block[..0], block) };
                let rgba = decode_color_block(color, !dxt5);
                let alphas = dxt5.then(|| decode_alpha_block(alpha));
                for py in 0..4 {
                    for px in 0..4 {
                        let (x, y) = (bx + px, by + py);
                        if x >= w || y >= h {
                            continue;
                        }
                        let mut texel = rgba[py * 4 + px];
                        if let Some(a) = &alphas {
                            texel[3] = a[py * 4 + px];
                        }
                        out[(y * w + x) * 4..][..4].copy_from_slice(&texel);
                    }
                }
            }
        }
        other => bail!("cannot decode texture format {other}"),
    }
    Ok(out)
}

/// One of the two channels of a picture re-packed as two (`two_channels`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// The block's own alpha (a DXT5's alpha block, which is a BC4 block
    /// already, kept as it is).
    Alpha,
    Red,
    Green,
    /// The red channel, inverted: what is bright is dark.
    RedInverted,
    /// Full everywhere.
    Full,
}

/// Re-packs a DXT1 or DXT5 picture, every level, as BC5: two channels of
/// one byte each, each a BC4 block, taken from the picture's own blocks
/// **without decoding them to pixels**. A DXT5's alpha block is a BC4 block
/// as it stands; a channel of a colour block is a line of four values
/// between two endpoints, which a BC4 block holds exactly when its own
/// endpoints are put so that the four fall on four of its eight steps
/// (`bc4_of`). Where they cannot (a range that would go past 0 or 255, a
/// DXT1 block of three colours and black) each value takes the nearest of
/// the eight, within a fourteenth of the block's range.
///
/// What the GPU reads of it is what decoding the same picture to pixels
/// and keeping those two channels gives, to a step or two in 255; the work
/// is a few operations a block where decoding is sixteen pixels of
/// arithmetic and a picture four times the size. Other formats are not
/// done here (`None`): the caller decodes them.
pub fn two_channels(tex: &TextureData, channels: [Channel; 2]) -> Result<Option<Vec<u8>>> {
    let dxt5 = match tex.format {
        format::DXT5 => true,
        format::DXT1 => false,
        _ => return Ok(None),
    };
    if channels.contains(&Channel::Alpha) && !dxt5 {
        return Ok(None);
    }
    let block_size = if dxt5 { 16 } else { 8 };
    let mut out = Vec::new();
    for mip in mips(tex)? {
        let src = &tex.data[mip.offset..mip.offset + mip.size];
        out.reserve(src.len() / block_size * 16);
        for block in src.chunks_exact(block_size) {
            let (alpha, color) = if dxt5 { block.split_at(8) } else { (&block[..0], block) };
            for channel in channels {
                match channel {
                    Channel::Alpha => out.extend_from_slice(alpha),
                    Channel::Full => out.extend_from_slice(&[255, 255, 0, 0, 0, 0, 0, 0]),
                    _ => out.extend_from_slice(&bc4_of(color, channel, !dxt5)),
                }
            }
        }
    }
    Ok(Some(out))
}

/// One channel of a DXT colour block as a BC4 block.
fn bc4_of(color: &[u8], channel: Channel, punch_through: bool) -> [u8; 8] {
    let c0 = u16::from_le_bytes([color[0], color[1]]);
    let c1 = u16::from_le_bytes([color[2], color[3]]);
    let take = |c: u16| -> u32 {
        let [r, g, _] = rgb565(c);
        match channel {
            Channel::Red => r,
            Channel::Green => g,
            Channel::RedInverted => 255 - r,
            _ => 255,
        }
    };
    let (a, b) = (take(c0), take(c1));
    let four = c0 > c1 || !punch_through;
    // The four values as the decoder has them; the fourth of a DXT1 block
    // of three colours is black (inverted, white).
    let black = if channel == Channel::RedInverted { 255 } else { 0 };
    let values: [u32; 4] = if four { [a, b, (2 * a + b) / 3, (a + 2 * b) / 3] } else { [a, b, (a + b) / 2, black] };
    let bits = u32::from_le_bytes([color[4], color[5], color[6], color[7]]);
    let index_of = |texel: usize| (bits >> (texel * 2)) as usize & 3;
    let packed = |r0: u32, r1: u32, steps: [u64; 4]| -> [u8; 8] {
        let mut indices = 0u64;
        for texel in 0..16 {
            indices |= steps[index_of(texel)] << (texel * 3);
        }
        let i = indices.to_le_bytes();
        [r0 as u8, r1 as u8, i[0], i[1], i[2], i[3], i[4], i[5]]
    };
    // Nearly every block: one value, or a line of four held exactly on
    // steps 0, 2, 4 and 6 of eight, the second endpoint a sixth of the
    // range past the line's far end (BC4's steps 2..7 are 1/7..6/7 of the
    // way, its step 1 the second endpoint).
    if four {
        if a == b {
            return packed(a, a, [0; 4]);
        }
        let (near, far) = (a.max(b), a.min(b));
        let past = near as i32 - ((near - far) as i32 * 7 + 3) / 6;
        if past >= 0 {
            return packed(near, past as u32, if a > b { [0, 7, 3, 5] } else { [7, 0, 5, 3] });
        }
    }
    // Which BC4 step each of the four values takes, and the endpoints. A
    // line of four is held exactly on steps 0, 2, 4, 6 of eight with the
    // second endpoint a sixth of the range past the line's far end, or on
    // 1, 3, 5, 7 with the first a sixth past its near end, or, where the
    // line runs from 0 to 255, on the six-step kind's own 0 and 255 with
    // its endpoints at the thirds. Each is tried, and the one whose worst
    // value is nearest taken; and with them, the eight steps between the
    // largest value and the smallest.
    let used: [bool; 4] = std::array::from_fn(|value| (0..16).any(|texel| index_of(texel) == value));
    let (hi, lo) = (0..4).filter(|v| used[*v]).fold((0u32, 255u32), |(hi, lo), v| (hi.max(values[v]), lo.min(values[v])));
    let (near, far) = (a.max(b) as f32, a.min(b) as f32);
    let range = near - far;
    let fit = |v: f32| v.round().clamp(0.0, 255.0) as u32;
    let tried: [(u32, u32); 4] = if four {
        [(hi, lo), (fit(near), fit(near - range * 7.0 / 6.0)), (fit(near + range / 6.0), fit(far)), (fit(far + range / 3.0), fit(far + range * 2.0 / 3.0))]
    } else {
        // Three colours and black: the two colours at the six-step kind's
        // endpoints, black on its own 0 (or 255), the colour between them
        // on the nearest fifth.
        [(hi, lo), (a.min(b), a.max(b)), (hi, lo), (hi, lo)]
    };
    let levels = |r0: u32, r1: u32| -> [u32; 8] {
        if r0 > r1 {
            std::array::from_fn(|k| match k as u32 {
                0 => r0,
                1 => r1,
                k => ((8 - k) * r0 + (k - 1) * r1) / 7,
            })
        } else {
            std::array::from_fn(|k| match k as u32 {
                0 => r0,
                1 => r1,
                6 => 0,
                7 => 255,
                k => ((6 - k) * r0 + (k - 1) * r1) / 5,
            })
        }
    };
    let mut best: Option<(u32, u32, u32, [u64; 4])> = None;
    for (r0, r1) in tried {
        let steps = levels(r0, r1);
        let nearest = values.map(|v| (0..8u64).min_by_key(|k| steps[*k as usize].abs_diff(v)).unwrap_or(0));
        let worst = (0..4).filter(|v| used[*v]).map(|v| steps[nearest[v] as usize].abs_diff(values[v])).max().unwrap_or(0);
        if best.is_none_or(|(least, ..)| worst < least) {
            best = Some((worst, r0, r1, nearest));
        }
        if worst == 0 {
            break;
        }
    }
    let (_, r0, r1, steps) = best.unwrap_or((0, hi, lo, [0; 4]));
    packed(r0, r1, steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BC4 block as the GPU reads it, to check `bc4_of` with.
    fn read_bc4(block: &[u8]) -> [u8; 16] {
        decode_alpha_block(block)
    }

    /// Every channel of every kind of colour block re-packed as BC4 reads
    /// back as decoding the block to pixels gives it, to two steps in 255.
    #[test]
    fn a_colour_block_s_channel_is_re_packed_as_bc4() {
        let mut seed = 12345u32;
        let mut next = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            seed >> 8
        };
        let mut worst = 0u32;
        for round in 0..4000 {
            let mut block = [0u8; 8];
            for byte in &mut block {
                *byte = next() as u8;
            }
            // Some blocks with one value only, and some with endpoints at
            // the ends of the range.
            if round % 10 == 0 {
                block.copy_within(0..2, 2);
            }
            if round % 7 == 0 {
                block[0..4].copy_from_slice(&[0xff, 0xff, 0x00, 0x00]);
            }
            for punch_through in [false, true] {
                let pixels = decode_color_block(&block, punch_through);
                for (channel, of) in [(Channel::Red, 0usize), (Channel::Green, 1), (Channel::RedInverted, 0)] {
                    let read = read_bc4(&bc4_of(&block, channel, punch_through));
                    for texel in 0..16 {
                        let wanted = if channel == Channel::RedInverted { 255 - pixels[texel][of] as u32 } else { pixels[texel][of] as u32 };
                        // A block of four colours is held to a step or two
                        // (the thirds rounded), and to eight where its ends
                        // are so far apart that a BC4 block's range (255)
                        // cannot hold them at the thirds; one of three
                        // colours and black has its middle colour up to a
                        // tenth of its range off, on the nearest fifth.
                        let off = wanted.abs_diff(read[texel] as u32);
                        let c0 = u16::from_le_bytes([block[0], block[1]]);
                        let c1 = u16::from_le_bytes([block[2], block[3]]);
                        let range = { let ends = [c0, c1].map(|c| rgb565(c)[of]); ends[0].abs_diff(ends[1]) };
                        let allowed = if c0 > c1 || !punch_through { 3 + range / 24 } else { 3 + range / 10 };
                        worst = worst.max(off);
                        assert!(off <= allowed, "round {round}, {channel:?}, texel {texel}: {} for {wanted}", read[texel]);
                    }
                }
            }
        }
        println!("  worst {worst}");
    }

    /// A DXT5 picture re-packed: the alpha block as it was, every level.
    #[test]
    fn a_dxt5_picture_is_re_packed_as_bc5() {
        let mut data = Vec::new();
        // 8x8: four blocks, then one each for 4x4, 2x2 and 1x1.
        for k in 0..7u8 {
            data.extend([200, 10 + k, 0b1010_0101, 0, 0, 0, 0, 0]);
            data.extend(0xf800u16.to_le_bytes());
            data.extend(0x07e0u16.to_le_bytes());
            data.extend([0b1110_0100, 0, 0, 0]);
        }
        let tex = texture(format::DXT5, 8, 8, 4, data.clone());
        let packed = two_channels(&tex, [Channel::Alpha, Channel::Green]).unwrap().unwrap();
        assert_eq!(packed.len(), 7 * 16);
        for k in 0..7 {
            assert_eq!(&packed[k * 16..k * 16 + 8], &data[k * 16..k * 16 + 8], "the alpha block of block {k}");
            // Green: 0 for red (index 0), 255 for green (index 1), and a
            // third and two thirds between.
            let green = read_bc4(&packed[k * 16 + 8..k * 16 + 16]);
            for (got, wanted) in green[0..4].iter().zip([0i32, 255, 85, 170]) {
                assert!((*got as i32 - wanted).abs() <= 1, "block {k}: green {got} for {wanted}");
            }
        }
        // Another format is not done here.
        assert!(two_channels(&texture(format::BC7, 4, 4, 1, vec![0; 16]), [Channel::Red, Channel::Green]).unwrap().is_none());
        assert!(two_channels(&texture(format::DXT1, 4, 4, 1, vec![0; 8]), [Channel::Alpha, Channel::Green]).unwrap().is_none());
    }

    fn texture(fmt: i32, width: u32, height: u32, mip_count: u32, data: Vec<u8>) -> TextureData {
        TextureData { name: String::new(), width, height, format: fmt, mip_count, srgb: false, data, streaming_mipmaps: false, point: false, clamp: [false; 2], faces: 1 }
    }

    #[test]
    fn block_mips_shrink_to_one_block() {
        // 8x8 DXT5: 4 blocks, then one block for each of 4x4, 2x2 and 1x1.
        let tex = texture(format::DXT5, 8, 8, 4, vec![0; 64 + 16 * 3]);
        let sizes: Vec<_> = mips(&tex).unwrap().iter().map(|m| (m.width, m.size)).collect();
        assert_eq!(sizes, [(8, 64), (4, 16), (2, 16), (1, 16)]);
    }

    #[test]
    fn decodes_a_dxt5_block() {
        let mut block = Vec::new();
        // Alpha: endpoints 255 and 0; first texel uses index 1 (the second endpoint).
        block.extend([255, 0, 0b0000_0001, 0, 0, 0, 0, 0]);
        // Colour: pure red and pure blue; first texel index 0, second index 1.
        block.extend(0xf800u16.to_le_bytes());
        block.extend(0x001fu16.to_le_bytes());
        block.extend([0b0000_0100, 0, 0, 0]);
        let tex = texture(format::DXT5, 4, 4, 1, block);
        let rgba = decode_rgba(&tex, mips(&tex).unwrap()[0]).unwrap();
        assert_eq!(&rgba[0..4], [255, 0, 0, 0]);
        assert_eq!(&rgba[4..8], [0, 0, 255, 255]);
    }
}

fn rgb565(c: u16) -> [u32; 3] {
    let (r, g, b) = ((c >> 11) as u32 & 31, (c >> 5) as u32 & 63, c as u32 & 31);
    [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)]
}

pub(crate) fn decode_color_block(block: &[u8], punch_through: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |wa: u32, wb: u32, div: u32| -> [u8; 4] {
        [0, 1, 2, 3].map(|i| if i == 3 { 255 } else { ((a[i] * wa + b[i] * wb) / div) as u8 })
    };
    let palette = if c0 > c1 || !punch_through {
        [mix(1, 0, 1), mix(0, 1, 1), mix(2, 1, 3), mix(1, 2, 3)]
    } else {
        [mix(1, 0, 1), mix(0, 1, 1), mix(1, 1, 2), [0, 0, 0, 0]]
    };
    let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    std::array::from_fn(|i| palette[(bits >> (i * 2)) as usize & 3])
}

pub(crate) fn decode_alpha_block(block: &[u8]) -> [u8; 16] {
    let (a0, a1) = (block[0] as u32, block[1] as u32);
    let mut palette = [a0 as u8, a1 as u8, 0, 0, 0, 0, 0, 255];
    if a0 > a1 {
        for i in 2..8u32 {
            palette[i as usize] = (((8 - i) * a0 + (i - 1) * a1) / 7) as u8;
        }
    } else {
        for i in 2..6u32 {
            palette[i as usize] = (((6 - i) * a0 + (i - 1) * a1) / 5) as u8;
        }
    }
    let mut bits = 0u64;
    for (i, &b) in block[2..8].iter().enumerate() {
        bits |= (b as u64) << (i * 8);
    }
    std::array::from_fn(|i| palette[(bits >> (i * 3)) as usize & 7])
}
