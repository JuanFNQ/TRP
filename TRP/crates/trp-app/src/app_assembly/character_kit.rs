//! The database loaded, the run's weapons named and the character's kit put together.

use super::*;

pub(super) struct Kit {
    pub(super) db: std::sync::Arc<Database>,
    pub(super) fixed_wear: Vec<String>,
    pub(super) equipment: trp_sim::items::inventory::Equipment,
}

/// Loads the database, names the run's weapons and puts together what the character carries.
pub(super) fn load(install: &Install, options: &mut Options) -> anyhow::Result<Kit> {
    // Shared with the threads that read away from the frame (`GameData::db`).
    let db = std::sync::Arc::new(Database::load(&install)?);
    platform::run_log::phase("the database");
    // A weapon may be named by a part of its name: the first of the
    // database's weapons, by name, that has it.
    let named = |wanted: &str| -> Option<String> {
        if db.items.contains_key(wanted) {
            return Some(wanted.to_owned());
        }
        let wanted = wanted.to_lowercase();
        let mut found: Vec<_> = db.items.values().filter(|item| item.name.starts_with("weapon_") && db.is_a(item, "Weapon") && item.name.to_lowercase().contains(&wanted)).collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found.first().map(|item| item.id.clone())
    };
    match named(&options.weapon) {
        Some(template) => options.weapon = template,
        None => eprintln!("no weapon is named {}", options.weapon),
    }
    for wanted in std::mem::take(&mut options.added) {
        match named(&wanted) {
            Some(template) if !options.weapons.contains(&template) => options.weapons.push(template),
            Some(_) => {}
            None => eprintln!("--add-weapon {wanted}: the database has no such weapon"),
        }
    }
    // The weapons of one's own lie in their crate of weapons
    // (`weapon_crates`); one that could not be read is said here.
    for note in &db.custom_notes {
        eprintln!("a weapon of one's own is left out: {note}");
    }
    // What the character carries: nothing, or a starting character's
    // equipment as the install's database has it (inventory spec). Its
    // weight is the carried weight unless `--weight` says otherwise, and
    // what sits in the slots the game draws is worn on the body.
    // What was asked to be worn whatever the equipment says (`--wear`).
    let fixed_wear = options.wear.clone();
    let mut equipment = trp_sim::items::inventory::Equipment::empty(&db)?;
    // The character's own pockets and dogtag, which a character of the game
    // always has and keeps in a raid (`CantRemoveFromSlotsDuringRaid`): a
    // BEAR's of the Standard edition, as the crate's vest and backpack are,
    // empty (the app's choice of edition and side: `PROVISIONAL`, the
    // user's profile not read).
    match trp_data::character::inventory::Loadout::load(&install, "Standard", "bear").and_then(|list| trp_sim::items::inventory::Equipment::of_loadout(&db, &list)) {
        Ok((standard, _)) => {
            for slot in OWN_SLOTS {
                let (Some(from), Some(to)) = (standard.item.slot_index(slot), equipment.item.slot_index(slot)) else { continue };
                if let Some(mut own) = standard.item.slots[from].clone() {
                    own.grids.iter_mut().for_each(Vec::clear);
                    own.slots.iter_mut().for_each(|part| *part = None);
                    equipment.item.slots[to] = Some(own);
                }
            }
        }
        Err(e) => eprintln!("the character's own pockets and dogtag could not be read: {e:#}"),
    }
    // The install's starting characters have no dogtag (SPT's server gives
    // a profile its own as it is made): a BEAR's tag (`dogtags_bear`).
    if let Some(slot) = equipment.item.slot_index("Dogtag").filter(|&slot| equipment.item.slots[slot].is_none()) {
        match trp_sim::items::inventory::Kind::of(&db, BEAR_DOGTAG) {
            Ok(kind) => equipment.item.slots[slot] = Some(trp_sim::items::inventory::Item::new(std::sync::Arc::new(kind))),
            Err(e) => eprintln!("the character's dogtag could not be made: {e:#}"),
        }
    }
    if let Some(loadout) = &options.loadout {
        let (edition, side) = loadout.split_once('/').unwrap_or((loadout.as_str(), "usec"));
        match trp_data::character::inventory::Loadout::load(&install, edition, side).and_then(|list| trp_sim::items::inventory::Equipment::of_loadout(&db, &list)) {
            Ok((worn, left_out)) => {
                for entry in left_out {
                    eprintln!("the loadout's {entry} has no place and is left out");
                }
                equipment = worn;
                if !options.weight_given {
                    options.weight = equipment.carried_weight(false);
                }
                options.wear.extend(equipment.drawn().into_iter().map(str::to_owned));
                println!("the loadout {loadout}: {:.2} kg carried", equipment.carried_weight(false));
            }
            Err(e) => eprintln!("the loadout {loadout} could not be read: {e:#}"),
        }
    }
    // What `--carry` asks for, as its default preset has it where the
    // install has one (a helmet with its armour), in the first free place of
    // the vest's and the pockets' grids, else where a pick-up would put it
    // (the backpack too).
    for template in &options.carry {
        let preset = trp_data::character::inventory::Loadout::of_preset(&db, template).and_then(|list| trp_sim::items::inventory::Item::of_list(&db, &list)).map(|(item, _)| item);
        let made = preset.or_else(|_| trp_sim::items::inventory::Kind::of(&db, template).map(|kind| trp_sim::items::inventory::Item::new(std::sync::Arc::new(kind))));
        match made {
            Ok(item) => {
                if let Some(left) = trp_sim::items::inventory::put_unloaded(&mut equipment.item, item) {
                    let rules = trp_sim::items::inventory::Rules::of(&db);
                    if trp_sim::items::inventory::pick_up(&mut equipment.item, left, &rules).0.is_err() {
                        eprintln!("--carry {template}: no room on the character");
                    }
                }
            }
            Err(e) => eprintln!("--carry {template}: {e:#}"),
        }
    }
    // What `--put-on` asks for, worn in the first slot that takes it.
    for wanted in &options.put_on {
        let made = match std::fs::read_to_string(wanted) {
            Ok(text) => serde_json::from_str::<trp_data::setup::database::Preset>(&text)
                .map_err(anyhow::Error::from)
                .and_then(|build| trp_sim::items::inventory::Item::of_list(&db, &trp_data::character::inventory::Loadout::of_build(&build)))
                .map(|(item, _)| item),
            Err(_) => trp_data::character::inventory::Loadout::of_preset(&db, wanted)
                .and_then(|list| trp_sim::items::inventory::Item::of_list(&db, &list))
                .map(|(item, _)| item)
                .or_else(|_| trp_sim::items::inventory::Kind::of(&db, wanted).map(|kind| trp_sim::items::inventory::Item::new(std::sync::Arc::new(kind)))),
        };
        match made {
            Ok(item) => match trp_sim::items::inventory::slot_to_wear(&equipment.item, &item) {
                Some(slot) => {
                    println!("--put-on {wanted}: {} worn in {}", item.kind.template, equipment.item.kind.slots[slot].name);
                    equipment.item.slots[slot] = Some(item);
                }
                None => eprintln!("--put-on {wanted}: no free slot of the equipment takes it"),
            },
            Err(e) => eprintln!("--put-on {wanted}: {e:#}"),
        }
    }
    // What `--shield-hits` asks for: the face shields on the head and the
    // face marked so many times, by a seed of `--seed`'s in a capture.
    if let Some(hits) = options.shield_hits {
        for slot in ["Headwear", "FaceCover"] {
            let Some(id) = equipment.worn(slot).and_then(trp_sim::items::inventory::face_shield).map(|shield| shield.id) else { continue };
            if let Some(shield) = equipment.item.find_mut(id) {
                shield.hits = hits;
                if let Some(seed) = fixed_seed(0x5eed) {
                    shield.hit_seed = seed as u8;
                }
                println!("--shield-hits {hits}: the {slot}'s face shield {} has {hits} hits, its seed {}", shield.kind.template, shield.hit_seed);
            }
        }
    }
    Ok(Kit { db, fixed_wear, equipment })
}
