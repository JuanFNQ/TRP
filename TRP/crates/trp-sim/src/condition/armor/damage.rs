//! Applying shots and blows through armor to the body.

use super::*;

/// What a hit delivered to a character did (`ApplyShot`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotOutcome {
    pub part: crate::condition::health::Part,
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
pub fn apply_shot(wearer: Option<&mut Wearer>, health: &mut crate::condition::health::Health, hit: &crate::combat::ballistics::Hit) -> Option<ShotOutcome> {
    let body = hit.body?;
    if !hit.forward || hit.out_of_life || !health.alive {
        return None;
    }
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
    apply_damage_through(wearer, health, body, &shot, crate::condition::health::DamageKind::Bullet)
}

/// A melee weapon's blow delivered to a character (`BaseKnifeController.
/// ProcessHit` → `GameWorld.HackShot` → `BallisticCollider.ApplyHit` →
/// `Player.ApplyShot`; grenades-and-knife spec, "The knife's hands"): a
/// `Melee` hit, always forward, of the stroke's damage and penetration,
/// armour damage 1, no bleeding odds of its own, the knife's stamina burn
/// rate; nothing deflects or blocks it (no ballistics ran), so each piece
/// over it takes it as gone through, a light one with its Light Vests' cut
/// (`ArmorComponent.ApplyDamage`). None on the dead.
pub fn apply_blow(wearer: Option<&mut Wearer>, health: &mut crate::condition::health::Health, body: crate::combat::ballistics::BodyRef, damage: f32, penetration: f32, stamina_burn: f32) -> Option<ShotOutcome> {
    if !health.alive {
        return None;
    }
    apply_damage_through(wearer, health, body, &Shot::blow(damage, penetration, stamina_burn), crate::condition::health::DamageKind::Melee)
}

impl Shot {
    /// A melee weapon's blow (`ProcessHit`'s `DamageInfo`): its damage and
    /// penetration, armour damage 1, none deflecting or blocking it, no
    /// bleeding odds of its own, the knife's stamina burn rate.
    pub fn blow(damage: f32, penetration: f32, stamina_burn: f32) -> Self {
        Self { damage, penetration, armor_damage: 1.0, deflected_by: None, blocked_by: None, stamina_burn, heavy_bleeding: 0.0, light_bleeding: 0.0, weapon: true, melee: true }
    }
}

/// `ApplyShot` after its checks: through the armour, then to health as
/// `kind` (`ApplyDamageInfo`: the wound's relapse, the damage, a head's
/// blunt contusion).
fn apply_damage_through(wearer: Option<&mut Wearer>, health: &mut crate::condition::health::Health, body: crate::combat::ballistics::BodyRef, shot: &Shot, kind: crate::condition::health::DamageKind) -> Option<ShotOutcome> {
    let part = *crate::condition::health::Part::ALL.get(body.part as usize)?;
    let shot = *shot;
    let (taken, bleed_block) = match wearer {
        Some(w) => {
            let taken = w.take_shot(&shot, body.collider, body.plate, true);
            (taken, w.bleed_blocked(body.collider))
        }
        None => (Taken { damage: shot.damage, penetration: shot.penetration, stamina_burn: shot.stamina_burn, material: material::BODY, penetrated: true, absorbed: 0.0, worn_out: false }, false),
    };
    health.wound_relapse(taken.damage, Some(part));
    let did_body = health.apply_damage_info(part, taken.damage, kind, [shot.heavy_bleeding, shot.light_bleeding], bleed_block);
    // What the armour took off a hit to the head concusses (`BluntContusion`).
    health.blunt_contusion(part, taken.absorbed);
    Some(ShotOutcome { part, damage_in: shot.damage, taken, did_body })
}

/// `ArmorComponent.ApplyDamage`: what a piece takes of a hit and leaves of
/// it; returns its wear. A face shield's glass is marked first, whatever
/// its wear (`TryShatter`, `marks`: the shooter's to make).
pub(super) fn apply_damage(p: &mut Piece, info: &mut Shot, plate: u16, rules: &ArmorRules, skills: &VestSkills, marks: &mut Vec<Mark>) -> f32 {
    if !info.weapon {
        return 0.0;
    }
    if p.marks() {
        marks.push(Mark { piece: p.id, item: p.item });
    }
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
    // A repair buff's damage reduction, whichever way it went; then the
    // buff gone where the wear takes the piece under its threshold.
    info.damage *= crate::items::repair::RepairBuff::factor(p.buff.as_ref(), crate::items::repair::BuffKind::DamageReduction);
    let wear = wear.max(1.0);
    p.durability = (p.durability - wear).max(0.0);
    p.buff = p.buff.filter(|buff| buff.holds(p.durability));
    wear
}
