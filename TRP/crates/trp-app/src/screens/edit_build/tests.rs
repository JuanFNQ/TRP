//! Tests of the edit build screen.

use super::*;

fn kind_named(db: &trp_data::Database, start: &str) -> Option<Arc<Kind>> {
    db.items.keys().filter_map(|id| Kind::of(db, id).ok()).find(|kind| kind.name.starts_with(start)).map(Arc::new)
}

/// The CG101 put on the PP-19, then the pistol grips hovered over: a grip rested on in
/// its own slot's dropdown is measured on the build as it stands (the
/// CG101 on it, the grip in its slot), as the game's `ResetCompare`
/// keeps its compared build; not on the weapon as the screen was given
/// it, which has no CG101 to put the grip on.
#[test]
fn a_part_rested_on_is_measured_on_the_build_as_it_stands() {
    let Ok(install) = trp_data::Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = trp_data::Database::load(&install).expect("database");
    let weapon = kind_named(&db, "weapon_izhmash_pp-19-01").expect("the PP-19-01");
    let list = trp_data::character::inventory::Loadout::of_preset(&db, &weapon.template).expect("its stock preset");
    let mut build = Item::of_list(&db, &list).expect("the weapon as an item").0;
    let given = build.clone();
    let adapter = kind_named(&db, "pistolgrip_ak_custom_guns_ak_to_ar15_cg101").expect("the CG101");
    let slot = rules::build_slots(&build).into_iter().find(|slot| rules::fits(&build, slot.owner, slot.index, &adapter)).expect("a slot that takes the CG101");
    assert!(rules::select(&mut build, slot.owner, slot.index, Some(&adapter)));
    // Its own grip's slot, empty (the CG101 is put in bare), and a grip
    // it takes.
    let own = rules::build_slots(&build).into_iter().find(|slot| build.find(slot.owner).is_some_and(|owner| owner.kind.template == adapter.template) && slot.part.is_none()).expect("the CG101's own slot");
    let parts = rules::all_parts(db.items.keys().filter_map(|id| Kind::of(&db, id).ok().map(Arc::new)));
    let grip = parts.iter().find(|part| rules::fits(&build, own.owner, own.index, part)).expect("a grip for the CG101").clone();
    assert!(given.find(own.owner).is_none(), "the CG101 is not on the weapon as given");
    let rested = ((own.owner, own.index), Some(grip.clone()));
    let compared = compared_build(&build, Some(&rested)).expect("the build compared with");
    let held = compared.find(own.owner).and_then(|owner| owner.slots[own.index].as_ref()).map(|part| part.kind.template.clone());
    assert_eq!(held, Some(grip.template.clone()));
    let added = compared.weight() - build.weight();
    assert!((added - Item::new(grip.clone()).weight()).abs() < 1e-4, "the grip's weight added to the build's: {added}");
    // Nothing rested on: no build compared with but the build itself.
    assert!(compared_build(&build, None).is_none());
}

/// The Mutant's stock slot shown: the Mk47's stock slot's bone lies under its
/// model's own `mod_stock_000`, an object of no slot of it; it is
/// found, and the RipStock's slot on the tube hung there.
#[test]
fn a_bone_under_another_mod_object_is_found() {
    let Ok(install) = trp_data::Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = trp_data::Database::load(&install).expect("database");
    let mut unity = trp_data::unity::Assets::new(install).expect("the install's assets");
    let weapon = kind_named(&db, "weapon_cmmg_mk47").expect("the Mk47");
    let list = trp_data::character::inventory::Loadout::of_preset(&db, &weapon.template).expect("its stock preset");
    let build = Item::of_list(&db, &list).expect("the weapon as an item").0;
    let model = trp_data::weapons::weapon::assemble(&mut unity, &db, &weapon.template).expect("its model");
    let names: Vec<String> = model.hierarchy.nodes.iter().map(|node| node.name.clone()).collect();
    let parents: Vec<Option<usize>> = model.hierarchy.nodes.iter().map(|node| node.parent).collect();
    let found = bones(&build, &names, &parents);
    let stock = build.kind.slots.iter().position(|slot| slot.name == "mod_stock_001").expect("the Mk47's stock slot");
    let bone = *found.get(&(build.id, stock)).expect("the stock slot's bone");
    assert_eq!(model.hierarchy.path(bone), "Weapon_root/Weapon_root_anim/weapon/mod_stock_000/mod_stock_001");
    let tube = build.slots[stock].as_ref().expect("the buffer tube on it");
    let butt = tube.kind.slots.iter().position(|slot| slot.name == "mod_stock_000").expect("the tube's stock slot");
    let bone = *found.get(&(tube.id, butt)).expect("the RipStock's slot's bone");
    assert!(model.hierarchy.path(bone).ends_with("mod_stock_001/stock_ar15_cmmg_mil_spec_tube/mod_stock_000"), "{}", model.hierarchy.path(bone));
    // Every slot of the build with a part has its bone.
    fn each(item: &Item, found: &HashMap<SlotId, usize>, missed: &mut Vec<String>) {
        for (index, (holder, part)) in item.kind.slots.iter().zip(&item.slots).enumerate() {
            let Some(part) = part.as_ref().filter(|part| rules::is_part(part)) else { continue };
            if !found.contains_key(&(item.id, index)) {
                missed.push(holder.name.clone());
            }
            each(part, found, missed);
        }
    }
    let mut missed = Vec::new();
    each(&build, &found, &mut missed);
    assert!(missed.is_empty(), "slots with a part and no bone: {missed:?}");
}
