//! Health read from the install (`docs/spec/health.md`): its figures, and
//! every medical item, food and drink of the database as a thing to use.
//! Passes with a note where no install is configured.
//!
//! `cargo test -p trp-sim --test sim health:: -- --nocapture` prints them.

use std::sync::Arc;

use trp_data::Database;
use trp_sim::health::{Cure, DamageKind, Health, HealthSettings, MedClass, Medicine, Part};

fn database() -> Option<&'static Database> {
    let Ok(_install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    Some(crate::fixture::database().expect("database"))
}

#[test]
fn the_install_s_health_figures_are_read() {
    let Some(db) = database() else { return };
    let s = HealthSettings::new(&db.config).expect("config.Health");
    println!("{s:#?}");
    assert_eq!(s.maximum, [35.0, 85.0, 70.0, 60.0, 60.0, 65.0, 65.0]);
    assert_eq!(s.maximum.iter().sum::<f32>(), 440.0);
    assert_eq!((s.fall_safe_height, s.fall_damage_per_meter), (3.0, 9.0));
    assert_eq!(s.over_damage_factor, [1.0, 1.0, 1.5, 0.7, 0.7, 1.0, 1.0]);
    assert_eq!(s.interruption_time, 2.0);
    let health = Health::new(Arc::new(s), 1);
    assert_eq!(health.common(), [440.0, 440.0]);
}

#[test]
fn every_medical_item_food_and_drink_is_read() {
    let Some(db) = database() else { return };
    let mut found: Vec<Medicine> = db
        .items
        .values()
        .filter(|item| {
            let names = db.class_names(&item.id);
            names.iter().any(|n| n == "Meds" || n == "FoodDrink") && !names.iter().any(|n| n == "Meds" && item.name == "Meds") && item.props.get("Prefab").is_some()
        })
        .filter_map(|item| Medicine::of(&db, &item.id))
        .collect();
    found.sort_by(|a, b| a.template.cmp(&b.template));
    for m in &found {
        println!("{} {:?} use {} s, holds {}, heals {} a use, energy {}, hydration {}, cures {:?}", db.items[&m.template].name, m.class, m.use_time, m.full(), m.hp_resource_rate, m.energy, m.hydration, m.cures.iter().map(|(c, s)| (*c, s.cost)).collect::<Vec<_>>());
    }
    println!("{} things to use", found.len());
    assert!(found.len() >= 80, "{}", found.len());
    let ifak = found.iter().find(|m| m.template == "590c678286f77426c9660122").expect("the IFAK");
    assert_eq!((ifak.class, ifak.use_time, ifak.max_hp_resource, ifak.hp_resource_rate), (MedClass::MedKit, 3.0, 300.0, 50.0));
    assert!(ifak.cures.iter().any(|(c, s)| *c == Cure::HeavyBleeding && s.cost == 210.0));
    let water = found.iter().find(|m| m.template == "5448fee04bdc2dbc018b4567").expect("the water");
    assert_eq!((water.class, water.max_resource, water.hydration), (MedClass::Drink, 60.0, 60.0));
    let cms = found.iter().find(|m| m.template == "5d02778e86f774203e7dedbe").expect("the CMS");
    assert_eq!(cms.part_priority, vec![Part::Stomach, Part::RightLeg, Part::LeftLeg, Part::RightArm, Part::LeftArm]);
    assert!(cms.cures.iter().any(|(c, s)| *c == Cure::DestroyedPart && s.kept == [25, 45]));
}

#[test]
fn a_splint_goes_to_the_broken_leg_and_a_cms_mends_it() {
    let Some(db) = database() else { return };
    let mut health = Health::new(Arc::new(HealthSettings::new(&db.config).expect("config.Health")), 3);
    let splint = Arc::new(Medicine::of(&db, "544fb3364bdc2d34748b456a").expect("the splint"));
    assert_eq!(health.part_to_apply(&splint, splint.full(), None), None, "nothing broken");
    health.do_fracture(Part::RightLeg);
    assert_eq!(health.part_to_apply(&splint, splint.full(), None), Some(Part::RightLeg));
    // A leg destroyed: the surgical kit takes it before anything else.
    health.apply_damage(Part::LeftLeg, 70.0, DamageKind::Fall);
    assert!(health.part(Part::LeftLeg).destroyed);
    let cms = Arc::new(Medicine::of(&db, "5d02778e86f774203e7dedbe").expect("the CMS"));
    assert_eq!(health.part_to_apply(&cms, cms.full(), None), Some(Part::LeftLeg));
}
