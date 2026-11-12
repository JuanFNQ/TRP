//! Tests of a part with its own animations, on each weapon whose animator goes by one.
//! After the firing spec, "A part with its own animations", through `UniqueAnimationModID`:
//! each weapon whose animator goes by it,
//! run on its own animator from the install with the part and without, the
//! part put into its stock build where the game's slots take it. These pass
//! trivially, with a note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test sim unique_animations:: --
//! --nocapture` for what each does.

use trp_data::setup::database::PresetItem;
use trp_data::unity::Assets;
use trp_sim::{Firearm, FirearmEvent};

const FRAME: f32 = 1.0 / 120.0;

/// A weapon's stock build with parts put in: each `(parent's slot, slot,
/// template)`, the parent the weapon itself where its slot is empty; a part
/// already in that slot is replaced. The firearm drawn from it, and the
/// build's largest id.
fn firearm_of(weapon: &str, parts: &[(&str, &str, &str)]) -> Option<(Firearm, i32)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database()?;
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let mut preset = db.default_preset(weapon).expect("its stock build").clone();
    for &(parent_slot, slot, template) in parts {
        let parent = if parent_slot.is_empty() { preset.root.clone() } else { preset.items.iter().find(|i| i.slot.as_deref() == Some(parent_slot)).expect("the parent").id.clone() };
        match preset.items.iter_mut().find(|i| i.parent.as_deref() == Some(parent.as_str()) && i.slot.as_deref() == Some(slot)) {
            Some(item) => item.template = template.to_owned(),
            None => preset.items.push(PresetItem { id: format!("added_{slot}"), template: template.to_owned(), parent: Some(parent), slot: Some(slot.to_owned()) }),
        }
    }
    let weapon = trp_data::weapons::weapon::assemble_preset(&mut assets, db, &preset).expect("the weapon");
    let data = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("its settings");
    println!("{} with {:?}: id {}", weapon.name, parts, weapon.unique_animation);
    let unique = weapon.unique_animation;
    let mut w = Firearm::new(settings, &data);
    w.found();
    Some((w, unique))
}

/// Steps a weapon and gives the states its animator entered, in order.
fn run(w: &mut Firearm, seconds: f32) -> Vec<String> {
    let name = |w: &Firearm| {
        let (state, _, next) = w.animator().state();
        next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
    };
    let (mut entered, mut last) = (Vec::new(), name(w));
    for _ in 0..(seconds / FRAME).round() as usize {
        let _: Vec<FirearmEvent> = w.step(FRAME);
        let now = name(w);
        if now != last {
            entered.push(now.clone());
            last = now;
        }
    }
    entered
}

/// The PP-19-01's VR-LPS (id 6): its left-side handle is worked by the left
/// hand in its own states, and holds the bolt open in a reload from empty.
#[test]
fn the_pp19_s_left_side_charging_handle_has_its_own_states() {
    const PP19: &str = "59984ab886f7743e98271174";
    const VR_LPS: &str = "65ae4f57e343f0acc00824da";
    let Some((mut stock, 0)) = firearm_of(PP19, &[]) else { return };
    let Some((mut w, 6)) = firearm_of(PP19, &[("", "mod_gas_block", VR_LPS)]) else { panic!("the VR-LPS's id") };
    assert!(stock.tells_body_of_aim() && !w.tells_body_of_aim());
    // Drawn as found, its chamber empty: charged on the way up.
    stock.draw();
    w.draw();
    let (plain, left) = (run(&mut stock, 4.0), run(&mut w, 4.0));
    println!("  drawn: {plain:?} / {left:?}");
    assert_eq!(plain.first().map(String::as_str), Some("OUT TO IDLE ARM"));
    assert_eq!(left.first().map(String::as_str), Some("OUT TO IDLE ARM BOLT"));
    assert!(w.chamber && w.is_idle());
    // Its chamber checked.
    w.check_chamber();
    let entered = run(&mut w, 4.0);
    println!("  chamber checked: {entered:?}");
    assert_eq!(entered.first().map(String::as_str), Some("CHECK CHAMBER LEFT"));
    // Reloaded from empty: the bolt drawn back and held, the magazine
    // changed, the bolt let go on a round.
    (w.chamber, w.magazine) = (false, Some(0));
    w.reload();
    let entered = run(&mut w, 10.0);
    println!("  reloaded from empty: {entered:?}; {:?} + {}", w.magazine, w.chamber as u8);
    assert_eq!(entered.first().map(String::as_str), Some("BOLT IN"));
    assert!(entered.iter().any(|s| s == "BOLT OUT"), "{entered:?}");
    assert!(w.chamber && w.is_idle() && !w.bolt_caught());
    // With a round chambered, its reload is the plain one.
    w.reload();
    let entered = run(&mut w, 10.0);
    println!("  reloaded: {entered:?}");
    assert_eq!(entered.first().map(String::as_str), Some("RELOAD START"));
    assert!(w.chamber && w.is_idle());
}

/// The M4A1's Magpul BAD lever (id 13): the bolt let go from its catch by
/// the lever, at the end of a reload from empty.
#[test]
fn the_m4a1_s_bad_lever_lets_the_bolt_go_its_own_way() {
    const M4A1: &str = "5447a9cd4bdc2dbd208b4567";
    const BAD_LEVER: &str = "675307301f7c19a9780f2668";
    for (parts, end) in [(&[][..], "RELOAD CATCH END"), (&[("", "mod_charge_001", BAD_LEVER)][..], "RELOAD CATCH ALT END")] {
        let Some((mut w, unique)) = firearm_of(M4A1, parts) else { return };
        assert_eq!(unique, if parts.is_empty() { 0 } else { 13 });
        w.draw();
        run(&mut w, 4.0);
        // The last round fired: the bolt stays back.
        (w.chamber, w.magazine) = (true, Some(0));
        w.set_trigger(true, true);
        run(&mut w, 0.3);
        w.set_trigger(false, true);
        run(&mut w, 2.0);
        assert!(w.bolt_caught() && !w.chamber);
        w.reload();
        let entered = run(&mut w, 10.0);
        println!("  reloaded from empty: {entered:?}");
        assert!(entered.iter().any(|s| s == end), "{entered:?}");
        assert!(w.chamber && w.is_idle() && !w.bolt_caught());
    }
}

/// The PM's FAB Defence PM-G grip (id 10): its magazine taken out and
/// checked in states of their own.
#[test]
fn the_pm_s_fab_grip_has_its_own_magazine_states() {
    const PM: &str = "5448bd6b4bdc2dfc2f8b4569";
    const PM_G: &str = "637784c5f7b3f4ac1a0d1a9a";
    for (parts, out, check) in [(&[][..], "RELOAD OUT", "CHECK"), (&[("", "mod_pistolgrip", PM_G)][..], "RELOAD OUT FAB", "CHECK FAB")] {
        let Some((mut w, unique)) = firearm_of(PM, parts) else { return };
        assert_eq!(unique, if parts.is_empty() { 0 } else { 10 });
        w.draw();
        run(&mut w, 4.0);
        w.reload();
        let entered = run(&mut w, 8.0);
        println!("  reloaded: {entered:?}");
        assert!(entered.iter().any(|s| s == out), "{entered:?}");
        assert!(w.is_idle());
        assert!(w.check_magazine());
        let entered = run(&mut w, 6.0);
        println!("  magazine checked: {entered:?}");
        assert_eq!(entered.first().map(String::as_str), Some(check));
    }
}

/// The Uzi's IWI Type 6 stock (id 11): it folds to the side.
#[test]
fn the_uzi_s_type_6_stock_folds_to_the_side() {
    const UZI: &str = "66992b349950f5f4cd06029f";
    const TYPE_6: &str = "669924a69950f5f4cd060295";
    for (parts, fold) in [(&[][..], "STOCK FOLD"), (&[("", "mod_stock", TYPE_6)][..], "STOCK FOLD SIDE")] {
        let Some((mut w, _)) = firearm_of(UZI, parts) else { return };
        w.draw();
        run(&mut w, 4.0);
        assert!(w.fold_stock(), "the stock folds");
        let entered = run(&mut w, 4.0);
        println!("  folded: {entered:?}");
        assert_eq!(entered.first().map(String::as_str), Some(fold));
    }
}

/// The PKM's Zenit B-25U grip on the B-50 handguard (id 2): the sights go
/// up as on any other weapon, not by the animator's own aiming states, and
/// its clips are the foregrip's.
#[test]
fn the_pkm_s_foregrip_keeps_its_sights_from_the_animator() {
    const PKM: &str = "64637076203536ad5600c990";
    const B50: &str = "6491c6f6ef312a876705191b";
    const B25U: &str = "5c1cd46f2e22164bef5cfedb";
    for (parts, aim_in) in [(&[][..], true), (&[("mod_barrel", "mod_handguard", B50), ("mod_handguard", "mod_foregrip", B25U)][..], false)] {
        let Some((mut w, unique)) = firearm_of(PKM, parts) else { return };
        assert_eq!(unique, if parts.is_empty() { 0 } else { 2 });
        w.draw();
        run(&mut w, 5.0);
        w.set_aim(true, true);
        let entered = run(&mut w, 2.0);
        println!("  aimed: {entered:?}; IsAimingFloat {}, UniqueAnimationModIdFloat {}", w.animator().get("IsAimingFloat"), w.animator().get("UniqueAnimationModIdFloat"));
        assert_eq!(entered.iter().any(|s| s == "AIM IN"), aim_in, "{entered:?}");
        assert_eq!(w.animator().get("IsAimingFloat"), if aim_in { 1.0 } else { 0.0 });
        assert_eq!(w.animator().get("UniqueAnimationModIdFloat"), if aim_in { 0.0 } else { 1.0 });
    }
}

/// The VR-LPS is moved by the weapon's own clips (an animated part's bones
/// keep their names, `ObjectsFactory.CreateItemAsync`): the left-hand
/// handle's clip has tracks on its bones in the build's tree.
#[test]
fn the_vr_lps_handle_is_moved_by_the_weapon_s_clips() {
    use trp_data::unity::anim::{path_hash, Clip};
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let mut preset = db.default_preset("59984ab886f7743e98271174").expect("the PP-19's stock build").clone();
    preset.items.iter_mut().find(|i| i.slot.as_deref() == Some("mod_gas_block")).expect("its gas block").template = "65ae4f57e343f0acc00824da".into();
    let weapon = trp_data::weapons::weapon::assemble_preset(&mut assets, db, &preset).expect("the PP-19");
    let h = &weapon.hierarchy;
    let root = h.nodes.iter().position(|n| n.name == "gas_block_pp-19-01_vector_vr_lps_p").expect("the VR-LPS placed");
    let below = |mut i: usize| loop {
        if i == root {
            return true;
        }
        match h.nodes[i].parent {
            Some(p) => i = p,
            None => return false,
        }
    };
    let own: std::collections::HashSet<u32> = (0..h.nodes.len()).filter(|&i| below(i)).map(|i| path_hash(&h.path(i))).collect();
    let (_, clip) = weapon.clips(&assets).expect("its clips").into_iter().find(|(_, obj)| Clip::read(&assets, *obj).is_ok_and(|c| c.name == "pp-19-01_bolt_open")).expect("the handle's clip");
    let clip = Clip::read(&assets, clip).expect("the clip");
    let on_part = clip.tracks.iter().filter(|t| own.contains(&t.path)).count();
    println!("pp-19-01_bolt_open: {} tracks, {on_part} on the VR-LPS's {} bones", clip.tracks.len(), own.len());
    assert!(on_part > 0);
}
