//! Repairing a thing with a repair kit (`docs/spec/workbench.md`, "The
//! repair window"): what the game's repair window shows of it, by the
//! client's own reckoning (`RepairController`, `DefaultRepairStrategy`,
//! `ArmorRepairStrategy`, `RepairKitsCollection`, `RepairerParametersPanel`),
//! and what a repair does to the thing and the kits, by the server the
//! user's install runs (SPT's `RepairService.RepairItemByKit`,
//! `RepairHelper.UpdateItemDurability`: the client computes no result of
//! its own, only the window's preview).
//!
//! A weapon is one part, itself; an armour item (one that holds plates or
//! inserts, or is armour itself) is its armour parts: what wears in it
//! (`RepairableComponent`s), each by its own material and class.

use std::collections::HashMap;

use serde_json::Value as Json;

use crate::inventory::{is, Id, Item};

/// The skill that governs a part's repair (`RepairController.HasRepairBonus`):
/// a weapon's Weapon Maintenance; an armour part's Light or Heavy Vests by
/// its `ArmorType`, else Intellect (which has no buffs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Governing {
    WeaponTreatment,
    LightVests,
    HeavyVests,
    Intellect,
}

/// The globals' and the skills' figures (`RepairSettings`, `ArmorMaterials`,
/// `SkillsSettings`), and the hideout's greatest repair bonuses.
#[derive(Clone, Debug)]
pub struct Settings {
    pub cost_guns: f32,
    pub cost_armor: f32,
    pub armor_class_divisor: f32,
    pub min_level_to_buff: u32,
    /// Each armour material's degradation by a kit and by a trader
    /// (`MinRepairKitDegradation` and `Max...`; `MinRepairDegradation` and
    /// `Max...`), and its destructibility.
    pub materials: HashMap<String, ([f32; 2], [f32; 2], f32)>,
    /// The wear a repair takes less a level, by the governing skill
    /// (`WearAmountRepairGunsReducePerLevel`, `...LVests...`, `...HVests...`,
    /// Intellect's `WearAmountReducePerLevel`).
    pub wear_per_level: [f32; 4],
    /// Intellect's points' cost cut a level (`RepairPointsCostReduction`).
    pub intellect_cost: f32,
    /// The buff's chance figures of Weapon Maintenance, Light and Heavy
    /// Vests (`CommonBuffMinChanceValue`, `CommonBuffChanceLevelBonus`,
    /// `ReceivedDurabilityMaxPercent`).
    pub buff: [[f32; 3]; 3],
    /// The hideout's greatest bonus to a weapon's and an armour's repair,
    /// as a fraction (`Hideout.GetMaxBonus`: every stage's
    /// `RepairWeaponBonus`, `RepairArmorBonus` summed).
    pub max_bonus: [f32; 2],
}

impl Settings {
    /// From the install's globals and, where given, its hideout's areas.
    pub fn from_config(config: &Json, areas: Option<&Json>) -> Self {
        let config = config.get("config").unwrap_or(config);
        let f = |value: Option<&Json>, default: f32| value.and_then(Json::as_f64).map_or(default, |v| v as f32);
        let repair = config.get("RepairSettings");
        let skills = config.get("SkillsSettings");
        let skill = |name: &str, field: &str, default: f32| f(skills.and_then(|s| s.get(name)).and_then(|s| s.get(field)), default);
        let buff = |name: &str| {
            let settings = skills.and_then(|s| s.get(name)).and_then(|s| s.get("BuffSettings"));
            [f(settings.and_then(|b| b.get("CommonBuffMinChanceValue")), 0.0), f(settings.and_then(|b| b.get("CommonBuffChanceLevelBonus")), 0.0), f(settings.and_then(|b| b.get("ReceivedDurabilityMaxPercent")), 30.0)]
        };
        let materials = config
            .get("ArmorMaterials")
            .and_then(Json::as_object)
            .map(|all| {
                all.iter()
                    .map(|(name, m)| {
                        let n = |field: &str| f(m.get(field), 0.0);
                        (name.clone(), ([n("MinRepairKitDegradation"), n("MaxRepairKitDegradation")], [n("MinRepairDegradation"), n("MaxRepairDegradation")], n("Destructibility")))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Every area's every stage's bonus of the kind, summed, in hundredths.
        let bonus = |kind: &str| -> f32 {
            let Some(areas) = areas.and_then(Json::as_array) else { return 0.0 };
            let stages = areas.iter().filter_map(|area| area.get("stages")?.as_object());
            let bonuses = stages.flat_map(|stages| stages.values()).filter_map(|stage| stage.get("bonuses")?.as_array()).flatten();
            bonuses.filter(|b| b.get("type").and_then(Json::as_str) == Some(kind)).filter_map(|b| b.get("value")?.as_f64()).sum::<f64>() as f32 / 100.0
        };
        Self {
            cost_guns: f(repair.and_then(|r| r.get("durabilityPointCostGuns")), 0.5),
            cost_armor: f(repair.and_then(|r| r.get("durabilityPointCostArmor")), 0.335),
            armor_class_divisor: f(repair.and_then(|r| r.get("armorClassDivisor")), 6.0),
            min_level_to_buff: f(repair.and_then(|r| r.get("MinimumLevelToApplyBuff")), 3.0) as u32,
            materials,
            wear_per_level: [
                skill("WeaponTreatment", "WearAmountRepairGunsReducePerLevel", 0.01),
                skill("LightVests", "WearAmountRepairLVestsReducePerLevel", 0.008),
                skill("HeavyVests", "WearAmountRepairHVestsReducePerLevel", 0.01),
                skill("Intellect", "WearAmountReducePerLevel", 0.004),
            ],
            intellect_cost: skill("Intellect", "RepairPointsCostReduction", 0.004),
            buff: [buff("WeaponTreatment"), buff("LightVests"), buff("HeavyVests")],
            max_bonus: [bonus("RepairWeaponBonus"), bonus("RepairArmorBonus")],
        }
    }
}

/// One part that wears, as a repair goes by it.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub id: Id,
    pub durability: f32,
    pub max: f32,
    /// Its template's greatest durability (100 for every weapon:
    /// `WeaponTemplate`'s repairable figures).
    pub template_max: f32,
    /// What a kit's repair and a trader's take of its greatest durability,
    /// as shares of the template's ([least, most]).
    pub kit: [f32; 2],
    pub trader: [f32; 2],
    /// Kit points a durability point, before the bonuses: a weapon's
    /// `durabilityPointCostGuns`; an armour part's `durabilityPointCostArmor`
    /// times one plus its material's destructibility times one plus its
    /// class over `armorClassDivisor`.
    pub cost: f32,
    pub governing: Governing,
}

/// What the character brings to a repair: the governing skills' levels,
/// Intellect's and Charisma's, and the hideout's bonus to it (a fraction).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Repairer {
    pub levels: [u32; 4],
    pub intellect: u32,
    pub charisma: u32,
    pub bonus: f32,
}

const ELITE: u32 = crate::skills::ELITE;

impl Repairer {
    fn level(&self, governing: Governing) -> u32 {
        self.levels[governing as usize]
    }
}

/// A thing's repair as the window reckons it (`IRepairStrategy`).
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub parts: Vec<Part>,
    pub armour: bool,
}

/// A thing's parts that wear, in the order a repair spends on them; none
/// where nothing in it wears.
pub fn plan_of(item: &Item, db: &trp_data::Database, settings: &Settings) -> Option<Plan> {
    let prop = |template: &str, name: &str| db.items.get(template).and_then(|t| t.props.get(name).cloned());
    let number = |template: &str, name: &str| prop(template, name).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let armour = item.kind.armor_class > 0 || item.kind.slots.iter().any(|slot| slot.armour);
    if !armour {
        let wear = item.repairable?;
        let template = &item.kind.template;
        let governing = if is(item, "Weapon") { Governing::WeaponTreatment } else { Governing::Intellect };
        let template_max = if is(item, "Weapon") { 100.0 } else { item.kind.durability[1].max(1.0) };
        let part = Part {
            id: item.id,
            durability: wear.durability,
            max: wear.max_durability,
            template_max,
            kit: [number(template, "MinRepairKitDegradation"), number(template, "MaxRepairKitDegradation")],
            trader: [number(template, "MinRepairDegradation"), number(template, "MaxRepairDegradation")],
            cost: settings.cost_guns,
            governing,
        };
        return Some(Plan { parts: vec![part], armour: false });
    }
    // An armour's parts: its plates and inserts in its slots first, then
    // itself (`OrderRepair` puts plates in unlocked slots first, then goes
    // by each part's first collider: PROVISIONAL, the colliders' order is
    // not applied).
    let mut parts = Vec::new();
    let mut add = |thing: &Item| {
        let Some(wear) = thing.repairable.filter(|_| thing.kind.armor_class > 0) else { return };
        let template = &thing.kind.template;
        let material = prop(template, "ArmorMaterial").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        let (kit, trader, destructibility) = settings.materials.get(&material).copied().unwrap_or(([0.0; 2], [0.0; 2], 0.0));
        let governing = match prop(template, "ArmorType").and_then(|v| v.as_str().map(str::to_owned)).as_deref() {
            Some("Light") => Governing::LightVests,
            Some("Heavy") => Governing::HeavyVests,
            _ => Governing::Intellect,
        };
        let cost = settings.cost_armor * (1.0 + destructibility) * (1.0 + thing.kind.armor_class as f32 / settings.armor_class_divisor);
        parts.push(Part { id: thing.id, durability: wear.durability, max: wear.max_durability, template_max: thing.kind.durability[1].max(1.0), kit, trader, cost, governing });
    };
    for plate in item.slots.iter().zip(&item.kind.slots).filter(|(_, slot)| slot.armour).filter_map(|(part, _)| part.as_ref()) {
        add(plate);
    }
    add(item);
    (!parts.is_empty()).then_some(Plan { parts, armour: true })
}

impl Plan {
    /// The template durability the window's bar is measured by: the parts'
    /// summed (`TemplateDurability`).
    pub fn template_durability(&self) -> f32 {
        self.parts.iter().map(|part| part.template_max).sum()
    }

    pub fn durability(&self) -> f32 {
        self.parts.iter().map(|part| part.durability).sum()
    }

    pub fn max_durability(&self) -> f32 {
        self.parts.iter().map(|part| part.max).sum()
    }

    /// The repair's quality (`GetBaseRepairQuality`): one less the governing
    /// skill's wear cut; and as the window words it
    /// (`GetBaseRepairQualityForDisplay`), from 1 untrained to 0.5 elite.
    pub fn quality(&self, settings: &Settings, who: &Repairer) -> (f32, f32) {
        let governing = self.parts.first().map_or(Governing::Intellect, |part| part.governing);
        let per = settings.wear_per_level[governing as usize];
        let wear = who.level(governing) as f32 * per;
        let most = 50.0 * per;
        (1.0 - wear, if most > 0.0 { (most - wear) / (2.0 * most) + 0.5 } else { 1.0 })
    }

    /// A part's degradation range in points (`GetDegradationValues`): the
    /// quality times its template's durability times its kit's shares; the
    /// least nought elite at its skill (Weapon Maintenance, Light or Heavy
    /// Vests).
    fn part_range(part: &Part, settings: &Settings, who: &Repairer) -> [f32; 2] {
        let per = settings.wear_per_level[part.governing as usize];
        let q = 1.0 - who.level(part.governing) as f32 * per;
        let elite = part.governing != Governing::Intellect && who.level(part.governing) >= ELITE;
        [if elite { 0.0 } else { q * part.template_max * part.kit[0] }, q * part.template_max * part.kit[1]]
    }

    /// What can be repaired in all (`HowMuchRepairScoresCanAccept`): each
    /// part's greatest less its least loss less its durability, at least
    /// nought, summed.
    pub fn can_accept(&self, settings: &Settings, who: &Repairer) -> f32 {
        self.parts.iter().map(|part| (part.max - Self::part_range(part, settings, who)[0] - part.durability).max(0.0)).sum()
    }

    /// The amount spent over the parts in order: each part's share.
    fn spent(&self, amount: f32, settings: &Settings, who: &Repairer) -> Vec<f32> {
        let mut left = amount;
        self.parts
            .iter()
            .map(|part| {
                let need = (part.max - Self::part_range(part, settings, who)[0] - part.durability).max(0.0);
                let take = left.min(need);
                left -= take;
                take
            })
            .collect()
    }

    /// The greatest durability a repair of that amount may take away, its
    /// least and most (`GetDegradationValues`; an armour's summed over the
    /// parts the amount reaches that still need 0.1 or more), each within
    /// nought and the greatest durability.
    pub fn degradation(&self, amount: f32, settings: &Settings, who: &Repairer) -> [f32; 2] {
        let mut range = [0.0f32; 2];
        if self.armour {
            let mut left = amount;
            for part in &self.parts {
                let need = (part.max - Self::part_range(part, settings, who)[0] - part.durability).max(0.0);
                if left <= 0.0 || need < 0.1 {
                    continue;
                }
                left -= need;
                let r = Self::part_range(part, settings, who);
                range = [range[0] + r[0], range[1] + r[1]];
            }
        } else if let Some(part) = self.parts.first() {
            range = Self::part_range(part, settings, who);
        }
        let most = self.max_durability();
        range.map(|v| v.clamp(0.0, most))
    }

    /// The bonuses' cut of the points (`1 - b - i`).
    fn cut(&self, who: &Repairer, settings: &Settings) -> f32 {
        1.0 - who.bonus - who.intellect as f32 * settings.intellect_cost
    }

    /// The kit points a repair of that amount costs (`GetRepairPrice`).
    pub fn price(&self, amount: f32, settings: &Settings, who: &Repairer) -> f64 {
        let cut = self.cut(who, settings) as f64;
        if !self.armour {
            return amount as f64 * self.parts.first().map_or(0.0, |part| part.cost) as f64 * cut;
        }
        self.parts.iter().zip(self.spent(amount, settings, who)).map(|(part, d)| d as f64 * part.cost as f64 * cut).sum()
    }

    /// The price rate's line (`GetBaseRepairPriceCoefficient`): the
    /// greatest bonus and wear cut against the character's, to 120.
    pub fn price_rate(&self, settings: &Settings, who: &Repairer) -> f64 {
        let governing = self.parts.first().map_or(Governing::Intellect, |part| part.governing);
        let per = settings.wear_per_level[governing as usize] as f64;
        let max_bonus = settings.max_bonus[usize::from(self.armour)] as f64;
        let most = max_bonus + 50.0 * per;
        let now = who.bonus as f64 + who.level(governing) as f64 * per;
        if most > 0.0 { (most - now) * 120.0 / most } else { 0.0 }
    }

    /// The repairability's words (`RepairabilityText`), by the trader's
    /// degradation even for a kit; an armour's its parts' averaged.
    pub fn repairability(&self) -> &'static str {
        let n = self.parts.len().max(1) as f32;
        let [low, high] = self.parts.iter().fold([0.0f32; 2], |sum, part| [sum[0] + part.trader[0] / n, sum[1] + part.trader[1] / n]);
        let mean = (low + high) / 2.0;
        if high - low > 0.5 {
            return "Unpredictable";
        }
        match mean {
            m if m < 0.05 => "Very good",
            m if m < 0.11 => "Good",
            m if m < 0.19 => "Average",
            m if m < 0.32 => "Bad",
            m if m < 0.55 => "Very bad",
            m if m < 0.85 => "Terrible",
            _ => "Unpredictable",
        }
    }

    /// The chance of an enhancement (`GetEnhancementBonus`), where one may
    /// come (the governing skill at `MinimumLevelToApplyBuff` and with buffs):
    /// `(min + bonus * level) * clamp(share / max percent, 0.01, 1)`.
    pub fn enhancement_chance(&self, amount: f32, settings: &Settings, who: &Repairer) -> Option<f64> {
        let governing = self.parts.first()?.governing;
        let figures = match governing {
            Governing::WeaponTreatment => settings.buff[0],
            Governing::LightVests => settings.buff[1],
            Governing::HeavyVests => settings.buff[2],
            Governing::Intellect => return None,
        };
        let level = who.level(governing);
        if level < settings.min_level_to_buff {
            return None;
        }
        let share = (amount / self.template_durability().max(1.0)) / figures[2];
        Some(((figures[0] + figures[1] * level as f32) * share.clamp(0.01, 1.0)) as f64)
    }

    /// Every part at a durability of one or less (`BrokenItemError`).
    pub fn broken(&self) -> bool {
        self.parts.iter().all(|part| part.durability <= 1.0)
    }
}

/// The window's words for a quality (`GetRepairQualityRate`), a price
/// rate (`GetRepairPriceRateQuality`) and an enhancement chance
/// (`GetEnhancementChance`), as the install's texts' keys.
pub fn quality_words(quality: f32) -> &'static str {
    match quality {
        q if q >= 1.1 => "Terrible",
        q if q >= 0.9 => "Subpar",
        q if q >= 0.8 => "Average",
        q if q >= 0.7 => "Fine",
        q if q <= 0.7 => "Excellent",
        _ => "Unknown",
    }
}

pub fn price_rate_words(rate: f64) -> &'static str {
    match rate {
        r if r >= 100.0 => "very bad",
        r if r >= 80.0 => "bad",
        r if r >= 50.0 => "fine",
        r if r >= 30.0 => "good",
        _ => "very good",
    }
}

pub fn enhancement_words(chance: f64) -> &'static str {
    match chance {
        c if c < 0.25 => "enhancementChance/very low",
        c if c < 0.5 => "enhancementChance/low",
        c if c < 0.75 => "enhancementChance/average",
        _ => "enhancementChance/high",
    }
}

/// What a repair did: each part's durability and greatest after it, each
/// kit's resource left (none: the kit is used up and gone), and the
/// durability the window says it was repaired to.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub parts: Vec<(Id, f32, f32)>,
    pub kits: Vec<(Id, Option<f32>)>,
    pub repaired_to: f32,
}

/// A repair of that amount paid with the kits in that order (their ids and
/// resources: the one let go first, then the least first,
/// `RepairKitsCollection.RepairItems`), as the server makes it: each part's
/// points (`GetRepairPrice` of its share) taken from the kits in turn, a kit
/// that holds what is left paying it all, a smaller one all it has; the
/// durability added the **first** kit's points over the part's cost
/// (`RepairItemByKit`: only the first kit's count is converted); a kit its
/// points use up removed, one that has more lowered and the rest unpaid
/// (its loop ends there); then the durability capped at the greatest, and
/// the greatest less a draw between the kit's degradation shares of it (a
/// weapon's template's, a nought most falling back to the trader's; an
/// armour part's material's), at a durability capped again (elite Charisma
/// skips it half the time). `draw` gives draws in [0, 1).
pub fn repair(plan: &Plan, amount: f32, kits: &[(Id, f32)], settings: &Settings, who: &Repairer, draw: &mut dyn FnMut() -> f32) -> Outcome {
    let cut = plan.cut(who, settings);
    let shares = if plan.armour { plan.spent(amount, settings, who) } else { vec![amount] };
    let mut left: Vec<(Id, f32)> = kits.to_vec();
    let mut gone: Vec<Id> = Vec::new();
    let mut parts = Vec::new();
    for (part, share) in plan.parts.iter().zip(shares) {
        if share <= 0.0 {
            parts.push((part.id, part.durability, part.max));
            continue;
        }
        let points = share * part.cost * cut;
        // The client's split of the points among the kits.
        let mut owed = points;
        let mut counts: Vec<(usize, f32)> = Vec::new();
        for (at, (_, resource)) in left.iter().enumerate().filter(|(_, (id, _))| !gone.contains(id)) {
            if owed <= 0.0 {
                break;
            }
            let pay = owed.min(*resource);
            counts.push((at, pay));
            owed -= pay;
        }
        // The server's: the first kit's count converted, the kits paid until one is lowered.
        let first = counts.first().map_or(0.0, |(_, count)| *count);
        let divisor = part.cost * cut;
        let added = if divisor > 0.0 { first / divisor } else { 0.0 };
        for (at, count) in counts {
            let (id, resource) = left[at];
            if resource <= count {
                gone.push(id);
            } else {
                left[at].1 = resource - count;
                break;
            }
        }
        let mut durability = (part.durability + added).min(part.max);
        let mut max = part.max;
        let skip = who.charisma >= ELITE && draw() < 0.5;
        if !skip {
            let [low, high] = if part.governing == Governing::WeaponTreatment && part.kit[1] == 0.0 { part.trader } else { part.kit };
            let share = low + (high - low) * draw();
            max = (max - ((share * max) * 100.0).round() / 100.0).max(0.0);
            durability = durability.min(max);
        }
        parts.push((part.id, durability, max));
    }
    let kits = left.iter().map(|(id, resource)| (*id, (!gone.contains(id)).then_some(*resource))).collect();
    let repaired_to = parts.iter().map(|(_, durability, _)| durability).sum();
    Outcome { parts, kits, repaired_to }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings { cost_guns: 0.5, cost_armor: 0.335, armor_class_divisor: 6.0, min_level_to_buff: 3, materials: HashMap::new(), wear_per_level: [0.01, 0.008, 0.01, 0.004], intellect_cost: 0.004, buff: [[0.074, 0.0019, 30.0]; 3], max_bonus: [0.09, 0.09] }
    }

    /// The AKM at 60 of 100: its kit's shares 0 and 0.035 (the install's).
    fn akm(durability: f32, max: f32) -> Plan {
        Plan { parts: vec![Part { id: crate::inventory::Id::default(), durability, max, template_max: 100.0, kit: [0.0, 0.035], trader: [0.0, 0.04], cost: 0.5, governing: Governing::WeaponTreatment }], armour: false }
    }

    /// The window's figures for an AKM from 60 (digest B4): 40 to repair,
    /// 20 points of a kit, 0 to 3.5 of its greatest at an untrained level,
    /// "Very good" repairability, "Subpar" quality.
    #[test]
    fn the_window_reckons_an_akm_s_repair() {
        let (s, who) = (settings(), Repairer::default());
        let plan = akm(60.0, 100.0);
        assert!((plan.can_accept(&s, &who) - 40.0).abs() < 1e-5);
        assert!((plan.price(40.0, &s, &who) - 20.0).abs() < 1e-5);
        assert_eq!(plan.degradation(40.0, &s, &who), [0.0, 3.5]);
        assert_eq!(plan.repairability(), "Very good");
        assert_eq!(quality_words(plan.quality(&s, &who).1), "Subpar");
        assert!((plan.price_rate(&s, &who) - 120.0).abs() < 1e-6);
        assert_eq!(plan.enhancement_chance(40.0, &s, &who), None, "below the buff's least level");
        let trained = Repairer { levels: [50, 0, 0, 0], ..who };
        assert_eq!(quality_words(plan.quality(&s, &trained).1), "Excellent");
    }

    /// The server's repair: the durability added, the kit lowered, the
    /// greatest less its draw; a kit used up removed.
    #[test]
    fn a_kit_mends_and_the_greatest_durability_falls() {
        let (s, who) = (settings(), Repairer::default());
        let plan = akm(60.0, 100.0);
        let kit = crate::inventory::Id::default();
        let outcome = repair(&plan, 40.0, &[(kit, 1000.0)], &s, &who, &mut || 0.5);
        let (_, durability, max) = outcome.parts[0];
        assert!((max - (100.0 - 1.75)).abs() < 1e-4, "{max}");
        assert!((durability - max).abs() < 1e-4, "capped at the new greatest");
        assert_eq!(outcome.kits, vec![(kit, Some(980.0))]);
        let small = repair(&plan, 40.0, &[(kit, 15.0)], &s, &who, &mut || 0.0);
        assert_eq!(small.kits, vec![(kit, None)], "used up and gone");
        assert!((small.parts[0].1 - 90.0).abs() < 1e-4, "15 points mend 30");
    }
}
