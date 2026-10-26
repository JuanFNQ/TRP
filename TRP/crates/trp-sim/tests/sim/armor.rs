//! The armour read from the install and dressed as the game gathers it
//! (`docs/spec/body-hits.md`), and the player prefab's hit colliders.

use std::sync::Arc;

use trp_data::ragdoll::RagdollData;
use trp_data::unity::Assets;
use trp_sim::armor::{collider_type, plate, ArmorRules, Kind, VestSkills, Wearer, Worn};

fn worn(template: &str, slot: &str, children: Vec<Worn>) -> Worn {
    Worn { template: template.to_owned(), slot: slot.to_owned(), children }
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
    assert_eq!(mask.material, trp_sim::ballistics::material::HELMET);
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
    let shot = trp_sim::armor::Shot {
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
    let settings = Arc::new(trp_sim::health::HealthSettings::new(&db.config).expect("config.Health"));
    let mut health = trp_sim::health::Health::new(settings.clone(), 1);
    health.apply_damage(trp_sim::health::Part::Head, taken.damage, trp_sim::health::DamageKind::Bullet);
    health.blunt_contusion(trp_sim::health::Part::Head, taken.absorbed);
    let contusion = health.existing(trp_sim::health::Kind::Contusion, Some(trp_sim::health::Part::Head)).expect("concussed");
    assert_eq!((contusion.delay, contusion.strength), (0.1, 0.5));
    assert!((contusion.work - taken.absorbed / 4.0).abs() < 1e-4 && (contusion.residue - taken.absorbed / 4.0).abs() < 1e-4);
    // The chest's armour, or less than 10 taken: none.
    let mut health = trp_sim::health::Health::new(settings, 1);
    health.blunt_contusion(trp_sim::health::Part::Chest, 40.0);
    health.blunt_contusion(trp_sim::health::Part::Head, 9.9);
    assert!(health.existing(trp_sim::health::Kind::Contusion, None).is_none());
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
