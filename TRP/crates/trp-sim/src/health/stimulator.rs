//! Stimulators' buffs, and the buffs food and drink carry (health spec,
//! "Buffs"; `ActiveHealthController.Stimulator`, its `Buff`; `StimulatorSettings`,
//! `StimulatorHelper`). An item naming a set of buffs (`StimulatorBuffs`)
//! puts a stimulator on the head when its use ends whole; each buff of the
//! set waits its delay on the stimulator's own clock, rolls its chance,
//! works its duration, and is undone; the stimulator ends with its last.
//! What the buffs do outside health (the skills, the stamina, the weight)
//! is read off it by the host (`skill_boosts`, `stamina_buffs`,
//! `carry_modifier`).

use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as Json;

use super::{num, DamageKind, Health, Kind, Part};
use crate::skills::Skill;

/// A buff's kind (`EStimulatorBuffType`), by the install's names: the
/// members' own, or the game's JSON names cut to thirteen letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffKind {
    HealthRate,
    EnergyRate,
    HydrationRate,
    SkillRate,
    MaxStamina,
    StaminaRate,
    StomachBloodloss,
    ContusionBlur,
    ContusionWiggle,
    Pain,
    HandsTremor,
    QuantumTunnelling,
    RemoveNegativeEffects,
    RemoveAllBuffs,
    RemoveAllBloodLosses,
    DamageModifier,
    WeightLimit,
    UnknownToxin,
    LethalToxin,
    Antidote,
    BodyTemperature,
    LightBleeding,
    HeavyBleeding,
    Fracture,
    Contusion,
    /// The events' own (a zombie's infection, frostbite, a misfire, the
    /// holiday's), which no item names.
    Other,
}

impl BuffKind {
    const NAMES: [(BuffKind, &'static str); 25] = [
        (BuffKind::HealthRate, "HealthRate"),
        (BuffKind::EnergyRate, "EnergyRate"),
        (BuffKind::HydrationRate, "HydrationRate"),
        (BuffKind::SkillRate, "SkillRate"),
        (BuffKind::MaxStamina, "MaxStamina"),
        (BuffKind::StaminaRate, "StaminaRate"),
        (BuffKind::StomachBloodloss, "StomachBloodloss"),
        (BuffKind::ContusionBlur, "ContusionBlur"),
        (BuffKind::ContusionWiggle, "ContusionWiggle"),
        (BuffKind::Pain, "Pain"),
        (BuffKind::HandsTremor, "HandsTremor"),
        (BuffKind::QuantumTunnelling, "QuantumTunnelling"),
        (BuffKind::RemoveNegativeEffects, "RemoveNegativeEffects"),
        (BuffKind::RemoveAllBuffs, "RemoveAllBuffs"),
        (BuffKind::RemoveAllBloodLosses, "RemoveAllBloodLosses"),
        (BuffKind::DamageModifier, "DamageModifier"),
        (BuffKind::WeightLimit, "WeightLimit"),
        (BuffKind::UnknownToxin, "UnknownToxin"),
        (BuffKind::LethalToxin, "LethalToxin"),
        (BuffKind::Antidote, "Antidote"),
        (BuffKind::BodyTemperature, "BodyTemperature"),
        (BuffKind::LightBleeding, "LightBleeding"),
        (BuffKind::HeavyBleeding, "HeavyBleeding"),
        (BuffKind::Fracture, "Fracture"),
        (BuffKind::Contusion, "Contusion"),
    ];

    /// By either name: the member's own, or its first thirteen letters.
    pub fn of_name(name: &str) -> BuffKind {
        BuffKind::NAMES.iter().find(|(_, n)| *n == name || (name.len() == 13 && n.starts_with(name))).map_or(BuffKind::Other, |(k, _)| *k)
    }

    /// The member's name, which the texts are keyed by.
    pub fn name(self) -> &'static str {
        BuffKind::NAMES.iter().find(|(k, _)| *k == self).map_or("Other", |(_, n)| n)
    }

    /// Whether a buff of this kind and value is wanted (`StimulatorHelper.
    /// IsBuff`): the rates, the skills, the stamina and the weight when not
    /// below nought; the damage taken and the temperature when not above;
    /// the cleansing ones always; the rest never (side effects).
    pub fn is_buff(self, value: f32) -> bool {
        match self {
            BuffKind::HealthRate | BuffKind::EnergyRate | BuffKind::HydrationRate | BuffKind::SkillRate | BuffKind::MaxStamina | BuffKind::StaminaRate | BuffKind::WeightLimit => value > -1e-45,
            BuffKind::DamageModifier | BuffKind::BodyTemperature => value < f32::EPSILON,
            BuffKind::RemoveNegativeEffects | BuffKind::RemoveAllBloodLosses | BuffKind::Antidote => true,
            _ => false,
        }
    }

    /// The kinds that act once and are done at once.
    fn once(self) -> bool {
        matches!(self, BuffKind::UnknownToxin | BuffKind::LethalToxin | BuffKind::LightBleeding | BuffKind::HeavyBleeding | BuffKind::Fracture | BuffKind::Contusion)
    }
}

/// One buff of a set (`StimulatorBuffSettings`).
#[derive(Debug, Clone, PartialEq)]
pub struct BuffDef {
    pub kind: BuffKind,
    pub chance: f32,
    pub delay: f32,
    pub duration: f32,
    pub value: f32,
    pub absolute: bool,
    /// The skill a `SkillRate` acts on, by the game's name.
    pub skill: Option<String>,
    /// The parts it may act on (a bleeding's, a fracture's, a
    /// contusion's); any where none are named.
    pub applies_to: Vec<Part>,
}

/// The install's sets (`config.Health.Effects.Stimulator`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StimulatorSettings {
    /// How often the rates act, in seconds (`BuffLoopTime`).
    pub loop_time: f32,
    pub sets: HashMap<String, Vec<BuffDef>>,
}

impl StimulatorSettings {
    /// From `Health.Effects`; none where the install has none.
    pub fn read(effects: &Json) -> Result<Self> {
        let Some(s) = effects.get("Stimulator") else { return Ok(Self::default()) };
        let mut sets = HashMap::new();
        if let Some(all) = s.get("Buffs").and_then(Json::as_object) {
            for (name, list) in all {
                let buffs = list
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .map(|b| {
                                let f = |k: &str| b.get(k).and_then(Json::as_f64).unwrap_or(0.0) as f32;
                                BuffDef {
                                    kind: BuffKind::of_name(b.get("BuffType").and_then(Json::as_str).unwrap_or("")),
                                    chance: f("Chance"),
                                    delay: f("Delay"),
                                    duration: f("Duration"),
                                    value: f("Value"),
                                    absolute: b.get("AbsoluteValue").and_then(Json::as_bool).unwrap_or(false),
                                    skill: b.get("SkillName").and_then(Json::as_str).filter(|s| !s.is_empty()).map(str::to_owned),
                                    applies_to: b
                                        .get("AppliesTo")
                                        .and_then(Json::as_array)
                                        .map(|parts| parts.iter().filter_map(Json::as_str).filter_map(Part::of_name).collect())
                                        .unwrap_or_default(),
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                sets.insert(name.clone(), buffs);
            }
        }
        Ok(Self { loop_time: num(s, "BuffLoopTime").unwrap_or(0.2), sets })
    }
}

/// One buff of a stimulator at work (`Stimulator.Buff`): its set's
/// figures and its own copy of them (`GetPersonalBuffSettings`), whether
/// it works or is done, the value it acts by, and what it made (a pain, a
/// tremor, the weight's or the damage's effect), ended with it.
#[derive(Debug, Clone, PartialEq)]
pub struct BuffRun {
    pub original: BuffDef,
    pub personal: BuffDef,
    pub at_work: bool,
    pub done: bool,
    pub value: f32,
    made: Option<u32>,
}

/// A stimulator's own state: its set, its item, its buffs, its clock and
/// the rates' loop.
#[derive(Debug, Clone, PartialEq)]
pub struct StimWork {
    pub set: String,
    pub template: String,
    pub buffs: Vec<BuffRun>,
    pub clock: f32,
    loop_timer: f32,
}

impl Health {
    /// A buff's own copy (`GetPersonalBuffSettings`): a side effect's
    /// duration cut by Metabolism and its value by Immunity, then every
    /// duration times the share of the item used. The delay stays.
    fn personal(&self, def: &BuffDef, strength: f32) -> BuffDef {
        let mut p = def.clone();
        if !def.kind.is_buff(def.value) {
            p.duration *= 1.0 - self.skills.metabolism_debuff;
            p.value *= 1.0 - self.skills.immunity_misc;
        }
        p.duration *= strength;
        p
    }

    /// The item's set put on the head as a stimulator (`MedEffect.Residue`,
    /// `AddStackableEffect<Stimulator>`), at the share of the item used.
    /// The same item's again while its stimulator is still there renews
    /// its buffs instead (`UpdateWithSameOne`, `Buff.IncreaseBySameBuff`):
    /// one at work gets the new share's duration and what it had left, up
    /// to its whole; one done waits its delay again from now and rolls
    /// again; one waiting is left.
    pub fn add_stimulator(&mut self, template: &str, set: &str, strength: f32) {
        let Some(defs) = self.settings.stimulator.sets.get(set).cloned() else { return };
        if !self.alive {
            return;
        }
        let same = self.effects.iter().position(|e| e.kind == Kind::Stimulator && e.existing() && e.stim.as_ref().is_some_and(|s| s.template == template));
        if let Some(i) = same {
            let debuff_cut = 1.0 - self.skills.metabolism_debuff;
            let Some(stim) = self.effects[i].stim.as_mut() else { return };
            let now = stim.clock;
            for b in &mut stim.buffs {
                let whole = if b.original.kind.is_buff(b.original.value) { b.original.duration } else { b.original.duration * debuff_cut };
                let fresh = whole * strength;
                if b.at_work {
                    let left = (b.personal.delay + b.personal.duration - now).clamp(0.0, b.personal.duration);
                    b.personal.delay = now;
                    b.personal.duration = (fresh + left).min(whole);
                } else if b.done {
                    b.personal.duration = fresh.min(whole);
                    b.personal.delay = now + b.original.delay;
                    b.done = false;
                }
            }
            return;
        }
        let buffs = defs.iter().map(|d| BuffRun { original: d.clone(), personal: self.personal(d, strength), at_work: false, done: false, value: 0.0, made: None }).collect();
        let work = StimWork { set: set.to_owned(), template: template.to_owned(), buffs, clock: 0.0, loop_timer: 0.0 };
        self.add_with(Kind::Stimulator, Part::Head, Some(0.0), None, None, Some(strength), move |e| e.stim = Some(Box::new(work)));
    }

    /// Started (at once): every buff of no delay goes to work.
    pub(super) fn stim_started(&mut self, id: u32) {
        let count = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).map_or(0, |s| s.buffs.len());
        for b in 0..count {
            let pending = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| !s.buffs[b].at_work && !s.buffs[b].done && s.buffs[b].personal.delay <= 0.0);
            if pending {
                self.activate(id, b, true);
            }
        }
    }

    /// Each frame while started (`Stimulator.RegularUpdate`): the clock
    /// runs; buffs whose delay is up go to work, those whose time is up are
    /// undone; every loop of the set's time the rates act once; with every
    /// buff done the stimulator ends.
    pub(super) fn stim_update(&mut self, id: u32, dt: f32) {
        let loop_time = self.settings.stimulator.loop_time;
        let Some(i) = self.index_of(id) else { return };
        let Some(stim) = self.effects[i].stim.as_mut() else { return };
        stim.clock += dt;
        stim.loop_timer += dt;
        let now = stim.clock;
        let count = stim.buffs.len();
        for b in 0..count {
            let due = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| !s.buffs[b].at_work && !s.buffs[b].done && now >= s.buffs[b].personal.delay);
            if due {
                self.activate(id, b, true);
            }
        }
        for b in 0..count {
            let over = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| s.buffs[b].at_work && now >= s.buffs[b].personal.delay + s.buffs[b].personal.duration);
            if over {
                self.activate(id, b, false);
            }
        }
        let looped = self.index_of(id).and_then(|i| self.effects[i].stim.as_mut()).is_some_and(|s| {
            let due = s.loop_timer >= loop_time;
            if due {
                s.loop_timer -= loop_time;
            }
            due
        });
        if looped {
            for b in 0..count {
                let rate = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).filter(|s| s.buffs[b].at_work).map(|s| (s.buffs[b].personal.kind, s.buffs[b].value));
                if let Some((kind, value)) = rate {
                    self.process(kind, value * loop_time);
                }
            }
        }
        let all_done = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| s.buffs.iter().all(|b| b.done));
        if all_done {
            self.force_residue(id);
        }
    }

    /// Residual (its end): every buff still at work undone.
    pub(super) fn stim_residue(&mut self, id: u32) {
        let count = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).map_or(0, |s| s.buffs.len());
        for b in 0..count {
            let working = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| s.buffs[b].at_work);
            if working {
                self.activate(id, b, false);
            }
        }
    }

    /// A buff to work, or undone (`ActivateBuff`): to work, its chance rolled
    /// (`Random.value >= Chance` and it is done, nothing happening), a side
    /// effect dodged by Immunity's elite (0.9), then its action; undone, its
    /// action taken back.
    fn activate(&mut self, id: u32, b: usize, on: bool) {
        let Some(buff) = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).map(|s| s.buffs[b].clone()) else { return };
        let p = &buff.personal;
        if on {
            let failed = self.rng.unit() >= p.chance;
            let dodged = !failed && !p.kind.is_buff(p.value) && self.skills.immunity_avoid > 0.0 && self.rng.unit() <= self.skills.immunity_avoid;
            if failed || dodged {
                self.set_buff(id, b, |r| r.done = true);
                return;
            }
            let value = self.evaluate(p);
            let made = self.act(p, value);
            self.set_buff(id, b, |r| {
                r.at_work = true;
                r.value = value;
                r.made = made;
            });
            if p.kind.once() {
                self.set_buff(id, b, |r| {
                    r.at_work = false;
                    r.done = true;
                });
            }
        } else {
            self.set_buff(id, b, |r| {
                r.at_work = false;
                r.done = true;
            });
            if let Some(made) = buff.made {
                self.force_residue(made);
            }
        }
    }

    fn set_buff(&mut self, id: u32, b: usize, change: impl FnOnce(&mut BuffRun)) {
        if let Some(r) = self.index_of(id).and_then(|i| self.effects[i].stim.as_mut()).and_then(|s| s.buffs.get_mut(b)) {
            change(r);
        }
    }

    /// The value a buff acts by (`Stimulator.EvaluateValue`): its own where
    /// absolute, else one more than it times what it is reckoned against
    /// (the head's most, energy's, hydration's; the stamina's base
    /// capacity and refill, 115 and 4.5). Only the rates, the skills and
    /// the stamina are reckoned so; the rest take the value as it is.
    /// (None of the install's items has a relative one of these.)
    fn evaluate(&self, p: &BuffDef) -> f32 {
        let reference = match p.kind {
            BuffKind::HealthRate => self.part(Part::Head).maximum,
            BuffKind::EnergyRate => self.energy[2],
            BuffKind::HydrationRate => self.hydration[2],
            BuffKind::MaxStamina => STAMINA_CAPACITY,
            BuffKind::StaminaRate => STAMINA_REFILL,
            // A skill's is reckoned against its level by the host (`skill_boosts`).
            _ => return p.value,
        };
        if p.absolute { p.value } else { (p.value + 1.0) * reference }
    }

    /// What a buff does as it goes to work (the cases of `ActivateBuff`), and
    /// the effect it made, which its end takes away.
    fn act(&mut self, p: &BuffDef, value: f32) -> Option<u32> {
        let allowed = |part: Part| p.applies_to.is_empty() || p.applies_to.contains(&part);
        match p.kind {
            BuffKind::WeightLimit => Some(self.add(Kind::Endurance, Part::Head, Some(0.0), None, None, Some(p.value))),
            BuffKind::DamageModifier => Some(self.add(Kind::DamageModifier, Part::Head, Some(0.0), None, None, Some(p.value.max(-0.99)))),
            BuffKind::Pain => Some(self.add(Kind::Pain, Part::Head, Some(0.0), None, None, Some(1.0))),
            BuffKind::HandsTremor if p.value >= 0.0 => Some(self.add(Kind::Tremor, Part::Head, None, None, None, None)),
            BuffKind::HandsTremor => {
                if let Some(t) = self.active_id(Kind::Tremor, Some(Part::Head)) {
                    self.force_residue(t);
                }
                None
            }
            BuffKind::QuantumTunnelling if p.value >= 0.0 => Some(self.add(Kind::TunnelVision, Part::Head, None, None, None, None)),
            BuffKind::QuantumTunnelling => {
                if let Some(t) = self.existing_id(Kind::TunnelVision, Some(Part::Head)) {
                    self.force_residue(t);
                }
                None
            }
            BuffKind::RemoveAllBloodLosses => {
                let bleedings: Vec<u32> = self.effects.iter().filter(|e| e.kind.bleeding() && e.existing()).map(|e| e.id).collect();
                for bleeding in bleedings {
                    self.force_residue(bleeding);
                }
                None
            }
            BuffKind::Contusion if allowed(Part::Head) => {
                self.do_contusion(p.duration, p.value);
                None
            }
            BuffKind::LightBleeding | BuffKind::HeavyBleeding if allowed(Part::Head) => {
                self.do_bleed(p.kind == BuffKind::HeavyBleeding, Part::Head);
                None
            }
            // An item's stimulator is on the head, which does not break.
            BuffKind::Fracture => None,
            // The rates act each loop; the skills and the stamina are read
            // off the buffs at work (`skill_boosts`, `stamina_buffs`).
            // Toxins, the antidote, the body's temperature, the stomach's
            // bleeding, the contusion's own screens and the cleansing ones
            // are not built (nothing on the range causes or reads them).
            _ => {
                let _ = value;
                None
            }
        }
    }

    /// A rate's loop (`ProcessBuff`): health to one part a loop (the parts in
    /// a random order, the first neither full nor destroyed healed, or the
    /// first not destroyed hurt, straight, and destroyed at nought), energy,
    /// hydration.
    fn process(&mut self, kind: BuffKind, amount: f32) {
        match kind {
            BuffKind::HealthRate => {
                let mut order = Part::ALL;
                for i in (1..order.len()).rev() {
                    let j = (self.rng.unit() * (i + 1) as f32) as usize;
                    order.swap(i, j.min(i));
                }
                if amount > 0.0 {
                    if let Some(part) = order.into_iter().find(|p| {
                        let s = self.part(*p);
                        !s.destroyed && s.current < s.maximum
                    }) {
                        self.change_health(part, amount);
                    }
                } else if amount < 0.0 {
                    if let Some(part) = order.into_iter().find(|p| !self.part(*p).destroyed) {
                        if self.part(part).current > 0.0 {
                            self.change_health(part, amount);
                        }
                        if self.part(part).current <= 0.0 {
                            self.destroy(part, DamageKind::Stimulator);
                        }
                    }
                }
            }
            BuffKind::EnergyRate => self.change_energy(amount),
            BuffKind::HydrationRate => self.change_hydration(amount),
            _ => {}
        }
    }

    /// The buffs at work, of every stimulator.
    fn at_work(&self) -> impl Iterator<Item = &BuffRun> {
        self.effects.iter().filter(|e| e.kind == Kind::Stimulator).filter_map(|e| e.stim.as_ref()).flat_map(|s| s.buffs.iter()).filter(|b| b.at_work)
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
}

/// The stamina's base capacity and refill a relative buff is reckoned
/// against (`Physical.StaminaCapacity`, `StaminaRestoreRate`: the globals'
/// `Capacity` and `BaseRestorationRate`, 115 and 4.5 in the install).
/// PROVISIONAL: taken as those figures; no item's buff is relative.
const STAMINA_CAPACITY: f32 = 115.0;
const STAMINA_REFILL: f32 = 4.5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_names_are_read() {
        assert_eq!(BuffKind::of_name("QuantumTunnelling"), BuffKind::QuantumTunnelling);
        assert_eq!(BuffKind::of_name("QuantumTunnel"), BuffKind::QuantumTunnelling);
        assert_eq!(BuffKind::of_name("RemoveAllBloo"), BuffKind::RemoveAllBloodLosses);
        assert_eq!(BuffKind::of_name("Frostbite"), BuffKind::Other);
    }

    #[test]
    fn wanted_or_not_by_kind_and_sign() {
        assert!(BuffKind::SkillRate.is_buff(10.0) && !BuffKind::SkillRate.is_buff(-10.0));
        assert!(BuffKind::DamageModifier.is_buff(-0.2) && !BuffKind::DamageModifier.is_buff(0.2));
        assert!(!BuffKind::HandsTremor.is_buff(0.0) && BuffKind::RemoveAllBloodLosses.is_buff(0.0));
    }
}
