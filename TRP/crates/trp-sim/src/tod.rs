//! A level's time of day as the game runs it (`docs/spec/lighting.md`,
//! "Customs by its time of day"): the Time of Day sky's rules (`TOD_Sky`:
//! the scattering's constants, the sun's and the moon's places by the date,
//! the hour and the world's place, the sky's colours by the time of day, the
//! light's intensity, and the sky's colour in a direction as its shader
//! works it out, sampled on the processor) and the game's time-of-day
//! controller's (`ToDController`: the atmosphere's figures by the sun's
//! height, the sun's colour, the night's colour multiplier, and the ambient
//! harmonics from five samples of the sky). A fixed moment converges in a
//! few frames, as the game's two late updates feed each other.

use trp_data::particles::Gradient;
use trp_data::tod::{Fields, TimeOfDayLevel, WeightedCurve};

use crate::ambient::Harmonics;

/// A colour's three channels.
pub type Rgb = [f32; 3];

fn rgb(c: [f32; 4]) -> Rgb {
    [c[0], c[1], c[2]]
}

fn scale(c: Rgb, k: f32) -> Rgb {
    c.map(|v| v * k)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn lerp_unclamped(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    if a != b {
        ((v - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    if length > 1e-5 {
        v.map(|c| c / length)
    } else {
        [0.0; 3]
    }
}

/// A vector turned by a quaternion (x, y, z, w), and by its inverse.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    trp_data::level_light::rotate(q, v)
}

fn rotate_back(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    rotate([-q[0], -q[1], -q[2], q[3]], v)
}

/// `OrbitalToLocal`: a unit vector from the zenith angle and the azimuth.
fn orbital_to_local(theta: f32, phi: f32) -> [f32; 3] {
    let (s, c) = theta.sin_cos();
    let (sp, cp) = phi.sin_cos();
    [s * sp, c, s * cp]
}

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

/// What the sky works out on a frame (`UpdateCelestials`) and the
/// controller (`ToDController.Update`), a frame's state of both.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub atmosphere: Atmosphere,
    pub scattering: Scattering,
    pub sun_zenith: f32,
    pub moon_zenith: f32,
    /// In the world, toward the sun and the moon; in the dome's own axes.
    pub sun_direction: [f32; 3],
    pub moon_direction: [f32; 3],
    pub local_sun_direction: [f32; 3],
    pub local_moon_direction: [f32; 3],
    pub lerp_value: f32,
    pub sun_sky_color: Rgb,
    pub moon_sky_color: Rgb,
    pub ambient_color: Rgb,
    pub moon_halo_color: Rgb,
    /// The ground's colour below the horizon (the ambient's, the night's a
    /// quarter) and the sun's mesh's.
    pub ground_color: Rgb,
    pub sun_mesh_color: Rgb,
    /// The light (the sun by day): its turn, intensity, shadow strength and
    /// colour (the controller's).
    pub light_rotation: [f32; 4],
    pub light_intensity: f32,
    pub shadow_strength: f32,
    pub light_color: Rgb,
    pub night_color_multiplier: f32,
    /// The ambient's harmonics (the job's, a frame late as the game's) and
    /// the sum of its five samples' colours (`TopHorizontSkyColor`).
    pub harmonics: Harmonics,
    pub top_horizon_sky_color: Rgb,
    /// The job's harmonics without the sky's samples and the light from
    /// above (`SHWithoutTop`: the sun's bounce and the even ambient), which
    /// the ambient highlight draws.
    pub harmonics_without_top: Harmonics,
    /// The local sidereal angle (`UpdateCelestials`' `num32`, radians), which
    /// turns the stars and the moon's up.
    pub sidereal: f32,
}

/// `Quaternion.LookRotation(forward)` with up the world's up.
fn look_rotation(forward: [f32; 3]) -> [f32; 4] {
    let z = normalized(forward);
    let x = normalized([z[2], 0.0, -z[0]]);
    let y = [z[1] * x[2] - z[2] * x[1], z[2] * x[0] - z[0] * x[2], z[0] * x[1] - z[1] * x[0]];
    // The rotation whose columns are x, y, z.
    let (m00, m01, m02, m10, m11, m12, m20, m21, m22) = (x[0], y[0], z[0], x[1], y[1], z[1], x[2], y[2], z[2]);
    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s]
    }
}

impl Sky {
    /// The sun's and the moon's places, the sky's colours and the light, by
    /// the hour (`UpdateCelestials`); the atmosphere's constants from the
    /// state's atmosphere (`UpdateScattering`).
    pub fn celestials(&self, hour: f32, state: &mut State) {
        state.scattering = Scattering::of(&state.atmosphere);
        let deg = std::f32::consts::PI / 180.0;
        let f = deg * self.latitude;
        let (num, num2) = (f.sin(), f.cos());
        let longitude = self.longitude;
        let num3 = std::f32::consts::PI / 2.0;
        let (year, month, day) = self.date;
        let num4 = hour - self.utc;
        let num5 = (367 * year - 7 * (year + (month + 9) / 12) / 4 + 275 * month / 9 + day - 730_530) as f32 + num4 / 24.0;
        let num6 = 23.4393 - 3.563e-7 * num5;
        let f2 = deg * num6;
        let (num7, num8) = (f2.sin(), f2.cos());
        let num9 = self.north;
        let num10 = 282.9404 + 4.709_35e-5 * num5;
        let num11 = 0.016_709 - 1.151e-9 * num5;
        let num12 = 356.047 + 0.985_600_23 * num5;
        let num13 = deg * num12;
        let (num14, num15) = (num13.sin(), num13.cos());
        let f3 = num13 + num11 * num14 * (1.0 + num11 * num15);
        let num16 = f3.sin();
        let num17 = f3.cos() - num11;
        let num18 = (1.0 - num11 * num11).sqrt() * num16;
        let num19 = 57.295_78 * num18.atan2(num17);
        let num20 = (num17 * num17 + num18 * num18).sqrt();
        let num21 = num19 + num10;
        let f4 = deg * num21;
        let (num22, num23) = (f4.sin(), f4.cos());
        let num24 = num20 * num23;
        let num25 = num20 * num22;
        let num26 = num24;
        let num27 = num25 * num8;
        let y = num25 * num7;
        let num28 = num27.atan2(num26);
        let f5 = y.atan2((num26 * num26 + num27 * num27).sqrt());
        let (num29, num30) = (f5.sin(), f5.cos());
        let num31 = num19 + num10 + 180.0 + 15.0 * num4;
        let num32 = deg * (num31 + longitude);
        state.sidereal = num32;
        let f6 = num32 - num28;
        let num33 = f6.sin();
        let num34 = f6.cos() * num30;
        let num35 = num33 * num30;
        let num36 = num29;
        let num37 = num34 * num - num36 * num2;
        let num38 = num35;
        let y2 = num34 * num2 + num36 * num;
        let num39 = num38.atan2(num37) + deg * num9;
        let num40 = y2.atan2((num37 * num37 + num38 * num38).sqrt());
        let num41 = num3 - num40;
        let num42 = -num39;
        let (num82, num83) = if self.moon_realistic {
            let num43 = 125.1228 - 0.052_953_81 * num5;
            let num44 = 318.0634 + 0.164_357_32 * num5;
            let num45 = 0.0549f32;
            let num46 = 115.3654 + 13.064_993 * num5;
            let f7 = deg * num43;
            let (num47, num48) = (f7.sin(), f7.cos());
            let (num49, num50) = (0.089_804_17f32.sin(), 0.089_804_17f32.cos());
            let num51 = deg * num46;
            let (num52, num53) = (num51.sin(), num51.cos());
            let f8 = num51 + num45 * num52 * (1.0 + num45 * num53);
            let (num54, num55) = (f8.sin(), f8.cos());
            let num56 = 60.2666 * (num55 - num45);
            let num57 = 60.2666 * (0.996_986f32.sqrt() * num54);
            let num58 = 57.295_78 * num57.atan2(num56);
            let num59 = (num56 * num56 + num57 * num57).sqrt();
            let num60 = num58 + num44;
            let f9 = deg * num60;
            let (num61, num62) = (f9.sin(), f9.cos());
            let num63 = num59 * (num48 * num62 - num47 * num61 * num50);
            let num64 = num59 * (num47 * num62 + num48 * num61 * num50);
            let num65 = num59 * (num61 * num49);
            let num69 = num63;
            let num70 = num64 * num8 - num65 * num7;
            let y3 = num64 * num7 + num65 * num8;
            let num71 = num70.atan2(num69);
            let f10 = y3.atan2((num69 * num69 + num70 * num70).sqrt());
            let (num72, num73) = (f10.sin(), f10.cos());
            let f11 = num32 - num71;
            let num74 = f11.sin();
            let num75 = f11.cos() * num73;
            let num76 = num74 * num73;
            let num77 = num72;
            let num78 = num75 * num - num77 * num2;
            let num79 = num76;
            let y4 = num75 * num2 + num77 * num;
            let num80 = num79.atan2(num78) + deg * num9;
            let num81 = y4.atan2((num78 * num78 + num79 * num79).sqrt());
            (num3 - num81, -num80)
        } else {
            (num41 - std::f32::consts::PI, num42)
        };
        state.sun_zenith = 57.295_78 * num41;
        state.moon_zenith = 57.295_78 * num82;
        // The sun and the moon in the dome's axes; in the world through the
        // dome's turn (each object looks at the dome's middle).
        state.local_sun_direction = orbital_to_local(num41, num42);
        state.local_moon_direction = orbital_to_local(num82, num83);
        state.sun_direction = normalized(rotate(self.dome_rotation, state.local_sun_direction));
        state.moon_direction = normalized(rotate(self.dome_rotation, state.local_moon_direction));
        state.local_sun_direction = rotate_back(self.dome_rotation, state.sun_direction);
        state.local_moon_direction = rotate_back(self.dome_rotation, state.moon_direction);

        state.lerp_value = inverse_lerp(110.0, 80.0, state.sun_zenith);
        let time = 1.0 - state.lerp_value;
        let clear = 1.0 - state.atmosphere.fogginess;
        let moon_up = if self.moon_realistic { ((90.0 - num82 * 57.295_78) / 5.0).clamp(0.0, 1.0) } else { ((90.0 + num82 * 57.295_78) / 5.0).clamp(0.0, 1.0) };
        let num88 = (clear * (state.lerp_value - 0.1) / 0.9).clamp(0.0, 1.0);
        let num89 = (clear * moon_up).clamp(0.0, 1.0);
        let night = state.night_color_multiplier;
        state.sun_sky_color = scale(rgb(self.day.sky_color.evaluate(time)), self.day.color_multiplier);
        state.moon_sky_color = scale(rgb(self.night.sky_color.evaluate(time)), 0.25 * night);
        let day_ambient = scale(rgb(self.day.ambient_color.evaluate(time)), self.day.color_multiplier);
        let night_ambient = scale(rgb(self.night.ambient_color.evaluate(time)), 0.5 * night);
        state.ambient_color = std::array::from_fn(|i| lerp(night_ambient[i], day_ambient[i], state.lerp_value));
        let night_ground = scale(rgb(self.night.ambient_color.evaluate(time)), 0.25 * night);
        state.ground_color = std::array::from_fn(|i| lerp(night_ground[i], day_ambient[i], state.lerp_value));
        state.sun_mesh_color = scale(rgb(self.sun_mesh.color.evaluate(time)), self.day.color_multiplier);
        state.moon_halo_color = scale(rgb(self.moon_halo_color.evaluate(time)), 0.25 * night * moon_up);
        let is_day = state.lerp_value > 0.1;
        if is_day {
            state.shadow_strength = self.day.shadow_strength;
            state.light_intensity = lerp(0.0, self.day.light_intensity, num88);
        } else {
            state.shadow_strength = self.night.shadow_strength;
            state.light_intensity = lerp(0.0, self.night.light_intensity, num89);
        }
        // The light from the sun (or the moon) toward the dome's middle, its
        // height held to the least (`Light.MinimumHeight`); the dome's own
        // place taken as the world's middle, as the game's sum has it.
        let top = (1.0 - self.minimum_light_height) * std::f32::consts::PI / 2.0;
        let toward = if is_day { orbital_to_local(num41.min(top), num42) } else { orbital_to_local(num82.min(top), num83) };
        state.light_rotation = look_rotation(toward.map(|c| -c));
    }
}

/// `TOD_Util.MulRGB`, `ContrastColor`, `ContrastColor2`, `SaturateColor`,
/// `LimitColor` (the controller's).
fn contrast(c: Rgb, t: f32) -> Rgb {
    c.map(|v| lerp_unclamped(0.5, v, t))
}

fn contrast2(light: f32, c: Rgb, t: f32) -> Rgb {
    c.map(|v| lerp_unclamped(light, v, t))
}

fn saturate(c: Rgb, t: f32) -> Rgb {
    let grey = (c[0] + c[1] + c[2]) * (1.0 / 3.0);
    c.map(|v| lerp_unclamped(grey, v, t))
}

fn limit(c: Rgb) -> Rgb {
    c.map(|v| v.max(0.0))
}

impl Sky {
    /// `ShaderScale`.
    fn shader_scale(in_cos: f32) -> f32 {
        let n = 1.0 - in_cos;
        0.25 * (-0.00287 + n * (0.459 + n * (3.83 + n * (-6.8 + n * 5.25)))).exp()
    }

    /// The sky's colour in a direction of the dome's own axes, as its shader
    /// works it out (`ShaderScatteringColor`): two samples of the light
    /// scattered toward the eye, the Rayleigh's and (with the direct light)
    /// the Mie's phases, the night sky and (with the direct light) the
    /// moon's halo, half way to the ambient colour, times the brightness and
    /// to the contrast's power.
    pub fn scattering_color(state: &State, mut dir: [f32; 3], direct_light: bool, halo_size: f32) -> Rgb {
        dir[1] = dir[1].clamp(0.0, 1.0);
        let s = &state.scattering;
        let (x, y, w) = (s.radius[0], s.radius[1], s.radius[3]);
        let (x2, z, w2) = (s.scale[0], s.scale[2], s.scale[3]);
        let (x3, y2, z2, w3) = (s.four_pi[0], s.four_pi[1], s.four_pi[2], s.four_pi[3]);
        let (x4, y3, z3, w4) = (s.sun[0], s.sun[1], s.sun[2], s.sun[3]);
        let rhs = [0.0, x + w2, 0.0];
        let num = (w + y * dir[1] * dir[1] - y).sqrt() - x * dir[1];
        let num2 = (z * -w2).exp();
        let in_cos = dot(dir, rhs) / (x + w2);
        let num3 = num2 * Self::shader_scale(in_cos);
        let num4 = num / 2.0;
        let num5 = num4 * x2;
        let step = dir.map(|c| c * num4);
        let mut at = [rhs[0] + step[0] * 0.5, rhs[1] + step[1] * 0.5, rhs[2] + step[2] * 0.5];
        let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
        for _ in 0..2 {
            let magnitude = dot(at, at).sqrt();
            let inv = 1.0 / magnitude;
            let num10 = (z * (x - magnitude)).exp();
            let num11 = num10 * num5;
            let in_cos2 = dot(dir, at) * inv;
            let in_cos3 = dot(state.local_sun_direction, at) * inv;
            let num12 = num3 + num10 * (Self::shader_scale(in_cos3) - Self::shader_scale(in_cos2));
            r += (-num12 * (x3 + w3)).exp() * num11;
            g += (-num12 * (y2 + w3)).exp() * num11;
            b += (-num12 * (z2 + w3)).exp() * num11;
            at = [at[0] + step[0], at[1] + step[1], at[2] + step[2]];
        }
        let sky = state.sun_sky_color;
        let rayleigh = [sky[0] * r * x4, sky[1] * g * y3, sky[2] * b * z3];
        let mie = [sky[0] * r * w4, sky[1] * g * w4, sky[2] * b * w4];
        let cos = dot(state.local_sun_direction, dir);
        let cos2 = cos * cos;
        let phase = 0.75 + 0.75 * cos2;
        let mut c: Rgb = rayleigh.map(|v| phase * v);
        if direct_light {
            let m = &state.scattering.beta_mie;
            let mie_phase = m[0] * (1.0 + cos2) / (m[1] + m[2] * cos).powf(1.5);
            c = std::array::from_fn(|i| c[i] + mie_phase * mie[i]);
        }
        let night: Rgb = std::array::from_fn(|i| lerp(state.moon_sky_color[i], 0.0, dir[1]));
        c = std::array::from_fn(|i| c[i] + night[i]);
        if direct_light {
            let halo = dot(dir, state.local_moon_direction).max(0.0).powf(1.0 / halo_size);
            c = std::array::from_fn(|i| c[i] + state.moon_halo_color[i] * halo);
        }
        let a = &state.atmosphere;
        std::array::from_fn(|i| (lerp(c[i], state.ambient_color[i], 0.5) * a.brightness).powf(a.contrast))
    }

    /// `SampleAtmosphereRawMT`: the scattering in a world direction, through
    /// `TOD_HDR2LDR` and to gamma by a square root.
    pub fn sample_atmosphere(&self, state: &State, direction: [f32; 3], direct_light: bool) -> Rgb {
        let dir = rotate_back(self.dome_rotation, direction);
        let c = Self::scattering_color(state, dir, direct_light, 0.1);
        c.map(|v| (1.0 - 2f32.powf(-state.atmosphere.brightness * v)).sqrt())
    }

    /// The controller's frame (`ToDController.Update`): the atmosphere's
    /// figures by the sun's height, the light's colour, the night's colour
    /// multiplier; the ambient's harmonics from the frame before's job, and
    /// the job worked out again (`CalculateSphericalHarmonics`) for the
    /// next. `cloudiness` is the weather's (`WeatherCurve.Cloudiness`).
    pub fn controller_update(&self, state: &mut State, cloudiness: f32) {
        let c = &self.controller;
        let y = state.sun_direction[1];
        let fog = state.atmosphere.fogginess;
        let a = &mut state.atmosphere;
        a.rayleigh_multiplier = lerp_unclamped(c.rayleigh_min_max[0], c.rayleigh_min_max[1], c.rayleigh.evaluate(y));
        a.mie_multiplier = lerp_unclamped(c.mie_min_max[0], c.mie_min_max[1], c.mie.evaluate(y));
        a.directionality = 1.0 - c.directionality.evaluate(y) * c.directionality_mult;
        a.brightness = lerp_unclamped(c.brightness_min_max[0], c.brightness_min_max[1], c.brightness.evaluate(y)) * (1.0 - fog * c.brightness_overcast);
        a.scattering_brightness = c.scattering_brightness.evaluate(y) * c.scattering_brightness_multiplier * a.brightness;
        // The contrast's top as the weather sets it (`WeatherController`).
        let top = 1.5 - cloudiness.clamp(-1.0, 0.4) * 0.5;
        a.contrast = lerp_unclamped(c.contrast_min_max[0], top, c.contrast.evaluate(y));
        let t01 = y * 0.5 + 0.5;
        state.light_color = saturate(rgb(c.light_color.evaluate(t01)), 1.0 - fog);
        let t = 0.5 - 0.5 * dot(state.moon_direction, state.sun_direction);
        state.night_color_multiplier = lerp_unclamped(c.moon_light_min_max[0], c.moon_light_min_max[1], t);
        let (harmonics, without_top, top_horizon) = self.harmonics(state, y, t01, -cloudiness);
        state.harmonics = harmonics;
        state.harmonics_without_top = without_top;
        state.top_horizon_sky_color = top_horizon;
    }

    /// `ComputeSphericalHarmonicsContainerJob.CalculateSphericalHarmonics`
    /// (its full harmonics, the daylight's share nought as it stays), and
    /// the sum of its five samples.
    fn harmonics(&self, state: &State, t: f32, t01: f32, cloudiness: f32) -> (Harmonics, Harmonics, Rgb) {
        let c = &self.controller;
        let forward = scale(rgb(c.forward_harmonic_multiplier.evaluate(t01)), 2.0 * c.forward_harmonic_intensity.evaluate(t01));
        let backward = scale(rgb(c.backward_harmonic_multiplier.evaluate(t01)), 2.0);
        let mut top = c.top_harmonic_intensity.evaluate(t);
        let horizon = c.horizont_harmonic_intensity.evaluate(t01);
        let fog = state.atmosphere.fogginess;
        let mut sun_flat = state.sun_direction;
        sun_flat[1] = 0.0;
        let sun_flat = normalized(sun_flat);
        let dirs = [[0.0, 1.0, 0.0], sun_flat, sun_flat.map(|v| -v), [sun_flat[2], 0.0, -sun_flat[0]], [-sun_flat[2], 0.0, sun_flat[0]]];
        let contrast_t = c.ambient_contrast.evaluate(t);
        let mut saturation_t = c.ambient_saturation.evaluate(t);
        let brightness = c.ambient_brightness.evaluate(t);
        let light = lerp(0.003, 0.25, inverse_lerp(0.07, 1.0, t));
        let num6 = lerp(1.0, 0.3, fog);
        saturation_t *= 1.0 - fog;
        let num7 = inverse_lerp(-0.4, 0.2, cloudiness);
        let num8 = 1.0 + num7;
        top *= num8;
        let num9 = (1.0 - (cloudiness + fog) * 1.3).max(0.0);
        let graded = |col: Rgb| limit(scale(saturate(contrast2(light, contrast(col, contrast_t), num6), saturation_t), brightness));

        let mut without_top = Harmonics::default();
        let bounce = normalized([-state.sun_direction[0], -state.sun_direction[1] - 1.0, -state.sun_direction[2]]);
        without_top.add_directional(bounce, graded(state.light_color), c.bounce_harmonic_intensity.evaluate(t) * num9);
        let ambient = rgb(c.add_ambient.evaluate(t01));
        let clear = t.clamp(0.0, 1.0) * num8;
        without_top.add_ambient(graded(std::array::from_fn(|i| ambient[i] + c.cloudness_light_color[i] * clear)));
        let mut full = Harmonics::default().plus(without_top);
        let mut sum = [0.0; 3];
        for (i, dir) in dirs.iter().enumerate() {
            let mut col = self.sample_atmosphere(state, *dir, false);
            if i == 1 {
                col = std::array::from_fn(|k| col[k] * forward[k]);
            }
            if i == 2 {
                col = std::array::from_fn(|k| col[k] * backward[k]);
            }
            let col = graded(col);
            full.add_directional(*dir, col, if i == 0 { top } else { horizon });
            sum = std::array::from_fn(|k| sum[k] + col[k]);
        }
        full.add_directional([0.0, 1.0, 0.0], graded(rgb(c.add_top_ambient.evaluate(t01))), 1.0);
        (full, without_top, sum)
    }

    /// A fixed moment as the game settles on it: from the atmosphere as
    /// stored, the controller and the sky in turn, so many frames (the
    /// controller's harmonics are worked out with the sky's last state).
    pub fn settle(&self, hour: f32, cloudiness: f32, stored: Atmosphere, frames: usize) -> State {
        let mut state = State { atmosphere: stored, ..State::default() };
        // The weather's fogginess by its cloudiness (`WeatherController`).
        state.atmosphere.fogginess = inverse_lerp(0.0, 0.4, cloudiness) * 0.5 + inverse_lerp(0.1, 0.4, cloudiness) * 0.5;
        state.night_color_multiplier = self.night.color_multiplier;
        self.celestials(hour, &mut state);
        for _ in 0..frames {
            self.controller_update(&mut state, cloudiness);
            self.celestials(hour, &mut state);
        }
        state
    }
}

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
    pub moon_mesh: crate::clouds::MoonFigures,
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
            moon_mesh: crate::clouds::MoonFigures::of(
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
