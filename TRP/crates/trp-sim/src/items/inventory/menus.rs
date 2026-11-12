//! The reload and install menus and applying an item.

use super::*;

/// What the menu's Reload on a weapon comes to (`ItemUiContext.
/// ReloadWeapon`; firing spec, "A magazine taken out or put in from the
/// inventory").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuReload {
    /// The fitted magazine (the first) swapped for the found one (the
    /// second) at once: two things moved, the hands not asked.
    Swapped(Id, Id),
    /// No magazine fitted: the found one is to be moved into the slot (a
    /// move of one thing, the hands' if the weapon is in hand).
    Fit(Id),
}

/// The menu's Reload on a weapon lying in `roots`: the fullest magazine
/// within reach of the character (`equipment`) that the weapon's slot
/// takes; with one fitted, the two swapped, the old one to the first free
/// place of the smallest grid of the vest, the pockets and the backpack
/// that has one. Refused with no magazine to take (`NoAction`), and with
/// nowhere for the old one (`NoRoom`: the game then throws it down, which
/// is not built).
pub fn menu_reload(equipment: &mut Item, weapon: Id) -> Result<MenuReload, Refusal> {
    let gun = equipment.find(weapon).ok_or(Refusal::NoSuchPlace)?;
    let next = magazine_for(equipment, gun).ok_or(Refusal::NoAction)?;
    let slot = gun.slot_index(MAGAZINE_SLOT).ok_or(Refusal::NoSuchPlace)?;
    let Some(old) = gun.slots[slot].as_ref() else { return Ok(MenuReload::Fit(next)) };
    let old_id = old.id;
    // The place is found with the old one still in the weapon, as the game
    // finds it.
    let (holder, grid, place) = unloaded_place_of(equipment, old, true).ok_or(Refusal::NoRoom)?;
    let new = equipment.take(next).ok_or(Refusal::NoSuchPlace)?;
    let old = equipment.find_mut(weapon).ok_or(Refusal::NoSuchPlace)?.slots[slot].replace(new).ok_or(Refusal::NoSuchPlace)?;
    equipment.find_mut(holder).ok_or(Refusal::NoSuchPlace)?.grids[grid].push((place, old));
    Ok(MenuReload::Swapped(old_id, next))
}

/// What the menu's Install on a magazine comes to (`ItemUiContext.
/// InstallMod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuInstall {
    /// Swapped at once for the magazine the weapon had (the hands not
    /// asked): the weapon, and the old magazine.
    Swapped(Id, Id),
    /// To be moved into the weapon's empty slot (the hands' if the weapon
    /// is in hand): the weapon and the slot.
    Fit(Id, usize),
}

/// The menu's Install on a magazine the character has: into the magazine
/// slot of the weapon in hand if it takes the magazine, else of the first
/// weapon carried whose slot does (`GetSuitableWeapons`); a slot holding
/// another has the two swapped, the old one to the first free place for it
/// in the equipment's containers in the order for loot (`QuickFind`,
/// `GetPrioritizedGridsForLoot`). Refused with no such weapon
/// (`NoAction`) or nowhere for the old one (`NoRoom`).
pub fn menu_install(equipment: &mut Item, magazine: Id, in_hand: Option<Id>) -> Result<MenuInstall, Refusal> {
    let item = equipment.find(magazine).ok_or(Refusal::NoSuchPlace)?;
    let accepts = |weapon: &Item| {
        let slot = weapon.slot_index(MAGAZINE_SLOT)?;
        (is(weapon, "Weapon") && item.kind.among(&weapon.kind.slots[slot].takes)).then_some((weapon.id, slot))
    };
    fn all(item: &Item) -> Vec<&Item> {
        item.contents().flat_map(|inside| std::iter::once(inside).chain(all(inside))).collect()
    }
    let found = in_hand.and_then(|id| equipment.find(id)).and_then(|weapon| accepts(weapon)).or_else(|| all(equipment).into_iter().find_map(|weapon| accepts(weapon)));
    let (weapon, slot) = found.ok_or(Refusal::NoAction)?;
    let gun = equipment.find(weapon).ok_or(Refusal::NoSuchPlace)?;
    let Some(old) = gun.slots[slot].as_ref() else { return Ok(MenuInstall::Fit(weapon, slot)) };
    if old.id == magazine {
        return Err(Refusal::AlreadyThere);
    }
    let old_id = old.id;
    let (holder, grid, place) = loot_place(equipment, old).ok_or(Refusal::NoRoom)?;
    let new = equipment.take(magazine).ok_or(Refusal::NoSuchPlace)?;
    let old = equipment.find_mut(weapon).ok_or(Refusal::NoSuchPlace)?.slots[slot].replace(new).ok_or(Refusal::NoSuchPlace)?;
    equipment.find_mut(holder).ok_or(Refusal::NoSuchPlace)?.grids[grid].push((place, old));
    Ok(MenuInstall::Swapped(weapon, old_id))
}

/// The first free place for a thing in the equipment's four containers
/// in the order for its kind (`GetPrioritizedGridsForLoot`,
/// `FindLocationForItem`): the holder, the grid and the place.
fn loot_place(equipment: &Item, item: &Item) -> Option<(Id, usize, Place)> {
    loot_order(item).iter().filter_map(|slot| equipment.in_slot(slot)).find_map(|holder| (0..holder.grids.len()).find_map(|grid| holder.find_room(grid, item).map(|place| (holder.id, grid, place))))
}

/// The slots a weapon is taken in hand from, in the order the game looks
/// through them for something to hold (`Player._slotPriority`).
pub const HAND_SLOTS: [&str; 4] = ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster", "Scabbard"];

/// Whether a thing is the round in the chamber of the weapon in hand whose
/// hands would not let it out now (`Rules::chamber_held`; `Idling.Execute`
/// refuses the move: "Can't perform operation while mag in the weapon").
pub(super) fn chamber_held(root: &Item, id: Id, rules: &Rules) -> bool {
    let Some(weapon) = rules.chamber_held else { return false };
    let Some((holder, Spot::Slot(slot))) = root.holder_of(id) else { return false };
    holder == weapon && root.find(holder).and_then(|gun| gun.kind.slots.get(slot)).is_some_and(|slot| is_chamber(&slot.name))
}

/// Whether a place is the chamber of the weapon in hand whose hands would
/// not take a round into it now (`Rules::chamber_closed`).
pub(super) fn chamber_closed(target: &Item, spot: Spot, rules: &Rules) -> bool {
    rules.chamber_closed == Some(target.id) && matches!(spot, Spot::Slot(slot) if target.kind.slots.get(slot).is_some_and(|holder| is_chamber(&holder.name)))
}

/// Drops one item on another (`ItemController.ExecutePossibleAction`,
/// `CompoundItem.Apply`): two stacks of one kind become one if the other
/// has the room; else it goes into the first of the other's slots that
/// takes it, a weapon's chambers last (`Weapon.AllSlots`) and a round of a
/// stack split off for a chamber (`SplitMax`), else into the first of its
/// grids with room; else as much of a stack as the other holds goes over.
/// Swapping two things by dropping one on the other is switched off in the
/// game.
pub fn apply_item(roots: &mut [&mut Item], id: Id, onto: Id, rules: &Rules) -> Result<Applied, Refusal> {
    if id == onto {
        return Err(Refusal::NoAction);
    }
    let (from, to) = (root_of(roots, id)?, root_of(roots, onto)?);
    if chamber_held(roots[from], id, rules) {
        return Err(Refusal::HandsBusy);
    }
    let (item, target) = (roots[from].find(id).ok_or(Refusal::NoSuchPlace)?, roots[to].find(onto).ok_or(Refusal::NoSuchPlace)?);
    let alike = item.kind.stack_max > 1 && item.kind.template == target.kind.template;
    let (count, room) = (item.count, target.stack_room());
    if alike && room >= count {
        stacked_within_limits(roots, from, to, id, count, rules)?;
        roots[from].take(id);
        roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += count;
        return Ok(Applied::Merged);
    }
    // A weapon takes nothing while it or the thing is not known
    // (`Weapon.Apply`), nor a magazine rounds (`Magazine.ApplyItem`); a
    // merge is asked first and never refused so.
    let applies = is(target, "Weapon") || (is(target, "Magazine") && is(item, "Ammo"));
    if applies && !(crate::items::examine::known(item) && crate::items::examine::known(target)) {
        return Err(Refusal::NotExamined { equipment: false });
    }
    let slots: Vec<usize> = (0..target.kind.slots.len())
        .filter(|slot| !is_chamber(&target.kind.slots[*slot].name) && target.slot_takes(*slot, item).is_ok())
        .collect();
    // A chamber takes one round: of a stack, one is split off for it.
    let one = Item { count: 1, ..item.clone() };
    let chambers: Vec<usize> = (0..target.kind.slots.len())
        .filter(|slot| is_chamber(&target.kind.slots[*slot].name) && target.slot_takes(*slot, &one).is_ok())
        .collect();
    let closed = chambers.iter().any(|slot| chamber_closed(target, Spot::Slot(*slot), rules));
    let many = count > 1;
    let cells = (0..target.kind.grids.len()).find_map(|grid| target.find_room(grid, item).map(|place| Spot::Cell(grid, place)));
    let mut refused = Refusal::NoAction;
    for spot in slots.into_iter().map(Spot::Slot).chain(chambers.into_iter().map(Spot::Slot)).chain(cells) {
        let chamber = matches!(spot, Spot::Slot(slot) if is_chamber(&roots[to].find(onto).map_or(String::new(), |target| target.kind.slots[slot].name.clone())));
        let done = if chamber && closed {
            Err(Refusal::HandsBusy)
        } else if chamber && many {
            split_item(roots, id, 1, onto, spot, rules).map(|_| ())
        } else {
            move_item(roots, id, onto, spot, rules)
        };
        match done {
            Ok(()) => return Ok(Applied::Put(spot)),
            Err(refusal) => refused = refusal,
        }
    }
    if alike && room > 0 {
        stacked_within_limits(roots, from, to, id, room, rules)?;
        roots[from].find_mut(id).ok_or(Refusal::NoSuchPlace)?.count -= room;
        roots[to].find_mut(onto).ok_or(Refusal::NoSuchPlace)?.count += room;
        return Ok(Applied::Topped(room));
    }
    Err(refused)
}
