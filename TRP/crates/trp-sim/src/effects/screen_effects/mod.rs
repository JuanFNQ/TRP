//! What health's effects do to the screen, frame by frame.
//! After the health spec, "A contusion's screen effects" and "A hit and
//! health's other screen effects", and the grenade spec, "A flash"
//! (`EffectsController` and its accumulators): the double
//! vision's amount, the wiggle's timer, scale and strength, the motion
//! blur's amount, the flash's white, pain's blur, exhaustion's vignette,
//! near death's blend and a hit's blur, frame by frame, from the effects
//! listed on the character with their current strengths and the hits taken.
//! The host draws them.

use trp_data::drawing::camera_effects::HealthScreenData;
use trp_data::formats::curve::{Curve, Key};

use crate::condition::health::{DamageKind, Part};

mod hits;
mod desaturation;
mod frame;
mod eye_burn;

pub use hits::*;
pub use desaturation::*;
pub use eye_burn::*;
use hits::{HitBlur, Listing, inverse_lerp, looped};

/// The effects prefab's figures (`EffectsPrefab`): the double vision's
/// amount and displacement in pixels, and the motion blur's amount the
/// accumulator starts from.
pub const DOUBLE_VISION_AMOUNT: f32 = 1.0;
pub const DOUBLE_VISION_DISPLACE: [f32; 2] = [1.88, 0.22];
pub const MOTION_BLUR_START: f32 = 0.8;
/// The most a motion blur takes (`MotionBlur`'s clamp).
pub const BLUR_AT_MOST: f32 = 0.92;
/// How long the screen's flash runs its curve (`GrenadeFlashScreenEffect._flashTime`).
pub const FLASH_TIME: f32 = 1.0;
/// How long near death's blend takes to come and to go
/// (`CC_BlendAccumulator`'s 10 s).
pub const NEAR_DEATH_FADE: f32 = 10.0;
/// What a hit's damage is taken over for the force reaction's strength
/// (`EffectsController.AIM_PUNCH_COEFFICIENT`).
pub const AIM_PUNCH_COEFFICIENT: f32 = 11.5;
/// The share of pain's blur the stress-resistance skill leaves
/// (`1 − Skills.StressPain`): untrained, all of it (skills not built).
pub const PAIN_STRENGTH: f32 = 1.0;

/// What the screen shows this frame: each effect where it is on.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Shown {
    /// The double vision's amount (0 to 1).
    pub double_vision: Option<f32>,
    /// The wiggle's timer, scale and strength.
    pub wiggle: Option<[f32; 3]>,
    /// The motion blur's amount, and whether it came on this frame (its
    /// picture made afresh from the frame).
    pub motion_blur: Option<(f32, bool)>,
    /// The screen's flash: its white (`_Coef`), and its strength for the
    /// tonemapping (`EffectStrength`, not drawn).
    pub flash: Option<[f32; 2]>,
    /// A hit's blur's value (0 to 1).
    pub hit_blur: Option<f32>,
    /// Pain's blur's amount (`CC_RadialBlur.amount`, which a pulse can
    /// take below nothing).
    pub radial_blur: Option<f32>,
    /// Exhaustion's vignette's darkness (`CC_FastVignette.darkness`).
    pub vignette: Option<f32>,
    /// Near death's blend's amount (`CC_Blend.amount`).
    pub near_death: Option<f32>,
    /// The desaturation's health share, the painkiller's mask's and the
    /// mask's radius (`HealthDesaturate`, `MaskDesaturate`, `Radius`), as
    /// the sharpening's pass takes them (the health's held to 0 to 1 there).
    pub desaturation: [f32; 3],
    /// The berserk's share of the sharpening (`CC_SharpenAccumulator`): the
    /// sum of its current strengths, held to 0 to 2; nought without one.
    pub sharpen: f32,
}

/// The accumulators' state.
#[derive(Debug, Clone)]
pub struct ScreenEffects {
    /// The effects each accumulator lists: the double vision's, the
    /// wiggle's, the blur's and the flash's.
    double_listed: Vec<u32>,
    wiggle_listed: Vec<(u32, Source)>,
    blur_listed: Vec<(u32, Source)>,
    flash_listed: Vec<u32>,
    double_on: bool,
    wiggle_on: bool,
    timer: f32,
    speed: f32,
    scale: f32,
    strength: f32,
    max_speed: f32,
    max_scale: f32,
    blur_on: bool,
    blur_fresh: bool,
    blur_max: f32,
    blur_goal: f32,
    blur: f32,
    flash_on: bool,
    flash_since: f32,
    flash_strength: f32,
    white_black: Curve,
    /// The figures of the effects below, once read (none is on before).
    figures: Option<HealthScreenData>,
    /// The game's time (`Time.time`), from the first frame.
    time: f32,
    hit: HitBlur,
    radial: Listing,
    radial_amount: f32,
    vignette: Listing,
    near_death: Listing,
    near_death_start: f32,
    near_death_end: f32,
    near_death_fading: bool,
    desaturation: Desaturation,
    sharpen: Listing,
    sharpen_amount: f32,
}

impl Default for ScreenEffects {
    fn default() -> Self {
        // The camera's `GrenadeFlashScreenEffect.WhiteBlack` (`Cam2`).
        let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
        let white_black = Curve { keys: vec![key(0.0, 0.0, 0.0), key(0.05, 1.0, 0.012540116), key(0.1721974, 0.21457812, -3.7035296), key(0.35, 0.0, 0.0)] };
        Self {
            double_listed: Vec::new(),
            wiggle_listed: Vec::new(),
            blur_listed: Vec::new(),
            flash_listed: Vec::new(),
            double_on: false,
            wiggle_on: false,
            timer: 0.0,
            speed: 1.0,
            scale: 12.0,
            strength: 1.0,
            max_speed: 0.0,
            max_scale: 0.0,
            blur_on: false,
            blur_fresh: false,
            blur_max: MOTION_BLUR_START,
            blur_goal: 0.0,
            blur: 0.0,
            flash_on: false,
            flash_since: f32::INFINITY,
            flash_strength: 1.0,
            white_black,
            figures: None,
            time: 0.0,
            hit: HitBlur { on: false, timer: 1.0, power: 0.0 },
            radial: Listing::default(),
            radial_amount: 0.0,
            vignette: Listing::default(),
            near_death: Listing::default(),
            near_death_start: 0.0,
            near_death_end: 0.0,
            near_death_fading: false,
            desaturation: Desaturation::default(),
            sharpen: Listing::default(),
            sharpen_amount: 0.0,
        }
    }
}

#[cfg(test)]
mod tests;
