//! Melee, wounds, weapon switches, holding breath and aiming.

use super::*;

impl Stamina {
    /// Whether a melee stroke may start (`Physical.CanMeleeHit`): the melee
    /// consumption's start threshold, 10, in the legs or else in the hands
    /// (`Stamina.CanAdd`: more than it).
    pub fn can_melee(&self) -> bool {
        !self.holds_back || self.legs.can_add(MELEE_START) || self.arms.can_add(MELEE_START)
    }

    /// A melee stroke's swing speed (`PhysicalBase.MeleeSpeed`): the stamina
    /// settings' figure with the hands exhausted, else 1.
    pub fn melee_speed(&self) -> f32 {
        if self.arms.exhausted() { self.settings.exhausted_melee_speed } else { 1.0 }
    }

    /// What a blow's damage is cut by with the hands exhausted
    /// (`HandsStamina.Exhausted` in `ProcessHit`: `trp_sim::knife_hands::
    /// blow_damage`), none otherwise.
    pub fn melee_damage_cut(&self) -> Option<f32> {
        self.arms.exhausted().then_some(self.settings.exhausted_melee_damage)
    }

    /// A melee stroke's cost (`ConsumeAsMelee`, its consumption's charge):
    /// the knife's expense times one and the overweight, off the hands, the
    /// legs paying it again whole where the hands could not, and off the
    /// oxygen; each held from refilling half a second.
    pub fn melee(&mut self, expense: f32, now: f32) {
        if expense <= 0.0 {
            return;
        }
        let cost = expense * (1.0 + self.settings.overweight);
        let (unpaid, spent) = self.arms.charge(cost, Some(MELEE_DOWNTIME), now);
        let dry = unpaid > 0.0 && self.legs.charge(cost, Some(MELEE_DOWNTIME), now).1;
        self.oxygen.charge(cost, Some(MELEE_DOWNTIME), now);
        if spent {
            self.arms_ran_dry();
        }
        if dry {
            self.legs_ran_dry();
        }
    }

    /// A hit by a weapon (`Physical.BulletHit`, the `Wound` consumption):
    /// `burn` (the round's figure a point times the damage) off the legs at
    /// once, the refill not paused (`AllowsRestoration`). Stamina spec, "A
    /// drain". Nothing with the stamina switched off. PROVISIONAL: read
    /// from code, no recording has a hit the player lives.
    pub fn wound(&mut self, burn: f32, now: f32) {
        if !self.drains {
            return;
        }
        if self.legs.charge(burn, None, now).1 {
            self.legs_ran_dry();
        }
    }

    /// A fast weapon switch (`Physical.OnWeaponSwitchFast`, the
    /// `FastWeaponSwitch` consumption): `fast_switch_cost` over (the
    /// weapon's class skill level / 50, whole, + 1) off the arms; if the
    /// arms could not pay it all, the legs pay it all as well; each pool's
    /// refill paused as a cost that does not let it refill (no downtime of
    /// its own). Returns what the arms lacked (`FastWeaponSwitchStaminaLack`),
    /// which the draw's hand shake goes by. The charge is worked out again
    /// at each pool's reading of it (a `Compute`): the arms' leaves the lack
    /// what they were short, and the legs', after the arms have paid what
    /// they had, leaves it the whole charge. So the lack is nothing where
    /// the arms pay it all, and the whole charge where they do not, not the
    /// arms' shortfall alone. Set
    /// beside `courses8`'s `pistol-quickdraw`: each of its eight
    /// charges the game's, 15 off the arms, and the legs paying the whole
    /// once the arms were out (the refill's pause between them not
    /// compared); and its hand shake, 10 s for its two lacks of 15.
    /// While a pool pays nothing (a berserk) it does not work the
    /// charge out, and the lack is the last one's.
    pub fn fast_switch(&mut self, level: u32, now: f32) -> f32 {
        if !self.drains {
            return 0.0;
        }
        let cost = self.settings.fast_switch_cost / (level / 50 + 1) as f32;
        if !self.arms.forced() {
            self.switch_lack = (cost - self.arms.current).max(0.0);
        }
        let (unpaid, spent) = self.arms.charge(cost, Some(0.0), now);
        if spent {
            self.arms_ran_dry();
        }
        if unpaid > 0.0 {
            if !self.legs.forced() {
                self.switch_lack = (cost - self.arms.current).max(0.0);
            }
            if self.legs.charge(cost, Some(0.0), now).1 {
                self.legs_ran_dry();
            }
        }
        self.switch_lack
    }

    /// Holds the breath or lets it go (`Physical.HoldBreath`). Legs under
    /// the threshold do not hold it, nor does oxygen at or under it; and
    /// asked to on such legs nothing happens at all, a breath already held
    /// stays held. Gives back whether it is held now.
    pub fn hold_breath(&mut self, on: bool) -> bool {
        if !(self.legs_spent() && on) {
            if on && (!self.holds_back || self.oxygen.current > EXHAUSTED) {
                self.oxygen.run(Drain::HoldBreath);
            } else {
                self.oxygen.stop(Drain::HoldBreath);
            }
        }
        self.holding_breath()
    }

    pub fn holding_breath(&self) -> bool {
        self.oxygen.runs(Drain::HoldBreath)
    }

    /// How rested the hands count as for the weapon's breathing, 0.05 to
    /// 1: the arms' level, or the lower of the arms' and the legs' where
    /// the settings have exhaustion shake the hands (`BreathEffector`'s
    /// `StaminaLevel`; a broken arm, which caps it lower, is not here).
    /// What the arms lacked at the last fast switch (`Physical.
    /// FastWeaponSwitchStaminaLack`): the quick draw reads the put-away's
    /// before it pays its own (`SetAnimatorAndProceduralValues`).
    pub fn switch_lack(&self) -> f32 {
        self.switch_lack
    }

    pub fn breath_stamina(&self) -> f32 {
        /// The least the level counts as (`EnergyLowerLimit`).
        const LEAST: f32 = 0.05;
        if !self.holds_back {
            return 1.0;
        }
        let level = if self.settings.legs_shake_hands { self.arms.level().min(self.legs.level()) } else { self.arms.level() };
        level.clamp(LEAST, 1.0)
    }

    /// How much breath the character has for the same, 0 to 1: the
    /// oxygen's level, or the lower of that and the legs' where the
    /// settings have exhaustion deepen the breath (`OxygenLevel`).
    pub fn breath_oxygen(&self) -> f32 {
        if !self.holds_back {
            return 1.0;
        }
        if self.settings.legs_deepen_breath { self.oxygen.level().min(self.legs.level()) } else { self.oxygen.level() }
    }

    /// A mount's multiplier on the aiming arms' drain (mounting spec, "The
    /// weapon's motion while mounted"). The game works the cost out again as
    /// a mount is entered and left (`RecalculateErgonomic`, `Physical.Aim`).
    pub fn set_aim_drain_by(&mut self, by: f32) {
        if self.aim_drain_by != by {
            self.aim_drain_by = by;
            self.aim_cost = None;
        }
    }

    /// The handheld rangefinder in hand or not: the aim's rate its own
    /// (`AimRangeFinderDrainRate`) while it is.
    pub fn set_rangefinder(&mut self, on: bool) {
        if self.rangefinder != on {
            self.rangefinder = on;
            self.aim_cost = None;
        }
    }

    /// The sights up or down (`Physical.Aim`). The game calls this when
    /// the sights change and when the weapon is taken stock of again; a
    /// host may call it every frame, and only those two changes have the
    /// cost worked out afresh.
    pub fn aim(&mut self, on: bool) {
        if self.aim_taken != (on, self.aim_mass) {
            self.aim_taken = (on, self.aim_mass);
            self.aim_cost = None;
        }
        if on && self.aim_mass > 0.0 {
            self.arms.run(Drain::Aim);
            if self.arms.current <= 0.0 {
                self.arms_ran_dry();
            }
        } else {
            for pool in [&mut self.legs, &mut self.arms, &mut self.oxygen] {
                pool.stop(Drain::Aim);
            }
        }
    }
}
