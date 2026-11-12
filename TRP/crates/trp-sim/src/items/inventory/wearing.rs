//! The loot order, the slots to wear a kind in and fitting bits.

use super::*;

/// Whether an item is of one of the game's classes, by the database's
/// name for it (`Weapon`, `Magazine`, `Ammo` and so on: the game's own
/// item types go by the same names).
pub fn is(item: &Item, class: &str) -> bool {
    item.kind.class_names.iter().any(|name| name == class)
}

/// The four things worn that a thing is looked for room in, in the game's
/// order for that kind of thing (`GetPrioritizedContainersForLoot`):
/// rounds and magazines go to the vest first, money to the secured
/// container, grenades to the pockets, everything else to the backpack.
pub fn loot_order(item: &Item) -> [&'static str; 4] {
    if is(item, "Ammo") || is(item, "Magazine") {
        ["TacticalVest", "Pockets", "Backpack", "SecuredContainer"]
    } else if is(item, "Money") {
        ["SecuredContainer", "Backpack", "TacticalVest", "Pockets"]
    } else if is(item, "ThrowWeap") {
        ["Pockets", "TacticalVest", "Backpack", "SecuredContainer"]
    } else {
        ["Backpack", "TacticalVest", "Pockets", "SecuredContainer"]
    }
}

/// The equipment slots a thing is put on in, in the game's order for its
/// kind (`InventoryExtension.FindSlotToPickUp`, which asks the classes in
/// this order); none for what is neither worn nor held. Not told apart:
/// a flare gun, which the game sends to the holster, and what goes only
/// into the pockets' special slots.
fn slots_to_wear(item: &Item) -> &'static [&'static str] {
    slots_to_wear_kind(&item.kind)
}

/// The same of a kind of thing.
fn slots_to_wear_kind(kind: &Kind) -> &'static [&'static str] {
    const BY_CLASS: [(&str, &[&str]); 13] = [
        ("Armor", &["ArmorVest"]),
        ("Visors", &["Eyewear"]),
        ("Headwear", &["Headwear"]),
        ("FaceCover", &["FaceCover"]),
        ("Headphones", &["Earpiece"]),
        ("Backpack", &["Backpack"]),
        ("Vest", &["TacticalVest"]),
        ("Knife", &["Scabbard"]),
        ("ArmBand", &["ArmBand"]),
        ("Revolver", &["Holster", "FirstPrimaryWeapon", "SecondPrimaryWeapon"]),
        ("Pistol", &["Holster"]),
        ("Weapon", &["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster"]),
        ("MobContainer", &["SecuredContainer"]),
    ];
    BY_CLASS.iter().find(|(class, _)| kind.class_names.iter().any(|name| name == class)).map_or(&[], |(_, slots)| *slots)
}

/// The slots of the equipment whose things the head things' fittings go by
/// (`PlayerClippingRules.CalculateRules`, of `PlayerBody.SlotNames`).
pub const FITTING_SLOTS: [&str; 4] = ["Earpiece", "Eyewear", "Headwear", "FaceCover"];

/// The bits the things worn give the fittings of every thing on the head
/// (`PlayerClippingRules.CalculateRules`; body animator spec, "The head's
/// things fitted to one another"), each thing by the slot it is in: a face
/// cover gives the hat's and the helmet's, and the visor's unless it is a
/// half mask; eyewear the face cover's and the hat's; headwear that is not
/// a helmet (its material not `Helmet`) the headphones'; an earpiece the
/// hat's. The bits are `trp_data::dress::fit`'s.
pub fn fitting_bits<'a>(worn: impl IntoIterator<Item = (&'a str, &'a Kind)>) -> u8 {
    use trp_data::character::dress::fit;
    let class = |kind: &Kind, name: &str| kind.class_names.iter().any(|class| class == name);
    let mut bits = 0;
    for (slot, kind) in worn {
        match slot {
            "FaceCover" => {
                bits |= fit::HAT | fit::HELMET;
                if class(kind, "FaceCover") && !kind.half_mask {
                    bits |= fit::VISOR;
                }
            }
            "Eyewear" => bits |= fit::FACE_COVER | fit::HAT,
            "Headwear" if !(class(kind, "Headwear") && kind.helmet_material) => bits |= fit::HEADPHONES,
            "Earpiece" => bits |= fit::HAT,
            _ => {}
        }
    }
    bits
}

/// The same of an equipment's slots, and of things drawn on the body
/// whatever its slots hold (the app's `--wear`), each counted in the first
/// slot its kind is put on in (`slots_to_wear`).
pub fn fitting_bits_of(equipment: &Item, also: &[&Kind]) -> u8 {
    let worn = FITTING_SLOTS.iter().filter_map(|slot| equipment.in_slot(slot).map(|item| (*slot, &*item.kind)));
    let added = also.iter().filter_map(|kind| slots_to_wear_kind(kind).first().map(|slot| (*slot, *kind)));
    fitting_bits(worn.chain(added))
}

/// The equipment slot a thing would be put on in: the first of its
/// kind's that takes it (`FindSlotToPickUp`, `Slot.CheckConditions`).
pub fn slot_to_wear(equipment: &Item, item: &Item) -> Option<usize> {
    let takes = |slot: &usize| equipment.slot_takes(*slot, item).and_then(|()| equipment.blocking(*slot, item)).is_ok();
    let slots = slots_to_wear(item).iter().filter_map(|name| equipment.slot_index(name));
    slots.into_iter().find(takes).filter(|_| !item.missing_vital())
}
