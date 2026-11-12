//! Part states, the conditions the body is in, events, refusals and health skills.

use super::*;

/// A part's state (`BodyPartState`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartState {
    pub current: f32,
    pub maximum: f32,
    pub destroyed: bool,
}

/// What the character can and cannot do because of its health
/// (`EPhysicalCondition`, as `Player` sets them on the movement context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Conditions {
    /// A painkiller or a berserk at work (`UpdateSpeedLimitByHealth`); the
    /// arms' damage goes by a painkiller alone (`UpdateArmsCondition`).
    pub on_painkillers: bool,
    pub left_leg: bool,
    pub right_leg: bool,
    pub left_arm: bool,
    pub right_arm: bool,
    pub tremor: bool,
    pub using_meds: bool,
    pub healing_legs: bool,
}

impl Conditions {
    pub fn leg_damaged(&self) -> bool {
        self.left_leg || self.right_leg
    }

    /// `MovementContext.CanSprint`.
    pub fn can_sprint(&self) -> bool {
        if self.using_meds || self.healing_legs {
            return false;
        }
        self.on_painkillers || !self.leg_damaged()
    }

    /// Whether the stamina's sprint drain may start (`Physical`'s sprint
    /// availability): no medicine at work, and no damaged leg unless a
    /// painkiller is at work.
    pub fn can_sprint_drain(&self) -> bool {
        !self.using_meds && (!self.leg_damaged() || self.on_painkillers)
    }

    /// `MovementContext.CanJump`.
    pub fn can_jump(&self) -> bool {
        !self.using_meds && !self.healing_legs
    }

    /// `MovementContext.CanWalk`, health's part.
    pub fn can_walk(&self) -> bool {
        !self.healing_legs
    }

    /// `MovementContext.CanProne`, health's part.
    pub fn can_prone(&self) -> bool {
        !self.using_meds && !self.healing_legs
    }

    /// `MovementContext.CanMoveInProne`, health's part.
    pub fn can_crawl(&self) -> bool {
        !self.using_meds && !self.healing_legs
    }

    /// The speed limit health puts on the character, if any
    /// (`ESpeedLimit.HealthCondition`).
    pub fn speed_limit(&self) -> Option<f32> {
        if !self.leg_damaged() || self.on_painkillers {
            None
        } else if self.left_leg && self.right_leg {
            Some(0.2)
        } else {
            Some(0.3)
        }
    }

    /// What aiming's stamina cost is multiplied by (`Physical`'s aim drain).
    pub fn aim_cost(&self) -> f32 {
        if self.on_painkillers {
            1.0
        } else if self.left_arm && self.right_arm {
            2.0
        } else if self.left_arm || self.right_arm {
            1.5
        } else {
            1.0
        }
    }

    /// How far the sway spring is toward the damaged one's: 0, 0.5 or 1.
    pub fn sway_damage(&self) -> f32 {
        match (self.left_arm, self.right_arm) {
            (true, true) => 1.0,
            (false, false) => 0.0,
            _ => 0.5,
        }
    }

    /// The breathing's fracture (`BreathEffector.Fracture`).
    pub fn arms_fractured(&self) -> bool {
        self.left_arm || self.right_arm
    }

    /// The breathing's tremor (`BreathEffector.TremorOn`).
    pub fn tremor_on(&self) -> bool {
        self.tremor && !self.on_painkillers
    }

    /// The walk's bob is the lame one.
    pub fn lame(&self) -> bool {
        self.leg_damaged() && !self.on_painkillers
    }

    /// A vault's or a climb's slow end (`MovementContext.SetVaultingSpeed`):
    /// the figure its speed range is read at, before the weight's. `Some`
    /// only where the obstacle is high enough for a limb to matter.
    pub fn vault_slowing(&self, climb: bool, height: f32, without_hand: f32, one_hand: f32) -> f32 {
        let slow = if self.on_painkillers { 0.75 } else { 1.0 };
        let mut b = 0.0;
        if height > without_hand {
            if self.leg_damaged() || self.left_arm {
                b = slow;
            }
            if climb && self.right_arm && height > one_hand {
                b = slow;
            }
        }
        b
    }
}

/// Something that happened, for the host.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// An effect started (`EffectStartedEvent`).
    Started(Kind, Part),
    /// An effect went residual (`EffectResidualEvent`).
    Residual(Kind, Part),
    /// An effect was taken off (`EffectRemovedEvent`): for medicine, the
    /// hands go on to the next part or finish.
    Removed(Kind, Part, u32),
    Damaged(Part, f32, DamageKind),
    Destroyed(Part, DamageKind),
    Restored(Part),
    Died(DamageKind),
    /// What an item at work holds now.
    Resource(u64, f32),
    /// Health gained by a part, by the change (`HealthChangedEvent` with a
    /// positive difference: medicine, food, a stimulant's regeneration):
    /// the experience of healing (`HealthStatisticsManager.OnHealthChanged`).
    Healed(Part, f32),
    /// Energy and hydration gained, by the change after their limits
    /// (`EnergyChangedEvent`, `HydrationChangedEvent` with a positive one):
    /// Metabolism's points and their experience.
    EnergyGained(f32),
    HydrationGained(f32),
    /// An item's use over with what it gained of energy and of hydration
    /// kept (not cut short within the interruption's time, which takes
    /// them back): the whole use's gains, each as one change, as the game's
    /// controller out of a raid applies an item at once and tells each
    /// change once (`OfflineHealthController`'s `MedEffect`). What
    /// Metabolism earns by with simple XP gain (`earning::metabolism`).
    Fed { energy: f32, hydration: f32 },
    /// An effect a medicine put right (`HealerDoneEvent` of the effect it
    /// took to residue): its treatment's experience.
    Cured(Kind, Part),
    /// An item at work used up: it is gone.
    UsedUp(u64),
}

/// How a use of an item was refused (`FailedResult`'s word).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// `Health/PlayerIsDead`.
    Dead,
    /// `Inventory/IncompatibleItem`: it can do nothing, or something is at
    /// work already.
    Incompatible,
    /// `Health/ItemResourceDepleted`.
    Depleted,
}

/// What the character's skills do to its health (`SkillManager`'s buffs,
/// each by the skill's level: its progress over 100, at most 51; elite
/// above 50). All 0 untrained, as the app's character is; the health
/// replay gives the recorded character's (`bin/health_replay`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HealthSkills {
    /// Stress Resistance's `StressPain`, 0.01 a level: pain's residue,
    /// working time and strength cut by it (`Pain.Init`).
    pub stress_pain: f32,
    /// Health's `HealthEnergy` and `HealthHydration`, 0.006 a level: what
    /// existence drains, cut by them (`Existence.GetEnergyDamage`).
    pub health_energy: f32,
    pub health_hydration: f32,
    /// Health's `HealthBreakChanceRed`, 0.012 a level: the top of a
    /// fracture's odds raised by it.
    pub break_chance: f32,
    /// Vitality's `VitalityBuffBleedChanceRed`, 0.012 a level: the top of a
    /// bleeding's odds raised by it.
    pub bleed_chance: f32,
    /// Metabolism's `MetabolismRatioPlus`, 0.01 a level: what food and drink
    /// give, raised by it; Immunity's `ImmunityMiscEffects`, 0.01 a level:
    /// what they take, cut by it.
    pub metabolism_ratio_plus: f32,
    pub immunity_misc: f32,
    /// Immunity's `ImmunityPainKiller`, 0.006 a level: a painkiller's time
    /// raised by it.
    pub immunity_painkiller: f32,
    /// Surgery's `SurgerySpeed` (0.2 at level 50, 0.4 elite): a destroyed
    /// part's working time divided by one more than it; its
    /// `SurgeryReducePenalty` (0.01 a level, 1 elite): a mended part's kept
    /// share drawn that far toward all.
    pub surgery_speed: f32,
    pub surgery_penalty: f32,
    /// Vitality's elite (`VitalityBuffBleedStop`): a bleeding works for its
    /// `EliteVitalityDuration` (light 22 s, heavy 33 s here) and stops.
    pub bleed_stop: bool,
    /// Metabolism's elite (`MetabolismEliteBuffNoDyhydration`): strong
    /// dehydration and exhaustion do no damage.
    pub no_starvation_damage: bool,
    /// Metabolism's `MetabolismMiscDebuffTime`, 0.01 a level: a stimulator's
    /// side effects' time cut by it; Immunity's elite
    /// `ImmunityAvoidMiscEffectsChance` (0.9): the share of side effects
    /// dodged (health spec, "Buffs").
    pub metabolism_debuff: f32,
    pub immunity_avoid: f32,
    /// Immunity's `ImmunityPoisonBuff`, 0.01 a level: an intoxication's
    /// damage a loop cut by it (`Intoxication.Started`); its elite
    /// `ImmunityAvoidPoisonChance` (0.9): the share of poisonings kept
    /// off (`DoIntoxication`).
    pub poison_cut: f32,
    pub poison_avoid: f32,
    /// Stress Resistance's elite (`StressBerserk`): low health, as it
    /// starts, adds a berserk of strength 2 (`LowEdgeHealth.Started`).
    pub stress_berserk: bool,
}

impl HealthSkills {
    /// The buffs from the character's skills.
    pub fn of(levels: &crate::condition::skills::SkillLevels) -> Self {
        Self::of_levels(|name| levels.level_of(name))
    }

    /// The buffs from the skills' levels, by the game's names (`ESkillId`):
    /// each rule worked with the level held at 50 (`Skill.UpdateRules`),
    /// the elites above it.
    pub fn of_levels(level: impl Fn(&str) -> u32) -> Self {
        let at = |name: &str| level(name).min(50) as f32;
        let elite = |name: &str| level(name) > 50;
        Self {
            stress_pain: at("StressResistance") * 0.01,
            health_energy: at("Health") * 0.006,
            health_hydration: at("Health") * 0.006,
            break_chance: at("Health") * 0.012,
            bleed_chance: at("Vitality") * 0.012,
            metabolism_ratio_plus: at("Metabolism") * 0.01,
            immunity_misc: at("Immunity") * 0.01,
            immunity_painkiller: at("Immunity") * 0.006,
            surgery_speed: if elite("Surgery") { 0.4 } else { at("Surgery") * 0.2 / 50.0 },
            surgery_penalty: if elite("Surgery") { 1.0 } else { at("Surgery") * 0.01 },
            bleed_stop: elite("Vitality"),
            no_starvation_damage: elite("Metabolism"),
            metabolism_debuff: at("Metabolism") * 0.01,
            immunity_avoid: if elite("Immunity") { 0.9 } else { 0.0 },
            poison_cut: at("Immunity") * 0.01,
            poison_avoid: if elite("Immunity") { 0.9 } else { 0.0 },
            stress_berserk: elite("StressResistance"),
        }
    }
}
