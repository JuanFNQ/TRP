//! Rounds thrown into the world from the weapon, and the weapon in hand's fold and shots.

use super::*;

/// A chambered round thrown out, the chamber's racked round, a barrel's unloaded round and a cylinder's thrown out.
pub(super) fn thrown_rounds(books: Books, game: &crate::GameData, chamber_kinds: &crate::weapons::magazine_rounds::ChamberKinds) {
    let Books { mut carried, mut controller, mut hands } = books;
    // A chambered round the weapon threw out (a magazine filled with the
    // bolt open: `ReloadInternalMagWithOpenBoltOperation.
    // RemoveAmmoFromChamber`) is thrown from the shell port as the game
    // throws it (`ThrowPatronAsLoot`, `loose::How::FromPort`; `at` is where
    // it lies with the options' `ITEM PHYSICS` off): a round of the kind
    // the weapon names as its own.
    let ejected = std::mem::take(&mut controller.rounds_ejected);
    let round = hands
        .item
        .and_then(|weapon| carried.equipment.item.find(weapon))
        .and_then(|weapon| game.db.items.get(&weapon.kind.template)?.props.get("defAmmo")?.as_str().map(str::to_owned));
    if let (true, Some(round)) = (ejected > 0, round) {
        if let Ok(kind) = trp_sim::items::inventory::Kind::of(&game.db, &round).map(std::sync::Arc::new) {
            let m = &controller.movement;
            // Half a pace ahead and a little to the right.
            let at = beside(m.position, m.yaw, [DROP_AHEAD, 0.2]);
            for _ in 0..ejected {
                let round = Item::new(kind.clone());
                carried.thrown.push((round.id, crate::world::loose::How::FromPort));
                carried.ground.push((round, at));
            }
            carried.note = "the chambered round was thrown out as the bolt was opened: it lies on the ground".to_owned();
        }
    }
    // The round the chamber's key had the action throw out (firing spec,
    // "The magazine's and the chamber's keys"; `WeaponRechamberOperation`:
    // the chambered round discarded as its hands throw it): taken out of the
    // weapon's chamber as it is, and thrown into the world from the shell
    // port (`RechamberOperation.OnShellEjectEvent`: `ThrowPatronAsLoot`), as
    // the bolt-open fill's. The chamber then agrees with the logic's, empty
    // until the magazine's round is fed.
    let racked = std::mem::take(&mut controller.rounds_racked);
    if let (true, Some(gun)) = (racked > 0, hands.item) {
        let round = carried.equipment.item.find(gun).and_then(trp_sim::items::inventory::chambered).cloned();
        if let Some(round) = round {
            info!("the chamber's unload key: the action worked, the round ({}) thrown out", round.kind.template);
            trp_sim::items::inventory::set_chambered(&mut carried.equipment.item, gun, None);
            hands.chamber_agreed = Some((gun, false));
            let m = &controller.movement;
            carried.thrown.push((round.id, crate::world::loose::How::FromPort));
            carried.ground.push((round, beside(m.position, m.yaw, [DROP_AHEAD, 0.2])));
            carried.note = "the chambered round was thrown out as the action was worked: it lies on the ground".to_owned();
        }
    }
    // A live round a single barrel's reload took out goes to the vest or
    // the pockets where it has room, and else is thrown from the shell port
    // (more-weapons spec, "One barrel loaded alone, and the rocket
    // launcher"; `ReloadSingleBarrelOperation.OnShellEjectEvent`: with no
    // place, `ThrowPatronAsLoot`), of its own kind as kept beside the logic
    // (else the weapon's own).
    let unloaded = std::mem::take(&mut controller.rounds_unloaded);
    let round = chamber_kinds.kind(0).map(str::to_owned).or_else(|| {
        let weapon = carried.equipment.item.find(hands.item?)?;
        game.db.items.get(&weapon.kind.template)?.props.get("defAmmo")?.as_str().map(str::to_owned)
    });
    if let (true, Some(round)) = (unloaded > 0, round) {
        if let Ok(kind) = trp_sim::items::inventory::Kind::of(&game.db, &round).map(std::sync::Arc::new) {
            let m = &controller.movement;
            let at = beside(m.position, m.yaw, [DROP_AHEAD, 0.2]);
            for _ in 0..unloaded {
                if let Some(left) = trp_sim::items::inventory::put_unloaded(&mut carried.equipment.item, Item::new(kind.clone())) {
                    carried.thrown.push((left.id, crate::world::loose::How::FromPort));
                    carried.ground.push((left, at));
                    carried.note = "the round taken out of the barrel had no room: it lies on the ground".to_owned();
                }
            }
        }
    }
    // The live rounds a cylinder's quick reload threw out are thrown from
    // the weapon's first shell port, as the bolt-open fill's round (more-
    // weapons spec, "A cylinder's quick reload, and the fistful";
    // `ReloadCylinderMagOperation.RemoveAllAmmo`: `ThrowPatronAsLoot`, each
    // from `_shellPortsTransforms[0]`), each of
    // its own kind: the item's round in that chamber, which the logic's
    // has not yet taken away, else the kind kept for it, else the weapon's.
    let thrown_out = std::mem::take(&mut controller.rounds_dropped);
    if let (false, Some(gun)) = (thrown_out.is_empty(), hands.item) {
        let in_item = carried.find(gun).map(trp_sim::items::inventory::chamber_rounds).unwrap_or_default();
        let own = carried.find(gun).and_then(|weapon| game.db.items.get(&weapon.kind.template)?.props.get("defAmmo")?.as_str().map(str::to_owned));
        let m = &controller.movement;
        let at = beside(m.position, m.yaw, [DROP_AHEAD, 0.2]);
        for chamber in thrown_out {
            let template = in_item.get(chamber).cloned().flatten().map(|kind| kind.template.clone()).or_else(|| chamber_kinds.kind(chamber).map(str::to_owned)).or_else(|| own.clone());
            if let Some(kind) = template.and_then(|template| trp_sim::items::inventory::Kind::of(&game.db, &template).ok()).map(std::sync::Arc::new) {
                let round = Item::new(kind);
                carried.thrown.push((round.id, crate::world::loose::How::FromPort));
                carried.ground.push((round, at));
            }
        }
        carried.note = "the quick reload threw the cylinder's live rounds out: they lie on the ground".to_owned();
    }
}

/// The weapon in hand's fold, and a cylinder's shot unknowing its chambers.
pub(super) fn folds_and_shots(books: Books, firearm: &mut Option<&mut trp_sim::Firearm>) {
    let Books { mut carried, mut controller, mut hands } = books;
    // The weapon in hand's fold (inventory spec, "A fold from the menu";
    // shotgun spec, "The stock"): asked from the menu, its hands take it or
    // not, and the item is folded at once where they do (`FoldOperation`:
    // done, then the hands' `FoldStockOperation`; refused, undone); begun by
    // the key, the item folds with it.
    let fold_asked = if carried.fold_asked.is_some() { carried.fold_asked.take() } else { None };
    let begun = if controller.folds_begun > 0 { std::mem::take(&mut controller.folds_begun) } else { 0 };
    if let Some(gun) = hands.item.filter(|_| fold_asked.is_some() || begun > 0) {
        let mut turned = vec![gun; begun as usize];
        if let Some(part) = fold_asked {
            if firearm.as_deref_mut().is_some_and(|firearm| firearm.fold_stock()) {
                turned.push(part);
            } else {
                carried.note = "the hands can't fold the stock now".to_owned();
            }
        }
        for id in turned {
            if let Err(refusal) = trp_sim::items::inventory::fold(&mut [&mut carried.equipment.item], id) {
                warn!("the weapon in hand's fold not turned with its hands': {refusal:?}");
            }
        }
    }
    // The weapon in hand's detachable magazine as its logic counts it
    // (inventory spec, "Checking a magazine"): at rest, rounds its logic
    // has fired or chambered are gone from the item too, the magazine
    // known or not by the game's rule, a shot unknowing it where it is
    // left neither empty nor full (`UncheckOnShot`). A tube's shells are
    // not kept in its item, so it is left alone.
    let shots = std::mem::take(&mut controller.shots_made);
    hands.fired |= shots > 0;
    // A cylinder's shot: its chambers no longer known (`CylinderFire`'s
    // round taken from its chamber: `CheckChamber(weapon, false)`).
    if let (true, Some(gun), true) = (shots > 0, hands.item, firearm.as_deref().is_some_and(|firearm| firearm.settings.cylinder.is_some())) {
        if carried.checked_chambers.contains(&gun) {
            carried.know_chamber(gun, false);
        }
    }
}
