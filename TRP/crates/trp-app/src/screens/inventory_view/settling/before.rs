//! What is known and asked before the hands settle: checks, rigs to make, a grenade thrown, what health said of items.

use super::*;

/// The weapon's magazine checked, and the weapon in hand's chamber known.
pub(super) fn checks(books: Books) {
    let Books { mut carried, mut controller, hands } = books;
    // The weapon's magazine checked in the hands is known from now on.
    if std::mem::take(&mut controller.magazine_checked) {
        let magazine = hands.item.and_then(|weapon| carried.find(weapon)).and_then(|weapon| weapon.slots.iter().zip(&weapon.kind.slots).find(|(_, slot)| slot.name == "mod_magazine").and_then(|(part, _)| part.as_ref()).map(|magazine| magazine.id));
        // Its first check: Mag Drills' (`MagazineCheckAction`).
        if magazine.is_some_and(|magazine| carried.checked.insert(magazine)) {
            controller.happened.push(crate::player::experience::Happened::Act(trp_sim::condition::earning::Act::MagazineChecked));
        }
        // A check is an event of the inventory, known before or not
        // (`CheckMagazineOperation`'s refresh): the weight read again.
        carried.weighed.forget();
    }
    // The weapon in hand's chamber known (`InventoryController.
    // CheckChamber(weapon, true)`; inventory spec, "A chamber not checked"):
    // by a look into it, a cylinder's reload, or a look at the magazine of
    // a revolver or of a weapon of more than one chamber (`Idling.CheckAmmo`).
    let (checked, looked) = (std::mem::take(&mut controller.chamber_checked), std::mem::take(&mut controller.magazine_looked));
    let many = |weapon: &Item| is(weapon, "Revolver") || weapon.kind.slots.iter().filter(|slot| slot.name.starts_with("patron_in_weapon")).count() > 1;
    if let Some(gun) = hands.item.filter(|&gun| checked || (looked && carried.find(gun).is_some_and(many))) {
        carried.know_chamber(gun, true);
    }
}

/// A weapon hung where the hands can take it with no rig yet has its rig asked for first; true while one is awaited.
pub(super) fn ask_rigs(carried: &Carried, features: &crate::screens::options_view::Features, stowed: Option<&Stowed>, mut preparing: Option<&mut crate::weapons::weapon::Preparing>) -> bool {
    let hung: Vec<String> = HAND_SLOTS
        .into_iter()
        .filter(|slot| features.grenades_and_knife || *slot != "Scabbard")
        .filter_map(|slot| carried.equipment.worn(slot))
        .map(trp_sim::items::inventory::rig_key)
        .collect();
    let awaited: Vec<&String> = hung.iter().filter(|template| stowed.as_deref().is_some_and(|stowed| stowed.awaited(template))).collect();
    if let Some(preparing) = preparing.as_deref_mut() {
        for template in &awaited {
            preparing.ask(template);
        }
    }
    !awaited.is_empty()
}

/// A grenade let go of in a throw, gone from the inventory as the hand lets it go.
pub(super) fn grenade_thrown(books: Books) {
    let Books { mut carried, mut controller, hands } = books;
    // A grenade let go of in a throw is gone from the inventory as the hand
    // lets it go (`ThrowGrenade`, an inventory operation).
    // It flies on as a thing of the world's (`loose`, `How::Grenade`) until
    // its fuse.
    if std::mem::take(&mut controller.grenade_thrown) {
        let low = std::mem::take(&mut controller.throws).last().copied().unwrap_or(false);
        if let Some(gone) = hands.using.and_then(|id| trp_sim::items::inventory::use_up(&mut carried.equipment.item, id)) {
            carried.note = format!("{}: thrown", carried.name(&gone));
            let m = &controller.movement;
            let at = beside(m.position, m.yaw, [0.0, 0.0]);
            carried.thrown.push((gone.id, crate::world::loose::How::Grenade { low }));
            carried.ground.push((gone, at));
        }
    }
}

/// What health says of an item it was given: what it holds now, or that it is used up.
pub(super) fn items_said(books: Books) {
    let Books { mut carried, mut controller, .. } = books;
    // What health says of an item it was given: what it holds now (a
    // medkit's points, food's share left), or that it is used up and gone.
    for event in std::mem::take(&mut controller.items_said) {
        match event {
            trp_sim::condition::health::Event::Resource(id, left) => {
                if carried.equipment.item.find(id).is_some_and(|item| item.resource != Some(left)) {
                    trp_sim::items::inventory::set_resource(&mut carried.equipment.item, id, left);
                }
            }
            trp_sim::condition::health::Event::UsedUp(id) => {
                if let Some(gone) = trp_sim::items::inventory::use_up(&mut carried.equipment.item, id) {
                    carried.note = format!("{}: used up", carried.name(&gone));
                }
            }
            _ => {}
        }
    }
}
