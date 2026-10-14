//! A character seen from outside, measured with the real install's data
//! and the stock AKM (`docs/spec/third-person.md`). These pass trivially,
//! with a note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test third_person -- --nocapture` for
//! the numbers.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::{effector, HandsData};
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::{Firearm, FirearmEvent, Flat, Hands, HandsInput, Input, Movement, PointOfView, State, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;

fn rig() -> Option<(Hands, Movement, Firearm)> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let hands = Hands::new(
        trp_sim::hands_settings(data, &db, &hard, &engine, &weapon, ViewSettings::default()).expect("hands settings"),
    );
    let animator = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("the weapon's animator");
    let mut firearm = Firearm::new(trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("firearm settings"), &animator);
    firearm.start_drawn();
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    let movement = Movement::new(
        trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement"),
        trp_sim::Body::new(&body, weapon.cell_width as f32),
    );
    Some((hands, movement, firearm))
}

fn run(m: &mut Movement, seconds: f32, input: Input) {
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, &input, &Flat);
    }
}

fn still() -> HandsInput {
    HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..HandsInput::default() }
}

fn length(v: [f32; 3]) -> f32 {
    v.iter().map(|c| c * c).sum::<f32>().sqrt()
}

fn angle(q: [f32; 4]) -> f32 {
    2.0 * q[3].abs().min(1.0).acos().to_degrees()
}

/// The layer of the body's animator of that name, as `describe` has it.
fn layer(m: &Movement, name: &str) -> String {
    m.body.describe().into_iter().find(|line| line.starts_with(name)).unwrap_or_default()
}

/// The prefab's figures a character seen from outside goes by.
#[test]
fn the_prefab_has_figures_for_a_character_seen_from_outside() {
    let Some((hands, _, _)) = rig() else { return };
    let d = &hands.settings.data;
    println!("the weapon motion's mask: {} ({:#b})", d.mask, d.mask);
    assert_eq!(d.mask & (effector::SHOOTING | effector::DRAW_DOWN | effector::AIMING), effector::SHOOTING | effector::DRAW_DOWN | effector::AIMING);
    println!("the aim pose: {} curves, blended at {} a second", d.aim_pose.len(), d.aim_pose_speed);
    for entry in &d.aim_pose {
        let keys: Vec<String> = entry.curve.keys.iter().map(|k| format!("{:.3}: {:.3}", k.time, k.value)).collect();
        let vals: Vec<String> = entry.vals.iter().map(|v| format!("spring {} component {} by {}", v.target, v.component, v.intensity)).collect();
        println!("  [{}] -> {}", keys.join(", "), vals.join("; "));
    }
    assert_eq!(d.aim_pose.len(), 2);
    assert!((d.aim_pose_speed - 4.0).abs() < 1e-6);
    let back = &d.turn_away.rifle.position[1];
    println!("the rifle's turn-away draws it back {} m for its own eyes and {} m for another's", back.near, back.outside[0]);
    assert!(back.outside[0] > 0.0 && back.outside[0] < back.near);
}

/// At rest the first-person weapon is the rig's alone; seen from outside
/// the body's own weapon bone has it whole, with the hands solved onto it
/// from the body's own shoulders.
#[test]
fn seen_from_outside_the_body_carries_the_weapon() {
    let Some((_, mut m, _)) = rig() else { return };
    run(&mut m, 0.5, Input::default());
    assert!(m.carried().is_none(), "the character's own eyes see the rig's weapon at rest");
    let own = [m.arm_pose(true), m.arm_pose(false)];
    assert!(own.iter().all(|arm| arm.shoulder == 0.0 && arm.hand == 0.0), "{own:?}");

    m.body.set_point_of_view(PointOfView::ThirdPerson);
    run(&mut m, 0.5, Input::default());
    let carried = m.carried().expect("seen from outside the body has the weapon");
    println!("standing, seen from outside: the weapon is carried at {:?}, turned {:.1} degrees, by {:.3}", carried.position, angle(carried.rotation), carried.authority);
    assert!((carried.authority - 1.0).abs() < 1e-3, "{}", carried.authority);
    for (side, arm) in [m.arm_pose(true), m.arm_pose(false)].into_iter().enumerate() {
        println!("  arm {side}: the hand the body's by {:.3}, the shoulder by {:.3}, the elbow by {:.3}", arm.hand, arm.shoulder, arm.elbow);
        assert_eq!(arm.shoulder, 1.0);
        assert!(arm.hand < 1e-3, "at rest the hands are on the weapon: {}", arm.hand);
    }
    assert!(m.held().is_none(), "the weapon is not in the body's hand at rest");

    // A walk, which the character's own eyes see with the rig's weapon.
    run(&mut m, 1.5, Input { move_dir: [0.0, 1.0], ..Input::default() });
    assert_eq!(m.state, State::Move);
    let carried = m.carried().expect("walking, seen from outside");
    println!("walking: carried by {:.3}", carried.authority);
    assert!(carried.authority > 0.9, "{}", carried.authority);

    // Back to its own eyes, the weapon is the rig's again at once.
    m.body.set_point_of_view(PointOfView::FirstPerson);
    run(&mut m, 1.0, Input::default());
    assert!(m.carried().is_none());
}

/// The three layers the animator's flag switches: the body breathes, and
/// leans into a walk and a sprint's turn, only for eyes other than its own.
#[test]
fn seen_from_outside_the_body_breathes() {
    let Some((_, mut m, _)) = rig() else { return };
    run(&mut m, 1.0, Input::default());
    println!("its own eyes:   {}", layer(&m, "Additive_FullBody"));
    assert!(layer(&m, "Additive_FullBody").contains("INACTIVE_FP"));
    let at_rest = m.body.arm_bone(true, 1);

    m.body.set_point_of_view(PointOfView::ThirdPerson);
    let mut travel = 0.0f32;
    let mut before = m.body.arm_bone(true, 1).0;
    for _ in 0..240 {
        m.step(DT, &Input::default(), &Flat);
        let now = m.body.arm_bone(true, 1).0;
        travel += length(std::array::from_fn(|k| now[k] - before[k]));
        before = now;
    }
    println!("from outside:   {}", layer(&m, "Additive_FullBody"));
    println!("the right upper arm travelled {:.1} mm in four seconds of standing", travel * 1000.0);
    assert!(layer(&m, "Additive_FullBody").contains("Breath"));
    assert!(travel > 0.002, "the breath moves the shoulders: {travel}");

    run(&mut m, 1.0, Input { move_dir: [0.0, 1.0], ..Input::default() });
    println!("walking:        {}", layer(&m, "Additive_BodyTurningOnMove"));
    assert!(layer(&m, "Additive_BodyTurningOnMove").contains("Move_Offset"));

    m.body.set_point_of_view(PointOfView::FirstPerson);
    run(&mut m, 2.0, Input::default());
    assert!(layer(&m, "Additive_FullBody").contains("INACTIVE_FP"));
    let back = m.body.arm_bone(true, 1);
    let gap = length(std::array::from_fn(|k| back.0[k] - at_rest.0[k]));
    println!("back to its own eyes the upper arm is {:.2} mm from where it rested", gap * 1000.0);
}

/// A weapon's draw and put-away mark the body's own part in them, which
/// plays on a layer that only eyes other than the character's own see.
#[test]
fn the_body_has_a_part_in_a_weapon_drawn_and_put_away() {
    let Some((_, mut m, mut firearm)) = rig() else { return };
    m.body.set_point_of_view(PointOfView::ThirdPerson);
    run(&mut m, 0.5, Input::default());
    println!("at rest:     {}", layer(&m, "Weapon"));
    assert!(layer(&m, "Weapon").contains("weight 1.00") && layer(&m, "Weapon").contains("INACTIVE"));

    // Put away: the weapon's own state for it has the mark.
    firearm.hide(false);
    let mut said = Vec::new();
    for _ in 0..90 {
        for event in firearm.step(DT) {
            if let FirearmEvent::BodyAction(action) = event {
                said.push(action);
                m.body.set_weapon_action(action);
            }
        }
        m.step(DT, &Input::default(), &Flat);
    }
    println!("put away:    the weapon said {said:?}; {}", layer(&m, "Weapon"));
    assert_eq!(said, vec![2]);
    assert!(layer(&m, "Weapon").contains("idle_to_out"));

    // Drawn again.
    firearm.draw();
    said.clear();
    let mut seen = Vec::new();
    for _ in 0..180 {
        for event in firearm.step(DT) {
            if let FirearmEvent::BodyAction(action) = event {
                said.push(action);
                m.body.set_weapon_action(action);
            }
        }
        m.step(DT, &Input::default(), &Flat);
        let now = layer(&m, "Weapon");
        if seen.last() != Some(&now.split(':').nth(1).unwrap_or("").split(" at ").next().unwrap_or("").to_owned()) {
            seen.push(now.split(':').nth(1).unwrap_or("").split(" at ").next().unwrap_or("").to_owned());
        }
    }
    println!("drawn:       the weapon said {said:?}; the layer went through {seen:?}");
    assert_eq!(said, vec![1]);
    assert!(seen.iter().any(|state| state.contains("out_to_idle")));
    assert!(layer(&m, "Weapon").contains("INACTIVE"), "{}", layer(&m, "Weapon"));

    // The character's own eyes are told too, and see none of it.
    let Some((_, mut own, _)) = rig() else { return };
    run(&mut own, 0.5, Input::default());
    let before = own.body.arm_bone(false, 5);
    own.body.set_weapon_action(2);
    run(&mut own, 0.4, Input::default());
    let after = own.body.arm_bone(false, 5);
    println!("its own eyes: {}", layer(&own, "Weapon"));
    assert!(layer(&own, "Weapon").contains("weight 0.00"));
    assert!(length(std::array::from_fn(|k| after.0[k] - before.0[k])) < 1e-6);
}

/// The reach for the vest is weighed by how near the left hand's marker
/// comes to the weapon's marker for the vest, and fades once no reload is
/// in hand.
#[test]
fn the_reach_for_the_vest_is_weighed_by_the_hand() {
    let Some((_, mut m, _)) = rig() else { return };
    m.body.set_point_of_view(PointOfView::ThirdPerson);
    assert_eq!(m.body.reach_for_vest(), 0.0);
    m.body.weigh_reach_for_vest(true, Some(0.6), DT);
    assert_eq!(m.body.reach_for_vest(), 0.0);
    m.body.weigh_reach_for_vest(true, Some(0.35), DT);
    assert!((m.body.reach_for_vest() - 0.5).abs() < 1e-5);
    m.body.weigh_reach_for_vest(true, Some(0.1), DT);
    assert_eq!(m.body.reach_for_vest(), 1.0);
    // Three twentieths of what is left go each sixtieth of a second.
    m.body.weigh_reach_for_vest(true, None, DT);
    assert!((m.body.reach_for_vest() - 0.95).abs() < 1e-5);
    m.body.weigh_reach_for_vest(false, Some(0.1), DT);
    assert_eq!(m.body.reach_for_vest(), 0.0);

    // With a reload in hand the layer has its state for it.
    run(&mut m, 0.5, Input { hands_busy: true, reloading: true, ..Input::default() });
    println!("{}", layer(&m, "UtilityOperation"));
    assert!(layer(&m, "UtilityOperation").contains("Idle_Reload_Utility"));
}

/// The weapon's motion for eyes other than the character's own: nothing
/// of the view's turning, the breath or the steps reaches it, and a shot
/// kicks it back without turning it.
#[test]
fn seen_from_outside_the_weapon_neither_sways_nor_breathes() {
    let Some((mut own, _, _)) = rig() else { return };
    let Some((mut other, _, _)) = rig() else { return };
    other.set_point_of_view(PointOfView::ThirdPerson);
    let (mut most_own, mut most_other) = ([0.0f32; 2], [0.0f32; 2]);
    for step in 0..240 {
        // The view swung to and fro, a quarter turn a second.
        let input = HandsInput { yaw: 30.0 * (step as f32 * DT * 3.0).sin(), ..still() };
        for (hands, most) in [(&mut own, &mut most_own), (&mut other, &mut most_other)] {
            hands.advance(DT, &input);
            hands.turn_rig(input.yaw, 0.0);
            let (position, rotation) = hands.weapon_transform();
            most[0] = most[0].max(length(position));
            most[1] = most[1].max(angle(rotation));
        }
    }
    println!("the view swung: its own eyes see the weapon move {:.1} mm and {:.2} degrees, another's {:.3} mm and {:.3} degrees", most_own[0] * 1000.0, most_own[1], most_other[0] * 1000.0, most_other[1]);
    assert!(most_own[0] > 0.001 && most_own[1] > 0.5);
    assert!(most_other[0] < 1e-6 && most_other[1] < 1e-4);

    // A shot: kicked back for both, turned for its own eyes only.
    let (mut back, mut turned) = ([0.0f32; 2], [0.0f32; 2]);
    for (index, hands) in [&mut own, &mut other].into_iter().enumerate() {
        hands.shoot(1.0);
        for _ in 0..30 {
            hands.advance(DT, &still());
            let (position, rotation) = hands.weapon_transform();
            back[index] = back[index].max(position[1]);
            turned[index] = turned[index].max(angle(rotation));
        }
    }
    println!("a shot: kicked back {:.1} and {:.1} mm, turned {:.2} and {:.3} degrees", back[0] * 1000.0, back[1] * 1000.0, turned[0], turned[1]);
    assert!(back[1] > 0.001, "{back:?}");
    assert!(turned[0] > 0.3 && turned[1] < 1e-3, "{turned:?}");
}

/// The sights coming up push a small pose of its own into the springs of
/// a character seen from outside, and nothing into its own eyes' weapon.
#[test]
fn the_sights_coming_up_move_the_weapon_seen_from_outside() {
    let Some((mut other, _, _)) = rig() else { return };
    other.set_point_of_view(PointOfView::ThirdPerson);
    for _ in 0..30 {
        other.advance(DT, &still());
    }
    assert_eq!(other.weapon_transform().0, [0.0; 3]);
    let aiming = HandsInput { aiming: true, ..still() };
    let (mut most, mut tipped) = (0.0f32, 0.0f32);
    for _ in 0..20 {
        other.advance(DT, &aiming);
        let (position, rotation) = other.weapon_transform();
        most = most.max(length(position));
        tipped = tipped.max(angle(rotation));
    }
    for _ in 0..120 {
        other.advance(DT, &aiming);
    }
    let held = other.weapon_transform();
    println!("the sights coming up: the weapon moves up to {:.2} mm and {:.2} degrees, and is held {:.2} mm off with them up", most * 1000.0, tipped, length(held.0) * 1000.0);
    assert!(most > 0.0 && tipped > 0.0);

    // A bot's mask has the turn-away alone: no pose, and no shot felt.
    let Some((mut bot, _, _)) = rig() else { return };
    bot.set_point_of_view(PointOfView::ThirdPerson);
    bot.mask = effector::DRAW_DOWN;
    bot.shoot(1.0);
    for _ in 0..60 {
        bot.advance(DT, &aiming);
    }
    let (position, rotation) = bot.weapon_transform();
    assert!(length(position) < 1e-7 && angle(rotation) < 1e-4, "{position:?}");
}
