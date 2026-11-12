//! Scattering colours, atmosphere samples, the controller update and the harmonics.

use super::*;

/// `TOD_Util.MulRGB`, `ContrastColor`, `ContrastColor2`, `SaturateColor`,
/// `LimitColor` (the controller's).
pub(super) fn contrast(c: Rgb, t: f32) -> Rgb {
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
    pub(super) fn harmonics(&self, state: &State, t: f32, t01: f32, cloudiness: f32) -> (Harmonics, Harmonics, Rgb) {
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
