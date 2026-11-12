//! Tests of room for things sent over, conflicts, locked slots, raid limits and weight.

use super::*;

/// What Ctrl and a click looks in: the priority window's container and
/// nothing else, unless the thing lies in it; else what is open, with
/// the equipment after it for a thing the character has; else the
/// equipment.
#[test]
fn what_a_thing_sent_over_is_looked_for_room_in() {
    let (mut worn, [vest_id, _, bag_id], mut crate_) = dressed();
    let (worn_id, crate_id) = (worn.id, crate_.id);
    let (mine, theirs) = (thing("tin", "food", 0.3, [1, 1]), thing("can", "food", 0.3, [1, 1]));
    let (mine_id, theirs_id) = (mine.id, theirs.id);
    worn.find_mut(vest_id).unwrap().stow(mine).unwrap();
    crate_.stow(theirs).unwrap();
    assert_eq!(other_side(&worn, None, None, mine_id), [worn_id]);
    assert_eq!(other_side(&worn, Some(&crate_), None, mine_id), [crate_id, worn_id]);
    assert_eq!(other_side(&worn, Some(&crate_), None, theirs_id), [worn_id]);
    // The bag's window is the priority one: that and nothing else.
    assert_eq!(other_side(&worn, Some(&crate_), Some(bag_id), mine_id), [bag_id]);
    assert_eq!(other_side(&worn, Some(&crate_), Some(bag_id), theirs_id), [bag_id]);
    // Unless the thing lies in it.
    assert_eq!(other_side(&worn, Some(&crate_), Some(vest_id), mine_id), [crate_id, worn_id]);
}

/// What goes together goes either way round: a rifle with the
/// magazines its slot takes, the rounds its chamber or its magazine
/// takes, and the parts that fit it or a part on it; a magazine with its
/// rounds. A refused kind, another calibre, a hat, a backpack's room and
/// the thing itself do not.
#[test]
fn what_goes_together_goes_either_way() {
    let rail = |name: &str| holder(name, [0, 0], &["sight"], &["big_sight"]);
    let mount = Item::new(Arc::new(Kind { slots: vec![rail("mod_scope")], ..kind("mount", "mod", 0.1, [1, 1]) }));
    let slots = vec![holder(MAGAZINE_SLOT, [0, 0], &["mag"], &[]), holder("mod_mount", [0, 0], &["mount"], &[]), holder("patron_in_weapon", [0, 0], &["round"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    rifle.slots[1] = Some(mount);
    let magazine = Item::new(Arc::new(Kind { cartridges: vec![holder("cartridges", [0, 0], &["round"], &[])], ..kind("mag", "magazine", 0.2, [1, 2]) }));
    let (round, other_round) = (thing("round", "ammo", 0.01, [1, 1]), thing("shell", "ammo", 0.04, [1, 1]));
    let (sight, big_sight, hat) = (thing("red_dot", "sight", 0.1, [1, 1]), Item::new(Arc::new(Kind { classes: vec!["big".into(), "big_sight".into(), "sight".into()], ..kind("big", "sight", 0.4, [2, 1]) })), thing("hat", "headwear", 0.1, [1, 1]));
    let bag = Item::new(Arc::new(Kind { grids: vec![holder("main", [4, 4], &["item"], &[])], ..kind("bag", "backpack", 1.0, [3, 3]) }));
    for (a, b) in [(&rifle, &magazine), (&rifle, &round), (&rifle, &sight), (&magazine, &round)] {
        assert!(go_together(a, b) && go_together(b, a), "{} and {}", a.kind.name, b.kind.name);
    }
    for (a, b) in [(&rifle, &other_round), (&rifle, &big_sight), (&rifle, &hat), (&magazine, &sight), (&bag, &round), (&rifle, &rifle)] {
        assert!(!go_together(a, b) && !go_together(b, a), "{} and {}", a.kind.name, b.kind.name);
    }
    // Rounds a rifle has no chamber for, which its magazine takes.
    let mut tube = Item::new(Arc::new(Kind { slots: vec![holder(MAGAZINE_SLOT, [0, 0], &["mag"], &[])], ..kind("tube", "weapon", 3.0, [5, 1]) }));
    tube.slots[0] = Some(again(&magazine));
    assert!(go_together(&tube, &round));
}

/// A box of a few cells for things to come from and go to.
fn a_box() -> Item {
    Item::new(Arc::new(Kind { grids: vec![holder("main", [8, 8], &["item"], &[])], ..kind("box", "container", 1.0, [8, 8]) }))
}

/// Two parts that may not be on one weapon together are refused either
/// way round, whichever names the other, the pair said with the one
/// there first (`Slot.CheckConflictingItems`); a slot that does not
/// merge with its item asks nothing.
#[test]
fn conflicting_parts_are_refused_whichever_names_the_other() {
    let rail = |name: &str| holder(name, [0, 0], &["mod"], &[]);
    let slots = vec![rail("mod_sight_rear"), rail("mod_reciever")];
    let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    let sight = thing("rear_sight", "mod", 0.1, [1, 1]);
    let sight_id = sight.id;
    rifle.slots[0] = Some(sight);
    // The dust cover names the rear sight; the sight names nothing.
    let cover = Item::new(Arc::new(Kind { conflicting: vec!["rear_sight".into()], merges: true, ..kind("cover", "mod", 0.2, [2, 1]) }));
    let (rifle_id, cover_id) = (rifle.id, cover.id);
    let mut box_ = a_box();
    let box_id = box_.id;
    box_.stow(cover).unwrap();
    let free = Rules { in_raid: false, ..RAID };
    let mut roots = [&mut rifle, &mut box_];
    assert_eq!(move_item(&mut roots, cover_id, rifle_id, Spot::Slot(1), &free), Err(Refusal::Conflicting { existing: "rear_sight".into(), new: "cover".into() }));
    // The sight off, the cover goes on; the sight is then refused, the
    // cover there naming it.
    let cell = Spot::Cell(0, roots[1].find_room(0, roots[0].find(sight_id).unwrap()).unwrap());
    assert_eq!(move_item(&mut roots, sight_id, box_id, cell, &free), Ok(()));
    assert_eq!(move_item(&mut roots, cover_id, rifle_id, Spot::Slot(1), &free), Ok(()));
    assert_eq!(move_item(&mut roots, sight_id, rifle_id, Spot::Slot(0), &free), Err(Refusal::Conflicting { existing: "cover".into(), new: "rear_sight".into() }));
    // A stand whose slots do not merge with it holds both.
    let mut stand = Item::new(Arc::new(Kind { slots: vec![rail("one"), rail("two")], ..kind("stand", "container", 5.0, [5, 5]) }));
    stand.slots[0] = roots[0].take(cover_id);
    let stand_id = stand.id;
    let mut roots = [&mut stand, &mut box_];
    assert_eq!(move_item(&mut roots, sight_id, stand_id, Spot::Slot(1), &free), Ok(()));
}

/// What a part carries comes with it: a mount bearing a part the
/// weapon's own names is refused, and the pair named is the part on the
/// mount; a slot's excluded list goes for what is on the part too.
#[test]
fn what_a_part_carries_is_asked_with_it() {
    let slots = vec![holder("mod_mount", [0, 0], &["mod"], &["laser"]), holder("mod_tactical", [0, 0], &["mod"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    rifle.slots[1] = Some(Item::new(Arc::new(Kind { conflicting: vec!["light".into()], merges: true, ..kind("grip", "mod", 0.1, [1, 1]) })));
    let mount = |on: Item| {
        let mut mount = Item::new(Arc::new(Kind { slots: vec![holder("mod_tactical", [0, 0], &["mod", "laser"], &[])], merges: true, ..kind("mount", "mod", 0.1, [1, 1]) }));
        mount.slots[0] = Some(on);
        mount
    };
    let lit = mount(thing("light", "mod", 0.1, [1, 1]));
    let lasered = mount(Item::new(Arc::new(Kind { classes: vec!["beam".into(), "laser".into(), "mod".into(), "item".into()], ..kind("beam", "mod", 0.1, [1, 1]) })));
    assert_eq!(rifle.slot_takes(0, &lasered), Err(Refusal::Filter));
    assert_eq!(rifle.slot_takes(0, &lit), Ok(()));
    let rifle_id = rifle.id;
    let mut box_ = a_box();
    let lit_id = lit.id;
    box_.stow(lit).unwrap();
    let mut roots = [&mut rifle, &mut box_];
    assert_eq!(move_item(&mut roots, lit_id, rifle_id, Spot::Slot(0), &Rules { in_raid: false, ..RAID }), Err(Refusal::Conflicting { existing: "grip".into(), new: "light".into() }));
}

/// A locked slot takes nothing and gives nothing up, in a raid or not
/// (`Slot.Locked`: the pockets worn, an armour's built-in inserts); a
/// thing for the special slots alone goes only into one of them.
#[test]
fn a_locked_slot_neither_takes_nor_gives() {
    let locked = |name: &str| Holder { locked: true, armour: true, ..holder(name, [0, 0], &["insert"], &[]) };
    let mut vest = Item::new(Arc::new(Kind { slots: vec![locked("Soft_armor_front"), locked("Soft_armor_back")], ..kind("vest", "armor", 4.0, [3, 3]) }));
    let insert = thing("insert", "insert", 0.5, [1, 1]);
    let insert_id = insert.id;
    vest.slots[0] = Some(insert);
    let vest_id = vest.id;
    let mut box_ = a_box();
    let box_id = box_.id;
    let spare = thing("insert", "insert", 0.5, [1, 1]);
    let spare_id = spare.id;
    box_.stow(spare).unwrap();
    let free = Rules { in_raid: false, ..RAID };
    let mut roots = [&mut vest, &mut box_];
    let cell = Spot::Cell(0, Place { x: 6, y: 6, turned: false });
    assert_eq!(move_item(&mut roots, insert_id, box_id, cell, &free), Err(Refusal::Locked));
    assert_eq!(take_out(&mut roots, insert_id, &free), Err(Refusal::Locked));
    assert_eq!(move_item(&mut roots, spare_id, vest_id, Spot::Slot(1), &free), Err(Refusal::Locked));
    // The special slots' own things.
    let pockets = Item::new(Arc::new(Kind { slots: vec![holder("SpecialSlot1", [0, 0], &["item"], &[]), holder("pocket1", [0, 0], &["item"], &[])], ..kind("pockets", "pockets", 0.0, [1, 1]) }));
    let marker = Item::new(Arc::new(Kind { special_only: true, ..kind("marker", "special", 0.1, [1, 1]) }));
    assert_eq!(pockets.slot_takes(0, &marker), Ok(()));
    assert_eq!(pockets.slot_takes(1, &marker), Err(Refusal::Filter));
}

/// In a raid a plate's slot of armour worn takes no plate; the armour
/// off, or out of a raid, it does (`ArmorSlot.CanAcceptRaid`).
#[test]
fn a_worn_armour_takes_no_plate_in_a_raid() {
    let plate_slot = Holder { armour: true, ..holder("Front_plate", [0, 0], &["plate"], &[]) };
    let armour = Item::new(Arc::new(Kind { slots: vec![plate_slot], ..kind("armour", "armor", 4.0, [3, 3]) }));
    let armour_id = armour.id;
    let slots = vec![holder("ArmorVest", [0, 0], &["armor"], &[])];
    let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
    worn.slots[0] = Some(armour);
    let mut box_ = a_box();
    let plate = thing("plate", "plate", 2.0, [2, 2]);
    let plate_id = plate.id;
    box_.stow(plate).unwrap();
    let mut roots = [&mut worn, &mut box_];
    assert_eq!(move_item(&mut roots, plate_id, armour_id, Spot::Slot(0), &RAID), Err(Refusal::PlateWorn));
    assert_eq!(move_item(&mut roots, plate_id, armour_id, Spot::Slot(0), &Rules { in_raid: false, ..RAID }), Ok(()));
}

/// The things worn on the head merge with the character (their slots
/// say so), so a headset is refused under a helmet that names it; the
/// armour's slot does not merge, so armour asks nothing of the helmet.
#[test]
fn what_is_worn_on_the_head_is_set_against_the_rest() {
    let merging = |name: &str, takes: &str| Holder { merges: Some(true), ..holder(name, [0, 0], &[takes], &[]) };
    let slots = vec![merging("Headwear", "headwear"), merging("Earpiece", "headphones"), holder("ArmorVest", [0, 0], &["armor"], &[])];
    let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
    worn.slots[0] = Some(Item::new(Arc::new(Kind { conflicting: vec!["headset".into()], ..kind("helmet", "headwear", 1.0, [2, 2]) })));
    let worn_id = worn.id;
    let mut box_ = a_box();
    let (headset, armour) = (thing("headset", "headphones", 0.3, [2, 1]), Item::new(Arc::new(Kind { conflicting: vec!["helmet".into()], ..kind("armour", "armor", 4.0, [3, 3]) })));
    let (headset_id, armour_id) = (headset.id, armour.id);
    box_.stow(headset).unwrap();
    box_.stow(armour).unwrap();
    let mut roots = [&mut worn, &mut box_];
    assert_eq!(move_item(&mut roots, headset_id, worn_id, Spot::Slot(1), &RAID), Err(Refusal::Conflicting { existing: "helmet".into(), new: "headset".into() }));
    assert_eq!(move_item(&mut roots, armour_id, worn_id, Spot::Slot(2), &RAID), Ok(()));
}

/// An armoured rig blocks the body armour's slot: worn, it keeps armour
/// off; armour worn keeps the rig off, and a rig that blocks nothing
/// goes on beside it.
#[test]
fn an_armoured_rig_and_body_armour_are_not_worn_together() {
    let slots = vec![holder("TacticalVest", [0, 0], &["vest"], &[]), holder("ArmorVest", [0, 0], &["armor"], &[])];
    let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
    let worn_id = worn.id;
    let rig = Item::new(Arc::new(Kind { blocks: vec!["ArmorVest".into()], ..kind("armoured rig", "vest", 6.0, [2, 2]) }));
    let (armour, plain) = (thing("armour", "armor", 4.0, [2, 2]), thing("rig", "vest", 1.0, [2, 2]));
    let (rig_id, armour_id, plain_id) = (rig.id, armour.id, plain.id);
    let mut box_ = a_box();
    let box_id = box_.id;
    for item in [rig, armour, plain] {
        box_.stow(item).unwrap();
    }
    let mut roots = [&mut worn, &mut box_];
    assert_eq!(move_item(&mut roots, rig_id, worn_id, Spot::Slot(0), &RAID), Ok(()));
    assert_eq!(move_item(&mut roots, armour_id, worn_id, Spot::Slot(1), &RAID), Err(Refusal::Blocked("TacticalVest".into())));
    // The other way round: the rig off, the armour on.
    assert_eq!(move_item(&mut roots, rig_id, box_id, Spot::Cell(0, Place { x: 6, y: 6, turned: false }), &RAID), Ok(()));
    assert_eq!(move_item(&mut roots, armour_id, worn_id, Spot::Slot(1), &RAID), Ok(()));
    assert_eq!(move_item(&mut roots, rig_id, worn_id, Spot::Slot(0), &RAID), Err(Refusal::WouldBlock("ArmorVest".into())));
    assert_eq!(move_item(&mut roots, plain_id, worn_id, Spot::Slot(0), &RAID), Ok(()));
}

/// With the raid's limits, no more of a kind comes onto the character
/// from elsewhere than its limit leaves room for, by a move, a split or
/// a stack put on a stack; a thing moved within what the character has
/// is not counted again.
#[test]
fn the_raid_s_limits_keep_what_is_carried_down() {
    let limits: &'static std::collections::HashMap<String, u32> = Box::leak(Box::new([("big_pack".to_owned(), 1), ("round".to_owned(), 10)].into_iter().collect()));
    let rules = Rules { limits: Some(limits), ..RAID };
    let pockets = Item::new(Arc::new(Kind { grids: vec![holder("main", [8, 8], &["item"], &[])], ..kind("pockets", "pockets", 0.0, [1, 1]) }));
    let pockets_id = pockets.id;
    let slots = vec![holder("Backpack", [0, 0], &["backpack"], &[]), holder("Pockets", [0, 0], &["pockets"], &[])];
    let mut worn = Item::new(Arc::new(Kind { slots, ..kind(EQUIPMENT_TEMPLATE, "inventory", 0.0, [1, 1]) }));
    worn.slots[1] = Some(pockets);
    let worn_id = worn.id;
    let mut box_ = a_box();
    let pack = || thing("big_pack", "backpack", 2.0, [2, 2]);
    let (first, second) = (pack(), pack());
    let (first_id, second_id) = (first.id, second.id);
    box_.stow(first).unwrap();
    box_.stow(second).unwrap();
    let stack = |count: u32| Item { count, ..Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", "ammo", 0.01, [1, 1]) })) };
    let (eight, five) = (stack(8), stack(5));
    let (eight_id, five_id) = (eight.id, five.id);
    box_.stow(eight).unwrap();
    box_.stow(five).unwrap();
    let at = |x: u32, y: u32| Spot::Cell(0, Place { x, y, turned: false });
    let mut roots = [&mut worn, &mut box_];
    assert_eq!(move_item(&mut roots, first_id, worn_id, Spot::Slot(0), &rules), Ok(()));
    assert_eq!(move_item(&mut roots, second_id, pockets_id, at(0, 0), &rules), Err(Refusal::OverLimit { template: "big_pack".into(), limit: 1 }));
    // The first moved within the character is not counted again.
    assert_eq!(move_item(&mut roots, first_id, pockets_id, at(0, 0), &rules), Ok(()));
    assert_eq!(move_item(&mut roots, eight_id, pockets_id, at(4, 0), &rules), Ok(()));
    assert_eq!(move_item(&mut roots, five_id, pockets_id, at(5, 0), &rules), Err(Refusal::OverLimit { template: "round".into(), limit: 10 }));
    assert!(split_item(&mut roots, five_id, 2, pockets_id, at(5, 0), &rules).is_ok());
    // Ten now: a stack put on the stack there is refused.
    assert_eq!(apply_item(&mut roots, five_id, eight_id, &rules), Err(Refusal::OverLimit { template: "round".into(), limit: 10 }));
    // Without the limits, it goes.
    assert_eq!(apply_item(&mut roots, five_id, eight_id, &RAID), Ok(Applied::Merged));
}

/// What the inventory tells the weight of (`Weighed`): a magazine
/// stowed, a medkit used, armour worn down, a fold are told; the hands'
/// shots passed over unheard are not, nor a weapon's wear; a shot after
/// a change not yet told is left for that change to count; a magazine
/// checked is told though nothing in an item changed.
#[test]
fn the_weight_hears_the_inventory_s_events_and_not_the_shots() {
    let round = |count: u32| Item { count, ..Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", "ammo", 0.016, [1, 1]) })) };
    let mut magazine = thing("magazine", "magazine", 0.1, [1, 2]);
    magazine.loose = vec![round(30)];
    let slots = vec![holder("mod_magazine", [0, 0], &["magazine"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    rifle.repairable = Some(Repairable { durability: 100.0, max_durability: 100.0 });
    let magazine_id = magazine.id;
    rifle.slots[0] = Some(magazine);
    let mut vest = pack("vest", [4, 4]);
    let mut medkit = thing("medkit", "medKit", 0.2, [1, 1]);
    medkit.resource = Some(400.0);
    let mut armour = thing("armour", "armor", 5.0, [3, 3]);
    armour.repairable = Some(Repairable { durability: 50.0, max_durability: 50.0 });
    let (medkit_id, armour_id) = (medkit.id, armour.id);
    vest.stow(medkit).unwrap();
    vest.stow(armour).unwrap();
    let mut body = Item::new(Arc::new(Kind { slots: vec![holder("FirstPrimaryWeapon", [0, 0], &["weapon"], &[]), holder("TacticalVest", [0, 0], &["backpack"], &[])], ..kind("body", "equipment", 0.0, [1, 1]) }));
    let (rifle_id, vest_id) = (rifle.id, vest.id);
    body.slots = vec![Some(rifle), Some(vest)];
    let mut weighed = Weighed::default();
    assert!(weighed.told(&body), "the first is an event");
    assert!(!weighed.told(&body), "nothing changed");
    // Five shots: the rounds out and the weapon worn, passed over.
    let quiet = weighed.as_told(&body);
    assert!(quiet && set_rounds_of(&mut body, magazine_id, 25));
    body.find_mut(rifle_id).unwrap().repairable.as_mut().unwrap().durability = 99.5;
    weighed.pass(&body);
    assert!(!weighed.told(&body), "the shots go unheard");
    // A weapon's wear alone is no event; its greatest durability is (a repair).
    body.find_mut(rifle_id).unwrap().repairable.as_mut().unwrap().durability = 99.0;
    assert!(!weighed.told(&body));
    body.find_mut(rifle_id).unwrap().repairable.as_mut().unwrap().max_durability = 99.8;
    assert!(weighed.told(&body));
    // The reload's end: the magazine stowed in the vest is an event.
    let magazine = body.find_mut(rifle_id).unwrap().slots[0].take().unwrap();
    body.find_mut(vest_id).unwrap().stow(magazine).unwrap();
    assert!(weighed.told(&body), "the magazine stowed");
    // A medkit used, armour worn down by a hit, each an event.
    assert!(set_resource(&mut body, medkit_id, 300.0) && weighed.told(&body));
    body.find_mut(armour_id).unwrap().repairable.as_mut().unwrap().durability = 45.0;
    assert!(weighed.told(&body));
    // A move first, then a shot: not passed over, so the move counts it.
    let magazine = body.find_mut(vest_id).unwrap().grids[0].pop().unwrap().1;
    body.find_mut(rifle_id).unwrap().slots[0] = Some(magazine);
    assert!(!weighed.as_told(&body));
    set_rounds_of(&mut body, magazine_id, 24);
    assert!(weighed.told(&body));
    // A magazine checked: told though nothing in an item changed.
    weighed.forget();
    assert!(weighed.told(&body) && !weighed.told(&body));
}
