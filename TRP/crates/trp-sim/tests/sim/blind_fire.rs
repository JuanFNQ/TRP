//! Blind fire (`docs/spec/blind-fire.md`) with the install's body animator,
//! the stock AKM and its motion: the state and its float by the install's
//! curves, the gate's refusals, the keys it holds back, the lean's quirk,
//! the change of side, the blender and the camera's rest, and the weapon's
//! refusals and the operations that end it. These pass trivially, with a
//! note, where no install is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::unity::Assets;
use trp_sim::math::quat_euler;
use trp_sim::{FireMode, Firearm, FirearmEvent, Flat, Hands, Input, Movement, State, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;

fn movement() -> Option<Movement> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = HardSettings::load(&assets.install).expect("settings object");
    let engine = EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement settings");
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    Some(Movement::new(settings, trp_sim::Body::new(&body, 5.0)))
}

/// A frame's input with a firearm at rest in hand.
fn armed() -> Input {
    Input { blind_firearm: true, blind_may_start: true, ..Input::default() }
}

fn run(m: &mut Movement, seconds: f32, input: Input) {
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, &input, &Flat);
    }
}

/// One frame with blind fire's command for a side (1 over the top, -1 to
/// the right, 0 the end).
fn press(m: &mut Movement, side: i32, input: Input) {
    let input = if side == 0 { Input { blind_end: true, ..input } } else { Input { blind_toggle: side, ..input } };
    m.step(DT, &input, &Flat);
}

#[test]
fn over_the_top_and_back_by_the_install_s_curves() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed());
    press(&mut m, 1, armed());
    assert_eq!(m.take_blind_fire(), Some(1), "the gate took it");
    assert_eq!((m.blind_fire(), m.state), (1, State::BlindFire), "standing leads straight in, no blend");
    // The way in plays at 1.3 and holds the float at 0 to 0.15 of it.
    run(&mut m, 0.08, armed());
    assert_eq!(m.blind_fire_animator(), (1.0, 0.0));
    // Past 0.65 of it the float is whole.
    run(&mut m, 0.5, armed());
    assert!((m.blind_fire_animator().1 - 1.0).abs() < 1e-3, "{:?}", m.blind_fire_animator());
    // Let go: the way out holds 1 to 0.15 of its 1.233 s at 1.3, falls to
    // 0 at 0.6, and leads to standing at 0.9 (0.854 s).
    press(&mut m, 0, armed());
    assert_eq!((m.take_blind_fire(), m.blind_fire(), m.state), (Some(0), 0, State::BlindFire));
    run(&mut m, 0.1, armed());
    assert!((m.blind_fire_animator().1 - 1.0).abs() < 1e-3);
    run(&mut m, 0.5, armed());
    assert!(m.blind_fire_animator().1.abs() < 1e-3, "{:?}", m.blind_fire_animator());
    run(&mut m, 0.3, armed());
    assert_eq!(m.state, State::Idle);
}

#[test]
fn refused_walking_without_a_free_weapon_and_in_the_air() {
    let Some(mut m) = movement() else { return };
    // Walking: the walk passes 0, which ends any blind fire.
    run(&mut m, 1.0, Input { move_dir: [0.0, 1.0], ..armed() });
    assert_eq!(m.state, State::Move);
    press(&mut m, 1, Input { move_dir: [0.0, 1.0], ..armed() });
    assert_eq!((m.take_blind_fire(), m.blind_fire(), m.state), (Some(0), 0, State::Move));
    // Standing with the weapon busy: the gate refuses and changes nothing.
    run(&mut m, 1.5, armed());
    assert_eq!(m.state, State::Idle);
    press(&mut m, 1, Input { blind_may_start: false, ..armed() });
    assert_eq!((m.take_blind_fire(), m.blind_fire()), (None, 0));
    run(&mut m, 0.2, armed());
    assert_eq!(m.state, State::Idle);
    // Anything but a firearm puts it to 0.
    press(&mut m, 1, Input { blind_firearm: false, ..armed() });
    assert_eq!((m.take_blind_fire(), m.blind_fire()), (Some(0), 0));
    // In a jump: the jump passes 0.
    m.step(DT, &Input { jump: true, ..armed() }, &Flat);
    run(&mut m, 0.1, armed());
    assert_eq!(m.state, State::Jump);
    press(&mut m, 1, armed());
    assert_eq!((m.take_blind_fire(), m.blind_fire()), (Some(0), 0));
}

#[test]
fn the_movement_keys_are_not_heard_and_the_last_direction_stands() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed());
    press(&mut m, 1, armed());
    let from = m.position;
    run(&mut m, 1.0, Input { move_dir: [0.0, 1.0], ..armed() });
    let moved = ((m.position[0] - from[0]).powi(2) + (m.position[2] - from[2]).powi(2)).sqrt();
    assert!(moved < 1e-3, "moved {moved} m");
    assert_eq!(m.state, State::BlindFire);
    // Let go, the keys are heard again; the way out leads to the walk.
    press(&mut m, 0, Input { move_dir: [0.0, 1.0], ..armed() });
    run(&mut m, 1.0, Input { move_dir: [0.0, 1.0], ..armed() });
    assert_eq!(m.state, State::Move);
}

#[test]
fn a_lean_ends_the_body_s_blind_fire_and_nothing_else() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed());
    press(&mut m, 1, armed());
    run(&mut m, 0.6, armed());
    m.step(DT, &Input { lean_toggle: [false, true], ..armed() }, &Flat);
    assert_eq!(m.blind_fire_animator().0, 0.0, "the animator's side");
    assert_eq!(m.blind_fire(), 1, "the movement's figure stays");
    run(&mut m, 1.2, armed());
    assert_eq!(m.state, State::Idle, "the body played its way out");
    let from = m.position;
    run(&mut m, 0.5, Input { move_dir: [0.0, 1.0], ..armed() });
    assert!((m.position[2] - from[2]).abs() < 1e-3, "still not walking");
    press(&mut m, 0, armed());
    assert_eq!(m.blind_fire(), 0);
}

#[test]
fn over_the_top_to_the_right_never_stands() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, armed());
    press(&mut m, 1, armed());
    run(&mut m, 0.6, armed());
    press(&mut m, -1, armed());
    assert_eq!(m.take_blind_fire(), Some(-1));
    for _ in 0..90 {
        m.step(DT, &armed(), &Flat);
        assert_eq!(m.state, State::BlindFire, "through the way out straight into the right");
    }
    assert!((m.blind_fire_animator().1 + 1.0).abs() < 1e-3, "{:?}", m.blind_fire_animator());
}

/// The AKM's motion and logic from the install.
fn weapon() -> Option<(Hands, Firearm)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let hands = Hands::new(trp_sim::hands_settings(data, db, &hard, &engine, &weapon, ViewSettings::default()).expect("hands settings"));
    let animator = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("the weapon's animator");
    let mut firearm = Firearm::new(trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("firearm settings"), &animator);
    firearm.start_drawn();
    Some((hands, firearm))
}

#[test]
fn the_blender_and_the_camera_s_rest() {
    let Some((mut hands, _)) = weapon() else { return };
    let frame = 1.0 / 120.0;
    let bone = quat_euler([-90.0, 0.0, 0.0]);
    let mut step = |hands: &mut Hands| _ = hands.frame_on_clock(frame, frame, bone, None);
    // The prefab's speed: 3 a second, from the next frame.
    hands.start_blind_fire(1);
    step(&mut hands);
    assert_eq!(hands.blind_fire(), (0.0, 1.0));
    step(&mut hands);
    assert!((hands.blind_fire().0 - 0.025).abs() < 1e-4, "{:?}", hands.blind_fire());
    for _ in 0..40 {
        step(&mut hands);
    }
    assert_eq!(hands.blind_fire().0, 1.0);
    // The rest: the prefab's (-15, -5, 0) over the top, halved at full.
    let zero = hands.camera_rotation.zero;
    assert!((zero[0] + 7.5).abs() < 1e-4 && (zero[1] + 2.5).abs() < 1e-4 && zero[2].abs() < 1e-4, "{zero:?}");
    // Looking 60 degrees up the strength is (90 - 60) / 45.
    hands.turn_rig(0.0, -60.0);
    step(&mut hands);
    let zero = hands.camera_rotation.zero;
    assert!((zero[0] + 5.0).abs() < 1e-3 && (zero[1] + 2.5 * 2.0 / 3.0).abs() < 1e-3, "{zero:?}");
    // To the right: two thirds of a second through 0 to -1, then the
    // prefab's (-2, -15, 0), halved.
    hands.turn_rig(0.0, 0.0);
    hands.start_blind_fire(-1);
    for _ in 0..83 {
        step(&mut hands);
    }
    assert_eq!(hands.blind_fire(), (-1.0, -1.0));
    let zero = hands.camera_rotation.zero;
    assert!((zero[0] + 1.0).abs() < 1e-4 && (zero[1] + 7.5).abs() < 1e-4, "{zero:?}");
}

#[test]
fn the_weapon_refuses_its_keys_and_the_fold_ends_it() {
    let Some((_, mut firearm)) = weapon() else { return };
    assert!(firearm.can_set_blind_fire());
    firearm.set_aim(true, true);
    firearm.step(DT);
    assert!(firearm.aiming());
    // Taken from rest: passed on, and the sights put down without the aim
    // key's call (no `Aimed`).
    assert_eq!(firearm.set_blind_fire(1), 1);
    assert!(firearm.blind_fire() && !firearm.aiming());
    assert!(!firearm.step(DT).iter().any(|event| matches!(event, FirearmEvent::Aimed(_))));
    // Refused: the aim, the fire mode, the looks, the reload.
    firearm.set_aim(true, true);
    assert!(!firearm.aiming());
    assert!(!firearm.set_fire_mode(FireMode::FullAuto));
    assert!(!firearm.check_magazine() && !firearm.check_fire_mode() && !firearm.examine());
    firearm.reload();
    assert!(!firearm.reloading());
    // The fold is not refused, and ends it.
    if firearm.fold_stock() {
        assert!(!firearm.blind_fire());
        assert!(firearm.step(DT).iter().any(|event| matches!(event, FirearmEvent::BlindFireEnded)));
    }
    // In a reload it cannot start: the operation passes 0.
    let Some((_, mut firearm)) = weapon() else { return };
    firearm.reload();
    assert!(firearm.reloading() && !firearm.can_set_blind_fire());
    assert_eq!(firearm.set_blind_fire(1), 0);
    assert!(!firearm.blind_fire());
}
