//! Tests of equipment slots, moving, stacking, splitting, putting on and binding things.

use super::*;

/// An equipment slot takes what its filter names, one at a time; a
/// helmet that blocks the earpiece keeps a headset out, and is kept
/// out by one already worn. The carried weight is every slot's, and
/// without the weapons' at elite Strength.
#[test]
fn equipment_slots_filter_block_and_weigh() {
    let slots = vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[]), holder("Headwear", [0, 0], &["headwear"], &[]), holder("Earpiece", [0, 0], &["headset"], &[])];
    let mut equipment = Equipment { item: Item::new(Arc::new(Kind { slots, ..kind("equipment", "inventory", 0.0, [1, 1]) })) };
    let rifle = thing("rifle", "weapon", 3.5, [5, 2]);
    let headset = thing("comtac", "headset", 0.6, [2, 2]);
    let cap = thing("cap", "headwear", 0.1, [1, 1]);
    let helmet = Item::new(Arc::new(Kind { blocks: vec!["Earpiece".into()], ..kind("helmet", "headwear", 1.5, [2, 2]) }));
    assert_eq!(equipment.can_wear("Headwear", &rifle), Err(Refusal::Filter));
    assert_eq!(equipment.can_wear("Backpack", &rifle), Err(Refusal::NoSuchPlace));
    equipment.wear("FirstPrimaryWeapon", rifle.clone()).unwrap();
    assert_eq!(equipment.can_wear("FirstPrimaryWeapon", &rifle), Err(Refusal::Taken));
    equipment.wear("Earpiece", headset.clone()).unwrap();
    assert_eq!(equipment.can_wear("Headwear", &helmet), Err(Refusal::WouldBlock("Earpiece".into())));
    equipment.wear("Headwear", cap).unwrap();
    assert!((equipment.carried_weight(false) - 4.2).abs() < 1e-6);
    assert!((equipment.carried_weight(true) - 0.7).abs() < 1e-6);
    // The other way round: the helmet on, the headset is kept out.
    equipment.take_off("Headwear");
    assert_eq!(equipment.take_off("Earpiece"), Some(headset.clone()));
    equipment.wear("Headwear", helmet).unwrap();
    assert_eq!(equipment.can_wear("Earpiece", &headset), Err(Refusal::Blocked("Headwear".into())));
    assert_eq!(equipment.drawn(), ["helmet"]);
}

pub(super) const RAID: Rules = Rules { in_raid: true, backpacks_deep: 2, busy_weapon: None, chamber_closed: None, chamber_held: None, limits: None, own: None };

pub(super) fn pack(name: &str, cells: [u32; 2]) -> Item {
    Item::new(Arc::new(Kind { grids: vec![holder("main", cells, &["item"], &[])], classes: vec![name.into(), BACKPACK.into(), "item".into()], ..kind(name, "backpack", 1.0, [3, 3]) }))
}

/// A thing is moved from a crate onto the character and about in its
/// containers by the rules of the place it goes to; a move that is
/// refused leaves everything where it was.
#[test]
fn things_move_between_a_crate_and_the_character() {
    let slots = vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[]), holder("Backpack", [0, 0], &[BACKPACK], &[]), holder("Dogtag", [0, 0], &["item"], &[])];
    let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
    let mut crate_ = Item::new(Arc::new(Kind { grids: vec![holder("main", [5, 5], &["item"], &[])], ..kind("crate", "container", 1.0, [1, 1]) }));
    let (rifle, bag, magazine) = (thing("rifle", "weapon", 3.5, [5, 2]), pack("bag", [4, 4]), thing("magazine", "magazine", 0.3, [1, 2]));
    let (rifle_id, bag_id, magazine_id) = (rifle.id, bag.id, magazine.id);
    for item in [rifle, bag, magazine] {
        crate_.stow(item).unwrap();
    }
    let (worn_id, crate_id) = (worn.id, crate_.id);
    let before = (worn.clone(), crate_.clone());
    let mut roots = [&mut worn, &mut crate_];
    // The rifle is no backpack, and the dogtag's slot takes nothing.
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, Spot::Slot(1), &RAID), Err(Refusal::Filter));
    assert_eq!(move_item(&mut roots, magazine_id, worn_id, Spot::Slot(2), &RAID), Err(Refusal::Dogtag));
    // Where it lies already, and onto a taken cell.
    let (_, lies) = roots[1].holder_of(magazine_id).unwrap();
    assert_eq!(move_item(&mut roots, magazine_id, crate_id, lies, &RAID), Err(Refusal::AlreadyThere));
    assert_eq!(move_item(&mut roots, magazine_id, crate_id, Spot::Cell(0, Place { x: 0, y: 0, turned: false }), &RAID), Err(Refusal::Taken));
    assert_eq!(move_item(&mut roots, bag_id, bag_id, Spot::Cell(0, Place { x: 0, y: 0, turned: false }), &RAID), Err(Refusal::Cycle));
    assert!(*roots[0] == before.0 && *roots[1] == before.1, "a refused move changed something");
    // Onto the sling, onto the back, and the magazine into the bag worn.
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, Spot::Slot(0), &RAID), Ok(()));
    assert_eq!(move_item(&mut roots, bag_id, worn_id, Spot::Slot(1), &RAID), Ok(()));
    assert_eq!(apply_item(&mut roots, magazine_id, bag_id, &RAID), Ok(Applied::Put(Spot::Cell(0, Place { x: 0, y: 0, turned: false }))));
    assert_eq!(roots[0].holder_of(magazine_id), Some((bag_id, Spot::Cell(0, Place { x: 0, y: 0, turned: false }))));
    assert!(roots[1].contents().next().is_none());
    // What is carried is what was in the crate.
    assert!((roots[0].weight() - (3.5 + 1.0 + 0.3)).abs() < 1e-6);
    // What names its own slot stays on the character through a raid.
    let pouch = Item::new(Arc::new(Kind { kept_in: vec!["Dogtag".into()], ..kind("pouch", "item", 0.5, [1, 1]) }));
    let pouch_id = pouch.id;
    roots[0].slots[2] = Some(pouch);
    let free_cell = Spot::Cell(0, Place { x: 0, y: 0, turned: false });
    assert_eq!(move_item(&mut roots, pouch_id, crate_id, free_cell, &RAID), Err(Refusal::KeptOn("Dogtag".into())));
    assert_eq!(move_item(&mut roots, pouch_id, crate_id, free_cell, &Rules { in_raid: false, ..RAID }), Ok(()));
    // Turned and moved within the bag.
    let turned = Spot::Cell(0, Place { x: 2, y: 1, turned: true });
    assert_eq!(move_item(&mut roots, magazine_id, bag_id, turned, &RAID), Ok(()));
    assert_eq!(roots[0].holder_of(magazine_id), Some((bag_id, turned)));
}

/// A stack is split into a free cell, a thing is put down and picked
/// up again into the first place that takes it, and what stays in its
/// slot through a raid is not put down.
#[test]
fn stacks_split_and_things_are_dropped_and_picked_up() {
    let slots = vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[]), holder("Backpack", [0, 0], &[BACKPACK], &[]), holder("Pockets", [0, 0], &["pockets"], &[])];
    let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
    let bag = pack("bag", [3, 2]);
    let mut rounds = Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", "ammo", 0.01, [1, 1]) }));
    rounds.count = 30;
    let pockets = Item::new(Arc::new(Kind { kept_in: vec!["Pockets".into()], ..kind("pockets", "pockets", 0.0, [1, 1]) }));
    let rifle = thing("rifle", "weapon", 3.0, [3, 1]);
    let (bag_id, rounds_id, pockets_id, rifle_id, worn_id) = (bag.id, rounds.id, pockets.id, rifle.id, worn.id);
    worn.slots[1] = Some(bag);
    worn.slots[2] = Some(pockets);
    worn.find_mut(bag_id).unwrap().stow(rounds).unwrap();
    let mut roots = [&mut worn];
    // Ten of thirty into the next cell; not onto a taken one, and not all.
    let cell = |x| Spot::Cell(0, Place { x, y: 0, turned: false });
    assert_eq!(split_item(&mut roots, rounds_id, 10, bag_id, cell(0), &RAID), Err(Refusal::Taken));
    assert_eq!(split_item(&mut roots, rounds_id, 30, bag_id, cell(1), &RAID), Err(Refusal::NoAction));
    let part = split_item(&mut roots, rounds_id, 10, bag_id, cell(1), &RAID).unwrap();
    assert_eq!((roots[0].find(rounds_id).unwrap().count, roots[0].find(part).unwrap().count), (20, 10));
    // Put down, and the weight goes with it.
    assert_eq!(take_out(&mut roots, pockets_id, &RAID), Err(Refusal::KeptOn("Pockets".into())));
    let dropped = take_out(&mut roots, part, &RAID).unwrap();
    assert!((roots[0].weight() - (1.0 + 0.2)).abs() < 1e-6);
    // Picked up: the rounds go onto the stack of their kind in the
    // bag, a rifle onto the sling, and a second rifle, too long for
    // what room the bag has left, nowhere.
    assert_eq!(pick_up(roots[0], dropped, &RAID), (Ok((rounds_id, Applied::Merged)), None));
    assert_eq!(roots[0].find(rounds_id).unwrap().count, 30);
    assert_eq!(pick_up(roots[0], rifle, &RAID), (Ok((worn_id, Applied::Put(Spot::Slot(0)))), None));
    assert_eq!(roots[0].holder_of(rifle_id), Some((worn_id, Spot::Slot(0))));
    let long = thing("long rifle", "weapon", 4.0, [4, 1]);
    assert!(!can_pick_up(roots[0], &long, &RAID));
    let back = pick_up(roots[0], long.clone(), &RAID);
    assert_eq!(back, (Err(Refusal::NoRoom), Some(long)));
}

/// A character with a vest, pockets and a backpack on, and a crate
/// beside it: the equipment, the three worn, the crate.
pub(super) fn dressed() -> (Item, [Id; 3], Item) {
    let slot = |name: &str, class: &str| holder(name, [0, 0], &[class], &[]);
    let slots = vec![
        slot("FirstPrimaryWeapon", "weapon"),
        slot("SecondPrimaryWeapon", "weapon"),
        slot("Holster", "pistol"),
        slot("Backpack", BACKPACK),
        slot("TacticalVest", "vest"),
        slot("Pockets", "pockets"),
        slot("Headwear", "headwear"),
    ];
    let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
    let grids = |cells: &[[u32; 2]]| cells.iter().enumerate().map(|(at, cells)| holder(&format!("{at}"), *cells, &["item"], &[])).collect();
    let vest = Item::new(Arc::new(Kind { grids: grids(&[[1, 2], [1, 2], [2, 2]]), ..kind("vest", "vest", 1.0, [3, 3]) }));
    let pockets = Item::new(Arc::new(Kind { grids: grids(&[[1, 1], [1, 1]]), kept_in: vec!["Pockets".into()], ..kind("pockets", "pockets", 0.0, [1, 1]) }));
    let bag = pack("bag", [4, 4]);
    let ids = [vest.id, pockets.id, bag.id];
    (worn.slots[4], worn.slots[5], worn.slots[3]) = (Some(vest), Some(pockets), Some(bag));
    let crate_ = Item::new(Arc::new(Kind { grids: vec![holder("main", [6, 6], &["item"], &[])], ..kind("crate", "container", 1.0, [1, 1]) }));
    (worn, ids, crate_)
}

pub(super) fn stack(name: &str, class: &str, count: u32) -> Item {
    let mut rounds = Item::new(Arc::new(Kind { stack_max: 60, ..kind(name, class, 0.01, [1, 1]) }));
    rounds.count = count;
    rounds
}

/// Ctrl and a click sends a thing to the other side: out of the crate
/// into the container the game looks in first for that kind of thing,
/// off the character into the crate, and with nothing open to the
/// next place the character has. A stack of the kind over there takes
/// it before any free cell does.
#[test]
fn a_thing_is_sent_to_the_other_side() {
    let (mut worn, [vest_id, pockets_id, bag_id], mut crate_) = dressed();
    let (worn_id, crate_id) = (worn.id, crate_.id);
    let (magazine, grenade, tin) = (thing("magazine", "magazine", 0.3, [1, 2]), thing("grenade", "throwWeap", 0.6, [1, 1]), thing("tin", "food", 0.3, [1, 1]));
    let (rounds, more) = (stack("round", "ammo", 40), stack("round", "ammo", 30));
    let (magazine_id, grenade_id, tin_id, rounds_id, more_id) = (magazine.id, grenade.id, tin.id, rounds.id, more.id);
    for item in [magazine, grenade, tin, rounds, more] {
        crate_.stow(item).unwrap();
    }
    let cell = |grid, x, y| Applied::Put(Spot::Cell(grid, Place { x, y, turned: false }));
    let mut roots = [&mut worn, &mut crate_];
    // Out of the crate: a magazine to the vest, a grenade to the
    // pockets, anything else to the backpack.
    assert_eq!(quick_find(&mut roots, magazine_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((vest_id, cell(0, 0, 0))));
    assert_eq!(quick_find(&mut roots, grenade_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((pockets_id, cell(0, 0, 0))));
    assert_eq!(quick_find(&mut roots, tin_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((bag_id, cell(0, 0, 0))));
    // Rounds to the vest; the second lot onto the first, as far as it
    // has room, and the rest stays in the crate.
    assert_eq!(quick_find(&mut roots, rounds_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((vest_id, cell(1, 0, 0))));
    assert_eq!(quick_find(&mut roots, more_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((rounds_id, Applied::Topped(20))));
    assert_eq!((roots[0].find(rounds_id).unwrap().count, roots[1].find(more_id).unwrap().count), (60, 10));
    // And again: the stack over there is full, so into the next room
    // the vest has.
    assert_eq!(quick_find(&mut roots, more_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((vest_id, cell(1, 0, 1))));
    // Off the character, with the crate open: into the crate's first
    // free cell (the crate, then the character, are looked in).
    assert_eq!(quick_find(&mut roots, magazine_id, &[crate_id, worn_id], Order::OTHER_SIDE, &RAID), Ok((crate_id, cell(0, 0, 0))));
    // With nothing open, a thing on the character goes to the first
    // other place the character has for it: the tin from the bag's
    // first cell to its second.
    assert_eq!(quick_find(&mut roots, tin_id, &[worn_id], Order::OTHER_SIDE, &RAID), Ok((bag_id, cell(0, 1, 0))));
    // What stays on through a raid is not sent anywhere.
    assert_eq!(quick_find(&mut roots, pockets_id, &[crate_id, worn_id], Order::OTHER_SIDE, &RAID), Err(Refusal::KeptOn("Pockets".into())));
}

/// Alt and a click puts a thing on: a weapon onto the sling, the next
/// onto the back, a third nowhere; a magazine is not put on at all.
#[test]
fn a_thing_is_put_on_by_its_kind() {
    let (mut worn, [_, _, bag_id], mut crate_) = dressed();
    let rifles = [thing("rifle", "weapon", 3.0, [4, 1]), thing("carbine", "weapon", 3.0, [3, 1]), thing("shotgun", "weapon", 3.0, [4, 1])];
    let [rifle_id, carbine_id, shotgun_id] = [rifles[0].id, rifles[1].id, rifles[2].id];
    // (In the database headwear is equipment, which is what is put on.)
    let hat = Item::new(Arc::new(Kind { class_names: vec!["hat".into(), "Headwear".into(), "Equipment".into()], ..kind("hat", "headwear", 0.1, [1, 1]) }));
    let magazine = thing("magazine", "magazine", 0.3, [1, 2]);
    let (hat_id, magazine_id) = (hat.id, magazine.id);
    for item in rifles.into_iter().chain([hat, magazine]) {
        crate_.stow(item).unwrap();
    }
    let mut roots = [&mut worn, &mut crate_];
    assert_eq!(equip(&mut roots, rifle_id, &RAID), Ok(0));
    assert_eq!(equip(&mut roots, rifle_id, &RAID), Err(Refusal::AlreadyThere));
    assert_eq!(equip(&mut roots, carbine_id, &RAID), Ok(1));
    // The holster's filter does not take a long weapon.
    assert_eq!(equip(&mut roots, shotgun_id, &RAID), Err(Refusal::NoRoom));
    assert_eq!(equip(&mut roots, hat_id, &RAID), Ok(6));
    assert_eq!(equip(&mut roots, magazine_id, &RAID), Err(Refusal::NoAction));
    // A pick-up puts a weapon on the same way, and with the slots
    // taken puts it in the backpack.
    let (found, left) = pick_up(roots[0], thing("pistol-sized rifle", "weapon", 1.0, [2, 1]), &RAID);
    assert_eq!((found.map(|(into, _)| into), left), (Ok(bag_id), None));
}

/// With Ctrl held a drop is only ever part of a stack: all but one at
/// the most, into a slot one, onto a stack of the kind as many as it
/// has room for.
#[test]
fn part_of_a_stack_is_split_off_or_handed_over() {
    let (mut worn, [vest_id, ..], mut crate_) = dressed();
    let (a, b, lone) = (stack("round", "ammo", 60), stack("round", "ammo", 45), stack("round", "ammo", 1));
    let (a_id, b_id, lone_id) = (a.id, b.id, lone.id);
    for item in [a, b, lone] {
        crate_.stow(item).unwrap();
    }
    let mut roots = [&mut worn, &mut crate_];
    let cell = Spot::Cell(0, Place { x: 0, y: 0, turned: false });
    assert_eq!((split_most(&roots, a_id, cell), split_most(&roots, a_id, Spot::Slot(0)), split_most(&roots, lone_id, cell)), (59, 1, 0));
    // Onto the 45: it has room for 15; onto the 60: none.
    assert_eq!((transfer_most(&roots, a_id, b_id), transfer_most(&roots, b_id, a_id), transfer_most(&roots, a_id, a_id)), (15, 0, 0));
    assert_eq!(transfer_item(&mut roots, a_id, b_id, 16, &RAID), Err(Refusal::NoAction));
    assert_eq!(transfer_item(&mut roots, a_id, b_id, 7, &RAID), Ok(()));
    assert_eq!((roots[1].find(a_id).unwrap().count, roots[1].find(b_id).unwrap().count), (53, 52));
    let part = split_item(&mut roots, a_id, 20, vest_id, cell, &RAID).unwrap();
    assert_eq!((roots[1].find(a_id).unwrap().count, roots[0].find(part).unwrap().count), (33, 20));
}

/// A quick-use key takes medicine, food, a grenade or a weapon that
/// lies in the vest or the pockets, one key a thing and one thing a
/// key, and lets go of what is moved out of them.
#[test]
fn things_are_bound_to_the_quick_use_keys() {
    let (mut worn, [vest_id, pockets_id, bag_id], _) = dressed();
    let (kit, tin, magazine) = (thing("kit", "meds", 0.2, [1, 1]), thing("tin", "foodDrink", 0.3, [1, 1]), thing("magazine", "magazine", 0.3, [1, 2]));
    let (kit_id, tin_id, magazine_id) = (kit.id, tin.id, magazine.id);
    worn.find_mut(vest_id).unwrap().stow(magazine).unwrap();
    worn.find_mut(pockets_id).unwrap().stow(kit).unwrap();
    worn.find_mut(bag_id).unwrap().stow(tin).unwrap();
    let mut bound = Bound::default();
    // A magazine is not bound, and nothing in the backpack is.
    assert!(!bound.bind(&worn, magazine_id, 0) && !bound.bind(&worn, tin_id, 0));
    assert!(bound.bind(&worn, kit_id, 0));
    assert!(bound.bind(&worn, kit_id, 3));
    assert_eq!((bound.items[0], bound.key_of(kit_id)), (None, Some(3)));
    // Into the vest's last grid the tin may be bound, and takes the
    // kit's key from it.
    let cell = Spot::Cell(2, Place { x: 0, y: 0, turned: false });
    assert_eq!(move_item(&mut [&mut worn], tin_id, vest_id, cell, &RAID), Ok(()));
    assert!(bound.bind(&worn, tin_id, 3));
    assert_eq!((bound.key_of(kit_id), bound.key_of(tin_id)), (None, Some(3)));
    // Moved back to the backpack it is bound no longer.
    let free = Spot::Cell(0, Place { x: 2, y: 2, turned: false });
    assert_eq!(move_item(&mut [&mut worn], tin_id, bag_id, free, &RAID), Ok(()));
    bound.settle(&worn);
    assert_eq!(bound.items, [None; 7]);
}

/// Two stacks of a kind become one where the other has the room, and
/// else the other is filled up; backpacks lie in one another only so
/// deep; a vital part stays on its weapon in a raid, and a weapon
/// without one does not go onto the character.
#[test]
fn stacks_merge_and_the_raid_rules_hold() {
    let rounds = |count: u32| {
        let mut stack = Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", "ammo", 0.01, [1, 1]) }));
        stack.count = count;
        stack
    };
    let mut box_ = pack("box", [6, 6]);
    let (a, b, c) = (rounds(20), rounds(30), rounds(50));
    let (a_id, b_id, c_id) = (a.id, b.id, c.id);
    for stack in [a, b, c] {
        box_.stow(stack).unwrap();
    }
    let (inner, innermost) = (pack("inner", [3, 3]), pack("innermost", [2, 2]));
    let (inner_id, innermost_id, box_id) = (inner.id, innermost.id, box_.id);
    box_.stow(inner).unwrap();
    box_.stow(innermost).unwrap();
    let slots = vec![Holder { required: true, ..holder("barrel", [0, 0], &["mod"], &[]) }];
    let mut rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [3, 1]) }));
    let barrel = thing("barrel", "mod", 0.5, [2, 1]);
    let (rifle_id, barrel_id) = (rifle.id, barrel.id);
    rifle.slots[0] = Some(barrel);
    box_.stow(rifle).unwrap();
    let mut worn = Item::new(Arc::new(Kind { slots: vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[])], ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
    let worn_id = worn.id;
    let mut roots = [&mut box_, &mut worn];
    assert_eq!(apply_item(&mut roots, a_id, b_id, &RAID), Ok(Applied::Merged));
    assert_eq!((roots[0].find(a_id), roots[0].find(b_id).map(|stack| stack.count)), (None, Some(50)));
    assert_eq!(apply_item(&mut roots, c_id, b_id, &RAID), Ok(Applied::Topped(10)));
    assert_eq!((roots[0].find(c_id).map(|stack| stack.count), roots[0].find(b_id).map(|stack| stack.count)), (Some(40), Some(60)));
    // One backpack in the box is two deep; a third is one too many.
    assert_eq!(apply_item(&mut roots, innermost_id, inner_id, &RAID), Err(Refusal::TooDeep));
    let free = Rules { backpacks_deep: 0, ..RAID };
    assert!(matches!(apply_item(&mut roots, innermost_id, inner_id, &free), Ok(Applied::Put(_))));
    // The barrel stays on in a raid; out of one it comes off, and the
    // rifle then does not go onto the character.
    let cell = Spot::Cell(0, roots[0].find_room(0, roots[0].find(barrel_id).unwrap()).unwrap());
    assert_eq!(move_item(&mut roots, barrel_id, box_id, cell, &RAID), Err(Refusal::Vital("barrel".into())));
    assert_eq!(move_item(&mut roots, barrel_id, box_id, cell, &Rules { in_raid: false, ..RAID }), Ok(()));
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, Spot::Slot(0), &RAID), Err(Refusal::MissingParts));
    assert_eq!(apply_item(&mut roots, barrel_id, rifle_id, &Rules { in_raid: false, ..RAID }), Ok(Applied::Put(Spot::Slot(0))));
    assert_eq!(move_item(&mut roots, rifle_id, worn_id, Spot::Slot(0), &RAID), Ok(()));
}
