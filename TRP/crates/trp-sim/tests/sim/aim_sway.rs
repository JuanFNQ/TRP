//! A heavy weapon's sway as its sights come up (weapon-handling spec, "Aim
//! sway"): its strength by weight less ergonomics, the direction drawn in
//! the prefab's box, the sway spring kicked by it. Passes trivially, with a
//! note, where no install is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::unity::Assets;
use trp_sim::{Hands, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";
const M60E6: &str = "661cec09b2c6356b4d0c7a36";
const DT: f32 = 1.0 / 120.0;

fn hands_of(template: &str) -> Option<Hands> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database()?;
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, db, template).expect("the weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    Some(Hands::new(trp_sim::hands_settings(data, db, &hard, &engine, &weapon, ViewSettings::default()).expect("its motion")))
}

#[test]
fn a_heavy_weapon_sways_as_its_sights_come_up() {
    let Some(mut m60) = hands_of(M60E6) else { return };
    // 12.3 kg at 14.55 ergonomics: past the prefab's 7 kg, whole.
    assert!((m60.aim_sway().0 - 1.0).abs() < 1e-6, "{:?}", m60.aim_sway());
    m60.aim_changed(true);
    let d = m60.aim_sway().1;
    // In the prefab's box (`AimSwayMin` -0.33, 1, -0.33; `AimSwayMax` -1, 6,
    // -2), standing (an intensity of 1).
    let within = |v: f32, a: f32, b: f32| v >= a.min(b) - 1e-5 && v <= a.max(b) + 1e-5;
    assert!(within(d[0], -0.33, -1.0) && within(d[1], 1.0, 6.0) && within(d[2], -0.33, -2.0), "{d:?}");
    // Each frame's kick moves the sway spring off its rest.
    let bone = trp_sim::math::quat_euler([-90.0, 0.0, 0.0]);
    m60.frame(DT, bone, None);
    m60.sway.step(DT);
    let moved = m60.sway.position;
    assert!(moved.iter().any(|c| c.abs() > 1e-6), "{moved:?}");
    // Down and up again: a new direction drawn.
    m60.aim_changed(false);
    m60.aim_changed(true);
    assert_ne!(m60.aim_sway().1, d);
    // The AKM, 3.9 kg at 36 ergonomics, has none.
    let Some(akm) = hands_of(AKM) else { return };
    assert_eq!(akm.aim_sway().0, 0.0);
}
