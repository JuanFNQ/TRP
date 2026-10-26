//! The camera's PRISM effect as a camera prefab keeps it (`PrismEffects`;
//! `docs/spec/lighting.md`, "PRISM: the exposure, the vignette and the
//! tonemap"): its switches and figures, read from the script's fields in
//! the order the game's code declares them.

use anyhow::{Context, Result};

use crate::eye_burn::pointer;
use crate::screen::Scene;
use crate::unity::reader::Reader;
use crate::Install;

/// `PrismEffects`' figures this work draws by, and the switches of what it
/// does not.
#[derive(Debug, Clone, PartialEq)]
pub struct PrismSettings {
    pub use_bloom: bool,
    pub use_vignette: bool,
    pub vignette_start: f32,
    pub vignette_end: f32,
    pub vignette_strength: f32,
    pub vignette_color: [f32; 4],
    pub use_night_vision: bool,
    pub use_noise: bool,
    pub use_chromatic_aberration: bool,
    pub use_tonemap: bool,
    /// `TonemapType`: 0 Filmic (the one this work draws), 1 RomB, 2 ACES,
    /// the order the game's enum declares.
    pub tonemap_type: i32,
    pub tone: [f32; 3],
    pub secondary_tone: [f32; 3],
    pub use_exposure: bool,
    /// The middle grey and the speed as stored; the game takes the
    /// environment manager's on every frame instead (`EnvironmentManager`).
    pub middle_grey: f32,
    pub lower_limit: f32,
    pub upper_limit: f32,
    pub speed: f32,
    /// The side the brightness is halved down to (`histWidth`,
    /// `histHeight`).
    pub hist: [i32; 2],
    pub use_gamma_correction: bool,
    pub gamma: f32,
    pub use_dof: bool,
    pub use_lut: bool,
    pub use_sharpen: bool,
    pub use_fog: bool,
    pub use_ambient_obscurance: bool,
}

/// A flag of a script's: one byte, padded to four.
fn flag(r: &mut Reader) -> Result<bool> {
    let on = r.u8()? != 0;
    r.align(4);
    Ok(on)
}

fn color(r: &mut Reader) -> Result<[f32; 4]> {
    Ok([r.f32()?, r.f32()?, r.f32()?, r.f32()?])
}

impl PrismSettings {
    /// From the fields of a `PrismEffects` script.
    pub fn read(fields: &[u8]) -> Result<Self> {
        let mut r = Reader::new(fields);
        pointer(&mut r)?; // its preset
        flag(&mut r)?; // a parent
        flag(&mut r)?; // a child
        // Its materials and shaders, the exposure's master, the ambient
        // obscurance's material and shader, the third material and shader.
        for _ in 0..9 {
            pointer(&mut r)?;
        }
        flag(&mut r)?; // the big pass
        flag(&mut r)?; // the separable blur
        pointer(&mut r)?; // the lens dirt
        flag(&mut r)?; // the lens dirt's use
        let use_bloom = flag(&mut r)?;
        flag(&mut r)?; // its screen blend
        r.skip(4)?; // its kind
        flag(&mut r)?; // its debug
        r.skip(4 * 5)?; // its downsample, passes, intensity, threshold, dirt
        flag(&mut r)?; // its stability
        flag(&mut r)?; // the screens' blur
        r.skip(4)?; // the pass it is taken from
        let use_vignette = flag(&mut r)?;
        let (vignette_start, vignette_end, vignette_strength) = (r.f32()?, r.f32()?, r.f32()?);
        let vignette_color = color(&mut r)?;
        let use_night_vision = flag(&mut r)?;
        r.skip(16 * 2 + 4 * 2)?; // its colours, contribution and sensitivity
        let use_noise = flag(&mut r)?;
        pointer(&mut r)?;
        r.skip(4 * 3)?; // its scale, intensity and kind
        let use_chromatic_aberration = flag(&mut r)?;
        r.skip(4 * 5)?; // its kind, distances, intensity and blur's width
        flag(&mut r)?; // its blur
        let use_tonemap = flag(&mut r)?;
        let tonemap_type = r.i32()?;
        let tone = [r.f32()?, r.f32()?, r.f32()?];
        let secondary_tone = [r.f32()?, r.f32()?, r.f32()?];
        let use_exposure = flag(&mut r)?;
        flag(&mut r)?; // its debug view
        let (middle_grey, lower_limit, upper_limit, speed) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
        let hist = [r.i32()?, r.i32()?];
        let use_gamma_correction = flag(&mut r)?;
        let gamma = r.f32()?;
        let use_dof = flag(&mut r)?;
        // The depth of field's own: forced median, near blur, full screen
        // blur, five figures, its samples, the skybox's blur, its debug,
        // its focus.
        for _ in 0..3 {
            flag(&mut r)?;
        }
        r.skip(4 * 6)?;
        flag(&mut r)?;
        flag(&mut r)?;
        pointer(&mut r)?;
        let use_lut = flag(&mut r)?;
        pointer(&mut r)?;
        pointer(&mut r)?;
        let skip_string = |r: &mut Reader| -> Result<()> {
            let length = r.u32()? as usize;
            r.skip(length)?;
            r.align(4);
            Ok(())
        };
        skip_string(&mut r)?;
        r.skip(4)?; // its share
        flag(&mut r)?; // the second lookup
        pointer(&mut r)?;
        pointer(&mut r)?;
        skip_string(&mut r)?;
        r.skip(4)?;
        let use_sharpen = flag(&mut r)?;
        r.skip(4)?;
        let use_fog = flag(&mut r)?;
        flag(&mut r)?; // on the skybox
        r.skip(4 * 3 + 16 * 2 + 4)?; // intensity, start, distance, colours, height
        let use_ambient_obscurance = flag(&mut r)?;
        Ok(Self {
            use_bloom,
            use_vignette,
            vignette_start,
            vignette_end,
            vignette_strength,
            vignette_color,
            use_night_vision,
            use_noise,
            use_chromatic_aberration,
            use_tonemap,
            tonemap_type,
            tone,
            secondary_tone,
            use_exposure,
            middle_grey,
            lower_limit,
            upper_limit,
            speed,
            hist,
            use_gamma_correction,
            gamma,
            use_dof,
            use_lut,
            use_sharpen,
            use_fog,
            use_ambient_obscurance,
        })
    }

    /// The camera prefab of an engine part of that number in a file of the
    /// build (`LevelSettings.CameraPrefab`, a pointer to its `Camera`).
    pub fn of_camera(install: &Install, file: &str, part: i64) -> Result<Self> {
        let camera = Scene::load_prefab_of_script(install, file, part)?;
        Self::of_scene(&camera)
    }

    /// The Resources prefab `Cam2`, the game's camera where a level names
    /// none.
    pub fn of_cam2(install: &Install) -> Result<Self> {
        let (_, file, root) = crate::screen::resource_paths(install)?.into_iter().find(|(path, ..)| path == "cam2").context("no Resources prefab cam2")?;
        Self::of_scene(&Scene::load_prefab(install, &file, root)?)
    }

    fn of_scene(camera: &Scene) -> Result<Self> {
        let prism = camera.behaviour(0, "PrismEffects").context("the camera has no PrismEffects")?;
        Self::read(camera.fields(prism)).context("PrismEffects")
    }
}

/// `CC_Vintage.Filter`'s names, in the order the game's enum declares them:
/// each but `None` a table of `Resources/Instagram/`.
const FILTERS: [&str; 28] = [
    "None", "K506", "Zabid", "Cognac", "Edwards", "Cheese", "LateGoose", "Bread", "Montreal", "Feather", "Jason", "Fahrenheit", "Owl", "Chillwave", "Albert", "Bayswater", "Atlanta",
    "Felicity", "Stefano", "Boost", "Emilia", "Doze", "Clifden", "Blender", "Tokyo", "Walk", "Olive", "Hotshot",
];

/// The camera's own colour filter (`CC_Vintage`, a `CC_LookupFilter`; the
/// lighting spec, "The camera's colour filter"): on in every level's camera
/// as the game ships it, apart from the player's optional colour filter of
/// the Post-FX settings (`GradingPostFX`, off). Its fields after the
/// shader's pointer: the table's pointer, the amount, the filter.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorFilter {
    pub enabled: bool,
    pub amount: f32,
    /// `CC_Vintage.Filter` (10 Jason).
    pub filter: i32,
}

impl ColorFilter {
    /// The filter's name, whose table the camera loads on its first frame
    /// (`Resources.Load("Instagram/" + filter)`), or none.
    pub fn name(&self) -> Option<&'static str> {
        FILTERS.get(self.filter as usize).copied().filter(|name| *name != "None")
    }

    /// The filter of a level's camera prefab (as `PrismSettings::of_camera`).
    pub fn of_camera(install: &Install, file: &str, part: i64) -> Result<Option<Self>> {
        let camera = Scene::load_prefab_of_script(install, file, part)?;
        let Some(vintage) = camera.behaviour(0, "CC_Vintage") else { return Ok(None) };
        let mut r = Reader::new(camera.fields(vintage));
        pointer(&mut r)?;
        pointer(&mut r)?;
        Ok(Some(Self { enabled: vintage.enabled, amount: r.f32()?, filter: r.i32()? }))
    }

    /// Its table: Resources' texture of the filter's name, 64 levels of
    /// blue as 8 by 8 tiles of 64 by 64 (red across, green down).
    pub fn table(&self, install: &Install) -> Result<FilterTable> {
        let name = self.name().context("the filter is none")?;
        let wanted = format!("instagram/{}", name.to_lowercase());
        let (_, file, id) = crate::screen::resource_paths(install)?.into_iter().find(|(path, ..)| *path == wanted).with_context(|| format!("no Resources texture {wanted}"))?;
        let texture = crate::screen::ScenePictures::new(install)?.texture(&file, id)?;
        let levels = crate::unity::texture::mips(&texture)?;
        let rgba = crate::unity::texture::decode_rgba(&texture, *levels.first().context("the table has no level")?)?;
        Ok(FilterTable { rgba, width: texture.width, height: texture.height, clamp: texture.clamp, point: texture.point })
    }
}

/// The camera's scattering of the sky's light over the scene
/// (`TOD_Scattering`, an opaque image effect; the lighting spec, "The sky's
/// scattering over the scene"): its fields as the prefab keeps them (two
/// switches, the shader's and the dithering texture's pointers, the
/// density, the height falloff, the sunrise glow, the MBOIT switch, the
/// zero level). The level settings' height falloff and zero level are taken
/// in place of its own (`FromLevelSettings`).
#[derive(Debug, Clone, PartialEq)]
pub struct CameraScattering {
    pub enabled: bool,
    pub lighten: bool,
    pub from_level_settings: bool,
    /// The dithering texture: its file of the build and number.
    pub dithering: (String, i64),
    pub global_density: f32,
    pub height_falloff: f32,
    pub sunrise_glow: f32,
    pub mboit: bool,
    pub zero_level: f32,
}

impl CameraScattering {
    pub fn of_camera(install: &Install, file: &str, part: i64) -> Result<Option<Self>> {
        let camera = Scene::load_prefab_of_script(install, file, part)?;
        let Some(scattering) = camera.behaviour(0, "TOD_Scattering") else { return Ok(None) };
        let mut r = Reader::new(camera.fields(scattering));
        let flag = |r: &mut Reader| -> Result<bool> { Ok(r.i32()? != 0) };
        let lighten = flag(&mut r)?;
        let from_level_settings = flag(&mut r)?;
        pointer(&mut r)?;
        let dithering = pointer(&mut r)?;
        let dithering_file = crate::screen::ScenePictures::new(install)?.file_name(file, dithering.0)?;
        Ok(Some(Self {
            enabled: scattering.enabled,
            lighten,
            from_level_settings,
            dithering: (dithering_file, dithering.1),
            global_density: r.f32()?,
            height_falloff: r.f32()?,
            sunrise_glow: r.f32()?,
            mboit: flag(&mut r)?,
            zero_level: r.f32()?,
        }))
    }

    /// The dithering texture's alpha, its rows as stored, and its side.
    pub fn dithering(&self, install: &Install) -> Result<(Vec<u8>, u32, u32)> {
        let texture = crate::screen::ScenePictures::new(install)?.texture(&self.dithering.0, self.dithering.1)?;
        let (w, h) = (texture.width, texture.height);
        // `Alpha8` (1): its bytes are its alpha.
        if texture.format == 1 {
            let alpha = texture.data.get(..(w * h) as usize).context("a short dithering texture")?.to_vec();
            return Ok((alpha, w, h));
        }
        let levels = crate::unity::texture::mips(&texture)?;
        let rgba = crate::unity::texture::decode_rgba(&texture, *levels.first().context("the dithering has no level")?)?;
        Ok((rgba.chunks_exact(4).map(|p| p[3]).collect(), w, h))
    }
}

/// A colour filter's table as eight bits a channel, its rows as stored (the
/// first at the bottom), and how its texture is sampled: clamped across and
/// up or repeated, at its nearest texel or blended.
#[derive(Debug, Clone)]
pub struct FilterTable {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub clamp: [bool; 2],
    pub point: bool,
}
