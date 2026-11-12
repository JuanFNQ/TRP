//! Setting the figures and working out each frame's screen effects.

use super::*;

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

    /// The camera's sharpening as it stands, from the player's own
    /// (`CC_SharpenAccumulator.UpdateAmount`: the player's, and the
    /// berserk's share of the way from it to the effects prefab's, which a
    /// share of 2 takes as far again; the player's without the figures).
    pub fn sharpen(&self, player: f32) -> f32 {
        self.figures.as_ref().map_or(player, |f| player + self.sharpen_amount * (f.sharpen_most - player))
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
        // The berserk's sharpening: taken as it starts, dropped as it is
        // removed (worked out a last time, to the player's own, as the last
        // goes); the sum of their current strengths, held to 0 to 2.
        for e in listed.iter().filter(|e| e.started && e.source == Source::Berserk) {
            if !self.sharpen.ids.contains(&e.id) {
                self.sharpen.ids.push(e.id);
                self.sharpen.on = true;
            }
        }
        if self.sharpen.remove(listed) {
            self.sharpen.on = false;
        }
        self.sharpen_amount = self.sharpen.sum(listed).clamp(0.0, 2.0);
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
            sharpen: self.sharpen_amount,
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
    pub(super) fn radial_blur(&mut self, figures: &HealthScreenData, listed: &[Listed], now: f32) {
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
    pub(super) fn wiggle(&mut self, value: f32, dt: f32) {
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
    pub(super) fn motion_blur(&mut self, value: f32, dt: f32) {
        self.blur_max += (self.blur_goal - self.blur_max) * dt.clamp(0.0, 1.0);
        self.blur = value * self.blur_max;
    }
}
