//! The sky's shader globals and the atmosphere stored for a recorded moment.

use super::*;

/// The figures the sky's shaders are given (`TOD_Sky.UpdateShaderProperties`)
/// for the atmosphere and the sun's mesh.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SkyGlobals {
    pub sun_sky_color: Rgb,
    pub moon_sky_color: Rgb,
    pub ground_color: Rgb,
    pub moon_halo_color: Rgb,
    /// In the dome's own axes.
    pub local_sun_direction: [f32; 3],
    pub local_moon_direction: [f32; 3],
    pub contrast: f32,
    pub scattering_brightness: f32,
    pub fogginess: f32,
    /// One over the halo's size.
    pub moon_halo_power: f32,
    pub scattering: Scattering,
    pub sun_mesh_color: Rgb,
    /// One over the mesh's contrast (at least 0.001), and its brightness
    /// less the fogginess's share.
    pub sun_mesh_contrast: f32,
    pub sun_mesh_brightness: f32,
    /// Half the sun's quad's width where the sun is at one (its scale,
    /// `2 tan(2° × MeshSize)`: the quad, `Quad` of the level's shared
    /// assets, spans −1 to 1), and whether it is drawn (its place above
    /// minus that scale).
    pub sun_mesh_half_width: f32,
    pub sun_drawn: bool,
    /// The moon's sphere (`Time of Day/Moon`), and the way to the sun in the
    /// world (`TOD_SunDirection`, which lights it).
    pub moon_mesh: crate::lighting::clouds::MoonFigures,
    pub sun_direction: [f32; 3],
}

impl Sky {
    /// The shaders' figures at a state.
    pub fn globals(&self, state: &State) -> SkyGlobals {
        let a = &state.atmosphere;
        let width = 2.0 * (std::f32::consts::PI / 90.0 * self.sun_mesh.size).tan();
        SkyGlobals {
            sun_sky_color: state.sun_sky_color,
            moon_sky_color: state.moon_sky_color,
            ground_color: state.ground_color,
            moon_halo_color: state.moon_halo_color,
            local_sun_direction: state.local_sun_direction,
            local_moon_direction: state.local_moon_direction,
            contrast: a.contrast,
            scattering_brightness: a.scattering_brightness,
            fogginess: a.fogginess,
            moon_halo_power: 1.0 / self.moon_halo_size,
            scattering: state.scattering,
            sun_mesh_color: state.sun_mesh_color,
            sun_mesh_contrast: 1.0 / self.sun_mesh.contrast.max(0.001),
            sun_mesh_brightness: self.sun_mesh.brightness * (1.0 - a.fogginess),
            sun_mesh_half_width: width,
            sun_drawn: state.local_sun_direction[1] > -width,
            moon_mesh: crate::lighting::clouds::MoonFigures::of(
                state.local_moon_direction,
                self.latitude,
                self.longitude,
                state.sidereal,
                self.moon_mesh.size,
                self.moon_mesh.brightness,
                self.moon_mesh.contrast,
                a.fogginess,
            ),
            sun_direction: state.sun_direction,
        }
    }
}

/// A moment of a level's time of day: the hour (`TOD_Time`), the weather's
/// cloudiness (`WeatherCurve.Cloudiness`) and wind (`WeatherCurve.Wind`, x
/// and z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moment {
    pub hour: f32,
    pub cloudiness: f32,
    pub wind: [f32; 2],
}

/// Customs' morning as recorder 0.11.0 found it (`local/traces/courses3/
/// 0-light-outside`, 09:37:03): the sky's hour, the cloudiness the recorded
/// contrast's top gives (1.5 less half of it: 1.597 443 22), the wind the
/// wind controller handed the shaders (`_WindVector`, the weather's wind
/// below a magnitude of one).
pub const CUSTOMS_RECORDED_MORNING: Moment = Moment { hour: 9.6175, cloudiness: (1.5 - 1.597_443_2) / 0.5, wind: [-0.267_424_94, 0.267_424_94] };

/// The atmosphere as a level's sky stores it.
pub fn stored_atmosphere(sky: &Fields) -> anyhow::Result<Atmosphere> {
    let f = |name: &str| sky.f32(&format!("Atmosphere.{name}"));
    Ok(Atmosphere {
        rayleigh_multiplier: f("RayleighMultiplier")?,
        mie_multiplier: f("MieMultiplier")?,
        brightness: f("Brightness")?,
        scattering_brightness: f("ScatteringBrightness")?,
        contrast: f("Contrast")?,
        directionality: f("Directionality")?,
        fogginess: f("Fogginess")?,
    })
}
