//! Weapons built into slots, weapon skills, sight lines and parts on bones.

use super::*;

/// Every weapon the app builds (`trp-app`'s `WEAPONS`): its motion and its
/// logic are read from the install, a crate of ten cells by thirty holds
/// the four with a vest and a backpack, and put on, each goes to the slot
/// of its kind: the pistol to the holster.
#[test]
fn the_four_weapons_are_built_and_find_their_slots() {
    use trp_data::character::inventory::Loadout;
    use trp_sim::items::inventory::{equip, Equipment, Item, Kind, Rules};
    use std::sync::Arc;
    const WEAPONS: [(&str, &str); 4] = [
        ("59d6088586f774275f37482f", "FirstPrimaryWeapon"),
        ("6259b864ebedf17603599e88", "SecondPrimaryWeapon"),
        ("5d3eb3b0a4b93615055e84d2", "Holster"),
        ("5bfea6e90db834001b7347f3", ""),
    ];
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let engine = trp_data::setup::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let data = trp_data::character::player::HandsData::load(&mut assets).expect("weapon motion data");
    let rules = Rules::of(&db);
    let mut crate_kind = Kind::of(&db, "6223349b3136504a544d1608").expect("the airdrop crate");
    crate_kind.grids[0].cells = [10, 30];
    let mut loot = Item::new(Arc::new(crate_kind));
    let mut worn = Equipment::empty(&db).expect("the equipment's template").item;
    for (template, slot) in WEAPONS {
        let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, template).expect("the weapon");
        let hands = trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &weapon, trp_sim::ViewSettings::default()).expect("its motion settings");
        let logic = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its logic's settings");
        trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its animator");
        println!(
            "{}: {} cells, {:.2} kg, ergonomics {}, a pistol {}, held closer {}, line {:.3} m, magazine {}",
            weapon.name, weapon.cell_width, weapon.total_weight, weapon.total_ergonomics, hands.pistol, hands.held_closer, hands.weapon_length, logic.magazine_capacity
        );
        assert_eq!(hands.pistol, template == "5d3eb3b0a4b93615055e84d2");
        assert!(hands.weapon_length > 0.15 && logic.magazine_capacity > 0);
        let (item, left) = Loadout::of_preset(&db, template).and_then(|list| Item::of_list(&db, &list)).expect("its preset as an item");
        assert!(left.is_empty(), "{left:?}");
        let id = item.id;
        assert!(loot.stow(item).is_ok(), "the crate has no room for {}", weapon.name);
        // Put on with Alt and a click: the first slot of its kind that is free.
        let mut roots = [&mut worn, &mut loot];
        match equip(&mut roots, id, &rules) {
            Ok(into) => assert_eq!(roots[0].kind.slots[into].name, slot, "{}", weapon.name),
            // The third long weapon has no slot left.
            Err(refusal) => assert!(slot.is_empty(), "{}: {refusal:?}", weapon.name),
        }
    }
    let hangs: Vec<&str> = trp_sim::items::inventory::HAND_SLOTS.into_iter().filter(|slot| worn.in_slot(slot).is_some()).collect();
    assert_eq!(hangs, ["FirstPrimaryWeapon", "SecondPrimaryWeapon", "Holster"]);
    // The game's own words for a door's menu are in the install.
    let texts = trp_data::character::inventory::texts(&assets.install, "en", &["OpenDoor", "CloseDoor", "Breach", "Bang & clear", "Flash & clear", "Move in", "Search", "Unlock", "Take"]).expect("the texts");
    println!("the menu's words: {texts:?}");
    assert_eq!(texts.len(), 9);
    assert_eq!(texts["opendoor"].to_uppercase(), "OPEN");
}

/// The character's skill with a class of weapon speeds its reloading and
/// its drawing: the animator's own states go by two of its parameters,
/// which the game sets from the skill's level (`docs/spec/firing.md`,
/// "The skill's two speeds"). From the game's code and the install; the
/// recordings were all made by a character with no such skill.
#[test]
fn a_weapon_skill_speeds_the_reload_and_the_draw() {
    use trp_sim::weapons::firearm::weapon_skill_speeds;
    use trp_sim::{Firearm, FirearmEvent};
    assert_eq!(weapon_skill_speeds(0), (1.0, 1.0));
    let (reload, draw) = weapon_skill_speeds(50);
    assert!((reload - 1.2).abs() < 1e-6 && (draw - 1.4).abs() < 1e-6);
    for (template, name) in [("5d3eb3b0a4b93615055e84d2", "the Five-seveN"), ("5bfea6e90db834001b7347f3", "the Model 700"), ("59d6088586f774275f37482f", "the AKM")] {
        let Some((_, data, settings, ..)) = weapon_of(template) else { return };
        let times = |level: u32| {
            let (reload_speed, draw_speed) = weapon_skill_speeds(level);
            let mut w = Firearm::new(trp_sim::FirearmSettings { reload_speed, draw_speed, ..settings.clone() }, &data);
            w.draw();
            let (_, said) = run_weapon(&mut w, 4.0);
            let drawn = said.iter().find(|(event, _)| *event == FirearmEvent::Appeared).expect("appeared").1;
            if settings.internal_magazine {
                w.magazine = Some(settings.magazine_capacity - 1);
                w.reload_shells(10);
            } else {
                w.reload();
            }
            let (entered, _) = run_weapon(&mut w, 12.0);
            assert!(w.is_idle(), "{name}: {entered:?}");
            (drawn, entered.last().expect("states").1)
        };
        let (plain, skilled) = (times(0), times(50));
        println!(
            "{name}: drawn in {:.2} s and {:.2} s at level 50 ({:.2} times as fast); reloaded in {:.2} s and {:.2} s ({:.2} times)",
            plain.0, skilled.0, plain.0 / skilled.0, plain.1, skilled.1, plain.1 / skilled.1
        );
        // Faster, and by no more than the parameters' own factors.
        assert!(skilled.0 < plain.0 && plain.0 / skilled.0 < 1.42, "{plain:?} {skilled:?}");
        assert!(skilled.1 < plain.1 && plain.1 / skilled.1 < 1.22, "{plain:?} {skilled:?}");
    }
}

/// The eye is lined up with the iron sights, not with the weapon's own
/// aim bone (`docs/spec/weapon-handling.md`, "The sights"): the rear
/// sight's aim bone, moved onto the line through the rear and the front
/// sight. On the AKM that is a millimetre above the weapon's own bone; on
/// the M3 and the Five-seveN it is centimetres from it. A build with no
/// sight at all aims by the weapon's own bone.
#[test]
fn the_eye_is_lined_up_with_the_iron_sights() {
    use trp_sim::base::rig::{SightNodes, WeaponRig};
    for (template, name, off) in [
        ("59d6088586f774275f37482f", "the AKM", 0.0011),
        ("6259b864ebedf17603599e88", "the M3", 0.0133),
        ("5d3eb3b0a4b93615055e84d2", "the Five-seveN", 0.0120),
    ] {
        let Some((weapon, ..)) = weapon_of(template) else { return };
        let nodes = SightNodes::of_weapon(&weapon).expect("its sights");
        let rig = WeaponRig::new(&weapon.hierarchy);
        let h = &weapon.hierarchy;
        assert_eq!(h.nodes[nodes.bone].name, "mod_aim_camera", "{name}");
        let line = nodes.line(&rig).expect("a rear and a front sight");
        let bone = nodes.bone_on_weapon(&rig);
        let own = (0..h.nodes.len()).find(|&i| h.nodes[i].name == "aim_camera").map(|own| SightNodes { bone: own, ..nodes }.bone_on_weapon(&rig)).expect("the weapon's own aim bone");
        // Across and up, the eye against the weapon's own bone.
        let gap = ((bone[0] - own[0]).powi(2) + (bone[2] + line.shift[1] - own[2]).powi(2)).sqrt();
        println!("{name}: the sights' line is pitched {:.4} degrees, the eye {:.2} mm up from the sight's bone and {:.1} mm from the weapon's own", line.angle, line.shift[1] * 1000.0, gap * 1000.0);
        assert!(line.angle.abs() < 0.1 && line.shift[0].abs() < 1e-5, "{line:?}");
        assert!((gap - off).abs() < 0.0006, "{name}: {gap}");
        assert!(nodes.muzzle.is_some());
    }
    // The Model 700's stock preset has no sight.
    let Some((weapon, ..)) = weapon_of("5bfea6e90db834001b7347f3") else { return };
    let nodes = SightNodes::of_weapon(&weapon).expect("an aim bone");
    assert_eq!(weapon.hierarchy.nodes[nodes.bone].name, "aim_camera");
    assert!(nodes.line.is_none());
}

/// The way the eye looks along a sight's aim bone (`SightNodes::
/// line_on_weapon`), along which the eye's target slides in automatic fire
/// (`docs/spec/weapon-handling.md`, "Camera"): the bone's -z, or -y for the
/// weapon's own `aim_camera`. On the stock weapons read it runs along the
/// barrel, the weapon's own -y: the iron sights' bone, an optic's, a
/// variable scope's and a rifle with no sight at all.
#[test]
fn a_sight_s_line_runs_along_the_barrel() {
    use trp_sim::base::rig::{SightNodes, WeaponRig};
    for (template, name, own) in [
        ("59d6088586f774275f37482f", "the AKM", false),
        ("661cec09b2c6356b4d0c7a36", "the M60E6", false),
        ("65290f395ae2ae97b80fdf2d", "the SPEAR", false),
        ("5bfea6e90db834001b7347f3", "the Model 700", true),
    ] {
        let Some((weapon, ..)) = weapon_of(template) else { return };
        let nodes = SightNodes::of_weapon(&weapon).expect("its sights");
        let rig = WeaponRig::new(&weapon.hierarchy);
        let line = nodes.line_on_weapon(&rig);
        let off = (-line[1]).clamp(-1.0, 1.0).acos().to_degrees();
        println!("{name}: {} ({}), its line {:.4} {:.4} {:.4}, {off:.3} degrees off the barrel", weapon.hierarchy.nodes[nodes.bone].name, if nodes.own { "-y" } else { "-z" }, line[0], line[1], line[2]);
        assert_eq!(nodes.own, own, "{name}");
        assert!((line.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 1e-4);
        assert!(off < 0.5, "{name}: {off}");
    }
}

/// A part goes on its bone as the game puts it there (`docs/spec/
/// hands-on-weapon.md`, "A part on its bone"): a quarter turn about x from
/// the bone, or where the part's own `ModPlacer` says. The M3's stock and
/// forend say so for themselves and its pistol grip, which hangs on the
/// stock, does not: so the grip is behind the trigger and below it, in the
/// right hand, where a rule that stood every part upright had it ahead of
/// the trigger guard. And with the rule every weapon's idle clip begins in
/// the build's own rest pose, the parts' roots included.
#[test]
fn a_part_goes_on_its_bone_by_the_game_s_rule() {
    use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
    use trp_data::unity::extract::read_mesh;
    use trp_sim::base::rig::WeaponRig;
    let quarter = [0.5f32.sqrt(), 0.0, 0.0, 0.5f32.sqrt()];
    let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(&b).map(|(a, b)| a * b).sum::<f32>().abs() > 0.999_99;
    let Some((weapon, _, _, _, assets)) = weapon_of("6259b864ebedf17603599e88") else { return };
    let h = &weapon.hierarchy;
    let named = |name: &str| (0..h.nodes.len()).find(|&i| h.nodes[i].name == name).unwrap_or_else(|| panic!("no {name}"));
    let (stock, forend, grip) = (
        named("stock_m3_benelli_5_position_telescoping_stock"),
        named("handguard_m3_benelli_forend_std"),
        named("pistolgrip_m3_benelli_telescopic_stock_handle"),
    );
    for part in [stock, forend] {
        let placer = h.nodes[part].placer.expect("its own place on the bone");
        assert_eq!((placer.rotation, placer.scale), ([0.0; 3], [1.0; 3]), "{}", h.nodes[part].name);
        assert!(near(h.nodes[part].rotation, [0.0, 0.0, 0.0, 1.0]));
    }
    assert!(h.nodes[grip].placer.is_none());
    assert!(near(h.nodes[grip].rotation, quarter), "{:?}", h.nodes[grip].rotation);
    // The grip's middle in the weapon's own bone (the barrel along -y, up
    // z), against the trigger's pivot.
    let rig = WeaponRig::new(h);
    let (origin, turn) = rig.placed(rig.bone("Weapon_root/Weapon_root_anim/weapon").expect("the weapon's own bone"));
    let back = [-turn[0], -turn[1], -turn[2], turn[3]];
    let on_weapon = |at: [f32; 3]| trp_sim::base::math::quat_rotate(back, std::array::from_fn(|k| at[k] - origin[k]));
    let mesh_node = (0..h.nodes.len()).find(|&i| h.nodes[i].parent == Some(grip) && h.nodes[i].renderer.is_some()).expect("the grip's mesh");
    let mesh = read_mesh(&assets, h.nodes[mesh_node].renderer.as_ref().unwrap().mesh).expect("mesh");
    let (at, turn) = rig.placed(mesh_node);
    let trigger = on_weapon(rig.placed(named("weapon_trigger")).0);
    // How far behind the trigger's pivot its nearest and its farthest
    // point are, and how far below the pivot its lowest.
    let (mut nearest, mut farthest, mut lowest) = (f32::MAX, f32::MIN, f32::MIN);
    for vertex in &mesh.positions {
        let there = trp_sim::base::math::quat_rotate(turn, *vertex);
        let there = on_weapon(std::array::from_fn(|k| at[k] + there[k]));
        nearest = nearest.min(there[1] - trigger[1]);
        farthest = farthest.max(there[1] - trigger[1]);
        lowest = lowest.max(trigger[2] - there[2]);
    }
    println!("the M3's pistol grip: from {:.1} to {:.1} cm behind the trigger's pivot, down to {:.1} cm below it", nearest * 100.0, farthest * 100.0, lowest * 100.0);
    assert!(nearest > 0.0 && farthest < 0.15, "{nearest} {farthest}");
    assert!((0.05..0.15).contains(&lowest), "{lowest}");
    // Every weapon built: no bone its idle clip turns is turned otherwise
    // at rest, and no part without a place of its own is anything but a
    // quarter turn from its bone.
    for (template, idle) in [
        ("59d6088586f774275f37482f", "akm_idle_armed"),
        ("6259b864ebedf17603599e88", "m3s90_idle_armed"),
        ("5d3eb3b0a4b93615055e84d2", "fiveseven_idle"),
        ("5bfea6e90db834001b7347f3", "m700_idle_armed"),
    ] {
        let Some((weapon, _, _, _, assets)) = weapon_of(template) else { return };
        let h = &weapon.hierarchy;
        let nodes: std::collections::HashMap<u32, usize> = (0..h.nodes.len()).map(|i| (path_hash(&h.path(i)), i)).collect();
        let clips = weapon.clips(&assets).expect("clips");
        let clip = Clip::read(&assets, clips.iter().find(|(name, _)| name == idle).unwrap_or_else(|| panic!("no {idle}")).1).expect("the clip");
        let mut values = Vec::new();
        clip.sample(0.0, &mut values);
        let mut turned = 0;
        for track in clip.tracks.iter().filter(|track| track.property == Property::Euler) {
            let Some(&node) = nodes.get(&track.path) else { continue };
            let there = euler_to_quat([values[track.curve], values[track.curve + 1], values[track.curve + 2]]);
            let dot: f32 = there.iter().zip(&h.nodes[node].rotation).map(|(a, b)| a * b).sum();
            let off = 2.0 * dot.abs().min(1.0).acos().to_degrees();
            assert!(off < 1.0, "{idle}: {} is {off} degrees from where it rests", h.path(node));
            turned += 1;
        }
        // A part's root: what hangs on a bone named for a slot and is not
        // one of the prefab's own bones of that kind.
        let parts: Vec<usize> = (0..h.nodes.len())
            .filter(|&i| h.nodes[i].parent.is_some_and(|parent| h.nodes[parent].name.starts_with("mod_")) && !h.nodes[i].name.starts_with("mod_"))
            .filter(|&i| h.nodes[i].scripts.iter().any(|script| script == "PreviewPivot"))
            .collect();
        let placed = parts.iter().filter(|&&i| h.nodes[i].placer.is_some()).count();
        for &part in parts.iter().filter(|&&i| h.nodes[i].placer.is_none()) {
            assert!(near(h.nodes[part].rotation, quarter), "{}", h.path(part));
        }
        println!("{idle}: {turned} bones turned by the clip rest as it has them; {} parts, {placed} with a place of their own", parts.len());
        assert!(parts.len() >= 3, "{}", parts.len());
    }
}
