//! The game's ambient light as spherical harmonics (`docs/spec/lighting.md`,
//! "The ambient"). A level's simple sky (`TODSkySimple`) works them out on
//! every frame from the sun's height by its gradients and curves; the
//! game's ambient script (`AmbientLight.SetSH`) packs them into the
//! shaders' `_SHAr` to `_SHC` and hands the shaders their colour from
//! straight above (`_EFT_Ambient`). The engine's own operations on
//! harmonics (`SphericalHarmonicsL2`) are as its code has them. All in the
//! game's gamma space, where a colour is used as it is stored.

use trp_data::level_light::SkySimple;

/// Nine coefficients for each of red, green and blue
/// (`SphericalHarmonicsL2`, its `[channel, coefficient]`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Harmonics(pub [[f32; 9]; 3]);

/// What a directional light's colour is scaled by before it is spread over
/// the harmonics (the engine's `SphericalHarmonicsL2::AddDirectionalLight`:
/// 16π/17).
const DIRECTIONAL_SCALE: f32 = 2.956_793;

/// The harmonics' bases at a unit direction, as the engine writes them:
/// the constant, the three of the first band, then xy, yz, z² less a third,
/// xz and x² less y² of the second.
const BASIS: [f32; 5] = [0.282_094_8, 0.488_602_5, 1.092_548_5, 0.946_174_7, 0.546_274_2];

/// The engine's normalisation of each coefficient (`kNormalizationConstants`):
/// each band's basis constant, signed as the engine's axes have it, times
/// the band's share of a cosine lobe (1, 2/3, 1/4).
const NORMALISATION: [f32; 9] = [0.282_094_8, -0.325_735, 0.325_735, -0.325_735, 0.273_137_1, -0.273_137_1, 0.078_847_9, -0.273_137_1, 0.136_568_6];

/// The packing the shaders take (`AmbientLight.SetSH`): for each channel
/// `a` (`_SHAr`, `_SHAg`, `_SHAb`) and `b` (`_SHBr` ...), and `c` (`_SHC`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Packed {
    pub a: [[f32; 4]; 3],
    pub b: [[f32; 4]; 3],
    pub c: [f32; 4],
}

impl Harmonics {
    /// An even light from everywhere (`AddAmbientLight`): added to each
    /// channel's constant as it is.
    pub fn add_ambient(&mut self, color: [f32; 3]) {
        for (channel, value) in self.0.iter_mut().zip(color) {
            channel[0] += value;
        }
    }

    /// A light from one way (`AddDirectionalLight`): its colour times twice
    /// the intensity (the engine's managed side), times 16π/17, times each
    /// coefficient's basis at the light's direction and normalisation.
    pub fn add_directional(&mut self, direction: [f32; 3], color: [f32; 3], intensity: f32) {
        let [x, y, z] = direction;
        let coefficients = [
            1.0 * BASIS[0] * NORMALISATION[0],
            -y * BASIS[1] * NORMALISATION[1],
            z * BASIS[1] * NORMALISATION[2],
            -x * BASIS[1] * NORMALISATION[3],
            x * y * BASIS[2] * NORMALISATION[4],
            z * -y * BASIS[2] * NORMALISATION[5],
            (z * z - 1.0 / 3.0) * BASIS[3] * NORMALISATION[6],
            -x * z * BASIS[2] * NORMALISATION[7],
            (x * x - y * y) * BASIS[4] * NORMALISATION[8],
        ];
        for (channel, value) in self.0.iter_mut().zip(color) {
            let scaled = DIRECTIONAL_SCALE * (value * (2.0 * intensity));
            for (coefficient, basis) in channel.iter_mut().zip(coefficients) {
                *coefficient += scaled * basis;
            }
        }
    }

    pub fn scaled(self, factor: f32) -> Self {
        Self(self.0.map(|channel| channel.map(|c| c * factor)))
    }

    pub fn plus(self, other: Self) -> Self {
        Self(std::array::from_fn(|i| std::array::from_fn(|k| self.0[i][k] + other.0[i][k])))
    }

    /// As the shaders take them (`SetSH`).
    pub fn packed(&self) -> Packed {
        let sh = &self.0;
        Packed {
            a: std::array::from_fn(|i| [sh[i][3], sh[i][1], sh[i][2], sh[i][0] - sh[i][6]]),
            b: std::array::from_fn(|i| [sh[i][4], sh[i][5], sh[i][6] * 3.0, sh[i][7]]),
            c: [sh[0][8], sh[1][8], sh[2][8], 1.0],
        }
    }

    /// The light from straight above (`GetColorTop`), which the shaders
    /// take as `_EFT_Ambient`: in gamma space as it is.
    pub fn top(&self) -> [f32; 3] {
        std::array::from_fn(|i| self.0[i][1] + (self.0[i][0] - self.0[i][6]) + -self.0[i][8])
    }

    /// The light a surface facing that way takes (`GetColor`, the shaders'
    /// sums over the packing), in the engine's axes.
    pub fn at(&self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        let p = self.packed();
        let dot = |a: [f32; 4], b: [f32; 4]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
        std::array::from_fn(|i| dot(p.a[i], [x, y, z, 1.0]) + dot(p.b[i], [x * y, y * z, z * z, z * x]) + p.c[i] * (x * x - y * y))
    }
}

/// `Mathf.LerpUnclamped`.
fn lerp_unclamped(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `Mathf.Lerp`.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// `Mathf.InverseLerp`.
fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    if a != b {
        ((v - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A colour taken toward a grey of `around` by `t` (`ContrastColor`; the
/// sky's own takes 0.5).
fn contrast(color: [f32; 3], around: f32, t: f32) -> [f32; 3] {
    color.map(|c| lerp_unclamped(around, c, t))
}

/// A colour taken from its own grey by `t` (`SaturateColor`).
fn saturate(color: [f32; 3], t: f32) -> [f32; 3] {
    let grey = (color[0] + color[1] + color[2]) * (1.0 / 3.0);
    color.map(|c| lerp_unclamped(grey, c, t))
}

/// The simple sky's harmonics with its sun's light going `forward` (the
/// light's transform's forward, the engine's axes): `TODSkySimple.Update`
/// and `CalculateSphericalHarmonics`. The sun's height `t` is the way to the
/// sun's up part (-1 to 1), `t01` the same from 0 to 1.
pub fn sky_simple(sky: &SkySimple, forward: [f32; 3]) -> Harmonics {
    let t = -forward[1];
    let t01 = t * 0.5 + 0.5;
    let rgb = |c: [f32; 4]| [c[0], c[1], c[2]];
    let light_source = rgb(sky.light_color.evaluate(t01));
    let (contrast_t, saturation_t, brightness) = (sky.contrast.evaluate(t), sky.saturation.evaluate(t), sky.brightness.evaluate(t));
    let light = lerp(0.003, 0.25, inverse_lerp(0.07, 1.0, t));
    // Each colour as the sky takes it: its contrast, a no-op toward the
    // sun's light, its saturation, its brightness, nothing below nought.
    let graded = |c: [f32; 3]| saturate(contrast(contrast(c, 0.5, contrast_t), light, 1.0), saturation_t).map(|v| (v * brightness).max(0.0));

    let mut without_top = Harmonics::default();
    // The sun's light bounced from the ground: from halfway between the way
    // it goes and straight down.
    let down = [forward[0], forward[1] - 1.0, forward[2]];
    let length = (down[0] * down[0] + down[1] * down[1] + down[2] * down[2]).sqrt();
    let bounce = if length > 1e-5 { down.map(|c| c / length) } else { [0.0; 3] };
    without_top.add_directional(bounce, graded(light_source), sky.bounce.evaluate(t));
    let ambient = rgb(sky.add_ambient.evaluate(t01));
    let clear = t.clamp(0.0, 1.0) * 0.333;
    without_top.add_ambient(graded(std::array::from_fn(|i| ambient[i] + sky.cloudness[i] * clear)));
    let mut full = Harmonics::default().plus(without_top);
    full.add_directional([0.0, 1.0, 0.0], graded(rgb(sky.add_top_ambient.evaluate(t01))), 1.0);
    // Taken toward a dim grey by the daylight figure.
    let mut environment = Harmonics::default();
    environment.add_ambient([0.013; 3]);
    full.scaled(1.0 - sky.harmonics_daylight).plus(environment.scaled(sky.harmonics_daylight))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_even_light_is_the_same_every_way() {
        let mut sh = Harmonics::default();
        sh.add_ambient([0.2, 0.3, 0.4]);
        for normal in [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]] {
            assert_eq!(sh.at(normal), [0.2, 0.3, 0.4]);
        }
        assert_eq!(sh.top(), [0.2, 0.3, 0.4]);
    }

    #[test]
    fn a_light_from_above_is_brightest_facing_it() {
        let mut sh = Harmonics::default();
        sh.add_directional([0.0, 1.0, 0.0], [1.0; 3], 1.0);
        let (up, side, down) = (sh.at([0.0, 1.0, 0.0])[0], sh.at([1.0, 0.0, 0.0])[0], sh.at([0.0, -1.0, 0.0])[0]);
        assert!(up > side && side > down, "{up} {side} {down}");
        assert!((sh.top()[0] - up).abs() < 1e-6);
    }
}

/// The ambient highlight's multiplier (`AmbientHighlight.UpdateAmbientBuffer`):
/// between its least and most by its curve at the sun's height (the light's
/// way's height turned round), held between them (`Mathf.Lerp`).
pub fn highlight_multiplier(settings: &trp_data::tod::HighlightSettings, light_forward_y: f32) -> f32 {
    let t = settings.intensity.evaluate(-light_forward_y).clamp(0.0, 1.0);
    settings.min_multiplier + (settings.max_multiplier - settings.min_multiplier) * t
}
