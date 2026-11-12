//! Tests of a character no one is at the keys of, with the stock AKM, from the install.
//! After the body hits spec, "The dummy on the range", through `trp_sim::puppet`:
//! standing and walking, its hits, its health, its load
//! as a bot's and its skills at level 1, as the app's dummies have them.
//! These pass trivially, with a note, where no install is configured.

use std::collections::HashMap;
use std::sync::Arc;

use trp_data::setup::engine::EngineSettings;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::character::player::{HandsData, LimbData};
use trp_data::unity::Assets;
use trp_data::Database;
use trp_sim::condition::health::{DamageKind, Health, HealthSettings, HealthSkills, Part};
use trp_sim::characters::puppet::{Posing, Puppet, PuppetPose};
use trp_sim::base::settings::WeightLimits;
use trp_sim::condition::skills::{Skill, SkillLevels};
use trp_sim::{Firearm, Flat, Hands, Input, Movement, Pacer, State, ViewSettings};

mod walking;
mod drawing;
mod economy;
mod colliders;

use walking::{empty_handed};
use economy::{Makings, SCENES, Scene, collider_bones, economy_health, made_for_economy, makings, pose_bits, simulation_of};

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;

/// A character made as the app makes a dummy's, carrying `weight` kg, its
/// encumbrance off as a bot's (`unencumbered`) or on as the player's, and
/// the install's figures it was made with.
fn made(weight: f32, unencumbered: bool) -> Option<(Puppet, HardSettings, &'static Database)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let hands = Hands::new(trp_sim::hands_settings(data, db, &hard, &engine, &weapon, ViewSettings::default()).expect("hands settings"));
    let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("the weapon's animator");
    let firearm = Firearm::new(trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("firearm settings"), &animator);
    let body = trp_sim::locomotion::body::BodyData::load(&mut assets).expect("body animator");
    let read = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement");
    let settings = read.relimited(&hard, &db.config, weight, WeightLimits { unencumbered, ..WeightLimits::BASE }).expect("the load's settings");
    let movement = Movement::new(settings, trp_sim::Body::new(&body, weapon.cell_width as f32));
    let limbs: [LimbData; 2] = LimbData::load(&mut assets).expect("the arms' solver");
    let pacer = Pacer::new(trp_sim::REFERENCE_FRAME_RATE, engine.fixed_timestep, engine.maximum_timestep);
    let puppet = Puppet::new(movement, hands, firearm, &weapon.hierarchy, limbs, weapon.cell_width as f32, pacer, true).expect("the puppet");
    Some((puppet, hard, db))
}

fn puppet() -> Option<Puppet> {
    made(0.0, true).map(|(p, ..)| p)
}

/// A PMC's health, as a dummy's.
fn health(db: &Database) -> Health {
    Health::new(Arc::new(HealthSettings::new(&db.config).expect("health settings")), 1)
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// How fast the character moves over the ground, in m/s.
fn ground_speed(p: &Puppet) -> f32 {
    let v = p.movement.velocity;
    (v[0] * v[0] + v[2] * v[2]).sqrt()
}
