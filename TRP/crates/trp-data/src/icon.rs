//! An item's picture in the inventory, as the game makes it
//! (`docs/spec/icons.md`): the item's own model, posed by its prefab's own
//! icon settings (`PreviewPivot.IconSettings`), drawn once by an icon
//! camera and kept.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::install::Install;
use crate::unity::extract::{read_hierarchy, script_class, Hierarchy};
use crate::unity::reader::Reader;
use crate::unity::serialized::SerializedFile;
use crate::unity::{class, Assets, ObjRef};

/// How an item's prefab asks to be posed for its picture
/// (`PreviewPivot.Icon`): the model's turn in the icon camera's world, a
/// shift after it is put in the middle of the picture (where
/// `has_offset`), and how much of the picture its bounds fill (one: all of
/// it on the longer side).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub rotation: [f32; 4],
    pub position: [f32; 3],
    pub has_offset: bool,
    pub bounds_scale: f32,
}

/// The prefab of a bundle that has icon settings: the object they are on,
/// and the settings. None where the bundle's objects have none (the game
/// then fits the camera to the model's bounds as it stands).
pub fn pose_in(assets: &Assets, files: &[usize]) -> Result<Option<(ObjRef, Pose)>> {
    let Some(behaviour) = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR)).find(|&obj| script_class(assets, obj).as_deref() == Some("PreviewPivot")) else {
        return Ok(None);
    };
    let value = assets.read(behaviour)?;
    let icon = value.req("Icon")?;
    let f = |v: &crate::unity::Value, name: &str| v.get(name).and_then(|x| x.as_f32()).unwrap_or(0.0);
    let rotation = icon.req("rotation")?;
    let position = icon.req("position")?;
    let pose = Pose {
        rotation: [f(rotation, "x"), f(rotation, "y"), f(rotation, "z"), f(rotation, "w")],
        position: [f(position, "x"), f(position, "y"), f(position, "z")],
        has_offset: icon.get("hasOffset").and_then(|v| v.as_bool()).unwrap_or(false),
        bounds_scale: icon.get("boundsScale").and_then(|v| v.as_f32()).filter(|scale| *scale > 0.0).unwrap_or(1.0),
    };
    let object = value.req("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(behaviour.file, p)).ok_or_else(|| anyhow!("PreviewPivot is on no object"))?;
    Ok(Some((object, pose)))
}

/// An item's model as its prefab has it, from the object its icon settings
/// are on (or, with none, the bundle's first root object), and those
/// settings.
pub fn model_of(assets: &mut Assets, key: &str) -> Result<(Hierarchy, Option<Pose>)> {
    let files = assets.load(key)?;
    if let Some((object, pose)) = pose_in(assets, &files)? {
        return Ok((read_hierarchy(assets, object)?, Some(pose)));
    }
    // A root: a game object whose transform has no parent.
    let root = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::GAME_OBJECT))
        .find(|&object| read_hierarchy(assets, object).is_ok_and(|tree| tree.nodes.first().is_some_and(|node| node.parent.is_none())))
        .ok_or_else(|| anyhow!("{key} has no object"))?;
    Ok((read_hierarchy(assets, root)?, None))
}

/// A light of the icon camera's: its turn in the camera's world, its
/// colour and its intensity, as the game's directional light has them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StudioLight {
    pub rotation: [f32; 4],
    pub color: [f32; 3],
    pub intensity: f32,
}

/// The game's icon camera (`Resources/IconCamera`): where it stands and
/// how it is turned, its directional lights, and its shadow's colour and
/// offset (`IconShadow`).
#[derive(Debug, Clone, PartialEq)]
pub struct Studio {
    pub rotation: [f32; 4],
    pub position: [f32; 3],
    pub lights: Vec<StudioLight>,
    pub shadow: [f32; 4],
    pub shadow_offset: [f32; 2],
}

impl Studio {
    /// The icon camera as `resources.assets` had it on 2026-10-14, for an
    /// install where it cannot be read.
    pub fn as_read() -> Self {
        let white = [1.0, 1.0, 1.0];
        Self {
            rotation: [0.0, 0.0, 0.0, 1.0],
            position: [0.0, 0.0, -1.0],
            lights: vec![
                StudioLight { rotation: [0.438_662_3, 0.030_208_346, 0.142_416_91, 0.886_780_86], color: white, intensity: 1.24 },
                StudioLight { rotation: [0.237_519_7, -0.446_088_97, 0.267_796_28, 0.820_289_1], color: white, intensity: 0.39 },
                StudioLight { rotation: [-0.175_900_34, -0.558_455_6, -0.800_963_8, 0.125_074_33], color: [0.514_705_9, 0.899_594_2, 1.0], intensity: 0.91 },
            ],
            shadow: [10.0 / 255.0, 10.0 / 255.0, 10.0 / 255.0, 1.0],
            shadow_offset: [0.0, 0.0],
        }
    }

    /// Reads the icon camera from the install's `resources.assets`, which
    /// keeps no type trees: each object by the engine's own layout of it
    /// (a game object, a transform, a light, a behaviour's script and its
    /// first fields), as `docs/spec/icons.md` has them.
    pub fn read(install: &Install) -> Result<Self> {
        let folder = install.root.join("EscapeFromTarkov_Data");
        let data = std::fs::read(folder.join("resources.assets"))?;
        let meta = SerializedFile::parse(&data)?;
        let bytes = |path_id: i64| meta.objects.iter().find(|o| o.path_id == path_id).map(|o| (o.class_id, &data[o.offset..o.offset + o.size]));
        // A game object: its components (file, path id), and its name.
        let object = |bytes: &[u8]| -> Result<(Vec<(i32, i64)>, String)> {
            let mut r = Reader::new(bytes);
            let count = r.u32()? as usize;
            let mut components = Vec::with_capacity(count);
            for _ in 0..count {
                components.push((r.i32()?, r.i64()?));
            }
            r.skip(4)?;
            let len = r.u32()? as usize;
            Ok((components, String::from_utf8_lossy(r.bytes(len)?).into_owned()))
        };
        // A transform: its turn, its place, and its children's transforms.
        let transform = |bytes: &[u8]| -> Result<([f32; 4], [f32; 3], Vec<i64>)> {
            let mut r = Reader::new(bytes);
            r.skip(12)?;
            let rotation = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
            let position = [r.f32()?, r.f32()?, r.f32()?];
            r.skip(12)?;
            let count = r.u32()? as usize;
            let mut children = Vec::with_capacity(count);
            for _ in 0..count {
                r.skip(4)?;
                children.push(r.i64()?);
            }
            Ok((rotation, position, children))
        };
        let camera = meta
            .objects
            .iter()
            .filter(|o| o.class_id == class::GAME_OBJECT)
            .find_map(|o| object(&data[o.offset..o.offset + o.size]).ok().filter(|(_, name)| name == "IconCamera").map(|(components, _)| components))
            .ok_or_else(|| anyhow!("resources.assets has no IconCamera"))?;
        let mine = |components: &[(i32, i64)], class_id: i32| components.iter().filter(|(file, _)| *file == 0).filter_map(|(_, id)| bytes(*id)).filter(move |(class, _)| *class == class_id).map(|(_, b)| b).collect::<Vec<_>>();
        let (rotation, position, children) = transform(mine(&camera, class::TRANSFORM).first().ok_or_else(|| anyhow!("the IconCamera has no transform"))?)?;
        // Its lights: the children's, each turned as its child is.
        let mut lights = Vec::new();
        for child in children {
            let Some((_, child_transform)) = bytes(child) else { continue };
            let (turn, _, _) = transform(child_transform)?;
            let game_object = i64::from_le_bytes(child_transform[4..12].try_into()?);
            let Some((_, child_object)) = bytes(game_object) else { continue };
            let (components, _) = object(child_object)?;
            for light in mine(&components, class::LIGHT) {
                let mut r = Reader::new(light);
                r.skip(12)?;
                let (enabled, kind) = (r.u32()? & 1 == 1, r.i32()?);
                r.skip(4)?;
                let color = [r.f32()?, r.f32()?, r.f32()?];
                r.skip(4)?;
                let intensity = r.f32()?;
                // Directional lights only: the icon camera has no other.
                if enabled && kind == 1 {
                    lights.push(StudioLight { rotation: turn, color, intensity });
                }
            }
        }
        // Its shadow: the behaviour whose script is `IconShadow`, its
        // colour and offset after its (empty) name.
        let script_name = |file: i32, id: i64| -> Option<String> {
            let owned;
            let script: &[u8] = if file == 0 {
                bytes(id)?.1
            } else {
                let name = meta.externals.get(file as usize - 1)?.rsplit(['/', '\\']).next()?.to_owned();
                owned = std::fs::read(folder.join(&name)).ok()?;
                let other = SerializedFile::parse(&owned).ok()?;
                let o = other.objects.iter().find(|o| o.path_id == id)?;
                &owned[o.offset..o.offset + o.size]
            };
            let mut r = Reader::new(script);
            let len = r.u32().ok()? as usize;
            Some(String::from_utf8_lossy(r.bytes(len).ok()?).into_owned())
        };
        let mut shadow = None;
        for behaviour in mine(&camera, class::MONO_BEHAVIOUR) {
            let mut r = Reader::new(behaviour);
            r.skip(16)?;
            let (file, id) = (r.i32()?, r.i64()?);
            if script_name(file, id).as_deref() != Some("IconShadow") {
                continue;
            }
            let len = r.u32()? as usize;
            r.skip(len)?;
            r.align(4);
            shadow = Some(([r.f32()?, r.f32()?, r.f32()?, r.f32()?], [r.f32()?, r.f32()?]));
        }
        let (shadow, shadow_offset) = shadow.ok_or_else(|| anyhow!("the IconCamera has no IconShadow"))?;
        anyhow::ensure!(!lights.is_empty(), "the IconCamera has no light");
        Ok(Self { rotation, position, lights, shadow, shadow_offset })
    }
}

/// A picture drawn at twice its size, finished as the game finishes it:
/// the shadow under the model (`IconShadow`: the drawn outline in the
/// shadow's colour, blurred, behind the model) and the picture halved.
/// `rgba` is as the engine draws on a clear background: each pixel's
/// colour already multiplied by how much of it is covered. The picture
/// returned is `width / 2` by `height / 2`, its colours not multiplied
/// (as a PNG and the screen's pictures have them).
///
/// The game blurs by drawing the outline at a quarter of the size and
/// back eight times; three box blurs of three pixels at full size come
/// near it (`PROVISIONAL`).
pub fn finish(rgba: &[u8], width: usize, height: usize, shadow: [f32; 4], offset: [f32; 2]) -> Vec<u8> {
    let at = |x: usize, y: usize| (y * width + x) * 4;
    // The outline, moved by the shadow's offset (x right, y up), then
    // blurred.
    let mut cover = vec![0f32; width * height];
    let (dx, dy) = (offset[0].round() as i64, -offset[1].round() as i64);
    for y in 0..height {
        for x in 0..width {
            let (sx, sy) = (x as i64 - dx, y as i64 - dy);
            if sx >= 0 && sy >= 0 && (sx as usize) < width && (sy as usize) < height {
                cover[y * width + x] = rgba[at(sx as usize, sy as usize) + 3] as f32 / 255.0;
            }
        }
    }
    for _ in 0..3 {
        cover = box_blur(&cover, width, height, 3);
    }
    // The model over its shadow, each pixel as the colour multiplied by
    // its cover.
    let shade = |c: usize| shadow[c] * 255.0;
    let mut over = vec![0f32; width * height * 4];
    for (index, pixel) in over.chunks_exact_mut(4).enumerate() {
        let source = &rgba[index * 4..index * 4 + 4];
        let a = source[3] as f32 / 255.0;
        let under = cover[index] * shadow[3];
        for c in 0..3 {
            pixel[c] = source[c] as f32 + shade(c) * under * (1.0 - a);
        }
        pixel[3] = (a + under * (1.0 - a)) * 255.0;
    }
    // Halved: each pixel the mean of four, and its colour no longer
    // multiplied.
    let (half_w, half_h) = (width / 2, height / 2);
    let mut out = vec![0u8; half_w * half_h * 4];
    for y in 0..half_h {
        for x in 0..half_w {
            let mut sum = [0f32; 4];
            for (sx, sy) in [(2 * x, 2 * y), (2 * x + 1, 2 * y), (2 * x, 2 * y + 1), (2 * x + 1, 2 * y + 1)] {
                for (c, total) in sum.iter_mut().enumerate() {
                    *total += over[at(sx, sy) + c];
                }
            }
            let alpha = sum[3] / 4.0;
            let pixel = &mut out[(y * half_w + x) * 4..][..4];
            for c in 0..3 {
                pixel[c] = if alpha > 0.0 { (sum[c] / 4.0 / (alpha / 255.0)).round().clamp(0.0, 255.0) as u8 } else { 0 };
            }
            pixel[3] = alpha.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Each value the mean of those within `radius` across, then up and down.
fn box_blur(values: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let mut across = vec![0f32; values.len()];
    for y in 0..height {
        for x in 0..width {
            let (from, to) = (x.saturating_sub(radius), (x + radius).min(width - 1));
            across[y * width + x] = (from..=to).map(|sx| values[y * width + sx]).sum::<f32>() / (2 * radius + 1) as f32;
        }
    }
    let mut out = vec![0f32; values.len()];
    for y in 0..height {
        for x in 0..width {
            let (from, to) = (y.saturating_sub(radius), (y + radius).min(height - 1));
            out[y * width + x] = (from..=to).map(|sy| across[sy * width + x]).sum::<f32>() / (2 * radius + 1) as f32;
        }
    }
    out
}

/// A picture turned a quarter, as the game turns a thing's picture for a
/// thing that lies turned in its grid (270 degrees about the screen's
/// axis): the left column becomes the top row. `height` by `width` after.
pub fn turned(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; rgba.len()];
    for y in 0..height {
        for x in 0..width {
            // (x, y) goes to column height-1-y, row x.
            let to = (x * height + (height - 1 - y)) * 4;
            out[to..to + 4].copy_from_slice(&rgba[(y * width + x) * 4..][..4]);
        }
    }
    out
}

/// Writes a picture (RGBA, colours not multiplied) as a PNG.
pub fn save_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

/// Reads a PNG as RGBA: its width, its height and its pixels.
pub fn load_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    let mut data = vec![0; reader.output_buffer_size().ok_or_else(|| anyhow!("a picture too large"))?];
    let info = reader.next_frame(&mut data)?;
    data.truncate(info.buffer_size());
    anyhow::ensure!(info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight, "not an RGBA picture of eight bits");
    Ok((info.width, info.height, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square of full cover finished: halved, its own colour kept, a
    /// dark rim of shadow round it, nothing far off.
    #[test]
    fn a_picture_is_finished_with_its_shadow_and_halved() {
        let (w, h) = (40, 20);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 6..14 {
            for x in 10..30 {
                rgba[(y * w + x) * 4..][..4].copy_from_slice(&[200, 100, 50, 255]);
            }
        }
        let out = finish(&rgba, w, h, [10.0 / 255.0, 10.0 / 255.0, 10.0 / 255.0, 1.0], [0.0, 0.0]);
        assert_eq!(out.len(), 20 * 10 * 4);
        let pixel = |x: usize, y: usize| &out[(y * 20 + x) * 4..][..4];
        assert_eq!(pixel(10, 5), [200, 100, 50, 255], "the middle is the model's own");
        let rim = pixel(4, 5);
        assert!(rim[3] > 20 && rim[3] < 200 && rim[0] < 20, "beside it, the shadow: {rim:?}");
        assert!(pixel(0, 0)[3] < 5, "far off, next to nothing: {:?}", pixel(0, 0));
    }

    /// A quarter turn: the left column becomes the top row.
    #[test]
    fn a_picture_is_turned_a_quarter() {
        // Two by one: red, then green.
        let out = turned(&[255, 0, 0, 255, 0, 255, 0, 255], 2, 1);
        // One by two: red on top, green under it.
        assert_eq!(out, [255, 0, 0, 255, 0, 255, 0, 255]);
        // Two by two: the left column (a over c) becomes the top row, c
        // then a, read from the left.
        let (a, b, c, d) = ([1u8, 0, 0, 255], [2u8, 0, 0, 255], [3u8, 0, 0, 255], [4u8, 0, 0, 255]);
        let out = turned(&[a, b, c, d].concat(), 2, 2);
        assert_eq!(out, [c, a, d, b].concat());
    }
}
