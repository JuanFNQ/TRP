//! A weapon made from its own parts (`trp_sim::inventory::rig_key`,
//! `trp_data::weapon::build_key`; inventory spec, "A weapon's build"), from
//! the install. These pass trivially, with a note, where no install is
//! configured.

use trp_data::inventory::Loadout;
use trp_data::unity::Assets;
use trp_data::Database;
use trp_sim::inventory::{build_of, key_template, rig_key, Item, MAGAZINE_SLOT};

const AKM: &str = "59d6088586f774275f37482f";

fn install() -> Option<(&'static Database, Assets)> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let assets = Assets::new(install).expect("manifest");
    Some((db, assets))
}

fn stock(db: &Database, template: &str) -> Item {
    let list = Loadout::of_preset(db, template).expect("the stock preset");
    Item::of_list(db, &list).expect("the weapon as an item").0
}

/// The weapon as its stock build has it is asked for by its template, as
/// before builds; its build's key comes back from the key as the same
/// build, and assembles as the stock build does.
#[test]
fn a_stock_weapon_is_its_template_and_its_key_comes_back() {
    let Some((db, mut assets)) = install() else { return };
    let rifle = stock(&db, AKM);
    assert_eq!(rig_key(&rifle), AKM);
    let key = trp_data::weapon::build_key(&build_of(&rifle));
    assert!(key.starts_with(&format!("{AKM}[")), "{key}");
    assert_eq!(key_template(&key), AKM);
    assert_eq!(Some(&key), rifle.kind.stock_key.as_ref());
    let again = trp_data::weapon::preset_of_key(&key).expect("the key read back");
    assert_eq!(trp_data::weapon::build_key(&again), key);
    assert_eq!(again.items.len(), build_of(&rifle).items.len());
    let from_key = trp_data::weapon::assemble_key(&mut assets, &db, &key).expect("assembled from its key");
    let stock = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("the stock build");
    assert_eq!(from_key.hierarchy.nodes.len(), stock.hierarchy.nodes.len());
    assert!((from_key.total_ergonomics - stock.total_ergonomics).abs() < 1e-4);
    assert_eq!(from_key.magazine_capacity, stock.magazine_capacity);
}

/// A part taken off makes another build: asked for by its key, assembled
/// without it, and handled without it (the magazine's rounds and weight).
#[test]
fn a_part_taken_off_is_another_build() {
    let Some((db, mut assets)) = install() else { return };
    let mut rifle = stock(&db, AKM);
    let at = rifle.kind.slots.iter().position(|slot| slot.name == MAGAZINE_SLOT).expect("a magazine's slot");
    assert!(rifle.slots[at].take().is_some(), "the stock AKM has a magazine");
    let key = rig_key(&rifle);
    assert_ne!(key, AKM);
    assert!(!key.contains(MAGAZINE_SLOT), "{key}");
    let bare = trp_data::weapon::assemble_key(&mut assets, &db, &key).expect("assembled from its key");
    let stock = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("the stock build");
    assert_eq!(bare.magazine_capacity, None);
    assert!(bare.total_weight < stock.total_weight);
    assert!(bare.hierarchy.nodes.len() < stock.hierarchy.nodes.len());
    // With builds switched off, every weapon is its template.
    trp_sim::inventory::set_builds(false);
    let off = rig_key(&rifle);
    trp_sim::inventory::set_builds(true);
    assert_eq!(off, AKM);
}

/// Every weapon of the install with a stock preset: its key read back is
/// the same key (no slot's or template's name breaks the key's form).
#[test]
fn every_stock_build_s_key_reads_back() {
    let Some((db, _)) = install() else { return };
    let mut checked = 0;
    for preset in db.presets.iter().filter(|preset| preset.encyclopedia.is_some()) {
        let key = trp_data::weapon::build_key(preset);
        let again = trp_data::weapon::preset_of_key(&key).unwrap_or_else(|| panic!("{key} does not read back"));
        assert_eq!(trp_data::weapon::build_key(&again), key);
        checked += 1;
    }
    assert!(checked > 100, "{checked} stock builds");
}

/// A slot takes parts by its filters: the AKM's muzzle slot its own muzzle
/// and not a magazine; every part the game has that the magazine's slot
/// takes is a magazine, the stock one among them; and the build's slots
/// are the weapon's and its parts' but the chamber.
#[test]
fn a_slot_takes_parts_by_its_filters() {
    use trp_sim::edit_build::{all_parts, build_slots, fits, ModClass};
    let Some((db, _)) = install() else { return };
    let rifle = stock(&db, AKM);
    let slots = build_slots(&rifle);
    assert!(slots.iter().all(|slot| !slot.name.starts_with("patron_in_weapon")), "{slots:?}");
    let muzzle = slots.iter().find(|slot| slot.name == "mod_muzzle").expect("a muzzle's slot");
    let magazine = slots.iter().find(|slot| slot.name == MAGAZINE_SLOT).expect("a magazine's slot");
    assert_eq!(magazine.class, ModClass::Gear);
    let own_muzzle = rifle.find(muzzle.part.expect("a muzzle")).unwrap().kind.clone();
    let own_magazine = rifle.find(magazine.part.expect("a magazine")).unwrap().kind.clone();
    assert!(fits(&rifle, muzzle.owner, muzzle.index, &own_muzzle));
    assert!(!fits(&rifle, muzzle.owner, muzzle.index, &own_magazine));
    let kinds = db.items.keys().filter_map(|id| trp_sim::inventory::Kind::of(&db, id).ok().map(std::sync::Arc::new));
    let parts = all_parts(kinds);
    let magazines: Vec<_> = parts.iter().filter(|kind| fits(&rifle, magazine.owner, magazine.index, kind)).collect();
    assert!(magazines.len() > 3 && magazines.iter().all(|kind| kind.class_names.iter().any(|class| class == "Magazine")), "{}", magazines.len());
    assert!(magazines.iter().any(|kind| kind.template == own_magazine.template));
}

/// ASSEMBLE keeps the parts the weapon has that the build has, by
/// template, with what they hold, and takes away the rest.
#[test]
fn assembling_keeps_parts_by_template() {
    use trp_sim::edit_build::{already_built, assemble, build_slots, select};
    let Some((db, _)) = install() else { return };
    let mut rifle = stock(&db, AKM);
    let magazine_id = rifle.in_slot(MAGAZINE_SLOT).expect("a magazine").id;
    let mut build = rifle.clone();
    assert!(already_built(&rifle, &build));
    let muzzle = build_slots(&build).into_iter().find(|slot| slot.name == "mod_muzzle").expect("a muzzle's slot");
    let muzzle_id = muzzle.part.expect("a muzzle");
    assert!(select(&mut build, muzzle.owner, muzzle.index, None));
    assert!(!already_built(&rifle, &build));
    let gone = assemble(&mut rifle, &build);
    assert_eq!(gone, vec![muzzle_id]);
    assert_eq!(rifle.in_slot(MAGAZINE_SLOT).map(|magazine| magazine.id), Some(magazine_id));
    assert!(already_built(&rifle, &build));
    assert_eq!(rig_key(&rifle), rig_key(&build));
}
