//! BC7 blocks decoded, and a BC7 picture made BC5 block by block.
//! BC7 is also called BPTC.
//!
//! From the format's published description (the Khronos Data Format
//! Specification, "BC7"; Microsoft's "BC7 Format"): the eight modes, the
//! partitions of two and three subsets and their anchors are the
//! description's tables. A block is read as one 128-bit number and its
//! fields are taken from it by shifts, where `texture2ddecoder` copies a
//! few bytes for every field it reads: decoded the same, pixel for pixel
//! (the tests), in a fraction of the time.
//!
//! The install packs some of its normal and gloss pictures as BC7, which the
//! material cannot read as they are (`texture::two_channels`): normals with
//! X in alpha and Y in green, gloss as grey. `to_bc5` keeps the two channels
//! the material reads as BC5, a byte a pixel as the BC7 was: every block of
//! every level decoded and made two BC4 blocks at once, the levels' blocks in
//! the same order in both formats, so that the whole picture is one run of
//! blocks shared among threads.

/// One mode's layout: subsets, then the bits of each field.
struct Mode {
    subsets: u32,
    partition: u32,
    rotation: u32,
    selector: u32,
    color: u32,
    alpha: u32,
    /// A p-bit for each endpoint, or one for each subset (shared).
    endpoint_pbits: bool,
    shared_pbits: bool,
    /// The bits of an index, and of the second set of indices (modes 4, 5).
    index: u32,
    index2: u32,
}

const fn mode(subsets: u32, partition: u32, rotation: u32, selector: u32, color: u32, alpha: u32, endpoint_pbits: bool, shared_pbits: bool, index: u32, index2: u32) -> Mode {
    Mode { subsets, partition, rotation, selector, color, alpha, endpoint_pbits, shared_pbits, index, index2 }
}

const MODES: [Mode; 8] = [
    mode(3, 4, 0, 0, 4, 0, true, false, 3, 0),
    mode(2, 6, 0, 0, 6, 0, false, true, 3, 0),
    mode(3, 6, 0, 0, 5, 0, false, false, 2, 0),
    mode(2, 6, 0, 0, 7, 0, true, false, 2, 0),
    mode(1, 0, 2, 1, 5, 6, false, false, 2, 3),
    mode(1, 0, 2, 0, 7, 8, false, false, 2, 2),
    mode(1, 0, 0, 0, 7, 7, true, false, 4, 0),
    mode(2, 6, 0, 0, 5, 5, true, false, 2, 0),
];

/// The weights of an index of 2, 3 and 4 bits, in 64ths of the way from the
/// first endpoint to the second.
const WEIGHTS: [[u32; 16]; 5] = [
    [0; 16],
    [0; 16],
    [0, 21, 43, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 9, 18, 27, 37, 46, 55, 64, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64],
];

/// The partitions of two subsets: bit `i`, pixel `i`'s subset.
const PARTITIONS2: [u16; 64] = [
    0xcccc, 0x8888, 0xeeee, 0xecc8, 0xc880, 0xfeec, 0xfec8, 0xec80, 0xc800, 0xffec, 0xfe80, 0xe800, 0xffe8, 0xff00, 0xfff0, 0xf000,
    0xf710, 0x008e, 0x7100, 0x08ce, 0x008c, 0x7310, 0x3100, 0x8cce, 0x088c, 0x3110, 0x6666, 0x366c, 0x17e8, 0x0ff0, 0x718e, 0x399c,
    0xaaaa, 0xf0f0, 0x5a5a, 0x33cc, 0x3c3c, 0x55aa, 0x9696, 0xa55a, 0x73ce, 0x13c8, 0x324c, 0x3bdc, 0x6996, 0xc33c, 0x9966, 0x0660,
    0x0272, 0x04e4, 0x4e40, 0x2720, 0xc936, 0x936c, 0x39c6, 0x639c, 0x9336, 0x9cc6, 0x817e, 0xe718, 0xccf0, 0x0fcc, 0x7744, 0xee22,
];

/// The partitions of three subsets: bits `2i` and `2i + 1`, pixel `i`'s.
const PARTITIONS3: [u32; 64] = [
    0xaa685050, 0x6a5a5040, 0x5a5a4200, 0x5450a0a8, 0xa5a50000, 0xa0a05050, 0x5555a0a0, 0x5a5a5050,
    0xaa550000, 0xaa555500, 0xaaaa5500, 0x90909090, 0x94949494, 0xa4a4a4a4, 0xa9a59450, 0x2a0a4250,
    0xa5945040, 0x0a425054, 0xa5a5a500, 0x55a0a0a0, 0xa8a85454, 0x6a6a4040, 0xa4a45000, 0x1a1a0500,
    0x0050a4a4, 0xaaa59090, 0x14696914, 0x69691400, 0xa08585a0, 0xaa821414, 0x50a4a450, 0x6a5a0200,
    0xa9a58000, 0x5090a0a8, 0xa8a09050, 0x24242424, 0x00aa5500, 0x24924924, 0x24499224, 0x50a50a50,
    0x500aa550, 0xaaaa4444, 0x66660000, 0xa5a0a5a0, 0x50a050a0, 0x69286928, 0x44aaaa44, 0x66666600,
    0xaa444444, 0x54a854a8, 0x95809580, 0x96969600, 0xa85454a8, 0x80959580, 0xaa141414, 0x96960000,
    0xaaaa1414, 0xa05050a0, 0xa0a5a5a0, 0x96000000, 0x40804080, 0xa9a8a9a8, 0xaaaaaa44, 0x2a4a5254,
];

/// The anchor of the second subset of two; of the second and third of three.
/// (The first subset's anchor is always pixel 0.)
const ANCHORS2: [u8; 64] = [
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 2, 8, 2, 2, 8, 8, 15, 2, 8, 2, 2, 8, 8, 2, 2,
    15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2, 2, 15, 15, 6, 6, 2, 6, 8, 15, 15, 2, 2, 15, 15, 15, 15, 15, 2, 2, 15,
];
const ANCHORS3: [[u8; 64]; 2] = [
    [
        3, 3, 15, 15, 8, 3, 15, 15, 8, 8, 6, 6, 6, 5, 3, 3, 3, 3, 8, 15, 3, 3, 6, 10, 5, 8, 8, 6, 8, 5, 15, 15,
        8, 15, 3, 5, 6, 10, 8, 15, 15, 3, 15, 5, 15, 15, 15, 15, 3, 15, 5, 5, 5, 8, 5, 10, 5, 10, 8, 13, 15, 12, 3, 3,
    ],
    [
        15, 8, 8, 3, 15, 15, 3, 8, 15, 15, 15, 15, 15, 15, 15, 8, 15, 8, 15, 3, 15, 8, 15, 8, 3, 15, 6, 10, 15, 15, 10, 8,
        15, 3, 15, 10, 10, 8, 9, 10, 6, 15, 8, 15, 3, 6, 6, 8, 15, 3, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 3, 15, 15, 8,
    ],
];

/// One block's sixteen pixels, red, green, blue and alpha, row after row. A
/// block of no mode (its first byte 0) is all 0, as the card reads it.
pub fn decode_block(block: &[u8; 16]) -> [[u8; 4]; 16] {
    let bits = u128::from_le_bytes(*block);
    let number = block[0].trailing_zeros() as usize;
    if number >= 8 {
        return [[0; 4]; 16];
    }
    let m = &MODES[number];
    let mut at = number as u32 + 1;
    let mut take = |n: u32| -> u32 {
        let value = (bits >> at) as u32 & ((1u32 << n) - 1);
        at += n;
        value
    };
    let partition = take(m.partition) as usize;
    let rotation = take(m.rotation);
    let selector = take(m.selector);
    let ends = (m.subsets * 2) as usize;
    let mut raw = [[0u32; 4]; 6];
    for channel in 0..3 {
        for end in raw.iter_mut().take(ends) {
            end[channel] = take(m.color);
        }
    }
    if m.alpha > 0 {
        for end in raw.iter_mut().take(ends) {
            end[3] = take(m.alpha);
        }
    }
    let (mut color_bits, mut alpha_bits) = (m.color, m.alpha);
    if m.endpoint_pbits || m.shared_pbits {
        let mut shared = 0;
        for (index, end) in raw.iter_mut().take(ends).enumerate() {
            let p = if m.endpoint_pbits || index % 2 == 0 { take(1) } else { shared };
            shared = p;
            for value in end.iter_mut() {
                *value = *value << 1 | p;
            }
        }
        color_bits += 1;
        if alpha_bits > 0 {
            alpha_bits += 1;
        }
    }
    // Each endpoint to eight bits, its high bits repeated below.
    let expand = |value: u32, n: u32| -> u32 {
        let value = value << (8 - n);
        value | value >> n
    };
    let mut endpoints = [[0u32; 4]; 6];
    for (out, raw) in endpoints.iter_mut().zip(&raw).take(ends) {
        for channel in 0..3 {
            out[channel] = expand(raw[channel], color_bits);
        }
        out[3] = if m.alpha > 0 { expand(raw[3], alpha_bits) } else { 255 };
    }

    // Each pixel's subset, and the anchors (an index of one bit fewer).
    let subset_of = |pixel: usize| -> usize {
        match m.subsets {
            2 => (PARTITIONS2[partition] >> pixel) as usize & 1,
            3 => (PARTITIONS3[partition] >> (2 * pixel)) as usize & 3,
            _ => 0,
        }
    };
    let (second, third) = match m.subsets {
        2 => (ANCHORS2[partition] as usize, 0),
        3 => (ANCHORS3[0][partition] as usize, ANCHORS3[1][partition] as usize),
        _ => (0, 0),
    };
    let indices = |at: u32, n: u32, anchors: bool| -> [u32; 16] {
        let mut word = (bits >> at) as u64;
        std::array::from_fn(|pixel| {
            let anchor = pixel == 0 || (anchors && (pixel == second || pixel == third));
            let width = n - anchor as u32;
            let index = word as u32 & ((1u32 << width) - 1);
            word >>= width;
            index
        })
    };
    let primary = indices(at, m.index, m.subsets > 1);
    let secondary = if m.index2 > 0 { indices(at + 16 * m.index - 1, m.index2, false) } else { primary };
    let (color_index, color_n, alpha_index, alpha_n) = match (m.index2, selector) {
        (0, _) => (&primary, m.index, &primary, m.index),
        (_, 0) => (&primary, m.index, &secondary, m.index2),
        _ => (&secondary, m.index2, &primary, m.index),
    };
    std::array::from_fn(|pixel| {
        let subset = subset_of(pixel);
        let (first, last) = (&endpoints[2 * subset], &endpoints[2 * subset + 1]);
        let wc = WEIGHTS[color_n as usize][color_index[pixel] as usize];
        let wa = WEIGHTS[alpha_n as usize][alpha_index[pixel] as usize];
        let mix = |channel: usize, w: u32| ((64 - w) * first[channel] + w * last[channel] + 32) >> 6;
        let mut out = [mix(0, wc) as u8, mix(1, wc) as u8, mix(2, wc) as u8, mix(3, wa) as u8];
        match rotation {
            1 => out.swap(0, 3),
            2 => out.swap(1, 3),
            3 => out.swap(2, 3),
            _ => {}
        }
        out
    })
}

/// A BC7 picture's blocks (every level's, as they lie) as BC5: of each
/// pixel, the two channels `pick` takes, each channel a BC4 block
/// (`compress::bc4_block`), block for block, over up to `threads` threads.
/// Data not of whole blocks has its last part left out.
pub fn to_bc5(data: &[u8], pick: impl Fn([u8; 4]) -> [u8; 2] + Sync, threads: usize) -> Vec<u8> {
    let blocks = data.len() / 16;
    let mut out = vec![0u8; blocks * 16];
    let one = |from: &[u8], to: &mut [u8]| {
        for (block, made) in from.chunks_exact(16).zip(to.chunks_exact_mut(16)) {
            let pixels = decode_block(block.try_into().unwrap_or(&[0; 16]));
            let two = pixels.map(&pick);
            made[..8].copy_from_slice(&bc4_block(&two.map(|p| p[0])));
            made[8..].copy_from_slice(&bc4_block(&two.map(|p| p[1])));
        }
    };
    // Below some thousands of blocks a thread costs more than it spares.
    let threads = threads.clamp(1, blocks.div_ceil(4096).max(1));
    if threads == 1 {
        one(&data[..blocks * 16], &mut out);
        return out;
    }
    let per = blocks.div_ceil(threads) * 16;
    std::thread::scope(|scope| {
        for (from, to) in data[..blocks * 16].chunks(per).zip(out.chunks_mut(per)) {
            scope.spawn(move || one(from, to));
        }
    });
    out
}

/// Sixteen values as a BC4 block: its endpoints the largest and the
/// smallest, the eight steps between them as the card makes them, and each
/// value on its nearest step. The step is found by how many of the seven
/// midpoints between steps the value reaches, where `compress::bc4_block`
/// measures it against all eight: the same values read back, in a fraction
/// of the time.
fn bc4_block(values: &[u8; 16]) -> [u8; 8] {
    let (hi, lo) = values.iter().fold((0u8, 255u8), |(hi, lo), v| (hi.max(*v), lo.min(*v)));
    if hi == lo {
        return [hi, lo, 0, 0, 0, 0, 0, 0];
    }
    let (low, range) = (lo as u32, (hi - lo) as u32);
    // The steps from the smallest up: the card's ((8 - i) * hi + (i - 1) *
    // lo) / 7 is the smallest and `k` sevenths of the range, rounded down.
    let steps: [u32; 8] = std::array::from_fn(|k| low + k as u32 * range / 7);
    let midpoints: [u32; 7] = std::array::from_fn(|k| (steps[k] + steps[k + 1] + 1) / 2);
    // A step's index in the block: the largest is the first endpoint (0),
    // the smallest the second (1), those between 7 down to 2.
    const INDEX: [u64; 8] = [1, 7, 6, 5, 4, 3, 2, 0];
    let mut bits = 0u64;
    for (texel, value) in values.iter().enumerate() {
        let step = midpoints.iter().map(|m| (*value as u32 >= *m) as usize).sum::<usize>();
        bits |= INDEX[step] << (texel * 3);
    }
    let i = bits.to_le_bytes();
    [hi, lo, i[0], i[1], i[2], i[3], i[4], i[5]]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quick BC4 block reads back as near as the full search's, value
    /// for value.
    #[test]
    fn bc4_is_as_near_as_the_full_search() {
        let mut seed = 99u32;
        for round in 0..200_000 {
            let values: [u8; 16] = std::array::from_fn(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (seed >> 16) as u8
            });
            // Narrow ranges too, where steps repeat.
            let values = if round % 3 == 0 { values.map(|v| 100 + v % 5) } else { values };
            let quick = crate::unity::texture::decode_alpha_block(&bc4_block(&values));
            let full = crate::unity::texture::decode_alpha_block(&crate::formats::compress::bc4_block(&values));
            for ((q, f), v) in quick.iter().zip(full).zip(values) {
                assert_eq!(q.abs_diff(v), f.abs_diff(v), "round {round}: {values:?}");
            }
        }
    }

    fn reference(block: &[u8; 16]) -> [[u8; 4]; 16] {
        let mut pixels = [0u32; 16];
        texture2ddecoder::decode_bc7_block(block, &mut pixels);
        pixels.map(|p| {
            let [b, g, r, a] = p.to_le_bytes();
            [r, g, b, a]
        })
    }

    /// Every mode, every partition, rotation and selector: random blocks
    /// decoded as `texture2ddecoder` decodes them, pixel for pixel.
    #[test]
    fn decodes_as_the_reference_does() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut by_mode = [0u32; 9];
        for round in 0..400_000 {
            let mut block = [0u8; 16];
            block[..8].copy_from_slice(&next().to_le_bytes());
            block[8..].copy_from_slice(&next().to_le_bytes());
            // Each mode as often as the others; now and then a block of none.
            let number = round % 9;
            block[0] = if number == 8 { 0 } else { (block[0] & !(((2u16 << number) - 1) as u8)) | (1 << number) };
            by_mode[number] += 1;
            assert_eq!(decode_block(&block), reference(&block), "mode {number}, block {block:02x?}");
        }
        assert!(by_mode.iter().all(|n| *n > 40_000));
    }

    /// A picture made BC5: each block's two channels as BC4 within a
    /// fourteenth of the block's range, the same over threads as over one.
    #[test]
    fn makes_bc5_of_two_channels() {
        let mut data = Vec::new();
        let mut seed = 7u32;
        for _ in 0..20_000 * 16 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            data.push((seed >> 16) as u8);
        }
        let pick = |p: [u8; 4]| [p[3], p[1]];
        let one = to_bc5(&data, pick, 1);
        assert_eq!(one, to_bc5(&data, pick, 8));
        for (block, made) in data.chunks_exact(16).zip(one.chunks_exact(16)) {
            let pixels = decode_block(block.try_into().unwrap());
            for (half, channel) in [(0usize, 3usize), (1, 1)] {
                let read = crate::unity::texture::decode_alpha_block(&made[half * 8..half * 8 + 8]);
                let values = pixels.map(|p| p[channel]);
                let range = values.iter().max().unwrap() - values.iter().min().unwrap();
                for (got, wanted) in read.iter().zip(values) {
                    assert!((got.abs_diff(wanted) as u32) * 14 <= range as u32 + 14, "{got} for {wanted}, range {range}");
                }
            }
        }
    }
}
