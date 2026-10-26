//! The game's ambient occlusion, its graphics setting `SSAO`
//! (`docs/spec/ambient-occlusion.md`): the setting's choices, the `HBAO`
//! preset each applies, the noise texture the occlusion turns its
//! directions by, and the figures its passes take. The app runs the passes
//! on the graphics card (`hbao_view`); what they are given is worked out
//! here, to be read and tested.

use std::f64::consts::PI;

/// The camera's far plane (`CameraManager.SetOverallVisibility` sets 10,000
/// whatever the visibility; recorder 0.11.0's camera has it): the occlusion
/// keeps each pixel's depth as a share of it, and the blur weighs depths by
/// it.
pub const FAR_PLANE: f32 = 10_000.0;

/// The setting's choices (`ESSAOMode`), in the game's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsaoMode {
    Off,
    FastestPerformance,
    FastPerformance,
    HighQuality,
    HighestQuality,
    ColoredHighestQuality,
}

impl SsaoMode {
    pub const ALL: [Self; 6] = [Self::Off, Self::FastestPerformance, Self::FastPerformance, Self::HighQuality, Self::HighestQuality, Self::ColoredHighestQuality];

    /// Its name as `Graphics.ini` writes it (`Ssao`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::FastestPerformance => "FastestPerformance",
            Self::FastPerformance => "FastPerformance",
            Self::HighQuality => "HighQuality",
            Self::HighestQuality => "HighestQuality",
            Self::ColoredHighestQuality => "ColoredHighestQuality",
        }
    }

    /// A choice by its name, in any case.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key().eq_ignore_ascii_case(key))
    }

    /// The `HBAO` preset it applies (`CameraManager.SetSSAO`), or none:
    /// `Off` turns `HBAO` off.
    pub fn hbao(self) -> Option<HbaoSettings> {
        let mut s = HbaoSettings::DEFAULTS;
        match self {
            Self::Off => return None,
            Self::FastestPerformance => {
                s.quality = Quality::Lowest;
                s.radius = 0.5;
                s.max_radius_pixels = 64.0;
                s.blur = Blur::ExtraWide;
                s.blur_downsample = true;
            }
            Self::FastPerformance => {
                s.quality = Quality::Low;
                s.radius = 1.0;
                s.max_radius_pixels = 64.0;
                s.blur = Blur::Wide;
                s.blur_downsample = true;
            }
            Self::HighQuality => {
                s.quality = Quality::High;
                s.radius = 1.46;
                s.blur = Blur::Narrow;
            }
            Self::HighestQuality => {
                s.quality = Quality::Highest;
                s.radius = 1.46;
                s.max_radius_pixels = 256.0;
                s.blur = Blur::Narrow;
            }
            Self::ColoredHighestQuality => {
                s.quality = Quality::Highest;
                s.radius = 1.46;
                s.max_radius_pixels = 256.0;
                s.blur = Blur::Narrow;
                s.color_bleeding = true;
            }
        }
        Some(s)
    }

    /// Whether Unity's cinematic ambient occlusion is on with it as well
    /// (`SetSSAO`: from `HighQuality` up). Not built (the spec, "Not built").
    pub fn cinematic(self) -> bool {
        matches!(self, Self::HighQuality | Self::HighestQuality | Self::ColoredHighestQuality)
    }
}

/// `HBAO`'s quality: how many directions the occlusion looks along, and how
/// many steps along each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Lowest,
    Low,
    Medium,
    High,
    Highest,
}

impl Quality {
    /// `HBAO_Core._numSampleDirections`: 3, 4, 6, 8, 8.
    pub fn directions(self) -> u32 {
        match self {
            Self::Lowest => 3,
            Self::Low => 4,
            Self::Medium => 6,
            Self::High | Self::Highest => 8,
        }
    }

    /// The steps along each direction, from the compiled passes of
    /// `Hidden/HBAO` (their step's share of the reach, 1/3, 1/4, 1/5, 1/5,
    /// 1/7, and their number of depth reads, 6, 12, 24, 32, 48).
    pub fn steps(self) -> u32 {
        match self {
            Self::Lowest => 2,
            Self::Low => 3,
            Self::Medium | Self::High => 4,
            Self::Highest => 6,
        }
    }
}

/// `HBAO`'s blur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blur {
    None,
    Narrow,
    Medium,
    Wide,
    ExtraWide,
}

impl Blur {
    /// Its taps on either side of the middle (the compiled passes: 5, 7, 9
    /// and 11 reads with the middle's).
    pub fn reach(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Narrow => 2,
            Self::Medium => 3,
            Self::Wide => 4,
            Self::ExtraWide => 5,
        }
    }

    /// The fall-off of a tap's weight with its place, `1 / (2 σ²)` with
    /// σ half the reach (the passes' constants: 0.5, 0.2222, 0.125, 0.08).
    pub fn falloff(self) -> f32 {
        match self.reach() {
            0 => 0.0,
            reach => 2.0 / (reach * reach) as f32,
        }
    }
}

/// What `HBAO` draws with (`HBAO_Core`'s settings) that the app uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HbaoSettings {
    pub quality: Quality,
    /// Metres: occluders farther than this are not counted.
    pub radius: f32,
    /// The reach on the screen at most.
    pub max_radius_pixels: f32,
    /// The angle bias.
    pub bias: f32,
    pub intensity: f32,
    /// Metres: beyond this, no occlusion.
    pub max_distance: f32,
    /// Metres before the maximum distance over which the occlusion fades.
    pub distance_falloff: f32,
    pub base_color: [f32; 3],
    pub color_bleeding: bool,
    pub blur: Blur,
    pub sharpness: f32,
    /// The blur's first pass into a texture of half the size.
    pub blur_downsample: bool,
}

impl HbaoSettings {
    /// The settings a preset starts from (`GeneralSettings`, `AOSettings`,
    /// `ColorBleedingSettings`, `BlurSettings`: `defaultSettings`): quality
    /// Medium, radius 0.8, 128 pixels, bias 0.05, intensity 0.75, 150 m
    /// with a fall-off of 50, black, no colour bleeding, the Medium blur at
    /// sharpness 8 at full size. Full resolution, no deinterleaving, the
    /// dithered noise and the G-buffer's normals are also its own, and no
    /// preset changes them.
    pub const DEFAULTS: Self = Self {
        quality: Quality::Medium,
        radius: 0.8,
        max_radius_pixels: 128.0,
        bias: 0.05,
        intensity: 0.75,
        max_distance: 150.0,
        distance_falloff: 50.0,
        base_color: [0.0, 0.0, 0.0],
        color_bleeding: false,
        blur: Blur::Medium,
        sharpness: 8.0,
        blur_downsample: false,
    };
}

/// The table the dithered noise is made of (`HBAO_Core.MersenneTwister.
/// Numbers`), taken two at a time.
const DITHER: [f32; 32] = [
    0.463937, 0.340042, 0.223035, 0.468465, 0.322224, 0.979269, 0.031798, 0.973392, 0.778313, 0.456168, 0.258593, 0.330083, 0.387332, 0.380117, 0.179842, 0.910755, 0.511623,
    0.092933, 0.180794, 0.620153, 0.101348, 0.556342, 0.642479, 0.442008, 0.215115, 0.475218, 0.157357, 0.568868, 0.501241, 0.629229, 0.699218, 0.707733,
];

/// A number into a texture of bytes: kept between nought and one, then to
/// the nearest of 255 steps (`PROVISIONAL`: the engine's rounding when a
/// colour is set into such a texture was not read).
fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The noise texture (`CreateRandomTexture` with the dithered noise): 4 × 4
/// texels of red, green, blue and alpha bytes, given row by row from its
/// bottom row (the engine's `y` 0) and each row from the left. The game
/// sets the texels column by column, each from the bottom, taking two of
/// the table's numbers for each: a turn of `2π × first / directions`,
/// whose cosine and sine are red and green, and the second as blue. Its
/// texture is of bytes, so a negative cosine is kept as nought.
pub fn noise(quality: Quality) -> [[u8; 4]; 16] {
    let directions = quality.directions() as f32;
    let mut texels = [[0u8; 4]; 16];
    let mut n = 0;
    for x in 0..4 {
        for y in 0..4 {
            let turn = std::f32::consts::PI * 2.0 * DITHER[n] / directions;
            let jitter = DITHER[n + 1];
            n += 2;
            // `Mathf.Cos` and `Mathf.Sin` work in double precision.
            texels[y * 4 + x] = [byte((turn as f64).cos() as f32), byte((turn as f64).sin() as f32), byte(jitter), 255];
        }
    }
    texels
}

/// Each direction's turn from the noise's, as cosine and sine: a whole turn
/// shared out among the directions (the compiled passes' constants: for
/// three, −0.5 and ±0.866025).
pub fn turns(quality: Quality) -> Vec<[f32; 2]> {
    let directions = quality.directions();
    (0..directions)
        .map(|d| {
            let turn = 2.0 * PI * d as f64 / directions as f64;
            [turn.cos() as f32, turn.sin() as f32]
        })
        .collect()
}

/// The figures the occlusion's pass takes for a view of `height` pixels
/// whose vertical field of view has `tan_half_fov` for its half's tangent
/// (`UpdateShaderProperties`, without deinterleaving).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Figures {
    /// `_Radius`: the reach in pixels at a depth of one metre.
    pub radius: f32,
    pub max_radius_pixels: f32,
    /// `_NegInvRadius2`: `−1 / radius²`.
    pub neg_inv_radius2: f32,
    pub angle_bias: f32,
    /// `_AOmultiplier`: `2 / (1 − bias)`.
    pub ao_multiplier: f32,
    pub max_distance: f32,
    pub distance_falloff: f32,
}

impl Figures {
    pub fn of(settings: &HbaoSettings, height: u32, tan_half_fov: f32) -> Self {
        Self {
            radius: settings.radius * 0.5 * (height as f32 / (tan_half_fov * 2.0)),
            max_radius_pixels: settings.max_radius_pixels,
            neg_inv_radius2: -1.0 / (settings.radius * settings.radius),
            angle_bias: settings.bias,
            ao_multiplier: 2.0 * (1.0 / (1.0 - settings.bias)),
            max_distance: settings.max_distance,
            distance_falloff: settings.distance_falloff,
        }
    }
}

/// `_UVToView` for a view whose vertical half-angle has the tangent `b`
/// and whose width is `aspect` times its height: a place on the screen
/// (`u` from the left, `v` from the bottom) times this, plus its second
/// half, times the depth, is the place in the view (x right, y down).
pub fn uv_to_view(tan_half_fov: f32, aspect: f32) -> [f32; 4] {
    let across = 1.0 / (1.0 / tan_half_fov * (1.0 / aspect));
    let up = 1.0 / (1.0 / tan_half_fov);
    [2.0 * across, -2.0 * up, -across, up]
}

/// A depth as the occlusion writes it: its share of the far plane in two
/// bytes' worth (`EncodeFloatRG`), red and green.
pub fn pack_depth(z: f32) -> [f32; 2] {
    let share = (z * (1.0 / FAR_PLANE)).clamp(0.0, 1.0);
    let fine = (share * 255.0).fract();
    [share.fract() - fine * (1.0 / 255.0), fine]
}

/// The share back from red and green as stored.
pub fn unpack_depth(rg: [f32; 2]) -> f32 {
    rg[0] + rg[1] * (1.0 / 255.0)
}

/// A blur tap's weight `k` texels from the middle, its stored depth
/// `depth_difference` from the middle's (`2^(−k² f − (Δ × far ×
/// sharpness)²)`; the middle weighs 1).
pub fn blur_weight(blur: Blur, k: i32, depth_difference: f32, sharpness: f32) -> f32 {
    let d = depth_difference * FAR_PLANE * sharpness;
    (-((k * k) as f32) * blur.falloff() - d * d).exp2()
}

/// The composite's factor for a stored occlusion: `min(ao^intensity, 1)`
/// between the base colour and white.
pub fn composite(ao: f32, intensity: f32, base: [f32; 3]) -> [f32; 3] {
    let f = (ao.log2() * intensity).exp2().min(1.0);
    base.map(|b| b + f * (1.0 - b))
}
