//! Switches, face shields, blindness protection and folding parts.

use super::*;

/// Whether a thing has a switch of its own (`TogglableComponent`:
/// armoured equipment on a hinge, night and thermal vision; inventory spec,
/// "A switch on the head").
pub fn has_switch(item: &Item) -> bool {
    item.kind.hinge || is(item, "NightVision") || is(item, "ThermalVision")
}

/// A thing's switch: the first of it and its parts, itself first, each
/// part before the parts fitted to it (`GetItemComponentsInChildren`).
pub fn switched_part(item: &Item) -> Option<&Item> {
    if has_switch(item) {
        return Some(item);
    }
    item.slots.iter().flatten().find_map(switched_part)
}

/// The first face shield of a thing, itself first, each part before the
/// parts fitted to it (`GetItemComponentsInChildren<FaceShieldComponent>`).
pub fn face_shield(item: &Item) -> Option<&Item> {
    with_parts(item).find(|part| part.kind.face_shield)
}

/// How long a flash blinds, as a share (`Player.BlindnessDuration`; grenade
/// spec, "A flash"): nothing with thermal vision on; else one less the
/// protection of each of a face shield on the head that is down (or has no
/// switch), a visor in the eyewear slot and a face cover's face shield
/// that is down, multiplied.
pub fn blindness_duration(equipment: &Equipment) -> f32 {
    let worn = |slot: &str| equipment.worn(slot);
    let thermal_on = WORN.iter().filter_map(|slot| worn(slot)).flat_map(with_parts).any(|part| is(part, "ThermalVision") && part.on);
    if thermal_on {
        return 0.0;
    }
    let shield_down = |shield: &Item| !has_switch(shield) || shield.on;
    let head = worn("Headwear").and_then(face_shield).filter(|shield| shield_down(shield)).map_or(1.0, |shield| 1.0 - shield.kind.blindness_protection);
    let eyes = worn("Eyewear").filter(|item| is(item, "Visors")).map_or(1.0, |visor| 1.0 - visor.kind.blindness_protection);
    let face = worn("FaceCover").and_then(face_shield).filter(|shield| shield_down(shield)).map_or(1.0, |shield| 1.0 - shield.kind.blindness_protection);
    head * eyes * face
}

/// The eyes' protection against the lights' flares (`Player.
/// GetBlindnessProtection`; tactical-devices spec, "The light shaft and
/// the flare"): the largest `BlindnessProtection` of the armoured things in
/// the eyewear, headwear and face cover slots, whatever their switch; then,
/// of the headwear and everything fitted to it, an armoured one at least as
/// high that has no face shield, or whose face shield has no switch or is
/// down.
pub fn blindness_protection(equipment: &Equipment) -> f32 {
    let mut most = 0.0f32;
    for slot in ["Eyewear", "Headwear", "FaceCover"] {
        if let Some(item) = equipment.worn(slot).filter(|item| is(item, "ArmoredEquipment")) {
            if item.kind.blindness_protection > most {
                most = item.kind.blindness_protection;
            }
        }
    }
    if let Some(head) = equipment.worn("Headwear") {
        for part in with_parts(head) {
            let shield_down = !part.kind.face_shield || !has_switch(part) || part.on;
            if is(part, "ArmoredEquipment") && part.kind.blindness_protection >= most && shield_down {
                most = part.kind.blindness_protection;
            }
        }
    }
    most
}

/// Whether the character wears glasses (`Player.TryFindGlasses`): the
/// eyewear's thing, else the headwear's, a visor.
pub fn has_glasses(equipment: &Equipment) -> bool {
    ["Eyewear", "Headwear"].iter().filter_map(|slot| equipment.worn(slot)).any(|item| is(item, "Visors"))
}

/// What the head's slots hold (the eyewear's, the headwear's and the face
/// cover's things and all fitted to them, by number): a change in it is a
/// thing put on or taken off there (`Player.OnItemAddedOrRemoved` for
/// those slots).
pub fn head_things(equipment: &Equipment) -> Vec<Id> {
    ["Eyewear", "Headwear", "FaceCover"].iter().filter_map(|slot| equipment.worn(slot)).flat_map(with_parts).map(|item| item.id).collect()
}

/// Whether a helmet's hinge is to be down (`HelmetDress.OnFaceShieldChanged`):
/// its face shield's switch on, or a face shield with no switch, or none.
pub fn hinge_down(helmet: &Item) -> bool {
    face_shield(helmet).is_none_or(|shield| !has_switch(shield) || shield.on)
}

/// Flips the switch of the thing of that number, wherever it lies in
/// `root` (`TogglableComponent.Set`): the part switched and how it stands
/// now; none if the thing has no switch.
pub fn toggle(root: &mut Item, id: Id) -> Option<(Id, bool)> {
    let part = switched_part(root.find(id)?)?.id;
    let part = root.find_mut(part)?;
    part.on = !part.on;
    Some((part.id, part.on))
}

/// The first thing in it that folds, itself first, each part before the
/// parts fitted to it, through the places that merge
/// (`GetItemComponentsInChildren<FoldableComponent>`).
pub fn folding_part(item: &Item) -> Option<&Item> {
    if item.kind.foldable {
        return Some(item);
    }
    item.slots.iter().zip(&item.kind.slots).filter(|(_, slot)| merges(item, slot)).find_map(|(part, _)| part.as_ref().and_then(folding_part))
}

/// A weapon's underbarrel launcher (`Weapon.GetUnderbarrelWeapon`: the first
/// part that is a `Launcher`, whatever its slot; a build has one at most),
/// whose own chamber holds its round (`chambered`, `set_chambered`).
pub fn launcher_part(weapon: &Item) -> Option<&Item> {
    with_parts(weapon).skip(1).find(|part| is(part, "Launcher"))
}

/// Whether the thing's fold can be turned at all (`FoldableComponent.
/// CanBeFolded`: its folded slot, where it names one, holds something).
pub fn can_be_folded(item: &Item) -> bool {
    folding_part(item).is_some_and(|part| {
        let slot = &part.kind.folded_slot;
        slot.is_empty() || part.kind.slots.iter().zip(&part.slots).any(|(holder, held)| holder.name == *slot && held.is_some())
    })
}

/// Whether the thing can be folded or unfolded (`ItemManipulator.
/// CanFold`): its fold can be turned, and on a weapon no part blocks it
/// (`BlocksFolding`).
pub fn can_fold(item: &Item) -> bool {
    can_be_folded(item) && (!is(item, "Weapon") || !item.slots.iter().flatten().any(|part| with_parts(part).any(|part| part.kind.blocks_folding)))
}

/// A thing and everything fitted to it.
pub(crate) fn with_parts(item: &Item) -> Box<dyn Iterator<Item = &Item> + '_> {
    Box::new(std::iter::once(item).chain(item.slots.iter().flatten().flat_map(with_parts)))
}

/// Folds or unfolds the thing of that number, wherever it lies in the roots
/// (`ItemManipulator.Fold`, `Resize_Helper`): its folding part turned the
/// other way, and from that part up each weapon or part that lies in a
/// grid made its new size where it lies, which is refused where the grid
/// has no room for it so (`Grid.Resize`: the same corner, the same turn).
/// The part folded and how it stands now; nothing changed if refused.
pub fn fold(roots: &mut [&mut Item], id: Id) -> Result<(Id, bool), Refusal> {
    let root = roots.iter_mut().find(|root| root.find(id).is_some()).ok_or(Refusal::NoSuchPlace)?;
    let thing = root.find(id).ok_or(Refusal::NoSuchPlace)?;
    if !can_fold(thing) {
        return Err(Refusal::NoAction);
    }
    let part = folding_part(thing).ok_or(Refusal::NoAction)?.id;
    let turn = |root: &mut Item| root.find_mut(part).map(|part| { part.folded = !part.folded; part.folded });
    let to = turn(root).ok_or(Refusal::NoSuchPlace)?;
    let chain: Vec<Id> = root.chain(part).iter().map(|item| item.id).collect();
    for pair in chain.windows(2).rev() {
        let (Some(holder), Some(item)) = (root.find(pair[0]), root.find(pair[1])) else { break };
        if !is(item, "Weapon") && !is(item, "Mod") {
            break;
        }
        if let Some((_, Spot::Cell(grid, place))) = holder.holder_of(item.id) {
            if holder.grid_takes(grid, item, place).is_err() {
                turn(root);
                return Err(Refusal::NoRoom);
            }
        }
    }
    Ok((part, to))
}
