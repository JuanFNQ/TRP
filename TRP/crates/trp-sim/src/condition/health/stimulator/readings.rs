//! What running stimulators give: listings, boosts, buffs and side effects.

use super::*;

impl Health {
    /// What the two icons of the stimulators list (`StimulatorHelper.
    /// GetDisplayableVariations`, `EffectsPanel.UpdateEffects`,
    /// `CharacterHealthPanel.UpdateActiveEffects`): for each stimulator at
    /// work, in the order made, its buffs at work in its set's order, the
    /// wanted ones (`side` false: the `IStimulatorBuff` icon) or the side
    /// effects (the `IStimulatorDebuff` icon), by `IsBuff` of their own
    /// figures; each with its seconds left (`BuffDescription.TimeLeft`: its
    /// delay and duration less the stimulator's whole time, rounded halves
    /// to the even; none where the two make nought). The words, and the
    /// merging of the lists by them, are the host's (`EffectDescription.
    /// Add`).
    pub fn stimulator_listing(&self, side: bool) -> Vec<Vec<(&BuffDef, Option<i32>)>> {
        self.effects
            .iter()
            .filter(|e| e.kind == Kind::Stimulator && e.active())
            .filter_map(|e| Some((e.whole, e.stim.as_ref()?)))
            .map(|(whole, s)| {
                s.buffs
                    .iter()
                    .filter(|b| b.at_work && b.personal.kind.is_buff(b.personal.value) != side)
                    .map(|b| {
                        let end = b.personal.delay + b.personal.duration;
                        let left = (end > 0.0 && end.is_finite()).then(|| (end - whole).round_ties_even() as i32);
                        (&b.personal, left)
                    })
                    .collect()
            })
            .collect()
    }

    /// What the buffs at work add to skills (`SkillRate`): each buff's value
    /// by its skill, for `SkillLevels::summary` (a relative one is taken as
    /// absolute: none of the install's items has one).
    pub fn skill_boosts(&self) -> Vec<(Skill, f32)> {
        self.at_work()
            .filter(|b| b.personal.kind == BuffKind::SkillRate)
            .filter_map(|b| Some((Skill::of_name(b.personal.skill.as_deref()?)?, b.value)))
            .collect()
    }

    /// What the buffs at work add to the stamina: to the legs' and arms'
    /// capacity (`MaxStamina`), and a refill a second (`StaminaRate`).
    pub fn stamina_buffs(&self) -> (f32, f32) {
        self.at_work().fold((0.0, 0.0), |(capacity, refill), b| match b.personal.kind {
            BuffKind::MaxStamina => (capacity + b.value, refill),
            BuffKind::StaminaRate => (capacity, refill + b.value),
            _ => (capacity, refill),
        })
    }

    /// What the weight's buffs make of every carrying limit
    /// (`CarryingWeightRelativeModifier`): one more than each weight effect's
    /// strength, multiplied, over those started.
    pub fn carry_modifier(&self) -> f32 {
        self.effects.iter().filter(|e| e.kind == Kind::Endurance && e.active()).map(|e| 1.0 + e.strength).product()
    }

    /// Whether a buff at work stops blood loss (`HasBloodLossBlockers`):
    /// damage then draws no bleeding and a wound does not open.
    pub fn blood_loss_blocked(&self) -> bool {
        self.at_work().any(|b| b.personal.kind == BuffKind::RemoveAllBloodLosses)
    }

    /// Whether an antidote's buff is at work (`HasStimulatorBuff(Antidote)`):
    /// no poisoning then begins.
    pub fn antidote_at_work(&self) -> bool {
        self.at_work().any(|b| b.personal.kind == BuffKind::Antidote)
    }

    /// A blow by a weapon with a poison of its own (`TryApplySideEffects`,
    /// after the blow's damage in `Player.ApplyDamageInfo`): where the blow
    /// took health (`did`), alive, and the weapon has a dose left, its set
    /// put on the head as a stimulator from that weapon, and a dose spent.
    /// Whether it was put on. The game's one such weapon is the cultists'
    /// knife (`Buffs_KultistsToxin`, two doses); nothing on the range
    /// strikes the player, so this is a host's to call.
    pub fn try_apply_side_effects(&mut self, did: f32, weapon: &str, side: &mut SideEffect) -> bool {
        if did <= 0.0 || !self.alive || side.empty() {
            return false;
        }
        self.add_stimulator(weapon, &side.buffs, 1.0);
        side.value = (side.value - 1.0).max(0.0);
        true
    }
}

/// A weapon's poison (`SideEffectComponent`): its set of buffs, the doses
/// left and the most it holds (`MaxResource`; none, never spent).
#[derive(Debug, Clone, PartialEq)]
pub struct SideEffect {
    pub buffs: String,
    pub value: f32,
    pub max: f32,
}

impl SideEffect {
    /// A weapon's template's (`KnifeTemplate.StimulatorBuffs`,
    /// `MaxResource`), full; none where it names no set.
    pub fn of(props: &Json) -> Option<Self> {
        let buffs = props.get("StimulatorBuffs").and_then(Json::as_str).filter(|s| !s.is_empty())?;
        let max = props.get("MaxResource").and_then(Json::as_f64).unwrap_or(0.0) as f32;
        Some(Self { buffs: buffs.to_owned(), value: max, max })
    }

    /// `IsEmpty`: under one dose, where it holds any.
    pub fn empty(&self) -> bool {
        self.max > 0.0 && self.value - 1.0 < 0.0
    }
}
