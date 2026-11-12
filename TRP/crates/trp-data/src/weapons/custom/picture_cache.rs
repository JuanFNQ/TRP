//! A custom weapon's pictures, their compression and the kept cache.

use super::*;

/// A picture ready to be drawn with: its rows from the bottom up, as the
/// install's own are kept, and every smaller size after the first.
#[derive(Debug, Clone)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    /// Bytes to a pixel: four, or two for a normal picture's red and
    /// green.
    pub channels: usize,
    pub data: Vec<u8>,
    /// How it is compressed, if it is (`Pictures::compressed`): its data is
    /// then blocks, every level's after the one before.
    pub blocks: Option<Blocks>,
}

/// A picture's compression, as the graphics card reads it (`compress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocks {
    /// Three channels, half a byte a pixel (four bytes a pixel before).
    Bc1,
    /// Two channels, a byte a pixel (two before).
    Bc5,
}

impl Picture {
    /// Compressed as the install's pictures are: four channels as BC1 (the
    /// fourth is not kept: a weapon's colour and surface have none), two
    /// as BC5; every level. One whose sides are not multiples of four is
    /// left as it is (the card takes compressed pictures in whole blocks).
    pub fn compressed(self) -> Picture {
        let (blocks, encode): (Blocks, fn(u32, u32, &[u8]) -> Vec<u8>) = match self.channels {
            4 => (Blocks::Bc1, crate::formats::compress::bc1),
            2 => (Blocks::Bc5, crate::formats::compress::bc5),
            _ => return self,
        };
        if self.blocks.is_some() || self.width % 4 != 0 || self.height % 4 != 0 {
            return self;
        }
        let mut data = Vec::new();
        let (mut width, mut height, mut at) = (self.width, self.height, 0);
        for _ in 0..self.mips {
            let size = width as usize * height as usize * self.channels;
            data.extend(encode(width, height, &self.data[at..at + size]));
            at += size;
            (width, height) = ((width / 2).max(1), (height / 2).max(1));
        }
        Picture { data, blocks: Some(blocks), ..self }
    }
}

/// A material's pictures: its colour; its normals; and one of its
/// occlusion (red), its roughness (green) and its metal (blue).
#[derive(Debug, Clone, Default)]
pub struct Pictures {
    pub colour: Option<Picture>,
    pub normal: Option<Picture>,
    pub surface: Option<Picture>,
}

impl Pictures {
    /// Each compressed (`Picture::compressed`): what is drawn with.
    pub fn compressed(self) -> Pictures {
        Pictures { colour: self.colour.map(Picture::compressed), normal: self.normal.map(Picture::compressed), surface: self.surface.map(Picture::compressed) }
    }
}

/// A PNG file as rows of red, green, blue and alpha, top row first.
fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let size = reader.output_buffer_size().ok_or_else(|| anyhow!("the picture is too large"))?;
    let mut buffer = vec![0; size];
    let info = reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());
    let pixels = (info.width * info.height) as usize;
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buffer.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        other => bail!("a picture of an unknown kind ({other:?})"),
    };
    ensure!(rgba.len() == pixels * 4, "the picture's size is not what it says");
    Ok((info.width, info.height, rgba))
}

/// A picture at half its size each way: every pixel the mean of four.
fn halved(width: u32, height: u32, channels: usize, data: &[u8]) -> (u32, u32, Vec<u8>) {
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    let mut out = Vec::with_capacity(w as usize * h as usize * channels);
    let at = |x: u32, y: u32, c: usize| data[(y.min(height - 1) as usize * width as usize + x.min(width - 1) as usize) * channels + c] as u32;
    for y in 0..h {
        for x in 0..w {
            for c in 0..channels {
                let sum = at(2 * x, 2 * y, c) + at(2 * x + 1, 2 * y, c) + at(2 * x, 2 * y + 1, c) + at(2 * x + 1, 2 * y + 1, c);
                out.push(((sum + 2) / 4) as u8);
            }
        }
    }
    (w, h, out)
}

/// Makes a picture ready: no larger than `size` a side, rows from the
/// bottom up, every smaller size after it.
pub(super) fn ready(mut width: u32, mut height: u32, channels: usize, mut data: Vec<u8>, size: u32) -> Picture {
    while width.max(height) > size {
        (width, height, data) = halved(width, height, channels, &data);
    }
    let row = width as usize * channels;
    let mut flipped = Vec::with_capacity(data.len());
    for line in data.chunks_exact(row).rev() {
        flipped.extend_from_slice(line);
    }
    let mut picture = Picture { width, height, mips: 1, channels, data: flipped.clone(), blocks: None };
    let (mut w, mut h, mut level) = (width, height, flipped);
    while w > 1 || h > 1 {
        (w, h, level) = halved(w, h, channels, &level);
        picture.data.extend_from_slice(&level);
        picture.mips += 1;
    }
    picture
}

/// Reads a material's pictures and makes them ready to draw with.
pub fn pictures(material: &Material) -> Result<Pictures> {
    let read = |path: &Option<PathBuf>| -> Result<Option<(u32, u32, Vec<u8>)>> {
        path.as_ref().map(|path| read_png(path).with_context(|| format!("reading {}", path.display()))).transpose()
    };
    let colour = read(&material.base_color)?.map(|(w, h, data)| ready(w, h, 4, data, material.size));
    let normal = read(&material.normal)?.map(|(w, h, data)| {
        // Red and green are enough: what draws it works the third out.
        let two: Vec<u8> = data.chunks_exact(4).flat_map(|p| [p[0], if material.normal_green_down { 255 - p[1] } else { p[1] }]).collect();
        ready(w, h, 2, two, material.size)
    });
    // One picture of the three that say what the surface is like, each
    // in the channel it is read from: occlusion in red, roughness in
    // green, metal in blue. One that is missing counts as not occluded,
    // fully rough and no metal.
    let (occlusion, roughness, metallic) = (read(&material.occlusion)?, read(&material.roughness)?, read(&material.metallic)?);
    let first = [&roughness, &metallic, &occlusion].into_iter().flatten().next().map(|(w, h, _)| (*w, *h));
    let surface = match first {
        None => None,
        Some((w, h)) => {
            for (which, _) in [&roughness, &metallic, &occlusion].into_iter().flatten().map(|(pw, ph, _)| ((*pw, *ph), ())) {
                ensure!(which == (w, h), "{}'s pictures of its surface are not of one size", material.name);
            }
            let channel = |picture: &Option<(u32, u32, Vec<u8>)>, pixel: usize, missing: u8| picture.as_ref().map_or(missing, |(_, _, data)| data[pixel * 4]);
            let data: Vec<u8> = (0..(w * h) as usize).flat_map(|pixel| [channel(&occlusion, pixel, 255), channel(&roughness, pixel, 255), channel(&metallic, pixel, 0), 255]).collect();
            Some(ready(w, h, 4, data, material.size))
        }
    };
    Ok(Pictures { colour, normal, surface })
}

/// Reads the pictures of several materials at once, each on a thread of
/// its own: the files are large and take their time to unpack. Compressed
/// as the install's are (`Pictures::compressed`), to be drawn with, and
/// kept so (`compressed_pictures`).
pub fn pictures_of(materials: &[Material]) -> Vec<Result<Pictures>> {
    std::thread::scope(|scope| {
        let running: Vec<_> = materials.iter().map(|material| scope.spawn(move || compressed_pictures(material))).collect();
        running.into_iter().map(|thread| thread.join().unwrap_or_else(|_| Err(anyhow!("reading a picture failed")))).collect()
    })
}

/// The folder compressed pictures are kept in between runs: a new one when
/// the encoders change what they make.
const PICTURE_CACHE: &str = "v1";

/// A material's pictures compressed, read from where they were kept the
/// last time if the files they come from are as they were then (their
/// paths, sizes and times, the material's size and the way its normals
/// are turned), else read, compressed and kept: `local\picture_cache` in
/// the folder the app runs in or any above it. Reading the PNG files and
/// compressing them takes the StG 44's about a second; what is kept, a
/// tenth of that.
fn compressed_pictures(material: &Material) -> Result<Pictures> {
    let kept = picture_cache_file(material);
    if let Some(pictures) = kept.as_ref().and_then(|file| read_kept(file).ok()) {
        return Ok(pictures);
    }
    let pictures = pictures(material)?.compressed();
    if let Some(file) = kept {
        // Not kept is only slower next time.
        let _ = file.parent().map(std::fs::create_dir_all).transpose().and_then(|_| write_kept(&file, &pictures));
    }
    Ok(pictures)
}

/// Where a material's compressed pictures are kept, by what they come
/// from; none where there is no `local` folder.
fn picture_cache_file(material: &Material) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut dir = std::env::current_dir().ok()?;
    let local = loop {
        if dir.join("local").is_dir() {
            break dir.join("local");
        }
        if !dir.pop() {
            return None;
        }
    };
    let mut hasher = std::hash::DefaultHasher::new();
    for path in [&material.base_color, &material.normal, &material.occlusion, &material.roughness, &material.metallic] {
        path.hash(&mut hasher);
        if let Some(meta) = path.as_ref().and_then(|path| std::fs::metadata(path).ok()) {
            meta.len().hash(&mut hasher);
            meta.modified().ok().hash(&mut hasher);
        }
    }
    (material.size, material.normal_green_down).hash(&mut hasher);
    let name: String = material.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    Some(local.join("picture_cache").join(PICTURE_CACHE).join(format!("{name}_{:016x}.bin", hasher.finish())))
}

/// What a kept file begins with.
const KEPT_MARK: &[u8; 8] = b"TRPPIC1\0";

pub(super) fn write_kept(file: &Path, pictures: &Pictures) -> std::io::Result<()> {
    let mut out = KEPT_MARK.to_vec();
    for picture in [&pictures.colour, &pictures.normal, &pictures.surface] {
        let Some(p) = picture else {
            out.push(0);
            continue;
        };
        out.push(1);
        for value in [p.width, p.height, p.mips, p.channels as u32] {
            out.extend(value.to_le_bytes());
        }
        out.push(match p.blocks {
            None => 0,
            Some(Blocks::Bc1) => 1,
            Some(Blocks::Bc5) => 2,
        });
        out.extend((p.data.len() as u64).to_le_bytes());
        out.extend_from_slice(&p.data);
    }
    std::fs::write(file, out)
}

pub(super) fn read_kept(file: &Path) -> Result<Pictures> {
    let bytes = std::fs::read(file)?;
    ensure!(bytes.starts_with(KEPT_MARK), "not a kept picture");
    let mut at = KEPT_MARK.len();
    let mut take = |n: usize| -> Result<&[u8]> {
        let slice = bytes.get(at..at + n).ok_or_else(|| anyhow!("a kept picture ends early"))?;
        at += n;
        Ok(slice)
    };
    let mut read = || -> Result<Option<Picture>> {
        if take(1)?[0] == 0 {
            return Ok(None);
        }
        let mut word = || -> Result<u32> { Ok(u32::from_le_bytes(take(4)?.try_into()?)) };
        let (width, height, mips, channels) = (word()?, word()?, word()?, word()? as usize);
        let blocks = match take(1)?[0] {
            0 => None,
            1 => Some(Blocks::Bc1),
            2 => Some(Blocks::Bc5),
            other => bail!("a kept picture of an unknown kind ({other})"),
        };
        let length = u64::from_le_bytes(take(8)?.try_into()?) as usize;
        Ok(Some(Picture { width, height, mips, channels, data: take(length)?.to_vec(), blocks }))
    };
    Ok(Pictures { colour: read()?, normal: read()?, surface: read()? })
}
