//! Speed limits from armour and from being hit (`docs/spec/movement.md`,
//! "Speed limits"), with the install's movement and database. These pass
//! trivially, with a note, where no install is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::armor::{speed_penalty, VestSkills, Worn};
use trp_sim::health::{DamageKind, Part};
use trp_sim::{Flat, Input, Movement, State};

const DT: f32 = 1.0 / 120.0;

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

fn run(m: &mut Movement, seconds: f32, input: &Input) {
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, input, &Flat);
    }
}

#[test]
fn plates_slow_by_their_penalties_less_the_vest_skills() {
    let Some(db) = crate::fixture::database() else { return };
    // `courses3\3-bot-armour`'s four plates, each counted where it sits in
    // an armour slot: two light (-1.5 and -1.0 %), two heavy (-0.5, -0.75).
    let plate = |template: &str| Worn { template: template.to_owned(), slot: "ArmorVest".to_owned(), children: Vec::new() };
    let kit = ["656fafe3498d1b7e3e071da4", "656fb21fa0dce000a2020f7c", "6557458f83942d705f0c4962", "64afd81707e2cf40e903a316"].map(plate);
    assert!((speed_penalty(db, &kit, &VestSkills::default()) + 3.75).abs() < 1e-5);
    // Light Vests at 40 take 24 % off the light ones, Heavy Vests at 40 20 %
    // off the heavy: -2.9 %, the game's limit for that kit (0.66643 of
    // 0.68633) in the recording.
    let skills = VestSkills::of_levels(|name| if name.ends_with("Vests") { 40 } else { 0 });
    let penalty = speed_penalty(db, &kit, &skills);
    assert!((penalty + 2.9).abs() < 1e-5, "{penalty}");
    assert!(((100.0 + penalty) / 100.0 * 0.686_333_36 - 0.666_430).abs() < 1e-5);
}

#[test]
fn armour_holds_the_speed_to_its_share_of_the_top() {
    let Some(mut m) = movement() else { return };
    let top = m.settings.max_speed;
    m.set_armour_penalty(-5.0);
    run(&mut m, 0.1, &Input::default());
    assert!((m.speed_limit() - 0.95 * top).abs() < 1e-6, "{} against {}", m.speed_limit(), 0.95 * top);
    m.set_armour_penalty(0.0);
    run(&mut m, 0.1, &Input::default());
    assert!((m.speed_limit() - top).abs() < 1e-6);
}

#[test]
fn a_hit_by_a_weapon_caps_the_speed_for_a_while() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.2, &Input::default());
    let top = m.speed_limit();
    // 32.5 of stamina burnt: halfway from 10 to 55, half the speed.
    m.hit(32.5, Part::Chest, DamageKind::Bullet);
    run(&mut m, 2.0 * DT, &Input::default());
    assert!((m.speed_limit() - 0.5).abs() < 1e-6, "{}", m.speed_limit());
    // Two hits in one frame burn together: past 55 the character stands.
    m.hit(30.0, Part::Chest, DamageKind::Bullet);
    m.hit(30.0, Part::Stomach, DamageKind::Bullet);
    run(&mut m, 2.0 * DT, &Input::default());
    assert_eq!(m.speed_limit(), 0.0);
    // Gone 0.66 s after the last.
    run(&mut m, 0.7, &Input::default());
    assert!((m.speed_limit() - top).abs() < 1e-6, "{} against {top}", m.speed_limit());
    // A grenade's fragment is no weapon's: nothing.
    m.hit(50.0, Part::Chest, DamageKind::GrenadeFragment);
    run(&mut m, 2.0 * DT, &Input::default());
    assert!((m.speed_limit() - top).abs() < 1e-6);
}

#[test]
fn a_hit_s_burn_comes_off_the_legs() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.2, &Input::default());
    let legs = m.stamina.legs.current;
    m.hit(30.0, Part::Chest, DamageKind::Bullet);
    assert!((legs - m.stamina.legs.current - 30.0).abs() < 1e-3, "{} from {legs}", m.stamina.legs.current);
    // A fragment burns nothing (it is no weapon's).
    let legs = m.stamina.legs.current;
    m.hit(30.0, Part::Chest, DamageKind::GrenadeFragment);
    assert_eq!(m.stamina.legs.current, legs);
}

#[test]
fn a_leg_hit_ends_a_sprint() {
    let Some(mut m) = movement() else { return };
    run(&mut m, 0.5, &Input { move_dir: [0.0, 1.0], ..Input::default() });
    let sprint = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    run(&mut m, 1.5, &sprint);
    assert_eq!(m.state, State::Sprint);
    m.hit(0.0, Part::LeftLeg, DamageKind::Bullet);
    assert!(!m.sprinting);
}
