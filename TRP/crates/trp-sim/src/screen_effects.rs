//! What health's effects do to the screen (health spec, "A contusion's
//! screen effects", "A hit and health's other screen effects"; grenade
//! spec, "A flash"; `EffectsController` and its accumulators): the double
//! vision's amount, the wiggle's timer, scale and strength, the motion
//! blur's amount, the flash's white, pain's blur, exhaustion's vignette,
//! near death's blend and a hit's blur, frame by frame, from the effects
//! listed on the character with their current strengths and the hits taken.
//! The host draws them.

use trp_data::camera_effects::HealthScreenData;
use trp_data::curve::{Curve, Key};

use crate::health::{DamageKind, Part};

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

/// The kinds of effect the accumulators take (of those the app has).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Contusion,
    LowHealth,
    Stun,
    Pain,
    PainKiller,
    Dehydration,
    Exhaustion,
    /// A stimulator's tunnel vision (`ITunnelVision`).
    TunnelVision,
    /// A medkit at work (`IMedEffect` whose item is a `MedKit`).
    MedKit,
}

impl Source {
    /// The wiggle's speed and scale for it (`_effectsSpeed`, `_effectsScale`).
    fn wiggle(self) -> Option<[f32; 2]> {
        match self {
            Source::Contusion => Some([0.8, 8.0]),
            Source::LowHealth => Some([0.4, 5.0]),
            _ => None,
        }
    }

    /// The motion blur's goal for it (`_effectsAmount`).
    fn blur(self) -> Option<f32> {
        match self {
            Source::Contusion => Some(0.8),
            Source::LowHealth => Some(0.3),
            _ => None,
        }
    }

    /// Whether the double vision takes it (`IContusion`, `IStun`).
    fn doubles(self) -> bool {
        matches!(self, Source::Contusion | Source::Stun)
    }

    /// Whether pain's blur takes it (`IDehydration`, `IPain`, `IPainKiller`).
    fn radial(self) -> bool {
        matches!(self, Source::Dehydration | Source::Pain | Source::PainKiller)
    }
}

/// An effect listed on the character: its id, kind, current strength,
/// strength, and whether it is residual (`EEffectState.Residued`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listed {
    pub id: u32,
    pub source: Source,
    pub current: f32,
    pub strength: f32,
    pub residual: bool,
    /// Started (or residual): the desaturation's accumulators take an
    /// effect as it starts (`EffectStartedEvent`).
    pub started: bool,
}

/// What a hit does to the view (`EffectsController.OnPlayerDamaged`): the
/// force reaction's push (`ForceEffector.AddForce`'s strength, and its
/// shares for the hands and the camera) and the hit blur's power.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitReaction {
    pub strength: f32,
    pub hands: f32,
    pub camera: f32,
    pub blur_power: f32,
    /// Blood on the screen (`BloodOnScreen.Hit`): any but a fall, of at
    /// least 0.1 damage (a second for the head).
    pub blood: bool,
}

/// A hit as the player is told of it (`Player.ReceiveDamage`: a shot, or
/// damage given to the player that no weapon did): its damage, what armour
/// took off it, the part, the kind, and whether the character is on
/// painkillers. Nothing for a kind the character did to itself or for none.
pub fn hit_reaction(damage: f32, absorbed: f32, part: Part, kind: DamageKind, on_painkillers: bool) -> Option<HitReaction> {
    if kind.self_inflicted() || kind == DamageKind::Undefined {
        return None;
    }
    let taken = damage + absorbed;
    let (hands, camera) = match part {
        Part::Head => (0.1, 1.3),
        Part::LeftArm | Part::RightArm => (0.15, 0.5),
        Part::LeftLeg | Part::RightLeg => (0.05, 0.3),
        _ => (0.05, 0.4),
    };
    let blur_power = if on_painkillers {
        taken
    } else if part == Part::Head {
        taken * 6.0
    } else {
        taken * 3.0
    };
    let blood = kind != DamageKind::Fall && damage >= 0.1;
    Some(HitReaction { strength: taken.max(0.0).sqrt() / AIM_PUNCH_COEFFICIENT, hands, camera, blur_power, blood })
}

/// `Mathf.InverseLerp`.
fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a != b {
        ((value - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A curve looped over its keys' span (`WrapMode.Loop`).
fn looped(curve: &Curve, t: f32) -> f32 {
    let (Some(first), Some(last)) = (curve.keys.first(), curve.keys.last()) else { return 0.0 };
    let span = last.time - first.time;
    if span <= 0.0 {
        return first.value;
    }
    curve.evaluate(first.time + (t - first.time).rem_euclid(span))
}

/// A hit's blur (`FastBlur` on the camera): on from a hit until its time is
/// up, its value by the curve over that time times the hit's power.
#[derive(Debug, Clone, Copy)]
struct HitBlur {
    on: bool,
    timer: f32,
    power: f32,
}

/// The listing of an accumulator of the base kind (`EffectAccumulator`):
/// the effects it takes, in the order they came, and whether it is on.
#[derive(Debug, Clone, Default)]
struct Listing {
    ids: Vec<u32>,
    on: bool,
}

impl Listing {
    /// Those new of its kinds added (`AddEffect`), each turning it on.
    fn add(&mut self, listed: &[Listed], takes: impl Fn(Source) -> bool) {
        for e in listed.iter().filter(|e| takes(e.source)) {
            if !self.ids.contains(&e.id) {
                self.ids.push(e.id);
                self.on = true;
            }
        }
    }

    /// Those gone taken off (`DeleteEffect`): whether that left it empty.
    fn remove(&mut self, listed: &[Listed]) -> bool {
        let before = self.ids.len();
        self.ids.retain(|id| listed.iter().any(|e| e.id == *id));
        before > 0 && self.ids.is_empty()
    }

    /// The sum of the current strengths of those it takes (`CurrentAmount`).
    fn sum(&self, listed: &[Listed]) -> f32 {
        self.ids.iter().filter_map(|id| listed.iter().find(|e| e.id == *id)).map(|e| e.current).sum()
    }
}

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
}

/// The desaturation's two accumulators (`DesaturateAccumulator`,
/// `DesaturateMaskAccumulator`): the effects each holds, the figures they
/// move, and the mask's state.
#[derive(Debug, Clone)]
pub struct Desaturation {
    health_listed: Vec<(u32, Source)>,
    health: f32,
    mask_listed: Vec<u32>,
    mask_on: bool,
    restore: bool,
    mask: f32,
    radius: f32,
    /// The ring's least and most radius (`MinMaxRadius`).
    radii: [f32; 2],
}

impl Default for Desaturation {
    fn default() -> Self {
        Self { health_listed: Vec::new(), health: 0.0, mask_listed: Vec::new(), mask_on: false, restore: false, mask: 0.0, radius: 1.0, radii: [0.325, 1.0] }
    }
}

/// The health's share each effect drives it toward (`_effectMaxValues`):
/// a medkit's use brightens, exhaustion greys.
fn health_desaturation(source: Source) -> f32 {
    if source == Source::MedKit {
        -0.15
    } else {
        0.15
    }
}

/// `Mathf.MoveTowards`.
fn move_towards(current: f32, target: f32, step: f32) -> f32 {
    if (target - current).abs() <= step {
        target
    } else {
        current + (target - current).signum() * step
    }
}

impl Desaturation {
    /// As a recording's first frame has it: the figures, the ring's
    /// radii, and the effects already started taken in (the mask on where
    /// a painkiller is), nothing moved.
    pub fn resume(figures: [f32; 3], radii: [f32; 2], listed: &[Listed]) -> Self {
        let mut d = Self { health: figures[0], mask: figures[1], radius: figures[2], radii, ..Self::default() };
        d.health_listed = listed.iter().filter(|e| e.started && matches!(e.source, Source::MedKit | Source::Exhaustion)).map(|e| (e.id, e.source)).collect();
        d.mask_listed = listed.iter().filter(|e| e.started && e.source == Source::PainKiller).map(|e| e.id).collect();
        d.mask_on = !d.mask_listed.is_empty() || figures[1] > 0.0;
        d
    }

    /// A frame (`AddEffect`, `DeleteEffect`, `UpdateAmount` of each): the
    /// health's toward the most of its effects at 0.1 a second, every
    /// frame; the mask, while on, its radius toward the least with a
    /// painkiller started (the most without) at 0.2 a second and its
    /// desaturation toward one (nought) at 0.5; a painkiller gone, the
    /// radius opens first, then the desaturation fades and the mask is off.
    pub fn frame(&mut self, dt: f32, listed: &[Listed]) -> [f32; 3] {
        for e in listed.iter().filter(|e| e.started && matches!(e.source, Source::MedKit | Source::Exhaustion)) {
            if !self.health_listed.iter().any(|(id, _)| *id == e.id) {
                self.health_listed.push((e.id, e.source));
            }
        }
        self.health_listed.retain(|(id, _)| listed.iter().any(|e| e.id == *id));
        let most = self.health_listed.iter().map(|(_, s)| health_desaturation(*s)).fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v)))).unwrap_or(0.0);
        self.health = move_towards(self.health, most, dt * 0.1);
        for e in listed.iter().filter(|e| e.started && e.source == Source::PainKiller) {
            if !self.mask_listed.contains(&e.id) {
                self.mask_listed.push(e.id);
                self.mask_on = true;
            }
        }
        let before = self.mask_listed.len();
        self.mask_listed.retain(|id| listed.iter().any(|e| e.id == *id));
        if self.mask_listed.len() < before {
            self.restore = true;
        }
        if self.mask_on {
            let any = !self.mask_listed.is_empty();
            let (goal, radius) = if any { (1.0, self.radii[0]) } else { (0.0, self.radii[1]) };
            // The radius as the frame began: the fading waits for it open.
            let was = self.radius;
            self.radius = move_towards(self.radius, radius, dt * 0.2);
            if !self.restore {
                self.mask = move_towards(self.mask, goal, dt * 0.5);
            } else if was >= self.radii[1] {
                self.mask = move_towards(self.mask, goal, dt * 0.5);
                if self.mask <= 0.0 {
                    self.radius = self.radii[1];
                    self.restore = false;
                    self.mask_on = false;
                }
            }
        }
        [self.health, self.mask, self.radius]
    }
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
        }
    }
}

impl ScreenEffects {
    /// The figures of pain's blur, exhaustion's vignette, near death and a
    /// hit's blur, as read from the camera (`trp_data::camera_effects`).
    pub fn set_figures(&mut self, figures: HealthScreenData) {
        self.figures = Some(figures);
    }

    /// The desaturation's health share, mask and radius as they stand.
    pub fn desaturation(&self) -> [f32; 3] {
        [self.desaturation.health, self.desaturation.mask, self.desaturation.radius]
    }

    /// The painkiller's ring's least and most radius (`MinMaxRadius`), as
    /// read from the level's camera; the mask's radius starts at the most.
    pub fn set_desaturation_radii(&mut self, radii: [f32; 2]) {
        self.desaturation.radii = radii;
        self.desaturation.radius = radii[1];
    }

    /// Pain's blur's centre (the screen's middle before the figures come).
    pub fn radial_center(&self) -> [f32; 2] {
        self.figures.as_ref().map_or([0.5, 0.5], |f| f.radial_center)
    }

    /// Exhaustion's vignette's centre and sharpness.
    pub fn vignette_shape(&self) -> ([f32; 2], f32) {
        self.figures.as_ref().map_or(([0.5, 0.5], 0.0), |f| (f.vignette_center, f.vignette_sharpness))
    }

    /// A hit (`FastBlur.Hit`, its effect turned on first): the blur's time
    /// from its start, at the hit's power. Not after a death (the death's
    /// own blur, not built, takes it).
    pub fn hit(&mut self, power: f32) {
        if self.figures.is_some() {
            self.hit = HitBlur { on: true, timer: 0.0, power };
        }
    }

    /// A frame: the effects listed now, those new added to the
    /// accumulators that take their kind and those gone taken off, then
    /// each accumulator on worked out (`UpdateAmount`), the wiggle's timer
    /// moved on by its speed (`CC_Wiggle.Update`) and the flash's time.
    pub fn frame(&mut self, dt: f32, listed: &[Listed]) -> Shown {
        let value_of = |take: &dyn Fn(Source) -> bool| listed.iter().filter(|e| take(e.source)).map(|e| e.current).sum::<f32>().clamp(0.0, 1.0);
        let doubled = value_of(&|s| s.doubles());
        let wiggled = value_of(&|s| s.wiggle().is_some());
        let blurred = value_of(&|s| s.blur().is_some());
        self.flash_since += dt;
        for e in listed {
            if e.source.doubles() && !self.double_listed.contains(&e.id) {
                self.double_listed.push(e.id);
                self.double_on = true;
            }
            if e.source.wiggle().is_some() && !self.wiggle_listed.iter().any(|(id, _)| *id == e.id) {
                self.wiggle_listed.push((e.id, e.source));
                self.wiggle_on = true;
                self.wiggle_figures();
                self.strength = 1.0;
            }
            if e.source.blur().is_some() && !self.blur_listed.iter().any(|(id, _)| *id == e.id) {
                self.blur_listed.push((e.id, e.source));
                if !self.blur_on {
                    self.blur_fresh = true;
                }
                self.blur_on = true;
                self.blur_goal_of();
            }
            // `FlashAccumulator.AddEffect`: the screen's flash goes off at
            // the stun's strength.
            if e.source == Source::Stun && !self.flash_listed.contains(&e.id) {
                self.flash_listed.push(e.id);
                self.flash_on = true;
                self.flash_since = 0.0;
                self.flash_strength = e.strength;
            }
        }
        let here = |id: u32| listed.iter().any(|e| e.id == id);
        // Taken off (`DeleteEffect`): the double vision and the blur worked
        // out a last time and off once empty; the wiggle's figures again,
        // left on to fade; the flash off once empty.
        let before = self.double_listed.len();
        self.double_listed.retain(|id| here(*id));
        if before > 0 && self.double_listed.is_empty() {
            self.double_on = false;
        }
        let before = self.wiggle_listed.len();
        self.wiggle_listed.retain(|(id, _)| here(*id));
        if self.wiggle_listed.len() != before {
            self.wiggle_figures();
        }
        let before = self.blur_listed.len();
        self.blur_listed.retain(|(id, _)| here(*id));
        if self.blur_listed.len() != before {
            if self.blur_listed.is_empty() && self.blur_on {
                self.motion_blur(blurred, dt);
                self.blur_on = false;
            }
            self.blur_goal_of();
        }
        let before = self.flash_listed.len();
        self.flash_listed.retain(|id| here(*id));
        if before > 0 && self.flash_listed.is_empty() {
            self.flash_on = false;
        }
        if self.wiggle_on {
            self.wiggle(wiggled, dt);
        }
        if self.blur_on {
            self.motion_blur(blurred, dt);
        }
        if self.wiggle_on {
            self.timer += self.speed * dt;
        }
        self.time += dt;
        let (hit_blur, radial_blur, vignette, near_death) = self.health_effects(dt, listed);
        let desaturation = self.desaturation.frame(dt, listed);
        let fresh = std::mem::take(&mut self.blur_fresh);
        let flash = self.flash_on.then(|| {
            // `FlashAccumulator.UpdateAmount`: the largest current strength's
            // square root (all outdoors here).
            let strongest = listed.iter().filter(|e| e.source == Source::Stun).map(|e| e.current).fold(0.0f32, f32::max);
            let share = self.flash_since / FLASH_TIME;
            let share = if share > 1.0 { 0.0 } else { share.clamp(0.0, 1.0) };
            [self.white_black.evaluate(share) * self.flash_strength, strongest.sqrt()]
        });
        Shown {
            double_vision: self.double_on.then_some(doubled * DOUBLE_VISION_AMOUNT),
            wiggle: self.wiggle_on.then_some([self.timer, self.scale, self.strength]),
            motion_blur: self.blur_on.then_some((self.blur.clamp(0.0, BLUR_AT_MOST), fresh)),
            flash,
            hit_blur,
            radial_blur,
            vignette,
            near_death,
            desaturation,
        }
    }

    /// The hit's blur (`FastBlur.Update`), pain's blur, exhaustion's
    /// vignette and near death's blend (their accumulators' `AddEffect`,
    /// `DeleteEffect` and `UpdateAmount`), each where it is on.
    fn health_effects(&mut self, dt: f32, listed: &[Listed]) -> (Option<f32>, Option<f32>, Option<f32>, Option<f32>) {
        let Some(figures) = self.figures.clone() else { return (None, None, None, None) };
        let now = self.time;
        // The hit's blur: off once its time is up, with nothing drawn then.
        let hit_blur = if self.hit.on {
            self.hit.timer += dt;
            let share = (self.hit.timer / figures.hit_time).clamp(0.0, 1.0);
            let value = (figures.hit_curve.evaluate(share) * self.hit.power).clamp(0.0, 1.0);
            if share >= 1.0 {
                self.hit.on = false;
            }
            self.hit.on.then_some(value)
        } else {
            None
        };
        // Pain's blur: worked out a last time as it empties, then off.
        self.radial.add(listed, Source::radial);
        if self.radial.remove(listed) {
            self.radial_blur(&figures, listed, now);
            self.radial.on = false;
        }
        if self.radial.on {
            self.radial_blur(&figures, listed, now);
        }
        let radial_blur = self.radial.on.then_some(self.radial_amount);
        // Exhaustion's and tunnel vision's vignette, by the flicker at the time.
        self.vignette.add(listed, |s| matches!(s, Source::Exhaustion | Source::TunnelVision));
        let emptied = self.vignette.remove(listed);
        let darkness = self.vignette.sum(listed).clamp(0.0, 1.0) * figures.vignette_darkness * looped(&figures.flicker, now);
        if emptied {
            self.vignette.on = false;
        }
        let vignette = self.vignette.on.then_some(darkness);
        // Near death: its fade begun afresh as the first comes and as the
        // last goes; off once faded out.
        for e in listed.iter().filter(|e| e.source == Source::LowHealth) {
            if !self.near_death.ids.contains(&e.id) {
                if self.near_death.ids.is_empty() {
                    (self.near_death_start, self.near_death_end) = (now, now + NEAR_DEATH_FADE);
                }
                self.near_death.ids.push(e.id);
                self.near_death.on = true;
                self.near_death_fading = false;
            }
        }
        if self.near_death.remove(listed) {
            (self.near_death_start, self.near_death_end) = (now, now + NEAR_DEATH_FADE);
            self.near_death_fading = true;
        }
        let near_death = if self.near_death.on {
            let share = if self.near_death_fading { inverse_lerp(self.near_death_end, self.near_death_start, now) } else { inverse_lerp(self.near_death_start, self.near_death_end, now) };
            if share <= 0.0 && self.near_death.ids.is_empty() {
                self.near_death.on = false;
            }
            self.near_death.on.then_some(share * figures.near_death_most)
        } else {
            None
        };
        (hit_blur, radial_blur, vignette, near_death)
    }

    /// `CC_RadialBlurAccumulator.UpdateAmount`: nothing while a painkiller
    /// is at work (not residual); else the sum, with a pulse while a pain
    /// is at work and no painkiller before it in the list, or eased from
    /// the last amount toward it by 0.15 a frame.
    fn radial_blur(&mut self, figures: &HealthScreenData, listed: &[Listed], now: f32) {
        let effects: Vec<&Listed> = self.radial.ids.iter().filter_map(|id| listed.iter().find(|e| e.id == *id)).collect();
        let mut pulse = false;
        for e in &effects {
            if e.source != Source::PainKiller || e.residual {
                if e.source == Source::Pain && !e.residual {
                    pulse = true;
                }
                continue;
            }
            pulse = false;
            break;
        }
        let working = effects.iter().any(|e| e.source == Source::PainKiller && !e.residual);
        let value = if working { 0.0 } else { effects.iter().map(|e| e.current).sum::<f32>() }.clamp(0.0, 1.0);
        let value = if pulse {
            value + ((now * 7.0).cos() + (now * 3.0).sin()) * 0.2
        } else {
            let last = self.radial_amount / figures.radial_most;
            last + (value - last) * 0.15
        };
        self.radial_amount = value * figures.radial_most * PAIN_STRENGTH;
    }

    /// `UpdateScaleAndSpeed`: the largest of the listed kinds'.
    fn wiggle_figures(&mut self) {
        let figures = self.wiggle_listed.iter().filter_map(|(_, s)| s.wiggle());
        (self.max_speed, self.max_scale) = figures.fold((0.0f32, 0.0f32), |(speed, scale), [a, b]| (speed.max(a), scale.max(b)));
    }

    /// `UpdateGoalValue`: the largest of the listed kinds'.
    fn blur_goal_of(&mut self) {
        self.blur_goal = self.blur_listed.iter().filter_map(|(_, s)| s.blur()).fold(0.0f32, f32::max);
    }

    /// `CC_WiggleAccumulator.UpdateAmount`.
    fn wiggle(&mut self, value: f32, dt: f32) {
        self.speed = value * self.max_speed;
        self.scale = value * self.max_scale;
        if self.wiggle_listed.is_empty() {
            self.strength += (0.0 - self.strength) * dt.clamp(0.0, 1.0);
            if self.strength < 0.01 {
                self.wiggle_on = false;
            }
        }
    }

    /// `MotionBlurAccumulator.UpdateAmount`.
    fn motion_blur(&mut self, value: f32, dt: f32) {
        self.blur_max += (self.blur_goal - self.blur_max) * dt.clamp(0.0, 1.0);
        self.blur = value * self.blur_max;
    }
}

/// The eyes' burn's figures (`EyeBurn` on `Cam2`; `trp_data::eye_burn`).
#[derive(Debug, Clone)]
pub struct EyeBurnFigures {
    pub scale: [f32; 2],
    pub max_time: f32,
    pub trail_every: f32,
    pub trail_shift: f32,
    pub max_trail: u32,
    pub trail_curve: Curve,
    pub scale_curve: Curve,
    /// How many spot materials and trail materials there are.
    pub spots: usize,
    pub trails: usize,
}

/// A spot of the burn as drawn this frame: its middle and half size in the
/// screen's terms (−1 to 1 each way, y up), its turn (`_Rotation`), its
/// life left (`_Coef`), and its picture (the spots' by their order, then
/// the trail's).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnSpot {
    pub middle: [f32; 2],
    pub half: [f32; 2],
    pub turn: f32,
    pub coef: f32,
    pub picture: usize,
}

/// A spot kept (`EyeBurnSpot`).
#[derive(Debug, Clone, Copy)]
struct Spot {
    middle: [f32; 2],
    picture: usize,
    turn: f32,
    scale: [f32; 2],
    start: f32,
    lasts: f32,
}

/// The eyes' burn (`EyeBurn`): its spots, the material last used, and the
/// time left before it goes off.
#[derive(Debug, Clone)]
pub struct EyeBurn {
    figures: EyeBurnFigures,
    spots: Vec<Spot>,
    current: usize,
    off_in: Option<f32>,
}

impl EyeBurn {
    pub fn new(figures: EyeBurnFigures) -> Self {
        Self { figures, spots: Vec::new(), current: 0, off_in: None }
    }

    /// A burn (`EyeBurn.Burn`): where the blast is on the screen (0 to 1
    /// from the lower left, and its depth), the distance's share, the
    /// view's turn this frame (degrees, across and up), the screen's width
    /// over its height, the time, and draws from 0 to 1. Nothing where the
    /// blast is not in sight; else a spot there and a trail along the turn,
    /// and the burn on for its longest time.
    pub fn burn(&mut self, screen: [f32; 3], strength: f32, turn: [f32; 2], aspect: f32, now: f32, random: &mut dyn FnMut() -> f32) -> bool {
        let size = self.figures.scale[0] + (self.figures.scale[1] - self.figures.scale[0]) * strength;
        let scale = [size, size * aspect];
        let lasts = self.figures.max_time * strength;
        if !(screen[0] > 0.0 && screen[0] < 1.0 && screen[1] > 0.0 && screen[1] < 1.0 && screen[2] > 0.0) {
            return false;
        }
        // `AddEyeBurn`: the next spot material, a whole number of degrees.
        self.current += 1;
        let picture = self.current % self.figures.spots.max(1);
        let middle = [screen[0] * 2.0 - 1.0, screen[1] * 2.0 - 1.0];
        let degrees = (random() * 360.0).floor().min(359.0);
        self.spots.push(Spot { middle, picture, turn: degrees, scale, start: now, lasts });
        // `AddTrailEyeburn`: along the turn (across, and up turned down).
        let way = [turn[0], -turn[1]];
        let length = (way[0] * way[0] + way[1] * way[1]).sqrt();
        let count = (self.figures.max_trail as f32).min((length / self.figures.trail_every).floor()) as u32;
        if count >= 1 {
            let unit = [way[0] / length, way[1] / length];
            let mut at = [middle[0] + unit[0] * self.figures.trail_shift * scale[0], middle[1] + unit[1] * self.figures.trail_shift * scale[0]];
            for i in 0..count {
                let trail = (random() * self.figures.trails.max(1) as f32).floor().min(self.figures.trails.max(1) as f32 - 1.0) as usize;
                let degrees = (random() * 360.0).floor().min(359.0);
                let shrink = self.figures.trail_curve.evaluate(i as f32);
                let scale = [scale[0] * shrink, scale[1] * shrink];
                self.spots.push(Spot { middle: at, picture: self.figures.spots + trail, turn: degrees, scale, start: now, lasts });
                at = [at[0] + unit[0] * self.figures.trail_shift * scale[0], at[1] + unit[1] * self.figures.trail_shift * scale[0]];
            }
        }
        self.off_in = Some(self.figures.max_time);
        true
    }

    /// The frame's spots, each with its life left and its size by it; none
    /// once the burn has gone off (`TimeDisable`: its longest time after
    /// the last burn, the spots let go).
    pub fn frame(&mut self, dt: f32, now: f32) -> Vec<BurnSpot> {
        let Some(left) = self.off_in.as_mut() else { return Vec::new() };
        *left -= dt;
        if *left < 0.0 {
            self.off_in = None;
            self.spots.clear();
            return Vec::new();
        }
        self.spots
            .iter()
            .map(|spot| {
                let coef = if spot.lasts > 0.0 { (1.0 - (now - spot.start) / spot.lasts).clamp(0.0, 1.0) } else { 0.0 };
                let size = self.figures.scale_curve.evaluate(coef);
                BurnSpot { middle: spot.middle, half: [spot.scale[0] * size, spot.scale[1] * size], turn: spot.turn, coef, picture: spot.picture }
            })
            .collect()
    }

    /// Whether the burn is on (its picture laid over the frame).
    pub fn on(&self) -> bool {
        self.off_in.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burn_leaves_a_spot_and_a_trail_and_goes_off() {
        let figures = EyeBurnFigures {
            scale: [0.2, 1.1],
            max_time: 100.0,
            trail_every: 0.5,
            trail_shift: 0.05,
            max_trail: 10,
            trail_curve: Curve::linear(&[(0.0, 1.0), (12.0, 0.5)]),
            scale_curve: Curve::linear(&[(0.0, 0.4), (1.0, 1.0)]),
            spots: 3,
            trails: 1,
        };
        let mut burn = EyeBurn::new(figures);
        let mut random = || 0.5;
        // Out of sight: nothing.
        assert!(!burn.burn([0.5, 0.5, -1.0], 1.0, [0.0, 0.0], 16.0 / 9.0, 0.0, &mut random));
        assert!(!burn.on());
        // In the middle, full strength, the view turning 2 degrees across:
        // a spot of size 1.1 (as tall in the screen's terms by its shape)
        // and four trail spots, each smaller by the curve.
        assert!(burn.burn([0.5, 0.5, 5.0], 1.0, [2.0, 0.0], 16.0 / 9.0, 0.0, &mut random));
        let spots = burn.frame(0.0, 0.0);
        assert_eq!(spots.len(), 5);
        assert_eq!(spots[0].middle, [0.0, 0.0]);
        assert!((spots[0].half[0] - 1.1).abs() < 1e-5 && (spots[0].half[1] - 1.1 * 16.0 / 9.0).abs() < 1e-4);
        assert_eq!(spots[0].picture, 1);
        assert!(spots[1..].iter().all(|spot| spot.picture == 3 && spot.middle[0] > 0.0 && spot.middle[1] == 0.0));
        // Half its time on: half its life, its size by the curve.
        let half = burn.frame(50.0, 50.0);
        assert!((half[0].coef - 0.5).abs() < 1e-5 && (half[0].half[0] - 1.1 * 0.7).abs() < 1e-4);
        // Its 100 s up: off, the spots let go.
        assert!(burn.frame(50.5, 100.5).is_empty() && !burn.on());
    }

    /// The camera's figures as the install has them (`the_camera_effects_are_read`).
    fn figures() -> HealthScreenData {
        let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
        HealthScreenData {
            hit_time: 0.65,
            hit_curve: Curve { keys: vec![key(0.0, -0.0023417473, 0.0), key(0.25, 0.33, 0.0), key(1.0, -0.0023417473, -0.027794667)] },
            hit_squares: [1024, 512],
            hit_passes: 3,
            radial_most: 0.035,
            radial_center: [0.5, 0.5],
            vignette_center: [0.5, 0.5],
            vignette_sharpness: 10.0,
            vignette_darkness: 74.9,
            flicker: Curve { keys: vec![key(0.0, 1.0, 0.0), key(1.0352969, 0.55802685, -0.047385693), key(3.438985, 0.56120956, -0.0056624413), key(5.6363144, 1.0021265, 0.0)] },
            near_death_most: 1.0,
        }
    }

    fn effect(id: u32, source: Source, current: f32, residual: bool) -> Listed {
        Listed { id, source, current, strength: 1.0, residual, started: true }
    }

    #[test]
    fn a_medkit_brightens_exhaustion_greys_and_the_stronger_wins() {
        let mut d = Desaturation::default();
        let medkit = effect(7, Source::MedKit, 1.0, false);
        // A second of frames toward -0.15 at 0.1 a second.
        for _ in 0..120 {
            d.frame(1.0 / 120.0, &[medkit]);
        }
        assert!((d.health + 0.1).abs() < 1e-4, "{}", d.health);
        // Exhaustion with it: the most, 0.15, wins.
        let tired = effect(8, Source::Exhaustion, 1.0, false);
        for _ in 0..360 {
            d.frame(1.0 / 120.0, &[medkit, tired]);
        }
        assert!((d.health - 0.15).abs() < 1e-4, "{}", d.health);
        // Not started yet: not taken.
        let mut d = Desaturation::default();
        d.frame(0.5, &[Listed { started: false, ..medkit }]);
        assert_eq!(d.health, 0.0);
    }

    #[test]
    fn a_painkiller_closes_the_ring_and_it_opens_before_the_grey_fades() {
        let mut d = Desaturation::default();
        let pill = effect(9, Source::PainKiller, 1.0, false);
        // The mask at 0.5 a second, the radius from 1 to 0.325 at 0.2.
        let [_, mask, radius] = d.frame(1.0, &[pill]);
        assert!((mask - 0.5).abs() < 1e-5 && (radius - 0.8).abs() < 1e-5, "{mask} {radius}");
        for _ in 0..4 {
            d.frame(1.0, &[pill]);
        }
        assert_eq!(d.frame(1.0, &[pill]), [0.0, 1.0, 0.325]);
        // Gone: the radius opens first (3.375 s), the grey waits for it.
        let [_, mask, radius] = d.frame(1.0, &[]);
        assert!((mask - 1.0).abs() < 1e-6 && (radius - 0.525).abs() < 1e-5);
        for _ in 0..3 {
            d.frame(1.0, &[]);
        }
        assert_eq!(d.radius, 1.0);
        assert_eq!(d.mask, 1.0, "the radius only just opened: the grey moves from the next frame");
        d.frame(1.0, &[]);
        d.frame(1.0, &[]);
        assert_eq!((d.mask, d.mask_on), (0.0, false));
    }

    #[test]
    fn tunnel_vision_darkens_as_exhaustion_does() {
        let mut screen = ScreenEffects::default();
        screen.set_figures(figures());
        let shown = screen.frame(0.0, &[effect(5, Source::TunnelVision, 1.0, false)]);
        assert!(shown.vignette.is_some_and(|v| v > 0.0));
    }

    #[test]
    fn a_hit_pushes_by_its_part_and_blurs_by_its_power() {
        // Barbed wire's 1 to a leg: a push of 1/11.5, the camera's share 0.3.
        let wire = hit_reaction(1.0, 0.0, Part::LeftLeg, DamageKind::Barbed, false).expect("a hit");
        assert!((wire.strength - 1.0 / 11.5).abs() < 1e-6 && wire.hands == 0.05 && wire.camera == 0.3 && wire.blur_power == 3.0 && wire.blood);
        assert!(!hit_reaction(0.05, 0.0, Part::LeftLeg, DamageKind::Barbed, false).expect("a hit").blood);
        // A shot to the head, armour having taken 5 of 25: all 25 count.
        let head = hit_reaction(20.0, 5.0, Part::Head, DamageKind::Bullet, false).expect("a hit");
        assert!((head.strength - 5.0 / 11.5).abs() < 1e-6 && head.camera == 1.3 && head.blur_power == 150.0);
        // On painkillers the blur's power is the damage alone.
        assert_eq!(hit_reaction(4.0, 0.0, Part::Chest, DamageKind::GrenadeFragment, true).map(|h| (h.hands, h.camera, h.blur_power)), Some((0.05, 0.4, 4.0)));
        // A bleeding's damage is the character's own: nothing.
        assert!(hit_reaction(1.0, 0.0, Part::Chest, DamageKind::LightBleeding, false).is_none());
        assert!(hit_reaction(1.0, 0.0, Part::Chest, DamageKind::Undefined, false).is_none());
    }

    #[test]
    fn the_hit_blur_peaks_and_ends_with_its_time() {
        let mut screen = ScreenEffects::default();
        screen.hit(3.0);
        assert!(screen.frame(0.016, &[]).hit_blur.is_none(), "nothing before the figures are read");
        screen.set_figures(figures());
        screen.hit(3.0);
        // A quarter of its time in: the curve's 0.33 times 3, held to 1.
        let peak = screen.frame(0.1625, &[]).hit_blur.expect("on");
        assert!((peak - 0.99).abs() < 1e-4, "{peak}");
        // Its time up: off, nothing drawn on that frame.
        assert!(screen.frame(0.4, &[]).hit_blur.is_some());
        assert!(screen.frame(0.1, &[]).hit_blur.is_none());
    }

    #[test]
    fn pain_pulses_the_radial_blur_a_painkiller_stills_it() {
        let mut screen = ScreenEffects::default();
        screen.set_figures(figures());
        // Pain at work: its strength plus the pulse, times 0.035.
        let shown = screen.frame(0.5, &[effect(1, Source::Pain, 1.0, false)]);
        let pulse = ((0.5f32 * 7.0).cos() + (0.5f32 * 3.0).sin()) * 0.2;
        assert!((shown.radial_blur.expect("on") - (1.0 + pulse) * 0.035).abs() < 1e-6);
        // A painkiller at work: nothing, eased toward by 0.15 a frame.
        let before = screen.frame(0.0, &[effect(1, Source::Pain, 1.0, false)]).radial_blur.expect("on");
        let eased = screen.frame(0.0, &[effect(1, Source::Pain, 1.0, false), effect(2, Source::PainKiller, 1.0, false)]).radial_blur.expect("on");
        assert!((eased - before * 0.85).abs() < 1e-6, "{before} {eased}");
        // The pain gone and the painkiller residual: eased toward its strength.
        let toward = screen.frame(0.0, &[effect(2, Source::PainKiller, 0.5, true)]).radial_blur.expect("on");
        assert!((toward - (eased + (0.5 * 0.035 - eased) * 0.15)).abs() < 1e-6);
        // None left: off.
        assert!(screen.frame(0.016, &[]).radial_blur.is_none());
    }

    #[test]
    fn exhaustion_darkens_by_the_flicker_and_near_death_fades_in_and_out() {
        let mut screen = ScreenEffects::default();
        screen.set_figures(figures());
        // Exhaustion at full, at the flicker's start: 74.9 dark.
        let shown = screen.frame(0.0, &[effect(5, Source::Exhaustion, 1.0, false)]);
        assert!((shown.vignette.expect("on") - 74.9).abs() < 1e-4);
        // A loop of the flicker later, the same.
        let later = screen.frame(5.6363144, &[effect(5, Source::Exhaustion, 1.0, false)]).vignette.expect("on");
        assert!((later - 74.9).abs() < 0.2, "{later}");
        assert!(screen.frame(0.016, &[]).vignette.is_none());
        // Low health: near death from nought to whole over 10 s.
        let low = effect(6, Source::LowHealth, 1.0, false);
        assert_eq!(screen.frame(0.0, &[low]).near_death, Some(0.0));
        assert!((screen.frame(5.0, &[low]).near_death.expect("on") - 0.5).abs() < 1e-5);
        // Gone at half: from whole down over 10 s, then off.
        assert_eq!(screen.frame(0.0, &[]).near_death, Some(1.0));
        assert!((screen.frame(7.5, &[]).near_death.expect("on") - 0.25).abs() < 1e-4);
        assert!(screen.frame(2.6, &[]).near_death.is_none());
    }

    fn contusion(id: u32, current: f32) -> Listed {
        Listed { id, source: Source::Contusion, current, strength: 1.0, residual: false, started: true }
    }

    #[test]
    fn a_contusion_turns_the_three_on_and_its_end_turns_them_off() {
        let mut screen = ScreenEffects::default();
        // Nothing on; the desaturation at rest (its ring open).
        assert_eq!(screen.frame(0.016, &[]), Shown { desaturation: [0.0, 0.0, 1.0], ..Shown::default() });
        // Added, still at nought: every one on, nothing yet to see.
        let shown = screen.frame(0.016, &[contusion(7, 0.0)]);
        assert_eq!(shown.double_vision, Some(0.0));
        assert_eq!(shown.motion_blur, Some((0.0, true)));
        assert_eq!(shown.wiggle.map(|w| w[2]), Some(1.0));
        // At full strength: the double vision whole, the wiggle at 8 and
        // its timer going at 0.8 a second, the blur at 0.8.
        let shown = screen.frame(0.5, &[contusion(7, 1.0)]);
        assert_eq!(shown.double_vision, Some(1.0));
        let [timer, scale, strength] = shown.wiggle.expect("the wiggle");
        assert!((scale - 8.0).abs() < 1e-6 && strength == 1.0 && (timer - 0.4).abs() < 1e-5, "{timer}");
        assert!((shown.motion_blur.expect("the blur").0 - 0.8).abs() < 1e-6);
        // Two at once: their sum held to one.
        let shown = screen.frame(0.016, &[contusion(7, 0.8), contusion(9, 0.8)]);
        assert_eq!(shown.double_vision, Some(1.0));
        // Gone: the double vision and the blur off at once; the wiggle
        // fades at a frame's time a frame and goes below 0.01.
        let shown = screen.frame(0.016, &[]);
        assert!(shown.double_vision.is_none() && shown.motion_blur.is_none());
        assert!(shown.wiggle.is_some_and(|w| w[2] < 1.0 && w[1] == 0.0));
        let mut frames = 0;
        while screen.frame(0.016, &[]).wiggle.is_some() {
            frames += 1;
            assert!(frames < 2000);
        }
        // ln(100) / 0.016 frames: about 288.
        assert!((280..=295).contains(&frames), "{frames}");
    }

    #[test]
    fn the_blur_comes_on_afresh_each_time() {
        let mut screen = ScreenEffects::default();
        assert_eq!(screen.frame(0.016, &[contusion(1, 0.5)]).motion_blur.map(|b| b.1), Some(true));
        assert_eq!(screen.frame(0.016, &[contusion(1, 0.5)]).motion_blur.map(|b| b.1), Some(false));
        screen.frame(0.016, &[]);
        assert_eq!(screen.frame(0.016, &[contusion(2, 0.5)]).motion_blur.map(|b| b.1), Some(true));
    }

    #[test]
    fn a_stun_flashes_white_and_doubles_low_health_wiggles_and_blurs() {
        let mut screen = ScreenEffects::default();
        let stun = |current| Listed { id: 3, source: Source::Stun, current, strength: 0.8, residual: false, started: true };
        // At once: the flash at its curve's start, nought.
        let shown = screen.frame(0.0, &[stun(0.0)]);
        assert_eq!(shown.flash.map(|f| f[0]), Some(0.0));
        assert!(shown.wiggle.is_none() && shown.motion_blur.is_none() && shown.double_vision == Some(0.0));
        // 0.05 s on: white at the stun's strength.
        let shown = screen.frame(0.05, &[stun(0.2)]);
        assert!((shown.flash.expect("the flash")[0] - 0.8).abs() < 1e-5);
        // Past its 0.35 s, and past its second: no white, still on.
        assert_eq!(screen.frame(0.4, &[stun(1.0)]).flash, Some([0.0, 1.0]));
        assert_eq!(screen.frame(1.0, &[stun(0.25)]).flash, Some([0.0, 0.5]));
        assert!(screen.frame(0.016, &[]).flash.is_none());
        // Low health alone: the wiggle at 5 and the blur toward 0.3.
        let low = Listed { id: 4, source: Source::LowHealth, current: 1.0, strength: 1.0, residual: false, started: true };
        let shown = screen.frame(0.016, &[low]);
        assert!(shown.double_vision.is_none());
        assert_eq!(shown.wiggle.map(|w| w[1]), Some(5.0));
        let blur = shown.motion_blur.expect("the blur").0;
        assert!(blur < 0.8 && blur > 0.79, "easing from 0.8 toward 0.3: {blur}");
    }
}
