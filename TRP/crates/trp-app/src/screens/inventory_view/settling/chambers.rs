//! The chambers settled: a load given up, the chambered round, the launcher's and chambers of their own.

use super::*;

/// A load given up undone, and the chambered round and the magazine as the logic counts them; whether the weight
/// had heard everything before.
pub(super) fn chamber_agreed(books: Books, game: &crate::GameData, firearm: &mut Option<&mut trp_sim::Firearm>, fitted: Option<(Id, Option<Id>)>) -> bool {
    let Books { mut carried, mut controller, mut hands } = books;
    // The chambered round as its hands have it (firing spec, "A round into
    // the chamber from the inventory"): a round the inventory has just put
    // into the empty chamber is loaded by the hands (the move was let only
    // where they would take it); a load given up for a put-away is undone,
    // the round put back where a pick-up puts it (`PROVISIONAL`: the game
    // puts it back where it was).
    let given_up = controller.chamber_given_up && std::mem::take(&mut controller.chamber_given_up);
    let round_in = |carried: &Carried, gun: Id| carried.equipment.item.find(gun).and_then(trp_sim::items::inventory::chambered).map(|round| round.kind.clone());
    // A barrel's the same (more-weapons spec, "A round moved into a chamber
    // of its own, or out of one, from the inventory").
    let barrel_round = |carried: &Carried, (gun, barrel): (Id, usize)| carried.find(gun).and_then(|weapon| trp_sim::items::inventory::chamber_rounds(weapon).get(barrel).cloned().flatten());
    let barrel = hands.loading_barrel.take_if(|_| given_up);
    if let Some(((gun, at), kind)) = barrel.and_then(|barrel| Some((barrel, barrel_round(&*carried, barrel)?))) {
        trp_sim::items::inventory::set_chamber_round(&mut carried.equipment.item, gun, at, None);
        let rules = carried.rules;
        let (put, left) = trp_sim::items::inventory::pick_up(&mut carried.equipment.item, Item::new(kind), &rules);
        if let (Err(_), Some(left)) = (put, left) {
            let m = &controller.movement;
            carried.ground.push((left, beside(m.position, m.yaw, [DROP_AHEAD, 0.2])));
        }
        carried.note = "the round was not put into the barrel".to_owned();
    }
    if let (true, Some(gun)) = (given_up, hands.loading_chamber.take()) {
        if let Some(kind) = round_in(&*carried, gun) {
            trp_sim::items::inventory::set_chambered(&mut carried.equipment.item, gun, None);
            let rules = carried.rules;
            let (put, left) = trp_sim::items::inventory::pick_up(&mut carried.equipment.item, Item::new(kind), &rules);
            if let (Err(_), Some(left)) = (put, left) {
                let m = &controller.movement;
                carried.ground.push((left, beside(m.position, m.yaw, [DROP_AHEAD, 0.2])));
            }
            carried.note = "the round was not put into the chamber".to_owned();
        }
    }
    // What the hands' logic did goes into the item from here to the
    // chambers of their own (below), unheard by the weight where nothing it
    // hears of changed before (`Carried::unheard`; movement spec, "The
    // weight, as the inventory tells of it"): a shot's rounds, a launcher's
    // grenade fired. A launcher's round taken from the gear is heard.
    let quiet = carried.as_weighed();
    if let (Some(firearm), Some((was_gun, was)), Some((gun, now))) = (firearm.as_deref_mut(), hands.fitted, fitted) {
        let same = was_gun == gun && was == now;
        let in_item = round_in(&*carried, gun);
        // A round moved into the chamber or out of it by the inventory is
        // a change in the item since it last agreed with the logic, not a
        // difference from the logic: a shot empties the logic's chamber and
        // the item keeps the fired round until it is brought to the logic
        // at rest (otherwise, with no magazine in, or
        // a magazine of the weapon's own, each press fired a round the hands
        // had loaded anew from the one just fired).
        let agreed = hands.chamber_agreed.filter(|(at, _)| *at == gun).map(|(_, round)| round);
        let (put_in, taken_out) = (in_item.is_some() && agreed == Some(false), in_item.is_none() && agreed == Some(true));
        if same && put_in && firearm.single_chamber() && !firearm.chamber && firearm.can_load_chamber() {
            if firearm.load_chamber() {
                hands.loading_chamber = Some(gun);
                hands.chamber_agreed = Some((gun, true));
            }
        } else if same && taken_out && firearm.single_chamber() && firearm.chamber && firearm.can_discharge() {
            hands.chamber_agreed = Some((gun, false));
            // The chambered round the inventory has just taken out (firing
            // spec, "The chambered round taken out from the inventory"): the
            // hands discharge it, the move let only where they would; the
            // chamber is empty at their mark. (It had been put back, of the
            // magazine's top kind or the weapon's own, so that a PS round kept
            // coming back into the chamber.) `PROVISIONAL`:
            // the round moved at once, where the game moves it at the mark.
            firearm.discharge();
        } else if same && firearm.can_move_magazine() {
            hands.loading_chamber = None;
            // The weapon in hand's magazine, or tube, as its logic counts it
            // (inventory spec, "Checking a magazine"): at rest, rounds its
            // logic has fired or chambered are gone from the item too, the
            // magazine known or not by the game's rule, a shot unknowing it
            // where it is left neither empty nor full (`UncheckOnShot`). A
            // cylinder's rounds lie in chambers of its own: left alone.
            let mut fed = None;
            let held = now.and_then(|magazine| carried.find(magazine).map(|item| (magazine, trp_sim::items::inventory::magazine_rounds(item), item.kind.capacity)));
            if let (Some(count), Some((magazine, rounds, capacity))) = (firearm.magazine.filter(|_| firearm.single_chamber()), held) {
                if count < rounds {
                    // The last round to leave it is the one now chambered.
                    fed = carried.find(magazine).and_then(|item| trp_sim::items::inventory::magazine_round_at(item, rounds - count - 1));
                    trp_sim::items::inventory::set_rounds_of(&mut carried.equipment.item, magazine, count);
                    let fired = std::mem::take(&mut hands.fired);
                    let shot = fired && carried.round_times.uncheck_on_shot;
                    // Each shot that fed the chamber leaves it known as the
                    // magazine was before it (`FireOperation.UncheckOnShot`:
                    // `CheckChamber(weapon, magazine checked)`, then the
                    // magazine's own uncheck): a checked magazine's first
                    // shot keeps it known, the second not.
                    if fired {
                        let known = chamber_after_shots(carried.checked.contains(&magazine), rounds, count, capacity, shot);
                        if carried.checked_chambers.contains(&gun) != known {
                            carried.know_chamber(gun, known);
                        }
                    }
                    if trp_sim::items::inventory::known_after(count, capacity, carried.checked.contains(&magazine), shot) {
                        carried.checked.insert(magazine);
                    } else {
                        carried.checked.remove(&magazine);
                    }
                }
            }
            // A failure to feed's round, out of the magazine and not into
            // the chamber (`MalfState.MalfunctionedAmmo`): kept with the
            // weapon for its repair to throw out (`crate::wear`). The item
            // is reached for only with a round to keep: reached for on every
            // frame of the jam, what is carried counted as changed, and the
            // page was drawn afresh on each, so that no box ever knew the
            // mouse was on it.
            if firearm.malfunction() == trp_sim::weapons::malfunction::Malf::Feed && !firearm.chamber {
                if let Some(kind) = fed.take() {
                    if let Some(gun) = carried.equipment.item.find_mut(gun) {
                        gun.fed_round = Some(kind);
                    }
                }
            }
            // Its chamber of its own as its logic has it: a round fed is
            // the one in it, one fired or thrown out is gone.
            if firearm.single_chamber() {
                match (firearm.chamber, in_item.is_some()) {
                    (true, false) | (true, true) if fed.is_some() => {
                        trp_sim::items::inventory::set_chambered(&mut carried.equipment.item, gun, fed);
                    }
                    (true, false) => {
                        let top = now.and_then(|magazine| carried.find(magazine)).and_then(|item| trp_sim::items::inventory::magazine_round_at(item, 0));
                        let own = || game.db.items.get(&carried.equipment.item.find(gun)?.kind.template)?.props.get("defAmmo")?.as_str().and_then(|round| trp_sim::items::inventory::Kind::of(&game.db, round).ok()).map(std::sync::Arc::new);
                        if let Some(kind) = top.or_else(own) {
                            trp_sim::items::inventory::set_chambered(&mut carried.equipment.item, gun, Some(kind));
                        }
                    }
                    (false, true) => {
                        trp_sim::items::inventory::set_chambered(&mut carried.equipment.item, gun, None);
                    }
                    _ => {}
                }
            }
            hands.chamber_agreed = Some((gun, round_in(&*carried, gun).is_some()));
        }
    }
    quiet
}

/// An underbarrel launcher's round taken and fired; whether one was taken.
pub(super) fn launcher(books: Books, game: &crate::GameData, firearm: &mut Option<&mut trp_sim::Firearm>) -> bool {
    let Books { mut carried, mut controller, mut hands } = books;
    // An underbarrel launcher's round (underbarrel-launcher spec,
    // "Reloading and unloading"): the reload's taken from within reach into
    // its chamber as the reload begins (`LauncherReload.Prepare`), the shot's
    // gone from it. A weapon newly in hand takes its item's; its item changed
    // by the inventory since they last agreed, the hands play the move: a
    // round put in loaded (`LoadAmmoToUnderbarrelWeapon`), one taken out
    // discharged (switching to the launcher first from the rifle's rest).
    let launcher_of = |carried: &Carried, gun: Id| carried.equipment.item.find(gun).and_then(trp_sim::items::inventory::launcher_part).map(|part| part.id);
    let launcher = hands.item.and_then(|gun| launcher_of(&*carried, gun).map(|launcher| (gun, launcher)));
    controller.launcher_rounds_in_reach = launcher.map_or(0, |(_, launcher)| trp_sim::items::inventory::rounds_in_reach(&carried.equipment.item, launcher));
    let (taken, fired) = (std::mem::take(&mut controller.launcher_round_taken), std::mem::take(&mut controller.launcher_fired));
    match (launcher, firearm.as_deref_mut().filter(|firearm| firearm.has_launcher())) {
        (Some((gun, launcher)), Some(firearm)) => {
            if taken {
                let rounds = trp_sim::items::inventory::take_rounds_of_kinds(&mut carried.equipment.item, launcher, 1, None);
                if let Some((kind, _)) = rounds.into_iter().next() {
                    trp_sim::items::inventory::set_chambered(&mut carried.equipment.item, launcher, Some(kind));
                }
            }
            if fired {
                // The shot's recoil, read off its round before it goes
                // (`LauncherShot`: 1 and the round's `ammoRec` a hundredth).
                let round = carried.find(launcher).and_then(trp_sim::items::inventory::chambered).map(|round| round.kind.template.clone());
                let recoil = round.as_ref().and_then(|round| game.db.items.get(round).and_then(|t| t.prop_f32("ammoRec"))).unwrap_or(0.0);
                controller.launcher_recoil = 1.0 + recoil / 100.0;
                // Its grenade, made from the launcher's fireport (`bullets`).
                controller.launcher_bullets.extend(round);
                trp_sim::items::inventory::set_chambered(&mut carried.equipment.item, launcher, None);
            }
            let in_item = carried.find(launcher).and_then(trp_sim::items::inventory::chambered).is_some();
            match hands.launcher_agreed.filter(|(at, _)| *at == gun).map(|(_, round)| round) {
                None => firearm.set_launcher_round(in_item),
                Some(agreed) if agreed != in_item && !taken && !fired => {
                    if in_item {
                        firearm.load_launcher();
                    } else {
                        firearm.discharge_launcher();
                    }
                }
                _ => {}
            }
            hands.launcher_agreed = Some((gun, in_item));
        }
        _ => hands.launcher_agreed = None,
    }
    taken
}

/// A cylinder's or barrels' rounds and cases kept with the logic's chambers.
pub(super) fn own_chambers(books: Books, game: &crate::GameData, held: &HandsHeld, mut chamber_kinds: Mut<crate::weapons::magazine_rounds::ChamberKinds>, stowed: Option<&Stowed>, firearm: &mut Option<&mut trp_sim::Firearm>) {
    let Books { mut carried, mut hands, .. } = books;
    // A cylinder's or barrels' rounds and cases kept with the logic's
    // chambers (more-weapons spec, "The rounds and cases seen in chambers of
    // their own"). A weapon newly in hand takes its item's at once, as the
    // game's hands are made from the item, less the cases its draw takes
    // away (`Firearm::hold_rounds`, made; also as the hands take it). Its
    // item changed by the inventory since it last agreed with the logic,
    // the hands play the move, one chamber at a time as their logic comes
    // to rest (more-weapons spec, "A round moved into a chamber of its own,
    // or out of one, from the inventory"): a round gone from the item taken
    // out (the highest chamber first, as the inventory empties a cylinder
    // from its last; going on into a chamber the item has filled with it),
    // then a round come into the item put in (the lowest first); where the
    // hands cannot, and once the rounds agree, the logic takes the item's
    // (`hold_rounds`). `PROVISIONAL`: the inventory has moved a round
    // taken out at once, where the game moves it at the hands' mark. Else
    // what the logic did (a shot, a reload, a round
    // taken out, a case thrown or left) goes into the item, each round of
    // the kind kept for its chamber and each case of its own
    // (`ChamberKinds`, else the weapon's own kind). Only while the rig in
    // hand is this weapon's; a weapon put away is made afresh when it is
    // taken again.
    let own_rig = |carried: &Carried, gun: Id| carried.find(gun).map(trp_sim::items::inventory::rig_key) == stowed.as_deref().and_then(|stowed| stowed.held()).map(str::to_owned);
    let with_chambers = firearm.as_deref_mut().filter(|firearm| !firearm.single_chamber());
    if with_chambers.is_none() || hands.item.is_none() {
        hands.chambers_agreed = None;
    }
    if let (Some(firearm), Some(gun)) = (with_chambers, hands.item.filter(|&gun| own_rig(&carried, gun))) {
        let in_item = item_chambers(&carried, gun);
        let agreed = hands.chambers_agreed.as_ref().filter(|(at, _)| *at == gun).map(|(_, held)| held);
        if agreed != Some(&in_item) {
            let made = agreed.is_none();
            let rounds: Vec<bool> = in_item.0.iter().map(Option::is_some).collect();
            let cases: Vec<bool> = in_item.1.iter().map(Option::is_some).collect();
            let held: Vec<bool> = firearm.in_chambers.iter().map(|chamber| chamber.round).collect();
            let played = !made && firearm.can_move_magazine() && held.len() == rounds.len() && {
                let out = (0..held.len()).rev().find(|&at| held[at] && !rounds[at]);
                let into = (0..held.len()).find(|&at| !held[at] && rounds[at]);
                match (out, into) {
                    (Some(at), onward) => {
                        let unloading = firearm.unload_own_chamber(at, onward.is_some());
                        if unloading {
                            debug!("weapon {gun}'s chamber {at} emptied by the inventory: the hands take its round out");
                        }
                        unloading
                    }
                    (None, Some(at)) => {
                        // The round's kind for its chamber, before it shows.
                        chamber_kinds.take_item(gun, &in_item.0, &in_item.1);
                        let loaded = firearm.load_own_chamber(at);
                        if loaded {
                            debug!("weapon {gun}'s chamber {at} filled by the inventory: the hands put its round in");
                            hands.loading_barrel = firearm.settings.cylinder.is_none().then_some((gun, at));
                        }
                        loaded
                    }
                    (None, None) => false,
                }
            };
            if !played && (made || firearm.can_move_magazine()) && firearm.hold_rounds(&rounds, &cases, made) {
                debug!("weapon {gun}'s item taken ({}): rounds {rounds:?}, cases {cases:?}", if made { "made" } else { "moved" });
                chamber_kinds.take_item(gun, &in_item.0, &in_item.1);
                hands.chambers_agreed = Some((gun, in_item));
                hands.loading_barrel = None;
            }
        } else {
            let kind_of = |template: Option<String>| template.and_then(|template| trp_sim::items::inventory::Kind::of(&game.db, &template).ok()).map(std::sync::Arc::new);
            let mut changed = false;
            for chamber in 0..firearm.in_chambers.len() {
                let now = firearm.in_chambers[chamber];
                if now.round != in_item.0.get(chamber).is_some_and(Option::is_some) {
                    let kind = match now.round {
                        true => kind_of(chamber_kinds.kind(chamber).map(str::to_owned).or_else(|| crate::weapons::cases::fired_round(&carried, &held, &game.db))),
                        false => None,
                    };
                    if kind.is_some() || !now.round {
                        changed |= trp_sim::items::inventory::set_chamber_round(&mut carried.equipment.item, gun, chamber, kind);
                    }
                }
                let case = now.case.then(|| chamber_kinds.case(chamber).or_else(|| chamber_kinds.kind(chamber)).map(str::to_owned).or_else(|| crate::weapons::cases::fired_round(&carried, &held, &game.db))).flatten();
                if case.as_deref() != in_item.1.get(chamber).and_then(Option::as_deref) {
                    let kind = kind_of(case);
                    if kind.is_some() || !now.case {
                        changed |= trp_sim::items::inventory::set_chamber_case(&mut carried.equipment.item, gun, chamber, kind);
                    }
                }
            }
            if changed {
                let now = item_chambers(&carried, gun);
                debug!("weapon {gun}'s item given the logic's: rounds {:?}, cases {:?}", now.0, now.1);
                hands.chambers_agreed = Some((gun, now));
            }
        }
    }
}
