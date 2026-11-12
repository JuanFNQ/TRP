//! Tests of weapons coming apart, magazines' rounds and chambers.

use super::*;

/// A weapon taken apart in a raid: each of its own parts that may come
/// off goes where Ctrl and a click sends it, with what is fitted to it;
/// a vital part stays, and so does the round in the chamber.
#[test]
fn a_weapon_comes_apart_but_for_its_vital_parts() {
    let part_of = |name: &str| {
        let mut kind = kind(name, "mod", 0.2, [1, 1]);
        kind.class_names.push("Mod".into());
        kind.slots = vec![holder("mod_mount", [0, 0], &["mount"], &[])];
        Item::new(Arc::new(kind))
    };
    let slots = vec![
        Holder { required: true, ..holder("mod_barrel", [0, 0], &["barrel"], &[]) },
        holder("mod_stock", [0, 0], &["stock"], &[]),
        holder("patron_in_weapon", [0, 0], &["round"], &[]),
    ];
    let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 3.0, [3, 1]) }));
    let mut stock = part_of("stock");
    stock.slots[0] = Some(part_of("pad"));
    let (stock_id, pad_id) = (stock.id, stock.slots[0].as_ref().unwrap().id);
    rifle.slots[0] = Some(part_of("barrel"));
    rifle.slots[1] = Some(stock);
    rifle.slots[2] = Some(thing("round", "ammo", 0.01, [1, 1]));
    let rifle_id = rifle.id;
    let mut crate_ = pack("crate", [6, 6]);
    crate_.stow(rifle).unwrap();
    let mut bag = pack("bag", [4, 4]);
    let bag_id = bag.id;
    let mut roots = [&mut crate_, &mut bag];
    let came = disassemble(&mut roots, rifle_id, &[bag_id], Order::OTHER_SIDE, &RAID);
    assert_eq!(came.len(), 2);
    assert!(matches!(came[0].1, Err(Refusal::Vital(_))));
    assert!(matches!(came[1], (id, Ok((holder, Applied::Put(_)))) if id == stock_id && holder == bag_id));
    assert!(roots[1].find(pad_id).is_some());
    let rifle = roots[0].find(rifle_id).unwrap();
    assert!(rifle.slots[0].is_some() && rifle.slots[1].is_none() && rifle.slots[2].is_some());
}

/// The rounds a magazine holds are counted from its top, the last
/// stack's first; a weapon's chamber is filled and emptied.
#[test]
fn a_magazine_s_rounds_from_the_top_and_a_chamber_filled() {
    let round = |name: &str, count: u32| Item { count, ..Item::new(Arc::new(Kind { stack_max: 60, ..kind(name, "ammo", 0.01, [1, 1]) })) };
    let mut magazine = thing("magazine", "magazine", 0.1, [1, 2]);
    magazine.loose = vec![round("bottom", 20), round("top", 2)];
    let named = |at: u32| magazine_round_at(&magazine, at).map(|kind| kind.name.clone());
    assert_eq!((named(0), named(1), named(2), named(22)), (Some("top".into()), Some("top".into()), Some("bottom".into()), None));
    let slots = vec![holder("patron_in_weapon", [0, 0], &["ammo"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    let rifle_id = rifle.id;
    let kind = magazine_round_at(&magazine, 0);
    assert!(set_chambered(&mut rifle, rifle_id, kind) && chambered(&rifle).is_some_and(|round| round.count == 1));
    assert!(set_chambered(&mut rifle, rifle_id, None) && chambered(&rifle).is_none());
}

/// The rounds a chamber refuses are counted from the magazine's foot up
/// to and with the first from its top (`Magazine.IsAmmoCompatible` looks
/// at the top round alone); a weapon with no chamber refuses none.
#[test]
fn rounds_the_chamber_refuses_are_counted_from_the_magazine_s_foot() {
    let round = |name: &str, count: u32| Item { count, ..Item::new(Arc::new(Kind { stack_max: 60, ..kind(name, "ammo", 0.01, [1, 1]) })) };
    let slots = vec![holder("mod_magazine", [0, 0], &["magazine"], &[]), holder("patron_in_weapon", [0, 0], &["fits"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    let mut magazine = thing("magazine", "magazine", 0.1, [1, 2]);
    let mut with = |loose: Vec<Item>| {
        magazine.loose = loose;
        rifle.slots[0] = Some(magazine.clone());
        (fitted_unfit_rounds(&rifle), ammo_incompatible(&rifle))
    };
    assert_eq!(with(vec![round("fits", 30)]), (0, false), "all taken");
    assert_eq!(with(vec![round("other", 30)]), (30, true), "none taken: none fed");
    assert_eq!(with(vec![round("other", 28), round("fits", 2)]), (28, false), "two fed, then the first refused is on top");
    assert_eq!(with(vec![round("fits", 5), round("other", 3), round("fits", 2)]), (8, false), "fitting rounds under it never come up");
    assert_eq!(with(Vec::new()), (0, false), "empty: compatible");
    // No chamber of its own (a revolver's cylinder, a weapon of none).
    let slots = vec![holder("mod_magazine", [0, 0], &["magazine"], &[])];
    let mut open = Item::new(Arc::new(Kind { slots, ..kind("open", "weapon", 3.0, [4, 1]) }));
    magazine.loose = vec![round("other", 30)];
    open.slots[0] = Some(magazine);
    assert_eq!((fitted_unfit_rounds(&open), ammo_incompatible(&open)), (0, false));
    assert_eq!(chamber_takes(&open, &round("other", 1).kind), None);
}

/// A round let go on a weapon goes into its empty chamber, one split
/// off a stack; not into a full one, nor into the chamber of the weapon
/// in hand whose hands would not take it.
#[test]
fn a_round_let_go_on_a_weapon_goes_into_its_chamber() {
    let slots = vec![holder("mod_magazine", [0, 0], &["magazine"], &[]), holder("patron_in_weapon", [0, 0], &["round"], &[])];
    let rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    let rifle_id = rifle.id;
    let mut rounds = Item::new(Arc::new(Kind { stack_max: 60, ..kind("round", "round", 0.01, [1, 1]) }));
    rounds.count = 20;
    let rounds_id = rounds.id;
    let mut bag = pack("bag", [6, 4]);
    bag.stow(rifle).unwrap();
    bag.stow(rounds).unwrap();
    let mut roots = [&mut bag];
    let closed = Rules { chamber_closed: Some(rifle_id), ..RAID };
    assert_eq!(apply_item(&mut roots, rounds_id, rifle_id, &closed), Err(Refusal::HandsBusy));
    assert_eq!(apply_item(&mut roots, rounds_id, rifle_id, &RAID), Ok(Applied::Put(Spot::Slot(1))));
    let rifle = roots[0].find(rifle_id).unwrap();
    assert_eq!(rifle.slots[1].as_ref().map(|round| round.count), Some(1));
    assert_eq!(roots[0].find(rounds_id).map(|stack| stack.count), Some(19));
    assert!(apply_item(&mut roots, rounds_id, rifle_id, &RAID).is_err(), "the chamber is full");
    // The round in it stays where the hands would not let it out: not
    // moved, merged or thrown out.
    let chambered_id = roots[0].find(rifle_id).and_then(|rifle| rifle.slots[1].as_ref()).map(|round| round.id).unwrap();
    let held = Rules { chamber_held: Some(rifle_id), ..RAID };
    let bag_id = roots[0].id;
    let cell = Spot::Cell(0, roots[0].find_room(0, roots[0].find(chambered_id).unwrap()).unwrap());
    assert_eq!(move_item(&mut roots, chambered_id, bag_id, cell, &held), Err(Refusal::HandsBusy));
    assert_eq!(apply_item(&mut roots, chambered_id, rounds_id, &held), Err(Refusal::HandsBusy));
    assert_eq!(take_out(&mut roots, chambered_id, &held).err(), Some(Refusal::HandsBusy));
    assert_eq!(apply_item(&mut roots, chambered_id, rounds_id, &RAID), Ok(Applied::Merged));
    assert!(roots[0].find(rifle_id).unwrap().slots[1].is_none());
}

/// A malfunctioned weapon keeps its magazine with a failure to feed and
/// takes no round into its chamber with any (`IncompatibleByMalfunction`);
/// it and such a magazine are drawn unusable once the shooter knows.
#[test]
fn a_malfunctioned_weapon_keeps_its_magazine_and_its_chamber() {
    use crate::weapons::malfunction::Malf;
    let slots = vec![holder("mod_magazine", [0, 0], &["magazine"], &[]), holder("patron_in_weapon", [0, 0], &["ammo"], &[])];
    let mut rifle = Item::new(Arc::new(Kind { slots, merges: true, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    let magazine = thing("magazine", "magazine", 0.1, [1, 2]);
    let magazine_id = magazine.id;
    rifle.slots[0] = Some(magazine);
    let rifle_id = rifle.id;
    let round = thing("round", "ammo", 0.01, [1, 1]);
    let round_id = round.id;
    let mut bag = pack("bag", [6, 4]);
    bag.stow(rifle).unwrap();
    bag.stow(round).unwrap();
    let bag_id = bag.id;
    let mut roots = [&mut bag];
    let set = |roots: &mut [&mut Item], malf: Malf, known: bool| {
        let rifle = roots[0].find_mut(rifle_id).unwrap();
        (rifle.malfunction.malf, rifle.malfunction.known) = (malf, known);
    };
    // A failure to feed: the magazine stays, and no round goes in.
    set(&mut roots, Malf::Feed, false);
    let out = Spot::Cell(0, roots[0].find_room(0, roots[0].find(magazine_id).unwrap()).unwrap());
    assert_eq!(move_item(&mut roots, magazine_id, bag_id, out, &RAID), Err(Refusal::Malfunction));
    assert_eq!(move_item(&mut roots, round_id, rifle_id, Spot::Slot(1), &RAID), Err(Refusal::Malfunction));
    assert!(!unusable_by_malfunction(roots[0], roots[0].find(rifle_id).unwrap()), "not known: drawn as it is");
    set(&mut roots, Malf::Feed, true);
    assert!(unusable_by_malfunction(roots[0], roots[0].find(rifle_id).unwrap()));
    assert!(unusable_by_malfunction(roots[0], roots[0].find(magazine_id).unwrap()), "its magazine with a failure to feed");
    // A jam: the magazine may leave, still no round in.
    set(&mut roots, Malf::Jam, true);
    assert!(!unusable_by_malfunction(roots[0], roots[0].find(magazine_id).unwrap()));
    assert_eq!(move_item(&mut roots, round_id, rifle_id, Spot::Slot(1), &RAID), Err(Refusal::Malfunction));
    assert_eq!(move_item(&mut roots, magazine_id, bag_id, out, &RAID), Ok(()));
    // Put right, the round goes in.
    set(&mut roots, Malf::None, false);
    assert_eq!(move_item(&mut roots, round_id, rifle_id, Spot::Slot(1), &RAID), Ok(()));
}

/// A magazine is known once a round in or out leaves it empty or
/// full; a shot that leaves it neither unknows it, anything else
/// leaves it as it was.
#[test]
fn a_magazine_is_known_empty_or_full_and_unknown_after_a_shot() {
    assert!(known_after(0, 30, false, true) && known_after(30, 30, false, false));
    assert!(!known_after(29, 30, true, true) && known_after(29, 30, true, false));
    assert!(!known_after(12, 30, false, false));
}
