//! Collider types, plates, armor rules, resistance and armor slots.

use super::*;

/// The collider types' names (`EBodyPartColliderType`), by number.
pub const COLLIDER_TYPES: [&str; 27] = [
    "HeadCommon", "RibcageUp", "", "Pelvis", "LeftUpperArm", "LeftForearm", "RightUpperArm", "RightForearm", "LeftThigh", "LeftCalf",
    "RightThigh", "RightCalf", "ParietalHead", "BackHead", "Ears", "Eyes", "Jaw", "NeckFront", "NeckBack", "RightSideChestUp",
    "LeftSideChestUp", "SpineTop", "SpineDown", "PelvisBack", "RightSideChestDown", "LeftSideChestDown", "RibcageLow",
];

/// The plates' names (`EArmorPlateCollider`), each its flag `1 << k`.
pub const PLATES: [&str; 12] = [
    "Plate_Granit_SAPI_chest",
    "Plate_Granit_SAPI_back",
    "Plate_Granit_SSAPI_side_left_high",
    "Plate_Granit_SSAPI_side_left_low",
    "Plate_Granit_SSAPI_side_right_high",
    "Plate_Granit_SSAPI_side_right_low",
    "Plate_Korund_chest",
    "Plate_6B13_back",
    "Plate_Korund_side_left_high",
    "Plate_Korund_side_left_low",
    "Plate_Korund_side_right_high",
    "Plate_Korund_side_right_low",
];

/// A collider type's number by its name.
pub fn collider_type(name: &str) -> Option<u8> {
    COLLIDER_TYPES.iter().position(|n| !n.is_empty() && *n == name).map(|at| at as u8)
}

/// A plate's flag by its name; 0 for none (`"0"`, as the recorder writes it).
pub fn plate(name: &str) -> Option<u16> {
    if name == "0" || name.is_empty() {
        return Some(0);
    }
    PLATES.iter().position(|n| *n == name).map(|at| 1 << at)
}

/// A list of names as a mask, unknown names left out.
pub(super) fn mask_of<T: Copy + Into<u32>>(names: &[String], of: impl Fn(&str) -> Option<T>) -> u32 {
    names.iter().filter_map(|n| of(n)).fold(0, |m, x| m | x.into())
}

/// `EArmorType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    None,
    Light,
    Heavy,
}

/// What the globals say of armour (`globals.json`'s `config`).
#[derive(Debug, Clone, PartialEq)]
pub struct ArmorRules {
    /// Each class's resistance (`armor.class`), by class.
    pub class_resistance: Vec<f32>,
    /// `BluntDamageReduceFromSoftArmorMod`.
    pub blunt_soft: f32,
    /// The Heavy Vests' deflection (`SkillsSettings.HeavyVests`): the
    /// durability's least shares of its greatest and of its template's
    /// (`RicochetChanceHVestsCurrentDurabilityThreshold`,
    /// `RicochetChanceHVestsMaxDurabilityThreshold`) and its chance
    /// (`RicochetChanceHVestsEliteLevel`).
    pub heavy_current_threshold: f32,
    pub heavy_max_threshold: f32,
    pub heavy_deflect_chance: f32,
    /// Each body collider's level, chance and damage share
    /// (`BodyPartColliderSettings`), by collider type.
    pub colliders: [Option<[f32; 3]>; 27],
}

pub(super) fn at<'a>(v: &'a Json, key: &str) -> Result<&'a Json> {
    v.get(key).ok_or_else(|| anyhow!("no {key}"))
}

fn num(v: &Json, key: &str) -> Result<f32> {
    at(v, key)?.as_f64().map(|x| x as f32).ok_or_else(|| anyhow!("{key} is not a number"))
}

impl ArmorRules {
    /// `config` is the `config` section of `globals.json`.
    pub fn new(config: &Json) -> Result<Self> {
        let classes = at(at(config, "armor")?, "class")?.as_array().context("armor.class")?;
        let class_resistance = classes.iter().map(|c| num(c, "resistance")).collect::<Result<Vec<_>>>()?;
        let heavy = at(at(config, "SkillsSettings")?, "HeavyVests")?;
        let settings = at(config, "BodyPartColliderSettings")?;
        let mut colliders = [None; 27];
        for (k, name) in COLLIDER_TYPES.iter().enumerate() {
            if let Some(s) = settings.get(*name) {
                colliders[k] = Some([num(s, "PenetrationLevel")?, num(s, "PenetrationChance")?, num(s, "PenetrationDamageMod")?]);
            }
        }
        Ok(Self {
            class_resistance,
            blunt_soft: num(config, "BluntDamageReduceFromSoftArmorMod")?,
            heavy_current_threshold: num(heavy, "RicochetChanceHVestsCurrentDurabilityThreshold")?,
            heavy_max_threshold: num(heavy, "RicochetChanceHVestsMaxDurabilityThreshold")?,
            heavy_deflect_chance: num(heavy, "RicochetChanceHVestsEliteLevel")?,
            colliders,
        })
    }

    /// A class's resistance (`GetArmorClass`: the class held within the
    /// list).
    pub fn resistance_of(&self, class: i32) -> f32 {
        let last = self.class_resistance.len().saturating_sub(1) as i32;
        self.class_resistance.get(class.clamp(0, last) as usize).copied().unwrap_or(0.0)
    }
}

/// A piece's resistance to a penetration (`ArmorResistanceData`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resistance {
    pub real: f32,
    pub class: f32,
    /// What it leaves of a bullet's damage and penetration (`CF`).
    pub factor: f32,
}

impl Resistance {
    /// `RealResistance`.
    pub fn of(durability: f32, template_durability: f32, class_resistance: f32, penetration: f32) -> Self {
        let share = durability / template_durability * 100.0;
        let real = (121.0 - 5000.0 / (45.0 + share * 2.0)) * class_resistance * 0.01;
        Self { real, class: class_resistance, factor: (penetration / (real + 12.0)).clamp(0.6, 1.0) }
    }

    /// The chance, in hundredths, of being gone through (`GetPenetrationChance`).
    pub fn chance(&self, penetration: f32) -> f32 {
        let real = self.real;
        if real >= penetration + 15.0 {
            0.0
        } else if real >= penetration {
            0.4 * (real - penetration - 15.0).powi(2)
        } else if real <= penetration - 15.0 {
            100.0
        } else {
            100.0 + penetration / (0.9 * real - penetration)
        }
    }
}

/// What an armour slot of a carrier says of the piece in it (the slot's
/// filter: `armorColliders`, `armorPlateColliders`, `locked`,
/// `bluntDamageReduceFromSoftArmor`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArmorSlot {
    pub colliders: u32,
    pub plates: u16,
    pub locked: bool,
    pub blunt_soft: bool,
}

/// The prototype of the plates' slots (`SlotSerializer.plateModSlotId`).
pub const PLATE_SLOT_PROTO: &str = trp_data::character::inventory::ARMOR_SLOT_PROTO;

impl ArmorSlot {
    /// The armour slot `slot` of a thing, none for a slot that is not one.
    pub fn of(template: &trp_data::setup::database::ItemTemplate, slot: &str) -> Option<Self> {
        let s = template.props.get("Slots")?.as_array()?.iter().find(|s| s.get("_name").and_then(Json::as_str) == Some(slot))?;
        if !s.get("_proto").and_then(Json::as_str).is_some_and(|p| p.eq_ignore_ascii_case(PLATE_SLOT_PROTO)) {
            return None;
        }
        let filter = s.get("_props")?.get("filters")?.get(0)?;
        let names = |key: &str| -> Vec<String> { filter.get(key).and_then(Json::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default() };
        Some(Self {
            colliders: mask_of(&names("armorColliders"), |n| collider_type(n).map(|k| 1u32 << k)),
            plates: mask_of(&names("armorPlateColliders"), |n| plate(n).map(u32::from)) as u16,
            locked: filter.get("locked").and_then(Json::as_bool).unwrap_or(false),
            blunt_soft: filter.get("bluntDamageReduceFromSoftArmor").and_then(Json::as_bool).unwrap_or(false),
        })
    }
}

/// Whether a thing's slot merges with what is in it
/// (`ContainerExtensions.MergesWithChildren`): when the slot says so, or,
/// saying nothing, when the thing holding it does.
pub fn slot_merges(template: &trp_data::setup::database::ItemTemplate, slot: &str) -> bool {
    let Some(s) = template.props.get("Slots").and_then(Json::as_array).and_then(|a| a.iter().find(|s| s.get("_name").and_then(Json::as_str) == Some(slot))) else {
        return false;
    };
    if slot == "patron_in_weapon" {
        return false;
    }
    s.get("_mergeSlotWithChildren").and_then(Json::as_bool).unwrap_or(false) || template.props.get("MergesWithChildren").and_then(Json::as_bool).unwrap_or(false)
}
