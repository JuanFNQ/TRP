//! Resource sprites, build fonts and materials' shaders and properties.

use super::*;

/// A sprite among the build's own Resources by its path (`Slots/
/// Equipment/TacticalVest`, as `ResourcesCache.Pop<Sprite>` names it): its
/// file and number.
pub fn resource_sprite(install: &Install, path: &str) -> Option<(String, i64)> {
    let wanted = path.to_lowercase();
    let paths = resource_paths(install).ok()?;
    let mut build: Option<(String, BuildFile)> = None;
    for (at, file, id) in paths.into_iter().filter(|(at, ..)| *at == wanted) {
        let _ = at;
        if build.as_ref().is_none_or(|(name, _)| *name != file) {
            build = Some((file.clone(), BuildFile::open(install, &file).ok()?));
        }
        let (_, opened) = build.as_ref()?;
        if opened.meta.object(id).is_some_and(|object| object.class_id == SPRITE) {
            return Some((file, id));
        }
    }
    None
}

/// The fonts the build keeps whole in one of its own files (`Font`, the
/// engine's class 128, `resources.assets`): each one's name and its font
/// file's bytes (TrueType or OpenType), read by the engine's own order of
/// a font's fields; one whose bytes are not a font file is left out.
pub fn build_fonts(install: &Install, file: &str) -> Result<Vec<(String, Vec<u8>)>> {
    const FONT: i32 = 128;
    let mut build = BuildFile::open(install, file)?;
    let mut fonts = Vec::new();
    let objects: Vec<_> = build.meta.objects.iter().filter(|object| object.class_id == FONT).copied().collect();
    for object in objects {
        let Ok(bytes) = build.bytes(object.offset, object.size) else { continue };
        let big_endian = build.meta.big_endian;
        let read = |r: &mut Reader| -> Result<(String, Vec<u8>)> {
            let name = string(r)?;
            r.skip(4 + 12 + 4 + 12)?; // line spacing, material, size, texture
            r.skip(4 + 4 + 4 + 4 + 4)?; // ASCII start, tracking, spacing, padding, case
            let characters = r.u32()? as usize;
            r.skip(characters * 44)?;
            let kernings = r.u32()? as usize;
            r.skip(kernings * 8)?;
            r.skip(4)?; // pixel scale
            let size = r.u32()? as usize;
            Ok((name, r.bytes(size)?.to_vec()))
        };
        let Ok((name, font)) = read(&mut Reader { data: &bytes, pos: 0, big_endian }) else { continue };
        let signature = font.get(..4).unwrap_or(&[]);
        if [&[0u8, 1, 0, 0][..], b"OTTO", b"true", b"ttcf"].contains(&signature) {
            fonts.push((name, font));
        }
    }
    Ok(fonts)
}

/// The font a TextMeshPro font asset is made from, by its name: the font
/// whose name the asset's begins with, the longest such (`Jovanny Lemonad
/// - Bender Outline SDF` is made from `Jovanny Lemonad - Bender`).
pub fn font_for<'a>(asset: &str, fonts: &'a [(String, Vec<u8>)]) -> Option<&'a (String, Vec<u8>)> {
    fonts.iter().filter(|(name, _)| asset.starts_with(name.as_str())).max_by_key(|(name, _)| name.len())
}

/// A material's saved properties as the build keeps them with no type tree
/// (`Material`: its name, shader, keywords, flags, render queue, tags and
/// passes off, then its textures', whole numbers', floats' and colours'
/// lists, each a name and its value): its floats and colours by name.
#[derive(Debug, Clone, Default)]
pub struct MaterialProperties {
    pub name: String,
    /// The shader's keywords it turns on (`UNDERLAY_ON`).
    pub keywords: Vec<String>,
    pub floats: HashMap<String, f32>,
    pub colors: HashMap<String, [f32; 4]>,
}

/// Reads a material of one of the build's own files.
pub fn material_properties(install: &Install, file: &str, id: i64) -> Result<MaterialProperties> {
    let (class_id, bytes) = object_bytes(install, file, id)?;
    anyhow::ensure!(class_id == 21, "{file} {id} is no material");
    let mut r = Reader::new(&bytes);
    let name = string(&mut r)?;
    pointer(&mut r)?; // shader
    let strings = |r: &mut Reader| -> Result<Vec<String>> { (0..r.u32()?).map(|_| string(r)).collect() };
    let keywords = strings(&mut r)?;
    strings(&mut r)?; // invalid keywords
    r.skip(4)?; // lightmap flags
    r.skip(2)?; // instancing, double-sided GI
    r.align(4);
    r.skip(4)?; // render queue
    for _ in 0..r.u32()? {
        string(&mut r)?;
        string(&mut r)?;
    }
    strings(&mut r)?; // passes off
    for _ in 0..r.u32()? {
        string(&mut r)?;
        r.skip(12 + 8 + 8)?;
    }
    for _ in 0..r.u32()? {
        string(&mut r)?;
        r.skip(4)?;
    }
    let mut floats = HashMap::new();
    for _ in 0..r.u32()? {
        let key = string(&mut r)?;
        floats.insert(key, r.f32()?);
    }
    let mut colors = HashMap::new();
    for _ in 0..r.u32()? {
        let key = string(&mut r)?;
        colors.insert(key, [r.f32()?, r.f32()?, r.f32()?, r.f32()?]);
    }
    Ok(MaterialProperties { name, keywords, floats, colors })
}

/// The name of the shader a material of one of the build's own files is
/// drawn with (`m_Shader`): the shader's own name as its parsed form keeps
/// it, found as the first run of letters in its bytes that begins with the
/// name's usual heads (`TextMeshPro/`, `UI/`, `Hidden/`), as a build keeps
/// no type tree to read it by.
pub fn material_shader(install: &Install, file: &str, id: i64) -> Result<String> {
    let (_, bytes) = object_bytes(install, file, id)?;
    let mut r = Reader::new(&bytes);
    string(&mut r)?;
    let to = pointer(&mut r)?;
    let shader_file = match to.file {
        0 => file.to_owned(),
        outside => {
            let build = BuildFile::open(install, file)?;
            build.meta.externals.get(outside as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).context("the shader's file")?
        }
    };
    let (_, shader) = object_bytes(install, &shader_file, to.path)?;
    for head in ["TextMeshPro/", "UI/", "Hidden/"] {
        if let Some(at) = shader.windows(head.len()).position(|w| w == head.as_bytes()) {
            let end = shader[at..].iter().position(|b| !(b.is_ascii_graphic() || *b == b' ')).map_or(shader.len(), |n| at + n);
            return Ok(String::from_utf8_lossy(&shader[at..end]).into_owned());
        }
    }
    bail!("no shader's name found in {shader_file} {}", to.path)
}

/// A TextMeshPro font asset's material (`m_Material`, after the asset's
/// name and its hash): its file and number.
pub fn font_material(install: &Install, file: &str, id: i64) -> Result<(String, i64)> {
    let (_, bytes) = object_bytes(install, file, id)?;
    let mut r = Reader::new(&bytes);
    r.skip(12 + 4 + 12)?;
    string(&mut r)?;
    r.skip(4)?;
    let to = pointer(&mut r)?;
    let name = match to.file {
        0 => file.to_owned(),
        outside => {
            let build = BuildFile::open(install, file)?;
            build.meta.externals.get(outside as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).context("the material's file")?
        }
    };
    Ok((name, to.path))
}

/// The font a font asset was made from by its face's family and style,
/// where its own name does not begin with the font's (a font asset named
/// "DON'T USE! Jovanny Lemonad - Bender Bold SDF", whose face is Bender
/// Bold): the font named for the family and the style, or for the family
/// alone in its regular style.
pub fn font_by_face<'a>(family: &str, style: &str, fonts: &'a [(String, Vec<u8>)]) -> Option<&'a (String, Vec<u8>)> {
    let styled = format!("{family} {style}");
    fonts.iter().find(|(name, _)| name.ends_with(&styled)).or_else(|| fonts.iter().find(|(name, _)| style.eq_ignore_ascii_case("regular") && name.ends_with(family)))
}

/// The name of an object of one of the build's own files that is a
/// script's (a `TMP_FontAsset`'s, `BENDER SDF`): after the object it is
/// on, whether it is on, and its script.
pub fn script_name(install: &Install, file: &str, path_id: i64) -> Option<String> {
    let mut build = BuildFile::open(install, file).ok()?;
    let object = *build.meta.object(path_id).filter(|object| object.class_id == class::MONO_BEHAVIOUR)?;
    let bytes = build.bytes(object.offset, object.size).ok()?;
    let mut r = Reader { data: &bytes, pos: 0, big_endian: build.meta.big_endian };
    r.skip(12 + 4 + 12).ok()?;
    string(&mut r).ok()
}
