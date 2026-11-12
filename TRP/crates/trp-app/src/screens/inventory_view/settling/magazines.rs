//! The magazines settled: a tube's rounds, reloads' exchanges, the next magazine and its build, and the fitted one changed.

use super::*;

/// What the tube took, the picker's stack, a reload's exchange, a switch on the head, the magazine let fall,
/// and the next magazine with the build it would make.
pub(super) fn reloads_and_next(
    books: Books,
    mut held: Mut<HandsHeld>,
    weapon: Option<&HandsRig>,
    mut stowed: Option<Mut<Stowed>>,
    mut preparing: Option<&mut crate::weapons::weapon::Preparing>,
    drag: &Drag,
) {
    let Books { mut carried, mut controller, mut hands } = books;
    // What the weapon's tube took leaves the pockets and the vest, and
    // what is left there is what the next filling has to go by; it goes
    // onto the tube's top in its item (the round for an empty chamber with
    // it: at rest it is taken off for the chamber, below).
    let taken = std::mem::take(&mut controller.rounds_taken);
    if let (true, Some(weapon)) = (taken > 0, hands.item) {
        let rounds = trp_sim::items::inventory::take_rounds_of_kinds(&mut carried.equipment.item, weapon, taken, held.pack);
        // (A cylinder's rounds go into its chambers with its logic's,
        // below.)
        let tube = carried.equipment.item.find(weapon).filter(|gun| gun.kind.reload_mode == "InternalMagazine").and_then(fitted_magazine);
        if let Some(tube) = tube.filter(|&tube| carried.find(tube).is_some_and(|magazine| !trp_sim::items::inventory::is_cylinder(magazine))) {
            trp_sim::items::inventory::put_rounds_into(&mut carried.equipment.item, tube, &rounds);
            // Left full, it is known (`CheckMagazineAmmoDepend`).
            let held = carried.find(tube).map(|item| (trp_sim::items::inventory::magazine_rounds(item), item.kind.capacity));
            if held.is_some_and(|(count, capacity)| trp_sim::items::inventory::known_after(count, capacity, false, false)) {
                carried.checked.insert(tube);
            }
        }
    }
    // The picker's stack is the reload's while it lasts, then gone.
    if held.pack.is_some() && !weapon.as_deref().and_then(|rig| rig.firearm.as_ref()).is_some_and(trp_sim::Firearm::reloading) {
        held.pack = None;
    }
    controller.rounds_in_reach = hands.item.map_or(0, |weapon| trp_sim::items::inventory::rounds_in_reach_from(&carried.equipment.item, weapon, held.pack));
    // A reloaded weapon's views take its new picture when the reload is
    // over, or as it leaves the hands (`Carried::reload_key`).
    let reloading = weapon.and_then(|rig| rig.firearm.as_ref()).is_some_and(trp_sim::Firearm::reloading);
    if carried.reload_key.as_ref().is_some_and(|(id, _)| hands.item != Some(*id) || !reloading) {
        carried.reload_key = None;
    }
    // A magazine's reload makes its exchange as it begins: the magazine
    // its key chose goes into the weapon, and the old one, with what the
    // weapon left in it, to the vest or the pockets, or (the quick way,
    // or with no room there) out to fall as it leaves the hand (firing
    // spec, "The quick reload").
    let reloads = std::mem::take(&mut controller.swaps);
    let reloaded = !reloads.is_empty();
    for (left, quick) in reloads {
        let (Some(weapon), Some(next)) = (hands.item, controller.magazine_asked.take()) else { continue };
        let m = &controller.movement;
        let at = beside(m.position, m.yaw, MAGAZINE_DROP);
        // One still on its way down from a reload before is on the ground.
        let fallen = std::mem::take(&mut carried.falling);
        carried.thrown.extend(fallen.iter().map(|old| (old.id, crate::world::loose::How::LetFall)));
        carried.ground.extend(fallen.into_iter().map(|old| (old, at)));
        // Its views keep the picture it had as the reload began (`shown_key`).
        if carried.reload_key.is_none() {
            let key = carried.equipment.item.find(weapon).map(crate::screens::icons::picture_key);
            carried.reload_key = key.map(|key| (weapon, key));
        }
        if let Some(old) = trp_sim::items::inventory::swap_magazine(&mut carried.equipment.item, weapon, Some(next), left, quick) {
            carried.falling.push(old);
        }
    }
    // A switch flipped on what is worn on the head (inventory spec, "A
    // switch on the head"): the hands asked for its gesture where the
    // character's state lets them act (the gesture itself is refused while
    // the thing in hand is not at rest, so never under the open screen); its
    // sound is heard with the rest of what is worn (`step_sound`).
    let switched = if carried.switched.is_empty() { Vec::new() } else { std::mem::take(&mut carried.switched) };
    for (part, on) in switched {
        switch_followed(&mut controller, &carried, part, on);
    }
    // What the weapon's own reloads changed in its magazine slot is the
    // hands' doing, not a move to tell them of (below).
    if reloaded {
        hands.fitted = hands.item.and_then(|id| carried.equipment.item.find(id)).map(|gun| (gun.id, fitted_magazine(gun)));
    }
    if std::mem::take(&mut controller.magazines_hidden) > 0 && !carried.falling.is_empty() {
        let m = &controller.movement;
        let at = beside(m.position, m.yaw, MAGAZINE_DROP);
        let fallen = std::mem::take(&mut carried.falling);
        carried.thrown.extend(fallen.iter().map(|old| (old.id, crate::world::loose::How::LetFall)));
        carried.ground.extend(fallen.into_iter().map(|old| (old, at)));
        carried.note = "the old magazine was let fall: it lies on the ground".to_owned();
    }
    let unknown = hands.item.and_then(|weapon| carried.equipment.item.find(weapon)).and_then(|weapon| weapon.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT)).is_some_and(|magazine| !trp_sim::items::examine::known(magazine));
    if controller.magazine_unknown != unknown {
        controller.magazine_unknown = unknown;
    }
    controller.next_magazine = hands.item.and_then(|weapon| {
        let next = trp_sim::items::inventory::next_magazine(&carried.equipment.item, weapon)?;
        let magazine = carried.equipment.item.find(next)?;
        let unfit = carried.equipment.item.find(weapon).map_or(0, |gun| trp_sim::items::inventory::unfit_rounds(gun, magazine));
        Some((next, trp_sim::items::inventory::magazine_rounds(magazine), magazine.kind.magazine_animation, magazine.kind.magazine_external, unfit))
    });
    // The build the next magazine would make, where it is of another kind
    // than the one fitted, has its rig made ready ahead (`Preparing::
    // ask_ahead`), so that a reload's change of the model finds it built
    // before the new magazine shows (`Firearm::carry_on_from`).
    // A magazine taken up with the screen open is the next one where the
    // weapon in hand takes it: let go on the weapon it is put in by the
    // hands, and the game shows its own model at the insert's `MagShow`
    // (`InsertMagOperation.OnMagAppeared`: `Firearms.SetupMod` with the
    // magazine's model made there and then).
    let taken = drag.0.as_ref().map(|held| held.id).filter(|id| carried.find(*id).is_some_and(|item| is(item, "Magazine")));
    let build_with = |next: Id| -> Option<String> {
        let gun = carried.equipment.item.find(hands.item?)?;
        let magazine = carried.find(next)?;
        let slot = gun.slot_index(trp_sim::items::inventory::MAGAZINE_SLOT)?;
        if !magazine.kind.among(&gun.kind.slots.get(slot)?.takes) || gun.slots[slot].as_ref().is_some_and(|fitted| fitted.kind.template == magazine.kind.template) {
            return None;
        }
        let mut with = gun.clone();
        with.slots[slot] = Some(magazine.clone());
        Some(trp_sim::items::inventory::rig_key(&with))
    };
    let next_build = taken.and_then(build_with).or_else(|| controller.next_magazine.and_then(|(next, ..)| build_with(next)));
    if let Some(stowed) = stowed.as_deref_mut() {
        if stowed.next_build != next_build {
            stowed.next_build = next_build.clone();
        }
    }
    if let (Some(key), Some(preparing), Some(stowed)) = (next_build, preparing.as_deref_mut(), stowed.as_deref()) {
        if !stowed.has_rig(&key) && stowed.awaited(&key) {
            preparing.ask_ahead(&key);
        }
    }
}

/// The fitted magazine's unfit rounds told to the logic, and a magazine taken out or put in from the inventory.
pub(super) fn fitted_changed(books: Books, firearm: &mut Option<&mut trp_sim::Firearm>, fitted: Option<(Id, Option<Id>)>) {
    let Books { mut carried, mut hands, .. } = books;
    // The fitted magazine's rounds its chamber refuses, as the item has them
    // (firing spec, "Rounds the chamber does not take"): counted from the
    // magazine's foot, true still while the item keeps rounds the logic has
    // fed off its top. A magazine just put in from here brings its own.
    let unfit = hands.item.and_then(|id| carried.equipment.item.find(id)).map(trp_sim::items::inventory::fitted_unfit_rounds);
    if let (Some(firearm), Some(unfit)) = (firearm.as_deref_mut(), unfit) {
        firearm.set_magazine_unfit(unfit);
    }
    if let (Some(firearm), Some((was_gun, was)), Some((gun, now))) = (firearm.as_deref_mut(), hands.fitted, fitted) {
        if was_gun == gun && was != now {
            let left = firearm.magazine.unwrap_or(0);
            let rounds = |carried: &Carried, id: Id| carried.find(id).map_or(0, trp_sim::items::inventory::magazine_rounds);
            let variant = |carried: &Carried, id: Id| carried.find(id).map_or(0, |magazine| magazine.kind.magazine_animation);
            let external = |carried: &Carried, id: Id| carried.find(id).is_some_and(|magazine| magazine.kind.magazine_external);
            if let Some(old) = was {
                trp_sim::items::inventory::set_rounds_of(&mut carried.equipment.item, old, left);
                trp_sim::items::inventory::set_rounds_of(&mut carried.loot, old, left);
            }
            match (was, now) {
                (Some(old), None) => {
                    // (One the magazine's unload key moved has its hands'
                    // move begun already: `unload_keys`.)
                    if firearm.pull_out_magazine() || firearm.pulling_magazine_out() {
                        hands.magazine_moving = Some(old);
                    } else {
                        firearm.magazine = None;
                    }
                }
                (None, Some(new)) => {
                    let rounds = rounds(&*carried, new);
                    let (variant, external) = (variant(&*carried, new), external(&*carried, new));
                    firearm.next_magazine_variant(variant);
                    firearm.next_magazine_external(external);
                    firearm.next_magazine_unfit(unfit.unwrap_or(0));
                    if firearm.insert_magazine(rounds) {
                        hands.magazine_moving = Some(new);
                    } else {
                        firearm.magazine = Some(rounds);
                        firearm.set_magazine_variant(variant);
                        firearm.set_magazine_external(external);
                    }
                }
                (Some(_), Some(new)) => {
                    firearm.magazine = Some(rounds(&*carried, new));
                    firearm.set_magazine_variant(variant(&*carried, new));
                    firearm.set_magazine_external(external(&*carried, new));
                }
                (None, None) => {}
            }
        }
    }
}
