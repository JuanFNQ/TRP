//! A character no one is at the keys of, with the stock AKM, from the
//! install (`trp_sim::puppet`; body hits spec, "The dummy on the range").
//! These pass trivially, with a note, where no install is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::{HandsData, LimbData};
use trp_data::unity::Assets;
use trp_sim::puppet::Puppet;
use trp_sim::{Firearm, Flat, Hands, Movement, Pacer, State, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";

fn puppet() -> Option<Puppet> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let hands = Hands::new(trp_sim::hands_settings(data, &db, &hard, &engine, &weapon, ViewSettings::default()).expect("hands settings"));
    let animator = trp_sim::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("the weapon's animator");
    let firearm = Firearm::new(trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("firearm settings"), &animator);
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    let movement = Movement::new(trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement"), trp_sim::Body::new(&body, weapon.cell_width as f32));
    let limbs: [LimbData; 2] = LimbData::load(&mut assets).expect("the arms' solver");
    let pacer = Pacer::new(trp_sim::REFERENCE_FRAME_RATE, engine.fixed_timestep, engine.maximum_timestep);
    Some(Puppet::new(movement, hands, firearm, &weapon.hierarchy, limbs, weapon.cell_width as f32, pacer, true).expect("the puppet"))
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// Standing, the body holds the weapon: both palms where the solver puts
/// them, close by the weapon's markers, and the character stays put.
#[test]
fn a_puppet_stands_with_its_weapon_in_hand() {
    let Some(mut p) = puppet() else { return };
    for _ in 0..120 {
        p.step(1.0 / 60.0, [0.0, 0.0], &Flat);
    }
    let pose = p.pose().expect("a pose");
    println!("feet {:?}, ribcage {:?}, {} body bones, {} rig bones", pose.position, pose.ribcage, pose.bones.len(), pose.rig.len());
    assert_eq!(p.movement.state, State::Idle);
    assert!(distance(pose.position, [0.0; 3]) < 1e-3);
    assert!(pose.ribcage[1] > 1.0 && pose.ribcage[1] < 1.6, "the ribcage {:?}", pose.ribcage);
    for (side, arm) in pose.arms.iter().enumerate() {
        let palm = arm[4].0;
        // The palm within reach of the shoulder: the solver held on.
        let reach = distance(palm, arm[0].0);
        println!("arm {side}: upper at {:?}, palm at {:?}, {reach:.3} m apart", arm[0].0, palm);
        assert!(reach > 0.2 && reach < 0.7, "arm {side} reaches {reach}");
    }
}

/// With the walk's key held to its right it steps sideways, and back.
#[test]
fn a_puppet_walks_sideways() {
    let Some(mut p) = puppet() else { return };
    for _ in 0..90 {
        p.step(1.0 / 60.0, [1.0, 0.0], &Flat);
    }
    let right = p.pose().expect("a pose").position;
    println!("after 1.5 s to the right: {right:?}, state {:?}", p.movement.state);
    assert_eq!(p.movement.state, State::Move);
    assert!(right[0] > 0.5, "{right:?}");
    for _ in 0..180 {
        p.step(1.0 / 60.0, [-1.0, 0.0], &Flat);
    }
    let left = p.pose().expect("a pose").position;
    println!("then 3 s to the left: {left:?}");
    assert!(left[0] < right[0] - 1.0, "{left:?}");
}
