//! The tactical lights' lens flares, and the eyes' protection that dims them.
//! After the tactical-devices spec, "The light shaft and the flare" (the
//! game's `MultiFlare`: `FlareLight`, `Flare`,
//! `SingleFlareController`, `PrepareGpuData`, `ManageFlareOverlap`,
//! `FlareBlindnessController`, `BatchSettings`): each light's alpha by how
//! squarely it faces the camera, each flare's figures for the shader, the
//! flashlights' shrink where they crowd, and the eyes' protection that dims
//! and shrinks them all. The host draws them with the shaders' sums
//! (`Custom/multiFlare`, `Custom/ShitOnScreen`).

use trp_data::weapons::tactical::{Flare, FlareController};

/// The atlas's pictures (`ProFlareAtlas` "FlareAtlas", `sharedassets3.assets`
/// #20): each one's corner and size in the picture, v up.
pub const ATLAS: [[f32; 4]; 10] = [
    [0.0, 0.5, 0.5, 0.5],
    [0.5, 0.5, 0.5, 0.5],
    [0.0, 0.0, 0.5, 0.5],
    [0.5, 0.25, 0.25, 0.25],
    [0.75, 0.25, 0.25, 0.25],
    [0.5, 0.1875, 0.25, 0.065],
    [0.5, 0.157, 0.25, 0.03],
    [0.5, 0.0825, 0.25, 0.075],
    [0.75, 0.0, 0.25, 0.25],
    [0.5, 0.007, 0.25, 0.07],
];

/// A flare's kind (`FlareType`).
pub const NORMAL: i32 = 0;
pub const OFF_SCREEN: i32 = 1;
pub const NOISE: i32 = 2;
pub const FLASHLIGHT: i32 = 3;

/// The two shaders' figures, as the level's materials set them (`MultiFlare`
/// and `FlashLightFlares` alike, `Custom/multiFlare`; `ShitOnScreen`,
/// `Custom/ShitOnScreen`): how far toward the camera the point is moved, the
/// screen edge's fade (`_FallofShift`, `_FallofStrength`), the depth taps'
/// spread (`_VisibilityCheckerSize`) and the brightness (`_Intensity`); the
/// noise flare's also its silhouette's point (`_FadeDepthOffset`), its
/// grain's strength (`_NoiseIntensity`) and scale (`_NoiseScale`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShaderFigures {
    pub depth_offset: f32,
    pub fade_depth_offset: f32,
    pub falloff_shift: f32,
    pub falloff_strength: f32,
    pub checker_size: f32,
    pub intensity: f32,
    pub noise_intensity: f32,
    pub noise_scale: [f32; 2],
}

pub const MULTI_FLARE: ShaderFigures = ShaderFigures { depth_offset: 0.1, fade_depth_offset: 0.3, falloff_shift: 1.0, falloff_strength: 18.0, checker_size: 0.003, intensity: 0.6, noise_intensity: 0.0, noise_scale: [1.0, 1.0] };
pub const SHIT_ON_SCREEN: ShaderFigures = ShaderFigures { depth_offset: 1.0, fade_depth_offset: 0.4, falloff_shift: 1.0, falloff_strength: 16.0, checker_size: 0.01, intensity: 1.0, noise_intensity: 5.0, noise_scale: [2.0, 2.0] };

/// The flashlights' crowding (`FlareOverlapSettings` of the level's
/// flashlight batch): at most so many neighbours counted, within so far,
/// and the size at that most.
pub const OVERLAP_NEIGHBOURS: f32 = 5.0;
pub const OVERLAP_RANGE: f32 = 0.5;
pub const OVERLAP_MOST_SHRINK: f32 = 0.25;

/// The protection's wait before it moves, and how fast it moves after
/// (`FlareBlindnessController.Execute`).
pub const PROTECTION_WAIT: f32 = 0.6;
pub const PROTECTION_SPEED: f32 = 4.0;

/// `Mathf.MoveTowards`.
fn move_towards(current: f32, target: f32, step: f32) -> f32 {
    if (target - current).abs() <= step {
        target
    } else {
        current + (target - current).signum() * step
    }
}

/// A light's alpha a frame on (`SingleFlareController.Update`): toward how
/// squarely the light's object faces the camera, `(k − min angle) / (1 −
/// min angle)` for `k` the cosine between its forward and the way to the
/// camera, not held at nothing, at its speed a second. So behind a light
/// it sinks below nothing and climbs back before a flare shows. A light
/// with no controller keeps its alpha.
pub fn step_alpha(alpha: f32, controller: Option<FlareController>, at: [f32; 3], forward: [f32; 3], camera: [f32; 3], dt: f32) -> f32 {
    let Some(c) = controller else { return alpha };
    let to = [camera[0] - at[0], camera[1] - at[1], camera[2] - at[2]];
    let length = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
    // `Vector3.Normalize` of a way too short is nought.
    let k = if length > 1e-5 { (to[0] * forward[0] + to[1] * forward[1] + to[2] * forward[2]) / length } else { 0.0 };
    let target = (k - c.min_angle) / (1.0 - c.min_angle);
    move_towards(alpha, target, dt * c.change_speed)
}

/// What one flare hands the shader (`FlareGpuData`, `PrepareGpuData`): its
/// light's place, its picture, its size, its colour (held within 0 and 1,
/// its fourth the light's alpha itself), and its distance curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlareGpu {
    pub position: [f32; 3],
    pub uv: [f32; 4],
    pub scale: [f32; 2],
    pub color: [f32; 4],
    pub min_dist: f32,
    pub dist_inverse: f32,
    pub scale_range: f32,
    pub alpha_range: f32,
    pub kind: i32,
}

/// One flare's figures (`PrepareGpuData.Execute`): `light_alpha` and
/// `light_scale` its light's (`_totalAlpha`, `_totalScale`), `alpha_mul` and
/// `size_mul` the batch's (the eyes' protection). A picture's number past
/// the atlas takes none.
pub fn gpu_data(flare: &Flare, light_alpha: f32, light_scale: f32, position: [f32; 3], alpha_mul: f32, size_mul: f32) -> FlareGpu {
    let num = light_alpha * flare.min_alpha * flare.alpha * alpha_mul;
    let uv = usize::try_from(flare.tex_id).ok().and_then(|i| ATLAS.get(i)).copied().unwrap_or([0.0; 4]);
    let size = flare.min_scale * light_scale * size_mul;
    FlareGpu {
        position,
        uv,
        scale: [flare.scale[0] * size, flare.scale[1] * size],
        color: [(flare.color[0] * num).clamp(0.0, 1.0), (flare.color[1] * num).clamp(0.0, 1.0), (flare.color[2] * num).clamp(0.0, 1.0), light_alpha],
        min_dist: flare.min_dist,
        dist_inverse: 1.0 / (flare.max_dist - flare.min_dist),
        scale_range: (flare.max_scale - flare.min_scale) / flare.min_scale,
        alpha_range: (flare.max_alpha - flare.min_alpha) / flare.min_alpha,
        kind: flare.kind,
    }
}

/// The flashlights' crowding (`ManageFlareOverlap`, the flashlight batch's,
/// skipped under night vision): each flare's size times `lerp(1, 0.25, n /
/// 5)`, `n` the sum over the others within 0.5 m of one less their distance
/// over 0.5, at most 5. A light's three flashlight flares at one point
/// count two each: 0.7 of their size.
pub fn overlap(entries: &mut [FlareGpu]) {
    let places: Vec<[f32; 3]> = entries.iter().map(|e| e.position).collect();
    for (i, entry) in entries.iter_mut().enumerate() {
        let mut n = 0.0;
        for (j, p) in places.iter().enumerate() {
            if i == j {
                continue;
            }
            let d = ((p[0] - places[i][0]).powi(2) + (p[1] - places[i][1]).powi(2) + (p[2] - places[i][2]).powi(2)).sqrt();
            if d <= OVERLAP_RANGE {
                n += 1.0 - d / OVERLAP_RANGE;
            }
        }
        let n = f32::min(n, OVERLAP_NEIGHBOURS);
        let shrink = 1.0 + (OVERLAP_MOST_SHRINK - 1.0) * (n / OVERLAP_NEIGHBOURS);
        entry.scale = [entry.scale[0] * shrink, entry.scale[1] * shrink];
    }
}

/// The eyes' protection as the flares take it (`FlareBlindnessController`,
/// driven by `Player.OnBlindnessProtectionChanged`): as the level starts,
/// what is worn then; at a change, 0.6 s later, moving to the new figure at
/// 4 a second, a change before that beginning the wait again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Protection {
    current: f32,
    desired: f32,
    /// The wait left before it moves; none while it moves or rests.
    wait: Option<f32>,
    /// The last figure told, to tell a change (`CheckBlindnessProtection`).
    told: f32,
}

impl Protection {
    pub fn new(worn: f32) -> Self {
        Self { current: worn, desired: worn, wait: None, told: worn }
    }

    /// What is worn now, told after the frame's step: a change starts the
    /// wait anew, counted from the next frame (`WaitForSeconds` from the
    /// change's frame's time).
    pub fn tell(&mut self, worn: f32) {
        if (worn - self.told).abs() > 1e-6 {
            self.told = worn;
            self.desired = worn;
            self.wait = Some(PROTECTION_WAIT);
        }
    }

    /// A frame on; the figure the batches take, held within 0 and 1. On
    /// the frame the wait runs out it moves already (the coroutine goes on
    /// into its loop on that frame).
    pub fn step(&mut self, dt: f32) -> f32 {
        if let Some(left) = self.wait {
            let left = left - dt;
            if left > 0.0 {
                self.wait = Some(left);
                return self.value();
            }
            self.wait = None;
        }
        if self.current != self.desired {
            self.current = move_towards(self.current, self.desired, dt * PROTECTION_SPEED);
        }
        self.value()
    }

    pub fn value(&self) -> f32 {
        self.current.clamp(0.0, 1.0)
    }

    /// The batches' alpha and size factors (`SetBlindnessProtectionFactor`,
    /// the level's factors 1): one less the protection.
    pub fn multipliers(&self) -> (f32, f32) {
        (1.0 - self.value(), 1.0 - self.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white_light() -> Vec<Flare> {
        let flare = |scale: f32, alpha: f32, kind: i32, max_dist: f32, max_alpha: f32, tex_id: i32| Flare {
            scale: [scale, scale],
            alpha,
            color: [1.0, 1.0, 1.0, 1.0],
            kind,
            min_dist: 0.1,
            max_dist,
            min_scale: 1.0,
            max_scale: 0.1,
            min_alpha: 1.0,
            max_alpha,
            tex_id,
        };
        // The Zenit Klesch-2U's (`probe devices 5b3a337e5acfc4704b4a19a0`).
        vec![flare(2.0, 1.0, FLASHLIGHT, 30.0, 0.2, 0), flare(1.0, 0.2, FLASHLIGHT, 30.0, 0.2, 4), flare(1.0, 1.0, FLASHLIGHT, 30.0, 0.2, 1), flare(2.0, 0.3, NOISE, 15.0, 0.0, 1)]
    }

    const WHITE: FlareController = FlareController { change_speed: 2.0, min_angle: 0.6 };

    /// Faced squarely the alpha climbs to 1 in half a second; from behind
    /// it sinks to −4 and takes two seconds back to nothing.
    #[test]
    fn a_light_s_alpha_goes_by_its_facing() {
        let camera = [0.0, 0.0, 10.0];
        let mut alpha = 0.0;
        for _ in 0..40 {
            alpha = step_alpha(alpha, Some(WHITE), [0.0; 3], [0.0, 0.0, 1.0], camera, 1.0 / 60.0);
        }
        assert!((alpha - 1.0).abs() < 1e-5, "{alpha}");
        for _ in 0..300 {
            alpha = step_alpha(alpha, Some(WHITE), [0.0; 3], [0.0, 0.0, -1.0], camera, 1.0 / 60.0);
        }
        assert!((alpha + 4.0).abs() < 1e-4, "not held at nothing: {alpha}");
        let mut frames = 0;
        while alpha < 0.0 {
            alpha = step_alpha(alpha, Some(WHITE), [0.0; 3], [0.0, 0.0, 1.0], camera, 1.0 / 60.0);
            frames += 1;
        }
        assert!((120..=121).contains(&frames), "two seconds back to nothing: {frames} frames");
        // At 60 degrees off its axis, below its least angle: it fades.
        let side = [(60f32).to_radians().sin(), 0.0, (60f32).to_radians().cos()];
        let target = step_alpha(0.5, Some(WHITE), [0.0; 3], side, camera, 10.0);
        assert!((target - (0.5 - 0.6) / 0.4).abs() < 1e-5, "{target}");
        assert_eq!(step_alpha(0.3, None, [0.0; 3], [0.0, 0.0, 1.0], camera, 1.0), 0.3);
    }

    #[test]
    fn a_flare_s_figures_are_the_game_s() {
        let flares = white_light();
        let g = gpu_data(&flares[1], 0.5, 1.5, [1.0, 2.0, 3.0], 0.8, 0.9);
        // Its colour: the flare's times the light's alpha, its own and the
        // batch's; its fourth the light's alpha alone.
        let num = 0.5 * 0.2 * 0.8;
        assert!((g.color[0] - num).abs() < 1e-6 && (g.color[3] - 0.5).abs() < 1e-6, "{:?}", g.color);
        assert_eq!(g.uv, [0.75, 0.25, 0.25, 0.25]);
        assert!((g.scale[0] - 1.5 * 0.9).abs() < 1e-6);
        assert!((g.dist_inverse - 1.0 / 29.9).abs() < 1e-6 && (g.scale_range + 0.9).abs() < 1e-6 && (g.alpha_range + 0.8).abs() < 1e-6);
        // A light's alpha below nothing gives no colour.
        assert_eq!(gpu_data(&flares[0], -2.0, 1.5, [0.0; 3], 1.0, 1.0).color[..3], [0.0; 3]);
    }

    /// A light's three flashlight flares at one point draw at 0.7 of their
    /// size; a second light 0.25 m off shrinks them further.
    #[test]
    fn crowded_flashlights_shrink() {
        let flares = white_light();
        let mut one: Vec<FlareGpu> = flares.iter().filter(|f| f.kind == FLASHLIGHT).map(|f| gpu_data(f, 1.0, 1.0, [0.0; 3], 1.0, 1.0)).collect();
        overlap(&mut one);
        assert!((one[0].scale[0] - 2.0 * 0.7).abs() < 1e-6, "{:?}", one[0].scale);
        let mut two: Vec<FlareGpu> = [[0.0f32; 3], [0.25, 0.0, 0.0]].iter().flat_map(|&at| flares.iter().filter(|f| f.kind == FLASHLIGHT).map(move |f| gpu_data(f, 1.0, 1.0, at, 1.0, 1.0))).collect();
        overlap(&mut two);
        // Two at one point and three at half the range: 2 + 1.5.
        assert!((two[0].scale[0] - 2.0 * (1.0 - 0.75 * 3.5 / 5.0)).abs() < 1e-6, "{:?}", two[0].scale);
    }

    /// Glasses put on: nothing for 0.6 s, then at 4 a second; taken off again
    /// before it moved, the wait begins anew.
    #[test]
    fn the_eyes_protection_waits_then_moves() {
        let mut p = Protection::new(0.0);
        assert_eq!(p.step(0.1), 0.0);
        p.tell(1.0);
        assert_eq!(p.step(0.5), 0.0, "waiting");
        assert_eq!(p.step(0.125), 0.5, "moving from the frame its wait ends, at 4 a second");
        assert_eq!(p.step(0.125), 1.0);
        assert_eq!(p.multipliers(), (0.0, 0.0));
        // A change before the wait is out begins it anew.
        p.tell(0.0);
        p.step(0.5);
        p.tell(0.5);
        assert_eq!(p.step(0.5), 1.0, "still waiting");
        assert_eq!(p.step(0.125), 0.5);
        // At the level's start, what is worn holds at once.
        assert_eq!(Protection::new(0.5).value(), 0.5);
    }
}
