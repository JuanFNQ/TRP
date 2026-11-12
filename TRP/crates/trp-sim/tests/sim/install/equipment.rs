//! Starting equipment, the equipment panel, crates and recorded equipment lists.


/// A starting character's equipment, read from the install and put
/// together (inventory spec): every entry of the database's list finds
/// its slot or grid, everything in a grid lies where the grid's own rules
/// allow it, and the whole has a weight. Nothing here is compared with
/// the game.
#[test]
fn a_starting_character_carries_its_equipment() {
    use trp_sim::items::inventory::{Equipment, SLOTS};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let empty = Equipment::empty(&db).expect("the equipment's template");
    assert_eq!(empty.item.kind.slots.iter().map(|slot| slot.name.as_str()).collect::<std::collections::BTreeSet<_>>(), SLOTS.into_iter().collect());
    assert_eq!(empty.carried_weight(false), 0.0);
    for side in ["usec", "bear"] {
        let loadout = trp_data::character::inventory::Loadout::load(&install, "Standard", side).expect("the starting character");
        let (equipment, left_out) = Equipment::of_loadout(&db, &loadout).expect("its equipment");
        assert!(left_out.is_empty(), "{left_out:?}");
        println!("Standard/{side}: {} entries, {:.3} kg carried ({:.3} at elite Strength)", loadout.items.len(), equipment.carried_weight(false), equipment.carried_weight(true));
        for slot in SLOTS {
            let Some(item) = equipment.worn(slot) else { continue };
            let [width, height] = item.cells();
            let grids: Vec<String> = item.kind.grids.iter().zip(&item.grids).map(|(grid, held)| format!("{}x{} holding {}", grid.cells[0], grid.cells[1], held.len())).collect();
            println!("  {slot}: {} ({width}x{height} cells, {:.3} kg){}", item.kind.name, item.weight(), if grids.is_empty() { String::new() } else { format!("; grids {}", grids.join(", ")) });
            // What the game put in a grid passes the grid's own rules,
            // each thing against the others.
            for (index, held) in item.grids.iter().enumerate() {
                let mut holder = item.clone();
                holder.grids[index].clear();
                for (place, inside) in held {
                    assert_eq!(holder.put(index, inside.clone(), *place), Ok(()), "{} in {}", inside.kind.name, item.kind.name);
                }
            }
        }
        assert!(equipment.worn("FirstPrimaryWeapon").is_some_and(|rifle| rifle.cells()[0] >= 4 && rifle.weight() > 2.0));
        assert!(equipment.carried_weight(false) > 10.0 && equipment.carried_weight(true) < equipment.carried_weight(false));
        assert!(!equipment.drawn().is_empty());
    }
}

/// The inventory screen's equipment panel as the install's own scene lays
/// it out (inventory spec, "Where the views sit"): a view for each of the
/// ten slots the game's panel shows, three to a row, none over another,
/// with the two long weapons' views wider than the rest.
#[test]
fn the_equipment_panel_is_the_installs_own() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let panel = trp_data::interface::screen::GearPanel::load(&install).expect("the equipment panel");
    println!("the equipment panel, from {}: {} x {}", panel.file, panel.size[0], panel.size[1]);
    for (name, [left, top, width, height]) in &panel.slots {
        println!("  {name}: left {left} top {top}, {width} x {height}");
    }
    let rows = [["Earpiece", "Headwear", "FaceCover"], ["ArmBand", "ArmorVest", "Eyewear"]];
    let weapons = [["FirstPrimaryWeapon", "Holster"], ["SecondPrimaryWeapon", "Scabbard"]];
    let rect = |slot: &str| panel.slot(slot).unwrap_or_else(|| panic!("no view for {slot}"));
    let mut below = 0.0;
    for row in rows.iter().map(|row| row.to_vec()).chain(weapons.iter().map(|row| row.to_vec())) {
        // Left to right along the row, on one line, under the row before.
        for pair in row.windows(2) {
            let (a, b) = (rect(pair[0]), rect(pair[1]));
            assert!(a[0] + a[2] <= b[0] && a[1] == b[1], "{pair:?}");
        }
        assert!(rect(row[0])[1] >= below, "{row:?} lies over the row above");
        below = row.iter().map(|slot| rect(slot)[1] + rect(slot)[3]).fold(0.0, f32::max);
    }
    assert!(below <= panel.size[1] && rect("FaceCover")[0] + rect("FaceCover")[2] <= panel.size[0]);
    for [long, short] in weapons {
        assert!(rect(long)[2] > 2.0 * rect(short)[2], "{long} is the wide one");
    }
}

/// What the app's crate holds goes onto a bare character by the model's
/// own moves (inventory spec, "Moving things"): the stock rifle onto the
/// sling, a starting vest and backpack into their slots with all that is
/// in them, and the carried weight is then the three's. A magazine out of
/// the vest goes into the backpack, and the pockets stay where they are.
#[test]
fn a_crate_is_emptied_onto_a_bare_character() {
    use trp_data::character::inventory::Loadout;
    use trp_sim::items::inventory::{apply_item, move_item, Applied, Equipment, Item, Kind, Refusal, Rules, Spot};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let rules = Rules::of(&db);
    println!("backpacks lie in one another {} deep at most", rules.backpacks_deep);
    assert!(rules.in_raid && rules.backpacks_deep > 0);
    // The airdrop crate, the stock AKM, and a BEAR's starting vest and
    // backpack with the magazines and rounds changed for the AKM's own.
    let mut loot = Item::new(std::sync::Arc::new(Kind::of(&db, "6223349b3136504a544d1608").expect("the crate")));
    let (rifle, _) = Loadout::of_preset(&db, "59d6088586f774275f37482f").and_then(|list| Item::of_list(&db, &list)).expect("the rifle");
    assert!(!rifle.missing_vital(), "the stock rifle is whole");
    let mut kit = Loadout::load(&install, "Standard", "bear").expect("the starting character");
    let changed = kit.fit_to(&db, "59d6088586f774275f37482f").expect("the rifle's own magazine and round");
    let (mut outfit, left_out) = Equipment::of_loadout(&db, &kit).expect("its equipment");
    assert!(left_out.is_empty(), "{left_out:?}");
    // Every magazine and every round in the kit is now one the rifle takes.
    let rifle_kind = db.items.get("59d6088586f774275f37482f").expect("the rifle");
    let magazine_slot = rifle_kind.slots().into_iter().find(|slot| slot.name == "mod_magazine").expect("a magazine slot");
    let (mut magazines, mut rounds) = (0, 0);
    for item in &kit.items {
        let classes = db.class_names(&item.template);
        if classes.iter().any(|class| class == "Magazine") {
            assert!(magazine_slot.takes.contains(&item.template), "{}", classes[0]);
            magazines += 1;
        }
        if classes.iter().any(|class| class == "Ammo") {
            assert!(classes[0].contains("762x39"), "{}", classes[0]);
            rounds += item.count;
        }
    }
    println!("the kit: {} magazines and {} lots of rounds changed; {magazines} magazines the rifle takes and {rounds} rounds of its own in all", changed[0], changed[1]);
    assert!(changed[0] > 0 && changed[1] > 0);
    let (vest, pack, pockets) = (outfit.take_off("TacticalVest").expect("a vest"), outfit.take_off("Backpack").expect("a backpack"), outfit.take_off("Pockets").expect("pockets"));
    let weights = rifle.weight() + vest.weight() + pack.weight();
    let (rifle_id, vest_id, pack_id, pockets_id) = (rifle.id, vest.id, pack.id, pockets.id);
    for thing in [rifle, vest, pack] {
        let (name, cells) = (thing.kind.name.clone(), thing.cells());
        loot.stow(thing).unwrap_or_else(|_| panic!("the crate has no room for {name} ({cells:?})"));
    }
    let mut worn = Equipment::empty(&db).expect("the equipment's template").item;
    let pockets_slot = worn.slot_index("Pockets").expect("a slot for pockets");
    worn.slots[pockets_slot] = Some(pockets);
    let (worn_id, loot_id) = (worn.id, loot.id);
    let slot = |name: &str| Spot::Slot(worn.slot_index(name).expect("the slot"));
    let (sling, back, vest_slot, pack_slot, headwear) = (slot("FirstPrimaryWeapon"), slot("SecondPrimaryWeapon"), slot("TacticalVest"), slot("Backpack"), slot("Headwear"));
    let mut roots = [&mut worn, &mut loot];
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, headwear, &rules), Err(Refusal::Filter));
    assert_eq!(move_item(&mut roots, vest_id, worn_id, sling, &rules), Err(Refusal::Filter));
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, sling, &rules), Ok(()));
    assert_eq!(move_item(&mut roots, vest_id, worn_id, vest_slot, &rules), Ok(()));
    assert_eq!(move_item(&mut roots, pack_id, worn_id, pack_slot, &rules), Ok(()));
    assert!(roots[1].contents().next().is_none(), "the crate is empty");
    let carried = |roots: &[&mut Item]| ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "TacticalVest", "Backpack"].iter().filter_map(|slot| roots[0].in_slot(slot)).map(Item::weight).sum::<f32>();
    println!("carried from the crate: {:.3} kg", carried(&roots));
    assert!((carried(&roots) - weights).abs() < 1e-4);
    // From the sling to the back and again.
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, back, &rules), Ok(()));
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, sling, &rules), Ok(()));
    // Something out of the vest, dropped on the backpack, goes into it.
    let magazine = roots[0].find(vest_id).and_then(|vest| vest.grids.iter().flatten().next()).map(|(_, item)| item.id).expect("something in the vest");
    assert!(matches!(apply_item(&mut roots, magazine, pack_id, &rules), Ok(Applied::Put(Spot::Cell(..)))));
    assert_eq!(roots[0].holder_of(magazine).map(|(holder, _)| holder), Some(pack_id));
    assert!((carried(&roots) - weights).abs() < 1e-4);
    // The pockets are not taken off in a raid.
    let cell = Spot::Cell(0, trp_sim::items::inventory::Place { x: 0, y: 0, turned: false });
    assert_eq!(move_item(&mut roots, pockets_id, loot_id, cell, &rules), Err(Refusal::KeptOn("Pockets".into())));
}

/// The moves nobody drags (inventory spec, "Clicks, keys and windows"),
/// on the install's own items: the crate's three things are put on by
/// their kind, a magazine is sent to the crate and back to the vest, a
/// pick-up finds the place the game's order has for the thing, and a
/// quick-use key takes medicine in the pockets and nothing in a backpack.
#[test]
fn things_are_sent_put_on_and_bound_by_the_install_s_classes() {
    use trp_data::character::inventory::Loadout;
    use trp_sim::items::inventory::{bindable, can_pick_up, equip, is, loot_order, move_item, pick_up, quick_find, take_out, Applied, Bound, Equipment, Item, Kind, Order, Refusal, Rules, Spot};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let rules = Rules::of(&db);
    let mut loot = Item::new(std::sync::Arc::new(Kind::of(&db, "6223349b3136504a544d1608").expect("the crate")));
    let (rifle, _) = Loadout::of_preset(&db, "59d6088586f774275f37482f").and_then(|list| Item::of_list(&db, &list)).expect("the rifle");
    let mut kit = Loadout::load(&install, "Standard", "bear").expect("the starting character");
    kit.fit_to(&db, "59d6088586f774275f37482f").expect("the rifle's own magazine and round");
    let (mut outfit, _) = Equipment::of_loadout(&db, &kit).expect("its equipment");
    let (vest, pack, pockets) = (outfit.take_off("TacticalVest").expect("a vest"), outfit.take_off("Backpack").expect("a backpack"), outfit.take_off("Pockets").expect("pockets"));
    let (rifle_id, vest_id, pack_id, pockets_id) = (rifle.id, vest.id, pack.id, pockets.id);
    for thing in [rifle, vest, pack] {
        loot.stow(thing).expect("room in the crate");
    }
    let mut worn = Equipment::empty(&db).expect("the equipment's template").item;
    let pockets_slot = worn.slot_index("Pockets").expect("a slot for pockets");
    worn.slots[pockets_slot] = Some(pockets);
    let (worn_id, loot_id) = (worn.id, loot.id);
    let slot = |name: &str| worn.slot_index(name).expect("the slot");
    let (sling, back, vest_slot, pack_slot) = (slot("FirstPrimaryWeapon"), slot("SecondPrimaryWeapon"), slot("TacticalVest"), slot("Backpack"));
    let mut roots = [&mut worn, &mut loot];
    // Ctrl and a click on the rifle with nothing worn to hold it: the
    // character has no place, so it goes to the next room in the crate.
    let sent = quick_find(&mut roots, rifle_id, &[worn_id], Order::OTHER_SIDE, &rules);
    println!("the rifle sent from the crate to a bare character: {sent:?}");
    assert!(matches!(sent, Ok((into, Applied::Put(Spot::Cell(..)))) if into == loot_id));
    // Alt and a click: each onto the slot of its kind.
    assert_eq!(equip(&mut roots, rifle_id, &rules), Ok(sling));
    assert_eq!(equip(&mut roots, vest_id, &rules), Ok(vest_slot));
    assert_eq!(equip(&mut roots, pack_id, &rules), Ok(pack_slot));
    assert_eq!(equip(&mut roots, pack_id, &rules), Err(Refusal::AlreadyThere));
    assert!(roots[1].contents().next().is_none(), "the crate is empty");
    // A second rifle goes onto the back, a third nowhere.
    let another = || Loadout::of_preset(&db, "59d6088586f774275f37482f").and_then(|list| Item::of_list(&db, &list)).expect("the rifle").0;
    let (second, third) = (another(), another());
    let (second_id, third_id) = (second.id, third.id);
    roots[1].stow(second).unwrap();
    roots[1].stow(third).unwrap();
    assert_eq!(equip(&mut roots, second_id, &rules), Ok(back));
    assert_eq!(equip(&mut roots, third_id, &rules), Err(Refusal::NoRoom));
    // What the kit holds, by the game's classes.
    let all = |roots: &[&mut Item], holder: Id_| roots[0].find(holder).map(|holder| holder.grids.iter().flatten().map(|(_, item)| item.id).collect::<Vec<_>>()).unwrap_or_default();
    let of = |roots: &[&mut Item], class: &str| {
        let among = [vest_id, pack_id, pockets_id].into_iter().flat_map(|holder| all(roots, holder));
        among.into_iter().find(|id| roots[0].find(*id).is_some_and(|item| is(item, class)))
    };
    let magazine = of(&roots, "Magazine").expect("a magazine in the kit");
    let rounds = of(&roots, "Ammo").expect("rounds in the kit");
    let kit_id = of(&roots, "Meds").expect("medicine in the kit");
    let food = of(&roots, "FoodDrink").expect("food or drink in the kit");
    for id in [magazine, rounds, kit_id, food] {
        let item = roots[0].find(id).unwrap();
        let lies = roots[0].holder_of(id).and_then(|(holder, _)| roots[0].find(holder)).map(|holder| holder.kind.name.clone()).unwrap_or_default();
        println!("  {} (in {lies}) is looked for room in {:?}", item.kind.name, loot_order(item));
    }
    // Wherever the kit had them: the magazine in the vest and the
    // medicine in the backpack, to begin with.
    for (id, holder) in [(magazine, vest_id), (kit_id, pack_id)] {
        if roots[0].holder_of(id).map(|(holder, _)| holder) != Some(holder) {
            let thing = roots[0].find(id).unwrap();
            let room = (0..roots[0].find(holder).unwrap().kind.grids.len()).find_map(|grid| roots[0].find(holder)?.find_room(grid, thing).map(|place| Spot::Cell(grid, place)));
            assert_eq!(move_item(&mut roots, id, holder, room.expect("room for it"), &rules), Ok(()));
        }
    }
    assert_eq!(loot_order(roots[0].find(magazine).unwrap())[0], "TacticalVest");
    assert_eq!(loot_order(roots[0].find(kit_id).unwrap())[0], "Backpack");
    // The magazine to the crate (the crate is looked in first while it is
    // open) and back: to the vest, which is where magazines go.
    let sent = quick_find(&mut roots, magazine, &[loot_id, worn_id], Order::OTHER_SIDE, &rules);
    assert!(matches!(sent, Ok((into, Applied::Put(_))) if into == loot_id), "{sent:?}");
    let back_again = quick_find(&mut roots, magazine, &[worn_id], Order::OTHER_SIDE, &rules);
    assert!(matches!(back_again, Ok((into, Applied::Put(_))) if into == vest_id), "{back_again:?}");
    // The rounds in the backpack, sent with nothing open: the vest is
    // looked in before the backpack they lie in.
    let sent = quick_find(&mut roots, rounds, &[worn_id], Order::OTHER_SIDE, &rules);
    println!("rounds sent with nothing open: {sent:?}");
    assert!(sent.is_ok());
    // Put down and taken up again: a magazine goes back to the vest.
    let dropped = take_out(&mut roots, magazine, &rules).expect("a magazine is put down");
    assert!(can_pick_up(roots[0], &dropped, &rules));
    let (taken, left) = pick_up(roots[0], dropped, &rules);
    assert!(matches!(taken, Ok((into, Applied::Put(_))) if into == vest_id) && left.is_none(), "{taken:?}");
    // The quick-use keys: medicine in the backpack is not bound; in the
    // pockets it is, and a magazine never.
    let mut bound = Bound::default();
    assert!(!bindable(roots[0], kit_id) && !bound.bind(roots[0], kit_id, 0));
    let pocket = (0..4).find_map(|grid| roots[0].find(pockets_id)?.find_room(grid, roots[0].find(kit_id)?).map(|place| Spot::Cell(grid, place))).expect("a free pocket");
    assert_eq!(move_item(&mut roots, kit_id, pockets_id, pocket, &rules), Ok(()));
    assert!(bound.bind(roots[0], kit_id, 0) && !bound.bind(roots[0], magazine, 1));
    // The rifle on the sling is not bound to a key of these: its slot has
    // a key of its own.
    assert!(!bindable(roots[0], rifle_id));
    assert_eq!(bound.key_of(kit_id), Some(0));
}

type Id_ = trp_sim::items::inventory::Id;

/// The equipment list a recording's header carries from recorder 0.5.0 on
/// is read back and set beside the model (inventory spec, "Compared with
/// the running game"). No recording has one yet: the list here is made up
/// from a starting character in the recorder's own form, with the model's
/// figures where the game's would stand, so that it agrees with itself;
/// one figure is then spoilt, which has to show.
#[test]
fn a_recorded_equipment_list_is_read_back() {
    use trp_data::character::inventory::Loadout;
    use trp_sim::items::inventory::Equipment;
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let loadout = Loadout::load(&install, "Standard", "usec").expect("the starting character");
    let (equipment, _) = Equipment::of_loadout(&db, &loadout).expect("its equipment");
    // The recorder lists the equipment first and then what is in it, each
    // with what it is in, where, and the game's cells and weight for it.
    let mut entries: Vec<serde_json::Value> = Vec::new();
    let mut waiting = vec![(&equipment.item, None::<(String, String, Option<trp_sim::items::inventory::Place>)>)];
    while let Some((item, lies)) = waiting.pop() {
        let id = format!("item{}", item.id);
        let mut entry = serde_json::json!({ "id": id, "tpl": item.kind.template, "n": item.count, "w": item.cells()[0], "h": item.cells()[1], "kg": item.weight() });
        if let Some((holder, at, place)) = lies {
            entry["in"] = holder.into();
            entry["at"] = at.into();
            if let Some(place) = place {
                (entry["x"], entry["y"], entry["r"]) = (place.x.into(), place.y.into(), (place.turned as u32).into());
            }
        }
        entries.push(entry);
        for (slot, held) in item.kind.slots.iter().zip(&item.slots) {
            waiting.extend(held.as_ref().map(|held| (held, Some((id.clone(), slot.name.clone(), None)))));
        }
        for (grid, held) in item.kind.grids.iter().zip(&item.grids) {
            waiting.extend(held.iter().map(|(place, held)| (held, Some((id.clone(), grid.name.clone(), Some(*place))))));
        }
        waiting.extend(item.loose.iter().map(|held| (held, Some((id.clone(), "cartridges".to_owned(), None)))));
    }
    let list = serde_json::Value::Array(entries.clone());
    let (read, compared, left_out) = Equipment::of_recording(&db, &list).expect("the list");
    println!("{} entries read back, {} items compared, {:.3} kg carried", entries.len(), compared.len(), read.carried_weight(false));
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(compared.len(), entries.len() - 1);
    assert!(compared.iter().all(|item| item.agrees()));
    assert!((read.carried_weight(false) - equipment.carried_weight(false)).abs() < 1e-4);
    // A weight the game would have summed otherwise is found.
    entries[1]["kg"] = (entries[1]["kg"].as_f64().unwrap() + 0.25).into();
    let (_, compared, _) = Equipment::of_recording(&db, &serde_json::Value::Array(entries)).expect("the list");
    assert_eq!(compared.iter().filter(|item| !item.agrees()).count(), 1);
}

/// The sizes the game gave everything the character carried in a
/// recording with the Benelli M3's stock folded (`courses1`, the
/// Five-seveN's course: the M3 on the back 4 cells by 2, its
/// stock 1 by 2), set beside the model's with that stock folded
/// (inventory spec, "Grids"). The recording says nothing of a fold (the
/// recorder writes it from 0.10.2): only the stock's own size shows it,
/// and the weapon's is what this measures. Unfolded, the model has the
/// two at 5 and 2.
#[test]
fn a_folded_stock_sizes_its_weapon_as_the_game_did() {
    use trp_sim::items::inventory::Equipment;
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses1/4-five-seven/five-seven.jsonl");
    let (Ok(_install), Ok(text)) = (crate::fixture::install().ok_or(()), std::fs::read_to_string(path)) else {
        eprintln!("skipped: no install or no recording");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let header: serde_json::Value = serde_json::from_str(text.lines().next().expect("a header")).expect("the header");
    let mut list = header["equipment"].clone();
    const STOCK: &str = "6259c3387d6aab70bc23a18d";
    let off = |list: &serde_json::Value| {
        let (_, compared, left_out) = Equipment::of_recording(&db, list).expect("the list");
        assert!(left_out.is_empty(), "{left_out:?}");
        compared.into_iter().filter(|item| !item.agrees()).map(|item| (item.name, item.cells)).collect::<Vec<_>>()
    };
    let unfolded = off(&list);
    assert_eq!(unfolded.len(), 2, "{unfolded:?}");
    assert!(unfolded.iter().all(|(_, cells)| cells[1][0] == cells[0][0] + 1));
    for entry in list.as_array_mut().expect("a list").iter_mut().filter(|entry| entry["tpl"] == STOCK) {
        entry["fold"] = 1.into();
    }
    assert_eq!(off(&list), Vec::new());
}
