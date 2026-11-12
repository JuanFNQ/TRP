//! Chambers, cylinder camoras and which rounds go together.

use super::*;

/// The name of a weapon's slot for a chambered round; a weapon with more
/// barrels than one numbers them after it (`patron_in_weapon_000`).
pub(super) const CHAMBER: &str = "patron_in_weapon";

/// What the name of each chamber of a cylinder begins with: slots of the
/// cylinder, which is the weapon's magazine (`CylinderMagazine.Camoras`).
pub(super) const CAMORA: &str = "camora_";

pub(super) fn is_chamber(slot: &str) -> bool {
    slot.starts_with(CHAMBER)
}

/// A cylinder's chambers (`CylinderMagazine.Camoras`), by their numbers:
/// its slots whose names begin `camora_`, each holding one round (more-
/// weapons spec, "The rounds and cases seen in chambers of their own").
/// None for any other magazine, whose rounds lie in its cartridges.
pub(super) fn camoras(magazine: &Item) -> impl Iterator<Item = usize> + '_ {
    magazine.kind.slots.iter().enumerate().filter(|(_, slot)| slot.name.starts_with(CAMORA)).map(|(index, _)| index)
}

/// Whether a magazine is a cylinder, its rounds in chambers of its own.
pub fn is_cylinder(magazine: &Item) -> bool {
    camoras(magazine).next().is_some()
}

/// A round into a cylinder's first empty chamber. Whether it had one.
pub(super) fn into_cylinder(cylinder: &mut Item, kind: Arc<Kind>) -> bool {
    let free = camoras(cylinder).find(|&index| cylinder.slots.get(index).is_some_and(Option::is_none));
    match free.and_then(|index| cylinder.slots.get_mut(index)) {
        Some(slot) => {
            *slot = Some(Item::new(kind));
            true
        }
        None => false,
    }
}

/// The places of a weapon's chambers of its own, by their numbers, and the
/// item they are slots of: a cylinder's (its magazine's `camora_` slots),
/// else the weapon's own (`patron_in_weapon`, or `_000` onward for
/// barrels). Read for a weapon whose rounds lie in chambers of their own.
pub(super) fn own_chambers(weapon: &Item) -> (Option<&Item>, Vec<usize>) {
    match weapon.in_slot(MAGAZINE_SLOT).filter(|magazine| is_cylinder(magazine)) {
        Some(cylinder) => (Some(cylinder), camoras(cylinder).collect()),
        None => (None, weapon.kind.slots.iter().enumerate().filter(|(_, slot)| is_chamber(&slot.name)).map(|(index, _)| index).collect()),
    }
}

/// The round in each of a weapon's chambers of its own (a cylinder's, or
/// its barrels'), by the chambers' numbers.
pub fn chamber_rounds(weapon: &Item) -> Vec<Option<Arc<Kind>>> {
    let (cylinder, places) = own_chambers(weapon);
    let holder = cylinder.unwrap_or(weapon);
    places.into_iter().map(|index| holder.slots.get(index).and_then(Option::as_ref).map(|round| round.kind.clone())).collect()
}

/// Puts a round of that kind into a chamber of its own of the weapon of
/// that number wherever it lies in `root`, or takes the one there away.
/// Whether the weapon and the chamber were there.
pub fn set_chamber_round(root: &mut Item, weapon: Id, chamber: usize, round: Option<Arc<Kind>>) -> bool {
    let Some(gun) = root.find_mut(weapon) else { return false };
    let (cylinder, places) = own_chambers(gun);
    let (Some(&index), cylinder) = (places.get(chamber), cylinder.map(|cylinder| cylinder.id)) else { return false };
    let holder = match cylinder {
        Some(id) => match gun.find_mut(id) {
            Some(cylinder) => cylinder,
            None => return false,
        },
        None => gun,
    };
    match holder.slots.get_mut(index) {
        Some(slot) => {
            *slot = round.map(Item::new);
            true
        }
        None => false,
    }
}

/// The case in each of a weapon's chambers of its own, by the chambers'
/// numbers, as many as `chamber_rounds` has (`Weapon.ShellsInChambers`).
pub fn chamber_cases(weapon: &Item) -> Vec<Option<Arc<Kind>>> {
    (0..own_chambers(weapon).1.len()).map(|chamber| weapon.shells.get(chamber).cloned().flatten()).collect()
}

/// Leaves a case of that kind in a chamber of its own of the weapon of that
/// number wherever it lies in `root`, or takes the one there away. Whether
/// the weapon and the chamber were there.
pub fn set_chamber_case(root: &mut Item, weapon: Id, chamber: usize, case: Option<Arc<Kind>>) -> bool {
    let Some(gun) = root.find_mut(weapon) else { return false };
    if chamber >= own_chambers(gun).1.len() {
        return false;
    }
    if gun.shells.len() <= chamber {
        gun.shells.resize(chamber + 1, None);
    }
    gun.shells[chamber] = case;
    true
}

/// Whether a thing's place counts its contents as part of the thing
/// (`ContainerExtensions.MergesWithChildren`): a place that says so
/// itself, or else the thing's own word; the chamber of a single barrel
/// never (`SlotSerializer`: `patron_in_weapon` alone).
pub(super) fn merges(holder: &Item, slot: &Holder) -> bool {
    slot.name != CHAMBER && (slot.merges == Some(true) || holder.kind.merges)
}

/// What a weapon's loose rounds are tested against when its reload key
/// looks for them (`FirearmHandsInputTranslator`): its chamber
/// (`ReloadWithAmmo`), the first of its barrels (`ReloadBarrels`), or for
/// a revolver, which has no chamber of the weapon's own, its cylinder's
/// (`ReloadRevolverDrum`, by `CylinderMagazine.CheckCompatibility`).
pub(super) fn takes_rounds(weapon: &Item) -> Option<Vec<String>> {
    if let Some(chamber) = weapon.kind.slots.iter().find(|slot| is_chamber(&slot.name)) {
        return Some(chamber.takes.clone());
    }
    let cylinder = weapon.in_slot("mod_magazine")?;
    cylinder.kind.slots.iter().find(|slot| slot.name.starts_with(CAMORA)).map(|camora| camora.takes.clone())
}

/// Whether two things go together, as the screen lights them (inventory
/// spec, "The box under the mouse, and what goes with it"): the one has a
/// place for the other, either way round. A weapon goes with its
/// magazines, its rounds and the parts that fit it or what is on it; a
/// magazine with the weapons it fits and its rounds. A thing does not go
/// with itself, and a grid's room is no place for this: a backpack would
/// go with everything.
pub fn go_together(one: &Item, other: &Item) -> bool {
    one.id != other.id && (has_place_for(one, &other.kind) || has_place_for(other, &one.kind))
}

/// Whether a thing, or a part on it, has a place for that kind of thing by
/// the game's filters: a slot (a chamber, a cylinder's chambers, a part's
/// slot) or a magazine's cartridges, which take what they list and do not
/// refuse.
fn has_place_for(holder: &Item, kind: &Kind) -> bool {
    let fits = |place: &Holder| kind.among(&place.takes) && !kind.among(&place.refuses);
    holder.kind.slots.iter().chain(&holder.kind.cartridges).any(fits) || holder.slots.iter().flatten().any(|part| has_place_for(part, kind))
}
