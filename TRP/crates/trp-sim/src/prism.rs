//! The camera's PRISM effect (`docs/spec/lighting.md`, "PRISM: the exposure,
//! the vignette and the tonemap"): the environment manager's middle grey
//! and speed following their targets, the adaptation's share of a frame,
//! and the main pass's sums for one colour. The app runs the per-pixel part
//! on the graphics card (`prism_view`); the same sums are here to be read
//! and tested.

use crate::math::smooth_damp;

/// The exposure's middle grey and speed as the environment manager hands
/// them to PRISM on every frame (`EnvironmentManager.Update`): each follows
/// its target by `Mathf.SmoothDamp` over the fade time, from 0 when the
/// manager is made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvironmentExposure {
    pub middle_grey: f32,
    pub speed: f32,
    grey_velocity: f32,
    speed_velocity: f32,
    pub target_grey: f32,
    pub target_speed: f32,
    pub fade_time: f32,
    /// The manager's outdoor figures, gone back to on leaving a trigger.
    outdoor: IndoorFigures,
    /// Whether the player is in an indoor trigger.
    pub indoors: bool,
}

/// An indoor trigger's figures (`IndoorTrigger`; the manager's outdoor ones
/// are of the same kind): the fade time, the exposure's speed and its
/// middle grey (`ExposureOffset`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndoorFigures {
    pub fade_time: f32,
    pub exposure_speed: f32,
    pub exposure_offset: f32,
}

impl EnvironmentExposure {
    /// Outside every indoor trigger: the manager's outdoor figures.
    pub fn outdoor(offset: f32, speed: f32, fade_time: f32) -> Self {
        let outdoor = IndoorFigures { fade_time, exposure_speed: speed, exposure_offset: offset };
        Self { middle_grey: 0.0, speed: 0.0, grey_velocity: 0.0, speed_velocity: 0.0, target_grey: offset, target_speed: speed, fade_time, outdoor, indoors: false }
    }

    /// The player's indoor trigger, or none outdoors
    /// (`EnvironmentManager.SetTriggerForPlayer`): where the environment
    /// changes, the targets and the fade time become the trigger's (or the
    /// outdoor ones) and both velocities nought; from one trigger into
    /// another nothing changes (no bunkers here).
    pub fn set_trigger(&mut self, trigger: Option<IndoorFigures>) {
        if self.indoors == trigger.is_some() {
            return;
        }
        self.indoors = trigger.is_some();
        let to = trigger.unwrap_or(self.outdoor);
        self.fade_time = to.fade_time;
        self.target_speed = to.exposure_speed;
        self.target_grey = to.exposure_offset;
        self.grey_velocity = 0.0;
        self.speed_velocity = 0.0;
    }

    /// One frame of `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let mut v = [self.grey_velocity];
        self.middle_grey = smooth_damp([self.middle_grey], [self.target_grey], &mut v, self.fade_time, f32::INFINITY, dt)[0];
        self.grey_velocity = v[0];
        let mut v = [self.speed_velocity];
        self.speed = smooth_damp([self.speed], [self.target_speed], &mut v, self.fade_time, f32::INFINITY, dt)[0];
        self.speed_velocity = v[0];
    }
}

#[cfg(test)]
mod indoor_tests {
    use super::*;

    /// Indoors the targets are the trigger's; out again, the outdoor ones;
    /// the middle grey follows at the fade's pace either way.
    #[test]
    fn a_trigger_hands_the_exposure_its_figures() {
        let mut e = EnvironmentExposure::outdoor(0.23, 2.0, 0.25);
        for _ in 0..120 {
            e.step(1.0 / 60.0);
        }
        assert!((e.middle_grey - 0.23).abs() < 1e-3);
        e.set_trigger(Some(IndoorFigures { fade_time: 1.0, exposure_speed: 4.0, exposure_offset: 0.14 }));
        assert_eq!((e.target_grey, e.target_speed, e.fade_time), (0.14, 4.0, 1.0));
        e.step(1.0 / 60.0);
        assert!(e.middle_grey < 0.23 && e.middle_grey > 0.2, "{}", e.middle_grey);
        for _ in 0..600 {
            e.step(1.0 / 60.0);
        }
        assert!((e.middle_grey - 0.14).abs() < 1e-3 && (e.speed - 4.0).abs() < 1e-2);
        // Another trigger changes nothing; outdoors again, the outdoor ones.
        e.set_trigger(Some(IndoorFigures { fade_time: 2.0, exposure_speed: 1.0, exposure_offset: 0.5 }));
        assert_eq!(e.target_grey, 0.14);
        e.set_trigger(None);
        assert_eq!((e.target_grey, e.target_speed, e.fade_time), (0.23, 2.0, 0.25));
    }
}

/// The share of the way the kept brightness goes to the frame's on each
/// frame (the secondary shader's pass 4: its alpha, blended over).
pub fn adaptation_share(speed: f32) -> f32 {
    (speed * 0.0125).clamp(0.0, 1.0)
}

/// A colour's brightness as the adaptation weighs it (pass 3).
pub fn brightness(rgb: [f32; 3]) -> f32 {
    let [r, g, b] = rgb.map(|c| c.clamp(0.0, 65472.0));
    r * 0.22 + g * 0.707 + b * 0.071
}

/// The exposure's multiplier for a kept brightness: the middle grey over
/// it, held to at most `upper`, then to at least `lower`.
pub fn exposure(middle_grey: f32, brightness: f32, lower: f32, upper: f32) -> f32 {
    (middle_grey / brightness).min(upper).max(lower)
}

/// The vignette's share at a place on the screen (`uv` in its own shares,
/// 0 to 1 each way): the colour is multiplied by `lerp(vignette colour, 1,
/// share)`.
pub fn vignette(uv: [f32; 2], start: f32, end: f32, strength: f32) -> f32 {
    let d = ((uv[0] - 0.5).powi(2) + (uv[1] - 0.5).powi(2)).sqrt();
    let t = ((d * strength - start) * (1.0 / (end - start))).clamp(0.0, 1.0);
    t * t * (t * -2.0 + 3.0)
}

/// The filmic curve for one channel (`PRISM_FILMIC_TONEMAP`), its result
/// squared.
pub fn filmic(c: f32, tone: [f32; 3], secondary: [f32; 3]) -> f32 {
    let x = (c - secondary[0]).max(0.0);
    let curve = x * (x * tone[0] + tone[1]) / (x * (x * tone[0] + tone[2]) + secondary[1]);
    curve * curve
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Factory's camera's curve.
    const TONE: [f32; 3] = [7.0, 0.3, 0.3];
    const SECONDARY: [f32; 3] = [-0.05, 0.25, 0.0];

    #[test]
    fn the_filmic_curve_lifts_black_and_rolls_off_white() {
        assert!((filmic(0.0, TONE, SECONDARY) - 0.0132).abs() < 1e-3);
        assert!((filmic(0.5, TONE, SECONDARY) - 0.812).abs() < 1e-3);
        assert!((filmic(1.0, TONE, SECONDARY) - 0.9405).abs() < 1e-3);
        assert!(filmic(100.0, TONE, SECONDARY) < 1.0);
    }

    #[test]
    fn the_vignette_leaves_the_middle_and_darkens_the_corners() {
        assert_eq!(vignette([0.5, 0.5], 0.9, 0.4, 0.887), 1.0);
        assert_eq!(vignette([0.5, 0.1], 0.9, 0.4, 0.887), 1.0);
        assert!((vignette([0.0, 0.0], 0.9, 0.4, 0.887) - 0.568).abs() < 1e-3);
    }

    #[test]
    fn the_exposure_never_brightens_past_its_upper_limit() {
        assert_eq!(exposure(0.23, 0.1, -10.0, 0.9), 0.9);
        assert!((exposure(0.23, 0.46, -10.0, 0.9) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn the_environment_reaches_its_outdoor_figures_within_a_second() {
        let mut env = EnvironmentExposure::outdoor(0.23, 2.0, 0.25);
        for _ in 0..60 {
            env.step(1.0 / 60.0);
        }
        assert!((env.middle_grey - 0.23).abs() < 1e-3 && (env.speed - 2.0).abs() < 1e-2, "{env:?}");
        assert_eq!(adaptation_share(env.target_speed), 0.025);
    }
}
