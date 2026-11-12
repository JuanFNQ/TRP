//! Movement measured with the real install's data. These pass trivially,
//! with a note, on a machine where no install is configured.
//!
//! Run with `cargo test -p trp-sim -- --nocapture` to see the numbers.

use trp_data::character::rootmotion::Node;
use trp_data::unity::Assets;
use trp_data::Database;
use trp_sim::{Flat, Input, Movement, State};

mod walking;
mod weight;
mod view;
mod prone;
mod legs_dry;
mod equipment;
mod falls;
mod vaults;
mod screens;
mod empty_hands;
mod weapon_animator;
mod containers;
mod m3;
mod hand_actions;
mod doors;
mod weapon_build;
mod controls;
mod own_weapons;
mod magazines;

use vaults::{until};

const DT: f32 = 1.0 / 60.0;

fn movement(weight: f32) -> Option<Movement> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::setup::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, weight).expect("movement settings");
    let body = trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator");
    // The stock AKM is five inventory cells wide.
    Some(Movement::new(settings, trp_sim::Body::new(&body, 5.0)))
}

fn run(m: &mut Movement, seconds: f32, input: Input) {
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, &input, &Flat);
    }
}

fn walk(dir: [f32; 2]) -> Input {
    Input { move_dir: dir, ..Input::default() }
}

/// What a weapon of the install is to the simulation: its build, its
/// animator and its logic's settings.
fn weapon_of(
    template: &str,
) -> Option<(trp_data::weapons::weapon::Weapon, trp_sim::weapons::weapon_animator::WeaponAnimatorData, trp_sim::FirearmSettings, &'static Database, Assets)> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, template).expect("the weapon");
    let data = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    Some((weapon, data, settings, db, assets))
}

/// Steps a weapon's logic at 120 frames a second and says which of its
/// animator's states were entered and what it said, each with its time.
fn run_weapon(w: &mut trp_sim::Firearm, seconds: f32) -> (Vec<(String, f32)>, Vec<(trp_sim::FirearmEvent, f32)>) {
    use trp_sim::{Firearm, FirearmEvent};
    const FRAME: f32 = 1.0 / 120.0;
    let name = |w: &Firearm| {
        let (state, _, next) = w.animator().state();
        next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
    };
    let (mut entered, mut said, mut last) = (Vec::new(), Vec::new(), name(w));
    for frame in 0..(seconds / FRAME).round() as usize {
        let at = (frame + 1) as f32 * FRAME;
        said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_))).map(|e| (e, at)));
        let now = name(w);
        if now != last {
            entered.push((now.clone(), at));
            last = now;
        }
    }
    (entered, said)
}

fn state_names(entered: &[(String, f32)]) -> Vec<&str> {
    entered.iter().map(|entry| entry.0.as_str()).collect()
}
