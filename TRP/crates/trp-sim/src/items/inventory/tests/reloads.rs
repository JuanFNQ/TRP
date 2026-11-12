//! Tests of cylinders, tubes, reloads and magazines loaded and swapped.

use super::*;

/// The rounds a tube is filled from are those within reach that the
/// weapon's chamber takes: what lies directly in the pockets and the
/// vest, the pockets' first; the backpack's are out of reach.
/// A cylinder holds one round in each of its chambers (`camora_` slots,
/// more-weapons spec, "The rounds and cases seen in chambers of their
/// own"): filled, counted, emptied from its last, loaded into its first
/// empty one; a weapon's chambers of its own read and written by number,
/// a cylinder's or barrels'.
#[test]
fn a_cylinder_holds_its_rounds_in_its_chambers() {
    let camoras: Vec<Holder> = (0..6).map(|n| holder(&format!("camora_00{n}"), [0, 0], &["round"], &[])).collect();
    let cylinder = Arc::new(Kind { slots: camoras, cartridges: vec![holder("cartridges", [0, 0], &["round"], &[])], capacity: 6, ..kind("cylinder", "magazine", 0.1, [1, 1]) });
    let round = Arc::new(kind("round", "round", 0.01, [1, 1]));
    let other = Arc::new(kind("other", "round", 0.01, [1, 1]));
    let full = |magazine: &Item| magazine.slots.iter().map(|slot| slot.as_ref().map_or('.', |round| round.kind.name.chars().next().unwrap_or('?'))).collect::<String>();
    let mut drum = Item::new(cylinder.clone());
    fill_magazine(&mut drum, round.clone(), 4);
    assert_eq!((full(&drum), magazine_rounds(&drum), drum.loose.len()), ("rrrr..".to_owned(), 4, 0));
    set_magazine_rounds(&mut drum, 2);
    assert_eq!(full(&drum), "rr....");
    let drum_id = drum.id;
    assert!(put_rounds_into(&mut drum, drum_id, &[(other.clone(), 1)]));
    assert_eq!((full(&drum), magazine_round_at(&drum, 0).map(|kind| kind.name.clone())), ("rro...".to_owned(), Some("other".to_owned())));
    // In a revolver: its chambers by number, written through the weapon.
    let revolver = Arc::new(Kind { slots: vec![holder(MAGAZINE_SLOT, [0, 0], &["cylinder"], &[])], ..kind("revolver", "weapon", 1.0, [2, 1]) });
    let mut gun = Item::new(revolver);
    gun.slots[0] = Some(drum);
    let names = |gun: &Item| chamber_rounds(gun).iter().map(|round| round.as_ref().map_or('.', |kind| kind.name.chars().next().unwrap_or('?'))).collect::<String>();
    assert_eq!(names(&gun), "rro...");
    let id = gun.id;
    assert!(set_chamber_round(&mut gun, id, 0, None) && set_chamber_round(&mut gun, id, 5, Some(other.clone())));
    assert_eq!(names(&gun), ".ro..o");
    // Barrels: the weapon's own chambers.
    let barrels = Arc::new(Kind { slots: vec![holder("patron_in_weapon_000", [0, 0], &["round"], &[]), holder("patron_in_weapon_001", [0, 0], &["round"], &[])], ..kind("double", "weapon", 3.0, [4, 1]) });
    let mut double = Item::new(barrels);
    let id = double.id;
    assert!(set_chamber_round(&mut double, id, 1, Some(round)));
    assert_eq!(names(&double), ".r");
}

#[test]
fn a_tube_is_filled_from_the_pockets_and_the_vest() {
    let (mut worn, [vest_id, pockets_id, bag_id], _) = dressed();
    let chamber = holder(CHAMBER, [0, 0], &["shell"], &[]);
    let gun = Item::new(Arc::new(Kind { slots: vec![chamber], ..kind("shotgun", "weapon", 3.0, [5, 1]) }));
    let gun_id = gun.id;
    worn.slots[0] = Some(gun);
    let (in_vest, in_pocket, in_bag, other) = (stack("shell", "ammo", 7), stack("shell", "ammo", 2), stack("shell", "ammo", 20), stack("round", "ammo", 30));
    let (vest_shells, pocket_shells) = (in_vest.id, in_pocket.id);
    for (holder, item) in [(vest_id, in_vest), (pockets_id, in_pocket), (bag_id, in_bag), (vest_id, other)] {
        worn.find_mut(holder).unwrap().stow(item).unwrap();
    }
    // Seven in the vest and two in a pocket; the bag's twenty and the
    // rifle rounds do not count.
    assert_eq!(rounds_in_reach(&worn, gun_id), 9);
    // The pocket's go first, and a stack used up is gone.
    assert_eq!(take_rounds(&mut worn, gun_id, 3), 3);
    assert!(worn.find(pocket_shells).is_none());
    assert_eq!(worn.find(vest_shells).unwrap().count, 6);
    // No more is taken than there is.
    assert_eq!(take_rounds(&mut worn, gun_id, 10), 6);
    // What is taken, by kind, goes onto a tube's top.
    let mut tube = thing("tube", "magazine", 0.1, [1, 1]);
    let tube_id = tube.id;
    let shell = Arc::new(kind("shell", "ammo", 0.04, [1, 1]));
    assert!(put_rounds_into(&mut tube, tube_id, &[(shell.clone(), 2), (shell, 1)]));
    assert_eq!((tube.loose.len(), magazine_rounds(&tube)), (1, 3));
    assert_eq!(rounds_in_reach(&worn, gun_id), 0);
}

/// A rifle on the character, its magazine fitted with `fitted` rounds;
/// and a magazine of the kind it takes holding so many.
fn rifle_worn(worn: &mut Item, fitted: u32) -> (Id, Id) {
    let rifle = Item::new(Arc::new(Kind { slots: vec![holder(MAGAZINE_SLOT, [0, 0], &["mag"], &[])], ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    let (rifle_id, fitted) = (rifle.id, loaded("mag", fitted));
    let fitted_id = fitted.id;
    worn.slots[0] = Some(rifle);
    worn.find_mut(rifle_id).unwrap().slots[0] = Some(fitted);
    (rifle_id, fitted_id)
}

fn loaded(name: &str, rounds: u32) -> Item {
    let mut magazine = thing(name, "magazine", 0.2, [1, 2]);
    fill_magazine(&mut magazine, Arc::new(kind("round", "ammo", 0.01, [1, 1])), rounds);
    magazine
}

/// The reload's magazine is the fullest one directly in the pockets
/// or the vest that the weapon takes, with a round in it; the
/// backpack's, an empty one and one of another kind are passed over.
#[test]
fn a_reload_takes_the_fullest_magazine_within_reach() {
    let (mut worn, [vest_id, _, bag_id], _) = dressed();
    let (rifle_id, _) = rifle_worn(&mut worn, 3);
    let (ten, twenty, empty, in_bag, other) = (loaded("mag", 10), loaded("mag", 20), loaded("mag", 0), loaded("mag", 30), loaded("drum", 30));
    let (ten_id, twenty_id) = (ten.id, twenty.id);
    for (holder, item) in [(vest_id, ten), (vest_id, empty), (bag_id, in_bag), (vest_id, other)] {
        worn.find_mut(holder).unwrap().stow(item).unwrap();
    }
    assert_eq!(next_magazine(&worn, rifle_id), Some(ten_id));
    worn.find_mut(vest_id).unwrap().grids[2].push((Place { x: 1, y: 0, turned: false }, twenty));
    assert_eq!(next_magazine(&worn, rifle_id), Some(twenty_id));
    // With nothing within reach that it takes, none.
    let (mut bare, _, _) = dressed();
    let (rifle_id, _) = rifle_worn(&mut bare, 3);
    assert_eq!(next_magazine(&bare, rifle_id), None);
}

/// The plain reload's old magazine goes to the smallest grid of the
/// vest and the pockets with room, keeping what is left in it; the
/// quick one's, or one that finds no room, is handed back to be let
/// fall. The new one is in the weapon either way.
#[test]
fn the_old_magazine_is_put_away_or_let_fall() {
    let (mut worn, [vest_id, pockets_id, _], _) = dressed();
    let (rifle_id, fitted_id) = rifle_worn(&mut worn, 30);
    let next = loaded("mag", 30);
    let next_id = next.id;
    worn.find_mut(vest_id).unwrap().put(0, next, Place { x: 0, y: 0, turned: false }).unwrap();
    // The pockets' grids are one cell each: the magazine (one by two)
    // goes to the vest's second grid, the smallest with room.
    assert_eq!(swap_magazine(&mut worn, rifle_id, Some(next_id), Some(12), false), None);
    assert_eq!(worn.holder_of(fitted_id), Some((vest_id, Spot::Cell(1, Place { x: 0, y: 0, turned: false }))));
    assert_eq!(magazine_rounds(worn.find(fitted_id).unwrap()), 12);
    assert_eq!(worn.holder_of(next_id), Some((rifle_id, Spot::Slot(0))));
    assert!(worn.find(pockets_id).unwrap().grids.iter().all(Vec::is_empty));
    // The quick way it is handed back with its rounds, though there is room.
    let back = swap_magazine(&mut worn, rifle_id, Some(fitted_id), Some(4), true).expect("let fall");
    assert_eq!((back.id, magazine_rounds(&back)), (next_id, 4));
    assert_eq!(worn.holder_of(fitted_id), Some((rifle_id, Spot::Slot(0))));
    // With no room it falls from a plain reload too.
    let (mut full, [vest_id, _, _], _) = dressed();
    let (rifle_id, fitted_id) = rifle_worn(&mut full, 5);
    for grid in 0..3 {
        full.find_mut(vest_id).unwrap().grids[grid].push((Place { x: 0, y: 0, turned: false }, thing("brick", "item", 1.0, [2, 2])));
    }
    assert_eq!(swap_magazine(&mut full, rifle_id, None, Some(5), false).map(|old| old.id), Some(fitted_id));
}

/// The reload key's last resort finds its round (`LoadAmmoToChamber`):
/// the first stack directly in the pockets, then the vest, whose round
/// the first chamber takes, known or not; not the backpack's, nor one the
/// chamber refuses. Let go on the weapon, one is split off into the
/// chamber. A weapon with no chamber of its own finds none.
#[test]
fn the_reload_key_s_last_resort_finds_a_round_for_the_chamber() {
    let (mut worn, [vest_id, pockets_id, bag_id], _) = dressed();
    let slots = vec![holder(MAGAZINE_SLOT, [0, 0], &["mag"], &[]), holder(CHAMBER, [0, 0], &["round"], &["refused"])];
    let rifle = Item::new(Arc::new(Kind { slots, ..kind("rifle", "weapon", 3.0, [4, 1]) }));
    let rifle_id = rifle.id;
    worn.slots[0] = Some(rifle);
    assert_eq!(round_for_chamber(&worn, rifle_id), None, "nothing within reach");
    let refused = Item::new(Arc::new(Kind { classes: vec!["refused".into(), "round".into(), "ammo".into(), "item".into()], ..kind("refused", "ammo", 0.01, [1, 1]) }));
    let (in_bag, in_vest, other) = (stack("round", "ammo", 20), stack("round", "ammo", 7), stack("shell", "ammo", 5));
    let (refused_id, vest_rounds) = (refused.id, in_vest.id);
    for (holder, item) in [(bag_id, in_bag), (pockets_id, refused), (pockets_id, other), (vest_id, in_vest)] {
        worn.find_mut(holder).unwrap().stow(item).unwrap();
    }
    assert!(worn.find(refused_id).is_some());
    assert_eq!(round_for_chamber(&worn, rifle_id), Some(vest_rounds), "the pockets' refused and other rounds passed over");
    assert_eq!(apply_item(&mut [&mut worn], vest_rounds, rifle_id, &RAID), Ok(Applied::Put(Spot::Slot(1))));
    assert_eq!(worn.find(vest_rounds).map(|stack| stack.count), Some(6));
    assert!(chambered(worn.find(rifle_id).unwrap()).is_some());
    let slots = vec![holder(MAGAZINE_SLOT, [0, 0], &["mag"], &[])];
    let open = Item::new(Arc::new(Kind { slots, ..kind("open", "weapon", 3.0, [4, 1]) }));
    let open_id = open.id;
    worn.slots[1] = Some(open);
    assert_eq!(round_for_chamber(&worn, open_id), None, "no chamber of its own");
}

/// The magazine's unload key moves the fitted magazine where a plain
/// reload's old one goes, the smallest grid of the vest and the pockets
/// with room, by the moving's rules (firing spec, "The magazine's and
/// the chamber's keys"): refused with the weapon's hands busy, and with
/// no room nothing moves.
#[test]
fn the_unload_key_puts_the_magazine_in_the_rig() {
    let (mut worn, [vest_id, _, _], _) = dressed();
    let (rifle_id, fitted_id) = rifle_worn(&mut worn, 30);
    let busy = Rules { busy_weapon: Some(rifle_id), ..RAID };
    assert_eq!(unload_to_rig(&mut worn, fitted_id, &busy), Err(Refusal::HandsBusy));
    assert_eq!(worn.holder_of(fitted_id), Some((rifle_id, Spot::Slot(0))));
    // The pockets' cells are too small for it (one by two): the vest's
    // first grid, of the two smallest with room.
    assert_eq!(unload_to_rig(&mut worn, fitted_id, &RAID), Ok(()));
    assert_eq!(worn.holder_of(fitted_id), Some((vest_id, Spot::Cell(0, Place { x: 0, y: 0, turned: false }))));
    assert_eq!(magazine_rounds(worn.find(fitted_id).unwrap()), 30);
    let (mut full, [vest_id, _, _], _) = dressed();
    let (rifle_id, fitted_id) = rifle_worn(&mut full, 5);
    for grid in 0..3 {
        full.find_mut(vest_id).unwrap().grids[grid].push((Place { x: 0, y: 0, turned: false }, thing("brick", "item", 1.0, [2, 2])));
    }
    assert_eq!(unload_to_rig(&mut full, fitted_id, &RAID), Err(Refusal::NoRoom));
    assert_eq!(full.holder_of(fitted_id), Some((rifle_id, Spot::Slot(0))));
}

/// A magazine is loaded a round at a time from a stack, onto its top
/// stack where the round is the same, as many as it has room for; it
/// refuses a round its filter does not take, and one more when full. It
/// is unloaded from the top, each round onto a stack of its kind with
/// room in the vest, else into a free cell; a round with nowhere to go
/// stays in.
#[test]
fn a_magazine_is_loaded_and_unloaded_a_round_at_a_time() {
    let (mut worn, [vest_id, _, _], _) = dressed();
    let worn_id = worn.id;
    let round = Arc::new(Kind { stack_max: 60, ..kind("round", "ammo", 0.01, [1, 1]) });
    let slug = Arc::new(Kind { stack_max: 60, classes: vec!["slug".into(), "round".into(), "ammo".into(), "item".into()], ..kind("slug", "ammo", 0.03, [1, 1]) });
    let magazine = Item::new(Arc::new(Kind { cartridges: vec![holder("cartridges", [0, 0], &["round"], &[])], capacity: 3, ..kind("mag", "magazine", 0.2, [1, 2]) }));
    let (mut stack, mut slugs, shells) = (Item::new(round.clone()), Item::new(slug), thing("shell", "ammo", 0.04, [1, 1]));
    (stack.count, slugs.count) = (2, 5);
    let (mag_id, stack_id, slugs_id, shells_id) = (magazine.id, stack.id, slugs.id, shells.id);
    let vest = worn.find_mut(vest_id).unwrap();
    vest.put(0, magazine, Place { x: 0, y: 0, turned: false }).unwrap();
    vest.put(2, stack, Place { x: 0, y: 0, turned: false }).unwrap();
    vest.put(2, slugs, Place { x: 1, y: 0, turned: false }).unwrap();
    vest.put(2, shells, Place { x: 0, y: 1, turned: false }).unwrap();
    assert_eq!(rounds_to_load(worn.find(mag_id).unwrap(), worn.find(stack_id).unwrap()), 2);
    assert_eq!(rounds_to_load(worn.find(mag_id).unwrap(), worn.find(slugs_id).unwrap()), 3);
    for _ in 0..2 {
        load_round(&mut [&mut worn], stack_id, mag_id).unwrap();
    }
    assert_eq!(load_round(&mut [&mut worn], shells_id, mag_id), Err(Refusal::Filter));
    // What a drop of rounds on it comes to (`Magazine.Apply`): how many
    // go in, the wrong round, and, asked after that, a full magazine.
    let onto = |worn: &Item, rounds: Id| rounds_onto_magazine(worn.find(mag_id).unwrap(), worn.find(rounds).unwrap());
    assert_eq!(onto(&worn, slugs_id), Some(Ok(1)));
    assert_eq!(onto(&worn, shells_id), Some(Err(Refusal::WrongAmmo)));
    assert_eq!(rounds_onto_magazine(worn.find(vest_id).unwrap(), worn.find(slugs_id).unwrap()), None, "no magazine");
    assert_eq!(onto(&worn, vest_id), None, "no rounds");
    load_round(&mut [&mut worn], slugs_id, mag_id).unwrap();
    assert_eq!(onto(&worn, slugs_id), Some(Err(Refusal::MagazineFull)));
    assert_eq!(onto(&worn, shells_id), Some(Err(Refusal::WrongAmmo)), "the wrong round before the room");
    assert_eq!(load_round(&mut [&mut worn], slugs_id, mag_id), Err(Refusal::NoRoom));
    let loaded = worn.find(mag_id).unwrap();
    assert_eq!(loaded.loose.iter().map(|stack| (stack.kind.name.as_str(), stack.count)).collect::<Vec<_>>(), [("round", 2), ("slug", 1)]);
    assert!(worn.find(stack_id).is_none());
    assert_eq!(worn.find(slugs_id).unwrap().count, 4);
    // The slug on top goes onto the slugs' stack; the rounds, whose
    // stack is gone, into the vest's first free cell, then onto it.
    assert_eq!(unload_round(&mut [&mut worn], mag_id, worn_id, &RAID), Ok((slugs_id, Applied::Merged)));
    assert_eq!(worn.find(slugs_id).unwrap().count, 5);
    let (put, _) = unload_round(&mut [&mut worn], mag_id, worn_id, &RAID).unwrap();
    assert_eq!(put, vest_id);
    let new_stack = worn.find(vest_id).unwrap().grids.iter().flatten().find(|(_, item)| item.kind.name == "round").map(|(_, item)| item.id).unwrap();
    assert_eq!(unload_round(&mut [&mut worn], mag_id, worn_id, &RAID), Ok((new_stack, Applied::Merged)));
    assert_eq!((worn.find(new_stack).unwrap().count, magazine_rounds(worn.find(mag_id).unwrap())), (2, 0));
    assert_eq!(unload_round(&mut [&mut worn], mag_id, worn_id, &RAID), Err(Refusal::NoAction));
    // With every cell taken the round stays in its magazine.
    let (mut full, [vest_id, pockets_id, bag_id], _) = dressed();
    let mut magazine = Item::new(Arc::new(Kind { cartridges: vec![holder("cartridges", [0, 0], &["round"], &[])], capacity: 3, ..kind("mag", "magazine", 0.2, [1, 2]) }));
    fill_magazine(&mut magazine, round, 3);
    let (mag_id, full_id) = (magazine.id, full.id);
    full.find_mut(vest_id).unwrap().put(0, magazine, Place { x: 0, y: 0, turned: false }).unwrap();
    for (holder, grid, cells) in [(vest_id, 1, [1, 2]), (vest_id, 2, [2, 2]), (pockets_id, 0, [1, 1]), (pockets_id, 1, [1, 1]), (bag_id, 0, [4, 4])] {
        let Some(holder) = full.find_mut(holder) else { continue };
        holder.grids[grid].push((Place { x: 0, y: 0, turned: false }, thing("brick", "item", 1.0, cells)));
    }
    assert!(unload_round(&mut [&mut full], mag_id, full_id, &RAID).is_err());
    assert_eq!(magazine_rounds(full.find(mag_id).unwrap()), 3);
}

/// The menu's Reload on a weapon swaps its magazine for the fullest
/// within reach at once, the old one to the smallest grid with room of
/// the vest, the pockets and the backpack; with none fitted it names the
/// found one to be moved in; with nothing to take it is refused. Install
/// puts a magazine into the weapon in hand's empty slot, or swaps the
/// one there out to the first place for it.
#[test]
fn the_menu_swaps_or_fits_a_weapon_s_magazine() {
    let place = |x, y| Place { x, y, turned: false };
    let (mut worn, [vest_id, _, bag_id], _) = dressed();
    let (rifle_id, fitted_id) = rifle_worn(&mut worn, 12);
    let next = loaded("mag", 30);
    let next_id = next.id;
    worn.find_mut(vest_id).unwrap().put(0, next, place(0, 0)).unwrap();
    assert_eq!(menu_reload(&mut worn, rifle_id), Ok(MenuReload::Swapped(fitted_id, next_id)));
    assert_eq!(worn.holder_of(next_id), Some((rifle_id, Spot::Slot(0))));
    // Its place found before the swap, as the game finds it: the new one
    // still fills the vest's first grid, so the second, as small.
    assert_eq!(worn.holder_of(fitted_id), Some((vest_id, Spot::Cell(1, place(0, 0)))));
    assert_eq!(magazine_rounds(worn.find(fitted_id).unwrap()), 12);
    // None fitted: the found one is named, to be moved in.
    worn.find_mut(rifle_id).unwrap().slots[0] = None;
    assert_eq!(menu_reload(&mut worn, rifle_id), Ok(MenuReload::Fit(fitted_id)));
    // Nothing with a round within reach: refused.
    let (mut bare, _, _) = dressed();
    let (bare_rifle, _) = rifle_worn(&mut bare, 0);
    assert_eq!(menu_reload(&mut bare, bare_rifle), Err(Refusal::NoAction));
    // Install: into the empty slot of the weapon in hand, by a move.
    assert_eq!(menu_install(&mut worn, fitted_id, Some(rifle_id)), Ok(MenuInstall::Fit(rifle_id, 0)));
    // With one fitted, swapped at once; the old one to the first place
    // for it, the vest first.
    let other = loaded("mag", 7);
    let other_id = other.id;
    worn.find_mut(vest_id).unwrap().put(2, other, place(0, 0)).unwrap();
    worn.find_mut(rifle_id).unwrap().slots[0] = worn.take(fitted_id);
    assert_eq!(menu_install(&mut worn, other_id, Some(rifle_id)), Ok(MenuInstall::Swapped(rifle_id, fitted_id)));
    assert_eq!(worn.holder_of(other_id), Some((rifle_id, Spot::Slot(0))));
    assert_eq!(worn.holder_of(fitted_id).map(|(holder, _)| holder), Some(vest_id));
    // While the weapon's hands are busy, nothing leaves it or goes in.
    let busy = Rules { busy_weapon: Some(rifle_id), ..RAID };
    let worn_id = worn.id;
    assert_eq!(quick_find(&mut [&mut worn], other_id, &[worn_id], Order::UNLOAD_WEAPON, &busy), Err(Refusal::HandsBusy));
    assert_eq!(worn.holder_of(other_id), Some((rifle_id, Spot::Slot(0))));
    assert!(quick_find(&mut [&mut worn], other_id, &[worn_id], Order::UNLOAD_WEAPON, &RAID).is_ok());
    assert_eq!(move_item(&mut [&mut worn], fitted_id, rifle_id, Spot::Slot(0), &busy), Err(Refusal::HandsBusy));
    // Nor does the weapon itself move, nor is it put down.
    assert_eq!(move_item(&mut [&mut worn], rifle_id, bag_id, Spot::Cell(0, place(0, 0)), &busy), Err(Refusal::HandsBusy));
    assert_eq!(take_out(&mut [&mut worn], rifle_id, &busy).err(), Some(Refusal::HandsBusy));
    // A magazine no weapon takes.
    let drum = loaded("drum", 50);
    let drum_id = drum.id;
    worn.find_mut(bag_id).unwrap().put(0, drum, place(0, 0)).unwrap();
    assert_eq!(menu_install(&mut worn, drum_id, Some(rifle_id)), Err(Refusal::NoAction));
}

#[test]
fn a_magazine_s_rounds_go_from_the_top() {
    let mut magazine = loaded("mag", 10);
    magazine.loose.push(Item { count: 5, ..again(&magazine.loose[0]) });
    set_magazine_rounds(&mut magazine, 12);
    assert_eq!(magazine.loose.iter().map(|stack| stack.count).collect::<Vec<_>>(), [10, 2]);
    set_magazine_rounds(&mut magazine, 7);
    assert_eq!(magazine.loose.iter().map(|stack| stack.count).collect::<Vec<_>>(), [7]);
    set_magazine_rounds(&mut magazine, 9);
    assert_eq!(magazine_rounds(&magazine), 9);
    set_magazine_rounds(&mut magazine, 0);
    assert!(magazine.loose.is_empty());
}
