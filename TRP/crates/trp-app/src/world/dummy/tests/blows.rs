//! Tests of melee blows on a dummy: slashes through armour and poison.

use super::*;

/// A standing dummy of the USEC's starting kit, its armour at its
/// things' wear with every vest skill at level 1 (`SKILL_LEVEL`), one
/// hit collider of each type given (on its first bone, a sphere), its
/// voice listened to; and the kit.
fn struck_dummy(db: &trp_data::Database, install: &trp_data::Install, types: &[(u8, u8, &str)]) -> (Dummies, trp_sim::items::inventory::Item) {
    let kit = crate::dummy_kit(install, db, "usec").expect("the USEC's kit");
    let rules = Arc::new(ArmorRules::new(&db.config).expect("the armour's rules"));
    let wearer = wearer_of(db, rules, &kit, VestSkills::of_levels(|_| SKILL_LEVEL));
    let colliders = types
        .iter()
        .map(|&(part, collider_type, object)| {
            let data = HitColliderData {
                object: object.into(),
                bone: object.into(),
                local: None,
                shape: Shape::Sphere { centre: [0.0; 3], radius: 0.1 },
                part,
                collider_type,
                plate: 0,
                figures: [0.0; 6],
                damage_mod: 1.0,
            };
            Collider { data, bone: 0, local: Affine3A::IDENTITY, body: BodyRef { character: 1, part, collider: collider_type, plate: 0, damage_mod: 1.0 }, figures: [0.0; 6] }
        })
        .collect();
    let mut health = Health::new(Arc::new(HealthSettings::new(&db.config).expect("the health's settings")), 1);
    health.skills = health_skills(&SkillLevels::all_at(SKILL_LEVEL));
    let dummy = Dummy {
        which: 0,
        colliders,
        placed: Vec::new(),
        objects: Vec::new(),
        wearer,
        worn: false,
        health,
        followed: false,
        load: (0.0, Vec::new()),
        dressed: true,
        taken: Vec::new(),
        killing: None,
        fall: None,
        hits: 0,
        puppet: None,
        heading: 1.0,
        model: None,
        empty_grip: None,
        own: None,
        holds: None,
        drawn: Vec::new(),
        arm_bones: [[None; 6]; 2],
        locals: Vec::new(),
        parents: Arc::from(Vec::<Option<usize>>::new()),
        colliders_frame: None,
        before_frame: None,
        placements: Mutex::new(Vec::new()),
        drawn_by: PosingDrawn::EveryFrame,
        held_frame: None,
        heard: Vec::new(),
    };
    (Dummies { list: vec![dummy], bleeding_drops: Vec::new(), experience: Vec::new(), marks: Vec::new(), listened: true, economy: true, posing_log: PosingLog::default() }, kit)
}

/// The ice pick (`weapon_red_rebel_axe`): slash 29, stab 35, both of
/// penetration 1, no stamina burn.
const ICE_PICK: &str = "5c0126f40db834002a125382";

fn blow(db: &trp_data::Database, template: &str, stroke: trp_sim::Stroke, index: usize) -> crate::weapons::knife::Blow {
    let settings = trp_sim::KnifeSettings::of(&db.items[template].props);
    let (damage, penetration) = match stroke {
        trp_sim::Stroke::Slash => (settings.slash_damage, settings.slash_penetration),
        trp_sim::Stroke::Stab => (settings.stab_damage, settings.stab_penetration),
    };
    let target = crate::weapons::knife::Target { which: 0, index, object: [0.0, 1.4, 2.0] };
    crate::weapons::knife::Blow { target, stroke, damage: trp_sim::held::knife_hands::blow_damage(damage, None, 0.0), penetration, burn_rate: settings.stamina_burn_rate, direction: [0.0, 0.0, 0.9], point: [0.0, 1.4, 2.0] }
}

/// A knife's slash on the standing dummy's chest (grenades-and-knife
/// spec, "The knife's hands"; body hits spec, "The dummy on the
/// range"): through its PACA as a blow, nothing deflecting or blocking
/// it, penetration 1 leaving 0.6 of the damage, less the Light Vests'
/// cut at level 1 (0.006); the PACA worn by its least, 1, and that wear
/// written onto the vest's thing; its voice told of a `Melee` hit; the
/// player's experience of a melee weapon's hit. A stab's 35 the same
/// way; on an arm, which the PACA does not cover, whole.
#[test]
fn a_slash_on_the_dummys_chest_goes_through_its_paca() {
    let Some((install, db)) = installed() else { return };
    let (ribcage_up, left_upper_arm) = (1u8, 4u8);
    let (mut dummies, mut kit) = struck_dummy(db, install, &[(1, ribcage_up, "Base HumanSpine3"), (3, left_upper_arm, "Base HumanLUpperarm")]);
    let paca = dummies.list[0].wearer.pieces.iter().find(|p| p.covers(ribcage_up, 0)).cloned().expect("a piece over the chest");
    assert_eq!((paca.kind, paca.class, paca.durability), (armor::Kind::Light, 2, 40.0), "{}", paca.template);
    assert!(!dummies.list[0].wearer.pieces.iter().any(|p| p.covers(left_upper_arm, 0)), "nothing over the arm");
    let chest = dummies.list[0].health.part(Part::Chest).current;
    let outcome = dummies.take_blow(&blow(db, ICE_PICK, trp_sim::Stroke::Slash, 0), None).expect("taken");
    let through = 29.0 * 0.6 * (1.0 - 0.006);
    assert_eq!((outcome.part, outcome.damage_in), (Part::Chest, 29.0));
    assert!((outcome.taken.damage - through).abs() < 1e-4 && (outcome.taken.penetration - 0.6).abs() < 1e-6, "{outcome:?}");
    assert!((dummies.list[0].health.part(Part::Chest).current - (chest - through)).abs() < 1e-3);
    let worn = dummies.list[0].wearer.pieces.iter().find(|p| p.item == paca.item).expect("the PACA");
    assert_eq!(worn.durability, 39.0, "worn by its least");
    // Its wear onto its thing (`keep_dummy_wear`).
    assert!(dummies.list[0].worn && wear_differs(&dummies.list[0].wearer, &kit));
    keep_wear(&dummies.list[0].wearer, &mut kit);
    assert_eq!(kit.find(paca.item.expect("its thing")).map(|vest| vest.durability()[0]), Some(39.0));
    // Its voice, and the player's experience.
    let heard = dummies.list[0].take_heard();
    assert!(heard.iter().any(|h| matches!(h, Heard::Struck(Part::Chest, d, trp_sim::condition::health::DamageKind::Melee) if (d - through).abs() < 1e-4)), "{heard:?}");
    assert_eq!(dummies.experience.last().map(|hit| (hit.bullet, hit.melee, hit.killed)), Some((true, true, false)));
    // A stab: 35, the same way.
    let stab = dummies.take_blow(&blow(db, ICE_PICK, trp_sim::Stroke::Stab, 0), None).expect("taken");
    assert!((stab.taken.damage - 35.0 * 0.6 * 0.994).abs() < 1e-4, "{stab:?}");
    assert_eq!(dummies.list[0].wearer.pieces.iter().find(|p| p.item == paca.item).map(|p| p.durability), Some(38.0));
    // On the arm: all of it.
    let arm = dummies.take_blow(&blow(db, ICE_PICK, trp_sim::Stroke::Stab, 1), None).expect("taken");
    assert_eq!((arm.part, arm.taken.damage), (Part::LeftArm, 35.0));
}

/// The cultists' knife (`weapon_cultist_damask`, `Buffs_KultistsToxin`,
/// two doses): a blow that takes health puts its poison on the dummy
/// and spends a dose (`TryApplySideEffects`), its intoxication's buff
/// at work six seconds on; the third blow finds it empty. A blow that
/// takes nothing (on the dead) poisons nothing.
#[test]
fn the_poisoned_blade_poisons_the_dummy() {
    const DAMASK: &str = "5fc64ea372b0dd78d51159dc";
    let Some((install, db)) = installed() else { return };
    let (mut dummies, _) = struck_dummy(db, install, &[(3, 4, "Base HumanLUpperarm")]);
    let mut side = trp_sim::condition::health::SideEffect::of(&db.items[DAMASK].props).expect("its poison");
    assert_eq!((side.buffs.as_str(), side.value), ("Buffs_KultistsToxin", 2.0));
    let slash = blow(db, DAMASK, trp_sim::Stroke::Slash, 0);
    assert_eq!(slash.damage, 25.0);
    dummies.take_blow(&slash, Some((DAMASK, &mut side))).expect("taken");
    assert_eq!(side.value, 1.0);
    let health = &mut dummies.list[0].health;
    for _ in 0..(6.2 / (1.0 / 60.0)) as u32 {
        health.step(1.0 / 60.0);
    }
    assert!(health.effects.iter().any(|e| e.kind == trp_sim::condition::health::Kind::Intoxication), "poisoned");
    dummies.take_blow(&slash, Some((DAMASK, &mut side))).expect("taken");
    assert_eq!(side.value, 0.0);
    assert!(side.empty());
    dummies.take_blow(&slash, Some((DAMASK, &mut side))).expect("taken");
    assert_eq!(side.value, 0.0, "empty: nothing put on");
    // The dead take no blow.
    dummies.list[0].health.alive = false;
    let mut full = trp_sim::condition::health::SideEffect::of(&db.items[DAMASK].props).expect("its poison");
    assert!(dummies.take_blow(&slash, Some((DAMASK, &mut full))).is_none());
    assert_eq!(full.value, 2.0);
}
