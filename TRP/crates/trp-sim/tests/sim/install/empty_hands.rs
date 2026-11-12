//! Empty hands read like a weapon, and fists in a sprint.

use super::*;

/// The game's empty hands are read from the install like a weapon: a rig
/// with both arms and the hand markers the arms are solved onto, clips of
/// its own, an animator whose resting state blends two of them by how far
/// down the view points, and settings for the weapon motion that hold no
/// weapon's figures.
#[test]
fn empty_hands_are_read_like_a_weapon() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::setup::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let hands = trp_data::weapons::weapon::assemble_prefab(&mut assets, trp_data::weapons::weapon::EMPTY_HANDS).expect("the empty hands' prefab");
    let has = |path: &str| (0..hands.hierarchy.nodes.len()).any(|i| hands.hierarchy.path(i) == path);
    for bone in [
        "Base HumanLCollarbone/Base HumanLUpperarm",
        "Base HumanRCollarbone/Base HumanRUpperarm",
        "Camera_animated",
        "Weapon_root/Weapon_root_anim/weapon/weapon_L_IK_marker",
        "Weapon_root/Weapon_root_anim/weapon/weapon_R_IK_marker",
        "Weapon_root/Weapon_root_anim/weapon/Bend_Goal_Left",
    ] {
        assert!(has(bone), "the rig has no {bone}");
    }
    assert!(hands.hierarchy.nodes.iter().all(|node| node.renderer.is_none()), "the rig draws nothing of its own");

    let table = trp_data::weapons::states::StateTable::load(&assets, &hands).expect("the empty hands' animator");
    let state = |name: &str| table.find(name).unwrap_or_else(|| panic!("no state {name}"));
    let rest = state("Hands.IDLE");
    assert_eq!(rest.blend_parameter.as_deref(), Some("Aim_angle"));
    assert_eq!(rest.thresholds, [0.0, 90.0]);
    let names: Vec<&str> = rest.clips.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["empty_hands_idle", "empty_hands_idle_down"]);
    assert!(state("Hands.SPAWN").transition_to("Hands.OUT TO IDLE").is_some());
    assert!(table.find("Hands.IN INVENTORY").is_none(), "the empty hands have no pose for an open inventory");
    let appear = state("Hands.OUT TO IDLE");
    let length = trp_data::weapons::states::clip_length(&assets, appear.clips[0].1).expect("clip");
    println!(
        "empty hands: {} bones, {} clips; they appear in {length:.3} s and are there {:.3} s in",
        hands.hierarchy.nodes.len(),
        hands.clips(&assets).expect("clips").len(),
        appear.event_time("WeapIn").unwrap_or(f32::NAN) * length
    );

    let data = trp_data::character::player::HandsData::load(&mut assets).expect("the player prefab's tuning");
    let prefab_center = data.rotation_center;
    let settings = trp_sim::empty_hands_settings(data, &db, &hard, &engine, &hands, trp_sim::ViewSettings::default()).expect("settings");
    assert_eq!((settings.weapon_weight, settings.weapon_length), (0.0, 0.0));
    assert_eq!(settings.rotation_center, prefab_center);
    println!("the prefab's own swing point {prefab_center:?}, position spring damping {}", settings.hand_damping);
    // The motion runs with them: a few seconds standing still stay finite.
    let mut motion = trp_sim::Hands::new(settings);
    for _ in 0..300 {
        motion.advance(DT, &trp_sim::HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..Default::default() });
    }
    let (at, turn) = motion.weapon_transform();
    assert!(at.iter().chain(turn.iter()).all(|v| v.is_finite()), "{at:?} {turn:?}");
}

/// Hands that hold no firearm take the motion as the last firearm left it
/// (`ClearPreviousWeapon`; inventory spec, "Empty hands"): its swing point,
/// its recoil pivot, its damping, its ergonomic weight and the sway's
/// strength as last worked out, with the displacement's strength put to
/// 0.5; both are worked out again from the weight left at the next change
/// of stance.
#[test]
fn empty_hands_keep_what_the_last_firearm_left() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::setup::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let empty = trp_data::weapons::weapon::assemble_prefab(&mut assets, trp_data::weapons::weapon::EMPTY_HANDS).expect("the empty hands' prefab");
    let akm = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = trp_data::character::player::HandsData::load(&mut assets).expect("the player prefab's tuning");
    let view = trp_sim::ViewSettings::default();
    let rifle = trp_sim::Hands::new(trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &akm, view).expect("the AKM's motion"));
    let mut hands = trp_sim::Hands::new(trp_sim::empty_hands_settings(data, &db, &hard, &engine, &empty, view).expect("the empty hands' motion"));
    let kept = rifle.kept();
    hands.keep_from(kept);
    assert_eq!(hands.kept(), kept, "everything the rifle left is kept");
    assert_eq!(hands.settings.rotation_center, rifle.settings.rotation_center);
    let (sway, displacement) = hands.sway_strengths();
    assert_eq!((sway, displacement), (rifle.sway_strengths().0, 0.5));
    // Standing on, nothing changes them.
    let standing = trp_sim::HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..Default::default() };
    for _ in 0..60 {
        hands.advance(DT, &standing);
    }
    assert_eq!(hands.sway_strengths(), (sway, 0.5));
    // Lying down works both out again from the rifle's ergonomic weight.
    hands.advance(DT, &trp_sim::HandsInput { prone: true, pose_level: 0.0, smoothed_pose_level: 0.0, ..Default::default() });
    let (sway_after, displacement_after) = hands.sway_strengths();
    let weight = kept.ergonomic_weight * (1.0 + hands.settings.overweight);
    assert!((sway_after - hands.settings.sway_strength_per_kg.evaluate(weight)).abs() < 1e-6, "{sway_after}");
    assert!((displacement_after - hands.settings.displacement_strength_per_kg.evaluate(weight)).abs() < 1e-6, "{displacement_after}");
    assert!((sway_after - sway).abs() < 1e-6, "the rifle's own sway: {sway} {sway_after}");
    assert!(displacement_after != 0.5);
    println!("kept of the AKM: {kept:?}; displacement 0.5, then {displacement_after:.3} lying");
}

/// The body's animator is told of an open inventory as it is of a reload:
/// lying down, the view does not turn while it is open.
#[test]
fn an_open_inventory_holds_a_lying_body_as_a_reload_does() {
    let Some(mut m) = movement(0.0) else { return };
    m.step(DT, &Input { prone: true, ..Input::default() }, &Flat);
    run(&mut m, 2.5, Input::default());
    assert_eq!(m.state, State::ProneIdle);
    let open = Input { inventory: true, ..Input::default() };
    run(&mut m, 0.5, open);
    let before = m.yaw;
    m.step(DT, &Input { look_delta: [9.0, 0.0], ..open }, &Flat);
    assert_eq!(m.yaw, before);
    run(&mut m, 0.5, Input::default());
    m.step(DT, &Input { look_delta: [9.0, 0.0], ..Input::default() }, &Flat);
    assert!(m.yaw > before);
}

/// Told that the hands are empty, the body's animator plays the sprint's
/// own clip for empty hands in the layer that has the arms: both hands are
/// then the body's, shoulder to palm, and the left hand's fingers, which go
/// with the hand, are closed (body-animator spec, "The kind of thing in
/// hand"). With the rifle the hands stay the weapon's. The figure the
/// trees go by follows the new kind over most of a second. (A firearm one
/// cell wide is given the same clip, so telling the animator the width the
/// game's empty hands tell it had the same pose before the kind was told.)
#[test]
fn empty_hands_are_fists_in_a_sprint() {
    use trp_sim::locomotion::body::hands_kind;
    let (Some(mut rifle), Some(mut empty)) = (movement(0.0), movement(0.0)) else { return };
    // The game's empty hands tell the animator a width of one cell.
    empty.body.set_weapon_width(1.0);
    empty.body.set_hands_kind(hands_kind::EMPTY_HANDS);
    empty.step(DT, &Input::default(), &Flat);
    let first = empty.body.hands_kind();
    assert!((first - 4.0 * 7.0 * DT).abs() < 1e-5, "one frame on: {first}");
    let mut frames = 1;
    while empty.body.hands_kind() != hands_kind::EMPTY_HANDS {
        empty.step(DT, &Input::default(), &Flat);
        frames += 1;
        assert!(frames < 600);
    }
    let sprint = Input { sprint: true, ..walk([0.0, 1.0]) };
    for m in [&mut rifle, &mut empty] {
        run(m, 0.5, walk([0.0, 1.0]));
        run(m, 3.0, sprint);
        assert_eq!(m.state, State::Sprint);
    }
    // How far a joint is bent from straight, in degrees.
    let bend = |q: [f32; 4]| 2.0 * q[3].abs().min(1.0).acos().to_degrees();
    // The index finger's three joints follow the thumb's.
    let index = |m: &Movement| {
        let fingers = m.arm_pose(false).fingers.expect("the left hand's fingers");
        [bend(fingers[3]), bend(fingers[4]), bend(fingers[5])]
    };
    let hands = |m: &Movement| [m.arm_pose(false).hand, m.arm_pose(true).hand];
    println!(
        "the animator's figure for empty hands: {first:.3} after a frame, 4 after {frames} frames ({:.2} s)",
        frames as f32 * DT
    );
    println!(
        "sprinting, the body's share of the hands (left, right): a rifle {:?}, empty hands {:?}",
        hands(&rifle),
        hands(&empty)
    );
    println!("the left index finger's joints, degrees: a rifle {:.0?}, empty hands {:.0?}", index(&rifle), index(&empty));
    assert_eq!(hands(&rifle), [0.0, 0.0]);
    assert_eq!(hands(&empty), [1.0, 1.0]);
    assert!(index(&empty)[1] > 100.0 && index(&rifle)[1] < 90.0);
    // Standing again, the hands are the rig's own once more.
    run(&mut empty, 2.0, Input::default());
    assert_eq!(empty.state, State::Idle);
    assert_eq!(hands(&empty), [0.0, 0.0]);
}
