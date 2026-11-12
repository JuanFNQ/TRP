//! A scene's pictures, sprite cuts, clips and cursors.

use super::*;

/// The sprites a scene's images draw. They are in the build's own files
/// beside the scene (`sharedassets44.assets`), which keep no type trees: a
/// sprite and a texture are read there by the trees of the same classes
/// in a bundle of the install's, which the same engine wrote.
pub struct ScenePictures {
    pub(super) install: Install,
    pub(super) folder: std::path::PathBuf,
    sprite_tree: crate::unity::typetree::TypeTree,
    texture_tree: crate::unity::typetree::TypeTree,
    pub(super) files: HashMap<String, BuildFile>,
}

/// A sprite: the texture it is cut from (its file and its number there),
/// the rectangle cut, in the texture's pixels from its lower left corner,
/// how far in from each side (left, bottom, right, top) a sliced image
/// keeps it whole, and its pixels to a unit of the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteCut {
    pub name: String,
    pub texture: (String, i64),
    pub rect: [f32; 4],
    pub border: [f32; 4],
    pub pixels_per_unit: f32,
    /// The sprite's own rectangle, of which the cut is the part that is
    /// not see-through (`m_Rect`), and where the cut lies in it.
    pub whole: [f32; 4],
    pub offset: [f32; 2],
    /// Packed into an atlas turned or flipped, which is not drawn here.
    pub turned: bool,
}

impl SpriteCut {
    /// The share of an image's rectangle that the cut covers, as the
    /// engine draws a sprite trimmed of its see-through margin (an image
    /// not kept to the sprite's shape): from the left, from the bottom, to
    /// the right, to the top.
    pub fn covers(&self) -> [f32; 4] {
        let [w, h] = [self.whole[2].max(1e-6), self.whole[3].max(1e-6)];
        [self.offset[0] / w, self.offset[1] / h, (self.offset[0] + self.rect[2]) / w, (self.offset[1] + self.rect[3]) / h]
    }
}

/// The engine's class numbers for a sprite.
pub(super) const SPRITE: i32 = 213;

/// The animation clips of the build's own files (`sharedassets44.assets`),
/// each by its number, read by the tree of a clip from a bundle of the
/// install's: the smallest that has one (`probe classfind 74`), or any
/// that has where that one is gone.
pub fn scene_clips(install: &Install, pictures: &mut ScenePictures, files: &[String]) -> Result<Vec<(String, i64, crate::unity::anim::Clip)>> {
    const SMALLEST: &str = "assets/content/weapons/weapon_root_anim_fix.bundle";
    let class = class::ANIMATION_CLIP;
    let tree = match tree_of(install, SMALLEST, class) {
        Ok(tree) => tree,
        Err(_) => {
            let assets = crate::unity::Assets::new(install.clone())?;
            let mut keys: Vec<(u64, String)> = assets.manifest.entries.keys().filter_map(|key| Some((std::fs::metadata(install.bundle_path(key)).ok()?.len(), key.clone()))).collect();
            keys.sort();
            keys.iter().find_map(|(_, key)| tree_of(install, key, class).ok()).context("no bundle of the install's has a clip to read clips by")?
        }
    };
    let mut clips = Vec::new();
    for name in files {
        for id in pictures.objects(name, class).unwrap_or_default() {
            if let Ok(clip) = pictures.read_by(name, id, class, &tree).and_then(|v| crate::unity::anim::Clip::of(&v)) {
                clips.push((name.clone(), id, clip));
            }
        }
    }
    Ok(clips)
}

/// The build's own Resources by their paths (`ResourceManager`, the
/// engine's class 147 in `globalgamemanagers`: its container, each path
/// in lower case with the object it is): the path, the file the object is
/// in and its number there.
pub fn resource_paths(install: &Install) -> Result<Vec<(String, String, i64)>> {
    const RESOURCE_MANAGER: i32 = 147;
    let mut build = BuildFile::open(install, "globalgamemanagers")?;
    let object = *build.meta.objects.iter().find(|object| object.class_id == RESOURCE_MANAGER).context("globalgamemanagers has no resource manager")?;
    let externals = build.meta.externals.clone();
    let bytes = build.bytes(object.offset, object.size)?;
    let mut r = Reader { data: &bytes, pos: 0, big_endian: build.meta.big_endian };
    let count = r.u32()? as usize;
    anyhow::ensure!(count < 1_000_000, "the resource manager does not begin with its container");
    let mut paths = Vec::with_capacity(count);
    for _ in 0..count {
        let path = string(&mut r)?;
        let to = pointer(&mut r)?;
        let file = match to.file {
            0 => "globalgamemanagers".to_owned(),
            n => externals.get(n as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).unwrap_or_default(),
        };
        paths.push((path, file, to.path));
    }
    Ok(paths)
}

/// The game's mouse pointers (`UI.CursorSwitcher.Init`, `ECursorType`):
/// each kind's picture among the build's Resources and the point of it
/// that points, in pixels from its top left, as the game hands them to the
/// system (`Cursor.SetCursor`, a hardware cursor at the picture's own
/// size). `Invisible`'s picture (`cursor_invisible`) is not in the build.
pub const CURSORS: [(&str, [u32; 2]); 9] = [
    ("cursor_idle", [11, 11]),
    ("cursor_over", [11, 11]),
    ("cursor_stretch_horizontal", [16, 16]),
    ("cursor_stretch_vertical", [16, 16]),
    ("cursor_stretch_corner", [16, 16]),
    ("cursor_stretch_corner_2", [16, 16]),
    ("cursor_hand", [17, 10]),
    ("cursor_set_pin", [7, 10]),
    ("cursor_set_lock", [7, 10]),
];

/// A mouse pointer's picture: its Resources path, its size, the point that
/// points (from the top left), and its pixels, eight-bit RGBA in the sRGB
/// space, top row first.
#[derive(Debug, Clone)]
pub struct CursorPicture {
    pub path: String,
    pub size: [u32; 2],
    pub hotspot: [u32; 2],
    pub rgba: Vec<u8>,
}

/// The game's mouse pointers that the build holds (`CURSORS`), each read
/// from its texture in the build's Resources (`resources.assets`), its rows
/// turned to come top first.
pub fn cursor_pictures(install: &Install) -> Result<Vec<CursorPicture>> {
    let paths = resource_paths(install)?;
    let mut pictures = ScenePictures::new(install)?;
    let mut out = Vec::new();
    for (path, hotspot) in CURSORS {
        let Some((_, file, id)) = paths.iter().find(|(name, _, _)| name == path) else { continue };
        let texture = pictures.texture(file, *id).with_context(|| format!("the pointer {path}"))?;
        let levels = crate::unity::texture::mips(&texture)?;
        let level = *levels.first().with_context(|| format!("the pointer {path} has no picture"))?;
        let pixels = crate::unity::texture::decode_rgba(&texture, level)?;
        let (width, height) = (texture.width as usize, texture.height as usize);
        let mut rgba = Vec::with_capacity(width * height * 4);
        for row in (0..height).rev() {
            rgba.extend_from_slice(&pixels[row * width * 4..(row + 1) * width * 4]);
        }
        out.push(CursorPicture { path: path.to_owned(), size: [texture.width, texture.height], hotspot, rgba });
    }
    Ok(out)
}

impl ScenePictures {
    /// Finds the trees: in the first bundle of the install's pictures for
    /// its screens (`assets/content/textures/gui/`) that has both classes.
    pub fn new(install: &Install) -> Result<Self> {
        let mut assets = crate::unity::Assets::new(install.clone())?;
        let mut keys: Vec<String> = assets.manifest.entries.keys().filter(|key| key.starts_with("assets/content/textures/gui/")).cloned().collect();
        keys.sort();
        for key in keys {
            let Ok(files) = assets.load(&key) else { continue };
            let tree = |class: i32| {
                files.iter().find_map(|&file| assets.file(file).meta.types.iter().find(|ty| ty.class_id == class && !ty.tree.is_empty()).map(|ty| ty.tree.clone()))
            };
            if let (Some(sprite_tree), Some(texture_tree)) = (tree(SPRITE), tree(class::TEXTURE_2D)) {
                let folder = install.root.join("EscapeFromTarkov_Data");
                return Ok(Self { install: install.clone(), folder, sprite_tree, texture_tree, files: HashMap::new() });
            }
        }
        anyhow::bail!("no bundle of the install's screen pictures has a sprite to read sprites by")
    }

    pub(super) fn file(&mut self, name: &str) -> Result<&mut BuildFile> {
        if !self.files.contains_key(name) {
            let file = BuildFile::open(&self.install, name)?;
            self.files.insert(name.to_owned(), file);
        }
        Ok(self.files.get_mut(name).expect("just opened"))
    }

    pub(super) fn read(&mut self, name: &str, path_id: i64, class: i32) -> Result<crate::unity::Value> {
        let tree = if class == SPRITE { self.sprite_tree.clone() } else { self.texture_tree.clone() };
        self.read_by(name, path_id, class, &tree)
    }

    /// An object of one of the build's files read by a tree of its class
    /// borrowed from a bundle (`tree_of`).
    pub fn read_by(&mut self, name: &str, path_id: i64, class: i32, tree: &crate::unity::typetree::TypeTree) -> Result<crate::unity::Value> {
        let file = self.file(name)?;
        let object = *file.meta.object(path_id).with_context(|| format!("no object {path_id} in {name}"))?;
        anyhow::ensure!(object.class_id == class, "object {path_id} in {name} is of class {}, not {class}", object.class_id);
        let big_endian = file.meta.big_endian;
        let bytes = file.bytes(object.offset, object.size)?;
        let mut r = Reader { data: &bytes, pos: 0, big_endian };
        tree.read(&mut r).with_context(|| format!("object {path_id} of {name}"))
    }

    /// The numbers of the objects of a class in one of the build's files.
    pub fn objects(&mut self, name: &str, class: i32) -> Result<Vec<i64>> {
        Ok(self.file(name)?.meta.objects.iter().filter(|object| object.class_id == class).map(|object| object.path_id).collect())
    }

    /// A sprite of that file and number.
    pub fn sprite(&mut self, name: &str, path_id: i64) -> Result<SpriteCut> {
        let v = self.read(name, path_id, SPRITE)?;
        let rd = v.req("m_RD")?;
        let texture = rd.req("texture")?.as_pptr().context("the sprite's texture")?;
        let externals = &self.file(name)?.meta.externals;
        let file = match texture.file_id {
            0 => name.to_owned(),
            n => externals.get(n as usize - 1).map(|outside| outside.rsplit('/').next().unwrap_or(outside).to_owned()).context("the texture's file")?,
        };
        let rect = rd.req("textureRect")?;
        let field = |value: &crate::unity::Value, name: &str| value.get(name).and_then(|f| f.as_f32()).unwrap_or(0.0);
        let border = v.req("m_Border")?.as_vec4().unwrap_or([0.0; 4]);
        let whole = v.req("m_Rect")?;
        let offset = rd.req("textureRectOffset")?;
        // Packed (the lowest bit), and turned or flipped (bits 2 to 5).
        let settings = rd.get("settingsRaw").and_then(|s| s.as_u32()).unwrap_or(0);
        Ok(SpriteCut {
            name: v.req("m_Name")?.as_str().unwrap_or("").to_owned(),
            texture: (file, texture.path_id),
            rect: [field(rect, "x"), field(rect, "y"), field(rect, "width"), field(rect, "height")],
            border,
            pixels_per_unit: v.req("m_PixelsToUnits")?.as_f32().unwrap_or(100.0),
            whole: [field(whole, "x"), field(whole, "y"), field(whole, "width"), field(whole, "height")],
            offset: [field(offset, "x"), field(offset, "y")],
            turned: settings & 1 != 0 && (settings >> 2) & 0xf != 0,
        })
    }

    /// A texture of that file and number, with its levels: in the file
    /// itself, or in the file beside it that its stream names.
    pub fn texture(&mut self, name: &str, path_id: i64) -> Result<crate::unity::extract::TextureData> {
        self.texture_of_class(name, path_id, class::TEXTURE_2D)
    }

    /// A cubemap of that file and number: a texture of six images (its
    /// faces, +x, −x, +y, −y, +z, −z, each with its levels), which the
    /// engine keeps as a texture's fields with one more after them, read by
    /// the texture's tree.
    pub fn cubemap(&mut self, name: &str, path_id: i64) -> Result<crate::unity::extract::TextureData> {
        self.texture_of_class(name, path_id, CUBEMAP)
    }

    /// The name of the file a pointer of a file of the build leads into, by
    /// the pointer's first number (0 the file itself).
    pub fn file_name(&mut self, name: &str, file_id: i32) -> Result<String> {
        if file_id == 0 {
            return Ok(name.to_owned());
        }
        let externals = &self.file(name)?.meta.externals;
        let outside = externals.get(file_id as usize - 1).with_context(|| format!("no file {file_id} among {name}'s"))?;
        // The engine's own meshes (a level's `Cube` collider) are in the
        // build's `Resources` folder, the rest beside the levels.
        Ok(match outside.rsplit('/').next().unwrap_or(outside) {
            "unity default resources" => "Resources/unity default resources".to_owned(),
            last => last.to_owned(),
        })
    }

    fn texture_of_class(&mut self, name: &str, path_id: i64, class: i32) -> Result<crate::unity::extract::TextureData> {
        let tree = self.texture_tree.clone();
        let v = self.read_by(name, path_id, class, &tree)?;
        let inline = v.req("image data")?.as_bytes().unwrap_or(&[]);
        let data = if !inline.is_empty() {
            inline.to_vec()
        } else {
            let stream = v.req("m_StreamData")?;
            let (path, offset, size) = (stream.req("path")?.as_str().unwrap_or(""), stream.req("offset")?.as_usize().unwrap_or(0), stream.req("size")?.as_usize().unwrap_or(0));
            let beside = self.folder.join(path.rsplit('/').next().unwrap_or(path));
            let read = || -> std::io::Result<Vec<u8>> {
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(&beside)?;
                file.seek(SeekFrom::Start(offset as u64))?;
                let mut data = vec![0; size];
                file.read_exact(&mut data)?;
                Ok(data)
            };
            read().with_context(|| format!("reading {}", beside.display()))?
        };
        Ok(crate::unity::extract::TextureData { data, ..crate::unity::extract::texture_header(&v)? })
    }
}
