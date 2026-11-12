//! The level's lighting read: exposure, sharpening, scattering, sky and harmonics.

use super::*;

/// What the frame's exposure takes a light of intensity one to (Bevy's
/// `Exposure` at EV100 0 multiplies by 1/1.2).
pub const EXPOSURE_EV100: f32 = 0.0;

pub const EXPOSURE_SCALE: f32 = 1.2;

/// The level's light at its moment, read at the start.
/// The camera's sharpening as the level has it (`sharpen_view`): its
/// figures, the strength it runs at, and the weather's desaturation.
#[derive(Debug, Clone)]
pub struct Sharpening {
    pub data: trp_data::drawing::camera_effects::SharpenData,
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
    /// straight above (the top ambient).
    pub harmonics: Harmonics,
    pub top_ambient: [f32; 3],
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
    pub indoor: Option<trp_sim::lighting::prism::IndoorFigures>,
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
    pub clouds: Option<crate::rendering::clouds_view::SceneClouds>,
    /// The ambient highlight over the hands' stencil type, where the level
    /// has one (`highlight_view`): its harmonics (the time of day's without
    /// the sky's samples and the light from above), packed; its multiplier
    /// at the moment's sun; its blur.
    pub highlight: Option<(trp_sim::lighting::ambient::Packed, f32, f32)>,
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
    pub moon: Option<trp_data::level::clouds::LevelMoon>,
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
/// pass takes them (the harmonics are linear light's; the top ambient, which
/// the effects take, is the same sum not so taken).
pub fn surface_ambient(sh: f32) -> f32 {
    (1.055 * sh.max(0.0).powf(0.416_667) - 0.055).max(0.0)
}

/// A float as a half float's bits (to nearest; no figure here is out of a
/// half's range).
pub(super) fn half(value: f32) -> u16 {
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
/// weather at 09:30.
pub const RANGE_MOMENT: Moment = Moment { hour: 9.5, ..CUSTOMS_RECORDED_MORNING };

/// The camera's sharpening (`CC_Sharpen` and its `DesaturateEffect`) as
/// Customs' camera has it, at the player's strength (`Sharpen` in
/// `Graphics.ini`, the prefab's where it has none), with the weather's
/// desaturation by the fogginess given (`WeatherController.
/// ControlDesaturateEffect`: its `MinMaxAmmount` by it); nought with none.
fn sharpening_of(install: &trp_data::Install, level: &TimeOfDayLevel, camera_file: &str, fogginess: Option<f32>) -> Result<Sharpening> {
    let data = trp_data::drawing::camera_effects::SharpenData::of_camera(install, camera_file, level.settings.camera.1)?;
    let strength = install.display_settings().sharpen.unwrap_or(data.strength);
    let weather = fogginess.map_or(0.0, |fogginess| {
        let [low, high] = level.weather.vec2("MinMaxAmmount").unwrap_or([0.0, 0.0]);
        low + (high - low) * fogginess
    });
    Ok(Sharpening { data, strength, weather })
}

/// The camera's sharpening read on its own, for the range's own light
/// (`CAMERA SHARPENING` without `CUSTOMS LIGHT`, the whole pass): Customs' camera's, at the player's strength, with no
/// weather's share, there being no weather without Customs' light.
pub fn camera_sharpening(install: &trp_data::Install) -> Result<Sharpening> {
    let level = TimeOfDayLevel::customs(install).context("Customs' time of day")?;
    let camera_file = level.file_of(level.settings.camera)?;
    sharpening_of(install, &level, &camera_file, None)
}

/// Reads Customs' light from the install and works a moment of its time of
/// day out by the game's rules: the moment settled by twelve frames of the
/// game's controller and sky in turn, as the tests compare the recorded
/// morning (`trp-sim/tests/time_of_day.rs`); its camera's PRISM and its
/// environment manager's figures for the open air; its clouds at a quality
/// where asked for (`trp-sim/tests/clouds.rs`).
pub fn read(install: &trp_data::Install, moment: Moment, clouds_quality: Option<trp_sim::lighting::clouds::CloudsQuality>) -> Result<LevelLighting> {
    let level = TimeOfDayLevel::customs(install).context("Customs' time of day")?;
    let sky = trp_sim::lighting::tod::Sky::of_level(&level).context("Customs' sky's figures")?;
    let stored = trp_sim::lighting::tod::stored_atmosphere(&level.sky)?;
    let state = sky.settle(moment.hour, moment.cloudiness, stored, 12);
    let camera_file = level.file_of(level.settings.camera)?;
    // The camera's sharpening, the weather's desaturation by the fogginess
    // the sky's settled state has.
    let sharpen = match sharpening_of(install, &level, &camera_file, Some(state.atmosphere.fogginess)) {
        Ok(sharpening) => Some(sharpening),
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
    let forward = trp_data::level::level_light::rotate(state.light_rotation, [0.0, 0.0, 1.0]);
    let indoor = level.common_indoor().map(|(t, _)| trp_sim::lighting::prism::IndoorFigures { fade_time: t.fade_time, exposure_speed: t.exposure_speed, exposure_offset: t.exposure_offset });
    let sun = state.light_color.map(|c| c * state.light_intensity);
    // The clouds, where asked for: a level whose clouds cannot be read is
    // lit without them.
    let read_clouds = |quality| -> Result<crate::rendering::clouds_view::SceneClouds> {
        let clouds = trp_data::level::clouds::LevelClouds::read(install, &level).context("Customs' clouds")?;
        let input = trp_sim::lighting::clouds::CloudInput {
            cloudiness: moment.cloudiness,
            wind: moment.wind,
            cloud_wind_multiplier: level.wind_controller.as_ref().context("Customs' wind controller")?.f32("CloudWindMultiplier")?,
            light_rotation: state.light_rotation,
            light_color: state.light_color,
            light_intensity: state.light_intensity,
            harmonics: state.harmonics,
            sun_direction: state.sun_direction,
        };
        let figures = trp_sim::lighting::clouds::CloudFigures::of(&clouds, quality, &input);
        Ok(crate::rendering::clouds_view::SceneClouds { clouds, figures, sun_position: level.sun_position })
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
        Some((state.harmonics_without_top.packed(), trp_sim::lighting::ambient::highlight_multiplier(setting, forward[1]), h.blur))
    });
    let moon = match trp_data::level::clouds::LevelMoon::read(install, &level) {
        Ok(moon) => Some(moon),
        Err(e) => {
            eprintln!("the moon is not drawn: {e:#}");
            None
        }
    };
    Ok(LevelLighting {
        sun_forward: crate::platform::convert::position(forward),
        sun,
        direction_light_shadow: s.direction_light_shadow,
        flat_ambient: flat,
        probe_constant: flat.map(engine_linear),
        top_ambient: state.harmonics.top(),
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
        let (color, illuminance) = if crate::rendering::shading::on() {
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
        trp_sim::lighting::shading::ambient_brightness(self.sky.globals.fogginess, 0.0, self.sun_forward.y)
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
pub(super) fn face_way(face: usize, s: f32, t: f32) -> [f32; 3] {
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
pub(super) fn cube(size: u32, levels: u32, colour: impl Fn([f32; 3]) -> [f32; 3]) -> Image {
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
