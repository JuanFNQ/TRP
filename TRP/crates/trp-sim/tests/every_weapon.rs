//! Every weapon of the install that has a stock preset, and every weapon
//! of one's own, put through what the app does with it (the user,
//! 2026-10-14: "ASVAL causes a crash. can we double check all weapons and
//! ensure it doesnt occur!"): assembled, its skinned parts' bones found in
//! its own tree, its logic and animator made, found in a crate, drawn,
//! fired, reloaded both ways, and put away, with nothing left waiting.
//! Passes with a note where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test every_weapon -- --nocapture`
//! for each weapon's line.

use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::{Firearm, FirearmEvent};

const FRAME: f32 = 1.0 / 60.0;

/// The weapons the app lays in its crates of weapons: every weapon item
/// with a stock preset that is not fired once and thrown away, and the
/// weapons of one's own.
fn weapons(db: &Database) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = db
        .items
        .values()
        .filter(|item| (item.name.starts_with("weapon_") && db.is_a(item, "Weapon")) || db.custom.contains_key(&item.id))
        .filter(|item| !item.props.get("IsOneoff").and_then(serde_json::Value::as_bool).unwrap_or(false))
        .filter(|item| db.default_preset(&item.id).is_ok())
        .map(|item| (item.name.clone(), item.id.clone()))
        .collect();
    all.sort();
    all
}

/// Steps the weapon until it is at rest, or for so long at most; whether
/// it came to rest.
fn settle(w: &mut Firearm, most: f32) -> bool {
    let mut time = 0.0;
    while time < most {
        w.step(FRAME);
        time += FRAME;
        if w.is_idle() && time > 0.3 {
            return true;
        }
    }
    w.is_idle()
}

#[test]
fn every_weapon_is_put_together_drawn_fired_and_reloaded() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let mut failed = Vec::new();
    let all = weapons(&db);
    for (name, id) in &all {
        let checked = (|| -> anyhow::Result<String> {
            let weapon = trp_data::weapon::assemble(&mut assets, &db, id)?;
            // A skinned part is drawn on its bones: each of them is a node
            // of the weapon's own tree, or it is drawn without them.
            let mut skinned = Vec::new();
            for node in &weapon.hierarchy.nodes {
                let Some(renderer) = node.renderer.as_ref().filter(|renderer| renderer.skinned) else { continue };
                let found: Vec<&str> = renderer.bones.iter().flatten().map(|bone| weapon.hierarchy.nodes[*bone].name.as_str()).collect();
                skinned.push(format!("{} ({} of {} bones: {})", node.name, found.len(), renderer.bones.len(), found.join(", ")));
            }
            let data = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon)?;
            let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon)?;
            let mut w = Firearm::new(settings, &data);
            w.found();
            w.draw();
            anyhow::ensure!(settle(&mut w, 8.0), "not at rest after its draw");
            // Aimed and fired, which a rocket launcher needs.
            w.set_aim(true, true);
            settle(&mut w, 1.5);
            w.set_trigger(true, true);
            for _ in 0..20 {
                w.step(FRAME);
            }
            w.set_trigger(false, true);
            w.set_aim(false, true);
            anyhow::ensure!(settle(&mut w, 6.0), "not at rest after a shot");
            // Reloaded as the app reloads it: a tube, a cylinder or barrels
            // from rounds in reach, a magazine from a full one, the quick
            // way too. (A rocket launcher has no state to reload in.)
            if !w.settings.rocket_launcher {
                if w.settings.internal_magazine {
                    w.reload_shells(60);
                } else {
                    w.reload_with(w.settings.magazine_capacity);
                }
                anyhow::ensure!(settle(&mut w, 20.0), "not at rest after a reload");
                if !w.settings.internal_magazine {
                    w.quick_reload_with(w.settings.magazine_capacity);
                    anyhow::ensure!(settle(&mut w, 20.0), "not at rest after a quick reload");
                }
            }
            w.hide(false);
            let mut gone = false;
            for _ in 0..(6.0 / FRAME) as usize {
                gone |= w.step(FRAME).contains(&FirearmEvent::Hidden);
            }
            anyhow::ensure!(gone, "not put away");
            Ok(if skinned.is_empty() { String::new() } else { format!("; skinned: {}", skinned.join(", ")) })
        })();
        match checked {
            Ok(note) => println!("ok   {name} {id}{note}"),
            Err(e) => {
                println!("FAIL {name}: {e:#}");
                failed.push(format!("{name}: {e:#}"));
            }
        }
    }
    println!("{} weapons, {} failed", all.len(), failed.len());
    assert!(failed.is_empty(), "{failed:#?}");
}

/// Every weapon fed from magazines is found with one in it (the RPK-16's
/// stock preset has none: `Database::fit_magazines`): the item in the
/// crate has it in its magazine slot, the rig has its model hung, and the
/// logic is told how many rounds it holds.
#[test]
fn every_weapon_fed_from_magazines_has_one_in_it() {
    use trp_sim::inventory::{Item, MAGAZINE_SLOT};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let mut failed = Vec::new();
    let mut counted = 0;
    for (name, id) in weapons(&db) {
        let template = db.item(&id).expect("its template");
        if template.props.get("ReloadMode").and_then(serde_json::Value::as_str) != Some("ExternalMagazine") {
            continue;
        }
        counted += 1;
        let checked = (|| -> anyhow::Result<()> {
            let (item, _) = Item::of_list(&db, &trp_data::inventory::Loadout::of_preset(&db, &id)?)?;
            let magazine = item.in_slot(MAGAZINE_SLOT).ok_or_else(|| anyhow::anyhow!("the item has nothing in its magazine slot"))?;
            let weapon = trp_data::weapon::assemble(&mut assets, &db, &id)?;
            anyhow::ensure!(!weapon.skipped.iter().any(|part| part.contains(MAGAZINE_SLOT)), "its magazine was not hung: {:?}", weapon.skipped);
            let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon)?;
            anyhow::ensure!(settings.magazine_capacity > 0, "its logic holds no rounds");
            anyhow::ensure!(weapon.magazine_capacity == Some(settings.magazine_capacity), "the rig says {:?} rounds, the logic {}", weapon.magazine_capacity, settings.magazine_capacity);
            if db.given_magazines.contains(&id) {
                println!("{name}: its stock preset has no magazine; given {} ({} rounds)", magazine.kind.name, settings.magazine_capacity);
            }
            Ok(())
        })();
        if let Err(e) = checked {
            println!("FAIL {name}: {e:#}");
            failed.push(format!("{name}: {e:#}"));
        }
    }
    println!("{counted} weapons fed from magazines, {} given one, {} failed", db.given_magazines.len(), failed.len());
    assert!(failed.is_empty(), "{failed:#?}");
}

/// The empty hands' rig can be what every weapon's rig hangs from (the
/// user, 2026-10-14, night: the AKM to be made when first wanted, as the
/// rest are): it has every bone the arms are skinned to, and every node a
/// weapon's prefab has outside its own tree, which a weapon's rig shares
/// with the one already there, but for nodes that draw nothing, which a
/// rig then has as its own (the Uzi's and the MPX's `Main_camera` under
/// the camera bone).
#[test]
fn the_empty_hands_have_every_bone_a_weapon_shares() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let empty = trp_data::weapon::assemble_prefab(&mut assets, trp_data::weapon::EMPTY_HANDS).expect("the empty hands");
    let has: std::collections::HashSet<String> = (0..empty.hierarchy.nodes.len()).map(|index| empty.hierarchy.path(index)).collect();
    let arms = trp_data::arms::load(&mut assets, "assets/content/hands/bear/bear_hands_skin.bundle").expect("the default arms");
    let missing: Vec<&str> = arms.weapon_bone_paths().into_iter().filter(|path| !has.contains(*path)).collect();
    assert!(missing.is_empty(), "the arms' bones the empty hands lack: {missing:?}");
    let mut lacking = Vec::new();
    for (name, id) in weapons(&db) {
        let weapon = trp_data::weapon::assemble(&mut assets, &db, &id).expect("assembled");
        let outside: Vec<usize> = (0..weapon.hierarchy.nodes.len()).filter(|index| !weapon.hierarchy.path(*index).starts_with("Weapon_root") && !has.contains(&weapon.hierarchy.path(*index))).collect();
        if !outside.is_empty() {
            println!("{name}: its own outside its tree: {:?}", outside.iter().map(|index| weapon.hierarchy.path(*index)).collect::<Vec<_>>());
        }
        if outside.iter().any(|index| weapon.hierarchy.nodes[*index].renderer.is_some()) {
            lacking.push(name);
        }
    }
    println!("the empty hands' {} nodes; {} weapons, {} drawing outside them", has.len(), weapons(&db).len(), lacking.len());
    assert!(lacking.is_empty(), "{lacking:?}");
}
