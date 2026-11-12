//! The sun's and moon's places and the sky's state at a moment.

use super::*;

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
