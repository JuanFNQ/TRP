//! Tests of the weapon bone held toward its rest by the tactical reload's blend.
//! After the firing spec, "The reload with the sights up", and the mounting spec, "The weapon's
//! motion while mounted": the M700's figures read from
//! its template, nothing done at a blend of nought, and at a blend of one the turn about x and z cut
//! to 5%, about y to 67% and put back about the recoil pivot's depth, the place drawn in to 5%.
//! Passes trivially, with a note, where no install is configured.

use trp_data::setup::engine::EngineSettings;
use trp_data::setup::hard_settings::HardSettings;
use trp_data::character::player::HandsData;
use trp_data::unity::Assets;
use trp_sim::base::math::{quat_axis, quat_euler, quat_rotate, quat_to_euler};
use trp_sim::{Hands, ViewSettings};

const M700: &str = "5bfea6e90db834001b7347f3";

fn hands_of(template: &str) -> Option<Hands> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database()?;
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, db, template).expect("the weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    Some(Hands::new(trp_sim::hands_settings(data, db, &hard, &engine, &weapon, ViewSettings::default()).expect("its motion")))
}

#[test]
fn the_weapon_bone_is_held_toward_its_rest() {
    let Some(mut hands) = hands_of(M700) else { return };
    assert_eq!(hands.settings.tactical_stiffness, [0.95, 0.33, 0.95]);
    assert_eq!(hands.settings.tactical_fixation, 0.95);
    let clip = ([0.01, 0.02, -0.01], quat_euler([2.0, 6.0, 1.0]));
    // Nought: the clip's pose as it is.
    hands.set_tactical_reload(0.0);
    assert_eq!(hands.stiffen_weapon_bone(clip), clip);
    // One: x and z to 5%, y to 67% about the parent's up through the recoil pivot's depth.
    hands.set_tactical_reload(1.0);
    let (at, turn) = hands.stiffen_weapon_bone(clip);
    let angles = quat_to_euler(turn);
    for (got, wanted) in angles.iter().zip([0.1, 4.02, 0.05]) {
        assert!((got - wanted).abs() < 1e-3, "{angles:?}");
    }
    let depth = hands.settings.recoil_pivot[2];
    let drawn_in = [0.0005, 0.001, -0.0005];
    let swung = quat_rotate(quat_axis(1, 4.02), [drawn_in[0], drawn_in[1], drawn_in[2] - depth]);
    let wanted = [swung[0], swung[1], swung[2] + depth];
    for k in 0..3 {
        assert!((at[k] - wanted[k]).abs() < 1e-5, "{at:?} against {wanted:?}");
    }
}
