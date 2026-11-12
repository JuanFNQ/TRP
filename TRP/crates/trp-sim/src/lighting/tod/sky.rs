//! The sky read from a level: periods, the controller, the atmosphere and scattering.

use super::*;

/// The figures of a day or a night (`TOD_DayParameters`).
#[derive(Debug, Clone)]
pub struct Period {
    pub sky_color: Gradient,
    pub light_color: Gradient,
    pub ambient_color: Gradient,
    pub light_intensity: f32,
    pub shadow_strength: f32,
    pub color_multiplier: f32,
}

impl Period {
    fn read(f: &Fields, prefix: &str) -> anyhow::Result<Self> {
        let n = |name: &str| format!("{prefix}.{name}");
        Ok(Self {
            sky_color: f.gradient(&n("SkyColor"))?.clone(),
            light_color: f.gradient(&n("LightColor"))?.clone(),
            ambient_color: f.gradient(&n("AmbientColor"))?.clone(),
            light_intensity: f.f32(&n("LightIntensity"))?,
            shadow_strength: f.f32(&n("ShadowStrength"))?,
            color_multiplier: f.f32(&n("ColorMultiplier"))?,
        })
    }
}

/// The game's time-of-day controller's figures (`ToDController`).
#[derive(Debug, Clone)]
pub struct Controller {
    pub rayleigh_min_max: [f32; 2],
    pub rayleigh: WeightedCurve,
    pub mie_min_max: [f32; 2],
    pub mie: WeightedCurve,
    pub directionality_mult: f32,
    pub directionality: WeightedCurve,
    pub brightness_min_max: [f32; 2],
    pub brightness: WeightedCurve,
    pub scattering_brightness_multiplier: f32,
    pub scattering_brightness: WeightedCurve,
    pub brightness_overcast: f32,
    pub contrast_min_max: [f32; 2],
    pub contrast: WeightedCurve,
    pub moon_light_min_max: [f32; 2],
    pub light_color: Gradient,
    pub top_harmonic_intensity: WeightedCurve,
    pub horizont_harmonic_intensity: WeightedCurve,
    pub bounce_harmonic_intensity: WeightedCurve,
    pub forward_harmonic_intensity: WeightedCurve,
    pub forward_harmonic_multiplier: Gradient,
    pub backward_harmonic_multiplier: Gradient,
    pub add_top_ambient: Gradient,
    pub add_ambient: Gradient,
    pub ambient_brightness: WeightedCurve,
    pub ambient_saturation: WeightedCurve,
    pub ambient_contrast: WeightedCurve,
    pub cloudness_light_color: Rgb,
}

/// The sun's mesh (`TOD_SunParameters`): its colour by the time of day, its
/// size in degrees, its brightness and contrast.
#[derive(Debug, Clone)]
pub struct SunMesh {
    pub color: Gradient,
    pub size: f32,
    pub brightness: f32,
    pub contrast: f32,
}

/// The sky's own figures (`TOD_Sky`) the rules take.
#[derive(Debug, Clone)]
pub struct Sky {
    pub latitude: f32,
    pub longitude: f32,
    pub utc: f32,
    pub day: Period,
    pub night: Period,
    pub moon_halo_color: Gradient,
    pub moon_halo_size: f32,
    pub sun_mesh: SunMesh,
    /// The moon's mesh (`TOD_MoonParameters`: its colour, size in degrees,
    /// brightness, contrast).
    pub moon_mesh: SunMesh,
    /// `TOD_MoonPositionType`: 1 realistic.
    pub moon_realistic: bool,
    pub minimum_light_height: f32,
    pub date: (i32, i32, i32),
    pub controller: Controller,
    /// The dome's turn in the world, and the level's north (degrees).
    pub dome_rotation: [f32; 4],
    pub north: f32,
}

impl Sky {
    /// From a level's scripts as read.
    pub fn of_level(level: &TimeOfDayLevel) -> anyhow::Result<Self> {
        let s = &level.sky;
        let w = &level.weather;
        let c = |name: &str| format!("TimeOfDayController.{name}");
        let curve = |name: &str| w.curve(&c(name)).cloned();
        let controller = Controller {
            rayleigh_min_max: w.vec2(&c("RayleighMultiplierMinMax"))?,
            rayleigh: curve("RayleighMultiplier")?,
            mie_min_max: w.vec2(&c("MieMultiplierMinMax"))?,
            mie: curve("MieMultiplier")?,
            directionality_mult: w.f32(&c("DirectionalityMult"))?,
            directionality: curve("Directionality")?,
            brightness_min_max: w.vec2(&c("BrightnessMinMax"))?,
            brightness: curve("Brightness")?,
            scattering_brightness_multiplier: w.f32(&c("ScatteringBrightnessMultiplier"))?,
            scattering_brightness: curve("ScatteringBrightness")?,
            brightness_overcast: w.f32(&c("BrightnessOvercast"))?,
            contrast_min_max: w.vec2(&c("ContrastMinMax"))?,
            contrast: curve("Contrast")?,
            moon_light_min_max: w.vec2(&c("MoonLightMinMax"))?,
            light_color: w.gradient(&c("LightColor"))?.clone(),
            top_harmonic_intensity: curve("TopHarmonicIntensity")?,
            horizont_harmonic_intensity: curve("HorizontHarmonicIntensity")?,
            bounce_harmonic_intensity: curve("BounceHarmonicIntensity")?,
            forward_harmonic_intensity: curve("ForwarHarmonicIntensity")?,
            forward_harmonic_multiplier: w.gradient(&c("ForwarHarmonicMultiplyer"))?.clone(),
            backward_harmonic_multiplier: w.gradient(&c("BackwardHarmonicMultiplyer"))?.clone(),
            add_top_ambient: w.gradient(&c("AddTopAmbient"))?.clone(),
            add_ambient: w.gradient(&c("AddAmbient"))?.clone(),
            ambient_brightness: curve("AmbientBrightness")?,
            ambient_saturation: curve("AmbientSaturation")?,
            ambient_contrast: curve("AmbientContrast")?,
            cloudness_light_color: rgb(w.color(&c("CloudnessLightColor"))?),
        };
        Ok(Self {
            latitude: s.f32("World.Latitude")?,
            longitude: s.f32("World.Longitude")?,
            utc: s.f32("World.UTC")?,
            day: Period::read(s, "Day")?,
            night: Period::read(s, "Night")?,
            moon_halo_color: s.gradient("Moon.HaloColor")?.clone(),
            moon_halo_size: s.f32("Moon.HaloSize")?,
            sun_mesh: SunMesh {
                color: s.gradient("Sun.MeshColor")?.clone(),
                size: s.f32("Sun.MeshSize")?,
                brightness: s.f32("Sun.MeshBrightness")?,
                contrast: s.f32("Sun.MeshContrast")?,
            },
            moon_mesh: SunMesh {
                color: s.gradient("Moon.MeshColor")?.clone(),
                size: s.f32("Moon.MeshSize")?,
                brightness: s.f32("Moon.MeshBrightness")?,
                contrast: s.f32("Moon.MeshContrast")?,
            },
            moon_realistic: s.int("Moon.Position")? == 1,
            minimum_light_height: s.f32("Light.MinimumHeight")?,
            date: (s.int("Cycle.Year")?, s.int("Cycle.Month")?, s.int("Cycle.Day")?),
            controller,
            dome_rotation: level.dome_rotation,
            north: level.settings.north,
        })
    }
}

/// The atmosphere's figures (`TOD_AtmosphereParameters`), as the controller
/// sets them each frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Atmosphere {
    pub rayleigh_multiplier: f32,
    pub mie_multiplier: f32,
    pub brightness: f32,
    pub scattering_brightness: f32,
    pub contrast: f32,
    pub directionality: f32,
    pub fogginess: f32,
}

/// The scattering's constants (`UpdateScattering`): `kBetaMie`, `kSun`,
/// `k4PI`, `kRadius`, `kScale`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Scattering {
    pub beta_mie: [f32; 3],
    pub sun: [f32; 4],
    pub four_pi: [f32; 4],
    pub radius: [f32; 4],
    pub scale: [f32; 4],
}

impl Scattering {
    pub fn of(atmosphere: &Atmosphere) -> Self {
        let g = -atmosphere.directionality;
        let g2 = g * g;
        let mie = 0.002 * atmosphere.mie_multiplier;
        let rayleigh = 0.002 * atmosphere.rayleigh_multiplier;
        let pi = std::f32::consts::PI;
        Self {
            beta_mie: [1.5 * ((1.0 - g2) / (2.0 + g2)), 1.0 + g2, 2.0 * g],
            sun: [rayleigh * 40.0 * 5.270_164_5, rayleigh * 40.0 * 9.473_284, rayleigh * 40.0 * 19.643_803, mie * 40.0],
            four_pi: [rayleigh * 4.0 * pi * 5.270_164_5, rayleigh * 4.0 * pi * 9.473_284, rayleigh * 4.0 * pi * 19.643_803, mie * 4.0 * pi],
            radius: [1.0, 1.0, 1.025, 1.050_625],
            scale: [40.000_04, 0.25, 160.000_15, 0.0001],
        }
    }
}
