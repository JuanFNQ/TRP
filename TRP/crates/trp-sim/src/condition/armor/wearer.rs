//! Shots and blasts taken by the wearer of armor, and the mark limits.

use super::*;

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
/// and its own things; the host's number for it, where it has one (a mark
/// goes to it), and whether its hinge is up (a switch off: a raised face
/// shield, which is no armour, `CompositeArmorComponent.ShotMatches`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Worn {
    pub template: String,
    pub slot: String,
    pub children: Vec<Worn>,
    pub item: Option<u64>,
    pub up: bool,
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
        if let Some(mut piece) = Piece::of(db, &worn.template, pieces.len() as u32 + 1, slot) {
            piece.item = worn.item;
            piece.on = !worn.up;
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
    /// The face shields its hits and blasts marked, in the order marked,
    /// for the host to take (`drain`) and hand to the shooter, who makes
    /// the mark if found among the living and within its limit
    /// (`MarkLimit`; body hits spec, "A face shield's marks").
    pub marks: Vec<Mark>,
}

/// A face shield a hit or a blast marks (`ArmorComponent.TryShatter`): the
/// piece's number and its thing's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub piece: u32,
    pub item: Option<u64>,
}

/// A blast's share of a face shield's template durability that marks it
/// (`TryShatterIfNeeded`: not below it).
pub const BLAST_MARK_SHARE: f32 = 0.4;

/// The most marks a character makes in one window (`Player.
/// FaceshieldMarkOperation`: one more while its count is 3 or less).
pub const MARKS_IN_A_WINDOW: u32 = 4;

/// A character's limit on the marks it makes (`Player.
/// FaceshieldMarkOperation`): when its window began, on the game's clock,
/// and how many it made in it. Nought and nought as the character is made.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MarkLimit {
    pub last: f32,
    pub count: u32,
}

impl MarkLimit {
    /// A mark asked at `now` (`Time.time`): more than one fixed step
    /// (`Time.fixedDeltaTime`) after the window began, a new window begins;
    /// then whether it is made (four in a window at most), counted if so.
    pub fn allow(&mut self, now: f32, fixed_dt: f32) -> bool {
        if now > self.last + fixed_dt {
            self.last = now;
            self.count = 0;
        }
        if self.count < MARKS_IN_A_WINDOW {
            self.count += 1;
            true
        } else {
            false
        }
    }
}

impl Wearer {
    /// Dressed in `equipment` (the things in the equipment's slots, each
    /// with what is in its own), every piece new, numbered in the order
    /// gathered (`GetPutOnArmorsNonAlloc`, body hits spec, "The armour
    /// worn").
    pub fn dress(db: &Database, rules: Arc<ArmorRules>, equipment: &[Worn], skills: VestSkills) -> Self {
        let pieces = worn_pieces(db, equipment);
        let invincible = pieces.iter().any(|p| p.template == INVINCIBLE_BALACLAVA);
        Self { rules, pieces, skills, invincible, marks: Vec::new() }
    }

    /// The marks made since last taken, in their order.
    pub fn drain(&mut self) -> Vec<Mark> {
        std::mem::take(&mut self.marks)
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

    /// A blast's damage to the armour (`ApplyExplosionDamageToArmor`;
    /// `blast::Blast::armor`): each piece takes the sum of what the
    /// colliders it covers were given, times its material's explosion
    /// destructibility (`ApplyExplosionDurabilityDamage`), off its
    /// durability, never below nought. What each piece took, by its number.
    /// A face shield's glass whose share is at least 0.4 of its template
    /// durability is marked first (`TryShatterIfNeeded`: `marks`, the
    /// thrower's to make).
    pub fn take_blast(&mut self, given: &[(u8, u16, f32)]) -> Vec<(u32, f32)> {
        let mut taken = Vec::new();
        for piece in &mut self.pieces {
            let sum: f32 = given.iter().filter(|(collider, plate, _)| piece.covers(*collider, *plate)).map(|(.., value)| *value).sum();
            if sum <= 0.0 {
                continue;
            }
            let damage = sum * piece.explosion_destructibility;
            if damage / piece.template_durability >= BLAST_MARK_SHARE && piece.marks() {
                self.marks.push(Mark { piece: piece.id, item: piece.item });
            }
            piece.durability = (piece.durability - damage).max(0.0);
            piece.buff = piece.buff.filter(|buff| buff.holds(piece.durability));
            taken.push((piece.id, damage));
        }
        taken
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
                    apply_damage(p, &mut info, plate, &rules, &self.skills, &mut self.marks);
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
