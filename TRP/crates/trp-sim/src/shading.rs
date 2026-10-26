//! The game's surface light, as its deferred passes work it out
//! (`docs/spec/lighting.md`, "Tarkov's surface shading"): one light on a
//! surface by the engine's standard light in its gamma-space form, the
//! engine's fall-off of a point or spot light with distance, and the
//! screen's ambient pass's brightness term. Every figure is the game's own
//! number (its project is in gamma space): the surface's stored colours,
//! the light's colour times its intensity. `trp-app`'s `game_shading.wgsl`
//! works the same per pixel.

/// A surface as the game's G-buffer holds it: its colour, its specular
/// colour and its smoothness, each from nought to one.
#[derive(Debug, Clone, Copy)]
pub struct Surface {
    pub color: [f32; 3],
    pub specular: [f32; 3],
    pub smoothness: f32,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn pow5(x: f32) -> f32 {
    let x2 = x * x;
    x2 * x2 * x
}

/// One light on a surface (`Hidden/Internal-DeferredShadingEFT`, every
/// variant alike): `n` its normal, `v` the way to the eye, `l` the way to
/// the light, each of length one; `light` the light's colour times its
/// intensity times its attenuation, cookie and shadow there. Disney's
/// diffuse without its 1/π, GGX's distribution with the joint Smith
/// visibility, the specular's square root (the engine's gamma-space form),
/// Schlick's Fresnel on the specular colour; no specular where that colour
/// is black. The colour is held between nought and one, as the pass holds
/// it.
pub fn standard_light(surface: &Surface, n: [f32; 3], v: [f32; 3], l: [f32; 3], light: [f32; 3]) -> [f32; 3] {
    let sum = [l[0] + v[0], l[1] + v[1], l[2] + v[2]];
    let length = dot(sum, sum).max(0.001).sqrt();
    let h = sum.map(|c| c / length);
    let nl = dot(n, l).clamp(0.0, 1.0);
    let nh = dot(n, h).clamp(0.0, 1.0);
    let lh = dot(l, h).clamp(0.0, 1.0);
    let nv = dot(n, v).abs();
    let perceptual = 1.0 - surface.smoothness;
    let a = (perceptual * perceptual).max(0.002);
    let a2 = a * a;
    // Disney's diffuse: its grazing figure less one.
    let grazing = 2.0 * lh * lh * perceptual - 0.5;
    let diffuse = (1.0 + grazing * pow5(1.0 - nl)) * (1.0 + grazing * pow5(1.0 - nv)) * nl;
    let visibility = 0.5 / (nl * (nv * (1.0 - a) + a) + nv * (nl * (1.0 - a) + a) + 1e-5);
    let d = (nh * a2 - nh) * nh + 1.0;
    let distribution = a2 * std::f32::consts::FRAC_1_PI / (d * d + 1e-7);
    let black = surface.specular.iter().all(|&s| s == 0.0);
    let specular = if black { 0.0 } else { (visibility * distribution * std::f32::consts::PI).max(1e-4).sqrt() * nl };
    let fresnel = pow5(1.0 - lh);
    std::array::from_fn(|i| {
        let color = surface.color[i].clamp(0.0, 1.0);
        let s = surface.specular[i];
        color * light[i] * diffuse + specular * light[i] * (s + (1.0 - s) * fresnel)
    })
}

/// The engine's attenuation of a point or spot light at a share of its
/// range squared (`BuildAttenuationTexture`'s texture, `LightAttenuation`
/// read with Ghidra): one over one plus 25 times it, faded to nought over
/// the range's last part from 0.64 (the share 0.8), nought from one.
pub fn attenuation(share_squared: f32) -> f32 {
    const FADE: f32 = 0.64;
    if share_squared >= 1.0 {
        return 0.0;
    }
    let near = 1.0 / (1.0 + 25.0 * share_squared.max(0.0));
    if share_squared < FADE {
        near
    } else {
        near * (1.0 - (share_squared - FADE) / (1.0 - FADE))
    }
}

/// The screen's ambient pass's brightness term (`Hidden/WriteScreenAmbient`):
/// what a surface's colour's brightness squared is multiplied by and added,
/// indoors and out, by the sky's fogginess (`TOD_Fogginess`), night vision
/// (`_NightVisionOn`) and the sun's height (`_SunDirection`'s, either way).
pub fn ambient_brightness(fogginess: f32, night_vision: f32, sun_height: f32) -> f32 {
    0.05 * (1.0 + (1.5 * fogginess).max(night_vision)) * (1.0 - sun_height.abs())
}

/// The brightness the term takes (the pass's weights).
pub fn brightness(color: [f32; 3]) -> f32 {
    dot(color, [0.2126, 0.7152, 0.0722])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_attenuation_falls_as_the_engines_texture() {
        assert_eq!(attenuation(0.0), 1.0);
        // A fifth of the range: halved.
        assert!((attenuation(0.04) - 0.5).abs() < 1e-6);
        assert!((attenuation(0.5) - 1.0 / 13.5).abs() < 1e-6);
        // Into the fade: at 0.82 of the range squared half the fade's way.
        assert!((attenuation(0.82) - 0.5 / (1.0 + 25.0 * 0.82)).abs() < 1e-6);
        assert_eq!(attenuation(1.0), 0.0);
        assert!(attenuation(0.999) < 2e-4);
    }

    #[test]
    fn a_white_surface_square_to_a_light_takes_it_whole() {
        // Facing the light and the eye: Disney's diffuse is one there.
        let surface = Surface { color: [1.0; 3], specular: [0.0; 3], smoothness: 0.5 };
        let lit = standard_light(&surface, [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.7, 0.6, 0.4]);
        for (got, want) in lit.iter().zip([0.7, 0.6, 0.4]) {
            assert!((got - want).abs() < 1e-5, "{lit:?}");
        }
    }

    #[test]
    fn a_grey_facing_up_in_customs_morning() {
        // 09:37's sun (its colour times intensity, its way 31.7 degrees up)
        // on a grey of 0.5 facing up, seen from above at a slant, with no
        // specular: the light's term alone.
        let up = 31.7f32.to_radians();
        let l = [up.cos(), up.sin(), 0.0];
        let v = [0.0, 0.6f32.sqrt(), 0.4f32.sqrt()];
        let surface = Surface { color: [0.5; 3], specular: [0.0; 3], smoothness: 0.05 };
        let lit = standard_light(&surface, [0.0, 1.0, 0.0], v, l, [0.728, 0.63, 0.40]);
        // Diffuse about the colour times the light times the slant.
        assert!((lit[0] - 0.5 * 0.728 * up.sin()).abs() < 0.03, "{lit:?}");
    }

    #[test]
    fn the_brightness_term_at_customs_morning() {
        // Recorded `_SunDirection` (0.845, -0.525, 0.098), no fog, no
        // night vision: a mid-grey gains under a hundredth.
        let k = ambient_brightness(0.0, 0.0, -0.525_056_8);
        assert!((k - 0.05 * 0.474_943_2).abs() < 1e-6);
        let b = brightness([0.5; 3]);
        assert!((b * b * k - 0.005_937).abs() < 1e-5);
    }
}
