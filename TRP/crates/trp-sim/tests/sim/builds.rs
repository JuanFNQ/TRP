//! Tests of a weapon made from its own parts, from the install.
//! After the inventory spec, "A weapon's build", through `trp_sim::inventory::rig_key` and
//! `trp_data::weapon::build_key`. These pass trivially, with a note, where no install is
//! configured.

use trp_data::character::inventory::Loadout;
use trp_data::unity::Assets;
use trp_data::Database;
use trp_sim::items::inventory::{build_of, key_template, rig_key, Item, MAGAZINE_SLOT};

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
    let key = trp_data::weapons::weapon::build_key(&build_of(&rifle));
    assert!(key.starts_with(&format!("{AKM}[")), "{key}");
    assert_eq!(key_template(&key), AKM);
    assert_eq!(Some(&key), rifle.kind.stock_key.as_ref());
    let again = trp_data::weapons::weapon::preset_of_key(&key).expect("the key read back");
    assert_eq!(trp_data::weapons::weapon::build_key(&again), key);
    assert_eq!(again.items.len(), build_of(&rifle).items.len());
    let from_key = trp_data::weapons::weapon::assemble_key(&mut assets, &db, &key).expect("assembled from its key");
    let stock = trp_data::weapons::weapon::assemble(&mut assets, &db, AKM).expect("the stock build");
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
    let bare = trp_data::weapons::weapon::assemble_key(&mut assets, &db, &key).expect("assembled from its key");
    let stock = trp_data::weapons::weapon::assemble(&mut assets, &db, AKM).expect("the stock build");
    assert_eq!(bare.magazine_capacity, None);
    assert!(bare.total_weight < stock.total_weight);
    assert!(bare.hierarchy.nodes.len() < stock.hierarchy.nodes.len());
    // With builds switched off, every weapon is its template.
    trp_sim::items::inventory::set_builds(false);
    let off = rig_key(&rifle);
    trp_sim::items::inventory::set_builds(true);
    assert_eq!(off, AKM);
}

/// Every weapon of the install with a stock preset: its key read back is
/// the same key (no slot's or template's name breaks the key's form).
#[test]
fn every_stock_build_s_key_reads_back() {
    let Some((db, _)) = install() else { return };
    let mut checked = 0;
    for preset in db.presets.iter().filter(|preset| preset.encyclopedia.is_some()) {
        let key = trp_data::weapons::weapon::build_key(preset);
        let again = trp_data::weapons::weapon::preset_of_key(&key).unwrap_or_else(|| panic!("{key} does not read back"));
        assert_eq!(trp_data::weapons::weapon::build_key(&again), key);
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
    use trp_sim::items::edit_build::{all_parts, build_slots, fits, ModClass};
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
    let kinds = db.items.keys().filter_map(|id| trp_sim::items::inventory::Kind::of(&db, id).ok().map(std::sync::Arc::new));
    let parts = all_parts(kinds);
    let magazines: Vec<_> = parts.iter().filter(|kind| fits(&rifle, magazine.owner, magazine.index, kind)).collect();
    assert!(magazines.len() > 3 && magazines.iter().all(|kind| kind.class_names.iter().any(|class| class == "Magazine")), "{}", magazines.len());
    assert!(magazines.iter().any(|kind| kind.template == own_magazine.template));
}

/// ASSEMBLE keeps the parts the weapon has that the build has, by
/// template, with what they hold, and takes away the rest.
#[test]
fn assembling_keeps_parts_by_template() {
    use trp_sim::items::edit_build::{already_built, assemble, build_slots, select};
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

/// The Akademia Bastion dust cover, which names the AKM's own rear sight
/// among its conflicting items.
const BASTION: &str = "5d2c76ed48f03532f2136169";
const AKM_REAR_SIGHT: &str = "59d650cf86f7741b846413a4";

/// The AKM's dust cover's dropdown lists the Bastion (its filter takes
/// it), but the bench refuses it while the AKM's rear sight is on, the
/// pair said with the sight first; the sight off, it goes on. The
/// inventory refuses it onto the weapon likewise.
#[test]
fn the_bastion_waits_for_the_rear_sight_to_come_off() {
    use trp_sim::items::edit_build::{build_slots, refusal, select, takes};
    use trp_sim::items::inventory::{move_item, take_out, Kind, Refusal, Rules, Spot};
    let Some((db, _)) = install() else { return };
    let mut rifle = stock(db, AKM);
    let bastion = std::sync::Arc::new(Kind::of(db, BASTION).expect("the Bastion"));
    let slots = build_slots(&rifle);
    let cover = slots.iter().find(|slot| slot.name == "mod_reciever").expect("a dust cover's slot").clone();
    let sight = slots.iter().find(|slot| slot.name == "mod_sight_rear").expect("a rear sight's slot").clone();
    let (sight_id, cover_id) = (sight.part.expect("a rear sight"), cover.part.expect("a dust cover"));
    assert_eq!(rifle.find(sight_id).unwrap().kind.template, AKM_REAR_SIGHT);
    assert!(takes(&rifle, cover.owner, cover.index, &bastion));
    let conflict = Refusal::Conflicting { existing: AKM_REAR_SIGHT.into(), new: BASTION.into() };
    assert_eq!(refusal(&rifle, cover.owner, cover.index, &bastion), Some(conflict.clone()));
    // In the inventory: the stock cover off, the Bastion refused.
    let rules = Rules { in_raid: false, ..Rules::of(db) };
    let mut ground = Item::new(std::sync::Arc::new(Kind::default()));
    let piece = Item::new(bastion.clone());
    let piece_id = piece.id;
    ground.loose.push(piece);
    take_out(&mut [&mut rifle], cover_id, &rules).expect("the stock dust cover off");
    let mut roots = [&mut rifle, &mut ground];
    assert_eq!(move_item(&mut roots, piece_id, cover.owner, Spot::Slot(cover.index), &rules), Err(conflict));
    // The sight off: the bench takes it, and so does the inventory.
    let mut build = roots[0].clone();
    assert!(select(&mut build, sight.owner, sight.index, None));
    assert_eq!(refusal(&build, cover.owner, cover.index, &bastion), None);
    take_out(&mut roots, sight_id, &rules).expect("the rear sight off");
    assert_eq!(move_item(&mut roots, piece_id, cover.owner, Spot::Slot(cover.index), &rules), Ok(()));
}

/// The install's limits of a raid (`RestrictionsInRaid`): 215 kinds, a
/// Blackjack backpack one, the multipurpose rig two, an arena crate none.
#[test]
fn the_install_s_raid_limits_are_read() {
    let Some((db, _)) = install() else { return };
    let limits = trp_sim::items::inventory::raid_limits(db);
    assert_eq!(limits.len(), 215);
    assert_eq!(limits.get("5c0e774286f77468413cc5b2"), Some(&1));
    assert_eq!(limits.get("5df8a42886f77412640e2e75"), Some(&2));
    assert_eq!(limits.get("66571bf06a723f7f005a0619"), Some(&0));
}

/// The LBT-6094A Slick plate carrier and the Ops-Core FAST MT helmet, as
/// the armor bench's screen has them.
const SLICK: &str = "5e4abb5086f77406975c9342";
const FAST_MT: &str = "5a154d5cfcdbcb001a3b00da";

/// At the armor bench a carrier's plates and soft
/// inserts are the parts of its build, which the game's weapon rules,
/// counting `Mod`s alone, would take for empty slots: none of its slots is
/// missing a part as it comes; a plate taken off and put to ASSEMBLE goes,
/// the rest kept; the app's check for gear (`same_parts`) has it built where
/// the game's (`already_built`) never does, its two plates being alike; its
/// inserts' slots are locked.
#[test]
fn gear_builds_take_plates_and_inserts() {
    use trp_sim::items::edit_build::{already_built, assemble, build_slots, same_parts, select};
    let Some((db, _)) = install() else { return };
    let mut carrier = stock(db, SLICK);
    let slots = build_slots(&carrier);
    let names: Vec<&str> = slots.iter().map(|slot| slot.name.as_str()).collect();
    assert_eq!(names, ["Front_plate", "Back_plate", "Soft_armor_front", "Soft_armor_back"]);
    assert!(slots.iter().all(|slot| slot.part.is_some() && !slot.missing), "{slots:?}");
    let mut build = carrier.clone();
    assert!(same_parts(&carrier, &build));
    // (Its two plates alike: the game's check counts it never built.)
    assert!(!already_built(&carrier, &build));
    let front = slots.iter().find(|slot| slot.name == "Front_plate").unwrap();
    assert!(select(&mut build, front.owner, front.index, None));
    assert!(!same_parts(&carrier, &build));
    // (One of the two like plates goes: kept by template, either may stay.)
    let gone = assemble(&mut carrier, &build);
    assert_eq!(gone.len(), 1);
    assert!(carrier.in_slot("Front_plate").is_none() && carrier.in_slot("Back_plate").is_some() && carrier.in_slot("Soft_armor_front").is_some());
    assert!(same_parts(&carrier, &build));
    // Its soft inserts' slots are locked (the screen opens no dropdown for
    // them: they are not to be modified), its
    // plates' not; a part offered to one is refused.
    let slots = build_slots(&build);
    assert!(slots.iter().all(|slot| slot.locked == slot.name.starts_with("Soft_armor")), "{slots:?}");
    let soft = slots.iter().find(|slot| slot.name == "Soft_armor_front").unwrap();
    let insert = carrier.in_slot("Soft_armor_front").unwrap().kind.clone();
    assert_eq!(trp_sim::items::edit_build::refusal(&build, soft.owner, soft.index, &insert), Some(trp_sim::items::inventory::Refusal::Locked));
}

/// The things offered to gear's slots (`all_gear_parts`): every item but a
/// round, so a carrier's front plate takes the plates its filter names and
/// a helmet's face shield slot its shields, none of them a `Mod`; the
/// weapon's list (`all_parts`) has none of them.
#[test]
fn gear_slots_are_offered_every_thing_that_fits() {
    use trp_sim::items::edit_build::{all_gear_parts, all_parts, build_slots, takes};
    let Some((db, _)) = install() else { return };
    let kinds: Vec<std::sync::Arc<trp_sim::items::inventory::Kind>> = db.items.keys().filter_map(|id| trp_sim::items::inventory::Kind::of(db, id).ok().map(std::sync::Arc::new)).collect();
    let gear = all_gear_parts(kinds.iter().cloned());
    let mods = all_parts(kinds.iter().cloned());
    assert!(gear.iter().all(|kind| !kind.class_names.iter().any(|class| class == "Ammo")));
    let carrier = stock(db, SLICK);
    let front = build_slots(&carrier).into_iter().find(|slot| slot.name == "Front_plate").unwrap();
    let plates: Vec<_> = gear.iter().filter(|kind| takes(&carrier, front.owner, front.index, kind)).collect();
    assert_eq!(plates.len(), 18, "the plates its filter names");
    assert!(plates.iter().all(|kind| kind.class_names.iter().any(|class| class == "ArmorPlate")));
    assert!(!mods.iter().any(|kind| takes(&carrier, front.owner, front.index, kind)));
    let helmet = stock(db, FAST_MT);
    let shield = build_slots(&helmet).into_iter().find(|slot| slot.name == "mod_equipment_000").unwrap();
    let shields: Vec<_> = gear.iter().filter(|kind| takes(&helmet, shield.owner, shield.index, kind)).map(|kind| kind.template.as_str()).collect();
    assert!(shields.contains(&"5a16b672fcdbcb001912fa83"), "the FAST visor: {shields:?}");
}
