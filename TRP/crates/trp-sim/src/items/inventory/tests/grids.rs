//! Tests of item grids, the room found, an item's size from its parts and folding.

use super::*;

/// A face shield is made with no hits and a seed of its own, which a
/// copy keeps with its hits; a mark is one more hit, a byte; anything
/// else has neither and takes no mark.
#[test]
fn a_face_shield_keeps_its_hits_and_seed() {
    let slots = vec![Holder { name: "mod_equipment".into(), ..Holder::default() }];
    let mut helmet = Item::new(Arc::new(Kind { slots, ..kind("helmet", "headwear", 1.0, [2, 2]) }));
    let shield = Item::new(Arc::new(Kind { face_shield: true, hinge: true, ..kind("shield", "armoredEquipment", 0.5, [2, 1]) }));
    assert_eq!(shield.hits, 0);
    let (id, seed) = (shield.id, shield.hit_seed);
    helmet.slots[0] = Some(shield);
    assert!(mark(&mut helmet, id) && mark(&mut helmet, id));
    let copy = helmet.renumbered();
    let copied = copy.slots[0].as_ref().unwrap();
    assert_eq!((copied.hits, copied.hit_seed), (2, seed));
    helmet.find_mut(id).unwrap().hits = 255;
    mark(&mut helmet, id);
    assert_eq!(helmet.find(id).unwrap().hits, 0, "the 256th mark: nought again");
    let helmet_id = helmet.id;
    assert!(!mark(&mut helmet, helmet_id), "the helmet is no face shield");
    assert_eq!((helmet.hits, helmet.hit_seed), (0, 0));
}

/// A grid gives the first room it has, rows from the top and cells
/// from the left, turns what fits no other way, and takes nothing on
/// a taken cell, outside itself or against its filter.
#[test]
fn a_grid_finds_room_row_by_row_and_turns_what_does_not_fit() {
    let mut pack = Item::new(Arc::new(Kind { grids: vec![holder("main", [4, 3], &["item"], &["case"])], ..kind("pack", "backpack", 1.0, [4, 4]) }));
    let (magazine, rifle, case) = (thing("magazine", "magazine", 0.2, [1, 2]), thing("rifle", "weapon", 3.0, [3, 1]), thing("box", "case", 1.0, [1, 1]));
    assert_eq!(pack.stow(again(&magazine)), Ok((0, Place { x: 0, y: 0, turned: false })));
    // The rifle goes beside the magazine in the top row.
    assert_eq!(pack.stow(again(&rifle)), Ok((0, Place { x: 1, y: 0, turned: false })));
    // The next magazine: the first free cell from which it fits is under the rifle.
    assert_eq!(pack.find_room(0, &magazine), Some(Place { x: 1, y: 1, turned: false }));
    pack.put(0, again(&magazine), Place { x: 1, y: 1, turned: false }).unwrap();
    // A second rifle fits flat nowhere now, and turned not either
    // (three rows, but no free column of three).
    assert_eq!(pack.find_room(0, &rifle), None);
    let turned = Place { x: 3, y: 0, turned: true };
    assert_eq!(pack.grid_takes(0, &rifle, turned), Err(Refusal::Taken));
    assert_eq!(pack.grid_takes(0, &magazine, Place { x: 3, y: 2, turned: false }), Err(Refusal::OutsideGrid));
    assert_eq!(pack.grid_takes(0, &magazine, Place { x: 2, y: 2, turned: true }), Ok(()));
    assert_eq!(pack.grid_takes(0, &case, Place { x: 0, y: 2, turned: false }), Err(Refusal::Filter));
    // Its weight is its own and all that is in it.
    assert!((pack.weight() - (1.0 + 0.2 + 3.0 + 0.2)).abs() < 1e-6);
}

/// The room a grid finds is the first place, rows from the top, cells
/// from the left, flat then turned, that `grid_room` takes, for things
/// of every size (a rifle made longer by its parts among them), in a
/// grid with gaps and things taken out, its own cells free to a thing
/// moved or not: the search's marked cells against the place-by-place
/// rule it stands for.
#[test]
fn the_room_found_is_the_first_place_the_grid_takes() {
    fn first(holder: &Item, item: &Item, own_free: bool) -> Option<Place> {
        let [columns, rows] = holder.kind.grids[0].cells;
        let [w, h] = item.cells();
        let turns: &[bool] = if w == h { &[false] } else { &[false, true] };
        turns.iter().find_map(|&turned| (0..rows).flat_map(|y| (0..columns).map(move |x| Place { x, y, turned })).find(|place| holder.grid_room(0, item, *place, own_free).is_ok()))
    }
    let mut seed = 0x2545_f491_u64;
    let mut roll = |below: u32| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 33) % below as u64) as u32
    };
    let mut pack = Item::new(Arc::new(Kind { grids: vec![holder("main", [7, 9], &["item"], &["case"])], ..kind("pack", "backpack", 1.0, [7, 9]) }));
    let slots = vec![holder("stock", [0, 0], &["mod"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 2.0, [3, 1]) }));
    rifle.slots[0] = Some(Item::new(Arc::new(Kind { extra: [0, 1, 0, 1], ..kind("stock", "mod", 0.1, [1, 1]) })));
    assert_eq!(rifle.cells(), [4, 2]);
    let case = thing("box", "case", 1.0, [1, 1]);
    let (mut found, mut none) = (0, 0);
    for _ in 0..600 {
        let item = match roll(6) {
            0 => again(&rifle),
            _ => thing("thing", "misc", 0.1, [1 + roll(4), 1 + roll(3)]),
        };
        let room = pack.find_room(0, &item);
        assert_eq!(room, first(&pack, &item, true));
        assert_eq!(pack.find_other_room(0, &item), first(&pack, &item, false));
        assert_eq!(pack.find_room(0, &case), None);
        if room.is_some() { found += 1 } else { none += 1 }
        match (roll(4), room) {
            // Taken out again, for a gap.
            (0, _) if !pack.grids[0].is_empty() => {
                let at = roll(pack.grids[0].len() as u32) as usize;
                pack.grids[0].remove(at);
            }
            // Put where it is first taken: no gap.
            (1, Some(place)) => pack.put(0, item, place).unwrap(),
            // Put anywhere it is taken: gaps.
            (_, Some(_)) => {
                let [columns, rows] = pack.kind.grids[0].cells;
                let place = Place { x: roll(columns), y: roll(rows), turned: roll(2) == 1 };
                if pack.grid_takes(0, &item, place).is_ok() {
                    pack.put(0, item, place).unwrap();
                }
            }
            _ => {}
        }
        // A thing in the grid, moved or searched for again.
        if let Some((_, lying)) = pack.grids[0].first() {
            let lying = lying.clone();
            assert_eq!(pack.find_room(0, &lying), first(&pack, &lying, true));
            assert_eq!(pack.find_other_room(0, &lying), first(&pack, &lying, false));
        }
    }
    assert!(found > 100 && none > 50, "{found} found, {none} not");
}

/// An item is as large as its parts make it: the largest extra on
/// each side, not their sum; and a stack weighs its count.
#[test]
fn parts_make_an_item_longer_by_the_largest_on_each_side() {
    let part = |name: &str, extra: [u32; 4]| Item::new(Arc::new(Kind { extra, ..kind(name, "mod", 0.1, [1, 1]) }));
    let slots = vec![holder("stock", [0, 0], &["mod"], &[]), holder("barrel", [0, 0], &["mod"], &[]), holder("sight", [0, 0], &["mod"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 2.0, [2, 1]) }));
    assert_eq!(rifle.cells(), [2, 1]);
    rifle.slots[0] = Some(part("stock", [0, 1, 0, 0]));
    rifle.slots[1] = Some(part("barrel", [2, 0, 0, 0]));
    rifle.slots[2] = Some(part("sight", [1, 0, 1, 0]));
    assert_eq!(rifle.cells(), [5, 2]);
    assert!((rifle.weight() - 2.3).abs() < 1e-6);
    let mut rounds = thing("round", "ammo", 0.012, [1, 1]);
    rounds.count = 30;
    assert!((rounds.weight() - 0.36).abs() < 1e-6);
}

/// Forced extra cells add up beside the largest of the rest
/// (`ExtraSizeForceAdd`); a thing that does not merge with its parts
/// counts none of them, whatever its places say of themselves; the
/// chamber's round never counts.
#[test]
fn forced_cells_add_up_and_only_a_merging_thing_counts_its_parts() {
    let part = |name: &str, extra: [u32; 4], forced_extra: bool| Item::new(Arc::new(Kind { extra, forced_extra, ..kind(name, "mod", 0.1, [1, 1]) }));
    let slots = vec![holder("mod_handguard", [0, 0], &["mod"], &[]), holder("mod_muzzle", [0, 0], &["mod"], &[]), holder("patron_in_weapon", [0, 0], &["mod"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots: slots.clone(), merges: true, ..kind("rifle", "weapon", 2.0, [2, 1]) }));
    rifle.slots[0] = Some(part("handguard", [1, 0, 0, 0], false));
    rifle.slots[1] = Some(part("silencer", [1, 0, 0, 0], true));
    assert_eq!(rifle.cells(), [4, 1]);
    rifle.slots[2] = Some(part("round", [3, 0, 0, 0], false));
    assert_eq!(rifle.cells(), [4, 1]);
    let mut merging_slots = slots;
    merging_slots[0].merges = Some(true);
    let mut armour = Item::new(Arc::new(Kind { slots: merging_slots, ..kind("armour", "armoredEquipment", 2.0, [3, 3]) }));
    armour.slots[0] = Some(part("plate", [1, 1, 1, 1], false));
    assert_eq!(armour.cells(), [3, 3]);
}

/// A stock folded takes its reduction off the right, of the weapon it
/// is on or of itself alone; a weapon that folds leaves its folded slot
/// out. In a grid the weapon is made its new size where it lies, and a
/// fold with no room for that is refused with nothing changed; a part
/// that blocks folding keeps the weapon from it.
#[test]
fn a_fold_shortens_the_weapon_where_it_lies() {
    // A stock is a part (`Stock` under `Mod`), which the fold's walk up
    // goes by.
    let part_of = |mut kind: Kind| {
        kind.class_names.push("Mod".into());
        kind
    };
    let stock_kind = |reduce: u32| Arc::new(Kind { extra: [0, 1, 0, 0], foldable: true, fold_reduce: reduce, ..part_of(kind("stock", "stock", 0.9, [2, 2])) });
    let slots = vec![holder("mod_stock", [0, 0], &["stock"], &[]), holder("mod_barrel", [0, 0], &["mod"], &[])];
    let mut shotgun = Item::new(Arc::new(Kind { slots: slots.clone(), merges: true, ..kind("shotgun", "weapon", 3.0, [4, 2]) }));
    shotgun.slots[0] = Some(Item::new(stock_kind(1)));
    assert_eq!(shotgun.cells(), [5, 2]);
    let mut pack = Item::new(Arc::new(Kind { grids: vec![holder("main", [5, 4], &[], &[])], ..kind("pack", "backpack", 1.0, [5, 5]) }));
    pack.put(0, shotgun.clone(), Place { x: 0, y: 0, turned: false }).unwrap();
    let (shotgun_id, stock_id) = (shotgun.id, shotgun.slots[0].as_ref().unwrap().id);
    assert_eq!(fold(&mut [&mut pack], shotgun_id), Ok((stock_id, true)));
    let folded = &pack.grids[0][0].1;
    assert_eq!((folded.cells(), folded.slots[0].as_ref().unwrap().cells()), ([4, 2], [1, 2]));
    // Something now lies where the stock would come out to: no room.
    pack.put(0, thing("box", "item", 0.1, [1, 1]), Place { x: 4, y: 1, turned: false }).unwrap();
    assert_eq!(fold(&mut [&mut pack], shotgun_id), Err(Refusal::NoRoom));
    assert!(pack.find(stock_id).unwrap().folded && pack.find(shotgun_id).unwrap().cells() == [4, 2]);
    pack.grids[0].retain(|(_, item)| item.id == shotgun_id);
    assert_eq!(fold(&mut [&mut pack], stock_id), Ok((stock_id, false)));
    assert_eq!(pack.find(shotgun_id).unwrap().cells(), [5, 2]);
    // A weapon that folds itself, its stock in its folded slot.
    let mut folding = Item::new(Arc::new(Kind { slots: slots.clone(), merges: true, foldable: true, folded_slot: "mod_stock".into(), ..kind("smg", "weapon", 2.0, [3, 2]) }));
    folding.slots[0] = Some(Item::new(Arc::new(Kind { extra: [0, 2, 0, 0], ..kind("fixed", "stock", 0.5, [2, 2]) })));
    assert_eq!(folding.cells(), [5, 2]);
    let folding_id = folding.id;
    assert_eq!(fold(&mut [&mut folding], folding_id), Ok((folding_id, true)));
    assert_eq!(folding.cells(), [3, 2]);
    // Its folded slot empty, it has nothing to fold.
    folding.slots[0] = None;
    assert!(!can_be_folded(&folding) && fold(&mut [&mut folding], folding_id) == Err(Refusal::NoAction));
    // A part that blocks folding.
    let mut blocked = Item::new(Arc::new(Kind { slots, merges: true, ..kind("shotgun", "weapon", 3.0, [4, 2]) }));
    blocked.slots[0] = Some(Item::new(stock_kind(1)));
    blocked.slots[1] = Some(Item::new(Arc::new(Kind { blocks_folding: true, ..kind("barrel", "mod", 0.5, [2, 1]) })));
    assert!(can_be_folded(&blocked) && !can_fold(&blocked));
}
