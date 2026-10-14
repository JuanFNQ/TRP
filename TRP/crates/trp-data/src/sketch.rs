//! A plain picture of triangles seen straight on, for measuring by eye
//! what the numbers are taken from: a weapon's model from its side or
//! from above, with a grid. Nothing of the game's; `probe` draws with it.

use std::path::Path;

use anyhow::Result;

pub struct Sketch {
    pub width: usize,
    pub height: usize,
    pixels: Vec<[u8; 3]>,
    depth: Vec<f32>,
}

impl Sketch {
    pub fn new(width: usize, height: usize, background: [u8; 3]) -> Self {
        Self { width, height, pixels: vec![background; width * height], depth: vec![f32::MIN; width * height] }
    }

    /// Fills a triangle given in the picture's own space: x to the right
    /// and y down in pixels, z towards the eye. It is lit by how squarely
    /// it faces the eye. With `alpha` under 1 it is laid over what is
    /// there and hides nothing drawn after it.
    pub fn triangle(&mut self, corners: [[f32; 3]; 3], colour: [u8; 3], alpha: f32) {
        let [a, b, c] = corners;
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if area.abs() < 1e-6 {
            return;
        }
        // How squarely it faces the eye, from its normal.
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let normal = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let length = normal.iter().map(|n| n * n).sum::<f32>().sqrt().max(1e-9);
        let light = 0.35 + 0.65 * (normal[2] / length).abs();
        let lit = colour.map(|channel| (channel as f32 * light) as u8);
        let span = |k: usize, most: usize| {
            let low = a[k].min(b[k]).min(c[k]).floor().max(0.0) as usize;
            let high = (a[k].max(b[k]).max(c[k]).ceil().max(0.0) as usize).min(most.saturating_sub(1));
            low..=high
        };
        for y in span(1, self.height) {
            for x in span(0, self.width) {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b[0] - px) * (c[1] - py) - (c[0] - px) * (b[1] - py)) / area;
                let w1 = ((c[0] - px) * (a[1] - py) - (a[0] - px) * (c[1] - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
                let at = y * self.width + x;
                if alpha < 1.0 {
                    let under = self.pixels[at];
                    self.pixels[at] = std::array::from_fn(|k| (under[k] as f32 * (1.0 - alpha) + lit[k] as f32 * alpha) as u8);
                } else if z > self.depth[at] {
                    self.depth[at] = z;
                    self.pixels[at] = lit;
                }
            }
        }
    }

    /// A line a pixel wide over everything, from one place to another.
    pub fn line(&mut self, from: [f32; 2], to: [f32; 2], colour: [u8; 3]) {
        let steps = (to[0] - from[0]).abs().max((to[1] - from[1]).abs()).ceil().max(1.0) as usize;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let (x, y) = (from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t);
            if x >= 0.0 && y >= 0.0 && (x as usize) < self.width && (y as usize) < self.height {
                self.pixels[y as usize * self.width + x as usize] = colour;
            }
        }
    }

    /// A small cross at a place, to mark a point.
    pub fn mark(&mut self, at: [f32; 2], colour: [u8; 3]) {
        for arm in 0..2 {
            for thick in -1..=1 {
                let (dx, dy) = if arm == 0 { (7.0, thick as f32) } else { (thick as f32, 7.0) };
                let (ox, oy) = if arm == 0 { (0.0, thick as f32) } else { (thick as f32, 0.0) };
                self.line([at[0] - dx + ox, at[1] - dy + oy], [at[0] + dx + ox, at[1] + dy + oy], colour);
            }
        }
    }

    /// Writes the picture as a PNG file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut raw = Vec::with_capacity((self.width * 3 + 1) * self.height);
        for row in self.pixels.chunks_exact(self.width) {
            raw.push(0);
            row.iter().for_each(|pixel| raw.extend_from_slice(pixel));
        }
        let mut file = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut chunk = |kind: &[u8; 4], data: &[u8]| {
            file.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let start = file.len();
            file.extend_from_slice(kind);
            file.extend_from_slice(data);
            let sum = crc32(&file[start..]);
            file.extend_from_slice(&sum.to_be_bytes());
        };
        let mut header = Vec::new();
        header.extend_from_slice(&(self.width as u32).to_be_bytes());
        header.extend_from_slice(&(self.height as u32).to_be_bytes());
        // Eight bits a channel, red green blue, no interlace.
        header.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(b"IHDR", &header);
        chunk(b"IDAT", &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6));
        chunk(b"IEND", &[]);
        if let Some(folder) = path.parent().filter(|folder| !folder.as_os_str().is_empty()) {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(path, file)?;
        Ok(())
    }
}

/// A triangle to draw: its corners as centimetres ahead, up and to the
/// right of the picture's origin, its colour, and how solid it is.
pub type Drawn = ([[f32; 3]; 3], [u8; 3], f32);

/// Draws triangles from the left side (the muzzle to the left) and, under
/// that, from above, each over a grid of centimetres: a faint line every
/// centimetre, a clearer one every five, a strong one every ten, and the
/// two axes through the origin. `marks` are points to cross and `lines`
/// lines to draw from one point to another, both over everything.
pub fn views(triangles: &[Drawn], marks: &[([f32; 3], [u8; 3])], lines: &[([f32; 3], [f32; 3], [u8; 3])], out: &str, scale: f32, window: Option<[f32; 4]>) -> Result<()> {
    // What is drawn: so far ahead of the origin and behind it, above it
    // and below it, and to either side in the view from above.
    let [ahead, behind, above, below] = window.unwrap_or([70.0, 55.0, 28.0, 28.0]);
    let aside = 14.0f32;
    let width = ((ahead + behind) * scale) as usize;
    let (side_height, top_height) = (((above + below) * scale) as usize, (2.0 * aside * scale) as usize);
    let mut sketch = Sketch::new(width, side_height + top_height, [250, 250, 248]);
    let origin_x = ahead * scale;
    let (side_y, top_y) = (above * scale, side_height as f32 + aside * scale);
    let side = |p: [f32; 3]| [origin_x - p[0] * scale, side_y - p[1] * scale, -p[2]];
    let top = |p: [f32; 3]| [origin_x - p[0] * scale, top_y - p[2] * scale, p[1]];
    // The opaque ones first, each view clipped to its own part.
    for pass in [true, false] {
        for (corners, colour, alpha) in triangles.iter().filter(|t| (t.2 >= 1.0) == pass) {
            let s = corners.map(side);
            if s.iter().all(|c| c[1] < side_height as f32) {
                sketch.triangle(s, *colour, *alpha);
            }
            let t = corners.map(top);
            if t.iter().all(|c| c[1] >= side_height as f32) {
                sketch.triangle(t, *colour, *alpha);
            }
        }
    }
    let shade = |cm: i32| match (cm % 10 == 0, cm % 5 == 0) {
        _ if cm == 0 => [40, 40, 200],
        (true, _) => [120, 120, 120],
        (_, true) => [180, 180, 180],
        _ => [225, 225, 222],
    };
    // A window drawn large has a line every centimetre.
    let every = if scale >= 24.0 { 1 } else { 5 };
    for cm in -(behind as i32)..=(ahead as i32) {
        let x = origin_x - cm as f32 * scale;
        if cm % every == 0 {
            sketch.line([x, 0.0], [x, (side_height + top_height) as f32], shade(cm));
        }
    }
    for cm in -(below as i32)..=(above as i32) {
        if cm % every == 0 {
            sketch.line([0.0, side_y - cm as f32 * scale], [width as f32, side_y - cm as f32 * scale], shade(cm));
        }
    }
    for cm in -(aside as i32)..=(aside as i32) {
        if cm % every == 0 {
            sketch.line([0.0, top_y - cm as f32 * scale], [width as f32, top_y - cm as f32 * scale], shade(cm));
        }
    }
    sketch.line([0.0, side_height as f32], [width as f32, side_height as f32], [0, 0, 0]);
    for (at, colour) in marks {
        let (s, t) = (side(*at), top(*at));
        sketch.mark([s[0], s[1]], *colour);
        sketch.mark([t[0], t[1]], *colour);
    }
    for (from, to, colour) in lines {
        for view in [0, 1] {
            let (a, b) = if view == 0 { (side(*from), side(*to)) } else { (top(*from), top(*to)) };
            for thick in [-0.5f32, 0.5] {
                sketch.line([a[0] + thick, a[1] + thick], [b[0] + thick, b[1] + thick], *colour);
            }
        }
    }
    sketch.save(Path::new(out))?;
    println!("{out}: {width} x {} pixels, {scale} to a centimetre; the side view's origin at ({origin_x}, {side_y}), the top view's at ({origin_x}, {top_y}); ahead is to the left", side_height + top_height);
    Ok(())
}

/// The check sum a PNG file's chunks carry.
fn crc32(data: &[u8]) -> u32 {
    let mut sum = !0u32;
    for &byte in data {
        sum ^= byte as u32;
        for _ in 0..8 {
            sum = if sum & 1 == 1 { (sum >> 1) ^ 0xedb8_8320 } else { sum >> 1 };
        }
    }
    !sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_check_sum_is_the_formats() {
        // The standard's own check value.
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn the_nearer_triangle_is_the_one_seen() {
        let mut sketch = Sketch::new(8, 8, [0; 3]);
        let over = |z: f32| [[0.0, 0.0, z], [8.0, 0.0, z], [0.0, 8.0, z]];
        sketch.triangle(over(1.0), [200, 0, 0], 1.0);
        sketch.triangle(over(0.0), [0, 200, 0], 1.0);
        assert_eq!(sketch.pixels[9], [200, 0, 0]);
    }
}
