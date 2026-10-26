//! The range lit as Customs in the recorded morning (`docs/spec/lighting.md`,
//! "Customs by its time of day"), with the options' `CUSTOMS LIGHT` on: the
//! sun, the ambient harmonics as an environment map, the sky
//! (`tod_sky_view`), all worked out by the game's time-of-day rules from
//! the install (`trp_sim::tod`); the camera in HDR with no tonemapper of
//! Bevy's (PRISM's in its place, `prism_view`), and the figures the effects'
//! shaders are lit by.
//!
//! The game shades in its gamma space and the app in Bevy's linear light;
//! the frame is turned into the game's numbers after the scene (`gamma_ui`,
//! by a plain 2.2 power with this light: the sRGB curve's straight part near
//! black would make a product of two dim figures, an albedo and an ambient,
//! three times darker than the game's). So each of the game's figures is
//! given to Bevy made linear by the same power, and Bevy's exposure is put
//! where a light of intensity one is one (EV100 0, as the icon studio's:
//! `icons`). `PROVISIONAL` (lighting spec, "On the range"): a product of a
//! picture's colour and one light is the game's (the picture's darkest
//! colours a little lighter: Bevy reads a picture by the sRGB curve); sums
//! and fall-off with angle are linear light's.

use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
use trp_data::prism::{CameraScattering, ColorFilter, PrismSettings};
use trp_data::screen::BuildMesh;
use trp_data::tod::TimeOfDayLevel;
use trp_sim::ambient::Harmonics;
use trp_sim::tod::{Moment, SkyGlobals, CUSTOMS_RECORDED_MORNING};

/// What the frame's exposure takes a light of intensity one to (Bevy's
/// `Exposure` at EV100 0 multiplies by 1/1.2).
pub const EXPOSURE_EV100: f32 = 0.0;
pub const EXPOSURE_SCALE: f32 = 1.2;

/// The level's light at its moment, read at the start.
/// The camera's sharpening as the level has it (`sharpen_view`): its
/// figures, the strength it runs at, and the weather's desaturation.
#[derive(Debug, Clone)]
pub struct Sharpening {
    pub data: trp_data::camera_effects::SharpenData,
    pub strength: f32,
    pub weather: f32,
}

#[derive(Resource, Clone)]
pub struct LevelLighting {
    /// The way the sun's light goes, in the app's axes.
    pub sun_forward: Vec3,
    /// The sun's colour times its intensity, as the game's shaders are given
    /// it (`_LightColor0`, `unity_LightColor`), in gamma.
    pub sun: [f32; 3],
    /// `_DirectionLightShadow`.
    pub direction_light_shadow: f32,
    /// The engine's flat ambient (the level's sky colour), in gamma: twice
    /// `glstate_lightmodel_ambient`.
    pub flat_ambient: [f32; 3],
    /// The engine's own harmonics' constant: the flat ambient made linear
    /// (`RenderSettings::SetupAmbient`, `ambientProbe`).
    pub probe_constant: [f32; 3],
    /// The game's ambient harmonics (`ToDController`'s), and the light from
    /// straight above (`_EFT_Ambient`).
    pub harmonics: Harmonics,
    pub eft_ambient: [f32; 3],
    /// The sky: the dome's mesh and its shaders' figures.
    pub sky: TimeOfDaySky,
    /// The level's camera's PRISM, and its own colour filter where it has
    /// one switched on (`color_filter_view`; none with `CAMERA COLOR
    /// FILTER` off).
    pub prism: PrismSettings,
    pub color_filter: Option<ColorFilter>,
    /// The camera's sharpening and desaturation (`sharpen_view`; none with
    /// `CAMERA SHARPENING` off or where it could not be read).
    pub sharpen: Option<Sharpening>,
    /// The level's indoor triggers' common figures (the camera's within
    /// the range hall).
    pub indoor: Option<trp_sim::prism::IndoorFigures>,
    /// The camera's scattering of the sky over the scene, where it has one
    /// switched on (`scattering_view`; none with `SKY SCATTERING` off): its
    /// height falloff, zero level, density and sunrise glow as it runs, and
    /// its dithering's 64 alphas.
    pub scattering: Option<SceneScattering>,
    /// The environment manager's figures for the open air: the exposure's
    /// middle grey, its speed, the fade.
    pub outdoor: [f32; 3],
    /// The weather's clouds, where asked for (`clouds_view`; none with
    /// `CUSTOMS CLOUDS` off).
    pub clouds: Option<crate::clouds_view::SceneClouds>,
    /// The ambient highlight over the hands' stencil type, where the level
    /// has one (`highlight_view`): its harmonics (the time of day's without
    /// the sky's samples and the light from above), packed; its multiplier
    /// at the moment's sun; its blur.
    pub highlight: Option<(trp_sim::ambient::Packed, f32, f32)>,
    /// The level's least smoke ambient (`_MinAmbientColor`, its settings')
    /// and the time of day's top horizon colour (`_TopHorizontSkyColor`),
    /// which the grenade smoke's shader takes.
    pub min_smoke_ambient: [f32; 3],
    pub top_horizon: [f32; 3],
}

/// The weather's globals the grenade smoke's shader takes, as the recorded
/// morning had them (`local\traces\courses3\0-light-outside`'s scene:
/// `_SunVisibility` 1, `_ClearSky` 1, `_ForceIndoorIntensity` 1;
/// `_ForceIndoor` not among them: `PROVISIONAL`, nought).
const SUN_VISIBILITY: f32 = 1.0;
const CLEAR_SKY: f32 = 1.0;
const FORCE_INDOOR: f32 = 0.0;
const FORCE_INDOOR_INTENSITY: f32 = 1.0;

/// The grenade smoke's level figures (`explosion_view::EffectUniform`'s
/// `smoke_a` to `smoke_d`): `_MinAmbientColor` and `_SunVisibility`; the
/// engine's `unity_AmbientSky` (the flat ambient) and `_ClearSky`;
/// `_TopHorizontSkyColor` and `_ForceIndoor`; `_ForceIndoorIntensity`.
/// Without `CUSTOMS LIGHT`, `ESTIMATE`: the effects' ambient (a thirtieth)
/// as the sky's, no least ambient or horizon.
pub fn smoke_globals(level: Option<&LevelLighting>) -> [Vec4; 4] {
    let (min, sky, top) = match level {
        Some(l) => (l.min_smoke_ambient, l.flat_ambient, l.top_horizon),
        None => ([0.0; 3], [1.0 / 30.0; 3], [0.0; 3]),
    };
    [Vec3::from(min).extend(SUN_VISIBILITY), Vec3::from(sky).extend(CLEAR_SKY), Vec3::from(top).extend(FORCE_INDOOR), Vec4::new(FORCE_INDOOR_INTENSITY, 0.0, 0.0, 0.0)]
}

/// The hall's lamp as the engine hands its vertex lights to a shader: its
/// colour, intensity, range and cone (degrees).
pub fn hall_lamp_figures() -> ([f32; 3], f32, f32, f32) {
    (LAMP_COLOR, LAMP_INTENSITY, LAMP_RANGE, LAMP_CONE)
}

/// The camera's scattering as it runs on the level (`TOD_Scattering`).
#[derive(Clone)]
pub struct SceneScattering {
    pub height_falloff: f32,
    pub zero_level: f32,
    pub density: f32,
    pub sunrise_glow: f32,
    pub dithering: Vec<u8>,
}

/// A time-of-day sky as drawn (`tod_sky_view`).
#[derive(Clone)]
pub struct TimeOfDaySky {
    pub mesh: BuildMesh,
    pub globals: SkyGlobals,
    /// The moon's picture and its tiling (`Time of Day/Moon`), where it
    /// could be read.
    pub moon: Option<trp_data::clouds::LevelMoon>,
}

/// The way a figure of the game's goes to linear light, for any figure at
/// or above nought (above one too): its 2.2 power, the inverse of the
/// encoding `gamma_ui` turns the frame by with this light.
pub fn linear(gamma: f32) -> f32 {
    gamma.max(0.0).powf(2.2)
}

/// The game's surface ambient for a value of its harmonics: held at nought
/// and taken to gamma by Unity's `LinearToGammaSpace` (its shaders' quick
/// form, `1.055 x^0.416667 - 0.055`, held at nought), as the screen ambient
/// pass takes them (the harmonics are linear light's; `_EFT_Ambient`, which
/// the effects take, is the same sum not so taken).
pub fn surface_ambient(sh: f32) -> f32 {
    (1.055 * sh.max(0.0).powf(0.416_667) - 0.055).max(0.0)
}

/// A float as a half float's bits (to nearest; no figure here is out of a
/// half's range).
fn half(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mantissa = bits & 0x7f_ffff;
    if exponent <= 0 {
        if exponent < -10 {
            return sign;
        }
        let m = (mantissa | 0x80_0000) >> (1 - exponent);
        return sign | ((m + 0x1000) >> 13) as u16;
    }
    if exponent >= 31 {
        return sign | 0x7c00;
    }
    // A carry out of the mantissa steps the exponent, as the bits add.
    sign | (((exponent as u32) << 10) + ((mantissa + 0x1000) >> 13)) as u16
}

/// The moment the range is lit at as a run begins: the recorded morning's
/// weather at 09:30 (the user, 2026-10-25: "make 09:30 the default so the
/// app always starts at that time"; the morning was recorded at 09:37, and
/// Customs' light is compared there, `trp-sim/tests/sim/time_of_day.rs`).
pub const RANGE_MOMENT: Moment = Moment { hour: 9.5, ..CUSTOMS_RECORDED_MORNING };

/// Reads Customs' light from the install and works a moment of its time of
/// day out by the game's rules: the moment settled by twelve frames of the
/// game's controller and sky in turn, as the tests compare the recorded
/// morning (`trp-sim/tests/time_of_day.rs`); its camera's PRISM and its
/// environment manager's figures for the open air; its clouds at a quality
/// where asked for (`trp-sim/tests/clouds.rs`).
pub fn read(install: &trp_data::Install, moment: Moment, clouds_quality: Option<trp_sim::clouds::CloudsQuality>) -> Result<LevelLighting> {
    let level = TimeOfDayLevel::customs(install).context("Customs' time of day")?;
    let sky = trp_sim::tod::Sky::of_level(&level).context("Customs' sky's figures")?;
    let stored = trp_sim::tod::stored_atmosphere(&level.sky)?;
    let state = sky.settle(moment.hour, moment.cloudiness, stored, 12);
    let camera_file = level.file_of(level.settings.camera)?;
    // The camera's sharpening (`CC_Sharpen` and its `DesaturateEffect`), at
    // the player's strength (`Sharpen` in `Graphics.ini`, the prefab's
    // where it has none) and the weather's desaturation
    // (`WeatherController.ControlDesaturateEffect`: its `MinMaxAmmount` by the
    // fogginess, which the sky's settled state has).
    let sharpen = match trp_data::camera_effects::SharpenData::of_camera(install, &camera_file, level.settings.camera.1) {
        Ok(data) => {
            let strength = install.display_settings().sharpen.unwrap_or(data.strength);
            let [low, high] = level.weather.vec2("MinMaxAmmount").unwrap_or([0.0, 0.0]);
            let weather = low + (high - low) * state.atmosphere.fogginess;
            Some(Sharpening { data, strength, weather })
        }
        Err(e) => {
            warn!("the camera's sharpening could not be read; it is not drawn: {e:#}");
            None
        }
    };
    let prism = PrismSettings::of_camera(install, &camera_file, level.settings.camera.1).context("Customs' camera's PRISM")?;
    let color_filter = ColorFilter::of_camera(install, &camera_file, level.settings.camera.1).context("Customs' camera's colour filter")?.filter(|f| f.enabled && f.name().is_some());
    // The scattering's own variant (without `LIGHTEN`) is the one ported;
    // its height from the level settings where it says so, as Customs'.
    let scattering = match CameraScattering::of_camera(install, &camera_file, level.settings.camera.1).context("Customs' camera's scattering")? {
        Some(c) if c.enabled && !c.lighten => {
            let (dithering, ..) = c.dithering(install).context("the scattering's dithering")?;
            let (height_falloff, zero_level) = if c.from_level_settings { (level.settings.height_falloff, level.settings.zero_level) } else { (c.height_falloff, c.zero_level) };
            Some(SceneScattering { height_falloff, zero_level, density: c.global_density, sunrise_glow: c.sunrise_glow, dithering })
        }
        _ => None,
    };
    let s = &level.settings;
    let flat = [s.sky_color[0], s.sky_color[1], s.sky_color[2]];
    // The light looks along its own forward way.
    let forward = trp_data::level_light::rotate(state.light_rotation, [0.0, 0.0, 1.0]);
    let indoor = level.common_indoor().map(|(t, _)| trp_sim::prism::IndoorFigures { fade_time: t.fade_time, exposure_speed: t.exposure_speed, exposure_offset: t.exposure_offset });
    let sun = state.light_color.map(|c| c * state.light_intensity);
    // The clouds, where asked for: a level whose clouds cannot be read is
    // lit without them.
    let read_clouds = |quality| -> Result<crate::clouds_view::SceneClouds> {
        let clouds = trp_data::clouds::LevelClouds::read(install, &level).context("Customs' clouds")?;
        let input = trp_sim::clouds::CloudInput {
            cloudiness: moment.cloudiness,
            wind: moment.wind,
            cloud_wind_multiplier: level.wind_controller.as_ref().context("Customs' wind controller")?.f32("CloudWindMultiplier")?,
            light_rotation: state.light_rotation,
            light_color: state.light_color,
            light_intensity: state.light_intensity,
            harmonics: state.harmonics,
            sun_direction: state.sun_direction,
        };
        let figures = trp_sim::clouds::CloudFigures::of(&clouds, quality, &input);
        Ok(crate::clouds_view::SceneClouds { clouds, figures, sun_position: level.sun_position })
    };
    let clouds = clouds_quality.and_then(|quality| match read_clouds(quality) {
        Ok(clouds) => Some(clouds),
        Err(e) => {
            eprintln!("the clouds are not drawn: {e:#}");
            None
        }
    });
    // The ambient highlight's setting for the hands' stencil type (Customs'
    // only one). `PROVISIONAL`: its own lights' ways not read (Customs has
    // none).
    let highlight = level.ambient_highlight.as_ref().and_then(|h| {
        let setting = h.settings.iter().find(|s| s.stencil == 2)?;
        if !h.additional_lights.is_empty() {
            eprintln!("the ambient highlight's own {} lights are not taken", h.additional_lights.len());
        }
        Some((state.harmonics_without_top.packed(), trp_sim::ambient::highlight_multiplier(setting, forward[1]), h.blur))
    });
    let moon = match trp_data::clouds::LevelMoon::read(install, &level) {
        Ok(moon) => Some(moon),
        Err(e) => {
            eprintln!("the moon is not drawn: {e:#}");
            None
        }
    };
    Ok(LevelLighting {
        sun_forward: crate::convert::position(forward),
        sun,
        direction_light_shadow: s.direction_light_shadow,
        flat_ambient: flat,
        probe_constant: flat.map(engine_linear),
        eft_ambient: state.harmonics.top(),
        harmonics: state.harmonics,
        sky: TimeOfDaySky { globals: sky.globals(&state), mesh: level.atmosphere_mesh, moon },
        clouds,
        highlight,
        indoor,
        prism,
        color_filter,
        sharpen,
        scattering,
        outdoor: [level.outdoor.exposure_offset, level.outdoor.exposure_speed, level.outdoor.fade_time],
        min_smoke_ambient: [s.min_smoke_ambient[0], s.min_smoke_ambient[1], s.min_smoke_ambient[2]],
        top_horizon: state.top_horizon_sky_color,
    })
}

/// The engine's own way to linear light (`GammaToLinearSpace`): below
/// 0.04045 over 12.92, at or above one its 2.2 power, between the sRGB
/// curve.
fn engine_linear(g: f32) -> f32 {
    if g <= 0.04045 {
        g / 12.92
    } else if g >= 1.0 {
        g.powf(2.2)
    } else {
        ((g + 0.055) / 1.055).powf(2.4)
    }
}


impl LevelLighting {
    /// The sun as Bevy's light: its colour per channel the linear of the
    /// game's colour times intensity, at an illuminance whose diffuse light
    /// on a white surface square to it is that, at the exposure above
    /// (Bevy's diffuse light has a 1/π the game's has not). With the game's
    /// shading (`shading`), the game's colour times intensity as it is, at
    /// an illuminance of one.
    pub fn sun_light(&self) -> DirectionalLight {
        let (color, illuminance) = if crate::shading::on() {
            (Color::linear_rgb(self.sun[0], self.sun[1], self.sun[2]), 1.0)
        } else {
            (Color::linear_rgb(linear(self.sun[0]), linear(self.sun[1]), linear(self.sun[2])), std::f32::consts::PI * EXPOSURE_SCALE)
        };
        DirectionalLight { color, illuminance, shadows_enabled: true, ..default() }
    }

    /// The screen's ambient pass's brightness term at the moment
    /// (`trp_sim::shading::ambient_brightness`): the sky's fogginess, no
    /// night vision, the sun's height.
    pub fn ambient_brightness(&self) -> f32 {
        trp_sim::shading::ambient_brightness(self.sky.globals.fogginess, 0.0, self.sun_forward.y)
    }

    /// The ambient as an environment map: every direction of it holds what
    /// the game's surface ambient pass gives a surface facing that way
    /// (`Hidden/WriteScreenAmbient`: the harmonics at the normal, held at
    /// nought and taken to gamma by Unity's `LinearToGammaSpace`), made
    /// linear. Bevy reads an environment map's cube by the world's way with
    /// z turned round, which is the game's own way: a face's texel at a way
    /// holds the harmonics at that way. The specular map holds the same, at
    /// each of its levels (`PROVISIONAL`: the game's pass reflects its
    /// ambient script's own cube, which it renders out to 0.2 m only).
    pub fn environment(&self, images: &mut Assets<Image>) -> EnvironmentMapLight {
        let diffuse = images.add(harmonics_cube(&self.harmonics, 32, 1));
        let specular = images.add(harmonics_cube(&self.harmonics, 32, 6));
        EnvironmentMapLight { diffuse_map: diffuse, specular_map: specular, intensity: EXPOSURE_SCALE, ..default() }
    }
}

/// The way a cube face's texel looks (the graphics card's convention for
/// faces +x, −x, +y, −y, +z, −z, `s` across and `t` down, each −1 to 1).
fn face_way(face: usize, s: f32, t: f32) -> [f32; 3] {
    let v = match face {
        0 => [1.0, -t, -s],
        1 => [-1.0, -t, s],
        2 => [s, 1.0, t],
        3 => [s, -1.0, -t],
        4 => [s, -t, 1.0],
        _ => [-s, -t, -1.0],
    };
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|c| c / length)
}

/// A cube of linear colours, `size` square, `levels` levels, each texel the
/// harmonics at its way made linear (four half floats a texel, which every
/// card samples blended).
fn harmonics_cube(harmonics: &Harmonics, size: u32, levels: u32) -> Image {
    cube(size, levels, |way| harmonics.at(way).map(|c| linear(surface_ambient(c))))
}

/// A cube of linear colours, each texel what `colour` gives its way (in the
/// world's axes with z turned round, the game's).
fn cube(size: u32, levels: u32, colour: impl Fn([f32; 3]) -> [f32; 3]) -> Image {
    let mut data = Vec::new();
    for face in 0..6 {
        for level in 0..levels {
            let side = (size >> level).max(1);
            for row in 0..side {
                for column in 0..side {
                    let s = (column as f32 + 0.5) / side as f32 * 2.0 - 1.0;
                    let t = (row as f32 + 0.5) / side as f32 * 2.0 - 1.0;
                    let c = colour(face_way(face, s, t));
                    for value in [c[0], c[1], c[2], 1.0] {
                        data.extend_from_slice(&half(value).to_le_bytes());
                    }
                }
            }
        }
    }
    let mut image = Image::new_uninit(Extent3d { width: size, height: size, depth_or_array_layers: 6 }, TextureDimension::D2, TextureFormat::Rgba16Float, RenderAssetUsages::RENDER_WORLD);
    image.texture_descriptor.mip_level_count = levels;
    image.data = Some(data);
    image.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_back_is_the_encodings_inverse() {
        for g in [0.0f32, 0.02, 0.3, 0.5, 1.0, 1.2] {
            let back = linear(g).powf(1.0 / 2.2);
            assert!((back - g).abs() < 1e-5, "{g} {back}");
        }
        // A product of two figures goes back to the product of the two.
        assert!(((linear(0.32) * linear(0.1)).powf(1.0 / 2.2) - 0.032).abs() < 1e-5);
    }

    #[test]
    fn the_surface_ambient_is_the_harmonics_taken_to_gamma() {
        // Factory by day's from above and sideways (lighting spec).
        assert!((surface_ambient(0.1009) - 0.3506).abs() < 1e-3);
        assert!((surface_ambient(0.0117) - 0.110).abs() < 1e-3);
        assert_eq!(surface_ambient(-0.01), 0.0);
    }

    #[test]
    fn halves_are_the_floats_to_nearest() {
        assert_eq!(half(1.0), 0x3c00);
        assert_eq!(half(0.5), 0x3800);
        assert_eq!(half(-2.0), 0xc000);
        assert_eq!(half(0.0), 0);
        // 0.01, as a half 0x211f (0.0099945).
        assert_eq!(half(0.01), 0x211f);
    }

    #[test]
    fn each_face_looks_its_own_way() {
        assert_eq!(face_way(0, 0.0, 0.0), [1.0, 0.0, 0.0]);
        assert_eq!(face_way(2, 0.0, 0.0), [0.0, 1.0, 0.0]);
        assert_eq!(face_way(5, 0.0, 0.0), [0.0, 0.0, -1.0]);
        // The first row of a side face is its top.
        assert!(face_way(4, 0.0, -1.0)[1] > 0.0);
    }
}

/// The share of the sky's ambient and its reflection the range hall's
/// stencil volume takes away inside it (`StencilShadow.Ambient`'s alpha,
/// which the screen's ambient passes keep one less of): Customs' concrete
/// warehouse's (`sklad_concrete`), whole (`trp-sim/tests/time_of_day.rs`
/// reads it from the install).
pub const HALL_STENCIL: f32 = 1.0;

/// The range hall's stencil volume, as a Bevy light probe over its inside
/// (`range::indoors`): within it the environment map's light at one
/// less the stencil's share, in place of the view's (Bevy's reflection
/// probes take a fragment inside them, its diffuse and its reflection,
/// over the view's). With its index put in the view's clusters by
/// `HallProbePlugin`.
pub fn hall_probe(view: &EnvironmentMapLight) -> Option<(bevy::light::LightProbe, EnvironmentMapLight, Transform, HallProbe)> {
    // The indoors (`range::indoors`: the hall's, a little into its walls,
    // or a level file's), turned as it is.
    let (centre, size, yaw) = crate::range::indoors()?;
    let map = EnvironmentMapLight { intensity: view.intensity * (1.0 - HALL_STENCIL), ..view.clone() };
    let place = Transform::from_translation(centre).with_rotation(Quat::from_rotation_y(yaw)).with_scale(size);
    Some((bevy::light::LightProbe::new(), map, place, HallProbe))
}

/// The hall's surfaces' colours for its bounced light (`hall_bounce`): its
/// floor the range's ground (0.35), its walls and roof its concrete
/// (`range::HALL_GREY`, 0.55).
const HALL_FLOOR_ALBEDO: f32 = 0.35;
const HALL_WALL_ALBEDO: f32 = 0.55;

/// The light the hall's lamps bounce off its floor, walls and roof, for its
/// probe with `HALL LAMPS` in place of the sky's ambient it takes away (the
/// user, 2026-10-19, of the character inside being "super dark": the game's
/// indoors are lit by their baked light, which the hall has not).
/// `ESTIMATE`, the app's own: the hall a box (its indoor volume), each lamp
/// lighting its faces as Bevy draws it (`hall_lamp`: its power, its cone
/// and its range's fade; a face's light the mean over a grid on it), each
/// face's light bounced by its colour made linear, the faces lighting each
/// other by the shares of each one's view the others fill (each weighted by
/// its slant), ten times over; then, for each way of the probe's map, what
/// a surface at the hall's middle facing it takes from the faces. The sun
/// through the windows and the things inside are left out. In the probe's
/// map at the exposure's intensity, as the sky's. With the game's shading
/// (`shading`), the lamps light the faces as it draws them, their sum the
/// game's number made linear.
pub fn hall_bounce(images: &mut Assets<Image>) -> Option<EnvironmentMapLight> {
    let game = crate::shading::on();
    let (centre, size, yaw) = crate::range::indoors()?;
    let half = size / 2.0;
    let turn = Quat::from_rotation_y(-yaw);
    // Each lamp in the box's own axes, with its height over the floor.
    let lamps: Vec<(Vec3, f32)> = crate::range::hall_lamps().into_iter().map(|at| (turn * (at - centre), at.y)).collect();
    let faces = BoxFaces { half };
    let albedo: [f32; 6] = std::array::from_fn(|f| linear(if f == 0 { HALL_FLOOR_ALBEDO } else { HALL_WALL_ALBEDO }));
    // The lamps' light on each face, its mean over a grid on it.
    let direct: [f32; 6] = std::array::from_fn(|f| {
        let n = 24;
        let mut sum = 0.0;
        for i in 0..n {
            for j in 0..n {
                let p = faces.point(f, (i as f32 + 0.5) / n as f32, (j as f32 + 0.5) / n as f32);
                let lit = lamps.iter().map(|&(at, height)| lamp_light(at, height, p, faces.normal(f), game)).sum::<f32>();
                sum += if game { linear(lit) } else { lit };
            }
        }
        sum / (n * n) as f32
    });
    // The faces' shares of each face's view, each way weighted by its
    // slant: from a grid on the face, ways spread evenly by their slant.
    let shares: [[f32; 6]; 6] = std::array::from_fn(|f| {
        let mut share = [0.0f32; 6];
        let (n, ways) = (8, 16);
        for i in 0..n {
            for j in 0..n {
                let p = faces.point(f, (i as f32 + 0.5) / n as f32, (j as f32 + 0.5) / n as f32);
                for way in slanted_ways(faces.normal(f), ways) {
                    if let Some(hit) = faces.hit(p + faces.normal(f) * 1e-3, way) {
                        share[hit] += 1.0;
                    }
                }
            }
        }
        let total: f32 = share.iter().sum::<f32>().max(1.0);
        share.map(|s| s / total)
    });
    let mut bright = [0.0f32; 6];
    for _ in 0..10 {
        bright = std::array::from_fn(|f| albedo[f] * (direct[f] + (0..6).map(|g| shares[f][g] * bright[g]).sum::<f32>()));
    }
    let at_middle = |way: [f32; 3]| -> [f32; 3] {
        // The game's way (z turned round) into the box's axes.
        let way = turn * Vec3::new(way[0], way[1], -way[2]);
        let rays = slanted_ways(way, 12);
        let taken = rays.iter().filter_map(|&r| faces.hit(Vec3::ZERO, r)).map(|f| bright[f]).sum::<f32>() / rays.len() as f32;
        [taken; 3]
    };
    let diffuse = images.add(cube(16, 1, at_middle));
    let specular = images.add(cube(16, 5, at_middle));
    let lit = (0..6).map(|f| format!("{:.3}", bright[f])).collect::<Vec<_>>().join(", ");
    info!("the hall's bounced light (floor, roof, walls): {lit}");
    Some(EnvironmentMapLight { diffuse_map: diffuse, specular_map: specular, intensity: EXPOSURE_SCALE, ..default() })
}

/// A lamp's light, as Bevy draws the hall's (`hall_lamp`), on a place of a
/// surface facing `normal`: in the frame's linear light, the lamp at `at`
/// hung `height` over the floor, the light at the floor under it the
/// game's there (made linear), elsewhere by Bevy's fall-off with the square
/// of the distance, its range's fade and its cone's edge. With the game's
/// shading (`game`), in the game's numbers: its intensity by the engine's
/// fall-off, its slant and Bevy's cone.
fn lamp_light(at: Vec3, height: f32, p: Vec3, normal: Vec3, game: bool) -> f32 {
    let to = p - at;
    let d2 = to.length_squared().max(1e-4);
    let way = to / d2.sqrt();
    let facing = (-way).dot(normal).max(0.0);
    let (inner, outer) = (0.5f32.to_radians().cos(), (LAMP_CONE / 2.0).to_radians().cos());
    let scale = 1.0 / (inner - outer).max(1e-4);
    let cone = ((way.dot(Vec3::NEG_Y) * scale - outer * scale).clamp(0.0, 1.0)).powi(2);
    if game {
        return LAMP_INTENSITY * trp_sim::shading::attenuation(d2 / (LAMP_RANGE * LAMP_RANGE)) * facing * cone;
    }
    let fade = (1.0 - (d2 / (LAMP_RANGE * LAMP_RANGE)).powi(2)).clamp(0.0, 1.0).powi(2);
    let at_floor = linear(LAMP_INTENSITY * unity_attenuation(FACTORY_LAMP_HEIGHT / LAMP_RANGE));
    at_floor * height * height / d2 * facing * cone * fade
}

/// So many ways about a normal (their count squared), spread evenly by
/// their slant to it (a disc's points raised onto the half sphere).
fn slanted_ways(normal: Vec3, count: usize) -> Vec<Vec3> {
    let n = normal.normalize_or(Vec3::Y);
    let t = if n.y.abs() < 0.9 { n.cross(Vec3::Y).normalize() } else { n.cross(Vec3::X).normalize() };
    let b = n.cross(t);
    let mut ways = Vec::with_capacity(count * count);
    for i in 0..count {
        for j in 0..count {
            let r = ((i as f32 + 0.5) / count as f32).sqrt();
            let a = (j as f32 + 0.5) / count as f32 * std::f32::consts::TAU;
            let (x, y) = (r * a.cos(), r * a.sin());
            let z = (1.0 - r * r).max(0.0).sqrt();
            ways.push((t * x + b * y + n * z).normalize());
        }
    }
    ways
}

/// A box about its middle: its faces by number (0 the floor, 1 the roof,
/// 2 and 3 the walls across, 4 and 5 those along), their places, inward
/// normals and which a ray from inside meets.
struct BoxFaces {
    half: Vec3,
}

impl BoxFaces {
    fn normal(&self, f: usize) -> Vec3 {
        [Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z][f]
    }

    fn point(&self, f: usize, u: f32, v: f32) -> Vec3 {
        let h = self.half;
        let (a, b) = (u * 2.0 - 1.0, v * 2.0 - 1.0);
        match f {
            0 => Vec3::new(a * h.x, -h.y, b * h.z),
            1 => Vec3::new(a * h.x, h.y, b * h.z),
            2 => Vec3::new(-h.x, a * h.y, b * h.z),
            3 => Vec3::new(h.x, a * h.y, b * h.z),
            4 => Vec3::new(a * h.x, b * h.y, -h.z),
            _ => Vec3::new(a * h.x, b * h.y, h.z),
        }
    }

    fn hit(&self, from: Vec3, way: Vec3) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for axis in 0..3 {
            let w = way[axis];
            if w.abs() < 1e-9 {
                continue;
            }
            let (neg, pos) = [(2, 3), (0, 1), (4, 5)][axis];
            let (face, plane) = if w < 0.0 { (neg, -self.half[axis]) } else { (pos, self.half[axis]) };
            let t = (plane - from[axis]) / w;
            if t > 0.0 && best.is_none_or(|(b, _)| t < b) {
                best = Some((t, face));
            }
        }
        best.map(|(_, face)| face)
    }
}

/// The hall's probe, marked to be carried into the render world.
#[derive(Component, Clone, Copy)]
pub struct HallProbe;

impl bevy::render::extract_component::ExtractComponent for HallProbe {
    type QueryData = ();
    type QueryFilter = With<HallProbe>;
    type Out = HallProbe;

    fn extract_component(_: bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(HallProbe)
    }
}

/// Puts the hall's probe in the view's clusters. Bevy 0.18 counts a
/// reflection probe in each cluster it reaches but never writes its index
/// there (the probe is not carried into the render world, nor given an
/// index among the clusters' objects, as its point and spot lights and
/// decals are), so a fragment read the index after the cluster's own: the
/// next cluster's first light's, or none, and took another probe or an
/// empty one, which holds every place and takes all the ambient away
/// (`findings.md`). Here the probe is carried over and given its place in
/// the view's list of probes, the first: the hall's is the range's only
/// one. And Bevy puts a probe in the clusters its bounding sphere reaches
/// (25 m about the hall's middle) but in a view's list only while its box
/// is in the view: looking away from the hall from within the sphere, the
/// clusters named a probe the list had not, and an empty one took the
/// sky's ambient away outdoors. So the probe is taken out of the clusters
/// of a view that leaves it out (`clear_hall_probe`).
pub struct HallProbePlugin;

impl Plugin for HallProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::render::extract_component::ExtractComponentPlugin::<HallProbe>::default());
        app.add_systems(PostUpdate, clear_hall_probe.after(bevy::light::SimulationLightSystems::AssignLightsToClusters));
        let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) else { return };
        render_app.add_systems(
            bevy::render::Render,
            index_hall_probe.in_set(bevy::render::RenderSystems::ManageViews).after(bevy::pbr::prepare_lights),
        );
    }
}

/// Takes the hall's probe out of every cluster of a view whose frustum
/// misses its box, by the test Bevy's own gathering of a view's probes
/// makes (`LightProbeInfo::frustum_cull`: the unit box by the probe's
/// transform, the near plane counted, the far not).
fn clear_hall_probe(probes: Query<(Entity, &GlobalTransform), With<HallProbe>>, mut views: Query<(&bevy::camera::primitives::Frustum, &mut bevy::light::cluster::Clusters)>) {
    let unit = bevy::camera::primitives::Aabb { center: bevy::math::Vec3A::ZERO, half_extents: bevy::math::Vec3A::splat(0.5) };
    for (probe, transform) in &probes {
        for (frustum, mut clusters) in &mut views {
            if frustum.intersects_obb(&unit, &transform.affine(), true, false) {
                continue;
            }
            for cluster in clusters.clusterable_objects.iter_mut() {
                if let Some(at) = cluster.entities.iter().position(|entity| *entity == probe) {
                    cluster.entities.remove(at);
                    cluster.counts.reflection_probes -= 1;
                }
            }
        }
    }
}

fn index_hall_probe(probes: Query<Entity, With<HallProbe>>, mut objects: ResMut<bevy::pbr::GlobalClusterableObjectMeta>) {
    for probe in &probes {
        objects.entity_to_index.insert(probe, 0);
    }
}

/// Factory's industrial ceiling lamp (`Lamp_Industrial_01_High_Big`'s main
/// spot, as recorded on Factory by day: its untuned instances' figures):
/// its intensity, range in metres and cone in degrees. Its colour (gamma)
/// was the lamp's warm (1, 0.863, 0.651) until 0.82.1; white since (the
/// user, 2026-10-19: "Can we make the inside lights shine white, instead
/// of orange? just to make the inside feel brighter"), the hall being the
/// app's own.
const LAMP_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
const LAMP_INTENSITY: f32 = 8.0;
const LAMP_RANGE: f32 = 18.2;
const LAMP_CONE: f32 = 90.0;
/// How high Factory's industrial lamps hang over its floor (recorded:
/// 10.577 m), which the floor's light under one is reckoned at.
const FACTORY_LAMP_HEIGHT: f32 = 10.577;

/// The engine's own fall-off of a point or spot light's light with the
/// share of its range (its attenuation texture: `1 / (1 + 25 x²)`, faded
/// to nought over the range's last fifth; `trp_sim::shading::attenuation`).
fn unity_attenuation(share: f32) -> f32 {
    trp_sim::shading::attenuation(share * share)
}

/// The hall's lamp as Bevy's spot light, hung this high over the floor:
/// its colour the linear of the lamp's, and its power such that the floor
/// straight under it takes what Factory's floor takes under the same lamp
/// hung where Factory hangs it (the lamp's intensity times the engine's
/// fall-off there, made linear, at the exposure above: Bevy's spot light
/// gives `I / 4π` over the square of the distance, and its diffuse light a
/// `1 / π`). Reckoned at the hall's own height, the floor took 1.7 times
/// Factory's and showed nearly white. `PROVISIONAL`: elsewhere Bevy's
/// fall-off (with the square of the distance) and its cone's edge are its
/// own, and no shadows are cast, as the game's lamp casts none.
///
/// With the game's shading (`shading`), the lamp as the game gives it: its
/// colour, its intensity (times 4π, which Bevy divides by) and its range,
/// the engine's fall-off at every distance; hung at the hall's height, the
/// floor under it takes 1.7 times what Factory's takes.
pub fn hall_lamp(height: f32) -> SpotLight {
    let at_floor = LAMP_INTENSITY * unity_attenuation(FACTORY_LAMP_HEIGHT / LAMP_RANGE);
    let pi = std::f32::consts::PI;
    let (color, intensity) = if crate::shading::on() {
        (Color::linear_rgb(LAMP_COLOR[0], LAMP_COLOR[1], LAMP_COLOR[2]), 4.0 * pi * LAMP_INTENSITY)
    } else {
        (Color::linear_rgb(linear(LAMP_COLOR[0]), linear(LAMP_COLOR[1]), linear(LAMP_COLOR[2])), 4.0 * pi * pi * EXPOSURE_SCALE * height * height * linear(at_floor))
    };
    SpotLight {
        color,
        intensity,
        range: LAMP_RANGE,
        radius: 0.0,
        shadows_enabled: false,
        inner_angle: 0.5f32.to_radians(),
        outer_angle: (LAMP_CONE / 2.0).to_radians(),
        ..default()
    }
}

/// The range's sun, Customs' or the range's own: the light the effects'
/// shaders take their way to the sun from (other directional lights light
/// the icon studio and the figures' views).
#[derive(Component)]
pub struct RangeSun;

/// The range's own light (`CUSTOMS LIGHT` off) as it stands at the range's
/// own hour: a sun of 12,000 lux from up and to the side, an even ambient
/// and a plain sky's colour (the app's, not the game's).
pub const RANGE_OWN_SUN: f32 = 12_000.0;
pub const RANGE_OWN_AMBIENT: f32 = 400.0;
pub const RANGE_OWN_SKY: [f32; 3] = [0.55, 0.63, 0.70];

/// The range's own sun's turn at its own hour.
pub fn range_own_sun_turn() -> Transform {
    Transform::from_xyz(3.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y)
}
