//! The range's weapon crates, gear cabinets, repair boxes and containers filled.

use super::*;

pub(super) fn weapon_crates(db: &trp_data::Database, hidden: &std::collections::HashSet<String>) -> anyhow::Result<Crates> {
    use trp_sim::items::inventory::{Item, Kind};
    /// Deeper than any needs, before each is cut to what it holds.
    const DEEPEST: u32 = 2000;
    let deep = |template: &str| -> anyhow::Result<Kind> {
        let mut kind = Kind::of(db, template)?;
        kind.grids.first_mut().ok_or_else(|| anyhow::anyhow!("{template} has no grid"))?.cells = [CRATE_CELLS[0], DEEPEST];
        Ok(kind)
    };
    let (case_kind, box_kind) = (deep(GUN_CASE)?, deep(MAGAZINE_BOX)?);
    let (case_deep, box_deep) = (std::sync::Arc::new(case_kind.clone()), std::sync::Arc::new(box_kind.clone()));
    let mut cases: Vec<Item> = world::range::WEAPON_CRATES.iter().map(|_| Item::new(case_deep.clone())).collect();
    let mut boxes: Vec<Item> = world::range::WEAPON_CRATES.iter().map(|_| Item::new(box_deep.clone())).collect();
    let mut ammo: Vec<Item> = (0..world::range::AMMO_TABLES).map(|_| Item::new(box_deep.clone())).collect();
    let mut on_table: Vec<Vec<&String>> = vec![Vec::new(); world::range::WEAPON_CRATES.len()];
    let mut weapons: Vec<(&String, &String)> = db
        .items
        .values()
        .filter(|item| (item.name.starts_with("weapon_") && db.is_a(item, "Weapon")) || db.custom.contains_key(&item.id))
        .filter(|item| logic_built(db, &item.id) && db.default_preset(&item.id).is_ok() && !hidden.contains(&item.id))
        .map(|item| (&item.name, &item.id))
        .collect();
    weapons.sort();
    let mut counts = [0; world::range::WEAPON_CRATES.len()];
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
                        eprintln!("the {} weapon case has no room for {name}", world::range::WEAPON_CRATES[at]);
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
    for (at, weapons) in on_table.iter().enumerate().take(world::range::AMMO_TABLES) {
        let (magazines, stacks) = table_loads(db, weapons, hidden);
        let label = world::range::WEAPON_CRATES[at];
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
    // the install in its case, one of every food and drink in its box; quest
    // items left out.
    if let Some(at) = world::range::WEAPON_CRATES.iter().position(|label| *label == world::range::MEDICAL) {
        let quest = |item: &trp_data::setup::database::ItemTemplate| item.props.get("QuestItem").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let mut supplies: Vec<(&String, &String, bool)> = db
            .items
            .values()
            .filter(|item| item.prefab_key().is_some() && !quest(item) && !hidden.contains(&item.id))
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
    // handheld rangefinder with them (rangefinder spec).
    if let Some(at) = world::range::WEAPON_CRATES.iter().position(|label| *label == world::range::GRENADES) {
        let quest = |item: &trp_data::setup::database::ItemTemplate| item.props.get("QuestItem").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let mut kept: Vec<(&String, &String, bool)> = db
            .items
            .values()
            .filter(|item| item.prefab_key().is_some() && !quest(item) && !hidden.contains(&item.id))
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
    for (((case, held), label), count) in cases.iter_mut().zip(boxes.iter_mut()).zip(world::range::WEAPON_CRATES).zip(counts) {
        let (case_rows, box_rows) = (cut(case, &case_kind), cut(held, &box_kind));
        if label == world::range::MEDICAL {
            println!("the table of {label}: {count} medical items, food and drink, in its case ({case_rows} rows) and its box ({box_rows} rows)");
        } else if label == world::range::GRENADES {
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

/// What each place of the storage holds, in `range::CABINETS`' order (rigs,
/// backpacks, helmets, armour, masks and hats, headsets, plates and
/// eyewear): one of every item of the install of its sort, as
/// its stock preset has it where it has one (an armour or a helmet with its
/// inserts), else bare but for the built-in inserts its locked slots name
/// (`Item::fill_locked`: six helmets have no stock build); quest items, those with no model, the suits of
/// one's own (the CLOTHING crate's) and those hidden as unfinished
/// (`unfinished_items`) left out. Rigs are the vests without
/// armour's slots; armour the body armour and the vests with them; helmets
/// the headwear with them, and the helmets' own parts (the face shields,
/// and with them the mandibles, ear covers, side armour and appliqués, armoured
/// equipment of no other sort, which no storage took); masks and hats the
/// face covers and the rest of the headwear; headsets the headphones;
/// plates the armour plates (`ArmorPlate`, but its `BuiltInInserts`: the
/// soft inserts, which come out of no armour in the game);
/// eyewear the glasses and goggles (`Visors`). Armour's slots are an
/// insert's or a plate's: any but an attachment's (`mod_`); or an armour
/// class of its own. Each is the drawer's container (the plates' the
/// magazine boxes' own) ten cells across, as deep as what it holds (the
/// app's, as the tables').
pub(super) fn gear_cabinets(db: &trp_data::Database, hidden: &std::collections::HashSet<String>) -> anyhow::Result<Vec<trp_sim::items::inventory::Item>> {
    use serde_json::Value as Json;
    use trp_data::setup::database::ItemTemplate;
    use trp_sim::items::inventory::{Item, Kind};
    const DEEPEST: u32 = 2000;
    let deep = |template: &str| -> anyhow::Result<Kind> {
        let mut kind = Kind::of(db, template)?;
        kind.grids.first_mut().ok_or_else(|| anyhow::anyhow!("{template} has no grid"))?.cells = [CRATE_CELLS[0], DEEPEST];
        Ok(kind)
    };
    let kinds: Vec<Kind> = world::range::CABINETS.iter().map(|label| deep(if *label == world::range::PLATES { MAGAZINE_BOX } else { CABINET })).collect::<anyhow::Result<_>>()?;
    let mut cabinets: Vec<Item> = kinds.iter().map(|kind| Item::new(std::sync::Arc::new(kind.clone()))).collect();
    let quest = |item: &ItemTemplate| item.props.get("QuestItem").and_then(Json::as_bool).unwrap_or(false);
    let sort = |item: &ItemTemplate| cabinet_of(db, item);
    let mut gear: Vec<(&String, &String, usize)> = db
        .items
        .values()
        .filter(|item| item.prefab_key().is_some() && !quest(item) && !db.outfits.contains_key(&item.id) && !db.custom.contains_key(&item.id) && !hidden.contains(&item.id))
        .filter_map(|item| Some((&item.name, &item.id, sort(item)?)))
        .collect();
    gear.sort();
    let mut counts = [0; world::range::CABINETS.len()];
    for (name, template, into) in gear {
        let made = if db.default_preset(template).is_ok() {
            trp_data::character::inventory::Loadout::of_preset(db, template).and_then(|list| Item::of_list(db, &list)).map(|(item, _)| item)
        } else {
            // Bare, but for the built-in inserts the game puts in
            // (`Item::fill_locked`).
            Kind::of(db, template).map(|kind| Item::new(std::sync::Arc::new(kind))).and_then(|mut item| item.fill_locked(db).map(|()| item))
        };
        match made {
            Ok(item) => {
                if cabinets[into].stow(item).is_err() {
                    eprintln!("the {} storage has no room for {name}", world::range::CABINETS[into]);
                }
                counts[into] += 1;
            }
            Err(e) => eprintln!("the {} storage has no {name}: {e:#}", world::range::CABINETS[into]),
        }
    }
    // Each cut to the rows it fills.
    for ((cabinet, label), count) in cabinets.iter_mut().zip(world::range::CABINETS).zip(counts) {
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
pub(super) fn cabinet_of(db: &trp_data::Database, item: &trp_data::setup::database::ItemTemplate) -> Option<usize> {
    use serde_json::Value as Json;
    let armoured = || {
        let class = item.props.get("armorClass").and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(0.0);
        let slots = item.props.get("Slots").and_then(Json::as_array).into_iter().flatten().filter_map(|slot| slot.get("_name").and_then(Json::as_str));
        class > 0.0 || slots.into_iter().any(|name| !name.starts_with("mod_"))
    };
    let at = |label: &str| world::range::CABINETS.iter().position(|l| *l == label);
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
        at(world::range::PLATES)
    } else if db.is_a(item, "Visors") {
        at(world::range::EYEWEAR)
    } else if db.is_a(item, "ArmoredEquipment") && !db.is_a(item, "ArmorPlate") && !db.is_a(item, "BuiltInInserts") {
        at("HELMETS")
    } else {
        None
    }
}

/// A container of the range (a gun case, a cabinet's drawer) cut to the
/// rows of its first grid that what it holds fills, at least one: the
/// rows.
fn cut_to_rows(container: &mut trp_sim::items::inventory::Item) -> u32 {
    let rows = container.grids[0].iter().map(|(place, thing)| place.y + if place.turned { thing.cells()[0] } else { thing.cells()[1] }).max().unwrap_or(0).max(1);
    let mut cut = (*container.kind).clone();
    cut.grids[0].cells[1] = rows;
    container.kind = std::sync::Arc::new(cut);
    rows
}

/// A thing put into a container of the range already cut to what it held
/// (`cut_to_rows`): the container made deep for it, then cut again. Where
/// it went, or why it could not.
pub(super) fn stow_more(container: &mut trp_sim::items::inventory::Item, thing: trp_sim::items::inventory::Item) -> Result<(), trp_sim::items::inventory::Refusal> {
    let mut deep = (*container.kind).clone();
    deep.grids[0].cells[1] = 2000;
    container.kind = std::sync::Arc::new(deep);
    let stowed = container.stow(thing).map(|_| ());
    cut_to_rows(container);
    stowed
}

/// The install's repair kits (`RepairKits`): the body armour's and the
/// weapons' (`item_spec_armorrepair`, `item_spec_weaprepair`), in the order
/// of `range::REPAIR_BOXES`, and how many of each a box holds (several
/// copies; the app's number).
const REPAIR_KITS: [&str; 2] = ["591094e086f7747caa7bb2ef", "5910968f86f77425cf569c32"];

const REPAIR_KITS_A_BOX: usize = 6;

/// The workbench's two boxes of repair kits (`range::REPAIR_BOXES`): the
/// game's ammunition box made deep, filled with its kits, and cut to what
/// it holds; a kit the install lacks is left out with a word.
pub(super) fn repair_boxes(db: &trp_data::Database) -> Vec<trp_sim::items::inventory::Item> {
    use trp_sim::items::inventory::{Item, Kind};
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
/// by hand, in the order the range keeps them
/// (`range::Placed::holds`, after the airdrop crate): a wooden crate, a
/// technical supply crate, a dead Scav and a drawer. They start empty, but
/// with `SEARCHING` on (`fill_crates`).
pub(super) const CONTAINERS: [&str; 4] = ["578f87ad245977356274f2cc", "5d6fd45b86f774317075ed43", "5909e4b686f7747f5b744fa4", "578f87b7245977356274f2cd"];

/// Four more past the car, the containers of `courses10`'s searching
/// takes, in the order the range keeps them (`range::MORE_HOLDS`): the
/// weapon box (6 by 3), the toolbox (4 by 3), the safe (3 by 3) and the
/// airdrop weapon crate (10 by 12). Filled and hidden as those four are.
pub(super) const MORE_CONTAINERS: [&str; 4] = ["5909d76c86f77471e53d2adf", "5909d50c86f774659e6aaebe", "578f8782245977354405a1e3", "6223351bb5d97a7b2c635ca7"];

/// The table that fills a container no location's static loot names: the
/// airdrop weapon crate (SPT fills an airdrop by its `airdrop.json`, a
/// random pick of presets and things, `LootGenerator.CreateRandomLoot`,
/// which the app does not make) by the weapon box's (`PROVISIONAL`: the
/// nearest weapon container of Customs' tables, its search sound the same).
const STANDS_IN: [(&str, &str); 1] = [("6223351bb5d97a7b2c635ca7", "5909d76c86f77471e53d2adf")];

/// Whose tables of a container's loot fill those four (inventory spec,
/// "The range's crates"): Customs' (SPT's `bigmap`).
const LOOT_LOCATION: &str = "bigmap";

/// The seasonal events' things among those four's tables, left out while no
/// event is on (SPT's `SeasonalEventService.GetInactiveSeasonalEventItems`,
/// its Christmas and Halloween lists, which name them: the Jason mask, the
/// jar of pickles, the special 40-degree fuel, the fox and Grinch masks,
/// matched to the install's English names). `PROVISIONAL`: no event taken
/// as on.
const SEASONAL: [&str; 5] = ["5bd071d786f7747e707b93a3", "67586af7036d7f3da60c3612", "67586bee39b1b82b0d0f9d06", "676029f997f19b78f00a0197", "67602a0f97f19b78f00a0199"];

/// The range's eight crates as a raid's containers (`SEARCHING`; inventory
/// spec, "The range's crates"): each filled by its template's table of
/// Customs' (the app's own fill by SPT's figures; the airdrop crate by
/// `STANDS_IN`'s), then hidden, to be searched. The armory's cases,
/// boxes and cabinets are known.
pub(super) fn fill_crates(install: &Install, db: &Database, away: &mut [Option<trp_sim::items::inventory::Item>], searcher: &mut trp_sim::items::search::Searcher) {
    let tables = trp_data::level::loot::static_loot(install, LOOT_LOCATION).unwrap_or_else(|e| {
        eprintln!("searching: the loot tables could not be read, so the crates are empty: {e:#}");
        Default::default()
    });
    let said = |what: &str, e: anyhow::Error| eprintln!("searching: {what} could not be read, and is left as none: {e:#}");
    let mut left_out: std::collections::HashSet<String> = trp_data::level::loot::lootable_blacklist(install).unwrap_or_else(|e| {
        said("the lootable blacklist", e);
        Vec::new()
    }).into_iter().collect();
    left_out.extend(SEASONAL.iter().map(|template| (*template).to_owned()));
    let mut rules = trp_sim::items::search::FillRules {
        left_out,
        limits: trp_data::level::loot::spawn_limits(install, LOOT_LOCATION).unwrap_or_else(|e| {
            said("the location's limits", e);
            Default::default()
        }),
        counted: Default::default(),
        multiplier: trp_data::level::loot::static_multiplier(install, LOOT_LOCATION).unwrap_or(1.0),
        fit_attempts: trp_data::level::loot::fit_attempts(install).unwrap_or(3),
    };
    let searched = |at: &usize| (1..=CONTAINERS.len()).contains(at) || (world::range::MORE_HOLDS..world::range::MORE_HOLDS + MORE_CONTAINERS.len()).contains(at);
    for (_, container) in away.iter_mut().enumerate().filter(|(at, _)| searched(at)) {
        let Some(container) = container else { continue };
        let template = container.kind.template.clone();
        let stand_in = STANDS_IN.iter().find(|(of, _)| *of == template).map(|(_, by)| *by);
        let own = tables.get(&template);
        if let Some(table) = own.or_else(|| stand_in.and_then(|by| tables.get(by))) {
            let put = trp_sim::items::search::fill(db, table, container, &mut rules, searcher.dice());
            println!("searching: the {} filled with {} things{}", container.kind.name, put.len(), if own.is_some() { "" } else { " (by the stand-in's table, STANDS_IN)" });
        }
        searcher.hide(container);
    }
}

/// The things hidden as unfinished with `HIDE UNFINISHED ITEMS` on: those
/// SPT never hands out (`item.json`'s `blacklist`:
/// `trp_data::loot::item_blacklist`), which the range's tables and cabinets
/// lay out none of and the benches' lists leave out; none with it off, or
/// where the list cannot be read. The crates keep SPT's own fill.
pub(super) fn unfinished_items(install: &Install, hide: bool) -> std::collections::HashSet<String> {
    if !hide {
        return Default::default();
    }
    match trp_data::level::loot::item_blacklist(install) {
        Ok(list) => {
            println!("unfinished items: {} hidden from the range and the benches (HIDE UNFINISHED ITEMS: SPT's item blacklist)", list.len());
            list.into_iter().collect()
        }
        Err(e) => {
            eprintln!("unfinished items: SPT's item blacklist could not be read, so none is hidden: {e:#}");
            Default::default()
        }
    }
}

/// Things lying about beyond the doors, to be taken up from a table, the
/// floor and a shelf (where `range::lying` has them): the rifle's own
/// magazine (empty) and a stack of its rounds.
pub(super) const LYING: [(&str, u32); 3] = [("59d625f086f774661516605d", 1), ("64b7af5a8532cf95ee0a0dbd", 30), ("59d625f086f774661516605d", 1)];

/// The window the app starts in, in pixels. The options tab may change
/// it for someone at the keys; a run nobody is at the keys of keeps it,
/// since its scripted clicks go by places in a window of this size.
pub const WINDOW_SIZE: [u32; 2] = [1600, 900];

/// Stock BEAR arms.
pub(super) const DEFAULT_ARMS: &str = "assets/content/hands/bear/bear_hands_skin.bundle";

/// The clothing that goes with those arms by default: the same faction's
/// own upper body and legs (`DefaultBearBody` and `DefaultBearFeet` in the
/// install's customization list).
pub(super) const DEFAULT_BODY: &str = "assets/content/characters/character/prefabs/tshirt_bear_turtleneck.bundle";

pub(super) const DEFAULT_LEGS: &str = "assets/content/characters/character/prefabs/pants_bear_srvv.bundle";

/// The first of the same faction's heads in the install's customization
/// list. Like the upper body it is a shadow and nothing more in first
/// person.
pub(super) const DEFAULT_HEAD: &str = "assets/content/characters/character/prefabs/bear_head_0.bundle";
