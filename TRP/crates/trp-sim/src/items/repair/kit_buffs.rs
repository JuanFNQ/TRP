//! The buffs a repair kit may roll and their chances.

use super::*;

/// A repair kit's enhancement of a thing (the game's `BuffComponent`, kept
/// with the thing; SPT's `UpdBuff`, which writes it): its rarity, its kind,
/// its value (a factor below 1, the double the server drew) and the
/// durability below which it is gone (`ThresholdDurability`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RepairBuff {
    pub rarity: Rarity,
    pub kind: BuffKind,
    pub value: f64,
    pub threshold: f64,
}

/// A buff's rarity (`EBuffRarity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Rare,
}

/// A buff's kind (`ERepairBuffType`): the three the game acts on and lists
/// (`BuffComponent.UpdateModifiers`); the install's server weighs no other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuffKind {
    WeaponSpread,
    DamageReduction,
    MalfunctionProtections,
}

impl BuffKind {
    fn named(name: &str) -> Option<Self> {
        match name {
            "WeaponSpread" => Some(Self::WeaponSpread),
            "DamageReduction" => Some(Self::DamageReduction),
            "MalfunctionProtections" => Some(Self::MalfunctionProtections),
            _ => None,
        }
    }
}

/// The durability under which a buff is gone whatever its threshold
/// (`BuffComponent.DURABILITY_THRESHOLD`).
const BUFF_FLOOR: f64 = 0.5;

impl RepairBuff {
    /// Whether it holds at a durability just worn to (`TryDisableComponent`,
    /// after a shot's wear or an armour's hit): gone for good below its
    /// threshold, and below 0.5 whatever that is.
    pub fn holds(&self, durability: f32) -> bool {
        durability as f64 >= self.threshold.max(BUFF_FLOOR)
    }

    /// Its factor on what a kind acts on, 1 where there is none or it is of
    /// another kind (`BuffComponent.WeaponSpread`, `DamageReduction`,
    /// `MalfunctionProtections`), as the float its users cast it to.
    pub fn factor(buff: Option<&RepairBuff>, kind: BuffKind) -> f32 {
        buff.filter(|buff| buff.kind == kind).map_or(1.0, |buff| buff.value as f32)
    }

    /// Its attribute's id as the install's tables name it (`EItemAttributeId`'s
    /// JSON names: its icon's key).
    pub fn attribute(&self) -> &'static str {
        match (self.kind, self.rarity) {
            (BuffKind::MalfunctionProtections, Rarity::Common) => "MalfunctionProtectionsCommonBuff",
            (BuffKind::MalfunctionProtections, Rarity::Rare) => "MalfunctionProtectionsRareBuff",
            (BuffKind::WeaponSpread, Rarity::Common) => "WeaponSpreadCommonBuff",
            (BuffKind::WeaponSpread, Rarity::Rare) => "WeaponSpreadRareBuff",
            (BuffKind::DamageReduction, Rarity::Common) => "DamageReductionCommonBuff",
            (BuffKind::DamageReduction, Rarity::Rare) => "DamageReductionRareBuff",
        }
    }

    /// The key of its row's name (`ItemAttributeNames.GetName`).
    pub fn name_key(&self) -> &'static str {
        match (self.kind, self.rarity) {
            (BuffKind::MalfunctionProtections, Rarity::Common) => "MALFUNCTION PROTECTIONS COMMON BUFF",
            (BuffKind::MalfunctionProtections, Rarity::Rare) => "MALFUNCTION PROTECTIONS RARE BUFF",
            (BuffKind::WeaponSpread, Rarity::Common) => "WEAPON SPREAD COMMON BUFF",
            (BuffKind::WeaponSpread, Rarity::Rare) => "WEAPON SPREAD RARE BUFF",
            (BuffKind::DamageReduction, Rarity::Common) => "DAMAGE REDUCTION COMMON BUFF",
            (BuffKind::DamageReduction, Rarity::Rare) => "DAMAGE REDUCTION RARE BUFF",
        }
    }

    /// The whole percent its row writes (`(int)((1 - value) * 100)`: cut
    /// toward nought).
    pub fn percent(&self) -> i32 {
        ((1.0 - self.value) * 100.0) as i32
    }
}

/// What one kind of thing's kit repair may enhance it with (SPT's
/// `BonusSettings`): the rarities' weights, the kinds' weights, and each
/// rarity's and kind's range of values and of the threshold's share of the
/// durability after the repair, in percent (`ValuesMinMax`,
/// `ActiveDurabilityPercentMinMax`). The weights are kept in their keys'
/// order as read (sorted: the file's order is lost in reading, and the odds
/// are the weights' whatever the order).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuffTable {
    pub rarities: Vec<(Rarity, f64)>,
    pub kinds: Vec<(BuffKind, f64)>,
    pub ranges: Vec<(Rarity, BuffKind, [f64; 2], [f64; 2])>,
}

/// A weapon's and an armour's tables (`repairKit.weapon`, `repairKit.armor`:
/// the server takes the armour's for armour, vests, headwear and plates
/// alike).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KitBuffs {
    pub weapon: BuffTable,
    pub armour: BuffTable,
}

impl KitBuffs {
    /// From SPT's `configs/repair.json`.
    pub fn from_config(config: &Json) -> Self {
        let table = |name: &str| -> BuffTable {
            let Some(kind) = config.get("repairKit").and_then(|kits| kits.get(name)) else { return BuffTable::default() };
            let rarity_of = |name: &str| match name {
                "Common" => Some(Rarity::Common),
                "Rare" => Some(Rarity::Rare),
                _ => None,
            };
            let weights = |field: &str| kind.get(field).and_then(Json::as_object).into_iter().flatten().filter_map(|(name, weight)| Some((name.as_str(), weight.as_f64()?))).collect::<Vec<_>>();
            let rarities = weights("rarityWeight").into_iter().filter_map(|(name, weight)| Some((rarity_of(name)?, weight))).collect();
            let kinds = weights("bonusTypeWeight").into_iter().filter_map(|(name, weight)| Some((BuffKind::named(name)?, weight))).collect();
            let mut ranges = Vec::new();
            for (rarity, field) in [(Rarity::Common, "Common"), (Rarity::Rare, "Rare")] {
                for (name, figures) in kind.get(field).and_then(Json::as_object).into_iter().flatten() {
                    let Some(buff) = BuffKind::named(name) else { continue };
                    let range = |field: &str| -> Option<[f64; 2]> {
                        let r = figures.get(field)?;
                        Some([r.get("min")?.as_f64()?, r.get("max")?.as_f64()?])
                    };
                    if let (Some(values), Some(share)) = (range("valuesMinMax"), range("activeDurabilityPercentMinMax")) {
                        ranges.push((rarity, buff, values, share));
                    }
                }
            }
            BuffTable { rarities, kinds, ranges }
        };
        Self { weapon: table("weapon"), armour: table("armor") }
    }
}

/// The server's pick by weights (`WeightedRandomHelper.GetWeightedValue`):
/// one alone with no draw; weights that sum to their count (all 1) a
/// uniform whole number (`RandomUtil.GetInt`); else the first whose
/// running sum reaches the sum times a draw. `PROVISIONAL`: the whole
/// number taken as the draw's share of the count, and the items in their
/// keys' sorted order (the file's is lost in reading); the odds are the
/// server's either way, which item a given draw names is not.
fn weighted<T: Copy>(items: &[(T, f64)], draw: &mut dyn FnMut() -> f64) -> Option<T> {
    match items {
        [] => None,
        [(only, _)] => Some(*only),
        _ => {
            let sum: f64 = items.iter().map(|(_, weight)| weight.max(0.0)).sum();
            if sum == items.len() as f64 {
                let at = ((draw() * items.len() as f64) as usize).min(items.len() - 1);
                return Some(items[at].0);
            }
            let goal = sum * draw();
            let mut running = 0.0;
            for (item, weight) in items {
                if *weight >= 0.0 {
                    running += weight;
                }
                if running >= goal {
                    return Some(*item);
                }
            }
            items.last().map(|(item, _)| *item)
        }
    }
}

/// The server's chance of a buff to a part's repair by a kit (SPT's
/// `RepairService.ShouldBuffItem`), none where it cannot come: the
/// character under `MinimumLevelToApplyBuff`, a part the server buffs not
/// (`buff_skill`), or its skill's progress under 1,000 (level 10). Else
/// `min + bonus × level × clamp((points / record durability) /
/// max(0.01, max percent), 0.01, 1)`: the level bonus alone times the
/// share (the window's preview multiplies the whole by it), the points the
/// request's first kit's.
pub fn buff_chance(part: &Part, points: f32, settings: &Settings, who: &Repairer) -> Option<f64> {
    if who.profile_level < settings.min_level_to_buff {
        return None;
    }
    let skill = part.buff_skill?;
    let at = match skill {
        Governing::WeaponTreatment => 0,
        Governing::LightVests => 1,
        Governing::HeavyVests => 2,
        Governing::Intellect => return None,
    };
    let progress = who.progress[at] as f64;
    if progress < 1000.0 {
        return None;
    }
    let [least, per_level, most_percent] = settings.buff[at].map(f64::from);
    let level = (progress / 100.0).trunc();
    let share = ((points as f64 / part.record_max as f64) / most_percent.max(0.01)).clamp(0.01, 1.0);
    Some(least + per_level * level * share)
}

/// The buff itself (SPT's `RepairService.AddBuff`), from a table: the
/// rarity by its weights, the kind by its weights, the value uniform in
/// its range (`RandomUtil.GetDouble`), the threshold's share uniform in its
/// range and the threshold that share of the durability after the repair,
/// rounded to a whole with halves to the even (`GetPercentOfValue(..., 0)`:
/// .NET's `Math.Round`). None where the table has nothing to give.
pub fn roll_buff(table: &BuffTable, durability: f32, draw: &mut dyn FnMut() -> f64) -> Option<RepairBuff> {
    let rarity = weighted(&table.rarities, draw)?;
    let kind = weighted(&table.kinds, draw)?;
    let (.., values, share) = table.ranges.iter().find(|(r, k, ..)| *r == rarity && *k == kind)?;
    let value = values[0] + draw() * (values[1] - values[0]);
    let percent = share[0] + draw() * (share[1] - share[0]);
    let threshold = (percent * (durability as f64 / 100.0)).round_ties_even();
    Some(RepairBuff { rarity, kind, value, threshold })
}

/// The buffs a repair brought (SPT's `AddBuffToItem`, after each part's
/// request): for each part the amount reached, its chance against a draw
/// (`Random.NextDouble() <= chance`), and on success a buff from its
/// kind's table at its durability after the repair, which replaces any it
/// had. The parts buffed, by their ids; a part not buffed keeps what it
/// had. `draw` gives draws in [0, 1).
pub fn buffs(plan: &Plan, outcome: &Outcome, settings: &Settings, who: &Repairer, draw: &mut dyn FnMut() -> f64) -> Vec<(Id, RepairBuff)> {
    let mut won = Vec::new();
    for ((part, paid), (_, durability, _)) in plan.parts.iter().zip(&outcome.paid).zip(&outcome.parts) {
        let Some(points) = *paid else { continue };
        let Some(chance) = buff_chance(part, points, settings, who) else { continue };
        if draw() > chance {
            continue;
        }
        let table = if part.buff_skill == Some(Governing::WeaponTreatment) { &settings.kit_buffs.weapon } else { &settings.kit_buffs.armour };
        if let Some(buff) = roll_buff(table, *durability, draw) {
            won.push((part.id, buff));
        }
    }
    won
}
