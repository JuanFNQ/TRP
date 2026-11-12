//! Reading textures' headers and data, and materials.

use super::*;

#[derive(Debug, Clone)]
pub struct TextureData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Unity `TextureFormat` value.
    pub format: i32,
    pub mip_count: u32,
    pub srgb: bool,
    /// All mip levels, largest first, rows bottom to top.
    pub data: Vec<u8>,
    /// Whether the engine streams it by its levels (`m_StreamingMipmaps`,
    /// set as it was imported): with streaming on, one that is not is
    /// drawn whole.
    pub streaming_mipmaps: bool,
    /// Whether it is sampled at its nearest pixel (`m_TextureSettings.
    /// m_FilterMode`, point), not blended between its pixels.
    pub point: bool,
    /// Whether it is clamped at its edges across and up
    /// (`m_TextureSettings.m_WrapU`, `m_WrapV`: `TextureWrapMode.Clamp`,
    /// 1), not repeated: sampled at an edge it gives its own edge, not the
    /// far side's.
    pub clamp: [bool; 2],
    /// How many pictures it holds one after another (`m_ImageCount`: a
    /// cube's six faces, each with all its levels), 1 for a plain one.
    pub faces: u32,
}

/// A texture's wrap modes across and up as clamped or not
/// (`TextureWrapMode`: 0 repeat, 1 clamp, 2 mirror, 3 mirror once; an
/// older file's one `m_WrapMode` for both).
fn clamps(v: &Value) -> [bool; 2] {
    let settings = v.get("m_TextureSettings");
    let mode = |name: &str| settings.and_then(|s| s.get(name)).and_then(|m| m.as_i64());
    let both = mode("m_WrapMode");
    [mode("m_WrapU").or(both) == Some(1), mode("m_WrapV").or(both) == Some(1)]
}

pub fn read_texture(assets: &Assets, obj: ObjRef) -> Result<TextureData> {
    Ok(read_texture_shared(assets, obj)?.0)
}

/// A texture, and where its levels are in their bundle's file when they are
/// kept beside its serialized file (`m_StreamData`), to be read again from
/// there (none where they are in the serialized file itself).
pub fn read_texture_shared(assets: &Assets, obj: ObjRef) -> Result<(TextureData, Option<ResourcePlace>)> {
    let v = assets.read(obj)?;
    let inline = v.req("image data")?.as_bytes().unwrap_or(&[]);
    let (data, place) = if !inline.is_empty() {
        (inline.to_vec(), None)
    } else {
        let stream = v.req("m_StreamData")?;
        let (path, offset, size) = (stream.req("path")?.as_str().unwrap_or(""), stream.req("offset")?.as_usize().unwrap_or(0), stream.req("size")?.as_usize().unwrap_or(0));
        (assets.resource(path, offset, size)?.to_vec(), assets.resource_place(path, offset, size).ok())
    };
    Ok((TextureData { data, ..texture_header(&v)? }, place))
}

/// A texture's own fields but its levels, which are left empty.
pub fn texture_header(v: &Value) -> Result<TextureData> {
    Ok(TextureData {
        name: v.req("m_Name")?.as_str().unwrap_or("").to_owned(),
        width: v.req("m_Width")?.as_u32().unwrap_or(0),
        height: v.req("m_Height")?.as_u32().unwrap_or(0),
        format: v.req("m_TextureFormat")?.as_i64().unwrap_or(0) as i32,
        mip_count: v.req("m_MipCount")?.as_u32().unwrap_or(1).max(1),
        srgb: v.req("m_ColorSpace")?.as_i64().unwrap_or(0) == 1,
        data: Vec::new(),
        streaming_mipmaps: v.get("m_StreamingMipmaps").and_then(|streamed| streamed.as_bool()).unwrap_or(false),
        point: v.get("m_TextureSettings").and_then(|settings| settings.get("m_FilterMode")).and_then(|mode| mode.as_i64()) == Some(0),
        clamp: clamps(v),
        faces: v.get("m_ImageCount").and_then(Value::as_u32).unwrap_or(1).max(1),
    })
}

#[derive(Debug, Clone, Default)]
pub struct MaterialData {
    pub name: String,
    pub textures: HashMap<String, ObjRef>,
    pub floats: HashMap<String, f32>,
    pub colors: HashMap<String, [f32; 4]>,
    pub keywords: Vec<String>,
    /// Its render queue (`m_CustomRenderQueue`): -1 for its shader's own.
    pub queue: i32,
    /// Its shader's object by its path id (`m_Shader`), in whichever file
    /// holds it (the `shaders` bundle for the game's own); 0 for none.
    pub shader: i64,
}

pub fn read_material(assets: &Assets, obj: ObjRef) -> Result<MaterialData> {
    let v = assets.read(obj)?;
    let mut out = MaterialData {
        name: v.req("m_Name")?.as_str().unwrap_or("").to_owned(),
        queue: v.get("m_CustomRenderQueue").and_then(Value::as_i64).map_or(-1, |queue| queue as i32),
        shader: v.get("m_Shader").and_then(Value::as_pptr).map_or(0, |shader| shader.path_id),
        ..Default::default()
    };
    for k in v.at("m_ValidKeywords").and_then(Value::as_array).unwrap_or(&[]) {
        out.keywords.extend(k.as_str().map(str::to_owned));
    }
    let props = v.req("m_SavedProperties")?;
    for pair in props.req("m_TexEnvs")?.as_array().unwrap_or(&[]) {
        let name = pair.req("first")?.as_str().unwrap_or("");
        let ptr = pair.req("second.m_Texture")?.as_pptr().context("texture reference")?;
        if let Some(tex) = assets.resolve_loaded(obj.file, ptr) {
            out.textures.insert(name.to_owned(), tex);
        }
    }
    for pair in props.req("m_Floats")?.as_array().unwrap_or(&[]) {
        if let (Some(name), Some(value)) = (pair.req("first")?.as_str(), pair.req("second")?.as_f32()) {
            out.floats.insert(name.to_owned(), value);
        }
    }
    for pair in props.req("m_Colors")?.as_array().unwrap_or(&[]) {
        let c = pair.req("second")?;
        let rgba = ["r", "g", "b", "a"].map(|k| c.get(k).and_then(Value::as_f32).unwrap_or(0.0));
        out.colors.insert(pair.req("first")?.as_str().unwrap_or("").to_owned(), rgba);
    }
    Ok(out)
}
