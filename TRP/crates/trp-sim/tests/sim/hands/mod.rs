//! Weapon motion measured with the real install's data and the stock AKM.
//! These pass trivially, with a note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test sim hands:: -- --nocapture` for the numbers.

use trp_data::setup::engine::EngineSettings;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::character::player::HandsData;
use trp_data::unity::Assets;
use trp_sim::base::math::quat_euler;
use trp_sim::{Firearm, FirearmEvent, Flat, Hands, HandsInput, Input, Movement, State, ViewSettings};

mod sway;
mod shots;
mod obstacles;
mod breath;
mod disturbances;
mod bob;

use shots::{quat_mul_inverse};

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;

fn rig() -> Option<(Hands, Movement)> {
    full_rig().map(|(hands, movement, _)| (hands, movement))
}

/// Standing upright and still.
fn still() -> HandsInput {
    HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..HandsInput::default() }
}

/// The weapon's animated camera bone at rest.
fn camera_bone() -> [f32; 4] {
    quat_euler([-90.0, 0.0, 0.0])
}

fn full_rig() -> Option<(Hands, Movement, Firearm)> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let hands = Hands::new(
        trp_sim::hands_settings(data, &db, &hard, &engine, &weapon, ViewSettings::default()).expect("hands settings"),
    );
    let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("the weapon's animator");
    let mut firearm = Firearm::new(trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("firearm settings"), &animator);
    firearm.start_drawn();
    let body = trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator");
    let movement = Movement::new(
        trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement"),
        trp_sim::Body::new(&body, weapon.cell_width as f32),
    );
    Some((hands, movement, firearm))
}

/// Angle of a rotation `xyzw`, in degrees.
fn angle(q: [f32; 4]) -> f32 {
    2.0 * q[3].abs().min(1.0).acos().to_degrees()
}

fn length(v: [f32; 3]) -> f32 {
    v.iter().map(|c| c * c).sum::<f32>().sqrt()
}
