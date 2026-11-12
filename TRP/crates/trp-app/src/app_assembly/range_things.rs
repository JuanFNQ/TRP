//! The range's things: the gun cases, the cabinets, the containers, what lies about and what is put on.

use super::*;

pub(super) struct RangeThings {
    pub(super) away: Vec<Option<trp_sim::items::inventory::Item>>,
    pub(super) ground: Vec<(trp_sim::items::inventory::Item, Vec3)>,
    pub(super) laid_from: usize,
}

/// Fills the range's cases, cabinets and containers, lays the things on the ground and puts on what a run begins with.
pub(super) fn lay_out(db: &std::sync::Arc<Database>, install: &Install, unfinished: &std::sync::Arc<std::collections::HashSet<String>>, options: &mut Options, equipment: &mut trp_sim::items::inventory::Equipment, rules: &trp_sim::items::inventory::Rules) -> anyhow::Result<RangeThings> {
    // The tables of weapons along the knee-high wall: their gun cases, and
    // their magazine boxes; the storage cabinets' drawers.
    let (mut cases, boxes, rounds_boxes) = weapon_crates(&db, &unfinished)?;
    let mut cabinets = gear_cabinets(&db, &unfinished)?;
    // What the airdrop crate held until the workbench took its place, moved
    // to the crates where its kind goes: a weapon in its class's gun case, the kit's
    // vest and backpack and the outfits of one's own in the cabinets
    // (`cabinet_of`), the door's key on the bench.
    let mut key_lies = None;
    // The run's own weapon is the one in its class's gun case, and a run that
    // begins with it on takes it from there: its class, and its number.
    // Another build of it asked for (`--preset`): made below with what loads
    // it, in place of the case's stock one.
    let build = options.preset.as_deref().and_then(|wanted| match preset_build(&db, wanted, &options.weapon) {
        Ok(build) => {
            println!("the run's weapon as the build {} ({} parts)", build.name, build.items.len());
            Some(build)
        }
        Err(e) => {
            eprintln!("--preset {wanted}: {e:#}");
            None
        }
    });
    let own = crate_of(&db, &options.weapon).filter(|_| build.is_none()).and_then(|class| {
        let found = cases.get(class)?.grids.iter().flatten().find(|(_, item)| item.kind.template == options.weapon);
        found.map(|(_, item)| (class, item.id))
    });
    // What a run nobody is at the keys of, or one begun with `--equip`,
    // has on from the start: the weapon it is run with, the vest and the
    // backpack.
    let mut put_on = Vec::new();
    put_on.extend(own.map(|(_, id)| id));
    {
        use trp_data::character::inventory::Loadout;
        use trp_sim::items::inventory::{Equipment, Item};
        let mut things = Vec::new();
        // Any other weapon asked for (`--add-weapon`), and the run's own
        // where no gun case has it, each as its stock preset has it and
        // with what loads it, in the crate; the run's own has the kit's
        // magazines in the vest.
        let weapons = std::iter::once(options.weapon.as_str()).filter(|_| own.is_none()).chain(options.weapons.iter().map(String::as_str).filter(|weapon| *weapon != options.weapon));
        for weapon in weapons {
            match armed(&db, weapon, weapon != options.weapon, build.as_ref().filter(|_| weapon == options.weapon)) {
                Ok(made) => {
                    if weapon == options.weapon {
                        put_on.push(made[0].id);
                    }
                    // In its class's gun case (the rifles' where it has
                    // none).
                    let class = crate_of(&db, weapon).unwrap_or(0);
                    for thing in made {
                        let name = thing.kind.name.clone();
                        match stow_more(&mut cases[class], thing) {
                            Ok(()) => println!("in the {} weapon case: {name}", world::range::WEAPON_CRATES[class]),
                            Err(_) => eprintln!("the {} weapon case has no room for {name}", world::range::WEAPON_CRATES[class]),
                        }
                    }
                }
                Err(e) => eprintln!("no {weapon} could be made: {e:#}"),
            }
        }
        let kit = Loadout::load(&install, "Standard", "bear").and_then(|mut list| {
            let [magazines, rounds] = list.fit_to(&db, &options.weapon)?;
            println!("the crate's kit: {magazines} magazines and {rounds} lots of rounds changed for the weapon's own");
            Equipment::of_loadout(&db, &list)
        });
        match kit {
            Ok((mut outfit, _)) => {
                for worn in ["TacticalVest", "Backpack"].into_iter().filter_map(|slot| outfit.take_off(slot)) {
                    put_on.push(worn.id);
                    things.push(worn);
                }
            }
            Err(e) => eprintln!("the crate has no vest and no backpack: {e:#}"),
        }
        // The outfits that are not the install's (`docs/spec/outfits.md`):
        // each goes on where the armour it is made from does.
        let mut outfits: Vec<&String> = db.outfits.keys().collect();
        outfits.sort();
        for id in outfits {
            match trp_sim::items::inventory::Kind::of(&db, id) {
                Ok(kind) => things.push(Item::new(std::sync::Arc::new(kind))),
                Err(e) => eprintln!("the crate has no {id}: {e:#}"),
            }
        }
        // The key of the range's locked door, on the bench (on the table
        // behind the door wall without it).
        match trp_sim::items::inventory::Kind::of(&db, DOOR_KEY) {
            Ok(kind) => key_lies = Some(Item::new(std::sync::Arc::new(kind))),
            Err(e) => eprintln!("there is no key: {e:#}"),
        }
        for thing in things {
            let (name, [width, height], weight) = (thing.kind.name.clone(), thing.cells(), thing.weight());
            let at = db.items.get(&thing.kind.template).and_then(|item| cabinet_of(&db, item));
            match at.map(|at| (at, stow_more(&mut cabinets[at], thing))) {
                Some((at, Ok(()))) => println!("in the {} storage: {name}, {width} x {height} cells, {weight:.2} kg", world::range::CABINETS[at]),
                Some((at, Err(_))) => eprintln!("the {} storage has no room for {name} ({width} x {height} cells)", world::range::CABINETS[at]),
                None => eprintln!("no storage takes {name}"),
            }
        }
    }
    // The range's other containers, and the things lying about.
    let mut away: Vec<Option<trp_sim::items::inventory::Item>> = std::iter::once(Ok(None))
        .chain(CONTAINERS.iter().map(|template| {
            trp_sim::items::inventory::Kind::of(&db, template).map(|kind| Some(trp_sim::items::inventory::Item::new(std::sync::Arc::new(kind))))
        }))
        .collect::<anyhow::Result<_>>()?;
    // The weapons whose rigs are to be measured (`--measure-rigs`).
    if let Some(wanted) = options.measure_crate.as_deref() {
        for (holds, label) in cases.iter().zip(world::range::WEAPON_CRATES) {
            if wanted.eq_ignore_ascii_case("all") || wanted.eq_ignore_ascii_case(label) {
                let weapons = holds.grids.iter().flatten().map(|(_, thing)| thing).filter(|thing| trp_sim::items::inventory::is(thing, "Weapon"));
                options.measure_rigs.extend(weapons.map(|weapon| weapon.kind.template.clone()));
            }
        }
    }
    // The tables' gun cases, then their magazine boxes, after those; then
    // the storage cabinets' drawers.
    away.extend(cases.into_iter().chain(boxes).map(Some));
    away.extend(cabinets.into_iter().map(Some));
    // The tables of weapons' boxes of rounds, after the cabinets (the
    // places before them kept: `range::AMMO_BOX_HOLDS`).
    away.extend(rounds_boxes.into_iter().map(Some));
    // The workbench's boxes of repair kits, after those
    // (`range::REPAIR_BOX_HOLDS`): the armour's kits, then the weapons'.
    away.extend(repair_boxes(&db).into_iter().map(Some));
    // The four past the car, after those (`range::MORE_HOLDS`).
    debug_assert_eq!(away.len(), world::range::MORE_HOLDS);
    for template in MORE_CONTAINERS {
        match trp_sim::items::inventory::Kind::of(&db, template) {
            Ok(kind) => away.push(Some(trp_sim::items::inventory::Item::new(std::sync::Arc::new(kind)))),
            Err(e) => {
                eprintln!("the range has no container {template}: {e:#}");
                away.push(None);
            }
        }
    }
    let mut ground = Vec::new();
    if let Some(key) = key_lies {
        ground.push((key, Vec3::from(world::range::key_lies())));
    }
    for ((template, count), at) in LYING.into_iter().zip(world::range::lying()) {
        match trp_sim::items::inventory::Kind::of(&db, template) {
            Ok(kind) => {
                let mut item = trp_sim::items::inventory::Item::new(std::sync::Arc::new(kind));
                item.count = count;
                ground.push((item, Vec3::from(at)));
            }
            Err(e) => eprintln!("nothing lies at {at:?}: {e:#}"),
        }
    }
    // What `--lay` asks for: placed once the start is known (below).
    let laid_from = ground.len();
    for template in &options.lay {
        match trp_sim::items::inventory::Kind::of(&db, template) {
            Ok(kind) => ground.push((trp_sim::items::inventory::Item::new(std::sync::Arc::new(kind)), Vec3::ZERO)),
            Err(e) => eprintln!("--lay {template}: {e:#}"),
        }
    }
    // A run nobody is at the keys of has it all on from the start, put on
    // by the inventory's own moves; so does `--equip`.
    if (options.equip || options.unattended()) && !options.bare {
        use trp_sim::items::inventory::{move_item, Spot};
        let worn = equipment.item.id;
        // (The run's own weapon is in its gun case; the vest and the
        // backpack in the cabinets.)
        for id in put_on {
            let mut roots: Vec<&mut trp_sim::items::inventory::Item> = vec![&mut equipment.item];
            roots.extend(away.iter_mut().flatten());
            let slots = 0..roots[0].kind.slots.len();
            if !slots.into_iter().any(|slot| move_item(&mut roots, id, worn, Spot::Slot(slot), &rules).is_ok()) {
                eprintln!("something put on at the start goes into no slot");
            }
        }
        if options.no_spare_magazines {
            let held = ["TacticalVest", "Pockets", "Backpack"].into_iter().filter_map(|slot| equipment.item.in_slot(slot));
            let spares: Vec<_> = held.flat_map(|holder| holder.grids.iter().flatten()).filter(|(_, item)| trp_sim::items::inventory::is(item, "Magazine")).map(|(_, item)| item.id).collect();
            let taken = spares.into_iter().filter(|id| trp_sim::items::inventory::take_out(&mut [&mut equipment.item], *id, &rules).is_ok()).count();
            println!("--no-spare-magazines: {taken} magazines taken off the character");
        }
    }
    options.starts_armed = trp_sim::items::inventory::HAND_SLOTS.into_iter().any(|slot| equipment.worn(slot).is_some());
    // Such a run is what the app's own figures are measured with, so it
    // weighs what it did before there was an inventory (what `--weight`
    // says; nothing, if nothing was said) unless the inventory is what it
    // is run for (`--equip`, `--inventory`, `--loot`).
    if options.unattended() && !(options.equip || options.inventory) {
        options.weight_given = true;
    }
    if !options.weight_given {
        options.weight = equipment.carried_weight(false);
    }
    println!("carried: {:.2} kg", options.weight);
    Ok(RangeThings { away, ground, laid_from })
}
