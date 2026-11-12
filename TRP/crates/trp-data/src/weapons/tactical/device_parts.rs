//! A device's light shafts, lights, lasers, flares and their materials.

use super::*;

/// A light's shaft (`VolumetricLight`, on the white lights; ray-marched by
/// the camera where the graphics option is on): its script's figures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shaft {
    pub sample_count: u32,
    pub scattering: f32,
    pub extinction: f32,
    pub skybox_extinction: f32,
    pub mie_g: f32,
    pub height_fog: bool,
    pub height_scale: f32,
    pub ground_level: f32,
    pub noise: bool,
    pub noise_scale: f32,
    pub noise_intensity: f32,
    pub noise_intensity_offset: f32,
    pub noise_velocity: [f32; 2],
    pub max_ray_length: f32,
}

/// One light of a device (Unity's `Light`): its object, kind, colour,
/// intensity, reach, cone, shadows and cookie as its prefab has them, and
/// what drives it.
#[derive(Debug, Clone)]
pub struct DeviceLight {
    pub node: usize,
    /// The mode whose object it is under (0 for `mode_001`), or none: under
    /// the off object, or under none.
    pub mode: Option<usize>,
    pub enabled: bool,
    /// A spot (`m_Type` 0); otherwise a point (2).
    pub spot: bool,
    pub color: [f32; 3],
    /// The light's own intensity (`m_Intensity`), and the one it is drawn
    /// with: its distance fade's (`CullingLightObject`, on the same object)
    /// `_maxLightIntensity` where that takes its own figure
    /// (`_useLightIntensityFromEditor`, every device's), times its fade by
    /// the camera's distance, 1 within 45 m (one's own lights always).
    pub intensity: f32,
    pub drawn_intensity: f32,
    pub range: f32,
    /// The cone's whole angle and its inner one, in degrees.
    pub spot_angle: f32,
    pub inner_spot_angle: f32,
    /// `LightShadows`: 0 none, 1 hard, 2 soft; their resolution (`m_Resolution`:
    /// −1 the quality's, 0 low, 1 medium, 2 high, 3 very high), strength,
    /// bias, normal bias and near plane.
    pub shadows: u32,
    pub shadow_resolution: i32,
    pub shadow_strength: f32,
    pub shadow_bias: f32,
    pub shadow_normal_bias: f32,
    pub shadow_near: f32,
    pub cookie: Option<Arc<TextureData>>,
    pub culling_mask: u32,
    /// An infrared light (`IkLight`): it lights nothing for a camera without
    /// night vision.
    pub infrared: bool,
    pub shaft: Option<Shaft>,
    /// Whether a flare sits beside it (`FlareLight` on a sibling object).
    pub flare: bool,
}

impl DeviceLight {
    /// A device's LED: a point light that lights only its own spot (a
    /// reach of a centimetre).
    pub fn is_led(&self) -> bool {
        !self.spot && self.range <= 0.05
    }
}

/// The laser's beam material (`Custom/LaserBeam`): its colour, its factor
/// without and with night vision (`_Factor`, `_IkFactor`), the noise's
/// scale, and its pictures: the soft spot across the ribbon (`_MainTex`)
/// and the 3D noise (`_Tex3D`).
#[derive(Debug, Clone)]
pub struct BeamMaterial {
    pub color: [f32; 4],
    pub factor: f32,
    pub ik_factor: f32,
    pub noise_scale: f32,
    pub main: Option<Arc<TextureData>>,
    pub noise: Option<Arc<Volume>>,
}

/// The laser's dot material (`Custom/LaserPoint`): its colour, factors,
/// the field of view below which it shrinks (`_MinFov`), its size by the
/// camera's distance (`_SizeFactorViewMin`, `_SizeFactorViewMax`,
/// `_MaxDist`), and its pictures (`_MainTex`, `_NoiseTex`).
#[derive(Debug, Clone)]
pub struct PointMaterial {
    pub color: [f32; 4],
    pub factor: f32,
    pub ik_factor: f32,
    pub min_fov: f32,
    pub size_view: [f32; 2],
    pub max_dist: f32,
    pub main: Option<Arc<TextureData>>,
    pub noise: Option<Arc<TextureData>>,
}

/// One laser of a device (`LaserBeam`, on its `beam` object, whose z is the
/// beam's way): its script's figures and its two materials.
#[derive(Debug, Clone)]
pub struct DeviceLaser {
    pub node: usize,
    pub mode: Option<usize>,
    pub ray_start: f32,
    pub max_distance: f32,
    pub use_point_light: bool,
    pub mask: u32,
    /// The ribbon's half width.
    pub beam_size: f32,
    /// The dot's size near and far (`PointSizeClose`, `PointSizeFar`).
    pub point_size: [f32; 2],
    pub surface_offset: f32,
    pub light_range: f32,
    pub light_intensity: f32,
    pub intensity_factor: f32,
    /// The dot's light's cone near and far, in degrees (`AngleCloseFar`).
    pub angle_close_far: [f32; 2],
    pub cookie: Option<Arc<TextureData>>,
    pub beam: Option<BeamMaterial>,
    pub point: Option<PointMaterial>,
    /// Its own `IkLight` (an infrared beam).
    pub infrared: bool,
}

/// One flare of a light's (`MultiFlare.Flare`): its size (both ways) and
/// alpha, its colour, its kind (`FlareType`: 0 normal, 1 off screen, 2
/// noise, 3 flashlight), the distances its size and alpha go between, its
/// size and alpha at each end, and its picture's number in the atlas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flare {
    pub scale: [f32; 2],
    pub alpha: f32,
    pub color: [f32; 4],
    pub kind: i32,
    pub min_dist: f32,
    pub max_dist: f32,
    pub min_scale: f32,
    pub max_scale: f32,
    pub min_alpha: f32,
    pub max_alpha: f32,
    pub tex_id: i32,
}

/// A light's flares (`FlareLight`, on a mode's `NewMultiFlareLight`): its
/// object and mode, its own size and alpha for all of them (`_totalScale`,
/// `_totalAlpha`), its flares, what moves its alpha (`SingleFlareController`
/// on the same object: `_changeSpeed`, `_minAngle`; none where none is), and
/// whether an infrared light's (`IkLight.FlareLight` names it: never shown
/// without night vision).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceFlare {
    pub node: usize,
    pub mode: Option<usize>,
    pub total_scale: f32,
    pub total_alpha: f32,
    pub flares: Vec<Flare>,
    pub controller: Option<FlareController>,
    pub infrared: bool,
}

/// `SingleFlareController`'s figures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlareController {
    pub change_speed: f32,
    pub min_angle: f32,
}

/// The flares' two pictures, as the level's flare settings name them
/// (`FlareSceneSettings`, Factory by day's `level525`; tactical-devices
/// spec, "The light shaft and the flare"): the atlas (`ProFlareAtlas`
/// "FlareAtlas"'s picture "Atlas2", `sharedassets3.assets` #11) and the
/// noise flares' grain ("Noise", `resources.assets` #1379), each checked by
/// its name.
pub fn flare_pictures(install: &crate::Install) -> Result<(TextureData, TextureData)> {
    let mut pictures = crate::interface::screen::ScenePictures::new(install)?;
    let mut named = |file: &str, id: i64, name: &str| -> Result<TextureData> {
        let texture = pictures.texture(file, id)?;
        anyhow::ensure!(texture.name == name, "{file} #{id} is {:?}, not the flares' {name:?}", texture.name);
        Ok(texture)
    };
    Ok((named("sharedassets3.assets", 11, "Atlas2")?, named("resources.assets", 1379, "Noise")?))
}

pub(super) fn read_flare(v: &Value) -> Flare {
    let f = |name: &str| v.get(name).and_then(Value::as_f32).unwrap_or(0.0);
    let at = |path: &str| v.at(path).and_then(Value::as_f32).unwrap_or(0.0);
    Flare {
        scale: [at("_scale.x"), at("_scale.y")],
        alpha: f("_alpha"),
        color: [at("_color.r"), at("_color.g"), at("_color.b"), at("_color.a")],
        kind: v.get("_flareType").and_then(Value::as_i64).unwrap_or(0) as i32,
        min_dist: f("_minDist"),
        max_dist: f("_maxDist"),
        min_scale: f("_minScale"),
        max_scale: f("_maxScale"),
        min_alpha: f("_minAlpha"),
        max_alpha: f("_maxAlpha"),
        tex_id: v.get("_texId").and_then(Value::as_i64).unwrap_or(0) as i32,
    }
}
