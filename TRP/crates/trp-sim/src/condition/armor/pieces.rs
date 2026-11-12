//! Armor pieces, their order, vest skills and the speed and ergonomics penalties.

use super::*;

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
    /// destructibility by a bullet and by a blast (`ArmorMaterials`:
    /// `Destructibility`, `ExplosionDestructibility`).
    pub material: i32,
    pub destructibility: f32,
    pub explosion_destructibility: f32,
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
    /// A hinged piece's state (`CompositeArmorComponent`'s toggle, its
    /// thing's: `Worn::up`); true for one without a hinge. Off, nothing
    /// meets it (`ShotMatches`).
    pub on: bool,
    /// The host's number for its thing (`Worn::item`), which a mark goes
    /// to, and whether that thing is a face shield (`FaceShieldComponent`):
    /// made of `GlassVisor` too, it is marked (`TryShatter`).
    pub item: Option<u64>,
    pub face_shield: bool,
    /// What it takes off the character's speed, in percent, negative
    /// (`SpeedPenaltyPercent`; movement spec, "Speed limits").
    pub speed_penalty: f32,
    /// What it takes off the weapon's ergonomics, in percent, negative
    /// (`WeaponErgonomicPenalty`; weapon handling spec, "The gear's share of
    /// the ergonomics").
    pub ergonomics_penalty: f32,
    /// Its thing's repair buff (`ArmorComponent.Buff`), as the host gives
    /// it: a damage reduction cuts the damage it leaves; any is gone once
    /// a hit's or a blast's wear takes it under its threshold. None new.
    pub buff: Option<crate::items::repair::RepairBuff>,
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
        let of_material = |key: &str| db.config.get("ArmorMaterials").and_then(|m| m.get(armor_material)).and_then(|m| m.get(key)).and_then(Json::as_f64).unwrap_or(0.0) as f32;
        let (destructibility, explosion_destructibility) = (of_material("Destructibility"), of_material("ExplosionDestructibility"));
        Some(Self {
            id,
            template: template.to_owned(),
            class,
            durability: number("Durability").unwrap_or(max),
            max_durability: max,
            template_durability: max,
            material: p.get("MaterialType").and_then(Json::as_str).and_then(material::named).unwrap_or(material::BODY),
            destructibility,
            explosion_destructibility,
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
            item: None,
            face_shield: p.get("FaceShieldComponent").and_then(Json::as_bool).unwrap_or(false),
            speed_penalty: number("speedPenaltyPercent").unwrap_or(0.0),
            ergonomics_penalty: number("weaponErgonomicPenalty").unwrap_or(0.0),
            buff: None,
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

    /// Whether a hit or a blast on it marks it (`TryShatter`): made of
    /// `GlassVisor`, its thing a face shield.
    pub fn marks(&self) -> bool {
        self.material == material::GLASS_VISOR && self.face_shield
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
    pub fn of(levels: &crate::condition::skills::SkillLevels) -> Self {
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

/// The gear's share of the weapon's ergonomics (`RecalculateEquipmentParams`,
/// `TotalErgonomicsFunc`): every armour piece worn, destroyed ones too, and
/// the backpack add their `WeaponErgonomicPenalty`, and the sum over 100 is
/// the share (0 or less; weapon handling spec, "The gear's share of the
/// ergonomics"). No skill lessens it.
pub fn ergonomics_share(db: &Database, equipment: &[Worn]) -> f32 {
    let armour: f32 = worn_pieces(db, equipment).iter().map(|p| p.ergonomics_penalty).sum();
    let backpack: f32 = equipment
        .iter()
        .filter(|w| w.slot == "Backpack")
        .filter_map(|w| db.items.get(&w.template))
        .filter_map(|t| t.props.get("weaponErgonomicPenalty").and_then(Json::as_f64))
        .map(|x| x as f32)
        .sum();
    (armour + backpack) / 100.0
}
