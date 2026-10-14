//! A model of the X-Ray engine's, the engine of the S.T.A.L.K.E.R. games:
//! an `.ogf` file and the `.dds` pictures it names, read in place from the
//! user's own install of the game (`docs/spec/outfits.md`). Nothing is
//! written to that install, and nothing of it is kept here.
//!
//! The format is the engine's own, and the engine's source is published
//! (OpenXRay, `xray-16`): the chunks' numbers and the vertex formats are
//! `xrCore/FMesh.hpp`'s, the skinned vertices `Layers/xrRender/SkeletonX.cpp`'s,
//! the bones and their rest pose `Layers/xrRender/SkeletonCustom.cpp`'s
//! (`CKinematics::Load`) with `xrCore/Animation/Bone.cpp`, and the turn of
//! a bone at rest `Fmatrix::setXYZi` (`utils/xrMiscMath/matrix.cpp`).
//!
//! The engine's space is the one Unity has: left-handed, x to the right,
//! y up, z ahead, in metres. So a point read here is a point of the
//! install's own meshes' kind, and goes the same way from there on.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::xray_archive::{self, Archive};

/// Where the user's S.T.A.L.K.E.R. is: a Mod Organizer folder (GAMMA is
/// one), whose mods lie over the game's own data in the order its profile
/// lists them, and the game's own folder. Either may be missing.
#[derive(Debug, Clone, Default)]
pub struct Stalker {
    /// The enabled mods' folders, the one that wins first.
    mods: Vec<PathBuf>,
    /// The game's own folder (Anomaly's).
    game: Option<PathBuf>,
    /// The game's packed archives, their tables read the first time a file
    /// is not found loose (`read`), and shared by every copy.
    archives: Arc<OnceLock<Vec<Archive>>>,
}

/// A folder named by an environment variable, or by a file of that name
/// in a `local` folder of the current directory or any above it.
fn hinted(variable: &str, file: &str) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var(variable) {
        return Some(PathBuf::from(dir));
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let hint = dir.join("local").join(file);
        if hint.is_file() {
            return crate::install::read_hint(&hint).map(PathBuf::from);
        }
        if !dir.pop() {
            return None;
        }
    }
}

impl Stalker {
    /// Finds the two folders: the Mod Organizer's from `GAMMA_DIR` or
    /// `local/gamma_dir.txt`, the game's from `ANOMALY_DIR` or
    /// `local/anomaly_dir.txt`.
    pub fn locate() -> Self {
        Self::at(hinted("GAMMA_DIR", "gamma_dir.txt").as_deref(), hinted("ANOMALY_DIR", "anomaly_dir.txt").as_deref())
    }

    /// The mods a Mod Organizer folder has enabled, in the order they
    /// win: its `ModOrganizer.ini` names the profile, and the profile's
    /// `modlist.txt` lists every mod, the one that wins first, each with a
    /// `+` if it is on.
    pub fn at(organizer: Option<&Path>, game: Option<&Path>) -> Self {
        let mods = organizer.map(|organizer| {
            let profile = std::fs::read_to_string(organizer.join("ModOrganizer.ini")).ok().and_then(|text| {
                let line = text.lines().find_map(|line| line.trim().strip_prefix("selected_profile="))?;
                Some(line.trim_start_matches("@ByteArray(").trim_end_matches(')').to_owned())
            });
            let listed = profile.and_then(|profile| std::fs::read_to_string(organizer.join("profiles").join(profile).join("modlist.txt")).ok());
            let enabled = listed.iter().flat_map(|text| text.lines()).filter_map(|line| line.trim_end().strip_prefix('+'));
            enabled.map(|name| organizer.join("mods").join(name)).filter(|folder| folder.is_dir()).collect()
        });
        Self { mods: mods.unwrap_or_default(), game: game.map(Path::to_path_buf).filter(|game| game.is_dir()), archives: Arc::default() }
    }

    /// The game's own folder as a Mod Organizer folder names it (its
    /// `ModOrganizer.ini`: `gamePath=@ByteArray(E:\\AnomalyEFT)`, every `\`
    /// doubled).
    pub fn organizer_game(organizer: &Path) -> Option<PathBuf> {
        let text = std::fs::read_to_string(organizer.join("ModOrganizer.ini")).ok()?;
        let line = text.lines().find_map(|line| line.trim().strip_prefix("gamePath="))?;
        let path = line.trim().trim_start_matches("@ByteArray(").trim_end_matches(')').replace("\\\\", "\\");
        (!path.is_empty()).then(|| PathBuf::from(path))
    }

    /// Whether either folder was found.
    pub fn known(&self) -> bool {
        !self.mods.is_empty() || self.game.is_some()
    }

    /// The game's own folder, where it was found.
    pub fn game(&self) -> Option<&Path> {
        self.game.as_deref()
    }

    /// A file of the game's data by its path under `gamedata`, as the game
    /// would find it loose: in the first enabled mod that has it, else in
    /// the game's own folder. What is only in the game's packed archives
    /// is not found.
    pub fn find(&self, under_gamedata: &str) -> Option<PathBuf> {
        let relative: PathBuf = under_gamedata.split(['\\', '/']).filter(|part| !part.is_empty()).collect();
        self.mods.iter().chain(&self.game).map(|folder| folder.join("gamedata").join(&relative)).find(|file| file.is_file())
    }

    /// The game's packed archives, the one that wins first, their tables
    /// read on the first call (`xray_archive::archives_of`). One that
    /// cannot be read is left out, with a word on why.
    pub fn archives(&self) -> &[Archive] {
        self.archives.get_or_init(|| {
            let Some(game) = &self.game else { return Vec::new() };
            xray_archive::archives_of(game)
                .iter()
                .filter_map(|path| match Archive::open(path) {
                    Ok(archive) => Some(archive),
                    Err(e) => {
                        eprintln!("the other game's archive {}: {e:#}", path.display());
                        None
                    }
                })
                .collect()
        })
    }

    /// A file of the game's data by its path under `gamedata`, as the game
    /// would find it: loose (`find`), else in its packed archives. Its
    /// bytes, and where it was found.
    pub fn read(&self, under_gamedata: &str) -> Result<(Vec<u8>, String)> {
        if let Some(file) = self.find(under_gamedata) {
            let bytes = std::fs::read(&file).with_context(|| file.display().to_string())?;
            return Ok((bytes, file.display().to_string()));
        }
        for archive in self.archives() {
            if let Some(bytes) = archive.read(under_gamedata)? {
                return Ok((bytes, format!("{under_gamedata} in {}", archive.path.display())));
            }
        }
        bail!("{under_gamedata} is in none of the other game's folders, loose or packed (local/gamma_dir.txt, local/anomaly_dir.txt)")
    }
}

/// A bone of a model at rest: where it is in the model, and its three
/// axes there (a point `p` of the bone's own is at
/// `at + p.x * axes[0] + p.y * axes[1] + p.z * axes[2]`).
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub at: [f32; 3],
    pub axes: [[f32; 3]; 3],
}

/// One mesh of a model, with the picture it is drawn with: every vertex
/// at rest in the model's space, and the bones it goes with, each with
/// its share (a share of nothing for a link not used).
#[derive(Debug, Clone)]
pub struct Part {
    /// The picture's path under `textures`, without its ending.
    pub texture: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub links: Vec<[(u16, f32); 4]>,
    pub triangles: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub bones: Vec<Bone>,
    pub parts: Vec<Part>,
}

const HEADER: u32 = 1;
const TEXTURE: u32 = 2;
const VERTICES: u32 = 3;
const INDICES: u32 = 4;
const PROGRESSIVE: u32 = 6;
const CHILDREN: u32 = 9;
const BONE_NAMES: u32 = 13;
const BONE_REST: u32 = 16;
/// A chunk whose body is packed: none of a model's are, in the files read.
const PACKED: u32 = 0x8000_0000;
/// The number the four skinned vertex formats are multiples of; a file
/// names its format by one of them or by the count of links alone.
const FORMAT_UNIT: u32 = 0x1207_1980;

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(count).filter(|end| *end <= self.bytes.len()).ok_or_else(|| anyhow!("the file ends early"))?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }

    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into()?))
    }

    fn vec3(&mut self) -> Result<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    /// A text that ends with a zero byte.
    fn text(&mut self) -> Result<String> {
        let rest = &self.bytes[self.at..];
        let length = rest.iter().position(|byte| *byte == 0).ok_or_else(|| anyhow!("a name does not end"))?;
        self.at += length + 1;
        Ok(String::from_utf8_lossy(&rest[..length]).into_owned())
    }

    fn done(&self) -> bool {
        self.at >= self.bytes.len()
    }
}

/// The chunks of a file or of a chunk: each a number, a length and a body.
fn chunks(bytes: &[u8]) -> Result<Vec<(u32, &[u8])>> {
    let mut cursor = Cursor { bytes, at: 0 };
    let mut found = Vec::new();
    while !cursor.done() {
        let (id, length) = (cursor.u32()?, cursor.u32()?);
        ensure!(id & PACKED == 0, "chunk {} is packed, which is not read", id & !PACKED);
        found.push((id, cursor.take(length as usize)?));
    }
    Ok(found)
}

fn chunk<'a>(of: &[(u32, &'a [u8])], id: u32) -> Option<&'a [u8]> {
    of.iter().find(|(found, _)| *found == id).map(|(_, body)| *body)
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scaled(a: [f32; 3], by: f32) -> [f32; 3] {
    [a[0] * by, a[1] * by, a[2] * by]
}

/// A vector of a bone's own in the space its axes are given in.
fn along(axes: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    add(add(scaled(axes[0], v[0]), scaled(axes[1], v[1])), scaled(axes[2], v[2]))
}

/// The axes of a turn the engine keeps as three angles (`Fmatrix::setXYZi`,
/// which is `setHPB` of the angles about y, x and z, each the other way).
fn axes_of(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let (heading, pitch, bank) = (-angles[1], -angles[0], -angles[2]);
    let ((sh, ch), (sp, cp), (sb, cb)) = (heading.sin_cos(), pitch.sin_cos(), bank.sin_cos());
    [
        [ch * cb - sp * sh * sb, -cp * sb, sp * ch * sb + sh * cb],
        [sp * sh * cb + ch * sb, cp * cb, sh * sb - sp * ch * cb],
        [-cp * sh, sp, cp * ch],
    ]
}

/// Reads a model with a skeleton: its bones at rest and its meshes.
pub fn read(bytes: &[u8]) -> Result<Model> {
    let file = chunks(bytes)?;
    let header = chunk(&file, HEADER).ok_or_else(|| anyhow!("not a model of the engine's: no header"))?;
    ensure!(header.first() == Some(&4), "a model of version {:?}, where 4 is read", header.first());
    let bones = read_bones(&file)?;
    let children = chunk(&file, CHILDREN).ok_or_else(|| anyhow!("the model has no meshes of its own"))?;
    let parts = chunks(children)?.into_iter().map(|(index, body)| read_part(body, bones.len()).with_context(|| format!("its mesh {index}"))).collect::<Result<Vec<_>>>()?;
    Ok(Model { bones, parts })
}

/// The bones, each with its parent, and where each is at rest: the file
/// has a bone's turn and place in its parent's axes, and the engine goes
/// down from the root (`CBoneData::CalculateM2B`).
fn read_bones(file: &[(u32, &[u8])]) -> Result<Vec<Bone>> {
    let names = chunk(file, BONE_NAMES).ok_or_else(|| anyhow!("the model has no bones"))?;
    let mut cursor = Cursor { bytes: names, at: 0 };
    let count = cursor.u32()? as usize;
    let mut named: Vec<(String, String)> = Vec::with_capacity(count);
    for _ in 0..count {
        named.push((cursor.text()?, cursor.text()?));
        // The box about the bone: its turn, its middle, its half size.
        cursor.take(60)?;
    }
    let rest = chunk(file, BONE_REST).ok_or_else(|| anyhow!("the model's bones have no rest pose"))?;
    let mut cursor = Cursor { bytes: rest, at: 0 };
    let mut local: Vec<([[f32; 3]; 3], [f32; 3])> = Vec::with_capacity(count);
    for _ in 0..count {
        let version = cursor.u32()?;
        cursor.text()?;
        // The bone's shape (a kind, flags, a box, a ball, a cylinder), then
        // its joint: a kind, three limits of four figures, two figures,
        // flags, two figures, and since the first version one more.
        cursor.take(2 + 2 + 60 + 16 + 32)?;
        cursor.take(4 + 3 * 16 + 4 + 4 + 4 + 4 + 4 + if version > 0 { 4 } else { 0 })?;
        let (angles, place) = (cursor.vec3()?, cursor.vec3()?);
        // Its mass and the middle of it.
        cursor.take(4 + 12)?;
        local.push((axes_of(angles), place));
    }
    let parents: Vec<Option<usize>> = named.iter().map(|(_, parent)| named.iter().position(|(name, _)| name == parent).filter(|_| !parent.is_empty())).collect();
    let mut bones: Vec<Option<Bone>> = vec![None; count];
    // A parent may be listed after its child: go round until all are placed.
    for _ in 0..count {
        for index in 0..count {
            if bones[index].is_some() {
                continue;
            }
            let (axes, place) = local[index];
            let placed = match parents[index] {
                None => Some((axes, place)),
                Some(parent) => bones[parent].as_ref().map(|above| (axes.map(|axis| along(&above.axes, axis)), add(above.at, along(&above.axes, place)))),
            };
            if let Some((axes, at)) = placed {
                bones[index] = Some(Bone { name: named[index].0.clone(), parent: parents[index], at, axes });
            }
        }
    }
    bones.into_iter().collect::<Option<Vec<_>>>().ok_or_else(|| anyhow!("a bone hangs from itself"))
}

fn read_part(body: &[u8], bones: usize) -> Result<Part> {
    let inner = chunks(body)?;
    let texture = Cursor { bytes: chunk(&inner, TEXTURE).ok_or_else(|| anyhow!("no picture is named"))?, at: 0 }.text()?;
    let mut cursor = Cursor { bytes: chunk(&inner, VERTICES).ok_or_else(|| anyhow!("no vertices"))?, at: 0 };
    let (format, count) = (cursor.u32()?, cursor.u32()? as usize);
    // How many bones a vertex goes with. (The format's own numbers are 1,
    // 2, 4 and 5 times the unit for one to four.)
    let links = match format {
        1..=4 => format as usize,
        other => match (other % FORMAT_UNIT == 0).then_some(other / FORMAT_UNIT) {
            Some(1) => 1,
            Some(2) => 2,
            Some(4) => 3,
            Some(5) => 4,
            _ => bail!("vertices of the kind {format:#x} are not read"),
        },
    };
    let mut part = Part { texture, positions: Vec::with_capacity(count), normals: Vec::with_capacity(count), uv: Vec::with_capacity(count), links: Vec::with_capacity(count), triangles: Vec::new() };
    for _ in 0..count {
        let mut linked = [(0u16, 0.0f32); 4];
        if links > 1 {
            for link in linked.iter_mut().take(links) {
                link.0 = cursor.u16()?;
            }
        }
        part.positions.push(cursor.vec3()?);
        part.normals.push(cursor.vec3()?);
        // The tangent and the binormal.
        cursor.take(24)?;
        match links {
            1 => {
                part.uv.push([cursor.f32()?, cursor.f32()?]);
                linked[0] = (cursor.u32()? as u16, 1.0);
            }
            // One share is kept, the second bone's: the first has the rest.
            2 => {
                let second = cursor.f32()?;
                (linked[0].1, linked[1].1) = (1.0 - second, second);
                part.uv.push([cursor.f32()?, cursor.f32()?]);
            }
            // All but the last bone's are kept: that has the rest.
            _ => {
                let mut left = 1.0;
                for link in linked.iter_mut().take(links - 1) {
                    link.1 = cursor.f32()?;
                    left -= link.1;
                }
                linked[links - 1].1 = left;
                part.uv.push([cursor.f32()?, cursor.f32()?]);
            }
        }
        ensure!(linked.iter().all(|(bone, share)| *share == 0.0 || (*bone as usize) < bones), "a vertex goes with a bone the model has not");
        part.links.push(linked);
    }
    let mut cursor = Cursor { bytes: chunk(&inner, INDICES).ok_or_else(|| anyhow!("no triangles"))?, at: 0 };
    let listed = cursor.u32()? as usize;
    let mut triangles = (0..listed).map(|_| cursor.u16().map(u32::from)).collect::<Result<Vec<_>>>()?;
    // A mesh kept at several levels of detail lists the fullest first: a
    // start among the triangles and how many.
    if let Some(levels) = chunk(&inner, PROGRESSIVE) {
        let mut cursor = Cursor { bytes: levels, at: 0 };
        cursor.take(16)?;
        if cursor.u32()? > 0 {
            let (start, count) = (cursor.u32()? as usize, cursor.u16()? as usize * 3);
            ensure!(start + count <= triangles.len(), "its fullest level has triangles the mesh has not");
            triangles = triangles[start..start + count].to_vec();
        }
    }
    ensure!(triangles.iter().all(|index| (*index as usize) < count), "a triangle has a vertex the mesh has not");
    part.triangles = triangles;
    Ok(part)
}

/// How a picture's bytes are packed: one of the block packings a graphics
/// card reads as they are, or four bytes a point (red, green, blue,
/// alpha).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packing {
    Bc1,
    Bc2,
    Bc3,
    Bc7,
    Rgba,
}

/// A picture as its `.dds` file has it: every level, the largest first,
/// its rows from the top.
#[derive(Debug, Clone)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub packing: Packing,
    pub data: Vec<u8>,
}

/// Reads a `.dds` file (Microsoft's own format, documented as "DDS file
/// layout"): a header of 124 bytes after the four letters, a second one
/// where the first says `DX10`, and the levels one after another.
pub fn read_picture(bytes: &[u8]) -> Result<Picture> {
    ensure!(bytes.len() >= 128 && &bytes[..4] == b"DDS ", "not a DDS file");
    let word = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let (height, width, listed) = (word(12), word(16), word(28).max(1));
    let (flags, letters, bits) = (word(80), &bytes[84..88], word(88));
    let (packing, start) = match letters {
        b"DXT1" => (Packing::Bc1, 128),
        b"DXT2" | b"DXT3" => (Packing::Bc2, 128),
        b"DXT4" | b"DXT5" => (Packing::Bc3, 128),
        b"DX10" => {
            ensure!(bytes.len() >= 148, "the file ends in its second header");
            let packing = match word(128) {
                70..=72 => Packing::Bc1,
                73..=75 => Packing::Bc2,
                76..=78 => Packing::Bc3,
                97..=99 => Packing::Bc7,
                other => bail!("a picture packed in the way numbered {other} is not read"),
            };
            (packing, 148)
        }
        // Not packed: four bytes a point, blue first.
        _ if flags & 0x40 != 0 && bits == 32 => (Packing::Rgba, 128),
        _ => bail!("a picture packed as {:?} is not read", String::from_utf8_lossy(letters)),
    };
    ensure!(width > 0 && height > 0, "a picture of no size");
    let block = match packing {
        Packing::Bc1 => 8,
        _ => 16,
    };
    let size = |level: u32| -> usize {
        let (w, h) = ((width >> level).max(1) as usize, (height >> level).max(1) as usize);
        match packing {
            Packing::Rgba => w * h * 4,
            _ => w.div_ceil(4) * h.div_ceil(4) * block,
        }
    };
    // As many levels as the file has whole.
    let (mut levels, mut end) = (0, start);
    while levels < listed && end + size(levels) <= bytes.len() && (width >> levels).max(height >> levels) > 0 {
        end += size(levels);
        levels += 1;
    }
    ensure!(levels > 0, "the picture's first level is not whole");
    let mut data = bytes[start..end].to_vec();
    if packing == Packing::Rgba {
        for point in data.chunks_exact_mut(4) {
            point.swap(0, 2);
        }
    }
    Ok(Picture { width, height, levels, packing, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_of_nothing_leaves_the_axes() {
        let axes = axes_of([0.0; 3]);
        assert_eq!(axes, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    }

    /// A quarter turn about y alone, as `setXYZi` makes it: x goes to
    /// minus z, and z to x.
    #[test]
    fn a_turn_about_one_axis() {
        let axes = axes_of([0.0, std::f32::consts::FRAC_PI_2, 0.0]);
        let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(&b).all(|(a, b)| (a - b).abs() < 1e-6);
        assert!(near(axes[0], [0.0, 0.0, -1.0]), "{axes:?}");
        assert!(near(axes[1], [0.0, 1.0, 0.0]) && near(axes[2], [1.0, 0.0, 0.0]), "{axes:?}");
    }

    /// A picture of one block: four letters, a header, eight bytes.
    #[test]
    fn a_picture_of_one_block() {
        let mut file = vec![0u8; 128 + 8];
        file[..4].copy_from_slice(b"DDS ");
        file[12..16].copy_from_slice(&4u32.to_le_bytes());
        file[16..20].copy_from_slice(&4u32.to_le_bytes());
        file[28..32].copy_from_slice(&3u32.to_le_bytes());
        file[84..88].copy_from_slice(b"DXT1");
        let picture = read_picture(&file).unwrap();
        // Three levels are listed and one is there.
        assert_eq!((picture.width, picture.height, picture.levels, picture.packing, picture.data.len()), (4, 4, 1, Packing::Bc1, 8));
    }
}
