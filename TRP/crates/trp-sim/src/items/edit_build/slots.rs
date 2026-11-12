//! Mod classes, build slots, which parts fit and assembling a build.

use super::*;

/// What sort of slot a slot is (`EModClass`), which its colour, its
/// toggle and its icon go by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModClass {
    /// A part its parent cannot do without (`Master`).
    Vital,
    Functional,
    Gear,
}

/// The game's mod types (`EWeaponModType`) that are gear
/// (`ItemViewFactory.GetSlotModClass`); every other slot is functional,
/// one whose name is not among the game's types too.
const GEAR: [&str; 10] = ["mod_mount", "mod_stock", "mod_magazine", "mod_barrel", "mod_handguard", "mod_reciever", "mod_charge", "mod_pistol_grip", "mod_launcher", "mod_mag_shaft"];

/// A slot's class: vital where its parent cannot do without it
/// (`_required`), else by its name.
pub fn slot_class(name: &str, required: bool) -> ModClass {
    if required {
        ModClass::Vital
    } else if GEAR.contains(&name) {
        ModClass::Gear
    } else {
        ModClass::Functional
    }
}

/// The words a gear or vital slot is named by (`GetModSlotName`, to be
/// looked up in the game's texts and put in capitals): its own name but
/// for those the game names otherwise. A name that is not one of the
/// game's mod types is shown as a tactical slot's (`mod_tactical`), as the
/// game's parse falls back to it.
pub fn slot_words(name: &str) -> &str {
    match name {
        "mod_magazine" => "Magazine",
        "mod_stock" => "Stock",
        "mod_foregrip" => "Foregrip",
        "mod_barrel" => "Barrel",
        "mod_charge" => "Charge",
        "mod_reciever" => "Receiver",
        "mod_mag_shaft" => "Shaft",
        "mod_pistol_grip" => "PGrip",
        _ if TYPES.contains(&name) => name,
        _ => "mod_tactical",
    }
}

/// The game's mod types by name (`EWeaponModType`).
const TYPES: [&str; 29] = [
    "mod_mount", "mod_scope", "mod_tactical", "mod_stock", "mod_magazine", "mod_barrel", "mod_handguard", "mod_muzzle", "mod_sight_front", "mod_sight_rear", "mod_foregrip", "mod_reciever", "mod_charge", "mod_pistol_grip", "mod_launcher",
    "mod_bipod", "mod_mag_shaft", "mod_silencer", "mod_tactical_2", "chamber0", "chamber1", "patron_in_weapon", "mod_gas_block", "mod_equipment", "mod_equipment_000", "mod_equipment_001", "mod_nvg", "mod_flashlight", "mod_muzzle_001",
];

/// Whether a slot is one of a weapon's chambers, which the screen shows no
/// view for (`CreateModSlots`).
fn is_chamber(name: &str) -> bool {
    name.starts_with("chamber") || name.starts_with("patron_in_weapon") || name.starts_with("camora")
}

/// A slot of a build: the item it is of (the weapon or a part, by its
/// number), its place among that item's slots, its name, its class and
/// what is in it.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildSlot {
    pub owner: Id,
    pub index: usize,
    pub name: String,
    pub class: ModClass,
    pub part: Option<Id>,
    /// Its parent cannot do without it and it is empty: drawn red.
    pub missing: bool,
    /// Its filter locks it (`Slot.Locked`: an armour's soft inserts, the
    /// aramid ones with their purple icons): its view faded, no dropdown,
    /// nothing put in or taken out (`ModdingScreenSlotView.Show`,
    /// `HoverStartHandler`).
    pub locked: bool,
}

/// Whether what is in a slot is a part of the build: the game's are its
/// `Mod`s (`EditBuildScreen.GetItemsInBuild`), so not the rounds in a
/// weapon's chambers and cylinders, the only other things a weapon's slots
/// hold; the armor bench's (the app's own edit build screen for the helmet
/// or armour worn) anything else in a slot too, the plates,
/// the soft armour's inserts, a face shield, a headset, which are no `Mod`.
/// A round is the one thing left out of either.
pub fn is_part(item: &Item) -> bool {
    !is(item, "Ammo")
}

/// Every slot of a build but the chambers, the weapon's first, each
/// part's after its own slot, in its slots' order (`Weapon.AllSlots`).
pub fn build_slots(weapon: &Item) -> Vec<BuildSlot> {
    fn of(item: &Item, out: &mut Vec<BuildSlot>) {
        for (index, (holder, part)) in item.kind.slots.iter().zip(&item.slots).enumerate() {
            if is_chamber(&holder.name) {
                continue;
            }
            let part = part.as_ref().filter(|part| is_part(part));
            out.push(BuildSlot { owner: item.id, index, name: holder.name.clone(), class: slot_class(&holder.name, holder.required), part: part.map(|part| part.id), missing: holder.required && part.is_none(), locked: holder.locked });
            if let Some(part) = part {
                of(part, out);
            }
        }
    }
    let mut out = Vec::new();
    of(weapon, &mut out);
    out
}

/// Every part of the game a slot could take (`ItemFactory.
/// CreateAllModsEver`): each kind of part that is an item.
pub fn all_parts(kinds: impl IntoIterator<Item = Arc<Kind>>) -> Vec<Arc<Kind>> {
    kinds.into_iter().filter(is_mod).collect()
}

/// Whether a kind is a part of a weapon's (`Mod`) that is an item.
pub fn is_mod(kind: &Arc<Kind>) -> bool {
    kind.is_item && kind.class_names.iter().any(|class| class == "Mod")
}

/// Every thing of the game a slot of a helmet or of armour could take at
/// the armor bench (the weapon's screen copied over for armour): any item
/// but a round, each slot's own filter choosing
/// among them (`takes`), as the game's modding screen offers what fits from
/// the stash (`WeaponModdingManipulation`), here every thing there is.
pub fn all_gear_parts(kinds: impl IntoIterator<Item = Arc<Kind>>) -> Vec<Arc<Kind>> {
    kinds.into_iter().filter(|kind| kind.is_item && !kind.class_names.iter().any(|class| class == "Ammo")).collect()
}

/// Whether the slot of that item of the build lists a part of that kind in
/// its dropdown (`EditBuildManipulation.GetItemCollections`: `Slot.
/// CanAccept`): the slot's filter names the part or a class of it and its
/// excluded list does not (`ItemFilterExtension.CheckItem`). A part so
/// listed may still be refused (`refusal`): the game shows it dim.
pub fn takes(weapon: &Item, owner: Id, index: usize, part: &Kind) -> bool {
    let Some(holder) = weapon.find(owner).and_then(|item| item.kind.slots.get(index)) else { return false };
    part.among(&holder.takes) && !part.among(&holder.refuses)
}

/// Why the slot of that item of the build would refuse a bare part of that
/// kind in place of what it holds (`DropdownManipulation.SelectCheck`: the
/// part there taken out, with what hangs on it, and the new one put in,
/// `Slot.CheckConditions`): its lock, then the conflicting items of the
/// whole build (`CheckConflictingItems`). None where it would go in.
pub fn refusal(weapon: &Item, owner: Id, index: usize, part: &Arc<Kind>) -> Option<Refusal> {
    let item = weapon.find(owner)?;
    let holder = item.kind.slots.get(index)?;
    if holder.locked {
        return Some(Refusal::Locked);
    }
    let leaving = item.slots.get(index).and_then(Option::as_ref).map(|old| old.id);
    conflict_at(weapon, owner, index, &Item::new(part.clone()), leaving).err()
}

/// Whether the slot both lists the part and takes it (`takes`, and no
/// `refusal`).
pub fn fits(weapon: &Item, owner: Id, index: usize, part: &Arc<Kind>) -> bool {
    takes(weapon, owner, index, part) && refusal(weapon, owner, index, part).is_none()
}

/// The build with a bare part of that kind in the slot of that item (none:
/// emptied), what was there taken out with what hung on it
/// (`EditBuildManipulation.Select`). Whether it was done.
pub fn select(build: &mut Item, owner: Id, index: usize, part: Option<&Arc<Kind>>) -> bool {
    let Some(item) = build.find_mut(owner) else { return false };
    let Some(slot) = item.slots.get_mut(index) else { return false };
    *slot = part.map(|kind| Item::new(kind.clone()));
    true
}

/// Whether a weapon already has a build's parts (`WeaponAssembler.
/// CheckIfAlreadyBuilt`): as many, and no template among either's that is
/// not among the other's.
pub fn already_built(weapon: &Item, build: &Item) -> bool {
    let (have, want) = (part_templates(weapon), part_templates(build));
    if have.len() != want.len() {
        return false;
    }
    let mut union: Vec<&String> = have.iter().chain(&want).collect();
    union.sort();
    union.dedup();
    union.len() == have.len()
}

/// The templates of every part of an item, at any depth.
fn part_templates(item: &Item) -> Vec<String> {
    fn of(item: &Item, out: &mut Vec<String>) {
        for part in item.slots.iter().flatten().filter(|part| is_part(part)) {
            out.push(part.kind.template.clone());
            of(part, out);
        }
    }
    let mut out = Vec::new();
    of(item, &mut out);
    out
}

/// Whether gear already has a build's parts at the armor bench (the
/// app's own rule): the same things, as many of each. The game's check for a
/// weapon (`already_built`, `CheckIfAlreadyBuilt`) counts a build with two
/// of one thing as never built, which a carrier's two like plates always
/// are; the app's own for gear, whose assembling is the app's.
pub fn same_parts(item: &Item, build: &Item) -> bool {
    let (mut have, mut want) = (part_templates(item), part_templates(build));
    have.sort();
    want.sort();
    have == want
}

/// A build put on the weapon (`WeaponAssembler.AssembleAsync`): every part
/// the weapon has taken off, its own parts with it; each part of the build,
/// slot by slot from the weapon down, one of those taken off of its
/// template where there is one (kept as it was: a magazine's rounds, a
/// fold, a switch), else a new bare one; what is left of those taken off
/// goes (the game's into the stash; the app's: nowhere). The weapon's own
/// chambers keep their rounds. The parts that went, by their numbers.
pub fn assemble(weapon: &mut Item, build: &Item) -> Vec<Id> {
    fn take_off(item: &mut Item, pool: &mut Vec<Item>) {
        let parts: Vec<usize> = (0..item.slots.len()).filter(|&at| item.slots[at].as_ref().is_some_and(is_part)).collect();
        for at in parts {
            if let Some(mut part) = item.slots[at].take() {
                take_off(&mut part, pool);
                pool.push(part);
            }
        }
    }
    fn put_on(item: &mut Item, wanted: &Item, pool: &mut Vec<Item>) {
        for (at, part) in wanted.slots.iter().enumerate() {
            let Some(part) = part.as_ref().filter(|part| is_part(part)) else { continue };
            let mut placed = match pool.iter().position(|old| old.kind.template == part.kind.template) {
                Some(found) => pool.swap_remove(found),
                None => Item::new(part.kind.clone()),
            };
            put_on(&mut placed, part, pool);
            if let Some(slot) = item.slots.get_mut(at) {
                *slot = Some(placed);
            }
        }
    }
    let mut pool = Vec::new();
    take_off(weapon, &mut pool);
    put_on(weapon, build, &mut pool);
    pool.iter().map(|part| part.id).collect()
}
