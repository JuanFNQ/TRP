//! Tests of the armour's pieces, face shields, wear and damage reduction.
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
        explosion_destructibility: 0.3,
        kind,
        blunt_throughput: blunt,
        ricochet: [0.0, 0.0, 80.0],
        colliders: 0,
        plates: 1,
        in_armor_slot: true,
        locked: false,
        blunt_soft: false,
        on: true,
        item: None,
        face_shield: false,
        speed_penalty: 0.0,
        ergonomics_penalty: 0.0,
        buff: None,
    }
}

fn wearer(pieces: Vec<Piece>) -> Wearer {
    Wearer { rules: Arc::new(rules()), pieces, skills: VestSkills::default(), invincible: false, marks: Vec::new() }
}

/// The ZSh-1-2M's face shield as the install has it: class 4 glass,
/// template durability 50, over `HeadCommon`, `Eyes` and `Jaw`, its
/// glass's destructibility 0.6 by a bullet and by a blast.
fn zsh_shield(id: u32) -> Piece {
    let mut shield = piece(id, 4, 50.0, Kind::None, 0.0, 0.6);
    shield.material = material::GLASS_VISOR;
    shield.explosion_destructibility = 0.6;
    shield.face_shield = true;
    shield.item = Some(100 + u64::from(id));
    shield.plates = 0;
    shield.colliders = [0u8, 15, 16].iter().fold(0, |m, k| m | 1 << k);
    shield
}

fn bullet(blocked_by: Option<u32>) -> Shot {
    Shot { damage: 50.0, penetration: 30.0, armor_damage: 0.4, deflected_by: None, blocked_by, stamina_burn: 0.0, heavy_bleeding: 0.0, light_bleeding: 0.0, weapon: true, melee: false }
}

const HEAD_COMMON: u8 = 0;
const EYES: u8 = 15;

/// A bullet on the lowered shield over the eyes marks it once, worn out
/// or not (`TryShatter` comes before the worn-out test); a blow or
/// anything not a weapon's does not.
#[test]
fn a_bullet_on_a_lowered_shield_marks_it_worn_out_too() {
    let mut w = wearer(vec![zsh_shield(1)]);
    w.take_shot(&bullet(None), EYES, 0, true);
    assert_eq!(w.drain(), [Mark { piece: 1, item: Some(101) }]);
    w.pieces[0].durability = 0.0;
    w.take_shot(&bullet(None), EYES, 0, true);
    assert_eq!(w.drain().len(), 1, "worn out, still marked");
    let mut not_a_weapon = bullet(None);
    not_a_weapon.weapon = false;
    w.take_shot(&not_a_weapon, EYES, 0, true);
    assert!(w.drain().is_empty());
    // Glass that is no face shield is not marked.
    w.pieces[0].face_shield = false;
    w.take_shot(&bullet(None), EYES, 0, true);
    assert!(w.drain().is_empty());
}

/// Raised, the shield is no armour (`CompositeArmorComponent.
/// ShotMatches`): nothing meets it, nothing marks it, a blast takes
/// nothing off it.
#[test]
fn a_raised_shield_is_no_armour() {
    let mut w = wearer(vec![zsh_shield(1)]);
    w.pieces[0].on = false;
    assert!(!w.pieces[0].covers(EYES, 0));
    assert_eq!(w.factor(EYES, 0, 30.0), None);
    let taken = w.take_shot(&bullet(None), EYES, 0, true);
    assert_eq!((taken.damage, taken.material), (50.0, material::BODY), "the bullet reaches the body whole");
    assert!(w.drain().is_empty());
    assert!(w.take_blast(&[(EYES, 0, 40.0)]).is_empty());
    assert_eq!(w.pieces[0].durability, 50.0);
    assert!(w.drain().is_empty());
}

/// A helmet that stops the bullet over `HeadCommon` before the shield
/// leaves the shield behind it untouched: only the pieces up to the one
/// that blocks take the hit, so it is not marked. Over the eyes, where
/// the helmet does not reach, the shield is the first and is marked.
#[test]
fn a_helmet_that_blocks_first_leaves_the_shield_unmarked() {
    let mut helmet = piece(1, 4, 40.0, Kind::Heavy, 0.2, 0.5);
    (helmet.plates, helmet.colliders, helmet.material) = (0, 1 << HEAD_COMMON, material::HELMET);
    let mut w = wearer(vec![helmet, zsh_shield(2)]);
    w.take_shot(&bullet(Some(1)), HEAD_COMMON, 0, true);
    assert!(w.drain().is_empty(), "behind the helmet that blocked it");
    w.take_shot(&bullet(None), HEAD_COMMON, 0, true);
    assert_eq!(w.drain(), [Mark { piece: 2, item: Some(102) }], "gone through the helmet, the shield takes it");
    w.take_shot(&bullet(None), EYES, 0, true);
    assert_eq!(w.drain().len(), 1);
}

/// A blast marks the shield where its share (the covered colliders'
/// armour values summed, times the glass's 0.6) is at least 0.4 of the
/// template durability: 20 of the ZSh-1-2M's 50. Marks come in the
/// order made, the shield's before its durability is taken.
#[test]
fn a_blast_marks_a_shield_from_four_tenths_of_its_template_durability() {
    // 19.9 of share: two colliders' values summed (33.1667), times 0.6.
    let mut w = wearer(vec![zsh_shield(1)]);
    let taken = w.take_blast(&[(EYES, 0, 19.9 / 0.6 - 10.0), (16, 0, 10.0)]);
    assert!((taken[0].1 - 19.9).abs() < 1e-4, "{taken:?}");
    assert!(w.drain().is_empty(), "19.9 of 50: none");
    let mut w = wearer(vec![zsh_shield(1)]);
    w.take_blast(&[(EYES, 0, 20.0 / 0.6)]);
    assert_eq!(w.drain().len(), 1, "20 of 50: marked");
    // Worn below the share, still marked: the template's counts.
    let mut w = wearer(vec![zsh_shield(1), zsh_shield(2)]);
    w.pieces[0].durability = 5.0;
    w.take_blast(&[(EYES, 0, 40.0)]);
    assert_eq!(w.drain().iter().map(|m| m.piece).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(w.pieces[0].durability, 0.0);
}

/// Four marks a window (`Player.FaceshieldMarkOperation`): of five
/// asked within one fixed step, four are made; one asked more than a
/// fixed step after the window began opens a new one.
#[test]
fn a_character_makes_four_marks_a_fixed_step() {
    let mut limit = MarkLimit::default();
    let step = 0.02;
    let made: Vec<bool> = (0..5).map(|k| limit.allow(10.0 + k as f32 * 0.003, step)).collect();
    assert_eq!(made, [true, true, true, true, false]);
    assert!(!limit.allow(10.02, step), "a fixed step on, not past it");
    assert!(limit.allow(10.0201, step), "past it: a new window");
    assert_eq!(limit.count, 1);
    // Eight pellets on one shield in one frame: four marks.
    let mut limit = MarkLimit::default();
    assert_eq!((0..8).filter(|_| limit.allow(3.0, step)).count(), 4);
}

#[test]
fn a_blast_takes_from_each_piece_the_sum_of_what_it_covers() {
    let rules = Arc::new(ArmorRules { class_resistance: vec![0.0; 7], blunt_soft: 0.0, heavy_current_threshold: 0.0, heavy_max_threshold: 0.0, heavy_deflect_chance: 0.0, colliders: [None; 27] });
    let mut front = piece(1, 4, 40.0, Kind::None, 0.0, 0.5);
    front.plates = 1;
    let mut soft = piece(2, 2, 10.0, Kind::None, 0.0, 0.5);
    (soft.plates, soft.colliders) = (0, 1 << 3);
    let mut wearer = Wearer { rules, pieces: vec![front, soft], skills: VestSkills::default(), invincible: false, marks: Vec::new() };
    // The front plate's collider at 20, two colliders of type 3 at 12
    // and 5 (the soft piece's), a collider no piece covers.
    let taken = wearer.take_blast(&[(0, 1, 20.0), (3, 0, 12.0), (5, 0, 9.0), (3, 0, 5.0)]);
    // Each times its material's 0.3.
    assert_eq!(taken.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![1, 2]);
    assert!((taken[0].1 - 6.0).abs() < 1e-5 && (taken[1].1 - 5.1).abs() < 1e-5, "{taken:?}");
    assert!((wearer.pieces[0].durability - 34.0).abs() < 1e-5);
    // Never below nought.
    wearer.take_blast(&[(3, 0, 100.0)]);
    assert_eq!(wearer.pieces[1].durability, 0.0);
}

/// A repair buff's damage reduction (`ArmorComponent.ApplyDamage`, read
/// from code): the damage a piece leaves times its value, the hit that
/// wears it under its threshold cut too (the cut comes before the wear),
/// the next not; a blast's wear under the threshold ends it as well.
#[test]
fn a_damage_reduction_buff_cuts_what_the_piece_leaves_until_its_threshold() {
    use crate::items::repair::{BuffKind, Rarity, RepairBuff};
    let buff = RepairBuff { rarity: Rarity::Common, kind: BuffKind::DamageReduction, value: 0.9, threshold: 38.0 };
    let made = |buff: Option<RepairBuff>| {
        let mut p = piece(1, 4, 40.0, Kind::None, 0.3, 0.5);
        (p.plates, p.colliders, p.buff) = (0, 1 << HEAD_COMMON, buff);
        p
    };
    let (mut bare, mut buffed) = (wearer(vec![made(None)]), wearer(vec![made(Some(buff))]));
    // Blocked: its wear 30 × 0.4 × 0.75 × 0.5 = 4.5, from 40 to 35.5.
    let (a, b) = (bare.take_shot(&bullet(Some(1)), HEAD_COMMON, 0, true), buffed.take_shot(&bullet(Some(1)), HEAD_COMMON, 0, true));
    assert!(a.damage > 0.0 && (b.damage - a.damage * 0.9).abs() < 1e-4, "{a:?} {b:?}");
    assert!((buffed.pieces[0].durability - 35.5).abs() < 1e-5 && buffed.pieces[0].buff.is_none(), "gone under 38");
    let (a, b) = (bare.take_shot(&bullet(Some(1)), HEAD_COMMON, 0, true), buffed.take_shot(&bullet(Some(1)), HEAD_COMMON, 0, true));
    assert_eq!(a.damage, b.damage, "no longer cut");
    // Gone through: cut the same way.
    let (mut bare, mut buffed) = (wearer(vec![made(None)]), wearer(vec![made(Some(RepairBuff { threshold: 1.0, ..buff }))]));
    let (a, b) = (bare.take_shot(&bullet(None), HEAD_COMMON, 0, true), buffed.take_shot(&bullet(None), HEAD_COMMON, 0, true));
    assert!(a.damage > 0.0 && (b.damage - a.damage * 0.9).abs() < 1e-4 && buffed.pieces[0].buff.is_some(), "{a:?} {b:?}");
    // A blast's wear: 5 × 0.3 leaves 38.5, held; 2 × 0.3 more, 37.9, gone.
    let mut blasted = wearer(vec![made(Some(buff))]);
    blasted.take_blast(&[(HEAD_COMMON, 0, 5.0)]);
    assert!(blasted.pieces[0].buff.is_some());
    blasted.take_blast(&[(HEAD_COMMON, 0, 2.0)]);
    assert!(blasted.pieces[0].buff.is_none(), "{:?}", blasted.pieces[0].durability);
}

/// A blow (`Shot::blow`: a knife's, `ArmorComponent.ApplyDamage`'s
/// gone-through branch, as nothing deflects or blocks it): penetration 1
/// against a fresh class 2 leaves 0.6 (`CF` held at its least) of the
/// damage and the penetration, a light piece then less its Light Vests'
/// cut (0.006 at level 1), a heavy one not; the piece worn by its least,
/// 1; the stamina burn as it came.
#[test]
fn a_blow_goes_through_light_armour_less_its_vest_skills_cut() {
    const RIBCAGE_UP: u8 = 1;
    let made = |kind: Kind| {
        let mut p = piece(1, 2, 40.0, kind, 0.3, 0.25);
        (p.plates, p.colliders) = (0, 1 << RIBCAGE_UP);
        p
    };
    let blow = Shot::blow(29.0, 1.0, 0.5);
    let mut light = wearer(vec![made(Kind::Light)]);
    light.skills.light_melee = 0.006;
    let taken = light.take_shot(&blow, RIBCAGE_UP, 0, true);
    assert!((taken.damage - 29.0 * 0.6 * 0.994).abs() < 1e-4, "{taken:?}");
    assert!((taken.penetration - 0.6).abs() < 1e-6 && taken.penetrated && taken.stamina_burn == 0.5, "{taken:?}");
    assert_eq!((light.pieces[0].durability, taken.material), (39.0, material::BODY_ARMOR));
    let mut untrained = wearer(vec![made(Kind::Light)]);
    assert!((untrained.take_shot(&blow, RIBCAGE_UP, 0, true).damage - 17.4).abs() < 1e-4);
    let mut heavy = wearer(vec![made(Kind::Heavy)]);
    heavy.skills.light_melee = 0.006;
    assert!((heavy.take_shot(&blow, RIBCAGE_UP, 0, true).damage - 17.4).abs() < 1e-4, "no cut on a heavy piece");
    // Off the armour: all of it.
    assert_eq!(light.take_shot(&blow, 4, 0, true).damage, 29.0);
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
    let mut wearer = Wearer { rules: Arc::new(rules()), pieces: vec![front, soft], skills: VestSkills::default(), invincible: false, marks: Vec::new() };
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
