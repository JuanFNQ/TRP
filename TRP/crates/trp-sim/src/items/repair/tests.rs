//! Tests of repairs: the window's reckoning, a kit's mending and the buffs' draws.
use super::*;

fn settings() -> Settings {
    Settings { cost_guns: 0.5, cost_armor: 0.335, armor_class_divisor: 6.0, min_level_to_buff: 3, materials: HashMap::new(), wear_per_level: [0.01, 0.008, 0.01, 0.004], intellect_cost: 0.004, buff: [[0.074, 0.0019, 30.0]; 3], max_bonus: [0.09, 0.09], kit_buffs: KitBuffs::from_config(&kit_config()) }
}

/// The install's `configs/repair.json` tables of a weapon and an armour
/// (SPT's figures), as far as the buffs go.
fn kit_config() -> Json {
    let range = |values: [f64; 2]| serde_json::json!({ "valuesMinMax": { "min": values[0], "max": values[1] }, "activeDurabilityPercentMinMax": { "min": 75, "max": 90 } });
    serde_json::json!({ "repairKit": {
        "weapon": {
            "rarityWeight": { "Common": 5, "Rare": 1 },
            "bonusTypeWeight": { "WeaponSpread": 1, "MalfunctionProtections": 1 },
            "Common": { "WeaponSpread": range([0.9, 0.99]), "MalfunctionProtections": range([0.94, 0.96]) },
            "Rare": { "WeaponSpread": range([0.8, 0.9]), "MalfunctionProtections": range([0.75, 0.9]), "WeaponDamage": range([0.3, 0.6]) }
        },
        "armor": {
            "rarityWeight": { "Common": 5, "Rare": 1 },
            "bonusTypeWeight": { "DamageReduction": 1 },
            "Common": { "DamageReduction": range([0.9, 0.98]) },
            "Rare": { "DamageReduction": range([0.8, 0.9]) }
        }
    } })
}

/// The AKM at 60 of 100: its kit's shares 0 and 0.035 (the install's).
fn akm(durability: f32, max: f32) -> Plan {
    Plan { parts: vec![Part { id: crate::items::inventory::Id::default(), durability, max, template_max: 100.0, kit: [0.0, 0.035], trader: [0.0, 0.04], cost: 0.5, governing: Governing::WeaponTreatment, buff_skill: Some(Governing::WeaponTreatment), record_max: 100.0 }], armour: false }
}

/// Draws given in turn, for a test.
fn draws(list: &[f64]) -> impl FnMut() -> f64 + '_ {
    let mut at = 0;
    move || {
        at += 1;
        list[at - 1]
    }
}

/// The server's gates and chance (`ShouldBuffItem`): none at level 2 or
/// at Weapon Maintenance 9; at level 10 paying 20 points (the AKM from
/// 60) 0.074 + 0.0019 × 10 × 0.01 = 0.07419; at 51 paying 100,
/// 0.074 + 0.0019 × 51 × (1 / 30) = 0.07723; none for a part the
/// server does not buff (a built-in insert).
#[test]
fn the_server_s_buff_chance_goes_by_its_gates_and_figures() {
    let s = settings();
    let part = &akm(60.0, 100.0).parts[0];
    let who = |level: u32, progress: f32| Repairer { profile_level: level, progress: [progress, 0.0, 0.0], ..Repairer::default() };
    assert_eq!(buff_chance(part, 20.0, &s, &who(2, 1000.0)), None, "the character under level 3");
    assert_eq!(buff_chance(part, 20.0, &s, &who(3, 999.0)), None, "Weapon Maintenance under 10");
    let ten = buff_chance(part, 20.0, &s, &who(3, 1000.0)).expect("a chance");
    assert!((ten - 0.07419).abs() < 1e-6, "{ten}");
    let elite = buff_chance(part, 100.0, &s, &who(60, 5100.0)).expect("a chance");
    assert!((elite - 0.07723).abs() < 1e-6, "{elite}");
    let insert = Part { buff_skill: None, governing: Governing::LightVests, ..part.clone() };
    assert_eq!(buff_chance(&insert, 20.0, &s, &Repairer { profile_level: 3, progress: [5100.0; 3], ..Repairer::default() }), None);
}

/// The buff by fixed draws (`AddBuff`): the rarity by 5 to 1 (a draw
/// of 0.5 is 3 of 6, Common; 0.9 is 5.4, Rare), the kind uniform by its
/// equal weights (the keys as read: MalfunctionProtections, then
/// WeaponSpread), the value and the threshold's share uniform in their
/// ranges, the threshold that share of the durability after, rounded:
/// Common spread 0.945 from 98 at 82.5 % is 80.85, 81; Rare protection
/// 0.75 at 75 % of 98 is 73.5, 74 (the even). An armour's one kind with
/// no draw.
#[test]
fn the_buff_s_rarity_kind_value_and_threshold_by_fixed_draws() {
    let s = settings();
    let common = roll_buff(&s.kit_buffs.weapon, 98.0, &mut draws(&[0.5, 0.7, 0.5, 0.5])).expect("a buff");
    assert_eq!((common.rarity, common.kind, common.threshold), (Rarity::Common, BuffKind::WeaponSpread, 81.0), "{common:?}");
    assert!((common.value - 0.945).abs() < 1e-12, "{common:?}");
    let rare = roll_buff(&s.kit_buffs.weapon, 98.0, &mut draws(&[0.9, 0.2, 0.0, 0.0])).expect("a buff");
    assert_eq!((rare.rarity, rare.kind, rare.value, rare.threshold), (Rarity::Rare, BuffKind::MalfunctionProtections, 0.75, 74.0), "{rare:?}");
    let armour = roll_buff(&s.kit_buffs.armour, 40.0, &mut draws(&[0.1, 1.0 - 1e-9, 1.0 - 1e-9])).expect("a buff");
    assert_eq!((armour.rarity, armour.kind, armour.threshold), (Rarity::Common, BuffKind::DamageReduction, 36.0), "{armour:?}");
    assert!((armour.value - 0.98).abs() < 1e-6, "{armour:?}");
    assert_eq!(common.percent(), 5, "(1 - 0.945) × 100 = 5.5, cut to 5");
    assert_eq!(common.attribute(), "WeaponSpreadCommonBuff");
}

/// The threshold's rounding (`Math.Round`, halves to the even): 81 % of
/// 50 is 40.5, 40; 83 % is 41.5, 42.
#[test]
fn the_threshold_rounds_its_halves_to_the_even() {
    let at = |percent: f64| BuffTable { rarities: vec![(Rarity::Common, 1.0)], kinds: vec![(BuffKind::MalfunctionProtections, 1.0)], ranges: vec![(Rarity::Common, BuffKind::MalfunctionProtections, [0.95, 0.95], [percent, percent])] };
    assert_eq!(roll_buff(&at(81.0), 50.0, &mut || 0.5).map(|buff| buff.threshold), Some(40.0));
    assert_eq!(roll_buff(&at(83.0), 50.0, &mut || 0.5).map(|buff| buff.threshold), Some(42.0));
}

/// A repair's buffs: the AKM from 60 by a trained character, its draw
/// under the chance, a buff at its durability after; over it, none; a
/// buff holds down to its threshold and is gone under it (and under 0.5
/// whatever it is).
#[test]
fn a_repair_draws_its_buff_and_the_buff_holds_to_its_threshold() {
    let s = settings();
    let who = Repairer { profile_level: 20, progress: [1000.0, 0.0, 0.0], ..Repairer::default() };
    let plan = akm(60.0, 100.0);
    let kit = crate::items::inventory::Id::default();
    let outcome = repair(&plan, 40.0, &[(kit, 1000.0)], &s, &who, &mut || 0.5);
    assert_eq!(outcome.paid, vec![Some(20.0)]);
    let won = buffs(&plan, &outcome, &s, &who, &mut draws(&[0.07, 0.5, 0.2, 0.5, 0.5]));
    let [(id, buff)] = won.as_slice() else { panic!("{won:?}") };
    assert_eq!((*id, buff.kind), (kit, BuffKind::MalfunctionProtections));
    assert_eq!(buff.threshold, (82.5f64 * 98.25 / 100.0).round_ties_even(), "of the durability after, 98.25");
    assert!(buffs(&plan, &outcome, &s, &who, &mut draws(&[0.08])).is_empty(), "over 0.07419");
    assert!(buff.holds(81.0) && !buff.holds(80.99), "{buff:?}");
    let low = RepairBuff { threshold: 0.0, ..*buff };
    assert!(low.holds(0.5) && !low.holds(0.49));
    assert_eq!(RepairBuff::factor(Some(buff), BuffKind::MalfunctionProtections), buff.value as f32);
    assert_eq!(RepairBuff::factor(Some(buff), BuffKind::WeaponSpread), 1.0);
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
    let kit = crate::items::inventory::Id::default();
    let outcome = repair(&plan, 40.0, &[(kit, 1000.0)], &s, &who, &mut || 0.5);
    let (_, durability, max) = outcome.parts[0];
    assert!((max - (100.0 - 1.75)).abs() < 1e-4, "{max}");
    assert!((durability - max).abs() < 1e-4, "capped at the new greatest");
    assert_eq!(outcome.kits, vec![(kit, Some(980.0))]);
    let small = repair(&plan, 40.0, &[(kit, 15.0)], &s, &who, &mut || 0.0);
    assert_eq!(small.kits, vec![(kit, None)], "used up and gone");
    assert!((small.parts[0].1 - 90.0).abs() < 1e-4, "15 points mend 30");
}
