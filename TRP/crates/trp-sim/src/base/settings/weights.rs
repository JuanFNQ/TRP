//! Weight limits and reweighing the settings by strength and load.

use super::*;

/// What the Strength skill makes of the carried weight's limits
/// (`PhysicalBase.UpdateWeightLimits`): the inertia's limits raised by
/// `InertiaLimitsStep` a level of Strength (`SummaryLevel`), and every
/// overweight limit times the skills' carrying factor (1 + Strength's lift,
/// `SkillManager.CarryingWeightRelativeModifier`). `BASE` is an untrained
/// character's, which the settings are made with unless told.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightLimits {
    pub strength_level: f32,
    pub carry: f32,
    /// Encumbrance switched off (`PhysicalBase.EncumberDisabled`, which the
    /// game turns on as a bot is made: `Player.SetOwnerToAIData`): every
    /// overweight limit `UNENCUMBERED`, whatever the carrying factor, so
    /// that nothing carried overweights the character; the inertia's limits
    /// stay as Strength made them (body hits spec, "The dummy on the
    /// range").
    pub unencumbered: bool,
}

impl WeightLimits {
    pub const BASE: Self = Self { strength_level: 0.0, carry: 1.0, unencumbered: false };

    /// Every overweight limit with encumbrance off, in kilograms
    /// (`UpdateWeightLimits`' other branch: `Set(9000f, 10000f)`).
    pub const UNENCUMBERED: [f32; 2] = [9000.0, 10000.0];

    /// From the skills' buffs.
    pub fn of(buffs: &crate::condition::skills::Buffs) -> Self {
        Self { strength_level: buffs.strength_level as f32, carry: 1.0 + buffs.strength_lift, unencumbered: false }
    }

    /// An overweight limit of the globals' (`low`, `high`) as these limits
    /// make it.
    pub fn overweight_limit(&self, [low, high]: [f32; 2]) -> [f32; 2] {
        if self.unencumbered { Self::UNENCUMBERED } else { [low * self.carry, high * self.carry] }
    }
}

impl MovementSettings {
    /// These settings for another carried weight: everything that goes by
    /// the weight worked out again, as for a character made carrying it,
    /// with the Strength this one has. The game does so whenever the
    /// inventory's sum changes (inventory spec, "Weight"). The curve under
    /// the walk's floor is keyed by this character again, as the game keys
    /// it for whichever character's weight changed last.
    /// The weight's two notices (`PhysicalBase.IsEncumbered`,
    /// `UpdateInternalParameters`): encumbered with any of the weight's shares
    /// over nothing or the walk's speed limited, and over-encumbered at a whole
    /// share of the base limits.
    pub fn encumbered(&self) -> (bool, bool) {
        let any = self.overweight > 0.0 || self.stamina.walk_overweight > 0.0 || self.walk_speed_limit < 1.0 || self.sprint_overweight > 0.0;
        (any, self.overweight >= 1.0)
    }

    pub fn reweighed(&self, hard: &HardSettings, config: &Json, weight: f32) -> Result<Self> {
        self.relimited(hard, config, weight, self.limits)
    }

    /// These settings for a carried weight and the weight's limits as
    /// Strength makes them (`PhysicalBase.UpdateWeightLimits`, which the
    /// game runs when Strength's level changes as when the weight does).
    pub fn relimited(&self, hard: &HardSettings, config: &Json, weight: f32, limits: WeightLimits) -> Result<Self> {
        let mut fresh = Self::new_limited(self.table.clone(), hard, config, weight, self.gravity, limits)?;
        fresh.set_strength(self.strength);
        fresh.prone_key_refused_in_reload = self.prone_key_refused_in_reload;
        fresh.steps = self.steps.clone();
        fresh.head = self.head.clone();
        Ok(fresh)
    }

    /// Sets the character's Strength skill level, 0 to 60. The top walking
    /// speed and the top sprint speed both rise with it.
    pub fn set_strength(&mut self, level: f32) {
        let t = (level / 60.0).clamp(0.0, 1.0);
        self.strength = t * 60.0;
        self.max_speed = lerp(self.walk_speed_range[0], self.walk_speed_range[1], t);
        self.sprinting_speed = lerp(self.sprint_speed_range[0], self.sprint_speed_range[1], t);
        // The skill also pushes harder off the ground: a fifth more at
        // level 50, in proportion below, and no more above
        // (`StrengthBuffJumpHeightInc`, its rule held at 50).
        self.jump_boost = 1.0 + 0.2 * level.clamp(0.0, 50.0) / 50.0;
        self.state_speed_limit =
            if self.walk_speed_limit < 1.0 { self.walk_speed_limit * self.max_speed } else { 1.0 };
    }

    /// Keys the curve under the walk's floor as a character of this
    /// inertia leaves it. The game has one such curve for the whole raid
    /// and every character, a bot too, re-keys it when its carried weight
    /// changes, so the player's walk reads whichever character's inertia
    /// came last. A host that wants the game's sharing, or a replay of a
    /// raid that had other characters, says whose; TRP's app leaves each
    /// character keyed by its own (movement spec, "The floor's curve is
    /// shared").
    pub fn key_acceleration_range(&mut self, inertia: f32) {
        let [lo, hi] = self.acceleration_range_right;
        let t = inertia.clamp(0.0, 1.0);
        self.movement_acceleration_range.move_key(1, lerp(lo[0], hi[0], t), lerp(lo[1], hi[1], t));
        self.floor_key = inertia;
    }

    /// The inertia whose key gives that curve the value `value` at this
    /// character's walk inertia: what a recorded floor says of the key it
    /// was worked out with. None where no key gives it within `within`.
    pub fn acceleration_key_giving(&self, value: f32, within: f32) -> Option<f32> {
        let [lo, hi] = self.acceleration_range_right;
        let at = |inertia: f32| {
            let mut keyed = self.movement_acceleration_range.clone();
            keyed.move_key(1, lerp(lo[0], hi[0], inertia), lerp(lo[1], hi[1], inertia));
            keyed.evaluate(self.walk_inertia)
        };
        let (none, full) = (at(0.0), at(1.0));
        if value < none.min(full) - within || value > none.max(full) + within {
            return None;
        }
        // The curve's value there rises or falls all the way with the key.
        let (mut low, mut high) = (0.0f32, 1.0f32);
        for _ in 0..40 {
            let middle = 0.5 * (low + high);
            if (at(middle) < value) == (none < full) {
                low = middle;
            } else {
                high = middle;
            }
        }
        Some(0.5 * (low + high))
    }

    /// The fastest the character can actually be moved, in metres per
    /// second, in a movement state whose cap runs over `range` from an
    /// untrained character to the top of the Strength skill. Carried weight
    /// brings it down, except in a sprint.
    pub fn hard_speed_cap(&self, range: [f32; 2], sprinting: bool) -> f32 {
        // The game counts the skill in whole levels.
        let by_skill = lerp(range[0], range[1], self.strength.round() / 60.0);
        let weight = if sprinting { 1.0 } else { self.walk_speed_limit };
        by_skill * lerp(self.sprint_speed_lower_limit, 1.0, weight)
    }

    /// What the direction held multiplies the walk's smoothing time by:
    /// more for a diagonal or a sideways walk.
    pub fn direction_inertia(&self, input: [f32; 2]) -> f32 {
        match (input[0] != 0.0, input[1] != 0.0) {
            (true, true) => self.diagonal_inertia,
            (true, false) => self.side_inertia,
            _ => 1.0,
        }
    }

    /// The Strength level at which the top walking speed is `max_speed`.
    pub fn strength_for(&self, max_speed: f32) -> f32 {
        let [low, high] = self.walk_speed_range;
        if high > low {
            ((max_speed - low) / (high - low)).clamp(0.0, 1.0) * 60.0
        } else {
            0.0
        }
    }
}
