//! Customs' clouds by the game's rules (`docs/spec/lighting.md`, "Customs'
//! clouds"): the graphics setting `CLOUDS QUALITY` and the preset each of
//! its choices applies, the remap of the weather's cloudiness to the cloud
//! map's channels, the wind's speed for them, and the figures the three
//! passes take (`BakeCloudTexture`, once; `BakeCloudShadows`, each frame
//! into the sun's cookie; `Hidden/HDRP/Sky/CloudLayer`, each frame over the
//! sky). The app runs the passes on the graphics card (`clouds_view`); what
//! they are given is worked out here, to be read and tested.

use trp_data::clouds::LevelClouds;

use crate::ambient::Harmonics;

/// The setting's choices (`CloudsMode`), in the game's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudsQuality {
    Low,
    Medium,
    High,
}

impl CloudsQuality {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// Its name as `Graphics.ini` writes it (`CloudsQuality`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.key().eq_ignore_ascii_case(key))
    }

    /// The preset it applies (`CloudQualityPreset`): the cloud texture's
    /// width (its height half that), the shadows' square, the light's
    /// steps through the layer, whether the clouds cast shadows.
    pub fn preset(self) -> Preset {
        match self {
            Self::Low => Preset { resolution: 2048, shadow_resolution: 128, steps: 5, cast_shadows: true },
            Self::Medium => Preset { resolution: 2048, shadow_resolution: 256, steps: 7, cast_shadows: true },
            Self::High => Preset { resolution: 2048, shadow_resolution: 512, steps: 12, cast_shadows: true },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub resolution: u32,
    pub shadow_resolution: u32,
    pub steps: u32,
    pub cast_shadows: bool,
}

/// What the clouds are worked out from on a frame: the weather's
/// cloudiness (−1 to 1) and wind, the sun's light, the sky's harmonics and
/// the way to the sun.
#[derive(Debug, Clone, Copy)]
pub struct CloudInput {
    pub cloudiness: f32,
    /// `WeatherCurve.Wind` (x, z).
    pub wind: [f32; 2],
    /// `WindController.CloudWindMultiplier`, the level's.
    pub cloud_wind_multiplier: f32,
    /// The light's turn (quaternion x, y, z, w) in the engine's axes, its
    /// colour (gamma) and intensity.
    pub light_rotation: [f32; 4],
    pub light_color: [f32; 3],
    pub light_intensity: f32,
    /// The time-of-day controller's harmonics (`ToDController.SH`) and
    /// `TOD_Sky.SunDirection`.
    pub harmonics: Harmonics,
    pub sun_direction: [f32; 3],
}

/// The figures of the three passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudFigures {
    /// The cloud texture's bake (`BakeCloudTexture`): the way to the sun
    /// (the light's backward way), the planet's radius, the map's channels'
    /// opacities, the layer's turn as a share of a whole turn, the light's
    /// steps, the extinction's figure, the layer's altitude, the texture's
    /// width (its height half).
    pub sun_direction: [f32; 3],
    pub planet_radius: f32,
    pub opacities: [f32; 4],
    pub rotation: f32,
    pub steps: u32,
    pub sigma_t: f32,
    pub altitude: f32,
    pub resolution: u32,
    /// The draw (`CloudLayer`'s second pass): the wind's way across the
    /// layer (x, z), the wind's speed in kilometres an hour (the scroll
    /// grows by `scroll_step` of it), the layer's opacity, the sun's colour
    /// for the clouds, the ambient for them (the harmonics at straight down
    /// taken to gamma, at least nought, times the dimmer).
    pub scroll_direction: [f32; 2],
    pub scroll_speed: f32,
    pub opacity: f32,
    pub sun_color: [f32; 3],
    pub ambient: [f32; 3],
    /// The sun's cookie (`BakeCloudShadows`), where the clouds cast
    /// shadows: its square, the shadow tint's luminance, the opacity's
    /// multiplier (an eighth of the settings'), the cookie's width on the
    /// ground; the light's right and up (the cookie's axes).
    pub shadows: Option<ShadowFigures>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowFigures {
    pub resolution: u32,
    pub tint_luminance: f32,
    pub opacity: f32,
    pub size: f32,
    pub sun_right: [f32; 3],
    pub sun_up: [f32; 3],
}

/// `RenderingUtils.LensImperfectionExposureScale`.
const LENS_IMPERFECTION_EXPOSURE_SCALE: f32 = 1.2;

/// `RenderingUtils.ConvertEv100ToExposure`.
pub fn ev100_to_exposure(ev100: f32) -> f32 {
    1.0 / (LENS_IMPERFECTION_EXPOSURE_SCALE * 2f32.powf(ev100))
}

/// `Mathf.GammaToLinearSpace` (a colour's `.linear`).
pub fn gamma_to_linear(g: f32) -> f32 {
    if g <= 0.04045 {
        g / 12.92
    } else if g < 1.0 {
        ((g + 0.055) / 1.055).powf(2.4)
    } else {
        g.powf(2.2)
    }
}

/// HDRP's `LinearToSRGB`, as the draw takes its ambient to gamma (the
/// project's colour space is gamma).
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        c.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

/// The scroll's growth over a frame of `dt` seconds at a speed in
/// kilometres an hour (`ScrollFactor += ScrollSpeed × dt × 0.277778`).
pub fn scroll_step(speed: f32, dt: f32) -> f32 {
    speed * dt * 0.277_778
}

impl CloudFigures {
    /// The figures of a level's clouds at a quality and a frame's input.
    pub fn of(clouds: &LevelClouds, quality: CloudsQuality, input: &CloudInput) -> Self {
        let s = &clouds.settings;
        let preset = quality.preset();
        // `WeatherController.ControlClouds`: the remap by the cloudiness
        // from 0 to 1; the speed by the wind's magnitude times the
        // multiplier, halved, between the layer's two speeds (its way is
        // the layer's own: the wind's is not taken).
        let opacities = clouds.remap.at((input.cloudiness + 1.0) / 2.0);
        let magnitude = (input.wind[0] * input.wind[0] + input.wind[1] * input.wind[1]).sqrt();
        let t = (magnitude * input.cloud_wind_multiplier / 2.0).clamp(0.0, 1.0);
        let scroll_speed = s.min_scroll_speed + (s.max_scroll_speed - s.min_scroll_speed) * t;
        let orientation = s.scroll_orientation.to_radians();
        // The light's own axes (its transform's right, up and forward).
        let rotate = |v: [f32; 3]| trp_data::level_light::rotate(input.light_rotation, v);
        let forward = rotate([0.0, 0.0, 1.0]);
        let sun_direction = forward.map(|c| -c);
        // `CloudLayer.Color` (the tint at the exposure taken off) times
        // `EvaluateLightColor` (the light's colour made linear, times its
        // intensity).
        let exposure = ev100_to_exposure(-s.exposure);
        let sun_color = std::array::from_fn(|i| s.tint[i] * exposure * gamma_to_linear(input.light_color[i]) * input.light_intensity);
        // `CloudController.UpdateAmbient`: the controller's harmonics with
        // the gradient's colour added evenly, then the draw's own sum at
        // straight down, to gamma, at least nought, times the dimmer.
        let mut harmonics = input.harmonics;
        let colour = clouds.ambient_color.evaluate(input.sun_direction[1] * 0.5 + 0.5);
        harmonics.add_ambient([colour[0], colour[1], colour[2]]);
        let down = harmonics.at([0.0, -1.0, 0.0]);
        let ambient = down.map(|c| linear_to_srgb(c).max(0.0) * s.ambient_probe_dimmer);
        let shadows = preset.cast_shadows.then(|| ShadowFigures {
            resolution: preset.shadow_resolution,
            tint_luminance: s.shadow_tint[0] * 0.299 + s.shadow_tint[1] * 0.587 + s.shadow_tint[2] * 0.114,
            opacity: s.shadow_multiplier / 8.0,
            size: s.shadow_size,
            sun_right: rotate([1.0, 0.0, 0.0]),
            sun_up: rotate([0.0, 1.0, 0.0]),
        });
        Self {
            sun_direction,
            planet_radius: s.planet_center_radius[3],
            opacities,
            rotation: s.rotation / 360.0,
            steps: if s.lighting { preset.steps } else { 0 },
            sigma_t: s.thickness * 0.095 + 0.005,
            altitude: s.altitude,
            resolution: preset.resolution,
            scroll_direction: [-orientation.cos(), -orientation.sin()],
            scroll_speed,
            opacity: s.opacity,
            sun_color,
            ambient,
            shadows,
        }
    }
}

/// The sky's moon as `TOD_Sky` places it and `Time of Day/Moon` draws it
/// (`docs/spec/lighting.md`, "The moon"): its sphere's radius where the
/// moon is at one (`2 tan(MeshSize°)`, the sphere of radius one), its
/// object's axes in the dome's (it looks at the dome's middle, its up the
/// stars' turn's left: `LookAt(dome, quaternion × −right)`), whether it is
/// drawn (its place above minus its radius), its picture's contrast's
/// power and brightness (less the fogginess's share).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MoonFigures {
    pub radius: f32,
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    pub drawn: bool,
    pub contrast: f32,
    pub brightness: f32,
}

impl MoonFigures {
    /// From the moon's way in the dome's axes, the stars' turn
    /// (`Quaternion.Euler(90 − latitude, 0, 0) × Euler(0, longitude, 0) ×
    /// Euler(0, sidereal, 0)`), the moon's mesh figures and the fogginess.
    pub fn of(local_moon_direction: [f32; 3], latitude: f32, longitude: f32, sidereal: f32, size: f32, brightness: f32, contrast: f32, fogginess: f32) -> Self {
        let radius = 2.0 * size.to_radians().tan();
        let euler_x = |deg: f32| {
            let h = deg.to_radians() / 2.0;
            [h.sin(), 0.0, 0.0, h.cos()]
        };
        let euler_y = |deg: f32| {
            let h = deg.to_radians() / 2.0;
            [0.0, h.sin(), 0.0, h.cos()]
        };
        let turn = mul(mul(euler_x(90.0 - latitude), euler_y(longitude)), euler_y(sidereal.to_degrees()));
        let world_up = trp_data::level_light::rotate(turn, [-1.0, 0.0, 0.0]);
        // `LookRotation(forward, up)`: z the way, x up × z, y z × x.
        let z = normalized(local_moon_direction.map(|c| -c));
        let x = normalized(cross(world_up, z));
        let y = cross(z, x);
        Self { radius, right: x, up: y, forward: z, drawn: local_moon_direction[1] > -radius, contrast: 1.0 / contrast.max(0.001), brightness: brightness * (1.0 - fogginess) }
    }
}

fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length > 0.0 {
        v.map(|c| c / length)
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exposure_takes_the_lens_scale() {
        // 2.1 EV taken off: 2^2.1 / 1.2.
        assert!((ev100_to_exposure(-2.1) - 3.572_58).abs() < 1e-4, "{}", ev100_to_exposure(-2.1));
    }

    #[test]
    fn the_curves_meet() {
        for c in [0.0f32, 0.001, 0.003_130_8, 0.2, 0.5, 1.0] {
            let back = gamma_to_linear(linear_to_srgb(c));
            assert!((back - c).abs() < 2e-4, "{c} came back as {back}");
        }
    }

    #[test]
    fn the_moon_looks_at_the_middle() {
        let m = MoonFigures::of([0.3, 0.2, -0.932_737_9], 46.0, 84.0, 1.2, 1.0, 1.0, 0.5, 0.0);
        let d = dot(m.forward, [0.3, 0.2, -0.932_737_9]);
        assert!((d + 1.0).abs() < 1e-5, "{d}");
        assert!(dot(m.right, m.up).abs() < 1e-6 && dot(m.right, m.forward).abs() < 1e-6);
        assert!((m.radius - 2.0 * 1f32.to_radians().tan()).abs() < 1e-7);
    }

    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
}
