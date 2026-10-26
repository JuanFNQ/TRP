//! TRP, the Tushonka Runtime Project: the first-person controller of the
//! game Single Player Tushonka runs on, rebuilt on Bevy.
//!
//! All game data is read at startup from the user's own install.

// ==== The modules ====

mod action_view;
mod animation;
mod arms;
mod battle_view;
mod sight_panels;
mod blast_view;
mod blast_sound;
mod blood_view;
mod contusion_view;
mod visor_view;
mod tactical_view;
mod range_day;
mod sharpen_view;
mod tinnitus;
mod voice_sound;
mod bullets;
mod cases;
mod impact_sound;
mod magazine_rounds;
mod muzzle_view;
mod particles_view;
mod explosion_view;
mod decal_view;
mod level;
mod lighting;
mod prism_view;
mod color_filter_view;
mod preview_tonemap;
mod scattering_view;
mod hbao_view;
mod heat_view;
mod clouds_view;
mod highlight_view;
mod optic_view;
mod shading;
mod frame_limit;
mod debug_menu;
mod park;
mod poppers;
mod rangefinder_view;
mod unready;
mod tod_sky_view;
mod distort_view;
mod step_sound;
mod thing_sound;
mod ui_sound;
mod body_view;
mod clothing;
mod edit_build;
mod info_preview;
mod controller;
mod convert;
mod cursor;
mod death;
mod dummy;
mod figure_view;
mod grid;
mod feet_down;
mod hands_view;
mod hands_sound;
mod shot_sound;
mod skills;

mod health_view;
mod hud;
mod icons;
mod setup;
mod inventory_view;
mod item_menu_view;
mod keys;
mod overall_view;
mod skills_view;
mod knife;
mod landing_sound;
mod loose;
mod backend;
mod gamma_ui;
mod screen_blur;
mod shadow_only;
mod shadows;
mod streaming;
mod options_view;
mod radial_fill;
mod range;
mod repair_view;
mod render_data;
mod render_timing;
mod weapon;
mod wear;
mod workbench;

use std::path::PathBuf;

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use bevy::window::PrimaryWindow;
use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::unity::Assets as UnityAssets;
use trp_data::{Database, Install};

use animation::ClipPlayer;
use controller::Controller;
use weapon::WeaponRig;

/// Kalashnikov AKM 7.62x39.
const DEFAULT_WEAPON: &str = "59d6088586f774275f37482f";
/// The equipment slots whose things are the character's own, which every run
/// begins with (the pockets and the dogtag).
const OWN_SLOTS: [&str; 2] = ["Pockets", "Dogtag"];
/// A BEAR's dogtag (`dogtags_bear`).
const BEAR_DOGTAG: &str = "59f32bb586f774757e1e8442";
/// The crate on the range: the install's airdrop crate, a loot container
/// with one grid of ten by twelve cells, which is the smallest of the
/// install's that holds a rifle, a vest and a backpack together (its
/// weapon boxes are five by five at most).
const CRATE: &str = "6223349b3136504a544d1608";
/// How many cells the crate has here: the install's ten across, and
/// thirty down where the install's has twelve. **The app's, by the user's
/// word** (2026-10-12: larger inside, with a scroll bar, to hold more).
const CRATE_CELLS: [u32; 2] = [10, 30];
/// How many full stacks of its own rounds lie in the crate for each
/// weapon the kit is not made for, and how many full magazines for each
/// such weapon fed from magazines. The app's choice.
const SPARE_STACKS: usize = 2;
const SPARE_MAGAZINES: usize = 3;

// ==== The range's crates, cabinets and the gear's rows ====

/// For a weapon whose magazine comes out to be changed
/// (`ReloadMode` `ExternalMagazine`): the kind of magazine its stock
/// preset has, how many rounds that holds, and the round the weapon
/// names as its own (`defAmmo`).
fn fed_from_magazines(db: &trp_data::Database, weapon: &str) -> Option<(std::sync::Arc<trp_sim::inventory::Kind>, u32, std::sync::Arc<trp_sim::inventory::Kind>)> {
    let item = db.items.get(weapon)?;
    if item.props.get("ReloadMode")?.as_str()? != "ExternalMagazine" {
        return None;
    }
    let preset = db.default_preset(weapon).ok()?;
    let magazine = &preset.items.iter().find(|part| part.slot.as_deref() == Some(trp_sim::inventory::MAGAZINE_SLOT))?.template;
    let capacity = db.items.get(magazine)?.props.get("Cartridges")?.get(0)?.get("_max_count")?.as_u64()? as u32;
    let round = item.props.get("defAmmo")?.as_str()?;
    let kind = |template: &str| trp_sim::inventory::Kind::of(db, template).ok().map(std::sync::Arc::new);
    Some((kind(magazine)?, capacity, kind(round)?))
}

/// A weapon as its stock preset has it, with what loads it: fed from
/// magazines, its own full (as the hands have it when it is drawn) and,
/// with `spares`, `SPARE_MAGAZINES` more of the kind, full of its own
/// round, which its reloads take (firing spec, "The quick reload");
/// otherwise, with `spares`, `SPARE_STACKS` full stacks of that round,
/// which a tube, a cylinder or barrels are filled from. The weapon first.
/// `--preset`'s build of the run's weapon: one of the database's by its id,
/// or one in the database's own form (`_id`, `_parent`, `_items` with
/// `_tpl`, `parentId`, `slotId`) in a file of that name.
fn preset_build(db: &trp_data::Database, wanted: &str, weapon: &str) -> anyhow::Result<trp_data::database::Preset> {
    use anyhow::Context;
    let build = match db.presets.iter().find(|preset| preset.id == wanted) {
        Some(preset) => preset.clone(),
        None => serde_json::from_str(&std::fs::read_to_string(wanted).with_context(|| format!("no build {wanted} in the database, nor a file of that name"))?)?,
    };
    let root = build.items.iter().find(|item| item.id == build.root).context("the build has no item at its root")?;
    anyhow::ensure!(root.template == weapon, "the build is of {}, not of the run's weapon {weapon} (`--weapon`)", root.template);
    Ok(build)
}

fn armed(db: &trp_data::Database, weapon: &str, spares: bool, build: Option<&trp_data::database::Preset>) -> anyhow::Result<Vec<trp_sim::inventory::Item>> {
    use trp_sim::inventory::{fill_magazine, Item, Kind, MAGAZINE_SLOT};
    // Its stock build, or the one asked for (`--preset`).
    let list = match build {
        Some(build) => trp_data::inventory::Loadout::of_build(build),
        None => trp_data::inventory::Loadout::of_preset(db, weapon)?,
    };
    let (mut item, _) = Item::of_list(db, &list)?;
    let mut made = Vec::new();
    match fed_from_magazines(db, weapon) {
        Some((magazine, capacity, round)) => {
            if let Some(fitted) = item.slot_index(MAGAZINE_SLOT).and_then(|slot| item.slots[slot].as_mut()) {
                fill_magazine(fitted, round.clone(), capacity);
            }
            for _ in (0..SPARE_MAGAZINES).filter(|_| spares) {
                let mut spare = Item::new(magazine.clone());
                fill_magazine(&mut spare, round.clone(), capacity);
                made.push(spare);
            }
        }
        // A rocket launcher's controller has no state to reload in: it
        // has its rocket and no more (more-weapons spec).
        None if !db.items.get(weapon).is_some_and(|item| db.is_a(item, "RocketLauncher")) => {
            let round = db.items.get(weapon).and_then(|item| item.props.get("defAmmo")?.as_str()).and_then(|round| db.items.get(round));
            if let Some(round) = round {
                let kind = std::sync::Arc::new(Kind::of(db, &round.id)?);
                // A tube full, as the hands have it when it is drawn.
                if let Some(tube) = item.slot_index(MAGAZINE_SLOT).and_then(|slot| item.slots[slot].as_mut()).filter(|tube| tube.kind.capacity > 0) {
                    let capacity = tube.kind.capacity;
                    fill_magazine(tube, kind.clone(), capacity);
                }
            }
            if let Some(round) = round.filter(|_| spares) {
                let kind = std::sync::Arc::new(Kind::of(db, &round.id)?);
                for _ in 0..SPARE_STACKS {
                    let mut stack = Item::new(kind.clone());
                    stack.count = round.stack_max();
                    made.push(stack);
                }
            }
        }
        None => {}
    }
    made.insert(0, item);
    Ok(made)
}

/// Whether the simulation has the logic a weapon runs on
/// (`docs/spec/more-weapons.md`): every kind of the install's but a weapon
/// fired once and thrown away (`IsOneoff`, whose firing is not read; no
/// weapon with a stock preset in the install read is one).
fn logic_built(db: &trp_data::Database, weapon: &str) -> bool {
    let Some(item) = db.items.get(weapon) else { return false };
    !item.props.get("IsOneoff").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// Which crate of weapons a weapon goes into (`range::WEAPON_CRATES`): a
/// weapon of one's own is custom; any other goes by the class its template
/// gives it (`weapClass`, which the weapon skills go by too; the item's
/// place among the database's classes would have the M32A1 a revolver).
fn crate_of(db: &trp_data::Database, weapon: &str) -> Option<usize> {
    if db.custom.contains_key(weapon) {
        return Some(7);
    }
    match db.items.get(weapon)?.props.get("weapClass")?.as_str()? {
        "assaultRifle" | "assaultCarbine" => Some(0),
        "smg" => Some(1),
        "machinegun" => Some(2),
        "sniperRifle" | "marksmanRifle" => Some(3),
        "shotgun" => Some(4),
        "pistol" => Some(5),
        "grenadeLauncher" | "specialWeapon" => Some(6),
        _ => None,
    }
}

/// The game's own weapon box and lootable ammunition box (`WeaponCrate`, a
/// grid of 5 by 2; `AmmoBox`, 3 by 3), which the tables of weapons' gun
/// cases and magazine boxes are.
const GUN_CASE: &str = "578f87ad245977356274f2cc";
const MAGAZINE_BOX: &str = "5909d45286f77465a8136dc6";

/// The templates a slot's or a magazine's filter names, a class among them
/// standing for every item under it: those with a model, not a quest's.
fn filtered(db: &trp_data::Database, takes: &[String]) -> Vec<String> {
    let quest = |item: &trp_data::database::ItemTemplate| item.props.get("QuestItem").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let under = |item: &trp_data::database::ItemTemplate, class: &str| {
        let mut at = Some(item);
        while let Some(template) = at {
            if template.id == class {
                return true;
            }
            at = db.items.get(&template.parent);
        }
        false
    };
    let mut found = Vec::new();
    for id in takes {
        let Some(item) = db.items.get(id) else { continue };
        if item.prefab_key().is_some() {
            if !quest(item) {
                found.push(id.clone());
            }
            continue;
        }
        found.extend(db.items.values().filter(|other| other.prefab_key().is_some() && !quest(other) && under(other, id)).map(|other| other.id.clone()));
    }
    found.sort();
    found.dedup();
    found
}

/// What a table's weapons load (the user, 2026-10-24: "One of each
/// different type/model of magazine and then a separate box for their
/// respective ammunition, one stack each of all compatible ammo types",
/// every compatible model, loaded full): one of every magazine any of them
/// takes, as its magazine slot's filter says (weapons fed from magazines;
/// its stock preset's own too), full of the first weapon's own round where
/// the magazine takes it, else of the first round it takes; and a full
/// stack of every round any of them or their magazines take (the chambers'
/// filters, and the magazines'), by name. The app's.
fn table_loads(db: &trp_data::Database, weapons: &[&String]) -> (Vec<trp_sim::inventory::Item>, Vec<trp_sim::inventory::Item>) {
    use trp_sim::inventory::{fill_magazine, Item, Kind, MAGAZINE_SLOT};
    let kind = |template: &str| Kind::of(db, template).ok().map(std::sync::Arc::new);
    // Each magazine model with the round of the first weapon that takes it.
    let mut magazines: std::collections::BTreeMap<String, Option<String>> = std::collections::BTreeMap::new();
    let mut rounds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for weapon in weapons {
        let Some(item) = db.items.get(weapon.as_str()) else { continue };
        let own_round = item.props.get("defAmmo").and_then(serde_json::Value::as_str).map(str::to_owned);
        for chamber in item.slots().into_iter().filter(|slot| slot.name.starts_with("patron_in_weapon")) {
            rounds.extend(filtered(db, &chamber.takes));
        }
        if item.props.get("ReloadMode").and_then(serde_json::Value::as_str) != Some("ExternalMagazine") {
            // A tube, a cylinder or barrels: what is fitted takes rounds,
            // which the box of rounds has.
            if let Some(round) = &own_round {
                rounds.insert(round.clone());
            }
            continue;
        }
        let mut takes: Vec<String> = item.slots().into_iter().filter(|slot| slot.name == MAGAZINE_SLOT).flat_map(|slot| filtered(db, &slot.takes)).collect();
        if let Ok(preset) = db.default_preset(weapon) {
            takes.extend(preset.items.iter().filter(|part| part.slot.as_deref() == Some(MAGAZINE_SLOT)).map(|part| part.template.clone()));
        }
        for magazine in takes {
            if db.items.get(&magazine).is_some_and(|it| db.is_a(it, "Magazine")) {
                magazines.entry(magazine).or_insert_with(|| own_round.clone());
            }
        }
    }
    let mut loaded = Vec::new();
    for (magazine, own_round) in &magazines {
        let Some(template) = db.items.get(magazine) else { continue };
        let takes: Vec<String> = template.cartridges().into_iter().flat_map(|holder| filtered(db, &holder.takes)).collect();
        rounds.extend(takes.iter().cloned());
        let capacity = template.props.get("Cartridges").and_then(|c| c.get(0)?.get("_max_count")?.as_u64()).unwrap_or(0) as u32;
        let round = own_round.as_ref().filter(|round| takes.contains(round)).or_else(|| takes.first());
        let Some(kind_of) = kind(magazine) else {
            eprintln!("no magazine {magazine} for the tables");
            continue;
        };
        let mut item = Item::new(kind_of);
        if let Some(round) = round.and_then(|round| kind(round)) {
            if capacity > 0 {
                fill_magazine(&mut item, round, capacity);
            }
        }
        loaded.push(item);
    }
    // Rounds only (a filter may name a grenade launcher's grenades: those
    // are rounds too), each a full stack, by calibre and then by name.
    let caliber = |item: &trp_data::database::ItemTemplate| item.props.get("Caliber").and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
    let mut named: Vec<(String, &String, &String)> = rounds.iter().filter_map(|id| db.items.get(id).filter(|it| db.is_a(it, "Ammo")).map(|it| (caliber(it), &it.name, &it.id))).collect();
    named.sort();
    let mut stacks = Vec::new();
    for (_, _, id) in named {
        let Some(round) = db.items.get(id) else { continue };
        let Some(kind_of) = kind(id) else { continue };
        let mut stack = Item::new(kind_of);
        stack.count = round.stack_max();
        stacks.push(stack);
    }
    (loaded, stacks)
}

/// What each table of weapons holds, in `range::WEAPON_CRATES`' order: in
/// its gun case every weapon of the install of its class that has a stock
/// preset and runs on a logic built, and every weapon of one's own, by
/// name, its own magazine full; in its magazine box one of every magazine
/// they take, full, and in its box of rounds (the tables of weapons' alone,
/// `range::AMMO_TABLES`) a stack of every round (`table_loads`). Each is its
/// game's container, ten cells across as the range's crates are, and as
/// deep as what it holds (the app's: the game's own hold one weapon, and
/// nine things).
type Crates = (Vec<trp_sim::inventory::Item>, Vec<trp_sim::inventory::Item>, Vec<trp_sim::inventory::Item>);

fn weapon_crates(db: &trp_data::Database) -> anyhow::Result<Crates> {
    use trp_sim::inventory::{Item, Kind};
    /// Deeper than any needs, before each is cut to what it holds.
    const DEEPEST: u32 = 2000;
    let deep = |template: &str| -> anyhow::Result<Kind> {
        let mut kind = Kind::of(db, template)?;
        kind.grids.first_mut().ok_or_else(|| anyhow::anyhow!("{template} has no grid"))?.cells = [CRATE_CELLS[0], DEEPEST];
        Ok(kind)
    };
    let (case_kind, box_kind) = (deep(GUN_CASE)?, deep(MAGAZINE_BOX)?);
    let (case_deep, box_deep) = (std::sync::Arc::new(case_kind.clone()), std::sync::Arc::new(box_kind.clone()));
    let mut cases: Vec<Item> = range::WEAPON_CRATES.iter().map(|_| Item::new(case_deep.clone())).collect();
    let mut boxes: Vec<Item> = range::WEAPON_CRATES.iter().map(|_| Item::new(box_deep.clone())).collect();
    let mut ammo: Vec<Item> = (0..range::AMMO_TABLES).map(|_| Item::new(box_deep.clone())).collect();
    let mut on_table: Vec<Vec<&String>> = vec![Vec::new(); range::WEAPON_CRATES.len()];
    let mut weapons: Vec<(&String, &String)> = db
        .items
        .values()
        .filter(|item| (item.name.starts_with("weapon_") && db.is_a(item, "Weapon")) || db.custom.contains_key(&item.id))
        .filter(|item| logic_built(db, &item.id) && db.default_preset(&item.id).is_ok())
        .map(|item| (&item.name, &item.id))
        .collect();
    weapons.sort();
    let mut counts = [0; range::WEAPON_CRATES.len()];
    for (name, weapon) in weapons {
        let Some(at) = crate_of(db, weapon) else {
            eprintln!("{name} is of no class a crate of weapons has");
            continue;
        };
        match armed(db, weapon, false, None) {
            Ok(made) => {
                // The weapon, its own magazine full.
                for thing in made {
                    if cases[at].stow(thing).is_err() {
                        eprintln!("the {} weapon case has no room for {name}", range::WEAPON_CRATES[at]);
                    }
                }
                on_table[at].push(weapon);
                counts[at] += 1;
            }
            Err(e) => eprintln!("the tables of weapons have no {name}: {e:#}"),
        }
    }
    // What loads each table's weapons: its magazines in its magazine box,
    // its rounds in its box of rounds.
    for (at, weapons) in on_table.iter().enumerate().take(range::AMMO_TABLES) {
        let (magazines, stacks) = table_loads(db, weapons);
        let label = range::WEAPON_CRATES[at];
        let (made, rounds) = (magazines.len(), stacks.len());
        for magazine in magazines {
            if boxes[at].stow(magazine).is_err() {
                eprintln!("the {label} magazine box has no room for a magazine");
            }
        }
        for stack in stacks {
            if ammo[at].stow(stack).is_err() {
                eprintln!("the {label} box of rounds has no room for a stack");
            }
        }
        println!("the table of {label}: {made} magazines, one of each its weapons take, and {rounds} kinds of round");
    }
    // The medical table (`range::MEDICAL`): one of every medical item of
    // the install in its case, one of every food and drink in its box (the
    // user, 2026-10-14, night: "A table of all of them"); quest items left
    // out.
    if let Some(at) = range::WEAPON_CRATES.iter().position(|label| *label == range::MEDICAL) {
        let quest = |item: &trp_data::database::ItemTemplate| item.props.get("QuestItem").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let mut supplies: Vec<(&String, &String, bool)> = db
            .items
            .values()
            .filter(|item| item.prefab_key().is_some() && !quest(item))
            .filter_map(|item| {
                let names = db.class_names(&item.id);
                let food = names.iter().any(|name| name == "FoodDrink");
                (food || names.iter().any(|name| name == "Meds")).then_some((&item.name, &item.id, food))
            })
            .collect();
        supplies.sort();
        for (name, template, food) in supplies {
            match Kind::of(db, template) {
                Ok(kind) => {
                    let into = if food { &mut boxes[at] } else { &mut cases[at] };
                    if into.stow(Item::new(std::sync::Arc::new(kind))).is_err() {
                        eprintln!("the medical table has no room for {name}");
                    }
                    counts[at] += 1;
                }
                Err(e) => eprintln!("the medical table has no {name}: {e:#}"),
            }
        }
    }
    // The grenades' table (`range::GRENADES`): one of every grenade of the
    // install in its case, one of every melee weapon in its box, and the
    // handheld rangefinder with them (rangefinder spec; the user,
    // 2026-10-26: "put the range finder somewhere").
    if let Some(at) = range::WEAPON_CRATES.iter().position(|label| *label == range::GRENADES) {
        let quest = |item: &trp_data::database::ItemTemplate| item.props.get("QuestItem").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let mut kept: Vec<(&String, &String, bool)> = db
            .items
            .values()
            .filter(|item| item.prefab_key().is_some() && !quest(item))
            .filter_map(|item| {
                let names = db.class_names(&item.id);
                let melee = names.iter().any(|name| name == "Knife" || name == "PortableRangeFinder");
                (melee || names.iter().any(|name| name == "ThrowWeap")).then_some((&item.name, &item.id, melee))
            })
            .collect();
        kept.sort();
        for (name, template, melee) in kept {
            match Kind::of(db, template) {
                Ok(kind) => {
                    let into = if melee { &mut boxes[at] } else { &mut cases[at] };
                    if into.stow(Item::new(std::sync::Arc::new(kind))).is_err() {
                        eprintln!("the grenades' table has no room for {name}");
                    }
                    counts[at] += 1;
                }
                Err(e) => eprintln!("the grenades' table has no {name}: {e:#}"),
            }
        }
    }
    // Each cut to the rows it fills.
    let cut = |item: &mut Item, kind: &Kind| {
        let rows = item.grids[0].iter().map(|(place, thing)| place.y + if place.turned { thing.cells()[0] } else { thing.cells()[1] }).max().unwrap_or(0);
        let mut cut = kind.clone();
        cut.grids[0].cells = [CRATE_CELLS[0], rows.max(1)];
        item.kind = std::sync::Arc::new(cut);
        item.kind.grids[0].cells[1]
    };
    for (((case, held), label), count) in cases.iter_mut().zip(boxes.iter_mut()).zip(range::WEAPON_CRATES).zip(counts) {
        let (case_rows, box_rows) = (cut(case, &case_kind), cut(held, &box_kind));
        if label == range::MEDICAL {
            println!("the table of {label}: {count} medical items, food and drink, in its case ({case_rows} rows) and its box ({box_rows} rows)");
        } else if label == range::GRENADES {
            println!("the table of {label}: {count} grenades and melee weapons, in its case ({case_rows} rows) and its box ({box_rows} rows)");
        } else {
            println!("the table of {label}: {count} weapons in its case ({case_rows} rows), their magazines in its box ({box_rows} rows)");
        }
    }
    for rounds in ammo.iter_mut() {
        cut(rounds, &box_kind);
    }
    Ok((cases, boxes, ammo))
}
/// The storage cabinets' container: the row's drawer's (`CONTAINERS`).
const CABINET: &str = "578f87b7245977356274f2cd";

/// What each place of the storage holds, in `range::CABINETS`' order (the
/// user, 2026-10-19: "for Rigs, backpacks, helmets, armored rigs and armor,
/// masks and hats, headsets", then "Add a plates box, and eyewear dresser.
/// We don't need quest items"): one of every item of the install of its sort, as
/// its stock preset has it where it has one (an armour or a helmet with its
/// inserts), else bare; quest items, those with no model and the suits of
/// one's own (the CLOTHING crate's) left out. Rigs are the vests without
/// armour's slots; armour the body armour and the vests with them; helmets
/// the headwear with them, and since 0.98.0 the helmets' own parts (the
/// user, 2026-10-21: "add the missing visors": the face shields, and with
/// them the mandibles, ear covers, side armour and appliqués, armoured
/// equipment of no other sort, which no storage took); masks and hats the
/// face covers and the rest of the headwear; headsets the headphones;
/// plates the armour plates (`ArmorPlate`, but its `BuiltInInserts`: the
/// soft inserts, which come out of no armour in the game);
/// eyewear the glasses and goggles (`Visors`). Armour's slots are an
/// insert's or a plate's: any but an attachment's (`mod_`); or an armour
/// class of its own. Each is the drawer's container (the plates' the
/// magazine boxes' own) ten cells across, as deep as what it holds (the
/// app's, as the tables').
fn gear_cabinets(db: &trp_data::Database) -> anyhow::Result<Vec<trp_sim::inventory::Item>> {
    use serde_json::Value as Json;
    use trp_data::database::ItemTemplate;
    use trp_sim::inventory::{Item, Kind};
    const DEEPEST: u32 = 2000;
    let deep = |template: &str| -> anyhow::Result<Kind> {
        let mut kind = Kind::of(db, template)?;
        kind.grids.first_mut().ok_or_else(|| anyhow::anyhow!("{template} has no grid"))?.cells = [CRATE_CELLS[0], DEEPEST];
        Ok(kind)
    };
    let kinds: Vec<Kind> = range::CABINETS.iter().map(|label| deep(if *label == range::PLATES { MAGAZINE_BOX } else { CABINET })).collect::<anyhow::Result<_>>()?;
    let mut cabinets: Vec<Item> = kinds.iter().map(|kind| Item::new(std::sync::Arc::new(kind.clone()))).collect();
    let quest = |item: &ItemTemplate| item.props.get("QuestItem").and_then(Json::as_bool).unwrap_or(false);
    let sort = |item: &ItemTemplate| cabinet_of(db, item);
    let mut gear: Vec<(&String, &String, usize)> = db
        .items
        .values()
        .filter(|item| item.prefab_key().is_some() && !quest(item) && !db.outfits.contains_key(&item.id) && !db.custom.contains_key(&item.id))
        .filter_map(|item| Some((&item.name, &item.id, sort(item)?)))
        .collect();
    gear.sort();
    let mut counts = [0; range::CABINETS.len()];
    for (name, template, into) in gear {
        let made = if db.default_preset(template).is_ok() {
            trp_data::inventory::Loadout::of_preset(db, template).and_then(|list| Item::of_list(db, &list)).map(|(item, _)| item)
        } else {
            Kind::of(db, template).map(|kind| Item::new(std::sync::Arc::new(kind)))
        };
        match made {
            Ok(item) => {
                if cabinets[into].stow(item).is_err() {
                    eprintln!("the {} storage has no room for {name}", range::CABINETS[into]);
                }
                counts[into] += 1;
            }
            Err(e) => eprintln!("the {} storage has no {name}: {e:#}", range::CABINETS[into]),
        }
    }
    // Each cut to the rows it fills.
    for ((cabinet, label), count) in cabinets.iter_mut().zip(range::CABINETS).zip(counts) {
        let rows = cut_to_rows(cabinet);
        println!("the {label} storage: {count} things ({rows} rows)");
    }
    Ok(cabinets)
}

/// Which storage cabinet a thing goes into, by its place in
/// `range::CABINETS` (`gear_cabinets`), if any: rigs the vests without
/// armour's slots; armour the body armour and the vests with them;
/// helmets the headwear with them and the helmets' parts (armoured
/// equipment of no other sort: face shields, mandibles, ear covers, side
/// armour, appliqués); masks and hats the face covers and the rest of the
/// headwear; headsets the headphones; plates the armour plates but the
/// built-in soft inserts; eyewear the glasses and goggles.
/// Armour's slots are an insert's or a plate's: any but an attachment's
/// (`mod_`); or an armour class of its own.
fn cabinet_of(db: &trp_data::Database, item: &trp_data::database::ItemTemplate) -> Option<usize> {
    use serde_json::Value as Json;
    let armoured = || {
        let class = item.props.get("armorClass").and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(0.0);
        let slots = item.props.get("Slots").and_then(Json::as_array).into_iter().flatten().filter_map(|slot| slot.get("_name").and_then(Json::as_str));
        class > 0.0 || slots.into_iter().any(|name| !name.starts_with("mod_"))
    };
    let at = |label: &str| range::CABINETS.iter().position(|l| *l == label);
    if db.is_a(item, "Vest") {
        at(if armoured() { "ARMOR" } else { "RIGS" })
    } else if db.is_a(item, "Backpack") {
        at("BACKPACKS")
    } else if db.is_a(item, "Headwear") {
        at(if armoured() { "HELMETS" } else { "MASKS AND HATS" })
    } else if db.is_a(item, "Armor") {
        at("ARMOR")
    } else if db.is_a(item, "FaceCover") {
        at("MASKS AND HATS")
    } else if db.is_a(item, "Headphones") {
        at("HEADSETS")
    } else if db.is_a(item, "ArmorPlate") && !db.is_a(item, "BuiltInInserts") {
        at(range::PLATES)
    } else if db.is_a(item, "Visors") {
        at(range::EYEWEAR)
    } else if db.is_a(item, "ArmoredEquipment") && !db.is_a(item, "ArmorPlate") && !db.is_a(item, "BuiltInInserts") {
        at("HELMETS")
    } else {
        None
    }
}

/// A container of the range (a gun case, a cabinet's drawer) cut to the
/// rows of its first grid that what it holds fills, at least one: the
/// rows.
fn cut_to_rows(container: &mut trp_sim::inventory::Item) -> u32 {
    let rows = container.grids[0].iter().map(|(place, thing)| place.y + if place.turned { thing.cells()[0] } else { thing.cells()[1] }).max().unwrap_or(0).max(1);
    let mut cut = (*container.kind).clone();
    cut.grids[0].cells[1] = rows;
    container.kind = std::sync::Arc::new(cut);
    rows
}

/// A thing put into a container of the range already cut to what it held
/// (`cut_to_rows`): the container made deep for it, then cut again. Where
/// it went, or why it could not.
fn stow_more(container: &mut trp_sim::inventory::Item, thing: trp_sim::inventory::Item) -> Result<(), trp_sim::inventory::Refusal> {
    let mut deep = (*container.kind).clone();
    deep.grids[0].cells[1] = 2000;
    container.kind = std::sync::Arc::new(deep);
    let stowed = container.stow(thing).map(|_| ());
    cut_to_rows(container);
    stowed
}

/// The install's repair kits (`RepairKits`): the body armour's and the
/// weapons' (`item_spec_armorrepair`, `item_spec_weaprepair`), in the order
/// of `range::REPAIR_BOXES`, and how many of each a box holds (the user's
/// "multiple copies"; the app's number).
const REPAIR_KITS: [&str; 2] = ["591094e086f7747caa7bb2ef", "5910968f86f77425cf569c32"];
const REPAIR_KITS_A_BOX: usize = 6;

/// The workbench's two boxes of repair kits (`range::REPAIR_BOXES`): the
/// game's ammunition box made deep, filled with its kits, and cut to what
/// it holds; a kit the install lacks is left out with a word.
fn repair_boxes(db: &trp_data::Database) -> Vec<trp_sim::inventory::Item> {
    use trp_sim::inventory::{Item, Kind};
    REPAIR_KITS
        .iter()
        .map(|template| {
            let mut kind = match Kind::of(db, MAGAZINE_BOX) {
                Ok(kind) => kind,
                Err(e) => {
                    eprintln!("no box for the repair kits: {e:#}");
                    return Item::new(std::sync::Arc::new(Kind::default()));
                }
            };
            if let Some(grid) = kind.grids.first_mut() {
                grid.cells[1] = 2000;
            }
            let mut holder = Item::new(std::sync::Arc::new(kind));
            match Kind::of(db, template) {
                Ok(kit) => {
                    let kit = std::sync::Arc::new(kit);
                    for _ in 0..REPAIR_KITS_A_BOX {
                        if holder.stow(Item::new(kit.clone())).is_err() {
                            eprintln!("the repair kits' box has no room for {}", kit.name);
                        }
                    }
                    println!("the workbench's box: {} x {}", REPAIR_KITS_A_BOX, kit.name);
                }
                Err(e) => eprintln!("there is no repair kit {template}: {e:#}"),
            }
            cut_to_rows(&mut holder);
            holder
        })
        .collect()
}

/// The key of the range's locked door: a key of the install's (the first
/// dormitory's marked room, by its name there). Any key would do; a door
/// is opened by the key whose item it names (`KeyTemplate.KeyId` is the
/// item's own id).
pub const DOOR_KEY: &str = "5780cf7f2459777de4559322";
/// The range's other containers, the install's items of the sorts recorded
/// by hand on 2026-10-12, in the order the range keeps them
/// (`range::Placed::holds`, after the airdrop crate): a wooden crate, a
/// technical supply crate, a dead Scav and a drawer. They start empty.
const CONTAINERS: [&str; 4] = ["578f87ad245977356274f2cc", "5d6fd45b86f774317075ed43", "5909e4b686f7747f5b744fa4", "578f87b7245977356274f2cd"];
/// Things lying about beyond the doors, to be taken up from a table, the
/// floor and a shelf (where `range::lying` has them): the rifle's own
/// magazine (empty) and a stack of its rounds.
const LYING: [(&str, u32); 3] = [("59d625f086f774661516605d", 1), ("64b7af5a8532cf95ee0a0dbd", 30), ("59d625f086f774661516605d", 1)];
/// The window the app starts in, in pixels. The options tab may change
/// it for someone at the keys; a run nobody is at the keys of keeps it,
/// since its scripted clicks go by places in a window of this size.
pub const WINDOW_SIZE: [u32; 2] = [1600, 900];
/// Stock BEAR arms.
const DEFAULT_ARMS: &str = "assets/content/hands/bear/bear_hands_skin.bundle";
/// The clothing that goes with those arms by default: the same faction's
/// own upper body and legs (`DefaultBearBody` and `DefaultBearFeet` in the
/// install's customization list).
const DEFAULT_BODY: &str = "assets/content/characters/character/prefabs/tshirt_bear_turtleneck.bundle";
const DEFAULT_LEGS: &str = "assets/content/characters/character/prefabs/pants_bear_srvv.bundle";
/// The first of the same faction's heads in the install's customization
/// list. Like the upper body it is a shadow and nothing more in first
/// person.
const DEFAULT_HEAD: &str = "assets/content/characters/character/prefabs/bear_head_0.bundle";

// ==== The run's data, the view and the options ====

#[derive(Resource)]
pub struct GameData {
    pub unity: UnityAssets,
    pub db: Database,
    pub hard: HardSettings,
    pub engine: EngineSettings,
    /// Tuning of the weapon motion, taken by the weapon when it is spawned.
    pub hands_data: Option<HandsData>,
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Playing: the camera rides the weapon's animated camera bone.
    FirstPerson,
    /// Free camera for looking at the rig from outside.
    Free,
    /// Playing, watched from a camera that stands outside the character
    /// and keeps its place beside it: the keys and the mouse are the
    /// character's, and the picture is what someone else would see of it.
    Watched,
}

impl View {
    /// Whether the keys and the mouse are the character's, and not the
    /// free camera's.
    pub fn played(self) -> bool {
        self != View::Free
    }
}

/// Where the watching camera stands from the character's feet (`View::Watched`).
#[derive(Resource, Default)]
struct Watching(Option<Vec3>);

#[derive(Resource)]
pub struct Options {
    /// Item template id of the weapon.
    pub weapon: String,
    /// What the airdrop crate holds besides the run's own weapon: those
    /// asked for (`--add-weapon`). Every weapon whose logic is built lies
    /// in the crates of weapons along the knee-high wall (`weapon_crates`).
    pub weapons: Vec<String>,
    /// `--measure-rigs <crate|all>`: the crate of weapons (by its label, or
    /// all of them) whose every weapon's rig is made ready and measured,
    /// not put into the world, before the run ends (`weapon::Measured`);
    /// then their templates.
    measure_crate: Option<String>,
    pub measure_rigs: Vec<String>,
    /// Manifest key of the arms bundle.
    pub arms: String,
    /// The bundles of the clothing the body is drawn in: its upper body and
    /// its legs. `none` leaves one out.
    pub body: String,
    pub legs: String,
    pub head: String,
    /// Suits of the install's customization list to begin in, by their ids
    /// (`--suit`, as often as wanted: an upper one, a lower one), over the
    /// choice the options' file keeps.
    pub suits: Vec<String>,
    /// What the character wears over its clothing: item template ids or
    /// bundles, any number.
    pub wear: Vec<String>,
    /// Things put in the vest or the pockets as the run begins, by their
    /// templates (`--carry`, as often as wanted: a testing aid, as
    /// `--damage` is).
    pub carry: Vec<String>,
    /// Things laid on the floor ahead of where the run starts, by their
    /// templates (`--lay`, as often as wanted: a testing aid, as `--carry`
    /// is).
    pub lay: Vec<String>,
    /// Weapons asked into the crate beside the app's own (`--add-weapon`),
    /// as given: a template, or a part of a name.
    added: Vec<String>,
    /// `--preset <id>`: the run's own weapon as another of the database's
    /// builds of it (`ItemPresets`, by its id), in its gun case and in hand
    /// with `--equip`, where the cases hold the stock builds (a testing
    /// aid: the M4A1 with a canted sight is `5a8ae36686f774377d6ce226`).
    preset: Option<String>,
    /// Carried weight in kilograms.
    weight: f32,
    view: View,
    /// Whose eyes the character is posed for, where the run says
    /// (`--pov first|third`); otherwise its own while the picture is its
    /// own, and another's from any camera outside it.
    pov: Option<trp_sim::PointOfView>,
    /// Clips to play at start, in order.
    play: Vec<String>,
    /// Freeze the first clip at this many seconds.
    hold_at: Option<f32>,
    /// Keys held for the whole run, joined by `+`: w a s d shift q e jump aim fire.
    pub hold: Option<String>,
    /// The game's field-of-view setting: vertical degrees, 50 to 75.
    pub field_of_view: f32,
    /// Whether `--fov` gave it, which then has the say over what the
    /// options tab last kept.
    fov_given: bool,
    /// The game's head-bobbing setting, 0.2 to 1.
    pub head_bobbing: f32,
    /// The game's mouse sensitivity setting.
    pub sensitivity: f32,
    /// The game's setting for the mouse's sensitivity with the sights up.
    /// The weapon's own factor multiplies it.
    pub aiming_sensitivity: f32,
    /// The game's setting for how far a turn of the wheel zooms a variable
    /// scope (`OpticSensitivity`, 1 to 5).
    pub optic_sensitivity: f32,
    /// Whether the command line gave either, which then has the say over
    /// what the options tab last kept.
    sensitivity_given: [bool; 2],
    /// Degrees a count of the mouse turns the view before the
    /// sensitivity (`keys::degrees_a_count`), and the game's two switches
    /// that turn an axis of the mouse about, across and up and down.
    pub degrees_a_count: f32,
    pub invert: [bool; 2],
    /// Degrees a count of the mouse turns the head's free look
    /// (`keys::look_degrees_a_count`): no sensitivity comes after it.
    pub look_degrees_a_count: f32,
    /// Seed of the recoil's random numbers.
    pub seed: u64,
    /// What the character's skills add to the aiming speed, as a fraction,
    /// for this run over Aim Drills' level (`--aim-speed-bonus`).
    pub aim_speed_bonus: Option<f32>,
    /// The character's mastering of the weapon, 0 to 3. From 2 a reload
    /// keeps the sights up.
    pub mastering: u32,
    /// Skill levels for this run over the options tab's (`skills`): every
    /// weapon class's (`--weapon-skill`), Strength's, Endurance's, and any
    /// by the game's name (`--skill Name=level`), each 0 to 51.
    pub weapon_skill: Option<u32>,
    pub skill_levels: Vec<(String, u32)>,
    /// Levels set in play on a drawn frame (`--skill-at frame:Name=level`),
    /// as a host game would set them: what the options tab does then.
    pub skill_at: Vec<(u32, String, u32)>,
    /// Frames a second the simulation runs at, whatever the rate of drawing.
    pub frame_rate: f32,
    /// The character's Strength and Endurance levels for this run, 0 to 51.
    pub strength: Option<u32>,
    pub endurance: Option<u32>,
    /// Where the view starts, in degrees below level: a capture run
    /// ignores the mouse, so this is how it looks down or up.
    pitch: f32,
    /// Which way the character starts facing, in degrees to the right of
    /// the lane: a capture run walks up to the range's walls with it.
    yaw: f32,
    /// Whether `--yaw` or `--pitch` was given: a `--start` before one of
    /// the range's things otherwise turns the view to it.
    view_given: bool,
    /// The moment `CUSTOMS LIGHT` lights the range at, where `--hour` or
    /// `--cloudiness` says another than the app's (`lighting::RANGE_MOMENT`).
    hour: Option<f32>,
    cloudiness: Option<f32>,
    /// `--malfunction <kind>`: the weapon first taken in hand malfunctions
    /// so at its next shot (`misfire`, `jam`, `feed`, `soft`, `hard`); and
    /// `--heat <n>`: it is taken in hand that hot (0 to 200), as if just
    /// fired. For a check (`crate::wear`).
    pub malfunction: Option<trp_sim::malfunction::Malf>,
    pub heat: Option<f32>,
    /// `--repair <durability>`: the first weapon in a slot worn down to that
    /// durability, and its repair window opened with the first repair kit
    /// carried, the inventory open for the repair. For a check
    /// (`repair_view`).
    pub repair: Option<f32>,
    /// Save one frame here and exit. Used to check the app unattended.
    screenshot: Option<PathBuf>,
    /// With `--screenshot`, the region of the frame kept, in the window's
    /// pixels (`--screenshot-crop x,y,w,h`): a reader of a capture pays for
    /// its pixels, and a screen's corner is a fraction of the whole.
    screenshot_crop: Option<[u32; 4]>,
    /// Everything the app's sounds mixed, from the start to the run's end,
    /// written here as a WAVE file (`--sound-out`): to hear, and set beside
    /// a recording of the game, what a run played.
    sound_out: Option<PathBuf>,
    /// A line for each sound of moving played, written here (`--log-sounds`).
    log_sounds: Option<PathBuf>,
    /// Frame on which that capture is taken.
    screenshot_frame: u32,
    /// How many frames in a row are captured from it (`--screenshot-frames`).
    screenshot_count: u32,
    /// The window's size as the run begins (`--resolution`; the options'
    /// own resolution has the last word for someone at the keys).
    window: [u32; 2],
    /// Write one line per rendered frame here: the clip on screen and
    /// where the weapon and the hands are as the camera sees them.
    log_frames: Option<PathBuf>,
    /// A level file to read for the run (`--level`; `level`).
    level: Option<PathBuf>,
    /// Write the range as a level file there and end (`--export-level`).
    export_level: Option<PathBuf>,
    /// Write one line per rendered frame here: how long the frame took,
    /// what the simulation ran and cost in it, and where the camera is.
    log_pacing: Option<PathBuf>,
    /// Degrees a second a capture run turns the view to the right, from a
    /// second in.
    pub turn: f32,
    /// One of the recorder's scripts to play as the input, in place of
    /// the keyboard and mouse (`--scenario <file>`), and how many seconds
    /// into the run it begins (`--scenario-at`).
    pub scenario: Option<trp_sim::script::Scenario>,
    pub scenario_at: f32,
    /// Whether a view from outside takes in the whole figure.
    whole: bool,
    /// How far round the character the watching camera stands from its
    /// usual place, degrees (`--view watch:<degrees>`).
    watch_turn: f32,
    /// A capture from outside sees the range from overhead, from this many
    /// metres over where the character starts (`--view map[:<metres>]`).
    map: Option<f32>,
    /// Where in the range the character starts, if not in its middle:
    /// `platform`, `shelf` or `stairs`.
    start: Option<String>,
    /// A starting character of the install's database whose equipment the
    /// character carries and wears, as `Standard/usec`; nothing if not
    /// given.
    loadout: Option<String>,
    /// Whether `--weight` was given, which then has the say over the
    /// equipment's own weight.
    weight_given: bool,
    /// Start with the inventory screen open (for a capture).
    inventory: bool,
    /// Start with the crate open in that screen (`--loot`).
    loot: bool,
    /// Start with what is in the crate put on by the inventory's own
    /// moves: the weapon on the sling, the vest and the backpack worn
    /// (`--equip`). A run nobody is at the keys of does so unless told
    /// `--bare`, so that it has its weapon in hand as before.
    equip: bool,
    bare: bool,
    /// Whether a weapon hangs where the hands take one from as the run
    /// begins (`--equip`, a run nobody is at the keys of, a loadout): the
    /// run begins with it in hand. Otherwise it begins with empty hands and
    /// every weapon's rig is made when it is first wanted (`weapon::
    /// spawn_weapon`).
    pub starts_armed: bool,
    /// Drags of the mouse to make in a run nobody is at the keys of, each
    /// from one place in the window to another, in pixels (`--drag
    /// x:y:x:y`, as often as wanted): how the inventory screen's moving
    /// of things is tried without a hand on the mouse.
    drags: Vec<[f32; 4]>,
    /// The drawn frame the first of them begins on (`--drag-at`; 90 unless
    /// told), for a screen that opens later than that.
    drag_at: u32,
    /// Moves of the mouse with no button, each on a drawn frame to a place
    /// in the window, in pixels (`--move <frame>:<x>:<y>`): onto a line of
    /// a menu, to open its own lines.
    moves: Vec<(u32, f32, f32)>,
    /// Keys to press in such a run: the drawn frame, the key, and for
    /// how many frames it is held (`--tap <frame>:<key>[:<frames>]` with
    /// a letter, a digit, `tab`, `delete`, `space`, `ctrl`, `alt`,
    /// `shift`, `lmb` and so on). With any given, the keys are live in
    /// the run: the inventory's, and, in a run that holds no keys of its
    /// own (`--hold`), the character's, heard through the bindings as a
    /// hand's are.
    taps: Vec<(u32, keys::Key, u32)>,
    /// The mouse's travel in such a run: from a drawn frame, for so many
    /// frames, so many counts a frame across and down (`--mouse
    /// <frame>:<frames>:<across>:<down>`). It goes through the bindings
    /// as a hand's does: the turn's, or with the free look's key held
    /// (`--tap <frame>:mmb:<frames>`) the head's.
    pub mouse: Vec<(u32, u32, Vec2)>,
    /// Notches of the wheel to turn in such a run: the drawn frame and
    /// how many, up counted as positive (`--wheel <frame>:<notches>`).
    pub wheel: Vec<(u32, i32)>,
    /// Notches of the wheel turned with the scope zoom's keys held in such
    /// a run (`--zoom <frame>:<notches>`, up counted as positive, which
    /// narrows): a variable scope's zoom (`optic_view`).
    pub zoom: Vec<(u32, i32)>,
    /// Damage done in such a run, to look at what health makes of it: on a
    /// drawn frame, to a part, so much, as a bullet's (`--damage
    /// <frame>:<part>:<amount>`, the part `head`, `chest`, `stomach`,
    /// `left-arm`, `right-arm`, `left-leg` or `right-leg`; and after them,
    /// if given, the stamina burnt a point as a round's `StaminaBurnPerDamage`,
    /// which slows the character for a while: `Movement::hit`). A capture's
    /// tool, as `--tap` is: no key does damage in play.
    pub damage: Vec<(u32, trp_sim::health::Part, f32, f32)>,
    /// Whether the range's ground and boxes carry grid lines (`--grid off`
    /// leaves them plain, as captures made before 2026-10-10 have them).
    grid: bool,
    /// Whether the window is left to answer a key pressed with Alt in the
    /// system's own way (`--alt-menu`), which shows the pointer for a
    /// moment at each such key: for setting beside what the app does.
    pub alt_menu: bool,
    /// Whether the app's panel at the top right (the frame rate, the debug
    /// figures and the keys) starts shown (`--fps on`); F1 switches it
    /// while running.
    pub show_fps: bool,
    /// When the battle screen's three panels show in this run, over the
    /// install's and the options' (`--hud always|autohide|never`).
    pub hud: Option<crate::battle_view::Shows>,
    /// The options' switch `CUSTOM FIRST PERSON HANDS` as the run begins,
    /// where the command line says (`--custom-hands on|off`): otherwise
    /// it is as the options' file has it, and on without one.
    pub custom_hands: Option<bool>,
    /// The switch `SMOOTH FIRST PERSON ANIMATIONS` as the run begins, the
    /// same way (`--smooth-animations on|off`).
    pub smooth_animations: Option<bool>,
    /// The switch `ICON GENERATION` as the run begins, the same way
    /// (`--icons on|off`).
    pub icon_generation: Option<bool>,
    /// The pictures' quality (0 to 2) and their streaming as the run
    /// begins, over the options' (`--texture-quality <n>`, `--mip-streaming
    /// on|off`).
    pub texture_quality: Option<u8>,
    pub mip_streaming: Option<bool>,
    /// The shadows' quality (0 to 3) as the run begins, over the options'
    /// (`--shadow-quality <n>`); what the run took, once it has begun.
    pub shadow_quality: Option<u8>,
    /// The ambient occlusion's choice as the run begins, over the options'
    /// (`--ssao <choice>`, as `Graphics.ini` names it); what the run took,
    /// once it has begun.
    pub ssao: Option<trp_sim::hbao::SsaoMode>,
    /// The occlusion shown alone, as the game's `HBAO` can show it
    /// (`--ssao-display ao`): for looking at it.
    pub ssao_ao_only: bool,
    /// The game's frame rate limit for this run (`--fps-limit <30 to
    /// 144|off>`), over the options' (`frame_limit`): a run nobody is at the
    /// keys of holds none unless told.
    pub fps_limit: Option<Option<u32>>,
    /// The samples of the scene's multisampling (`--msaa off|2|4`), over the
    /// options'; what the run took, once it has begun.
    pub msaa: Option<u8>,
    /// Any switch of the options' features as the run begins, by its name
    /// in the options' file (`--feature stamina=off`, as often as wanted).
    pub feature_switches: Vec<(String, bool)>,
    /// How the window presents its frames: `vsync` (the default), `off`,
    /// or `mailbox`.
    present: String,
    /// The graphics backend, `vulkan` or `dx12`, over the options' (`backend`).
    backend: Option<String>,
    /// Whether frames are rendered on a thread of their own while the next
    /// is worked out (Bevy's pipelined rendering, as the game's engine
    /// renders with its graphics jobs: `boot.config`), over the options'
    /// `RENDER THREAD` (`--render-thread on|off`). On for a run at the keys
    /// unless the options say otherwise; off for a capture run unless asked
    /// for, so that a capture's frame shows what it always has. Left off
    /// until 0.80.0: on Bevy 0.16 one update in eight ran a quarter of a
    /// frame early or late with it (the mouse read unevenly); on Bevy 0.18
    /// two in 1,110 do, on or off (`--log-pacing` at 165 a second,
    /// `docs/findings.md`, "What a frame costs, again"). Without it a frame
    /// shows input one frame sooner; with it the frame costs 38% less.
    render_thread: Option<bool>,
}

impl Options {
    /// Whether nobody is at the keys: a run that saves a frame and exits,
    /// or one that plays a script and logs how it was drawn.
    /// Whether the inventory's keys are to be listened to: someone is at
    /// them, or the run was given presses of its own.
    fn keys_live(&self) -> bool {
        !self.unattended() || !self.taps.is_empty() || !self.mouse.is_empty()
    }

    pub fn unattended(&self) -> bool {
        self.screenshot.is_some() || (self.scenario.is_some() && self.log_pacing.is_some())
    }

    /// Whose eyes the character is seen with in a view: the game has a
    /// character seen by anyone but itself in third person, with a pose
    /// and a weapon motion of its own (`docs/spec/third-person.md`).
    pub fn point_of_view(&self, view: View) -> trp_sim::PointOfView {
        self.pov.unwrap_or(match view {
            View::FirstPerson => trp_sim::PointOfView::FirstPerson,
            View::Free | View::Watched => trp_sim::PointOfView::ThirdPerson,
        })
    }
}


// ==== Parsing the command line ====

/// DirectX's newer shader compiler, DXC, as its two libraries: beside
/// `trp.exe`, or in the newest Windows 10 or 11 SDK that has both for this
/// machine's kind (`Windows Kits\10\bin\<version>\x64`). The engine looks
/// for them in the folder it is run from alone, and without them compiles
/// with the older FXC: measured on 2026-10-14, DXC built what a run draws
/// with in 0.43 to 0.61 s where FXC took 1.59 to 2.04, and the rifles
/// crate's pictures in 4.0 s where FXC took 7.2; a frame once all is built
/// takes the same (`docs/findings.md`).
fn dxc_libraries() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    if !cfg!(windows) {
        return None;
    }
    let both = |folder: &std::path::Path| {
        let (dxc, dxil) = (folder.join("dxcompiler.dll"), folder.join("dxil.dll"));
        (dxc.is_file() && dxil.is_file()).then_some((dxc, dxil))
    };
    let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().and_then(both));
    beside.or_else(|| {
        let kits = std::path::PathBuf::from(std::env::var_os("ProgramFiles(x86)")?).join("Windows Kits").join("10").join("bin");
        let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
        let mut versions: Vec<std::path::PathBuf> = std::fs::read_dir(&kits).ok()?.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
        // The newest first: the folders are named by version, `10.0.n.0`.
        versions.sort_by_key(|path| path.file_name().and_then(|name| name.to_str()).map(|name| name.split('.').map(|part| part.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>()));
        versions.iter().rev().find_map(|version| both(&version.join(arch)))
    })
}

/// The game's own defaults for the four settings of its menu the app
/// takes: the field of view, head bobbing, and the mouse's sensitivity
/// from the hip and with the sights up (the defaults written in the
/// game's settings classes).
const DEFAULT_VIEW: [f32; 4] = [50.0, 0.2, 0.7, 0.5];

/// `settings` is what the player has set in the install's own game: the
/// app starts with those, since they are what the game beside it shows,
/// and with the game's defaults where the install does not say.
fn parse_options(settings: &trp_data::PlayerSettings) -> Options {
    let mut options = Options {
        weapon: DEFAULT_WEAPON.to_owned(),
        weapons: Vec::new(),
        measure_crate: None,
        measure_rigs: Vec::new(),
        arms: DEFAULT_ARMS.to_owned(),
        body: DEFAULT_BODY.to_owned(),
        legs: DEFAULT_LEGS.to_owned(),
        head: DEFAULT_HEAD.to_owned(),
        suits: Vec::new(),
        wear: Vec::new(),
        carry: Vec::new(),
        lay: Vec::new(),
        added: Vec::new(),
        preset: None,
        weight: 0.0,
        view: View::FirstPerson,
        pov: None,
        play: Vec::new(),
        hold_at: None,
        hold: None,
        field_of_view: settings.field_of_view.unwrap_or(DEFAULT_VIEW[0]).clamp(50.0, 75.0),
        fov_given: false,
        head_bobbing: settings.head_bobbing.unwrap_or(DEFAULT_VIEW[1]).clamp(0.2, 1.0),
        sensitivity: settings.mouse_sensitivity.unwrap_or(DEFAULT_VIEW[2]).clamp(0.1, 5.0),
        aiming_sensitivity: settings.aiming_sensitivity.unwrap_or(DEFAULT_VIEW[3]).clamp(0.1, 5.0),
        optic_sensitivity: 1.0,
        sensitivity_given: [false; 2],
        degrees_a_count: 0.125,
        invert: [false; 2],
        look_degrees_a_count: 0.125,
        seed: 1,
        aim_speed_bonus: None,
        mastering: 0,
        weapon_skill: None,
        skill_levels: Vec::new(),
        skill_at: Vec::new(),
        frame_rate: trp_sim::REFERENCE_FRAME_RATE,
        strength: None,
        endurance: None,
        pitch: 0.0,
        yaw: 0.0,
        view_given: false,
        hour: None,
        cloudiness: None,
        malfunction: None,
        heat: None,
        repair: None,
        start: None,
        loadout: None,
        weight_given: false,
        inventory: false,
        loot: false,
        equip: false,
        bare: false,
        starts_armed: false,
        drags: Vec::new(),
        drag_at: 90,
        moves: Vec::new(),
        taps: Vec::new(),
        damage: Vec::new(),
        mouse: Vec::new(),
        wheel: Vec::new(),
        zoom: Vec::new(),
        grid: true,
        alt_menu: false,
        show_fps: false,
        hud: None,
        custom_hands: None,
        smooth_animations: None,
        icon_generation: None,
        texture_quality: None,
        mip_streaming: None,
        shadow_quality: None,
        ssao: None,
        ssao_ao_only: false,
        fps_limit: None,
        msaa: None,
        feature_switches: Vec::new(),
        whole: false,
        watch_turn: 0.0,
        map: None,
        screenshot: None,
        screenshot_crop: None,
        sound_out: None,
        log_sounds: None,
        screenshot_frame: 90,
        screenshot_count: 1,
        window: WINDOW_SIZE,
        log_frames: None,
        level: None,
        export_level: None,
        log_pacing: None,
        turn: 0.0,
        scenario: None,
        scenario_at: 2.0,
        present: String::from("vsync"),
        backend: None,
        render_thread: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--weapon" => options.weapon = args.next().unwrap_or_default(),
            // Another weapon for the crate, by its template or a part of
            // its name (`--add-weapon sv-98`): any the database has a
            // stock preset for. Its rig is built when it is first hung on
            // a slot, and it can be worked if its kind of logic is built.
            "--add-weapon" => options.added.extend(args.next()),
            "--preset" => options.preset = args.next(),
            "--measure-rigs" => options.measure_crate = args.next(),
            "--arms" => options.arms = args.next().unwrap_or_default(),
            "--body" => options.body = args.next().unwrap_or_default(),
            "--legs" => options.legs = args.next().unwrap_or_default(),
            "--head" => options.head = args.next().unwrap_or_default(),
            "--suit" => options.suits.extend(args.next()),
            "--wear" => options.wear.extend(args.next()),
            "--carry" => options.carry.extend(args.next()),
            "--lay" => options.lay.extend(args.next()),
            "--weight" => {
                options.weight = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                options.weight_given = true;
            }
            "--loadout" => options.loadout = args.next(),
            "--inventory" => options.inventory = true,
            "--loot" => (options.inventory, options.loot) = (true, true),
            "--equip" => options.equip = true,
            "--bare" => options.bare = true,
            "--wheel" => {
                let given = args.next().unwrap_or_default();
                match given.split_once(':').and_then(|(frame, notches)| Some((frame.parse().ok()?, notches.parse().ok()?))) {
                    Some(turn) => options.wheel.push(turn),
                    None => eprintln!("ignoring --wheel {given}: wanted <frame>:<notches>"),
                }
            }
            "--zoom" => {
                let given = args.next().unwrap_or_default();
                match given.split_once(':').and_then(|(frame, notches)| Some((frame.parse().ok()?, notches.parse().ok()?))) {
                    Some(turn) => options.zoom.push(turn),
                    None => eprintln!("ignoring --zoom {given}: wanted <frame>:<notches>"),
                }
            }
            "--damage" => {
                use trp_sim::health::Part;
                let given = args.next().unwrap_or_default();
                let mut parts = given.split(':');
                let frame = parts.next().and_then(|v| v.parse().ok());
                let part = parts.next().and_then(|name| {
                    Some(match name {
                        "head" => Part::Head,
                        "chest" | "thorax" => Part::Chest,
                        "stomach" => Part::Stomach,
                        "left-arm" => Part::LeftArm,
                        "right-arm" => Part::RightArm,
                        "left-leg" => Part::LeftLeg,
                        "right-leg" => Part::RightLeg,
                        _ => return None,
                    })
                });
                match (frame, part, parts.next().and_then(|v| v.parse().ok())) {
                    // And, as a round would, the stamina it burns a point
                    // (`StaminaBurnPerDamage`), nought unless given.
                    (Some(frame), Some(part), Some(amount)) => {
                        options.damage.push((frame, part, amount, parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0)))
                    }
                    _ => eprintln!("--damage takes a frame, a part and an amount: {given}"),
                }
            }
            "--tap" => {
                let given = args.next().unwrap_or_default();
                let mut parts = given.split(':');
                let frame = parts.next().and_then(|v| v.parse().ok());
                let key = parts.next().and_then(keys::tapped);
                match (frame, key) {
                    (Some(frame), Some(key)) => options.taps.push((frame, key, parts.next().and_then(|v| v.parse().ok()).unwrap_or(2))),
                    _ => eprintln!("--tap takes a frame and a key joined by a colon: a letter, a digit, tab, delete, space, ctrl, alt, shift, caps, enter, escape, lmb or rmb"),
                }
            }
            "--mouse" => {
                let given = args.next().unwrap_or_default();
                let numbers: Vec<f32> = given.split(':').filter_map(|v| v.parse().ok()).collect();
                match numbers[..] {
                    [frame, frames, across, down] => options.mouse.push((frame as u32, frames as u32, Vec2::new(across, down))),
                    _ => eprintln!("ignoring --mouse {given}: wanted <frame>:<frames>:<counts across a frame>:<counts down a frame>"),
                }
            }
            "--drag-at" => options.drag_at = args.next().and_then(|frame| frame.parse().ok()).unwrap_or(options.drag_at),
            "--move" => {
                let numbers: Vec<f32> = args.next().unwrap_or_default().split(':').filter_map(|v| v.parse().ok()).collect();
                match numbers[..] {
                    [frame, x, y] => options.moves.push((frame as u32, x, y)),
                    _ => eprintln!("--move takes three numbers joined by colons: the drawn frame, x, y"),
                }
            }
            "--drag" => {
                let numbers: Vec<f32> = args.next().unwrap_or_default().split(':').filter_map(|v| v.parse().ok()).collect();
                match numbers[..] {
                    [x, y, to_x, to_y] => options.drags.push([x, y, to_x, to_y]),
                    _ => eprintln!("--drag takes four numbers joined by colons: from x, from y, to x, to y"),
                }
            }
            "--view" => {
                options.view = match args.next().as_deref() {
                    Some("side") => View::Free,
                    // The whole figure from three metres off, for captures.
                    Some("body") => {
                        options.whole = true;
                        View::Free
                    }
                    // The whole figure likewise, with the keys the
                    // character's: a capture that presses keys of its own
                    // (`--tap`) is heard as a played run's.
                    Some(watch) if watch == "watch" || watch.starts_with("watch:") => {
                        options.whole = true;
                        options.watch_turn = watch.strip_prefix("watch:").and_then(|d| d.parse().ok()).unwrap_or(0.0);
                        View::Watched
                    }
                    // The range from overhead, ahead at the top; `map:<metres>`
                    // from that height.
                    Some(map) if map.starts_with("map") => {
                        options.map = Some(map.strip_prefix("map:").and_then(|h| h.parse().ok()).unwrap_or(30.0));
                        View::Free
                    }
                    _ => View::FirstPerson,
                }
            }
            // The pose and weapon motion of the character's own eyes or of
            // another's, whatever the view: `--pov first --view side` is
            // the first-person rig looked at from outside.
            "--pov" => {
                options.pov = match args.next().as_deref() {
                    Some("first") => Some(trp_sim::PointOfView::FirstPerson),
                    Some("third") => Some(trp_sim::PointOfView::ThirdPerson),
                    other => {
                        eprintln!("--pov takes first or third, not {other:?}");
                        None
                    }
                }
            }
            "--play" => options.play.extend(args.next()),
            "--at" => options.hold_at = args.next().and_then(|v| v.parse().ok()),
            "--hold" => options.hold = args.next(),
            "--fov" => {
                options.field_of_view = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[0]).clamp(50.0, 75.0);
                options.fov_given = true;
            }
            "--head-bobbing" => {
                options.head_bobbing = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[1]).clamp(0.2, 1.0)
            }
            "--sensitivity" => {
                options.sensitivity = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[2]).clamp(0.1, 5.0);
                options.sensitivity_given[0] = true;
            }
            "--aiming-sensitivity" => {
                options.aiming_sensitivity =
                    args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[3]).clamp(0.1, 5.0);
                options.sensitivity_given[1] = true;
            }
            "--strength" => options.strength = Some(args.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0).clamp(0.0, 51.0) as u32),
            "--endurance" => options.endurance = Some(args.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0).clamp(0.0, 51.0) as u32),
            "--skill-at" => {
                let given = args.next().unwrap_or_default();
                let parsed = given.split_once(':').and_then(|(frame, rest)| {
                    let (name, level) = rest.split_once('=')?;
                    Some((frame.parse::<u32>().ok()?, name.to_owned(), level.parse::<u32>().ok()?.min(51)))
                });
                match parsed {
                    Some(at) => options.skill_at.push(at),
                    None => eprintln!("--skill-at takes frame:Name=level: {given}"),
                }
            }
            "--skill" => {
                let given = args.next().unwrap_or_default();
                match given.split_once('=').and_then(|(name, level)| Some((name.to_owned(), level.parse::<u32>().ok()?.min(51)))) {
                    Some(pair) => options.skill_levels.push(pair),
                    None => eprintln!("--skill takes Name=level (the game's name, 0 to 51): {given}"),
                }
            }
            "--pitch" => {
                options.pitch = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32).clamp(-90.0, 90.0);
                options.view_given = true;
            }
            "--yaw" => {
                options.yaw = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32);
                options.view_given = true;
            }
            "--hour" => options.hour = args.next().and_then(|v| v.parse().ok()).map(|h: f32| h.rem_euclid(24.0)),
            "--malfunction" => {
                use trp_sim::malfunction::Malf;
                options.malfunction = match args.next().as_deref() {
                    Some("misfire") => Some(Malf::Misfire),
                    Some("jam") => Some(Malf::Jam),
                    Some("feed") => Some(Malf::Feed),
                    Some("soft") => Some(Malf::SoftSlide),
                    Some("hard") => Some(Malf::HardSlide),
                    other => {
                        eprintln!("--malfunction takes misfire, jam, feed, soft or hard, not {other:?}");
                        None
                    }
                }
            }
            "--heat" => options.heat = args.next().and_then(|v| v.parse().ok()).map(|h: f32| h.clamp(0.0, 200.0)),
            "--repair" => {
                options.repair = args.next().and_then(|v| v.parse().ok()).map(|d: f32| d.clamp(0.0, 100.0));
                options.inventory = true;
            }
            "--cloudiness" => options.cloudiness = args.next().and_then(|v| v.parse().ok()).map(|c: f32| c.clamp(-1.0, 1.0)),
            "--start" => options.start = args.next(),
            "--grid" => options.grid = args.next().as_deref() != Some("off"),
            "--alt-menu" => options.alt_menu = true,
            "--fps" => options.show_fps = args.next().as_deref() != Some("off"),
            "--hud" => {
                let given = args.next().unwrap_or_default();
                match given.as_str() {
                    "always" => options.hud = Some(battle_view::Shows::Always),
                    "autohide" => options.hud = Some(battle_view::Shows::Autohide),
                    "never" => options.hud = Some(battle_view::Shows::Never),
                    _ => eprintln!("--hud takes always, autohide or never: {given}"),
                }
            }
            "--custom-hands" => options.custom_hands = Some(args.next().as_deref() != Some("off")),
            "--smooth-animations" => options.smooth_animations = Some(args.next().as_deref() != Some("off")),
            "--icons" => options.icon_generation = Some(args.next().as_deref() != Some("off")),
            "--texture-quality" => options.texture_quality = args.next().and_then(|quality| quality.parse::<u8>().ok()).map(|quality| quality.min(2)),
            "--mip-streaming" => options.mip_streaming = Some(args.next().as_deref() != Some("off")),
            "--shadow-quality" => options.shadow_quality = args.next().and_then(|quality| quality.parse::<u8>().ok()).map(|quality| quality.min(3)),
            "--ssao" => {
                let given = args.next().unwrap_or_default();
                options.ssao = trp_sim::hbao::SsaoMode::from_key(&given);
                if options.ssao.is_none() {
                    eprintln!("--ssao takes one of {}: {given}", trp_sim::hbao::SsaoMode::ALL.map(trp_sim::hbao::SsaoMode::key).join(", "));
                }
            }
            "--ssao-display" => options.ssao_ao_only = args.next().as_deref() == Some("ao"),
            "--fps-limit" => {
                let given = args.next().unwrap_or_default();
                options.fps_limit = match given.as_str() {
                    "off" => Some(None),
                    number => match number.parse::<u32>() {
                        Ok(fps) => Some(Some(fps)),
                        Err(_) => {
                            eprintln!("--fps-limit takes frames a second (30 to 144) or off: {given}");
                            None
                        }
                    },
                };
            }
            "--msaa" => {
                let given = args.next().unwrap_or_default();
                options.msaa = match given.as_str() {
                    "off" | "1" => Some(1),
                    "2" | "2x" => Some(2),
                    "4" | "4x" => Some(4),
                    _ => {
                        eprintln!("--msaa takes off, 2 or 4: {given}");
                        None
                    }
                };
            }
            "--feature" => {
                let given = args.next().unwrap_or_default();
                match given.split_once('=') {
                    Some((name, on)) => options.feature_switches.push((name.to_owned(), on != "off")),
                    None => eprintln!("--feature takes a switch's name and on or off: {given}"),
                }
            }
            // Taken before the rest (`setup::wanted`).
            "--setup" => {}
            "--seed" => options.seed = args.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "--aim-speed-bonus" => {
                options.aim_speed_bonus = Some(args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32).clamp(0.0, 5.0))
            }
            "--mastering" => options.mastering = args.next().and_then(|v| v.parse().ok()).unwrap_or(0u32).min(3),
            "--weapon-skill" => options.weapon_skill = Some(args.next().and_then(|v| v.parse().ok()).unwrap_or(0u32).min(51)),
            "--frame-rate" => {
                options.frame_rate = args.next().and_then(|v| v.parse().ok()).unwrap_or(trp_sim::REFERENCE_FRAME_RATE).clamp(30.0, 480.0)
            }
            "--log-frames" => options.log_frames = args.next().map(PathBuf::from),
            "--level" => options.level = args.next().map(PathBuf::from),
            "--export-level" => options.export_level = args.next().map(PathBuf::from),
            "--log-pacing" => options.log_pacing = args.next().map(PathBuf::from),
            "--turn" => options.turn = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            "--scenario" => {
                let file = PathBuf::from(args.next().unwrap_or_default());
                match trp_sim::script::Scenario::load(&file) {
                    Ok(scenario) => options.scenario = Some(scenario),
                    Err(e) => eprintln!("ignoring the scenario: {e:#}"),
                }
            }
            "--scenario-at" => options.scenario_at = args.next().and_then(|v| v.parse().ok()).unwrap_or(2.0f32).max(0.0),
            "--present" => options.present = args.next().unwrap_or_default(),
            "--backend" => options.backend = args.next(),
            "--render-thread" => options.render_thread = Some(args.next().as_deref() != Some("off")),
            "--screenshot" => options.screenshot = args.next().map(PathBuf::from),
            "--screenshot-crop" => {
                options.screenshot_crop = args.next().and_then(|v| {
                    let n: Vec<u32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                    (n.len() == 4).then(|| [n[0], n[1], n[2], n[3]])
                })
            }
            "--sound-out" => options.sound_out = args.next().map(PathBuf::from),
            "--log-sounds" => options.log_sounds = args.next().map(PathBuf::from),
            "--screenshot-frames" => options.screenshot_count = args.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "--screenshot-frame" => {
                options.screenshot_frame = args.next().and_then(|v| v.parse().ok()).unwrap_or(90)
            }
            "--resolution" => {
                let given = args.next().unwrap_or_default();
                match given.split_once('x').and_then(|(width, height)| Some([width.parse::<u32>().ok()?, height.parse::<u32>().ok()?])) {
                    Some([width, height]) => options.window = [width.max(640), height.max(360)],
                    None => eprintln!("--resolution takes a width and a height joined by an x: {given}"),
                }
            }
            other => eprintln!("ignoring unknown argument {other}"),
        }
    }
    options
}

// ==== main: the app assembled ====

fn main() -> anyhow::Result<()> {
    // The version before anything else, so that whatever a run prints says
    // which build it came from (`docs\manual.md`, "Versions"); `--version`
    // prints it alone.
    println!("TRP {}", env!("TRP_VERSION"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        return Ok(());
    }
    // Where the games are: asked in the console on a first start, or with
    // `--setup` (`setup`); `--setup-only` asks and ends (`Install.bat`).
    if setup::wanted(&args) {
        setup::run(&mut std::io::stdin().lock())?;
        if args.iter().any(|arg| arg == "--setup-only") {
            return Ok(());
        }
    }
    let install = Install::locate()?;
    let settings = install.player_settings();
    let mut options = parse_options(&settings);
    options.degrees_a_count = keys::degrees_a_count(&install);
    options.look_degrees_a_count = keys::look_degrees_a_count(&install);
    options.invert = ["InvertedXAxis", "InvertedYAxis"].map(|name| install.control_flag(name).unwrap_or(false));
    options.optic_sensitivity = install.control_setting("OpticSensitivity").unwrap_or(1.0).clamp(1.0, 5.0);
    println!(
        "view settings: field of view {}, head bobbing {}, mouse sensitivity {} and {} with the sights up, a count of the mouse {} degrees before either{}",
        options.field_of_view,
        options.head_bobbing,
        options.sensitivity,
        options.aiming_sensitivity,
        options.degrees_a_count,
        if settings == trp_data::PlayerSettings::default() { " (the install has none of its own)" } else { "" }
    );
    let db = Database::load(&install)?;
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
    let mut equipment = trp_sim::inventory::Equipment::empty(&db)?;
    // The character's own pockets and dogtag, which a character of the game
    // always has and keeps in a raid (`CantRemoveFromSlotsDuringRaid`): a
    // BEAR's of the Standard edition, as the crate's vest and backpack are,
    // empty (the app's choice of edition and side: `PROVISIONAL`, the
    // user's profile not read).
    match trp_data::inventory::Loadout::load(&install, "Standard", "bear").and_then(|list| trp_sim::inventory::Equipment::of_loadout(&db, &list)) {
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
        match trp_sim::inventory::Kind::of(&db, BEAR_DOGTAG) {
            Ok(kind) => equipment.item.slots[slot] = Some(trp_sim::inventory::Item::new(std::sync::Arc::new(kind))),
            Err(e) => eprintln!("the character's dogtag could not be made: {e:#}"),
        }
    }
    if let Some(loadout) = &options.loadout {
        let (edition, side) = loadout.split_once('/').unwrap_or((loadout.as_str(), "usec"));
        match trp_data::inventory::Loadout::load(&install, edition, side).and_then(|list| trp_sim::inventory::Equipment::of_loadout(&db, &list)) {
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
        let preset = trp_data::inventory::Loadout::of_preset(&db, template).and_then(|list| trp_sim::inventory::Item::of_list(&db, &list)).map(|(item, _)| item);
        let made = preset.or_else(|_| trp_sim::inventory::Kind::of(&db, template).map(|kind| trp_sim::inventory::Item::new(std::sync::Arc::new(kind))));
        match made {
            Ok(item) => {
                if let Some(left) = trp_sim::inventory::put_unloaded(&mut equipment.item, item) {
                    let rules = trp_sim::inventory::Rules::of(&db);
                    if trp_sim::inventory::pick_up(&mut equipment.item, left, &rules).0.is_err() {
                        eprintln!("--carry {template}: no room on the character");
                    }
                }
            }
            Err(e) => eprintln!("--carry {template}: {e:#}"),
        }
    }
    // What the window that shows a thing says of it.
    let texts = trp_data::inventory::item_texts(&install, "en").unwrap_or_else(|e| {
        eprintln!("the install's item texts could not be read; a thing's window says less: {e:#}");
        Default::default()
    });
    let mut names = trp_data::inventory::short_names(&install, "en").unwrap_or_else(|e| {
        eprintln!("the install's item names could not be read: {e:#}");
        Default::default()
    });
    // A weapon of one's own has no name in the install's texts: its
    // file's.
    for definition in db.custom.values() {
        names.extend(definition.names().map(|(id, name)| (id.to_owned(), name.to_owned())));
    }
    // Nor has an outfit of one's own.
    for definition in db.outfits.values() {
        names.insert(definition.id.clone(), definition.shown_name().to_owned());
    }
    // The crate on the range (inventory spec, "The crate"): the install's
    // airdrop crate, holding the app's weapon as its stock preset
    // has it and the vest and the backpack a BEAR of the Standard edition
    // starts with, each with what the database has in it, but for the
    // magazines and the rounds: those are the weapon's own (the user,
    // 2026-10-11; `Loadout::fit_to`). The two are made for the body at the
    // start and drawn once they are put on.
    // Where the game's own screen has the equipment's slot views.
    let panel = trp_data::screen::GearPanel::load(&install).unwrap_or_else(|e| {
        eprintln!("the install's equipment panel could not be read, the app's own arrangement is used: {e:#}");
        Default::default()
    });
    let rules = trp_sim::inventory::Rules::of(&db);
    let mut crate_kind = trp_sim::inventory::Kind::of(&db, CRATE)?;
    if let Some(grid) = crate_kind.grids.first_mut() {
        grid.cells = CRATE_CELLS;
    }
    // (The airdrop crate, empty since 0.96.0: the screen's first container
    // to bring, which nothing on the range opens.)
    let loot = trp_sim::inventory::Item::new(std::sync::Arc::new(crate_kind));
    // The tables of weapons along the knee-high wall: their gun cases, and
    // their magazine boxes; the storage cabinets' drawers.
    let (mut cases, boxes, rounds_boxes) = weapon_crates(&db)?;
    let mut cabinets = gear_cabinets(&db)?;
    // What the airdrop crate held until the workbench took its place (the
    // user, 2026-10-20: "Move its contents to the corresponding crates"),
    // each where its kind goes: a weapon in its class's gun case, the kit's
    // vest and backpack and the outfits of one's own in the cabinets
    // (`cabinet_of`), the door's key on the bench.
    let mut key_lies = None;
    // The run's own weapon is the one in its class's gun case (the user,
    // 2026-10-14, night: "move the AKM into the rifle case"), and a run that
    // begins with it on takes it from there: its class, and its number.
    // Another build of it asked for (`--preset`): made below with what loads
    // it, in place of the case's stock one.
    let build = options.preset.as_deref().and_then(|wanted| match preset_build(&db, wanted, &options.weapon) {
        Ok(build) => {
            println!("the run's weapon as the build {} ({} parts)", build.name, build.items.len());
            Some(build)
        }
        Err(e) => {
            eprintln!("--preset {wanted}: {e:#}");
            None
        }
    });
    let own = crate_of(&db, &options.weapon).filter(|_| build.is_none()).and_then(|class| {
        let found = cases.get(class)?.grids.iter().flatten().find(|(_, item)| item.kind.template == options.weapon);
        found.map(|(_, item)| (class, item.id))
    });
    // What a run nobody is at the keys of, or one begun with `--equip`,
    // has on from the start: the weapon it is run with, the vest and the
    // backpack.
    let mut put_on = Vec::new();
    put_on.extend(own.map(|(_, id)| id));
    {
        use trp_data::inventory::Loadout;
        use trp_sim::inventory::{Equipment, Item};
        let mut things = Vec::new();
        // Any other weapon asked for (`--add-weapon`), and the run's own
        // where no gun case has it, each as its stock preset has it and
        // with what loads it, in the crate; the run's own has the kit's
        // magazines in the vest.
        let weapons = std::iter::once(options.weapon.as_str()).filter(|_| own.is_none()).chain(options.weapons.iter().map(String::as_str).filter(|weapon| *weapon != options.weapon));
        for weapon in weapons {
            match armed(&db, weapon, weapon != options.weapon, build.as_ref().filter(|_| weapon == options.weapon)) {
                Ok(made) => {
                    if weapon == options.weapon {
                        put_on.push(made[0].id);
                    }
                    // In its class's gun case (the rifles' where it has
                    // none).
                    let class = crate_of(&db, weapon).unwrap_or(0);
                    for thing in made {
                        let name = thing.kind.name.clone();
                        match stow_more(&mut cases[class], thing) {
                            Ok(()) => println!("in the {} weapon case: {name}", range::WEAPON_CRATES[class]),
                            Err(_) => eprintln!("the {} weapon case has no room for {name}", range::WEAPON_CRATES[class]),
                        }
                    }
                }
                Err(e) => eprintln!("no {weapon} could be made: {e:#}"),
            }
        }
        let kit = Loadout::load(&install, "Standard", "bear").and_then(|mut list| {
            let [magazines, rounds] = list.fit_to(&db, &options.weapon)?;
            println!("the crate's kit: {magazines} magazines and {rounds} lots of rounds changed for the weapon's own");
            Equipment::of_loadout(&db, &list)
        });
        match kit {
            Ok((mut outfit, _)) => {
                for worn in ["TacticalVest", "Backpack"].into_iter().filter_map(|slot| outfit.take_off(slot)) {
                    options.wear.push(worn.kind.template.clone());
                    put_on.push(worn.id);
                    things.push(worn);
                }
            }
            Err(e) => eprintln!("the crate has no vest and no backpack: {e:#}"),
        }
        // The outfits that are not the install's (`docs/spec/outfits.md`):
        // each goes on where the armour it is made from does.
        let mut outfits: Vec<&String> = db.outfits.keys().collect();
        outfits.sort();
        for id in outfits {
            match trp_sim::inventory::Kind::of(&db, id) {
                Ok(kind) => things.push(Item::new(std::sync::Arc::new(kind))),
                Err(e) => eprintln!("the crate has no {id}: {e:#}"),
            }
        }
        // The key of the range's locked door, on the bench (on the table
        // behind the door wall without it).
        match trp_sim::inventory::Kind::of(&db, DOOR_KEY) {
            Ok(kind) => key_lies = Some(Item::new(std::sync::Arc::new(kind))),
            Err(e) => eprintln!("there is no key: {e:#}"),
        }
        for thing in things {
            let (name, [width, height], weight) = (thing.kind.name.clone(), thing.cells(), thing.weight());
            let at = db.items.get(&thing.kind.template).and_then(|item| cabinet_of(&db, item));
            match at.map(|at| (at, stow_more(&mut cabinets[at], thing))) {
                Some((at, Ok(()))) => println!("in the {} storage: {name}, {width} x {height} cells, {weight:.2} kg", range::CABINETS[at]),
                Some((at, Err(_))) => eprintln!("the {} storage has no room for {name} ({width} x {height} cells)", range::CABINETS[at]),
                None => eprintln!("no storage takes {name}"),
            }
        }
    }
    // The range's other containers, and the things lying about.
    let mut away: Vec<Option<trp_sim::inventory::Item>> = std::iter::once(Ok(None))
        .chain(CONTAINERS.iter().map(|template| {
            trp_sim::inventory::Kind::of(&db, template).map(|kind| Some(trp_sim::inventory::Item::new(std::sync::Arc::new(kind))))
        }))
        .collect::<anyhow::Result<_>>()?;
    // The weapons whose rigs are to be measured (`--measure-rigs`).
    if let Some(wanted) = options.measure_crate.as_deref() {
        for (holds, label) in cases.iter().zip(range::WEAPON_CRATES) {
            if wanted.eq_ignore_ascii_case("all") || wanted.eq_ignore_ascii_case(label) {
                let weapons = holds.grids.iter().flatten().map(|(_, thing)| thing).filter(|thing| trp_sim::inventory::is(thing, "Weapon"));
                options.measure_rigs.extend(weapons.map(|weapon| weapon.kind.template.clone()));
            }
        }
    }
    // The tables' gun cases, then their magazine boxes, after those; then
    // the storage cabinets' drawers.
    away.extend(cases.into_iter().chain(boxes).map(Some));
    away.extend(cabinets.into_iter().map(Some));
    // The tables of weapons' boxes of rounds, after the cabinets (the
    // places before them kept: `range::AMMO_BOX_HOLDS`).
    away.extend(rounds_boxes.into_iter().map(Some));
    // The workbench's boxes of repair kits, after those
    // (`range::REPAIR_BOX_HOLDS`): the armour's kits, then the weapons'.
    away.extend(repair_boxes(&db).into_iter().map(Some));
    let mut ground = Vec::new();
    if let Some(key) = key_lies {
        ground.push((key, Vec3::from(range::key_lies())));
    }
    for ((template, count), at) in LYING.into_iter().zip(range::lying()) {
        match trp_sim::inventory::Kind::of(&db, template) {
            Ok(kind) => {
                let mut item = trp_sim::inventory::Item::new(std::sync::Arc::new(kind));
                item.count = count;
                ground.push((item, Vec3::from(at)));
            }
            Err(e) => eprintln!("nothing lies at {at:?}: {e:#}"),
        }
    }
    // What `--lay` asks for: placed once the start is known (below).
    let laid_from = ground.len();
    for template in &options.lay {
        match trp_sim::inventory::Kind::of(&db, template) {
            Ok(kind) => ground.push((trp_sim::inventory::Item::new(std::sync::Arc::new(kind)), Vec3::ZERO)),
            Err(e) => eprintln!("--lay {template}: {e:#}"),
        }
    }
    // A run nobody is at the keys of has it all on from the start, put on
    // by the inventory's own moves; so does `--equip`.
    if (options.equip || options.unattended()) && !options.bare {
        use trp_sim::inventory::{move_item, Spot};
        let worn = equipment.item.id;
        // (The run's own weapon is in its gun case; the vest and the
        // backpack in the cabinets.)
        for id in put_on {
            let mut roots: Vec<&mut trp_sim::inventory::Item> = vec![&mut equipment.item];
            roots.extend(away.iter_mut().flatten());
            let slots = 0..roots[0].kind.slots.len();
            if !slots.into_iter().any(|slot| move_item(&mut roots, id, worn, Spot::Slot(slot), &rules).is_ok()) {
                eprintln!("something put on at the start goes into no slot");
            }
        }
    }
    options.starts_armed = trp_sim::inventory::HAND_SLOTS.into_iter().any(|slot| equipment.worn(slot).is_some());
    // Such a run is what the app's own figures are measured with, so it
    // weighs what it did before there was an inventory (what `--weight`
    // says; nothing, if nothing was said) unless the inventory is what it
    // is run for (`--equip`, `--inventory`, `--loot`).
    if options.unattended() && !(options.equip || options.inventory) {
        options.weight_given = true;
    }
    if !options.weight_given {
        options.weight = equipment.carried_weight(false);
    }
    println!("carried: {:.2} kg", options.weight);
    let mut unity = UnityAssets::new(install)?;
    let hard = HardSettings::load(&unity.install)?;
    let engine = EngineSettings::load(&unity.install)?;
    let mut movement_settings = trp_sim::load_movement_settings(&mut unity, &db, &hard, &engine, options.weight)?;
    // The body's animator decides the movement states and supplies the
    // motion. The weapon's width, which it also takes, is set once the
    // weapon is assembled.
    // The keys as the install binds them, and what the options tab has
    // kept over them and for the display (for someone at the keys only);
    // the features' switches as the run begins, which hold until it ends
    // (each is "restart required": `options_view::Features`).
    let mut bindings = keys::Bindings::of(&unity.install);
    let window = options.window;
    let options_state = options_view::OptionsState::new(&mut options, &unity.install, &mut bindings, window);
    // The character's skills: the tab's levels (the run's flags over them),
    // in force while `SKILLS` is on (`skills`). With Strength at its elite
    // level the weapons weigh nothing (inventory spec, "Weight").
    let skills = skills::Skills::new(options_state.skills.clone(), options_state.in_force.skills, options.aim_speed_bonus);
    let buffs = skills.buffs();
    if buffs.strength_elite && !options.weight_given {
        options.weight = equipment.carried_weight(true);
        movement_settings = movement_settings.reweighed(&hard, &db.config, options.weight)?;
        println!("carried at elite Strength: {:.2} kg", options.weight);
    }
    if !skills.levels.untrained() {
        let set: Vec<String> = trp_sim::skills::Skill::ALL.iter().filter(|s| skills.levels.level(**s) > 0).map(|s| format!("{} {}", s.name(), skills.levels.level(*s))).collect();
        println!("skills{}: {}", if skills.on() { "" } else { " (SKILLS off: no buff)" }, set.join(", "));
    }
    // A magazine's rounds in its model and its picture (`MAGAZINE ROUNDS`).
    let rounds_drawn = options_state.in_force.magazine_rounds;
    icons::draw_rounds(rounds_drawn);
    // A weapon made from its own parts (`ADVANCED ATTACHMENT SYSTEM`).
    trp_sim::inventory::set_builds(options_state.in_force.attachments);
    // A shot's flash (`MUZZLE FLASH`), where the hard settings have shot
    // effects at all (`SHOT_EFFECTS_ENABLED`).
    let muzzle_drawn = options_state.in_force.muzzle_flash && hard.float("SHOT_EFFECTS_ENABLED").is_ok_and(|on| on != 0.0);
    // Its fumes (`PARTICLES`), from the same shot.
    let particles_drawn = options_state.in_force.particles && hard.float("SHOT_EFFECTS_ENABLED").is_ok_and(|on| on != 0.0);
    // The pictures' quality and streaming, as the options have them or the
    // command line says.
    let textures = streaming::TextureSettings {
        quality: options.texture_quality.unwrap_or(options_state.graphics.texture_quality),
        streaming: options.mip_streaming.unwrap_or(options_state.graphics.mip_streaming),
    };
    println!(
        "pictures: texture quality {} of 2, mip streaming {}",
        textures.quality,
        if textures.streaming { format!("on (within {} MB, at most {} levels down)", streaming::BUDGET >> 20, textures.most_reduction()) } else { format!("off ({} of every picture's levels left off)", textures.limit()) }
    );
    // Before any picture is made.
    streaming::set(textures);
    // The sun's shadows, as the options have them or the command line says.
    let shadow_quality = options.shadow_quality.unwrap_or(options_state.graphics.shadow_quality).min(3);
    options.shadow_quality = Some(shadow_quality);
    println!("shadows: quality {}", shadows::describe(shadow_quality));
    // The ambient occlusion, as the options have it or the command line
    // says, with `AMBIENT OCCLUSION` (`hbao_view`).
    // The scene's multisampling and the frame rate limit, as the options
    // have them or the command line says (`frame_limit`).
    let msaa = options.msaa.unwrap_or(options_state.graphics.msaa);
    options.msaa = Some(msaa);
    let limiter = frame_limit::FrameLimit::new(options.fps_limit, !options.unattended());
    println!(
        "msaa: {}; frame rate limit: {}",
        if msaa > 1 { format!("{msaa}x") } else { "off".to_owned() },
        match (options.fps_limit, options.unattended(), options_state.graphics.fps_limit_off) {
            (Some(Some(fps)), ..) => format!("{fps} a second with vsync off (--fps-limit)"),
            (Some(None), ..) => "off (--fps-limit)".to_owned(),
            (None, true, _) => "none (a run nobody is at the keys of)".to_owned(),
            (None, false, true) => "off (the options)".to_owned(),
            (None, false, false) => format!("{} a second with vsync off (the options)", options_state.graphics.game_fps),
        }
    );
    let ssao = options.ssao.unwrap_or(options_state.graphics.ssao);
    options.ssao = Some(ssao);
    match (options_state.in_force.ambient_occlusion, ssao.hbao()) {
        (false, _) => println!("ambient occlusion: off (AMBIENT OCCLUSION)"),
        (true, None) => println!("ambient occlusion: off (SSAO {})", ssao.key()),
        (true, Some(s)) => println!(
            "ambient occlusion: SSAO {}, HBAO {:?} ({} directions of {} steps), radius {} m, at most {} pixels, blur {:?}{}{}",
            ssao.key(),
            s.quality,
            s.quality.directions(),
            s.quality.steps(),
            s.radius,
            s.max_radius_pixels,
            s.blur,
            if s.blur_downsample { " at half size" } else { "" },
            if ssao.cinematic() { "; the game's second effect at this choice is not built" } else { "" }
        ),
    }
    // The clothing the body is drawn in names the bones the animator has
    // to pose for it.

    // Every thing the run has, anywhere: of the parts a model on the head
    // may have fitted, those are read; and whatever of it a slot the game
    // draws takes is read for the body with what is worn, since it may be
    // put on (a helmet carried).
    let present: std::collections::HashSet<String> = {
        let mut found = std::collections::HashSet::new();
        let mut stack: Vec<&trp_sim::inventory::Item> = [&equipment.item, &loot].into_iter().chain(away.iter().flatten()).chain(ground.iter().map(|(item, _)| item)).collect();
        let drawn: Vec<usize> = trp_sim::inventory::WORN.iter().filter_map(|slot| equipment.item.slot_index(slot)).collect();
        while let Some(item) = stack.pop() {
            found.insert(item.kind.template.clone());
            if drawn.iter().any(|&slot| item.kind.among(&equipment.item.kind.slots[slot].takes)) && !options.wear.contains(&item.kind.template) {
                options.wear.push(item.kind.template.clone());
            }
            stack.extend(item.contents());
        }
        found
    };
    // The suits of the install (`CLOTHING`), and those to begin in: as
    // `--suit` says, else as the options' file keeps them (for someone at
    // the keys), each where the run was not told its bundles.
    let wardrobe = if options_state.in_force.clothing {
        let suits = trp_data::suits::suits(&unity.install, "en").unwrap_or_else(|e| {
            eprintln!("the install's suits could not be read; none is offered: {e:#}");
            Vec::new()
        });
        let file = (!options.unattended()).then(options_view::options_file);
        let kept: Vec<String> = file.as_deref().map(clothing::kept).into_iter().flatten().flatten().collect();
        for id in kept.iter().filter(|_| options.suits.is_empty()).chain(&options.suits) {
            let Some(suit) = suits.iter().find(|suit| &suit.id == id) else {
                eprintln!("no suit {id} in the install's customization list");
                continue;
            };
            match (suit.upper, &suit.body, &suit.hands, &suit.feet) {
                (true, Some(body), hands, _) if options.body == DEFAULT_BODY => {
                    options.body = body.clone();
                    if let Some(hands) = hands.as_ref().filter(|_| options.arms == DEFAULT_ARMS) {
                        options.arms = hands.clone();
                    }
                }
                (false, _, _, Some(feet)) if options.legs == DEFAULT_LEGS => options.legs = feet.clone(),
                _ => {}
            }
            println!("begun in the suit {}", suit.name);
        }
        Some(clothing::Clothing::new(unity.install.clone(), suits, &options.body, &options.legs, file))
    } else {
        None
    };
    let mut clothing = body_view::BodyParts::load(&mut unity, &db, &[&options.body, &options.legs, &options.head], &options.wear, &present, options_state.in_force.custom_hands, options_state.in_force.clothing);
    // And, with the options' `CHARACTER MODEL` on, the pose the game's
    // screens stand the character in, for the model of it on the gear
    // page and on the OVERALL tab; with `TARKOV UI` on the gear page has
    // the game's flat outline of the character in its place (the user,
    // 2026-10-15, at night: "with the feature on, drop the 3d model. 1;1
    // parity with tarkov"), and the OVERALL tab alone, as the game's, has
    // the model.
    let model = options_state.in_force.character_model;
    // The dummy stands so too (`dummy`), with its switch on and the
    // ballistics' (nothing would hit it without).
    let with_dummy = options_state.in_force.dummy && options_state.in_force.ballistics;
    let with_poppers = options_state.in_force.reactive_targets;
    // The scopes' second picture (`optic_view`), with `PIP`.
    let pip = options_state.in_force.pip;
    let standing: &[String] = if model || with_dummy { &clothing.standing } else { &[] };
    let body_data = match trp_sim::body::BodyData::load_standing(&mut unity, &clothing.bones, standing) {
        Ok((data, standing)) => {
            match standing {
                Some(Ok(pose)) => {
                    if with_dummy {
                        clothing.dummy_pose = Some(pose.clone());
                    }
                    if model {
                        clothing.standing_pose = Some(pose);
                    }
                }
                Some(Err(e)) => eprintln!("the character model is not drawn: {e:#}"),
                None => {}
            }
            data
        }
        Err(e) => {
            eprintln!("the body is not drawn: {e:#}");
            clothing = body_view::BodyParts::default();
            trp_sim::body::BodyData::load(&mut unity)?
        }
    };
    let body = trp_sim::Body::new(&body_data, 1.0);
    let mut movement = trp_sim::Movement::new(movement_settings, body);
    // What the skills make of the movement, the weight's limits, stamina
    // and the steps.
    skills::put_on_movement(&mut movement, &buffs, skills.carry(&buffs), (&hard, &db.config));
    movement.stamina.resettle();
    movement.pitch = options.pitch;
    (movement.yaw, movement.body_yaw) = (options.yaw, options.yaw);
    movement.note_rotation();
    let hands_data = match HandsData::load(&mut unity) {
        Ok(data) => Some(data),
        Err(e) => {
            eprintln!("weapon motion data could not be read: {e:#}");
            None
        }
    };

    // A level file (`level`), `--level`'s, to try an edit of the range in
    // Blender for the run.
    if let Some(path) = options.level.clone() {
        match level::Level::read(&path) {
            Ok(read) => {
                println!(
                    "the range from {}: {} boxes, {} props placed, {} starts, {} lamps, {}",
                    path.display(),
                    read.boxes.len(),
                    read.props.len(),
                    read.starts.len(),
                    read.lamps.len(),
                    if read.indoors.is_some() { "an indoors" } else { "no indoors" }
                );
                level::set(read);
            }
            Err(e) => eprintln!("the level file {} could not be read, the range's own instead: {e:#}", path.display()),
        }
    }
    // The test range stops the character as far off as the game's capsule
    // would. The workbench stands on it with the options' `WORKBENCH`.
    range::set_workbench(options_state.in_force.workbench);
    let mut range = range::Range::default();
    // `--export-level`: the range as it stands, written, and the run ends.
    if let Some(path) = &options.export_level {
        level::write(&range.layout(), path)?;
        println!("the range written to {}", path.display());
        return Ok(());
    }
    match trp_data::player::CapsuleData::load(&mut unity) {
        Ok(capsule) => {
            // The player's controller is the game's own, which keeps no skin
            // outside the capsule (movement spec, "The character's own
            // collision"): recorded, a character stops the radius from a wall.
            range.radius = capsule.radius;
        }
        Err(e) => eprintln!("the character's capsule could not be read: {e:#}"),
    }
    // The doors, containers and trunk turn by the install's curve and wait
    // its delay.
    match trp_sim::things::ThingSettings::new(&hard) {
        Ok(settings) => range.thing_settings = Some(settings),
        Err(e) => eprintln!("the doors and containers will not move: {e:#}"),
    }
    // The right-hand door is locked, its key in the crate: to be unlocked
    // with it, or kicked (which opens a door only from the side it swings
    // away from: here, from the yard).
    range.lock_door(1, DOOR_KEY);
    // A capture can start somewhere else in the range than its middle.
    if let Some(name) = &options.start {
        match range.place(name) {
            Some(at) => movement.position = at,
            None => eprintln!("ignoring unknown place {name}: platform, shelf, stairs, doors, yard, car, wooden-crate, supply-crate, scav, drawer, table, high-shelf or floor"),
        }
        if let (false, Some([yaw, pitch])) = (options.view_given, range.start_view(name)) {
            movement.pitch = pitch;
            (movement.yaw, movement.body_yaw) = (yaw, yaw);
            movement.note_rotation();
        }
    }
    // What `--lay` asked for, on the floor 0.8 m ahead of the start, each
    // 0.4 m right of the one before.
    for (index, (_, at)) in ground[laid_from..].iter_mut().enumerate() {
        *at = inventory_view::beside(movement.position, movement.yaw, [0.8, 0.4 * index as f32]);
    }

    // The camera is kept out of the world by a box of its own.
    let camera_box = match trp_data::player::CameraBoxData::load(&mut unity) {
        Ok(shape) => Some(shape),
        Err(e) => {
            eprintln!("the camera's collider could not be read: {e:#}");
            None
        }
    };


    let mut player = ClipPlayer::new("");
    player.queue = options.play.iter().cloned().collect();
    player.hold_at = options.hold_at;
    player.by_name = !options.play.is_empty() || options.hold_at.is_some();
    let present_mode = match options.present.as_str() {
        "off" => bevy::window::PresentMode::AutoNoVsync,
        "mailbox" => bevy::window::PresentMode::Mailbox,
        _ => bevy::window::PresentMode::AutoVsync,
    };
    // A window without the focus is updated only some 60 times a second, on
    // a timer. An unattended run seldom has the focus and is there to show
    // what a played one does, so it updates as a focused window would.
    let mut updates = bevy::winit::WinitSettings::game();
    if options.unattended() {
        updates.unfocused_mode = bevy::winit::UpdateMode::Continuous;
    }
    let mut renderer = bevy::render::settings::WgpuSettings::default();
    // The backend (`backend`): the engine's own variable `WGPU_BACKEND` has
    // the last word, then `--backend`, then the options; the other where
    // the one chosen finds no graphics card.
    let mut uses_dx12 = true;
    if std::env::var_os("WGPU_BACKEND").is_none() {
        let wanted = match options.backend.as_deref() {
            Some(key) => backend::Renderer::from_key(key).unwrap_or_else(|| {
                eprintln!("ignoring unknown backend {key}");
                options_state.graphics.renderer
            }),
            None => options_state.graphics.renderer,
        };
        let (used, said) = backend::choose(wanted, backend::has_card);
        println!("{said}");
        renderer.backends = Some(backend::backends(used));
        uses_dx12 = used == backend::Renderer::Dx12;
    } else {
        println!("renderer: as WGPU_BACKEND says");
    }
    // DirectX 12's shaders by its newer compiler where it is found
    // (`dxc_libraries`); the engine's own choice, the variable
    // `WGPU_DX12_COMPILER`, still has the last word.
    if uses_dx12 && bevy::render::settings::Dx12Compiler::from_env().is_none() {
        match dxc_libraries() {
            Some((dxc, _)) => {
                println!("DirectX 12's shaders by DXC: {}", dxc.display());
                // (As the engine itself asks for it: shader model 6.7 at the
                // most, the dll's own signing taking the place of dxil.dll's.)
                renderer.dx12_shader_compiler =
                    bevy::render::settings::Dx12Compiler::DynamicDxc { dxc_path: dxc.display().to_string(), max_shader_model: wgpu_types::DxcShaderModel::V6_7 };
            }
            None if cfg!(windows) => println!("DirectX 12's shaders by FXC: DXC's dxcompiler.dll and dxil.dll are neither beside trp.exe nor in a Windows SDK"),
            None => {}
        }
    }
    let mut plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                // `local\window_keys_test.ps1` finds the window by it.
                title: "TRP (Tushonka Runtime Project)".into(),
                resolution: bevy::window::WindowResolution::new(options.window[0], options.window[1]),
                present_mode,
                ..default()
            }),
            ..default()
        })
        .set(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(renderer),
            ..default()
        });
    let render_thread = options_state.render_thread_started;
    println!("render thread: {}", if render_thread { "on" } else { "off" });
    if !render_thread {
        plugins = plugins.disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>();
    }

    let mut controller =
        Controller::new(movement, trp_sim::Pacer::new(options.frame_rate, engine.fixed_timestep, engine.maximum_timestep));
    controller.camera_box = camera_box;
    // The simulation's systems as the options' switches have them for
    // this run (settings spec, "Features").
    controller.movement.systems = options_state.in_force.systems();
    if !controller.movement.systems.weight {
        controller.movement.set_weight(&hard, &db.config, 0.0)?;
    }
    // The character's health, with the options' `HEALTH SYSTEM` on: full,
    // as every run begins (the user, 2026-10-14, night), its chances drawn
    // from the run's seed (`--seed`).
    if options_state.in_force.health {
        match trp_sim::health::HealthSettings::new(&db.config) {
            Ok(settings) => {
                let mut health = trp_sim::health::Health::new(std::sync::Arc::new(settings), options.seed);
                // Its skills' buffs (`skills`).
                health.skills = skills.health();
                controller.health = Some(health);
            }
            Err(e) => eprintln!("the install's figures for health could not be read; the character cannot be hurt: {e:#}"),
        }
    }
    // The game's own health panel for the inventory's HEALTH tab, read
    // from the scene that has its equipment panel: its layout, where the
    // app's own look puts its boxes too, and with `TARKOV UI` on its
    // pictures and clips.
    let layouts: Vec<String> = db.items.values().map(|item| item.grid_layout_name().to_owned()).filter(|name| !name.is_empty()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let mut health_look = health_view::HealthLook::read(&unity.install, &panel.file, options_state.in_force.tarkov_ui, health_view::weight_limits(&db.config), &layouts, options_state.in_force.clothing, options_state.in_force.workbench);
    // The game's own mouse pointers, with `TARKOV UI` (`cursor::point`).
    let pointer_pictures = if options_state.in_force.tarkov_ui {
        trp_data::screen::cursor_pictures(&unity.install).unwrap_or_else(|e| {
            warn!("the game's mouse pointers could not be read; the system's stays: {e:#}");
            Vec::new()
        })
    } else {
        Vec::new()
    };
    // The repair's figures (`trp_sim::repair`): the globals' and the
    // hideout's areas' greatest bonuses.
    let areas = std::fs::read_to_string(unity.install.database_dir().join("hideout/areas.json")).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let repair_settings = std::sync::Arc::new(trp_sim::repair::Settings::from_config(&db.config, areas.as_ref()));
    health_look.level_experience = health_view::level_experience(&db.config);
    health_look.skill_effectiveness = skills_view::effectiveness(&db.config, 0);
    let health_words = health_view::Words::load(&unity.install, &health_look.keys());
    // The battle screen's panels of a sight (the zeroing, the
    // magnification), whatever `TARKOV UI` says (the user, 2026-10-24).
    let sight_panels = sight_panels::SightPanelsLook::read(&unity.install, &panel.file);
    // The ragdoll the character becomes when it dies, and the figure its
    // bodies go by (`death`).
    let ragdoll = death::Ragdoll {
        data: match trp_data::ragdoll::RagdollData::load(&mut unity) {
            Ok(data) => Some(data),
            Err(e) => {
                eprintln!("the player's ragdoll could not be read; a corpse will stand where it died: {e:#}");
                None
            }
        },
        max_depenetration: hard.float("CorpseMaxDepenetrationVelocity").unwrap_or_else(|e| {
            eprintln!("{e:#}; the engine's own figure is used for a corpse");
            10.0
        }),
        physics: match trp_data::engine::PhysicsSettings::load(&unity.install) {
            Ok(settings) => Some(settings),
            Err(e) => {
                eprintln!("the engine's physics settings could not be read; a corpse will stand where it died: {e:#}");
                None
            }
        },
    };
    // Things on the ground in the game's engine, with the options' `ITEM
    // PHYSICS` on (`loose`).
    let loose = loose::Loose::new(unity.install.clone(), ragdoll.physics.clone().filter(|_| options_state.in_force.item_physics), &hard, options.seed);
    // The game's step of its physics, at most `MaxSmoothPhysicsDeltaTime`.
    let physics_clock = death::PhysicsClock::new(hard.float("MaxSmoothPhysicsDeltaTime").unwrap_or(0.03));
    // What the run begins with, for `Enter` after a death to put back.
    let start_of_run = death::StartOfRun {
        controller: controller.clone(),
        equipment: equipment.clone(),
        loot: loot.clone(),
        away: away.clone(),
        ground: ground.clone(),
        range: range.clone(),
    };

    // The range lit as Customs in the recorded morning, with the options'
    // `CUSTOMS LIGHT` (`lighting`): read and worked out before the window
    // opens.
    let mut filter_table = None;
    let level_lighting = if options_state.in_force.customs_light {
        // At the options' TIME OF DAY as the run began (`--hour` over it).
        let moment = trp_sim::tod::Moment {
            hour: options_state.hour_started,
            cloudiness: options.cloudiness.unwrap_or(lighting::RANGE_MOMENT.cloudiness),
            wind: lighting::RANGE_MOMENT.wind,
        };
        // The clouds at the quality the run began with, with `CUSTOMS CLOUDS`.
        let clouds_quality = options_state.in_force.customs_clouds.then_some(options_state.clouds_started);
        match lighting::read(&unity.install, moment, clouds_quality) {
            Ok(mut light) => {
                if let Some(c) = &light.clouds {
                    let f = &c.figures;
                    println!(
                        "Customs' clouds at {} quality: the map's opacities {:?}, the wind {:.1} km/h, the sun's colour for them {:?}, their ambient {:?}{}",
                        options_state.clouds_started.key().to_lowercase(),
                        f.opacities,
                        f.scroll_speed,
                        f.sun_color,
                        f.ambient,
                        f.shadows.map_or(String::new(), |s| format!(", their shadows {} square over {} m", s.resolution, s.size))
                    );
                }
                // The camera's own colour filter, with `CAMERA COLOR FILTER`.
                if !options_state.in_force.camera_color_filter {
                    light.color_filter = None;
                }
                // The camera's sharpening and desaturation, with `CAMERA SHARPENING`.
                if !options_state.in_force.camera_sharpening {
                    light.sharpen = None;
                } else if let Some(s) = &light.sharpen {
                    info!("the camera's sharpening: strength {} (clamp {}), the weather's desaturation {:.3}, the ring {:?} by {}", s.strength, s.data.clamp, s.weather, s.data.min_max_radius, s.data.radius_falloff);
                }
                // The sky's scattering over the scene, with `SKY SCATTERING`.
                if !options_state.in_force.sky_scattering {
                    light.scattering = None;
                }
                if let Some(s) = &light.scattering {
                    println!("the sky's scattering over the scene: density {}, height falloff {}, zero level {}, glow {}", s.density, s.height_falloff, s.zero_level, s.sunrise_glow);
                }
                if let Some(filter) = &light.color_filter {
                    match filter.table(&unity.install) {
                        Ok(table) => {
                            println!("the camera's colour filter: {} at {}", filter.name().unwrap_or_default(), filter.amount);
                            filter_table = Some(table);
                        }
                        Err(e) => {
                            eprintln!("the camera's colour filter could not be read; it is not drawn: {e:#}");
                            light.color_filter = None;
                        }
                    }
                }
                println!(
                    "the range lit as Customs at {:02}:{:02} (cloudiness {}): the sun {:?} going {:?}, the ambient from above {:?}, the sky's dome of {} vertices",
                    moment.hour as u32,
                    (moment.hour.fract() * 60.0) as u32,
                    moment.cloudiness,
                    light.sun,
                    light.sun_forward,
                    light.eft_ambient,
                    light.sky.mesh.positions.len()
                );
                Some(light)
            }
            Err(e) => {
                eprintln!("Customs' light could not be read; the range keeps its own: {e:#}");
                None
            }
        }
    } else {
        None
    };
    // Tarkov's surface shading, with `TARKOV SHADING` and the level's light
    // (`shading`).
    let game_shading = options_state.in_force.tarkov_shading && level_lighting.is_some();
    // Without Customs' light, the range's own by the options' hour
    // (`range_day`).
    let range_own_light = level_lighting.is_none();
    if game_shading {
        println!("the scene lit by Tarkov's surface shading");
    }

    // Until the renderer has reported, it counts as having work to do.
    let waiting = PipelinesWaiting(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)));
    // With `TARKOV UI` the screens are composed in gamma space, as the
    // game's are (`gamma_ui`).
    let gamma_composed = options_state.in_force.tarkov_ui;
    // The install, for the sounds read away from the frame (`ui_sound`,
    // `step_sound`), and whether the sounds of moving are wanted.
    let sound_install = unity.install.clone();
    let day_install = unity.install.clone();
    let movement_sounds = options_state.in_force.movement_sounds;
    let world_sounds = options_state.in_force.world_sounds;
    let hands_sounds = options_state.in_force.hands_sounds;
    let grenades = options_state.in_force.grenades_and_knife;
    let health_on = options_state.in_force.health;
    // The face shield's mask over the view (`FACE SHIELD MASK`, a part of
    // `VISUAL EFFECTS`).
    let visor_on = options_state.in_force.face_shield_mask;
    // The weapons' lights and lasers and the head light (`TACTICAL DEVICES`).
    let tactical_on = options_state.in_force.tactical_devices;
    // The shots' bullets (`BALLISTICS`): the calculator, with what it goes by.
    // The effects' entries the hits play, by material.
    let impact_entries = if options_state.in_force.ballistics { trp_data::particles::material_entries(&mut unity).unwrap_or_default() } else { Vec::new() };
    let impact_names: Vec<String> = impact_entries.iter().map(|e| e.name.clone()).collect();
    let bullets = options_state.in_force.ballistics.then(|| bullets::Bullets::new(&hard, &db, engine.gravity, impact_entries)).flatten();
    if options_state.in_force.ballistics && bullets.is_none() {
        warn!("the bullets could not be set up: the ballistic presets or the degradation coefficient were not read");
    }
    // The dummy on the range (`DUMMY`, with the bullets): the player prefab's
    // hit colliders, the armour's and health's figures, the killing hit's
    // force.
    let dummy_source = if options_state.in_force.dummy && bullets.is_some() {
        let colliders = trp_data::ragdoll::RagdollData::hit_colliders(&mut unity);
        let rules = trp_sim::armor::ArmorRules::new(&db.config);
        let health = trp_sim::health::HealthSettings::new(&db.config);
        // Each dummy's character (`trp_sim::puppet`): the AKM in its hands,
        // assembled as the run's own, its animator and logic's settings, the
        // weapon's motion's data, the movement's settings unburdened, the
        // player's body animator, the arms' solver, the player's frame rate.
        let maker = (|| -> anyhow::Result<dummy::PuppetMaker> {
            let weapon = trp_data::weapon::assemble(&mut unity, &db, dummy::WEAPON)?;
            let animator = trp_sim::weapon_animator::WeaponAnimatorData::load(&unity, &weapon)?;
            let firearm = trp_sim::firearm_settings(&unity, &db, &hard, &weapon)?;
            let hands_data = HandsData::load(&mut unity)?;
            let movement = trp_sim::load_movement_settings(&mut unity, &db, &hard, &engine, 0.0)?;
            let limbs = trp_data::player::LimbData::load(&mut unity)?;
            Ok(dummy::PuppetMaker {
                weapon: std::sync::Arc::new(weapon),
                animator,
                firearm,
                hands_data,
                view: trp_sim::ViewSettings::default(),
                movement,
                body: body_data.clone(),
                limbs,
                frame_rate: options.frame_rate,
                physics_step: engine.fixed_timestep,
                longest_frame: engine.maximum_timestep,
                smooth: options_state.in_force.smooth_animations,
            })
        })();
        let maker = match maker {
            Ok(maker) => Some(maker),
            Err(e) => {
                warn!("the dummies' characters could not be made: {e:#}");
                None
            }
        };
        match (colliders, rules, health) {
            (Ok(colliders), Ok(rules), Ok(health)) => Some(dummy::DummySource {
                colliders,
                rules: std::sync::Arc::new(rules),
                health: std::sync::Arc::new(health),
                hit_force: hard.float("HIT_FORCE").unwrap_or(150.0),
                maker,
            }),
            (colliders, rules, health) => {
                warn!(
                    "the dummy could not be set up: {}",
                    [colliders.err(), rules.err(), health.err()].into_iter().flatten().map(|e| format!("{e:#}")).collect::<Vec<_>>().join("; ")
                );
                None
            }
        }
    } else {
        None
    };
    let mut app = App::new();
    let [r, g, b] = lighting::RANGE_OWN_SKY;
    app.insert_resource(ClearColor(Color::srgb(r, g, b)))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: lighting::RANGE_OWN_AMBIENT, ..default() })
        .insert_resource(controller)
        .add_message::<loose::Blasted>()
        .add_message::<tinnitus::Contused>()
        .insert_resource(inventory_view::Carried {
            checked: inventory_view::magazines_in(&equipment.item),
            mastering: options.mastering,
            slot_keys: Vec::new(),
            equipment,
            names,
            loot,
            loot_open: false,
            away,
            loot_of: 0,
            searched_ground: None,
            repairing: false,
            repairs: Vec::new(),
            repair_settings: Some(repair_settings),
            rules,
            fixed_wear,
            note: String::new(),
            panel,
            ground,
            icons: Default::default(),
            pictures: Default::default(),
            pictures_on: options_state.in_force.icon_generation,
            picture_failed: Default::default(),
            spinner: Handle::default(),
            bound: Default::default(),
            top_grenade: None,
            magazine_work: None,
            frozen_keys: Default::default(),
            switched: Vec::new(),
            fold_asked: None,
            round_times: inventory_view::RoundTimes { drills: buffs.mag_drills, ..inventory_view::RoundTimes::of(&db) },
            strength_elite: buffs.strength_elite,
            carry: 1.0 + buffs.strength_lift,
            windows: Vec::new(),
            priority: None,
            // The game's setting for which window Ctrl and a click sends
            // things to, as the install has it (`Auto` where it has none:
            // what the one read on 2026-10-12 had).
            priority_auto: unity.install.game_setting("PriorityWindowMode").is_none_or(|mode| mode == "Auto"),
            asking: None,
            pack_asked: false,
            falling: Vec::new(),
            thrown: Vec::new(),
            item_looks: Default::default(),
            texts,
            info_size: None,
            game_grid_window: false,
            tag_size: None,
            tag_limit: crate::health_view::TAG_LIMIT,
            tag_edits: Vec::new(),
            wishlist: Default::default(),
        })
        .insert_resource(loose)
        .insert_resource(cases::Cases::new(&hard, options_state.in_force.spent_cases))
        // The models' pictures by their levels of detail (`streaming`).
        .insert_resource(streaming::MipStreaming::new(textures))
        .insert_resource(bindings)
        .insert_resource(icons::Icons::new(unity.install.clone(), options_state.in_force.icon_generation, icons::signatures(&db)))
        .insert_resource(options_state.in_force.clone())
        .insert_resource(options_state.game)
        .insert_resource(options_state.sound)
        .insert_resource(options_state)
        .insert_resource(skills)
        .insert_resource(GameData { unity, db, hard, engine, hands_data })
        .insert_resource(options.view)
        .insert_resource(options)
        .insert_resource(player)
        .init_resource::<render_data::Converted>()
        .insert_resource(range)
        .insert_resource(clothing)
        .add_plugins(clothing::ClothingPlugin)
        .add_plugins(edit_build::EditBuildPlugin)
        .add_plugins(info_preview::InfoPreviewPlugin)
        .add_plugins(overall_view::OverallPlugin)
        .add_plugins(skills_view::SkillsPlugin)
        .init_resource::<body_view::KeptCollarbones>()
        .init_resource::<body_view::OutfitOn>()
        .init_resource::<body_view::OutfitPose>()
        .add_plugins(plugins)
        .add_plugins(grid::GridPlugin)
        .add_plugins(shadow_only::ShadowOnlyPlugin)
        // What is hidden kept out of the engine's work (`park`).
        .add_plugins(park::ParkPlugin)
        .add_plugins(unready::UnreadyPlugin)
        .add_plugins(radial_fill::RadialFillPlugin)
        .add_plugins(screen_blur::ScreenBlurPlugin)
        .add_plugins(contusion_view::ContusionViewPlugin)
        .add_plugins(sharpen_view::SharpenViewPlugin)
        .add_plugins(blood_view::BloodViewPlugin)
        .add_plugins(visor_view::VisorViewPlugin)
        .add_plugins(tactical_view::TacticalViewPlugin { on: tactical_on, own_light: range_own_light })
        .add_plugins(distort_view::DistortViewPlugin)
        .add_systems(Update, screen_blur::fade.after(inventory_view::ScreenKeys))
        .init_resource::<feet_down::FeetDown>()
        .insert_resource(FrameMark(std::time::Instant::now()))
        .init_resource::<Watching>()
        .init_resource::<keys::ScriptedMouse>()
        .insert_resource(Warmup(true))
        .init_resource::<render_data::SharedPictures>()
        .insert_resource(waiting.clone())
        .insert_resource(updates)
        .add_plugins(hud::HudPlugin)
        .add_plugins(debug_menu::DebugMenuPlugin)
        .add_plugins(inventory_view::InventoryPlugin)
        .add_plugins(item_menu_view::ItemMenuPlugin)
        .add_plugins(hands_sound::HandsSoundPlugin)
        .add_plugins(shot_sound::ShotSoundPlugin)


        .add_plugins(health_view::HealthViewPlugin)
        .add_plugins(battle_view::BattleViewPlugin)
        .add_plugins(wear::WearPlugin)
        .add_plugins(heat_view::HeatViewPlugin)
        .add_plugins(repair_view::RepairViewPlugin)
        .add_plugins(action_view::ActionViewPlugin)
        .add_plugins(sight_panels::SightPanelsPlugin)
        .insert_resource(sight_panels)
        .add_plugins(ui_sound::UiSoundPlugin)
        .add_plugins(step_sound::StepSoundPlugin)
        .insert_resource(health_words)
        .insert_resource(health_look)
        .add_plugins(options_view::OptionsPlugin)
        .add_plugins(icons::IconsPlugin)
        .add_plugins(figure_view::FigurePlugin)
        // A death: its corpse in the game's own engine (`trp_physics`).
        .add_plugins(death::DeathPlugin)
        .insert_resource(ragdoll)
        .insert_resource(physics_clock)
        .insert_resource(start_of_run)
        .insert_resource(cursor::Pointers::of(pointer_pictures))
        .add_systems(Startup, (setup_world, range::spawn_labels, (weapon::spawn_weapon, body_view::spawn_body).chain()))
        .add_systems(
            Update,
            (
                // The skills' buffs put on again when a level changes.
                (mark_frame, keys::hear, weapon::prepare_rigs, weapon::free_rigs, skills::host_levels, skills::stimulated, skills::apply).chain(),
                (switch_view, cursor::hold, cursor::keep_alt, cursor::point).chain(),
                controller::gather_input.run_if(running),
                (controller::step.run_if(running), feet_down::ground_feet).chain(),
                fly_camera,
                controller::apply,
                (show_things, range::place_labels),
                animation::play_clips,
                animation::pose_rig,
                hands_view::apply_hands,
                arms::place_hands,
                (body_view::show_outfit, body_view::show_torso, body_view::show_upper_body).chain(),
                body_view::pose_body,
                (body_view::see_collarbones, body_view::keep_collarbones, body_view::see_outfit, body_view::pose_outfit).chain(),
                // A death, after the body is drawn for the frame; and
                // things thrown and let fall, stepped with the corpse.
                (death::die, death::follow, death::fade, death::begin_again, loose::set_going, loose::read_models, loose::fly, blast_view::grenade_blasts, blast_sound::play_blasts.run_if(resource_exists::<blast_sound::BlastSounds>), tinnitus::ring.run_if(resource_exists::<tinnitus::Tinnitus>), (contusion_view::hits, contusion_view::update, contusion_view::eye_burn, blood_view::blood_frame).chain().run_if(resource_exists::<contusion_view::EyeBurnState>), cases::fly_cases, (bullets::fly_bullets, explosion_view::impacts.run_if(resource_exists::<explosion_view::Explosions>), decal_view::lay_holes.run_if(resource_exists::<decal_view::BulletHoles>), impact_sound::play_impacts.run_if(resource_exists::<impact_sound::ImpactSounds>)).chain().run_if(resource_exists::<bullets::Bullets>), muzzle_view::muzzle_flash.run_if(resource_exists::<muzzle_view::MuzzleViews>), (magazine_rounds::find_hands_magazine, magazine_rounds::show_rounds, magazine_rounds::find_hands_chamber, magazine_rounds::show_chambered).chain().run_if(resource_exists::<magazine_rounds::NextParity>), knife::sweep).chain(),
                hud::debug,
                log_frame.run_if(running),
                log_pacing.run_if(running),
                capture.run_if(running),
                warm_up,
            )
                .chain(),
        );
    // The reactive targets (`REACTIVE TARGETS`), met by the bullets: read
    // as the run begins and moved after the frame's bullets (`poppers`).
    if bullets.is_some() && with_poppers {
        app.init_resource::<poppers::Poppers>();
        app.add_systems(Update, (poppers::read_poppers, poppers::place_poppers).chain());
        app.add_systems(Update, poppers::move_poppers.after(bullets::fly_bullets));
    }
    if let Some(bullets) = bullets {
        app.insert_resource(bullets);
        impact_sound::start_reading(&mut app, sound_install.clone(), impact_names);
    }
    // The workbench's model, read as the run begins (`workbench`).
    app.init_resource::<workbench::WorkbenchModel>().init_resource::<workbench::WorkbenchAsked>();
    app.add_systems(Update, (workbench::read_workbench, workbench::place_workbench).chain());
    if let Some(source) = dummy_source {
        app.insert_resource(source);
        app.add_systems(Update, (dummy::make_dummies, dummy::run_dummies, dummy::place_dummies).chain().before(bullets::fly_bullets));
        // Read after the screen's keys, which a capture's taps press.
        app.add_systems(Update, dummy::dress_dummies.after(inventory_view::ScreenDrawn).after(dummy::make_dummies));
        app.add_systems(Update, dummy::fall_dummies.after(bullets::fly_bullets).after(death::die));
    }
    // What a rig brought to the graphics card, asked about here and
    // answered by the renderer (`weapon::Uploads`).
    let uploads = weapon::UploadWatch::default();
    app.insert_resource(uploads.clone());
    if let Some(wardrobe) = wardrobe {
        app.insert_resource(wardrobe);
    }
    if let Some(renderer) = app.get_sub_app_mut(bevy::render::RenderApp) {
        use bevy::render::{Render, RenderSystems};
        renderer.insert_resource(waiting).add_systems(Render, count_waiting.in_set(RenderSystems::Cleanup));
        renderer.insert_resource(uploads).add_systems(Render, weapon::watch_uploads.in_set(RenderSystems::Cleanup));
        // Where a slow frame of the renderer's goes, if asked for.
        render_timing::add(renderer);
        render_timing::add_memory(renderer);
    }
    // New pictures and materials taken in, and the levels the pictures want
    // worked out, after the frame and before the card takes what changed
    // (`streaming`). The copies made ready taken before the stores say what
    // changed, so that the card hears of a copy with its materials.
    app.add_systems(PostUpdate, streaming::take_made.before(bevy::asset::AssetEventSystems));
    streaming::watch_card(&mut app);
    app.add_systems(Last, (streaming::take_new, streaming::stream).chain());
    render_timing::add_main(&mut app);
    render_timing::add_census(&mut app);
    render_timing::add_drop_hidden(&mut app);
    ui_sound::start_reading(&mut app, sound_install.clone());
    step_sound::start_reading(&mut app, sound_install.clone(), movement_sounds);
    // The things' own sounds, after the range has stepped its things.
    thing_sound::start_reading(&mut app, sound_install.clone(), world_sounds);
    app.add_systems(Update, thing_sound::play_things.run_if(resource_exists::<thing_sound::ThingSoundBanks>).after(controller::step));
    // A weapon heard as it lands, under the same switch.
    landing_sound::start_reading(&mut app, sound_install.clone(), world_sounds);
    cases::start_reading(&mut app, sound_install.clone());
    if grenades {
        blast_sound::start_reading(&mut app, sound_install.clone());
    }
    if health_on {
        tinnitus::start_reading(&mut app, sound_install.clone());
        // The character's voice and a bone's crack.
        voice_sound::start_reading(&mut app, sound_install.clone(), true);
        app.init_resource::<contusion_view::ContusionState>();
        app.init_resource::<blood_view::BloodState>();
        contusion_view::start_reading(&mut app, sound_install.clone());
    }
    // The camera's sharpening, after health's accumulators have run.
    app.add_systems(Update, sharpen_view::update.after(contusion_view::update));
    if visor_on {
        visor_view::start_reading(&mut app, sound_install.clone());
        app.add_systems(Update, visor_view::visor_frame.after(contusion_view::update).after(inventory_view::ScreenKeys));
    }
    if rounds_drawn {
        app.insert_resource(magazine_rounds::NextParity::default());
    }
    // The flash's pass is always in the graph (the gamma passes go round
    // it); with `MUZZLE FLASH` off nothing feeds it.
    app.add_plugins(muzzle_view::MuzzleViewPlugin { on: muzzle_drawn });
    // Drawn by the flash's pass, before its jets.
    app.add_plugins(particles_view::ParticlesViewPlugin { on: particles_drawn });
    // A grenade's explosion, on the frame before the muzzle's pass.
    app.add_plugins(explosion_view::ExplosionViewPlugin { on: particles_drawn });
    // A grenade's scorch, with the explosions and the hard settings'
    // deferred decals (`decal_view`).
    let deferred_decals = app.world().get_resource::<GameData>().is_some_and(|game| game.hard.float("DEFERRED_DECALS_ENABLED").is_ok_and(|on| on != 0.0));
    app.add_plugins(decal_view::DecalPlugin { on: particles_drawn && deferred_decals });
    hands_sound::start_reading(&mut app, sound_install, hands_sounds);
    if let Some(path) = app.world().resource::<Options>().log_sounds.clone() {
        match std::fs::File::create(&path) {
            Ok(file) => {
                app.insert_resource(step_sound::SoundLog(std::sync::Mutex::new(std::io::BufWriter::new(file))));
            }
            Err(e) => eprintln!("the sounds' log could not be made at {}: {e}", path.display()),
        }
    }
    if app.world().resource::<Options>().sound_out.is_some() {
        app.world().resource::<ui_sound::Mixer>().0.start_tape();
        app.add_systems(Last, write_tape);
    }
    // The camera's PRISM, before its screen effects, and the sky, with
    // `CUSTOMS LIGHT`: the light figures, the environment's exposure, no
    // flat ambient of Bevy's (the ambient is the environment map's).
    app.add_plugins((prism_view::PrismPlugin, tod_sky_view::TodSkyPlugin, color_filter_view::ColorFilterPlugin { table: filter_table }, scattering_view::ScatteringPlugin, lighting::HallProbePlugin));
    app.add_plugins(preview_tonemap::PreviewTonemapPlugin);
    // The ambient occlusion, between the depth's copy and the scattering
    // (`hbao_view`): after both their plugins.
    app.add_plugins(hbao_view::HbaoPlugin);
    // The clouds, between the depth's copy and the ambient occlusion
    // (`clouds_view`): after their plugins.
    app.add_plugins(clouds_view::CloudsPlugin);
    // The ambient highlight's twins (`highlight_view`).
    app.add_plugins(highlight_view::HighlightPlugin);
    // The scopes' lenses and their optic camera (`optic_view`).
    app.add_plugins(optic_view::OpticViewPlugin { pip });
    // The handheld rangefinder's readout (`rangefinder_view`).
    app.add_plugins(rangefinder_view::RangefinderViewPlugin);
    // Bevy's PBR lighting patched for the game's shading (`shading`).
    app.add_plugins(shading::ShadingPlugin { on: game_shading });
    // The game's frame rate limit (`frame_limit`).
    app.insert_resource(limiter).add_plugins(frame_limit::FrameLimitPlugin);
    if range_own_light {
        range_day::start(&mut app, day_install);
    }
    if let Some(light) = level_lighting {
        let [grey, speed, fade] = light.outdoor;
        app.insert_resource(prism_view::PrismEnvironment { exposure: trp_sim::prism::EnvironmentExposure::outdoor(grey, speed, fade), indoor: light.indoor });
        app.insert_resource(GlobalAmbientLight { brightness: 0.0, ..default() });
        app.insert_resource(light);
    }
    // The camera's effects in gamma space, and the screens with `TARKOV UI`.
    app.add_plugins(gamma_ui::GammaUiPlugin { ui: gamma_composed });
    app.run();
    Ok(())
}

// ==== The warm-up, the pacing log and the frames' log ====

/// Whether the run is still warming up.
///
/// The engine builds what it draws a kind of thing with (a pipeline: its
/// shaders, for one layout of mesh and one sort of material) the first
/// time that kind of thing is in the picture. It builds in the background,
/// several at once, and leaves the thing out of the picture until its
/// pipeline is ready, which on DirectX is a second or more: the range, the
/// body and the weapon were missing from the first second and a half of
/// every run, and a weapon first drawn later (a character that begins with
/// empty hands) was missing again while its own were built.
///
/// So the run begins with what it will come to draw in the picture at
/// once, behind a curtain: the weapon though the character may begin
/// without it, and a thing as it lies on the ground. When the renderer
/// says it has nothing left to build, the curtain goes and the run begins.
/// Measured on the machine this was written on: 1.7 s with DirectX 12 and
/// 0.4 s with Vulkan (`--backend vulkan`), which is how long the range and
/// the weapon used to be missing for; nothing is built that would not have
/// been, and nothing of it is left once it is over. Not shown beforehand,
/// and so still a moment late the first time they are seen: the upper body
/// from outside (the free camera; a fifth of the kind and a second more on
/// DirectX), and whatever is first worn in a run that began bare.
#[derive(Resource)]
pub struct Warmup(pub bool);

/// Whether the run has begun: the warm-up is over. The simulation, the
/// keys, the scripted presses and drags of an unattended run and the
/// frames a capture counts all wait for it, so that a run's first frame
/// is the first one that shows it whole.
/// The mixed sound of the run written out as it ends (`--sound-out`).
fn write_tape(mut exit: MessageReader<AppExit>, options: Res<Options>, mixer: Res<ui_sound::Mixer>) {
    if exit.read().next().is_none() {
        return;
    }
    let (Some(path), Some(wav)) = (&options.sound_out, mixer.0.tape_wav()) else { return };
    match std::fs::write(path, wav) {
        Ok(()) => info!("the run's sound written to {}", path.display()),
        Err(e) => warn!("the run's sound could not be written to {}: {e}", path.display()),
    }
}

pub fn running(warmup: Res<Warmup>) -> bool {
    !warmup.0
}

/// How many pipelines the renderer still has to build, as it last said.
/// The renderer's own world writes it at the end of each of its frames.
#[derive(Resource, Clone)]
pub(crate) struct PipelinesWaiting(pub(crate) std::sync::Arc<std::sync::atomic::AtomicUsize>);

/// The frames a warm-up lasts at the least (what reaches the renderer does
/// so a frame or two after it is made), the frames in a row the renderer
/// must have had nothing to build, and the seconds after which the run
/// begins whatever the renderer says.
const WARMUP_FRAMES: u32 = 4;
const WARMUP_CALM: u32 = 8;
const WARMUP_LIMIT: f32 = 30.0;

/// What is in the scene only while the run warms up: the curtain, and
/// things shown for the building's sake.
#[derive(Component)]
struct WarmupProp;

/// In the renderer's world: how many pipelines are still to be built.
fn count_waiting(cache: Res<bevy::render::render_resource::PipelineCache>, waiting: Res<PipelinesWaiting>, mut frame: Local<u32>) {
    waiting.0.store(cache.waiting_pipelines().count(), std::sync::atomic::Ordering::Relaxed);
    *frame += 1;
    // `TRP_ICON_TRACE`: each pipeline still waiting, every 60 of the
    // renderer's frames, by its label and how far it has got (a new kind of
    // material's took 1 to 2.5 s on DirectX 12, 2026-10-14).
    if std::env::var_os("TRP_ICON_TRACE").is_some() && *frame % 60 == 0 {
        use bevy::render::render_resource::{CachedPipelineState, PipelineDescriptor};
        let pipelines: Vec<&bevy::render::render_resource::CachedPipeline> = cache.pipelines().collect();
        for id in cache.waiting_pipelines() {
            let Some(pipeline) = pipelines.get(id) else { continue };
            let state = match &pipeline.state {
                CachedPipelineState::Queued => "queued".to_owned(),
                CachedPipelineState::Creating(_) => "creating".to_owned(),
                CachedPipelineState::Ok(_) => "ok".to_owned(),
                CachedPipelineState::Err(e) => format!("error {e}"),
            };
            let (label, defs) = match &pipeline.descriptor {
                PipelineDescriptor::RenderPipelineDescriptor(d) => (d.label.clone(), d.fragment.as_ref().map_or(0, |f| f.shader_defs.len())),
                PipelineDescriptor::ComputePipelineDescriptor(d) => (d.label.clone(), d.shader_defs.len()),
            };
            info!("pipeline trace {}: {id} {label:?} {state}, {defs} defs", *frame);
        }
    }
}

/// Ends the warm-up when the renderer has caught up, and clears its
/// things away.
fn warm_up(
    mut commands: Commands,
    mut warmup: ResMut<Warmup>,
    waiting: Res<PipelinesWaiting>,
    time: Res<Time<Real>>,
    props: Query<Entity, With<WarmupProp>>,
    mut budget: ResMut<bevy::render::render_asset::RenderAssetBytesPerFrame>,
    preparing: Option<Res<weapon::Preparing>>,
    mut frames: Local<[u32; 2]>,
) {
    if !warmup.0 {
        return;
    }
    let [seen, calm] = &mut *frames;
    *seen += 1;
    // A rig asked for at the start is made ready on another thread; the
    // curtain waits for it, so that what it brings to the graphics card
    // goes up under it (the StG 44's uncompressed pictures had stood a
    // frame of play still for 100 ms). The inventory's pictures are not
    // waited for: as in the game they are asked for when the screen first
    // shows an item (`icons::ask_icons`; icons spec, "When a picture is
    // asked for").
    let preparing = preparing.is_some_and(|preparing| preparing.busy());
    let building = waiting.0.load(std::sync::atomic::Ordering::Relaxed) > 0 || *seen < WARMUP_FRAMES || preparing;
    *calm = if building { 0 } else { *calm + 1 };
    if *calm >= WARMUP_CALM || time.elapsed_secs() > WARMUP_LIMIT {
        warmup.0 = false;
        info!("the renderer had built what it draws with {:.2} s into the run ({} frames)", time.elapsed_secs(), *seen);
        for prop in &props {
            commands.entity(prop).despawn();
        }
        // From here on what goes up to the graphics card is spread over
        // frames, a picture or so a frame: a rig made ready away from the
        // frame (`weapon::Preparing`) brings tens of megabytes of pictures
        // at once, which in one frame stood it still for 137 ms (the
        // StG 44's, uncompressed). Under the curtain, all at once.
        budget.max_bytes = Some(UPLOAD_BUDGET);
    }
}

/// What goes up to the graphics card a frame after the curtain, at most
/// (a whole picture or mesh at least).
const UPLOAD_BUDGET: usize = 8 << 20;

/// When this frame's systems began, for `--log-pacing`.
#[derive(Resource)]
struct FrameMark(std::time::Instant);

fn mark_frame(mut mark: ResMut<FrameMark>) {
    mark.0 = std::time::Instant::now();
}

/// Writes one line for the frame about to be drawn, when asked to: how long
/// the frame took by the engine's clock and by a clock read here, what the
/// simulation ran in it and what that cost, and where the camera is in the
/// world. Whether the app draws evenly is read off these.
fn log_pacing(
    options: Res<Options>,
    real: Res<Time<Real>>,
    mark: Res<FrameMark>,
    controller: Res<Controller>,
    player: Query<&Transform, With<weapon::Player>>,
    pivot: Query<&Transform, With<weapon::ViewPivot>>,
    camera: Query<&Transform, With<MainCamera>>,
    body: Option<Res<body_view::BodyRig>>,
    parts: Res<body_view::BodyParts>,
    helper: TransformHelper,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut out: Local<Option<std::io::BufWriter<std::fs::File>>>,
    mut last: Local<Option<std::time::Instant>>,
    mut frame: Local<u32>,
) {
    use std::io::Write as _;
    let Some(path) = &options.log_pacing else { return };
    let (Ok(player), Ok(pivot), Ok(camera)) = (player.single(), pivot.single(), camera.single()) else { return };
    *frame += 1;
    // An unattended run is there to show what a played window does, and a
    // window is presented differently without the focus: it asks for it.
    let mut focused = false;
    if let Ok(mut window) = window.single_mut() {
        focused = window.focused;
        if options.unattended() && !focused && *frame % 60 == 30 {
            window.focused = true;
        }
    }
    if out.is_none() {
        match std::fs::File::create(path) {
            Ok(file) => {
                let mut file = std::io::BufWriter::new(file);
                let _ = writeln!(
                    file,
                    "frame t dt_ms wall_ms pacer_ms sim_frames physics_steps sim_ms update_ms blend cam_x cam_y cam_z yaw pitch state focused feet_x feet_y feet_z body_yaw lfoot_x lfoot_y lfoot_z rfoot_x rfoot_y rfoot_z pelvis_x pelvis_y pelvis_z rib_x rib_y rib_z script_t ltoe_x ltoe_y ltoe_z rtoe_x rtoe_y rtoe_z anim anim_next"
                );
                *out = Some(file);
            }
            Err(e) => {
                warn!("{}: {e}", path.display());
                return;
            }
        }
    }
    let now = std::time::Instant::now();
    let wall = last.replace(now).map_or(0.0, |before| (now - before).as_secs_f64() * 1000.0);
    // The camera hangs from the view pivot, and that from the player.
    let eye = player.transform_point(pivot.transform_point(camera.translation));
    let (yaw, pitch) = controller.look();
    // The point the view hangs at, in the world.
    let ribcage = player.transform_point(pivot.translation);
    let paced = controller.paced;
    // The drawn body's feet and hips, in the world, where a body is drawn.
    let drawn = |end: &str| {
        body.as_ref()
            .and_then(|rig| rig.bone(&parts, end))
            .and_then(|bone| helper.compute_global_transform(bone).ok())
            .map_or(Vec3::ZERO, |at| at.translation())
    };
    let (left, right, pelvis) = (drawn("/Base HumanLFoot"), drawn("/Base HumanRFoot"), drawn("/Base HumanPelvis"));
    let (left_toe, right_toe) = (drawn("/Base HumanLToe"), drawn("/Base HumanRToe"));
    // The moment drawn, by the clock of the script being played if one is.
    let script_time = controller.drawn_time() - options.scenario.as_ref().map_or(0.0, |_| options.scenario_at);
    // The state the body's animator is in and the one it is entering, by
    // their own names. These are the simulation's newest, a frame or two
    // ahead of what is drawn.
    let own = |name: &str| name.rsplit('.').next().unwrap_or(name).replace(' ', "_");
    let (anim, anim_next) = controller.movement.body.state_names();
    let (anim, anim_next) = (own(anim), anim_next.map_or(String::from("-"), own));
    let Some(out) = out.as_mut() else { return };
    let _ = writeln!(
        out,
        "{} {:.5} {:.4} {:.4} {:.4} {} {} {:.4} {:.4} {:.4} {:.5} {:.5} {:.5} {:.4} {:.4} {:?} {} {:.5} {:.5} {:.5} {:.4} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {:.5} {anim} {anim_next}",
        *frame,
        real.elapsed_secs_f64(),
        real.delta_secs_f64() * 1000.0,
        wall,
        paced.host_dt * 1000.0,
        paced.frames,
        paced.physics_steps,
        paced.cost * 1000.0,
        (now - mark.0).as_secs_f64() * 1000.0,
        paced.blend,
        eye.x,
        eye.y,
        eye.z,
        yaw,
        pitch,
        controller.movement.state,
        focused as u8,
        player.translation.x,
        player.translation.y,
        player.translation.z,
        controller.movement.drawn_body_yaw(),
        left.x,
        left.y,
        left.z,
        right.x,
        right.y,
        right.z,
        pelvis.x,
        pelvis.y,
        pelvis.z,
        ribcage.x,
        ribcage.y,
        ribcage.z,
        script_time,
        left_toe.x,
        left_toe.y,
        left_toe.z,
        right_toe.x,
        right_toe.y,
        right_toe.z,
    );
}

/// Writes one line for the frame about to be drawn, when asked to. Places
/// are in the camera's own axes (x right, y up, -z ahead), in metres.
fn log_frame(
    options: Res<Options>,
    time: Res<Time>,
    player: Res<ClipPlayer>,
    rig: Option<Res<WeaponRig>>,
    held: Option<Res<weapon::HandsRig>>,
    controller: Res<Controller>,
    body: Option<Res<body_view::BodyRig>>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    camera: Query<(Entity, &Transform), With<MainCamera>>,
    mut out: Local<Option<std::io::BufWriter<std::fs::File>>>,
    mut frame: Local<u32>,
) {
    use std::io::Write as _;
    let (Some(path), Some(rig), Ok((camera_entity, camera))) = (&options.log_frames, rig, camera.single()) else { return };
    *frame += 1;
    if out.is_none() {
        match std::fs::File::create(path) {
            Ok(file) => *out = Some(std::io::BufWriter::new(file)),
            Err(e) => {
                warn!("{}: {e}", path.display());
                return;
            }
        }
    }
    // The rig and the camera both hang from the view pivot.
    let seen = |entity: Entity| {
        hands_view::in_root(entity, rig.pivot, |e| transforms.get(e).ok().copied(), &parents)
            .map(|t| camera.rotation.inverse() * (t.translation - camera.translation))
    };
    let show = |v: Option<Vec3>| v.map_or("-".to_owned(), |v| format!("{:.4} {:.4} {:.4}", v.x, v.y, v.z));
    // In the world: up the whole hierarchy from the thing.
    let world = |entity: Entity| -> Option<Transform> {
        let mut result = Transform::IDENTITY;
        let mut current = entity;
        loop {
            result = transforms.get(current).ok()?.mul_transform(result);
            match parents.get(current) {
                Ok(parent) => current = parent.parent(),
                Err(_) => return Some(result),
            }
        }
    };
    let world_camera = world(camera_entity);
    let world_weapon = rig.weapon_root_anim.and_then(world).map(|t| t.translation);
    // Each arm's upper arm to elbow, elbow to palm and upper arm to palm,
    // and the weapon from the view pivot (the ribcage), in the world.
    let at = |e: Entity| world(e).map(|t| t.translation);
    let arm_lengths: Vec<String> = rig
        .arms
        .iter()
        .map(|arm| match (at(arm.upper()), at(arm.elbow()), at(arm.palm())) {
            (Some(u), Some(e), Some(p)) => format!("{:.4} {:.4} {:.4}", u.distance(e), e.distance(p), u.distance(p)),
            _ => "- - -".to_owned(),
        })
        .collect();
    let apart = at(rig.pivot).zip(world_weapon).map_or(-1.0, |(p, w)| p.distance(w));
    // Each collarbone the meshes hang on to the nearest upper arm.
    let collars: Vec<String> = body
        .as_deref()
        .map(|body| {
            body.kept_collarbones()
                .filter_map(at)
                .map(|c| rig.arms.iter().filter_map(|arm| at(arm.upper())).map(|u| c.distance(u)).fold(f32::MAX, f32::min))
                .map(|d| format!("{d:.4}"))
                .collect()
        })
        .unwrap_or_default();
    let weapon = rig.weapon_root_anim.and_then(seen);
    let hands: Vec<String> = rig.arms.iter().map(|arm| show(seen(arm.palm()))).collect();
    // The clip on screen, where clips are played by name; in play, the
    // state the rig's animator is in or on its way into, how far through
    // the state it is in, and whether it is on a way between two.
    let state = held.as_deref().and_then(|h| h.animator()).map(|animator| animator.state());
    let (clip, at, blend) = match (player.by_name, &state) {
        (false, Some((state, at, next))) => (next.unwrap_or(state).trim_start_matches("Hands.").replace(' ', "_"), *at, next.is_some() as u8 as f32),
        _ => {
            let (name, at) = player.status().map_or(("-", 0.0), |(name, at, _)| (name, at));
            (name.to_owned(), at, player.blend())
        }
    };
    // The state the body's layer of gestures is in or entering, how far
    // through its state it is, and how much of the left hand is the
    // body's just now.
    let gesture = controller.movement.body.interaction_state().map_or(String::from("- 0 0"), |(state, at, next)| {
        let name = next.unwrap_or(state).trim_start_matches("InteractionsLayer.").replace(' ', "_");
        format!("{name} {at:.3} {:.3}", controller.movement.arm_pose(false).hand)
    });
    let Some(out) = out.as_mut() else { return };
    let _ = writeln!(
        out,
        "{} t {:.4} clip {clip} {at:.3} blend {blend:.2} weapon {} hands {} camera {:.4} {:.4} {:.4} {:.4} gesture {gesture} world_weapon {} world_camera {} {} arms {} apart {apart:.4} collar_to_upper {}",
        *frame,
        time.elapsed_secs(),
        show(weapon),
        hands.join(" | "),
        camera.rotation.x,
        camera.rotation.y,
        camera.rotation.z,
        camera.rotation.w,
        show(world_weapon),
        show(world_camera.map(|t| t.translation)),
        world_camera.map_or("-".to_owned(), |t| {
            let f = t.rotation * Vec3::NEG_Z;
            format!("{:.4} {:.4} {:.4}", f.x, f.y, f.z)
        }),
        arm_lengths.join(" | "),
        collars.join(" "),
    );
    let _ = out.flush();
}

// ==== The world set up, the views switched, the free camera, the capture ====

#[derive(Component)]
pub struct MainCamera;

/// What moves of one of the range's things, by its place in `Range::things`.
#[derive(Component)]
pub struct ThingPart(pub usize);

/// Puts each thing's moving part where the thing stands now. The things
/// move on the simulation's frames (`Range::step_things`), and are drawn
/// as the last of them left them.
fn show_things(range: Res<range::Range>, mut parts: Query<(&ThingPart, &mut Transform)>) {
    for (part, mut transform) in &mut parts {
        if let Some((place, _, _)) = range.things.get(part.0).and_then(|placed| placed.part_shown()) {
            if *transform != place {
                *transform = place;
            }
        }
    }
}


/// The near plane of the game's camera, as a recorded trace reports it.
const NEAR_PLANE: f32 = 0.03;

fn setup_world(
    mut commands: Commands,
    options: Res<Options>,
    features: Res<options_view::Features>,
    range: Res<range::Range>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<grid::GridMaterial>>,
    level: Option<Res<lighting::LevelLighting>>,
    mut images: ResMut<Assets<Image>>,
) {
    // The warm-up's curtain, over everything, in the sky's colour.
    let curtain = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    commands.spawn((WarmupProp, curtain, BackgroundColor(Color::srgb(0.55, 0.63, 0.70)), GlobalZIndex(1000)));
    let camera = commands.spawn((
        MainCamera,
        // It sees the hands' rigs besides the world (`weapon::HANDS_LAYER`).
        weapon::seeing_hands(),
        // Its depth readable after the main pass (`muzzle_view`'s copy).
        Camera3d { depth_texture_usages: (bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT | bevy::render::render_resource::TextureUsages::TEXTURE_BINDING).into(), ..default() },
        // The field of view follows the weapon motion every frame.
        // The near plane is the game camera's, as a recorded trace reports it.
        // Bevy 0.18 clips by a plane of its own besides (`near_clip_plane`,
        // 0.1 m unless told): it is put at the near plane, or everything
        // nearer than 10 cm is cut away (`docs/findings.md`).
        Projection::Perspective(PerspectiveProjection {
            fov: options.field_of_view.to_radians(),
            near: NEAR_PLANE,
            near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -NEAR_PLANE),
            ..default()
        }),
        Transform::from_xyz(0.0, 1.6, 0.0),
        // The world blurred behind the inventory, as the game's camera
        // blurs it (`screen_blur`).
        screen_blur::ScreenBlur::default(),
        // A contusion's screen effects and a flash's burn (`contusion_view`).
        contusion_view::ContusionScreen::default(),
        contusion_view::EyeBurnScreen::default(),
        // Blood on the screen (`blood_view`).
        blood_view::BloodScreen::default(),
        // A shot's flash and the gun smoke (`muzzle_view`).
        muzzle_view::JetScreen::default(),
        muzzle_view::SmokeScreen::default(),
        particles_view::FumeScreen::default(),
        particles_view::SparkScreen::default(),
        particles_view::HazeScreen::default(),
        explosion_view::ExplosionScreen::default(),
    )).id();
    // The scene's multisampling, as the options have it (`GRAPHICS`, `MSAA`).
    commands.entity(camera).insert(options_view::msaa_of(options.msaa.unwrap_or(4)));
    // The game's ambient occlusion at the choice the run took, with
    // `AMBIENT OCCLUSION`: its passes, and Bevy's normal prepass for the
    // normals they read (`hbao_view`).
    if let Some(settings) = options.ssao.and_then(trp_sim::hbao::SsaoMode::hbao).filter(|_| features.ambient_occlusion) {
        commands.entity(camera).insert((bevy::core_pipeline::prepass::NormalPrepass, hbao_view::camera_parts(settings, options.ssao_ao_only)));
    }
    // With `TARKOV UI` the interface is drawn as the game's engine draws
    // its own, its edges hard: Bevy's softening of every node's edge over a
    // pixel left a light seam where two of the game's see-through pieces
    // meet (a slot's dark ground beside the next one's in the quick-use
    // bar). The app's own look keeps it (its one rounded corner).
    if features.tarkov_ui {
        commands.entity(camera).insert(bevy::ui_render::UiAntiAlias::Off);
    }
    // The screens composed in gamma space with `TARKOV UI` (`gamma_ui`).
    if features.tarkov_ui {
        commands.entity(camera).insert(gamma_ui::GammaUi);
    }
    let cascades = shadows::with_baked_reach(shadows::cascades(options.shadow_quality.unwrap_or(shadows::START_QUALITY), NEAR_PLANE), range::baked_reach());
    match level.as_deref() {
        // Customs' sun, its ambient as an environment map, and the camera
        // drawing the scene's values in HDR for PRISM (`lighting`,
        // `prism_view`); the sky is `tod_sky_view`'s, black behind it as the
        // game's camera clears.
        Some(light) => {
            // The clouds with `CUSTOMS CLOUDS` (`clouds_view`): the sun
            // carries their shadows' cookie, laid from its light's place.
            let cookie = light.clouds.as_ref().and_then(|clouds| clouds_view::begin(&mut commands, &mut images, camera, clouds));
            let turned = Transform::default().looking_to(light.sun_forward, Vec3::Y);
            match cookie {
                Some((image, place, scale)) => {
                    commands.spawn((light.sun_light(), cascades, turned.with_translation(place).with_scale(Vec3::splat(scale)), bevy::light::DirectionalLightTexture { image, tiled: true }, lighting::RangeSun, weapon::seeing_hands()));
                }
                None => {
                    commands.spawn((light.sun_light(), cascades, turned, lighting::RangeSun, weapon::seeing_hands()));
                }
            }
            let environment = light.environment(&mut images);
            // The range hall's stencil volume over its inside, and its
            // lamps with `HALL LAMPS` (`lighting`, `range::hall_lamps`).
            if let Some(mut probe) = lighting::hall_probe(&environment) {
                // With the lamps, the light they bounce off the hall in place
                // of the sky's it takes away (`lighting::hall_bounce`).
                if features.hall_lamps {
                    if let Some(bounce) = lighting::hall_bounce(&mut images) {
                        probe.1 = bounce;
                    }
                }
                commands.spawn(probe);
            }
            // The game's ambient highlight over the hands' stencil type, with
            // `AMBIENT HIGHLIGHT` (`highlight_view`): within the hall, by its
            // indoor volume's alpha.
            if let Some((sh, multiplier, blur)) = light.highlight.filter(|_| features.ambient_highlight) {
                let rows = [sh.a[0], sh.a[1], sh.a[2], sh.b[0], sh.b[1], sh.b[2], sh.c].map(Vec4::from_array);
                let indoors = range::indoors().map(|(centre, size, yaw)| (Transform::from_translation(centre).with_rotation(Quat::from_rotation_y(yaw)).with_scale(size).to_matrix().inverse(), lighting::HALL_STENCIL));
                info!("the ambient highlight over the hands: times {multiplier}");
                commands.insert_resource(highlight_view::HighlightFigures { sh: rows, multiplier, blur, indoors });
            }
            if features.hall_lamps {
                for at in range::hall_lamps() {
                    commands.spawn((lighting::hall_lamp(at.y), Transform::from_translation(at).looking_to(Vec3::NEG_Y, Vec3::Z)));
                }
            }
            commands.entity(camera).insert((
                bevy::render::view::Hdr,
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                bevy::camera::Exposure { ev100: lighting::EXPOSURE_EV100 },
                environment,
                prism_view::screen_of(&light.prism),
                Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
            ));
            // The game's shading asked for, with its ambient pass's
            // brightness term (`shading`).
            if shading::on() {
                commands.entity(camera).insert(shading::asking(light.ambient_brightness()));
            }
            if let Some(filter) = &light.color_filter {
                commands.entity(camera).insert(color_filter_view::ColorFilterScreen::of(filter.amount));
            }
            if let Some(s) = &light.scattering {
                commands.entity(camera).insert((
                    scattering_view::ScatteringScreen::of(&light.sky.globals, s.height_falloff, s.density, s.sunrise_glow, &s.dithering),
                    scattering_view::ScatteringZero(s.zero_level),
                ));
            }
        }
        None => {
            commands.spawn((
                DirectionalLight { illuminance: lighting::RANGE_OWN_SUN, shadows_enabled: true, ..default() },
                weapon::seeing_hands(),
                // Its cascades as the game's shadow quality has them, from
                // the camera's near plane (`shadows`).
                cascades,
                // At the options' hour once Customs' time of day is read
                // (`range_day`).
                lighting::range_own_sun_turn(),
                lighting::RangeSun,
            ));
        }
    }

    // A plain test range: ground, posts every 5 m along a lane to judge
    // speed against, a ring of blocks to judge turning against, and two
    // walls to push the weapon into. The simulation feels the same boxes.
    // All of it carries the same grid of lines, to read distance, speed and
    // height by.
    let lined = |lines: grid::Lines| grid::GridLines { lines: if options.grid { lines } else { lines.none() } };
    commands.spawn((
        // (800 m across since 0.142.0, for the reactive targets 300 m down
        // the lanes.)
        Mesh3d(meshes.add(Plane3d::default().mesh().size(800.0, 800.0))),
        MeshMaterial3d(materials.add(grid::GridMaterial {
            // A neutral grey (the user, 2026-10-19: "I want to make the
            // whole picture brighter, so replace whatever texture you think
            // is a better idea"; white before that, an olive grey first):
            // with `CUSTOMS LIGHT` the camera's exposure holds the frame's
            // brightness, so a lighter ground darkens all else. Measured
            // (the camera's kept brightness, `docs/spec/lighting.md`, "On
            // the range"): at 0.35 the views from the start, before the
            // storage and down at the ground keep the exposure at its top;
            // at 0.45 two of them do not, and white darkens every one.
            base: StandardMaterial { base_color: Color::srgb(0.35, 0.35, 0.35), perceptual_roughness: 0.95, ..default() },
            extension: lined(grid::Lines::ground()),
        })),
    ));
    // One material for each colour, with lines or without, shared by every
    // block of it: each material holds six of the graphics card's places
    // for samplers, of which DirectX 12 has 2,048 for all that is drawn
    // (`docs/findings.md`), and one for each of the range's 67 blocks and
    // parts had taken 402 of them. The lines go by where a point is, so a
    // shared material draws each block as its own did.
    // The lamps' glowing undersides (`range::hall_lamps`), lit while the
    // lamps are, white as their light (0.82.1).
    let lit = features.hall_lamps && level.is_some();
    let glow = materials.add(grid::GridMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.9, 0.9, 0.9),
            emissive: if lit { LinearRgba::rgb(2.4, 2.4, 2.4) } else { LinearRgba::BLACK },
            ..default()
        },
        extension: lined(grid::Lines::solid().none()),
    });
    let mut made: std::collections::HashMap<([u8; 4], bool), Handle<grid::GridMaterial>> = std::collections::HashMap::new();
    let mut material = |color: Color, lines: bool| {
        let key = (color.to_srgba().to_u8_array(), lines);
        made.entry(key)
            .or_insert_with(|| {
                let extension = if lines { lined(grid::Lines::solid()) } else { lined(grid::Lines::solid().none()) };
                materials.add(grid::GridMaterial { base: StandardMaterial::from_color(color), extension })
            })
            .clone()
    };
    // (A solid of no colour is not drawn: the workbench's, under its
    // model.)
    for solid in range.solids.iter().filter(|solid| solid.color != range::UNSEEN) {
        let place = Transform::from_translation(solid.centre).with_rotation(Quat::from_rotation_y(solid.yaw));
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(solid.size))), MeshMaterial3d(material(solid.color, true)), place));
    }
    // The sampler of surfaces: each square a slab a centimetre thick on the
    // ground, a little apart from the next, drawn only (`range::surface_squares`).
    for square in range::surface_squares() {
        let slab = Vec3::new(square.size.x - 0.04, 0.01, square.size.y - 0.04);
        let place = Transform::from_translation(square.centre + Vec3::Y * slab.y / 2.0).with_rotation(Quat::from_rotation_y(square.yaw));
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(slab))), MeshMaterial3d(material(square.color, false)), place));
    }
    // The hall's lamps' housings under the roof, which are no solids: their
    // undersides glowing while the lamps are on.
    let housing = material(Color::srgb(0.18, 0.18, 0.19), false);
    for at in range::hall_lamps() {
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(0.7, 0.14, 0.7))), MeshMaterial3d(housing.clone()), Transform::from_translation(at + Vec3::Y * 0.08)));
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(0.6, 0.01, 0.6))), MeshMaterial3d(glow.clone()), Transform::from_translation(at)));
    }
    // The barbed wire, which is no solid.
    let wire = material(Color::srgb(0.22, 0.20, 0.17), false);
    for (place, size) in range::barbed_wire_look() {
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(wire.clone()), place));
    }
    // What moves of the things the hands work: a door's leaf, a lid, a
    // drawer. Plain, without the grid.
    for (index, placed) in range.things.iter().enumerate() {
        let Some((place, size, color)) = placed.part_shown() else { continue };
        commands.spawn((ThingPart(index), Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(material(color, false)), place));
    }

}

/// Applies the starting view, and switches views: `F4` between playing and
/// the free camera, `F5` between playing and playing watched from outside
/// (from where the free camera stands, if it was flying; else from three
/// metres off).
fn switch_view(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    options: Res<Options>,
    rig: Option<Res<WeaponRig>>,
    controller: Res<Controller>,
    globals: Query<&GlobalTransform>,
    mut view: ResMut<View>,
    mut watching: ResMut<Watching>,
    mut camera: Query<(Entity, &mut Transform), With<MainCamera>>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    // Global transforms of the new entities exist from the second frame.
    let starting = *frame == 2;
    let wanted = if starting {
        *view
    } else if keys.just_pressed(KeyCode::F4) {
        match *view {
            View::FirstPerson | View::Watched => View::Free,
            View::Free => View::FirstPerson,
        }
    } else if keys.just_pressed(KeyCode::F5) {
        match *view {
            View::FirstPerson | View::Free => View::Watched,
            View::Watched => View::FirstPerson,
        }
    } else {
        return;
    };
    let (Some(rig), Ok((camera, mut transform))) = (rig, camera.single_mut()) else { return };
    // A camera already outside stays where it is when the keys change
    // hands; one that leaves the character's eyes is put beside it.
    let leaving_the_eyes = starting || *view == View::FirstPerson;
    match wanted {
        View::FirstPerson => {
            // The camera hangs from the rig's root; its place and rotation
            // there are set every frame with the weapon motion.
            commands.entity(rig.pivot).add_child(camera);
            *transform = Transform::default();
        }
        View::Free | View::Watched => {
            if leaving_the_eyes {
                let root = globals.get(rig.root).map_or(GlobalTransform::IDENTITY, |g| *g);
                commands.entity(camera).remove::<ChildOf>();
                *transform = outside_view(&root, &options, options.whole || (wanted == View::Watched && !starting));
            }
        }
    }
    watching.0 = (wanted == View::Watched).then(|| transform.translation - controller.shown().position);
    // Whether playing has the mouse goes by the view (`cursor::hold`).
    *view = wanted;
}

/// Where the camera stands for a view from outside, given the weapon
/// rig's root (at the ribcage, turned to the view): beside a held rifle,
/// measured from the root so that animation does not move the viewpoint;
/// for the whole figure, three metres off to the front and left of where
/// the player faces, level, looking at the middle of the body; or, for a
/// plan of the range, high over the character looking straight down, with
/// the lane's way (ahead from the start) at the top.
fn outside_view(root: &GlobalTransform, options: &Options, whole: bool) -> Transform {
    if let Some(height) = options.map {
        let under = Vec3::new(root.translation().x, 0.0, root.translation().z - 2.0);
        return Transform::from_translation(under + Vec3::Y * height).looking_at(under, Vec3::NEG_Z);
    }
    if !whole {
        let (target, from) = (Vec3::new(0.13, -0.12, -0.42), Vec3::new(-0.62, 0.03, -0.67));
        return Transform::from_translation(root.transform_point(from)).looking_at(root.transform_point(target), Vec3::Y);
    }
    let ahead = root.forward();
    let facing = Quat::from_rotation_y(f32::atan2(-ahead.x, -ahead.z));
    // The middle of the body is about half the ribcage's height up, and
    // the camera is kept over the ground for a body that lies.
    let ribcage = root.translation();
    let middle = Vec3::new(ribcage.x, ribcage.y * 0.52, ribcage.z);
    let round = Quat::from_rotation_y(options.watch_turn.to_radians());
    Transform::from_translation(middle + facing * (round * Vec3::new(-2.4, 0.35, -1.6))).looking_at(middle, Vec3::Y)
}

fn fly_camera(
    time: Res<Time>,
    view: Res<View>,
    options: Res<Options>,
    rig: Option<Res<WeaponRig>>,
    globals: Query<&GlobalTransform>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    controller: Res<Controller>,
    watching: Res<Watching>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut transform) = camera.single_mut() else { return };
    if *view == View::FirstPerson {
        return;
    }
    // An unattended capture from outside keeps its place beside the rig,
    // so that a player who has walked or crawled away is still in it.
    if let (true, Some(rig)) = (options.unattended(), &rig) {
        if let Ok(root) = globals.get(rig.root) {
            *transform = outside_view(root, &options, options.whole);
        }
        return;
    }
    // A played run watched from outside: the camera keeps its place from
    // the character's feet, turned as it was left.
    if *view == View::Watched {
        if let Some(from_feet) = watching.0 {
            transform.translation = controller.shown().position + from_feet;
        }
        return;
    }
    if !buttons.pressed(MouseButton::Right) {
        return;
    }
    let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
    yaw -= motion.delta.x * 0.003;
    pitch = (pitch - motion.delta.y * 0.003).clamp(-1.54, 1.54);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);

    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let step = transform.right() * axis(KeyCode::KeyD, KeyCode::KeyA)
        + transform.up() * axis(KeyCode::KeyE, KeyCode::KeyQ)
        + transform.forward() * axis(KeyCode::KeyW, KeyCode::KeyS);
    let speed = if keys.pressed(KeyCode::ShiftLeft) { 4.0 } else { 0.6 };
    transform.translation += step * speed * time.delta_secs();
}

/// The region `crop` (`[x, y, w, h]`, in the window's pixels) of a captured frame, written as a
/// PNG. The frame comes as tightly packed 8-bit pixels, RGBA or BGRA by the window's surface.
fn save_cropped(image: &Image, crop: [u32; 4], path: &std::path::Path) -> anyhow::Result<()> {
    use bevy::render::render_resource::TextureFormat;
    let (width, height) = (image.width(), image.height());
    let data = image.data.as_ref().ok_or_else(|| anyhow::anyhow!("the captured frame has no data"))?;
    let format = image.texture_descriptor.format;
    let bgra = match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => false,
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => true,
        other => anyhow::bail!("a {other:?} frame: not four bytes a pixel"),
    };
    anyhow::ensure!(data.len() == (width * height * 4) as usize, "a {width}x{height} {format:?} frame of {} bytes", data.len());
    let [x, y, w, h] = crop;
    let (x, y) = (x.min(width), y.min(height));
    let (w, h) = (w.min(width - x), h.min(height - y));
    anyhow::ensure!(w > 0 && h > 0, "the crop {crop:?} lies outside the {width}x{height} frame");
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let start = ((row * width + x) * 4) as usize;
        for pixel in data[start..start + (w * 4) as usize].chunks_exact(4) {
            if bgra {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            } else {
                rgba.extend_from_slice(pixel);
            }
        }
    }
    trp_data::icon::save_png(path, w, h, &rgba)
}

fn capture(
    mut commands: Commands,
    options: Res<Options>,
    controller: Res<Controller>,
    mut frame: Local<u32>,
    mut written: Local<Option<u32>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = &options.screenshot else {
        // A run that plays a script and logs how it was drawn ends with
        // the script.
        if let (Some(scenario), true) = (&options.scenario, options.unattended()) {
            if controller.drawn_time() > options.scenario_at + scenario.duration + 0.25 {
                exit.write(AppExit::Success);
            }
        }
        return;
    };
    *frame += 1;
    // With `--screenshot-frames <n>`, a picture of each of n frames from
    // that one on, each named with its frame (`<name>_<frame>.png`); the
    // run ends once the last is written.
    let count = options.screenshot_count.max(1);
    let path_of = |at: u32| -> PathBuf {
        if count == 1 {
            return path.clone();
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        path.with_file_name(format!("{stem}_{at}.png"))
    };
    let last = options.screenshot_frame + count - 1;
    // The default leaves time for textures to reach the GPU.
    if (options.screenshot_frame..=last).contains(&*frame) {
        let path = &path_of(*frame);
        // Whatever an earlier run left under this name goes first, so
        // that the file being there means this run's picture is written.
        let _ = std::fs::remove_file(path);
        match options.screenshot_crop {
            None => {
                commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
            }
            Some(crop) => {
                let path = path.clone();
                commands.spawn(Screenshot::primary_window()).observe(move |trigger: On<ScreenshotCaptured>| {
                    if let Err(e) = save_cropped(&trigger.event().image, crop, &path) {
                        warn!("the cropped picture is not written: {e:#}");
                    }
                });
            }
        }
    }
    // The picture is written some frames after it is asked for, and a
    // good many more while the renderer is still building what it draws
    // with: the run ends once the file is there.
    if *frame > last {
        let path = &path_of(last);
        match (path.exists(), *written) {
            (true, None) => *written = Some(*frame),
            (true, Some(at)) if *frame > at + 5 => {
                exit.write(AppExit::Success);
            }
            (false, _) if *frame > options.screenshot_frame + 1800 => {
                warn!("the picture was not written: {}", path.display());
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}
