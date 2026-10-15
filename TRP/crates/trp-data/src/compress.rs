//! Pictures compressed as the install's are kept: in blocks of four pixels
//! by four, in formats the graphics card reads as they are. **BC1** (the
//! install's DXT1) holds three channels in half a byte a pixel, a line of
//! four colours a block between two endpoints of 5, 6 and 5 bits; **BC5**
//! holds two channels in a byte a pixel, each a line of eight values a
//! block. A picture of one's own (a weapon's colour, its surface, its
//! normals) is made so, an eighth of the size for the first two and half
//! for the third, where it had been sent to the card as it was read.
//!
//! The encoders are this project's own, from the formats' published
//! description (the same as the install's DXT1 and the decoders in
//! `unity::texture`): a block's endpoints are its colours' main axis
//! through their mean, cut to where they reach, then fitted again by least
//! squares to the steps the colours were given; each pixel takes the
//! nearest step.

/// One level of a picture of four bytes a pixel (red, green, blue and an
/// alpha that is not kept) as BC1, opaque, block after block, row after
/// row of blocks. Width and height need not be multiples of four: a block
/// past the edge repeats the edge's pixels.
pub fn bc1(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    blocks(width, height, 4, rgba, 8, |texels, out| out.extend_from_slice(&bc1_block(&texels.map(|t| [t[0] as f32, t[1] as f32, t[2] as f32]))))
}

/// One level of a picture of two bytes a pixel (red and green) as BC5.
pub fn bc5(width: u32, height: u32, rg: &[u8]) -> Vec<u8> {
    blocks(width, height, 2, rg, 16, |texels, out| {
        out.extend_from_slice(&bc4_block(&texels.map(|t| t[0])));
        out.extend_from_slice(&bc4_block(&texels.map(|t| t[1])));
    })
}

/// One level of BC1 back as four bytes a pixel, as the card reads it: for
/// measuring what compressing kept.
pub fn bc1_decoded(width: u32, height: u32, blocks: &[u8]) -> Vec<u8> {
    decoded(width, height, 4, blocks, 8, |block| crate::unity::texture::decode_color_block(block, true))
}

/// One level of BC5 back as two bytes a pixel.
pub fn bc5_decoded(width: u32, height: u32, blocks: &[u8]) -> Vec<u8> {
    decoded(width, height, 2, blocks, 16, |block| {
        let (red, green) = (crate::unity::texture::decode_alpha_block(&block[..8]), crate::unity::texture::decode_alpha_block(&block[8..]));
        std::array::from_fn(|i| [red[i], green[i], 0, 0])
    })
}

fn decoded(width: u32, height: u32, channels: usize, blocks: &[u8], block_bytes: usize, decode: impl Fn(&[u8]) -> [[u8; 4]; 16]) -> Vec<u8> {
    let across = width.div_ceil(4) as usize;
    let mut out = vec![0u8; width as usize * height as usize * channels];
    for (index, block) in blocks.chunks_exact(block_bytes).enumerate() {
        let (bx, by) = (index % across, index / across);
        for (i, texel) in decode(block).iter().enumerate() {
            let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
            if x < width as usize && y < height as usize {
                let at = (y * width as usize + x) * channels;
                out[at..at + channels].copy_from_slice(&texel[..channels]);
            }
        }
    }
    out
}

/// Bytes of one level of a picture so compressed: a block's bytes for
/// every block, a block for anything less than four pixels each way.
pub fn level_bytes(width: u32, height: u32, block_bytes: usize) -> usize {
    width.div_ceil(4) as usize * height.div_ceil(4) as usize * block_bytes
}

/// Runs `encode` over every block of a level, the rows of blocks shared
/// out among the machine's threads.
fn blocks(width: u32, height: u32, channels: usize, data: &[u8], block_bytes: usize, encode: impl Fn([[u8; 4]; 16], &mut Vec<u8>) + Sync) -> Vec<u8> {
    let (across, down) = (width.div_ceil(4) as usize, height.div_ceil(4) as usize);
    let texel = |x: usize, y: usize| -> [u8; 4] {
        let at = (y.min(height as usize - 1) * width as usize + x.min(width as usize - 1)) * channels;
        std::array::from_fn(|c| if c < channels { data[at + c] } else { 0 })
    };
    let row = |by: usize| -> Vec<u8> {
        let mut out = Vec::with_capacity(across * block_bytes);
        for bx in 0..across {
            encode(std::array::from_fn(|i| texel(bx * 4 + i % 4, by * 4 + i / 4)), &mut out);
        }
        out
    };
    let threads = std::thread::available_parallelism().map_or(1, usize::from).min(down.max(1));
    if threads <= 1 || down < 16 {
        return (0..down).flat_map(row).collect();
    }
    let per = down.div_ceil(threads);
    std::thread::scope(|scope| {
        let parts: Vec<_> = (0..threads).map(|part| scope.spawn(move || (part * per..((part + 1) * per).min(down)).flat_map(row).collect::<Vec<u8>>())).collect();
        parts.into_iter().flat_map(|part| part.join().unwrap_or_default()).collect()
    })
}

/// A colour of 5, 6 and 5 bits, and what the card makes of it.
fn to565(c: [f32; 3]) -> u16 {
    let q = |v: f32, max: f32| (v.clamp(0.0, 255.0) * max / 255.0).round() as u16;
    (q(c[0], 31.0) << 11) | (q(c[1], 63.0) << 5) | q(c[2], 31.0)
}

fn from565(c: u16) -> [f32; 3] {
    let (r, g, b) = ((c >> 11) as u32 & 31, (c >> 5) as u32 & 63, c as u32 & 31);
    [((r << 3) | (r >> 2)) as f32, ((g << 2) | (g >> 4)) as f32, ((b << 3) | (b >> 2)) as f32]
}

/// A block's sixteen colours as BC1, with four colours (the first endpoint
/// the greater): its endpoints and each pixel's step.
fn bc1_block(texels: &[[f32; 3]; 16]) -> [u8; 8] {
    let mean = texels.iter().fold([0.0f32; 3], |m, t| [m[0] + t[0] / 16.0, m[1] + t[1] / 16.0, m[2] + t[2] / 16.0]);
    // The colours' main axis: the covariance's leading direction.
    let mut cov = [0.0f32; 6];
    for t in texels {
        let d = [t[0] - mean[0], t[1] - mean[1], t[2] - mean[2]];
        cov[0] += d[0] * d[0];
        cov[1] += d[0] * d[1];
        cov[2] += d[0] * d[2];
        cov[3] += d[1] * d[1];
        cov[4] += d[1] * d[2];
        cov[5] += d[2] * d[2];
    }
    let mut axis = [1.0f32, 1.0, 1.0];
    for _ in 0..8 {
        let next = [
            cov[0] * axis[0] + cov[1] * axis[1] + cov[2] * axis[2],
            cov[1] * axis[0] + cov[3] * axis[1] + cov[4] * axis[2],
            cov[2] * axis[0] + cov[4] * axis[1] + cov[5] * axis[2],
        ];
        let length = (next[0] * next[0] + next[1] * next[1] + next[2] * next[2]).sqrt();
        if length < 1e-6 {
            break;
        }
        axis = next.map(|v| v / length);
    }
    let along = |t: &[f32; 3]| (t[0] - mean[0]) * axis[0] + (t[1] - mean[1]) * axis[1] + (t[2] - mean[2]) * axis[2];
    let (lo, hi) = texels.iter().map(along).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let at = |s: f32| [mean[0] + axis[0] * s, mean[1] + axis[1] * s, mean[2] + axis[2] * s];
    let first = fit_bc1(texels, at(hi), at(lo));
    // The endpoints again by least squares to the steps given, kept if
    // the block comes out nearer.
    let refit = least_squares(texels, &first.2).map(|(a, b)| fit_bc1(texels, a, b));
    let best = match refit {
        Some(other) if other.3 < first.3 => other,
        _ => first,
    };
    let (c0, c1, steps, _) = best;
    let mut bits = 0u32;
    for (texel, step) in steps.iter().enumerate() {
        bits |= (*step as u32) << (texel * 2);
    }
    let [a, b] = [c0.to_le_bytes(), c1.to_le_bytes()];
    let i = bits.to_le_bytes();
    [a[0], a[1], b[0], b[1], i[0], i[1], i[2], i[3]]
}

/// The endpoints as stored (the first the greater, for four colours), each
/// pixel's step, and the block's squared error.
fn fit_bc1(texels: &[[f32; 3]; 16], a: [f32; 3], b: [f32; 3]) -> (u16, u16, [u8; 16], f32) {
    let (mut c0, mut c1) = (to565(a), to565(b));
    if c0 < c1 {
        std::mem::swap(&mut c0, &mut c1);
    }
    if c0 == c1 {
        // One colour: every pixel on the first endpoint (three-colour mode,
        // whose first step is the endpoint as well).
        let e = from565(c0);
        let error = texels.iter().map(|t| (0..3).map(|c| (t[c] - e[c]).powi(2)).sum::<f32>()).sum();
        return (c0, c1, [0; 16], error);
    }
    let (e0, e1) = (from565(c0), from565(c1));
    let palette = [e0, e1, std::array::from_fn(|c| (2.0 * e0[c] + e1[c]) / 3.0), std::array::from_fn(|c| (e0[c] + 2.0 * e1[c]) / 3.0)];
    let mut steps = [0u8; 16];
    let mut error = 0.0;
    for (texel, t) in texels.iter().enumerate() {
        let (step, distance) = palette.iter().enumerate().map(|(i, p)| (i, (0..3).map(|c| (t[c] - p[c]).powi(2)).sum::<f32>())).fold((0, f32::MAX), |best, this| if this.1 < best.1 { this } else { best });
        steps[texel] = step as u8;
        error += distance;
    }
    (c0, c1, steps, error)
}

/// The two endpoints that, by the steps given (each a share of the first
/// endpoint: 1, 0, 2/3 or 1/3), come nearest the colours.
fn least_squares(texels: &[[f32; 3]; 16], steps: &[u8; 16]) -> Option<([f32; 3], [f32; 3])> {
    const SHARE: [f32; 4] = [1.0, 0.0, 2.0 / 3.0, 1.0 / 3.0];
    let (mut aa, mut ab, mut bb) = (0.0f32, 0.0f32, 0.0f32);
    let (mut xa, mut xb) = ([0.0f32; 3], [0.0f32; 3]);
    for (t, step) in texels.iter().zip(steps) {
        let w = SHARE[*step as usize];
        aa += w * w;
        ab += w * (1.0 - w);
        bb += (1.0 - w) * (1.0 - w);
        for c in 0..3 {
            xa[c] += w * t[c];
            xb[c] += (1.0 - w) * t[c];
        }
    }
    let det = aa * bb - ab * ab;
    if det.abs() < 1e-6 {
        return None;
    }
    Some((std::array::from_fn(|c| (bb * xa[c] - ab * xb[c]) / det), std::array::from_fn(|c| (aa * xb[c] - ab * xa[c]) / det)))
}

/// A block's sixteen values as BC4, with eight steps (the first endpoint
/// the greater): its greatest and least, and each pixel's nearest step.
fn bc4_block(values: &[u8; 16]) -> [u8; 8] {
    let (hi, lo) = values.iter().fold((0u8, 255u8), |(hi, lo), v| (hi.max(*v), lo.min(*v)));
    if hi == lo {
        // One value: every pixel on the first endpoint.
        return [hi, lo, 0, 0, 0, 0, 0, 0];
    }
    let (a0, a1) = (hi as u32, lo as u32);
    let palette: [u32; 8] = std::array::from_fn(|i| match i {
        0 => a0,
        1 => a1,
        _ => ((8 - i as u32) * a0 + (i as u32 - 1) * a1) / 7,
    });
    let mut bits = 0u64;
    for (texel, v) in values.iter().enumerate() {
        let step = (0..8).min_by_key(|i| (palette[*i] as i32 - *v as i32).abs()).unwrap_or(0) as u64;
        bits |= step << (texel * 3);
    }
    let i = bits.to_le_bytes();
    [hi, lo, i[0], i[1], i[2], i[3], i[4], i[5]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unity::texture::{decode_alpha_block, decode_color_block};

    /// Root mean square difference, per channel, between a picture and
    /// what its blocks decode to.
    fn rms(original: &[[u8; 4]], decoded: &[[u8; 4]], channels: usize) -> f32 {
        let sum: f32 = original.iter().zip(decoded).map(|(a, b)| (0..channels).map(|c| (a[c] as f32 - b[c] as f32).powi(2)).sum::<f32>()).sum();
        (sum / (original.len() * channels) as f32).sqrt()
    }

    /// A picture of 16 by 16: smooth ramps of every channel, with a little
    /// of each texel's own.
    fn ramps() -> Vec<u8> {
        (0..256u32).flat_map(|i| {
            let (x, y) = (i % 16, i / 16);
            [(x * 15 + (i * 7) % 5) as u8, (y * 15) as u8, ((x + y) * 7 + (i * 3) % 4) as u8, 255]
        })
        .collect()
    }

    /// A picture of 16 by 16 whose colour runs from one to another across
    /// it, with a little of each texel's own: what a block of a real
    /// picture mostly is, colours along a line.
    fn gradient() -> Vec<u8> {
        (0..256u32).flat_map(|i| {
            let t = (i % 16 + (i / 16) * 3) as f32 / 60.0;
            let noise = ((i * 7) % 5) as f32;
            [30.0 + 190.0 * t + noise, 200.0 - 120.0 * t + noise, 60.0 + 40.0 * t, 255.0].map(|v| v.clamp(0.0, 255.0) as u8)
        })
        .collect()
    }

    /// A picture's BC1 blocks decoded.
    fn bc1_decoded(packed: &[u8]) -> Vec<[u8; 4]> {
        let mut decoded = vec![[0u8; 4]; 256];
        for (block, bytes) in packed.chunks_exact(8).enumerate() {
            for (i, t) in decode_color_block(bytes, true).iter().enumerate() {
                decoded[((block / 4) * 4 + i / 4) * 16 + (block % 4) * 4 + i % 4] = *t;
            }
        }
        decoded
    }

    /// Colour along a line comes back within a step or two in 255; colour
    /// spread over a plane, which no block of a line holds, no worse than
    /// with each block's endpoints at its extremes; a single colour as its
    /// nearest of 5, 6 and 5 bits; and a picture whose sides are not
    /// multiples of four takes a whole block for its edge.
    #[test]
    fn bc1_keeps_colour_close() {
        let as_texels = |p: &[u8]| -> Vec<[u8; 4]> { p.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect() };
        // Each block's endpoints at its extremes, the simplest encoder.
        let naive = |picture: &[u8]| -> Vec<u8> {
            blocks(16, 16, 4, picture, 8, |texels, out| {
                let colours = texels.map(|t| [t[0] as f32, t[1] as f32, t[2] as f32]);
                let (lo, hi) = colours.iter().fold(([255.0f32; 3], [0.0f32; 3]), |(lo, hi), c| (std::array::from_fn(|i| lo[i].min(c[i])), std::array::from_fn(|i| hi[i].max(c[i]))));
                let (c0, c1, steps, _) = fit_bc1(&colours, hi, lo);
                let bits = steps.iter().enumerate().fold(0u32, |bits, (i, s)| bits | (*s as u32) << (i * 2));
                out.extend(c0.to_le_bytes().into_iter().chain(c1.to_le_bytes()).chain(bits.to_le_bytes()));
            })
        };
        for (picture, at_most) in [(gradient(), 3.5), (ramps(), 12.0)] {
            let packed = bc1(16, 16, &picture);
            assert_eq!(packed.len(), level_bytes(16, 16, 8));
            let ours = rms(&as_texels(&picture), &bc1_decoded(&packed), 3);
            let theirs = rms(&as_texels(&picture), &bc1_decoded(&naive(&picture)), 3);
            println!("BC1: {ours:.2} in 255, the extremes' {theirs:.2}");
            assert!(ours <= theirs && ours < at_most, "{ours} against {theirs}");
        }
        let solid = bc1(4, 4, &[200, 100, 50, 255].repeat(16));
        let texel = decode_color_block(&solid, true)[0];
        assert!(texel.iter().take(3).zip([200, 100, 50]).all(|(a, b)| (*a as i32 - b).abs() <= 4), "{texel:?}");
        assert_eq!(bc1(6, 5, &[0; 6 * 5 * 4]).len(), level_bytes(6, 5, 8));
    }

    /// Two channels come back within a step or two, each its own.
    #[test]
    fn bc5_keeps_two_channels_close() {
        let picture: Vec<u8> = ramps().chunks_exact(4).flat_map(|p| [p[0], p[1]]).collect();
        let packed = bc5(16, 16, &picture);
        assert_eq!(packed.len(), level_bytes(16, 16, 16));
        let mut worst = 0;
        for (block, bytes) in packed.chunks_exact(16).enumerate() {
            let (red, green) = (decode_alpha_block(&bytes[..8]), decode_alpha_block(&bytes[8..]));
            for i in 0..16 {
                let at = ((block / 4) * 4 + i / 4) * 16 + (block % 4) * 4 + i % 4;
                worst = worst.max((red[i] as i32 - picture[at * 2] as i32).abs()).max((green[i] as i32 - picture[at * 2 + 1] as i32).abs());
            }
        }
        assert!(worst <= 4, "{worst}");
    }
}
