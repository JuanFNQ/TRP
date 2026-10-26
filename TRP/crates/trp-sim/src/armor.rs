//! The armour a character wears, as specified in `docs/spec/body-hits.md`:
//! the pieces and the order a hit meets them in, each piece's resistance
//! and chance of being gone through, what it does to a bullet at a body
//! collider (deflected, blocked, worn), and what it leaves of a hit's
//! damage (`ProceedDamageThroughArmor`, `ArmorComponent.ApplyDamage`).
//!
//! Nothing here knows of an engine: the ballistics ask a wearer about a
//! bullet (`trp_sim::ballistics`), and a host hands a delivered hit to
//! [`Wearer::take_shot`] and its damage to health.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde_json::Value as Json;
use trp_data::Database;

use crate::ballistics::material;

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
fn mask_of<T: Copy + Into<u32>>(names: &[String], of: impl Fn(&str) -> Option<T>) -> u32 {
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

fn at<'a>(v: &'a Json, key: &str) -> Result<&'a Json> {
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
pub const PLATE_SLOT_PROTO: &str = trp_data::inventory::ARMOR_SLOT_PROTO;

impl ArmorSlot {
    /// The armour slot `slot` of a thing, none for a slot that is not one.
    pub fn of(template: &trp_data::database::ItemTemplate, slot: &str) -> Option<Self> {
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
pub fn slot_merges(template: &trp_data::database::ItemTemplate, slot: &str) -> bool {
    let Some(s) = template.props.get("Slots").and_then(Json::as_array).and_then(|a| a.iter().find(|s| s.get("_name").and_then(Json::as_str) == Some(slot))) else {
        return false;
    };
    if slot == "patron_in_weapon" {
        return false;
    }
    s.get("_mergeSlotWithChildren").and_then(Json::as_bool).unwrap_or(false) || template.props.get("MergesWithChildren").and_then(Json::as_bool).unwrap_or(false)
}

/// One piece of armour worn (`ArmorComponent`).
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// The host's number for the thing it is of (`Item.Id`).
    pub id: u32,
    pub template: String,
    pub class: i32,
    pub durability: f32,
    /// Its greatest durability now (`Repairable.MaxDurability`) and its
    /// template's (`TemplateDurability`, the template's `MaxDurability`).
    pub max_durability: f32,
    pub template_durability: f32,
    /// What it is made of (`MaterialType`), and its armour material's
    /// destructibility (`ArmorMaterials`).
    pub material: i32,
    pub destructibility: f32,
    pub kind: Kind,
    pub blunt_throughput: f32,
    /// `RicochetParams`: x, y, z.
    pub ricochet: [f32; 3],
    /// The collider types (a bit for each) and plates it covers, after its
    /// armour slot's.
    pub colliders: u32,
    pub plates: u16,
    /// It is held in an armour slot: whether that slot is locked and lowers
    /// a blunt hit.
    pub in_armor_slot: bool,
    pub locked: bool,
    pub blunt_soft: bool,
    /// A hinged piece's state (`CompositeArmorComponent`'s toggle); true for
    /// one without a hinge.
    pub on: bool,
    /// What it takes off the character's speed, in percent, negative
    /// (`SpeedPenaltyPercent`; movement spec, "Speed limits").
    pub speed_penalty: f32,
}

impl Piece {
    /// A thing's armour, none where it has none (`Vest`,
    /// `ArmoredEquipment`: a greatest durability and a class above 0),
    /// new, held in `slot` where that is an armour slot.
    pub fn of(db: &Database, template: &str, id: u32, slot: Option<&ArmorSlot>) -> Option<Self> {
        let t = db.items.get(template)?;
        let p = &t.props;
        let number = |key: &str| p.get(key).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).map(|x| x as f32);
        let class = number("armorClass").unwrap_or(0.0) as i32;
        let max = number("MaxDurability").unwrap_or(0.0);
        if max <= 0.0 || class <= 0 {
            return None;
        }
        let names = |key: &str| -> Vec<String> { p.get(key).and_then(Json::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default() };
        let mut colliders = mask_of(&names("armorColliders"), |n| collider_type(n).map(|k| 1u32 << k));
        let mut plates = mask_of(&names("armorPlateColliders"), |n| plate(n).map(u32::from)) as u16;
        if let Some(slot) = slot {
            colliders &= slot.colliders;
            plates &= slot.plates;
        }
        let ricochet = p.get("RicochetParams").map_or([0.0; 3], |r| {
            let axis = |k: &str| r.get(k).and_then(Json::as_f64).unwrap_or(0.0) as f32;
            [axis("x"), axis("y"), axis("z")]
        });
        let armor_material = p.get("ArmorMaterial").and_then(Json::as_str).unwrap_or("");
        let destructibility = db.config.get("ArmorMaterials").and_then(|m| m.get(armor_material)).and_then(|m| m.get("Destructibility")).and_then(Json::as_f64).unwrap_or(0.0) as f32;
        Some(Self {
            id,
            template: template.to_owned(),
            class,
            durability: number("Durability").unwrap_or(max),
            max_durability: max,
            template_durability: max,
            material: p.get("MaterialType").and_then(Json::as_str).and_then(material::named).unwrap_or(material::BODY),
            destructibility,
            kind: match p.get("ArmorType").and_then(Json::as_str) {
                Some("Light") => Kind::Light,
                Some("Heavy") => Kind::Heavy,
                _ => Kind::None,
            },
            blunt_throughput: number("BluntThroughput").unwrap_or(0.0),
            ricochet,
            colliders,
            plates,
            in_armor_slot: slot.is_some(),
            locked: slot.is_some_and(|s| s.locked),
            blunt_soft: slot.is_some_and(|s| s.blunt_soft),
            on: true,
            speed_penalty: number("speedPenaltyPercent").unwrap_or(0.0),
        })
    }

    /// Whether a hit on a collider meets it (`ShotMatches`): a plate
    /// collider's by its plate, a body collider's by its type.
    pub fn covers(&self, collider: u8, plate: u16) -> bool {
        let matches = if plate == 0 { self.colliders & (1 << collider) != 0 } else { self.plates & plate == plate };
        matches && self.on
    }

    pub fn worn_out(&self) -> bool {
        self.durability < 0.01
    }

    pub fn resistance(&self, rules: &ArmorRules, penetration: f32) -> Resistance {
        Resistance::of(self.durability, self.template_durability, rules.resistance_of(self.class), penetration)
    }

    /// `ArmorComponent.Deflects`: the angle between the bullet's way back and
    /// the face's normal, and the number at the bullet's seed.
    pub fn deflects(&self, angle: f32, draw: f32) -> bool {
        let [x, y, z] = self.ricochet;
        if x > 0.0 && angle > z {
            let t = inverse_lerp(90.0, z, angle);
            draw < x + (y - x) * t
        } else {
            false
        }
    }
}

fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a == b {
        0.0
    } else {
        ((value - a) / (b - a)).clamp(0.0, 1.0)
    }
}

/// The order a hit meets the pieces in (`OrderLockedLast` by .NET's
/// `List<T>.Sort`): a piece in an armour slot not locked first; two or
/// three by the sort's fixed exchanges, more by an insertion sort.
pub fn sort_locked_last(pieces: &mut [Piece]) {
    let key = |p: &Piece| if p.in_armor_slot && !p.locked { -1 } else { 1 };
    let swap_if_greater = |pieces: &mut [Piece], a: usize, b: usize| {
        if key(&pieces[a]) > key(&pieces[b]) {
            pieces.swap(a, b);
        }
    };
    match pieces.len() {
        0 | 1 => {}
        2 => swap_if_greater(pieces, 0, 1),
        3 => {
            swap_if_greater(pieces, 0, 1);
            swap_if_greater(pieces, 0, 2);
            swap_if_greater(pieces, 1, 2);
        }
        n => {
            for i in 0..n - 1 {
                // The (i+1)th moved back past those that sort after it.
                let mut at = i + 1;
                while at > 0 && key(&pieces[at - 1]) > key(&pieces[at]) {
                    pieces.swap(at - 1, at);
                    at -= 1;
                }
            }
        }
    }
}

/// What a character's skills do with its armour (`SkillManager`): Heavy
/// Vests' blunt reduction and deflection, Light Vests' blow reduction and
/// bleeding block. All nought untrained.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VestSkills {
    pub heavy_blunt: f32,
    pub heavy_deflect: bool,
    pub light_melee: f32,
    pub light_bleed_block: bool,
    /// The share of a light and a heavy piece's speed penalty taken off
    /// (`LightVestMoveSpeedPenaltyReduction` 0.006 a level,
    /// `HeavyVestMoveSpeedPenaltyReduction` 0.005, held at 50).
    pub light_speed: f32,
    pub heavy_speed: f32,
}

impl VestSkills {
    /// From the character's skills.
    pub fn of(levels: &crate::skills::SkillLevels) -> Self {
        Self::of_levels(|name| levels.level_of(name))
    }

    /// From the skills' levels by their names (`ESkillId`), as the health
    /// skills count them (each rule held at 50, elite above it).
    pub fn of_levels(level: impl Fn(&str) -> u32) -> Self {
        let at = |name: &str| level(name).min(50) as f32;
        let elite = |name: &str| level(name) > 50;
        Self {
            heavy_blunt: at("HeavyVests") * 0.004,
            heavy_deflect: elite("HeavyVests"),
            light_melee: at("LightVests") * 0.006,
            light_bleed_block: elite("LightVests"),
            light_speed: at("LightVests") * 0.006,
            heavy_speed: at("HeavyVests") * 0.005,
        }
    }
}

/// What the armour and the backpack worn take off the character's speed,
/// in percent (`Player.UpdatePenalties`'s speed part): every armour piece in
/// the armour slots and in theirs, each less its vest skill's share by its
/// kind, and the backpack's whole. Destroyed pieces count too. Negative
/// slows: the movement holds the speed to that share of its top while it is
/// (`Movement::set_armour_penalty`; movement spec, "Speed limits").
pub fn speed_penalty(db: &Database, equipment: &[Worn], skills: &VestSkills) -> f32 {
    let armour: f32 = worn_pieces(db, equipment)
        .iter()
        .map(|p| {
            let off = match p.kind {
                Kind::Light => skills.light_speed,
                Kind::Heavy => skills.heavy_speed,
                Kind::None => 0.0,
            };
            p.speed_penalty * (1.0 - off)
        })
        .sum();
    let backpack: f32 = equipment
        .iter()
        .filter(|w| w.slot == "Backpack")
        .filter_map(|w| db.items.get(&w.template))
        .filter_map(|t| t.props.get("speedPenaltyPercent").and_then(Json::as_f64))
        .map(|x| x as f32)
        .sum();
    armour + backpack
}

/// A hit as it reaches a character (`DamageInfo`, a bullet's).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    pub damage: f32,
    pub penetration: f32,
    /// The round's share of armour damage (`ArmorDamage` / 100).
    pub armor_damage: f32,
    /// The pieces that deflected and blocked it (`DeflectedBy`, `BlockedBy`).
    pub deflected_by: Option<u32>,
    pub blocked_by: Option<u32>,
    pub stamina_burn: f32,
    pub heavy_bleeding: f32,
    pub light_bleeding: f32,
    /// A weapon's damage or a grenade's fragment (armour takes nothing else).
    pub weapon: bool,
    /// A blow (`Melee`).
    pub melee: bool,
}

/// What a hit was, after the armour (`PlayerHitInfo`, `DamageInfo`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Taken {
    /// The damage left for health, the penetration as the armour left it,
    /// and the stamina burn.
    pub damage: f32,
    pub penetration: f32,
    pub stamina_burn: f32,
    /// The material the hit shows (`HelmetRicochet` deflected, else the first
    /// piece that took it, else `Body`).
    pub material: i32,
    /// Gone through: not blocked, or deflected.
    pub penetrated: bool,
    /// The damage the armour took away (`DidArmorDamage`).
    pub absorbed: f32,
    /// Some piece was worn out by it (`ProceedArmorDamaged`: the plates'
    /// colliders to be made again).
    pub worn_out: bool,
}

/// A thing worn and what is in its slots: its template, the slot it is in,
/// and its own things.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Worn {
    pub template: String,
    pub slot: String,
    pub children: Vec<Worn>,
}

/// The equipment's slots a hit's armour is gathered from, in order
/// (`Inventory.ArmorSlots`).
pub const ARMOR_SLOTS: [&str; 5] = ["Headwear", "FaceCover", "Eyewear", "TacticalVest", "ArmorVest"];

/// `ItemTemplateIds.InvincibleBalaclava`.
pub const INVINCIBLE_BALACLAVA: &str = "58ac60eb86f77401897560ff";

/// Every armour piece worn, in the order a hit meets them: gathered from
/// the armour slots and their own slots in order (`GetPutOnArmorsNonAlloc`,
/// body hits spec, "The armour worn"), then the locked ones last, each new
/// and numbered as gathered.
pub fn worn_pieces(db: &Database, equipment: &[Worn]) -> Vec<Piece> {
    fn gather(db: &Database, worn: &Worn, slot: Option<&ArmorSlot>, pieces: &mut Vec<Piece>) {
        if let Some(piece) = Piece::of(db, &worn.template, pieces.len() as u32 + 1, slot) {
            pieces.push(piece);
        }
        let Some(t) = db.items.get(&worn.template) else { return };
        let order: Vec<&str> = t.props.get("Slots").and_then(Json::as_array).map(|a| a.iter().filter_map(|s| s.get("_name")?.as_str()).collect()).unwrap_or_default();
        for name in order {
            if !slot_merges(t, name) {
                continue;
            }
            for child in worn.children.iter().filter(|c| c.slot == name) {
                gather(db, child, ArmorSlot::of(t, name).as_ref(), pieces);
            }
        }
    }
    let mut pieces = Vec::new();
    for slot in ARMOR_SLOTS {
        for worn in equipment.iter().filter(|w| w.slot == slot) {
            gather(db, worn, None, &mut pieces);
        }
    }
    sort_locked_last(&mut pieces);
    pieces
}

/// The armour a character wears and its skills.
#[derive(Debug, Clone, PartialEq)]
pub struct Wearer {
    pub rules: Arc<ArmorRules>,
    /// In the order a hit meets them.
    pub pieces: Vec<Piece>,
    pub skills: VestSkills,
    /// It wears the invincible balaclava (`ItemTemplateIds.InvincibleBalaclava`).
    pub invincible: bool,
}

impl Wearer {
    /// Dressed in `equipment` (the things in the equipment's slots, each
    /// with what is in its own), every piece new, numbered in the order
    /// gathered (`GetPutOnArmorsNonAlloc`, body hits spec, "The armour
    /// worn").
    pub fn dress(db: &Database, rules: Arc<ArmorRules>, equipment: &[Worn], skills: VestSkills) -> Self {
        let pieces = worn_pieces(db, equipment);
        let invincible = pieces.iter().any(|p| p.template == INVINCIBLE_BALACLAVA);
        Self { rules, pieces, skills, invincible }
    }

    /// The plates whose colliders are on (`RecalculateEquipmentParams`): of
    /// every piece not worn out.
    pub fn plates_on(&self) -> u16 {
        self.pieces.iter().filter(|p| p.durability >= f32::EPSILON).fold(0, |m, p| m | p.plates)
    }

    /// The factor of the first piece over a collider at a penetration
    /// (`TryGetArmorResistData`), none where none covers it.
    pub fn factor(&self, collider: u8, plate: u16, penetration: f32) -> Option<f32> {
        self.pieces.iter().find(|p| p.covers(collider, plate)).map(|p| p.resistance(&self.rules, penetration).factor)
    }

    /// `SetShotStatus` at a forward hit: the piece that deflects the bullet,
    /// if one does; on the way a piece may block it (`blocked`).
    pub fn shot_status(&self, collider: u8, plate: u16, angle: f32, penetration: f32, draw: f32, blocked: &mut Option<u32>) -> Option<u32> {
        for p in &self.pieces {
            if !p.covers(collider, plate) {
                continue;
            }
            if p.deflects(angle, draw) {
                return Some(p.id);
            }
            if blocked.is_none() && p.durability > 0.0 && p.resistance(&self.rules, penetration).chance(penetration) - draw * 100.0 < 0.0 {
                *blocked = Some(p.id);
            }
        }
        None
    }

    /// `IsShotDeflectedByHeavyArmor`, the angle within its bounds asked by
    /// the caller: `draw` the number at the seed.
    pub fn heavy_deflects(&self, collider: u8, plate: u16, draw: f32) -> bool {
        self.skills.heavy_deflect
            && self.pieces.iter().any(|p| {
                p.kind == Kind::Heavy
                    && p.durability >= self.rules.heavy_current_threshold * p.max_durability
                    && p.durability >= self.rules.heavy_max_threshold * p.template_durability
                    && p.covers(collider, plate)
                    && draw < self.rules.heavy_deflect_chance
            })
    }

    /// `IsBleedingBlockedByLightArmor`: a light piece not worn out over the
    /// collider type (by its types alone), with the Light Vests' elite buff.
    pub fn bleed_blocked(&self, collider: u8) -> bool {
        self.skills.light_bleed_block && self.pieces.iter().any(|p| !p.worn_out() && p.kind == Kind::Light && p.colliders & (1 << collider) != 0)
    }

    /// A hit through the armour (`ApplyShot`'s `ProceedDamageThroughArmor`
    /// and its material); `alive` whether the character lives (a dead one's
    /// armour takes nothing).
    pub fn take_shot(&mut self, shot: &Shot, collider: u8, plate: u16, alive: bool) -> Taken {
        let mut info = *shot;
        let before = info.damage;
        let (mut blocked, mut deflected, mut worn_out) = (false, false, false);
        let mut first = None;
        let rules = self.rules.clone();
        for p in &mut self.pieces {
            if !p.covers(collider, plate) {
                continue;
            }
            if blocked || deflected {
                let mut blunt = p.blunt_throughput;
                if p.kind == Kind::Heavy {
                    blunt *= 1.0 - self.skills.heavy_blunt;
                }
                info.damage *= blunt;
            } else {
                first.get_or_insert(p.material);
                if alive {
                    apply_damage(p, &mut info, plate, &rules, &self.skills);
                    worn_out |= p.durability <= f32::EPSILON;
                }
                blocked = info.blocked_by == Some(p.id);
                deflected = info.deflected_by == Some(p.id);
            }
        }
        if self.invincible {
            info.damage = 0.0;
        }
        let material = if shot.deflected_by.is_some() { material::HELMET_RICOCHET } else { first.unwrap_or(material::BODY) };
        Taken {
            damage: info.damage,
            penetration: info.penetration,
            stamina_burn: info.stamina_burn,
            material,
            penetrated: shot.blocked_by.is_none() || shot.deflected_by.is_some(),
            absorbed: before - info.damage,
            worn_out,
        }
    }
}

/// What a hit delivered to a character did (`ApplyShot`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotOutcome {
    pub part: crate::health::Part,
    /// The hit's figures as they came, and what the armour made of them.
    pub damage_in: f32,
    pub taken: Taken,
    /// The common health it took (`DidBodyDamage`).
    pub did_body: f32,
}

/// A hit handed on by the ballistics delivered to a character
/// (`BodyPartCollider.ApplyHit`, `Player.ApplyShot`, `ApplyDamageInfo`:
/// body hits spec, "The damage"): through its armour, then to its health
/// as a bullet's. None for a hit not taken: not a character's, from
/// behind, past its life, or on the dead.
pub fn apply_shot(wearer: Option<&mut Wearer>, health: &mut crate::health::Health, hit: &crate::ballistics::Hit) -> Option<ShotOutcome> {
    let body = hit.body?;
    if !hit.forward || hit.out_of_life || !health.alive {
        return None;
    }
    let part = *crate::health::Part::ALL.get(body.part as usize)?;
    let shot = Shot {
        damage: if hit.no_damage { 0.0 } else { hit.damage },
        penetration: hit.penetration,
        armor_damage: hit.armor_damage,
        deflected_by: hit.deflected_by,
        blocked_by: hit.blocked_by,
        stamina_burn: hit.round.stamina_burn,
        heavy_bleeding: hit.round.heavy_bleeding,
        light_bleeding: hit.round.light_bleeding,
        weapon: true,
        melee: false,
    };
    let (taken, bleed_block) = match wearer {
        Some(w) => {
            let taken = w.take_shot(&shot, body.collider, body.plate, true);
            (taken, w.bleed_blocked(body.collider))
        }
        None => (Taken { damage: shot.damage, penetration: shot.penetration, stamina_burn: shot.stamina_burn, material: material::BODY, penetrated: true, absorbed: 0.0, worn_out: false }, false),
    };
    health.wound_relapse(taken.damage, Some(part));
    let did_body = health.apply_damage_info(part, taken.damage, crate::health::DamageKind::Bullet, [shot.heavy_bleeding, shot.light_bleeding], bleed_block);
    // What the armour took off a hit to the head concusses (`BluntContusion`).
    health.blunt_contusion(part, taken.absorbed);
    Some(ShotOutcome { part, damage_in: shot.damage, taken, did_body })
}

/// `ArmorComponent.ApplyDamage`: what a piece takes of a hit and leaves of
/// it; returns its wear.
fn apply_damage(p: &mut Piece, info: &mut Shot, plate: u16, rules: &ArmorRules, skills: &VestSkills) -> f32 {
    if !info.weapon {
        return 0.0;
    }
    // A glass visor shatters here (not built).
    if p.durability <= 0.0 {
        return 0.0;
    }
    if info.deflected_by.is_some() {
        info.damage /= 2.0;
        info.armor_damage /= 2.0;
        info.penetration /= 2.0;
    }
    let mut blunt = p.blunt_throughput;
    if p.kind == Kind::Heavy {
        blunt *= 1.0 - skills.heavy_blunt;
    }
    if p.blunt_soft {
        blunt *= rules.blunt_soft;
    }
    let penetration = info.penetration;
    let r = p.resistance(rules, penetration);
    let wear;
    if info.blocked_by != Some(p.id) && info.deflected_by != Some(p.id) {
        if plate != 0 {
            info.damage = 0.0;
        }
        info.damage *= r.factor;
        info.penetration *= r.factor;
        wear = info.penetration * info.armor_damage * (penetration / r.class).clamp(0.5, 0.9) * p.destructibility;
        if p.kind == Kind::Light && info.melee {
            info.damage *= 1.0 - skills.light_melee;
        }
    } else {
        wear = info.penetration * info.armor_damage * (penetration / r.class).clamp(0.6, 1.1) * p.destructibility;
        info.damage *= blunt * (1.0 - 0.03 * (r.real - penetration)).clamp(0.2, 1.0);
        info.stamina_burn *= if blunt > 0.0 { 3.0 / blunt.sqrt() } else { 1.0 };
    }
    // A repair buff's damage reduction: not built (no buffs).
    let wear = wear.max(1.0);
    p.durability = (p.durability - wear).max(0.0);
    wear
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> ArmorRules {
        ArmorRules {
            class_resistance: (0..=10).map(|c| if c == 0 { 1.0 } else { c as f32 * 10.0 }).collect(),
            blunt_soft: 0.6,
            heavy_current_threshold: 0.5,
            heavy_max_threshold: 0.2,
            heavy_deflect_chance: 0.05,
            colliders: [None; 27],
        }
    }

    fn piece(id: u32, class: i32, durability: f32, kind: Kind, blunt: f32, destructibility: f32) -> Piece {
        Piece {
            id,
            template: String::new(),
            class,
            durability,
            max_durability: durability,
            template_durability: durability,
            material: material::BODY_ARMOR,
            destructibility,
            kind,
            blunt_throughput: blunt,
            ricochet: [0.0, 0.0, 80.0],
            colliders: 0,
            plates: 1,
            in_armor_slot: true,
            locked: false,
            blunt_soft: false,
            on: true,
            speed_penalty: 0.0,
        }
    }

    #[test]
    fn a_full_class_six_plate_resists_as_the_game_s() {
        let r = Resistance::of(65.0, 65.0, 60.0, 29.61);
        assert!((r.real - 60.35).abs() < 0.01, "{}", r.real);
        assert_eq!(r.chance(29.61), 0.0);
        // Class 4 whole: 40.24; 15 under 60, so always gone through; at 50,
        // 100 + 50 / (0.9 × 40.24 − 50).
        assert_eq!(Resistance::of(10.0, 10.0, 40.0, 60.0).chance(60.0), 100.0);
        assert!((Resistance::of(10.0, 10.0, 40.0, 50.0).chance(50.0) - 96.38).abs() < 0.01);
    }

    /// Tagilla's chest plate (`3-bot-armour`): a Termite front plate (class
    /// 6, titan, blunt 0.14, its slot lowering a blunt hit) and the soft
    /// insert behind it (light, 0.33); the first two hits blocked, the
    /// damage left as recorded.
    #[test]
    fn the_recorded_blocked_hits_on_tagilla_s_plate_are_left_as_the_game_left_them() {
        let mut front = piece(1, 6, 65.0, Kind::Heavy, 0.14, 0.4125);
        front.blunt_soft = true;
        let mut soft = piece(2, 3, 30.0, Kind::Light, 0.33, 0.1875);
        soft.locked = true;
        let mut wearer = Wearer { rules: Arc::new(rules()), pieces: vec![front, soft], skills: VestSkills::default(), invincible: false };
        let hit = |damage: f32, penetration: f32| Shot {
            damage,
            penetration,
            armor_damage: 0.44,
            deflected_by: None,
            blocked_by: Some(1),
            stamina_burn: 0.0,
            heavy_bleeding: 0.0,
            light_bleeding: 0.0,
            weapon: true,
            melee: false,
        };
        let first = wearer.take_shot(&hit(43.280_31, 29.607_683), 1, 1, true);
        assert!((first.damage - 0.239_946_08).abs() < 2e-6, "{}", first.damage);
        assert_eq!(first.material, material::BODY_ARMOR);
        let second = wearer.take_shot(&hit(54.0, 36.92), 1, 1, true);
        assert!((second.damage - 0.467_697_26).abs() < 2e-4, "{}", second.damage);
    }

    #[test]
    fn unlocked_armour_slots_come_first_by_the_game_s_sort() {
        let mut pieces: Vec<Piece> = (0..3).map(|k| piece(k, 4, 40.0, Kind::Heavy, 0.2, 0.5)).collect();
        pieces[0].in_armor_slot = false;
        pieces[1].in_armor_slot = false;
        // [1, 1, -1] by the three exchanges: the third first, then the
        // second, then the first.
        sort_locked_last(&mut pieces);
        assert_eq!(pieces.iter().map(|p| p.id).collect::<Vec<_>>(), vec![2, 1, 0]);
        let mut pieces: Vec<Piece> = (0..4).map(|k| piece(k, 4, 40.0, Kind::Heavy, 0.2, 0.5)).collect();
        pieces[0].in_armor_slot = false;
        pieces[1].in_armor_slot = false;
        sort_locked_last(&mut pieces);
        assert_eq!(pieces.iter().map(|p| p.id).collect::<Vec<_>>(), vec![2, 3, 0, 1]);
    }
}
