//! Choosing, unloading and swapping magazines.

use super::*;

/// What the magazine picker offers for a weapon carried (`AmmoSelector.
/// FindAmmoForWeapon`; firing spec, "The magazine picker"): what lies
/// directly in the fast-access slots' grids (`GetReachableItemsOfType`:
/// the pockets' and then the vest's, each grid's things in its own order),
/// not sorted, by the weapon's way of being loaded (`reload_mode`):
/// - a magazine that comes out: the magazines that hold a round, known,
///   that its magazine slot takes (`CheckMoveIgnoringTargetItem`);
/// - barrels: the stacks of rounds, known, that its first chamber takes;
/// - a magazine of its own fitted with one that comes out: those
///   magazines, then those rounds;
/// - a magazine of its own: with none fitted, the magazines; not full, the
///   rounds its first chamber takes (a revolver's, those its cylinder
///   takes: `CylinderMagazine.CheckCompatibility`); full, nothing.
///
/// None where the fitted magazine is not known (no magazine goes in or
/// out of the weapon then: `magazine_for`). Empty: no picker.
pub fn picker_choices(equipment: &Item, weapon: Id) -> Vec<Id> {
    let Some(gun) = equipment.find(weapon) else { return Vec::new() };
    let reachable = || WITHIN_REACH.iter().filter_map(|slot| equipment.in_slot(slot)).flat_map(|holder| holder.grids.iter().flatten()).map(|(_, item)| item);
    let fitted = gun.in_slot(MAGAZINE_SLOT);
    let magazines = || -> Vec<Id> {
        let Some(takes) = gun.slot_index(MAGAZINE_SLOT).and_then(|slot| gun.kind.slots.get(slot)).map(|slot| &slot.takes) else { return Vec::new() };
        if fitted.is_some_and(|fitted| !crate::items::examine::known(fitted)) {
            return Vec::new();
        }
        reachable().filter(|item| is(item, "Magazine") && item.count == 1 && magazine_rounds(item) > 0 && item.kind.among(takes) && crate::items::examine::known(item)).map(|item| item.id).collect()
    };
    let rounds = || -> Vec<Id> {
        let takes = if is(gun, "Revolver") {
            fitted.and_then(|cylinder| cylinder.kind.slots.iter().find(|slot| slot.name.starts_with(CAMORA))).map(|camora| camora.takes.clone())
        } else {
            gun.kind.slots.iter().find(|slot| is_chamber(&slot.name)).map(|chamber| chamber.takes.clone())
        };
        let Some(takes) = takes else { return Vec::new() };
        reachable().filter(|item| is(item, "Ammo") && item.count > 0 && item.kind.among(&takes) && crate::items::examine::known(item)).map(|item| item.id).collect()
    };
    match reload_mode(gun) {
        "ExternalMagazine" => magazines(),
        "OnlyBarrel" => rounds(),
        MIXED_RELOAD => [magazines(), rounds()].concat(),
        "InternalMagazine" => match fitted {
            None => magazines(),
            Some(magazine) if magazine_rounds(magazine) < magazine.kind.capacity => rounds(),
            Some(_) => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// The magazine a reload takes for a weapon carried
/// (`FirearmHandsInputTranslator.ReloadExternalMagazine`; firing spec,
/// "The quick reload"): of the magazines lying directly in the pockets'
/// and the vest's grids, in that order, those holding a round, the
/// fullest first, and of them the first the weapon's magazine slot
/// takes. Of two that hold the same, the one met first (`PROVISIONAL`:
/// the game's sort keeps no order between them).
pub fn next_magazine(equipment: &Item, weapon: Id) -> Option<Id> {
    magazine_for(equipment, equipment.find(weapon)?)
}

/// The same for a weapon wherever it lies (the menu's Load and Reload:
/// `ItemUiContext.FindSuitableMagazine`, the magazines within reach).
pub fn magazine_for(equipment: &Item, weapon: &Item) -> Option<Id> {
    let slot = weapon.slot_index(MAGAZINE_SLOT)?;
    let takes = &weapon.kind.slots.get(slot)?.takes;
    // A magazine not known goes into no weapon, nor comes out of one
    // carried (`Slot.CheckConditions`, `RemoveItemInternal`, by the move
    // each is tried with): none is found.
    if weapon.slots[slot].as_ref().is_some_and(|attached| !crate::items::examine::known(attached)) {
        return None;
    }
    let worn = WITHIN_REACH.iter().filter_map(|slot| equipment.in_slot(slot));
    let mut found: Vec<&Item> = worn
        .flat_map(|holder| holder.grids.iter().flatten())
        .map(|(_, item)| item)
        .filter(|item| is(item, "Magazine") && magazine_rounds(item) > 0)
        .collect();
    found.sort_by_key(|magazine| std::cmp::Reverse(magazine_rounds(magazine)));
    found.into_iter().find(|magazine| magazine.kind.among(takes) && magazine.count == 1 && crate::items::examine::known(magazine)).map(|magazine| magazine.id)
}

/// Where the old magazine of a plain reload goes
/// (`GetPrioritizedGridsForUnloadedObject`, `FindLocationForItem`): the
/// first free place in the vest's grids and then the pockets', the
/// smallest grid first; the backpack is not among them. The holder, the
/// grid and the place.
fn unloaded_place(equipment: &Item, magazine: &Item) -> Option<(Id, usize, Place)> {
    unloaded_place_of(equipment, magazine, false)
}

/// The same, with the backpack's grids after the pockets' when asked
/// (`backpackIncluded`: the menu's Reload).
pub(super) fn unloaded_place_of(equipment: &Item, magazine: &Item, backpack: bool) -> Option<(Id, usize, Place)> {
    let slots: &[&str] = if backpack { &["TacticalVest", "Pockets", "Backpack"] } else { &["TacticalVest", "Pockets"] };
    let holders = slots.iter().filter_map(|slot| equipment.in_slot(slot));
    let mut grids: Vec<(&Item, usize)> = holders.flat_map(|holder| (0..holder.grids.len()).map(move |grid| (holder, grid))).collect();
    grids.sort_by_key(|(holder, grid)| holder.kind.grids[*grid].cells[0] * holder.kind.grids[*grid].cells[1]);
    grids.into_iter().find_map(|(holder, grid)| holder.find_room(grid, magazine).map(|place| (holder.id, grid, place)))
}

/// Moves what a weapon's unload key takes out of it to where the game puts
/// it (`FirearmHandsInputTranslator.UnloadMag`, `UnloadChamber`; firing
/// spec, "The magazine's and the chamber's keys"): the first free place in
/// the vest's grids and then the pockets', the smallest grid first, as a
/// move by the moving's rules (`ItemManipulator.Move`). No place:
/// `Refusal::NoRoom`; refused, nothing has changed.
pub fn unload_to_rig(equipment: &mut Item, id: Id, rules: &Rules) -> Result<(), Refusal> {
    let item = equipment.find(id).ok_or(Refusal::NoSuchPlace)?;
    let (holder, grid, place) = unloaded_place(equipment, item).ok_or(Refusal::NoRoom)?;
    move_item(&mut [equipment], id, holder, Spot::Cell(grid, place), rules)
}

/// The first of a weapon's chambers that holds a round
/// (`Weapon.FirstLoadedChamberSlot`): its number among the weapon's
/// chambers (0 for one chamber), and the round. None for a cylinder, whose
/// rounds are its magazine's.
pub fn first_loaded_chamber(weapon: &Item) -> Option<(usize, Id)> {
    let (cylinder, places) = own_chambers(weapon);
    if cylinder.is_some() {
        return None;
    }
    places.into_iter().enumerate().find_map(|(at, index)| weapon.slots.get(index)?.as_ref().map(|round| (at, round.id)))
}

/// Puts what a weapon gives up where the game puts it
/// (`placeToPutContainedAmmoMagazine`): an old magazine, or the live round
/// out of a single barrel, to the first free place in the vest's and the
/// pockets' grids, the smallest grid first. Hands it back where there is
/// none, to be thrown down.
pub fn put_unloaded(equipment: &mut Item, item: Item) -> Option<Item> {
    let Some((holder, grid, place)) = unloaded_place(equipment, &item) else { return Some(item) };
    match equipment.find_mut(holder) {
        Some(holder) => {
            holder.grids[grid].push((place, item));
            None
        }
        None => Some(item),
    }
}

/// The exchange a magazine's reload makes as it begins
/// (`ReloadExternalMagResult.Run`): the magazine fitted leaves the weapon
/// holding `left` rounds and is put where a plain reload puts it, unless
/// the reload is the quick one or there is no such place; then `next`
/// goes into the weapon's magazine slot. Returns the old magazine where
/// it has nowhere to go: it is to be let fall at the mark `MagHide`.
pub fn swap_magazine(equipment: &mut Item, weapon: Id, next: Option<Id>, left: Option<u32>, quick: bool) -> Option<Item> {
    let slot = equipment.find(weapon)?.slot_index(MAGAZINE_SLOT)?;
    let mut old = equipment.find_mut(weapon)?.slots[slot].take();
    if let (Some(old), Some(left)) = (old.as_mut(), left) {
        set_magazine_rounds(old, left);
    }
    let falls = match old {
        Some(old) if quick => Some(old),
        Some(old) => put_unloaded(equipment, old),
        None => None,
    };
    if let Some(next) = next.and_then(|id| equipment.take(id)) {
        if let Some(weapon) = equipment.find_mut(weapon) {
            weapon.slots[slot] = Some(next);
        }
    }
    falls
}
