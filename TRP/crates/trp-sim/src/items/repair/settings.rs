//! The repair settings, the parts and the repairers.

use super::*;

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
    /// What a kit's repair may enhance a thing with (SPT's
    /// `configs/repair.json`, `repairKit`); none read, none drawn.
    pub kit_buffs: KitBuffs,
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
            kit_buffs: KitBuffs::default(),
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
    /// The skill whose figures a buff of its repair goes by, where the
    /// server buffs it at all (SPT's `GetItemSkillType`): a weapon's Weapon
    /// Maintenance; a thing of the armour's, a vest's, a headwear's or a
    /// plate's classes Light or Heavy Vests by its `ArmorType` (a built-in
    /// insert is of none of them); nothing else.
    pub buff_skill: Option<Governing>,
    /// Its record's `MaxDurability`, which the server's chance of a buff
    /// measures the points paid against (a weapon's too, where the window
    /// goes by 100).
    pub record_max: f32,
}

/// What the character brings to a repair: the governing skills' levels,
/// Intellect's and Charisma's, and the hideout's bonus to it (a fraction);
/// and what the server's buff goes by: the character's level and the
/// progress of Weapon Maintenance, Light and Heavy Vests (0 to 5,100, the
/// profile's own, no stimulant's buff in it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Repairer {
    pub levels: [u32; 4],
    pub intellect: u32,
    pub charisma: u32,
    pub bonus: f32,
    pub profile_level: u32,
    pub progress: [f32; 3],
}

pub(super) const ELITE: u32 = crate::condition::skills::ELITE;

impl Repairer {
    pub(super) fn level(&self, governing: Governing) -> u32 {
        self.levels[governing as usize]
    }
}

/// A thing's repair as the window reckons it (`IRepairStrategy`).
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub parts: Vec<Part>,
    pub armour: bool,
}

/// The armour's classes the server buffs (`AddBuffToItem`, `GetItemSkillType`:
/// `ARMOR`, `VEST`, `HEADWEAR`, `ARMOR_PLATE`).
pub(super) const BUFFED_ARMOUR: [&str; 4] = ["Armor", "Vest", "Headwear", "ArmorPlate"];
