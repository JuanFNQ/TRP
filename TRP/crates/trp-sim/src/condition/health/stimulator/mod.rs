//! Stimulators' buffs, and the buffs food and drink carry.
//! After the health spec, "Buffs": `ActiveHealthController.Stimulator`, its
//! `Buff`, `StimulatorSettings`, `StimulatorHelper`. An item naming a set of buffs (`StimulatorBuffs`)
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
use crate::condition::skills::Skill;

mod running;
mod readings;

pub use readings::*;

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

    /// Every member's name, which the texts are keyed by.
    pub fn names() -> impl Iterator<Item = &'static str> {
        BuffKind::NAMES.iter().map(|(_, n)| *n)
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
