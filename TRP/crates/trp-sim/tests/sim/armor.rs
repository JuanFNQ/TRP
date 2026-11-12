//! Tests of the armour read from the install and dressed as the game gathers it, and of the hit colliders.
//! After `docs/spec/body-hits.md`; the colliders are the player prefab's.

use std::sync::Arc;

use trp_data::character::ragdoll::RagdollData;
use trp_data::unity::Assets;
use trp_sim::condition::armor::{collider_type, plate, ArmorRules, Kind, VestSkills, Wearer, Worn};

fn worn(template: &str, slot: &str, children: Vec<Worn>) -> Worn {
    Worn { template: template.to_owned(), slot: slot.to_owned(), children, ..Worn::default() }
}

/// Every helmet the install gives no stock build (the FLUX, the Strike,
/// the winter 6B47, the Christmas Maska-1SCh: six) made as the game makes
/// one of every item (`Item::fill_locked`): its locked slots hold the
/// inserts their filters name, so nothing vital is missing and it can be
/// worn. Made bare, each was refused: such helmets could not be equipped.
#[test]
fn a_helmet_with_no_stock_build_comes_with_its_inserts() {
    use trp_sim::items::inventory::{Equipment, Item, Kind as ItemKind};
    let Ok(_install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let bare: Vec<&str> = db.items.values().filter(|item| item.prefab_key().is_some() && db.is_a(item, "Headwear") && db.default_preset(&item.id).is_err()).map(|item| item.id.as_str()).collect();
    let mut filled = 0;
    for template in bare {
        let mut helmet = Item::new(Arc::new(ItemKind::of(db, template).expect("the helmet")));
        let locked = helmet.kind.slots.iter().filter(|slot| slot.locked && slot.plate.is_some()).count();
        if locked == 0 {
            continue;
        }
        assert!(helmet.missing_vital(), "{template} bare");
        helmet.fill_locked(db).expect("its inserts");
        assert_eq!(helmet.slots.iter().flatten().count(), locked, "{template}");
        assert!(!helmet.missing_vital(), "{template} filled");
        assert_eq!(Equipment::empty(db).unwrap().can_wear("Headwear", &helmet), Ok(()), "{template}");
        filled += 1;
    }
    assert!(filled >= 6, "{filled} helmets filled");
}

/// Tagilla's recorded kit (`courses3`, `3-bot-armour`): his cap, his welding
/// mask, and his rig with two plates and two soft inserts. The plates in
/// their unlocked slots come first, then the mask, then the locked inserts;
/// the cap and the rig are no armour.
#[test]
fn tagilla_is_dressed_in_the_game_s_order() {
    let Ok(_install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let rules = Arc::new(ArmorRules::new(&db.config).expect("the armour's globals"));
    assert_eq!(rules.resistance_of(6), 60.0);
    assert_eq!(rules.blunt_soft, 0.6);
    assert_eq!(rules.colliders[collider_type("LeftForearm").unwrap() as usize], Some([5.0, 0.3, 0.67]));
    let equipment = vec![
        worn("60a7ad3a0c5cb24b0134664a", "FaceCover", Vec::new()),
        worn("60a7acf20c5cb24b01346648", "Headwear", Vec::new()),
        worn(
            "609e860ebd219504d8507525",
            "TacticalVest",
            vec![
                worn("656fa99800d62bcd2e024088", "Front_plate", Vec::new()),
                worn("656fa0fb498d1b7e3e071d9c", "Back_plate", Vec::new()),
                worn("6575f5cbf6a13a7b7100b0bf", "Soft_armor_front", Vec::new()),
                worn("6575f5e1da698a4e98067869", "Soft_armor_back", Vec::new()),
            ],
        ),
    ];
    let wearer = Wearer::dress(&db, rules, &equipment, VestSkills::default());
    let order: Vec<&str> = wearer.pieces.iter().map(|p| p.template.as_str()).collect();
    assert_eq!(order, ["656fa99800d62bcd2e024088", "656fa0fb498d1b7e3e071d9c", "60a7ad3a0c5cb24b0134664a", "6575f5cbf6a13a7b7100b0bf", "6575f5e1da698a4e98067869"]);
    let chest = plate("Plate_Granit_SAPI_chest").unwrap();
    let back = plate("Plate_Granit_SAPI_back").unwrap();
    assert_eq!(wearer.plates_on(), chest | back);
    let front = &wearer.pieces[0];
    // The Termite plate names both plates; its slot only the chest's.
    assert_eq!((front.class, front.plates, front.blunt_soft, front.locked, front.kind), (6, chest, true, false, Kind::Heavy));
    assert!((front.destructibility - 0.4125).abs() < 1e-6);
    let mask = &wearer.pieces[2];
    assert!(mask.covers(collider_type("ParietalHead").unwrap(), 0) && !mask.covers(collider_type("RibcageUp").unwrap(), 0));
    assert_eq!(mask.material, trp_sim::combat::ballistics::material::HELMET);
    let insert = &wearer.pieces[3];
    assert_eq!((insert.plates, insert.locked, insert.blunt_soft, insert.kind), (chest, true, false, Kind::Light));
}

/// A round a worn helmet stops still concusses through it (`BluntContusion`):
/// Tagilla's welding mask on the head blocks a pistol round, its blunt
/// throughput lets some damage through, and what the mask took (10 or more)
/// puts a contusion on the head for a quarter of it. A hit elsewhere, or one
/// the armour took less than 10 off, does not.
#[test]
fn a_helmet_that_stops_a_round_still_concusses() {
    let Ok(_install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let rules = Arc::new(ArmorRules::new(&db.config).expect("the armour's globals"));
    let mut wearer = Wearer::dress(&db, rules, &[worn("60a7ad3a0c5cb24b0134664a", "FaceCover", Vec::new())], VestSkills::default());
    let mask = wearer.pieces[0].id;
    let shot = trp_sim::condition::armor::Shot {
        damage: 60.0,
        penetration: 20.0,
        armor_damage: 0.4,
        deflected_by: None,
        blocked_by: Some(mask),
        stamina_burn: 0.0,
        heavy_bleeding: 0.0,
        light_bleeding: 0.0,
        weapon: true,
        melee: false,
    };
    let taken = wearer.take_shot(&shot, collider_type("ParietalHead").unwrap(), 0, true);
    println!("blocked: {} through, {} taken", taken.damage, taken.absorbed);
    assert!(!taken.penetrated && taken.damage > 0.0 && taken.absorbed >= 10.0, "{taken:?}");
    let settings = Arc::new(trp_sim::condition::health::HealthSettings::new(&db.config).expect("config.Health"));
    let mut health = trp_sim::condition::health::Health::new(settings.clone(), 1);
    health.apply_damage(trp_sim::condition::health::Part::Head, taken.damage, trp_sim::condition::health::DamageKind::Bullet);
    health.blunt_contusion(trp_sim::condition::health::Part::Head, taken.absorbed);
    let contusion = health.existing(trp_sim::condition::health::Kind::Contusion, Some(trp_sim::condition::health::Part::Head)).expect("concussed");
    assert_eq!((contusion.delay, contusion.strength), (0.1, 0.5));
    assert!((contusion.work - taken.absorbed / 4.0).abs() < 1e-4 && (contusion.residue - taken.absorbed / 4.0).abs() < 1e-4);
    // The chest's armour, or less than 10 taken: none.
    let mut health = trp_sim::condition::health::Health::new(settings, 1);
    health.blunt_contusion(trp_sim::condition::health::Part::Chest, 40.0);
    health.blunt_contusion(trp_sim::condition::health::Part::Head, 9.9);
    assert!(health.existing(trp_sim::condition::health::Kind::Contusion, None).is_none());
}

/// The ZSh-1-2M with its face shield (`5aa7e3abe5b5b000171d064d`) and the
/// FAST MT with its visor (`5a16b672fcdbcb001912fa83`), as the install has
/// them: each shield glass, a face shield, its own piece with its item's
/// number. Lowered, a bullet over the eyes marks it once; raised it is no
/// armour (`CompositeArmorComponent.ShotMatches`); a blast marks it from
/// 0.4 of its template durability on (20 of the ZSh's 50, 8 of the FAST's
/// 20: body hits spec, "A face shield's marks").
#[test]
fn a_face_shield_is_marked_lowered_and_no_armour_raised() {
    use trp_sim::combat::ballistics::material;
    let Ok(_install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let rules = Arc::new(ArmorRules::new(&db.config).expect("the armour's globals"));
    let helmet = |helmet: &str, shield: &str, up: bool| {
        let slot = if helmet == "5a154d5cfcdbcb001a3b00da" { "mod_equipment_000" } else { "mod_equipment" };
        let shield = Worn { template: shield.to_owned(), slot: slot.to_owned(), item: Some(7), up, ..Worn::default() };
        Worn { template: helmet.to_owned(), slot: "Headwear".to_owned(), children: vec![shield], item: Some(6), ..Worn::default() }
    };
    let eyes = collider_type("Eyes").unwrap();
    for (name, helmet_template, shield_template, needed) in [("ZSh-1-2M", "5aa7e454e5b5b0214e506fa2", "5aa7e3abe5b5b000171d064d", 20.0), ("FAST MT", "5a154d5cfcdbcb001a3b00da", "5a16b672fcdbcb001912fa83", 8.0)] {
        let mut wearer = Wearer::dress(&db, rules.clone(), &[helmet(helmet_template, shield_template, false)], VestSkills::default());
        let shield = wearer.pieces.iter().find(|p| p.template == shield_template).unwrap_or_else(|| panic!("{name}'s shield is armour"));
        assert!(shield.face_shield && shield.material == material::GLASS_VISOR && shield.on && shield.item == Some(7), "{name}: {shield:?}");
        assert!((shield.template_durability * 0.4 - needed).abs() < 1e-4 && (shield.explosion_destructibility - 0.6).abs() < 1e-6, "{name}: {shield:?}");
        assert!(shield.covers(eyes, 0), "{name} over the eyes");
        let shot = trp_sim::condition::armor::Shot {
            damage: 40.0,
            penetration: 25.0,
            armor_damage: 0.4,
            deflected_by: None,
            blocked_by: None,
            stamina_burn: 0.0,
            heavy_bleeding: 0.0,
            light_bleeding: 0.0,
            weapon: true,
            melee: false,
        };
        wearer.take_shot(&shot, eyes, 0, true);
        assert_eq!(wearer.drain().iter().map(|m| m.item).collect::<Vec<_>>(), [Some(7)], "{name}: one mark, on the shield");
        // A blast's share just under and at the threshold (the eyes' armour
        // value times the glass's 0.6).
        let at = |share: f32| (eyes, 0u16, share / 0.6);
        let mut under = Wearer::dress(&db, rules.clone(), &[helmet(helmet_template, shield_template, false)], VestSkills::default());
        under.take_blast(&[at(needed - 0.1)]);
        assert!(under.drain().is_empty(), "{name}: under the threshold");
        let mut over = Wearer::dress(&db, rules.clone(), &[helmet(helmet_template, shield_template, false)], VestSkills::default());
        over.take_blast(&[at(needed + 0.001)]);
        assert_eq!(over.drain().len(), 1, "{name}: at the threshold");
        // Raised: no armour at all.
        let mut raised = Wearer::dress(&db, rules.clone(), &[helmet(helmet_template, shield_template, true)], VestSkills::default());
        let shield = raised.pieces.iter().find(|p| p.template == shield_template).expect("still gathered");
        assert!(!shield.on && !shield.covers(eyes, 0), "{name} raised");
        raised.take_shot(&shot, eyes, 0, true);
        raised.take_blast(&[at(needed * 2.0)]);
        assert!(raised.drain().is_empty(), "{name} raised: no mark");
    }
}

/// The prefab's 27 body colliders and 12 plates, the forearm's figures as
/// the prefab has them (the globals set its level, chance and share again).
#[test]
fn the_player_prefab_has_the_game_s_hit_colliders() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let mut assets = Assets::new(install).expect("the assets");
    let colliders = RagdollData::hit_colliders(&mut assets).expect("the hit colliders");
    assert_eq!(colliders.iter().filter(|c| c.plate == 0).count(), 27);
    assert_eq!(colliders.iter().filter(|c| c.plate != 0).count(), 12);
    let forearm = colliders.iter().find(|c| c.object == "Base HumanLForearm1").expect("the left forearm");
    assert_eq!((forearm.part, forearm.collider_type, forearm.bone.as_str()), (3, collider_type("LeftForearm").unwrap(), "Base HumanLForearm1"));
    assert_eq!(forearm.figures, [20.0, 0.3, 0.0, 0.0, 0.441, 0.056]);
    let chest = colliders.iter().find(|c| c.object == "Plate_Granit_SAPI_chest").expect("the chest plate");
    assert_eq!((chest.part, chest.plate, chest.bone.as_str()), (1, 1, "Base HumanSpine3"));
    assert_eq!(chest.figures[..2], [0.0, 1.0]);
    assert!(chest.local.is_some());
}
