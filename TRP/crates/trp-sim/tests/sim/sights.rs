//! The sights as the game picks them (`trp_sim::sights`; optics spec, "The
//! sights a weapon has", "Zoom and modes", "Aiming through it"), on the
//! stock builds of the range that carry them: the SPEAR's Tango6T 1-6x,
//! the G36's carry handle (a 3x below, a backup above) with the red dot on
//! top, the M60E6's Specter DR 1x/4x over its folding rear sight. What the
//! recording `courses2/4-sights-optic` showed of the AKM with a Burris 1-4x
//! is `bin/sights`'. These pass trivially, with a note, where no install is
//! configured.

use trp_data::unity::Assets;
use trp_sim::sights::{Sights, OPTIC_FIELD_OF_VIEW};

const SPEAR: &str = "65290f395ae2ae97b80fdf2d";
const G36: &str = "623063e994fc3f7b302a9696";
const M60E6: &str = "661cec09b2c6356b4d0c7a36";
/// The install's field of view, and what the sights up over anything but
/// an optic leave of it.
const SETTING: f32 = 74.0;
const DT: f32 = 1.0 / 120.0;

fn sights_of(template: &str) -> Option<(Sights, trp_data::weapon::Weapon)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database()?;
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, db, template).expect("the weapon");
    Some((Sights::of_weapon(&weapon).expect("its sights"), weapon))
}

/// The current sight's bone, by its path's last two names.
fn bone(sights: &Sights, weapon: &trp_data::weapon::Weapon) -> String {
    let path = weapon.hierarchy.path(sights.current().bone);
    path.rsplit('/').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/")
}

#[test]
fn the_spear_aims_through_its_variable_scope() {
    let Some((mut sights, weapon)) = sights_of(SPEAR) else { return };
    assert_eq!(sights.list().len(), 1);
    assert_eq!(bone(&sights, &weapon), "scope_30mm_sig_tango6t_1_6x24/mod_aim_camera");
    assert!(sights.optic());
    assert_eq!(sights.aimed_field_of_view(SETTING), OPTIC_FIELD_OF_VIEW);
    // The item's range, not the prefab's 24 to 3.2: as made, the widest.
    assert_eq!(sights.zoom_value(), 24.5);
    assert_eq!(sights.optic_field_of_view(), Some(24.5));
    assert!((sights.aim_sensitivity(1.0) - 0.455).abs() < 1e-6);
    // One notch up with the sights up: the velocity is the item's zoom
    // sensitivity, 0.5, the wanted field of view 0.5 narrower, reached at
    // its speed of 5 times the velocity, degrees a second.
    sights.zoom_frame(1.0, DT, true);
    assert!((sights.zoom_value() - (24.5 - 5.0 * 0.5 * DT)).abs() < 1e-5, "{}", sights.zoom_value());
    for _ in 0..200 {
        sights.zoom_frame(0.0, DT, true);
    }
    assert!((sights.zoom_value() - 24.0).abs() < 1e-5, "{}", sights.zoom_value());
    // The magnification key snaps it to the far end; again, back.
    assert!(sights.change_magnification());
    assert!((sights.zoom_value() - 4.0833).abs() < 1e-4);
    // Its magnification with `PIP` off: the lowest zoom times the widest
    // field of view over the zoom's now, 6 at the narrowest.
    let m = sights.magnification().expect("an optic");
    assert!((m - 6.0).abs() < 1e-3, "{m}");
    assert!((sights.aim_sensitivity(1.0) - 0.0758).abs() < 1e-4, "the narrowest's sensitivity");
    assert!(sights.change_magnification());
    assert_eq!(sights.zoom_value(), 24.5);
    // The key to the next sight has nowhere to go.
    assert!(!sights.change_aim_scope(false));
    // Through an optic the eye goes by the far plane and the scope's own
    // distance (the Tango's `DistanceToCamera`, 0.1252 m).
    assert!((sights.eye().distance - 0.1252).abs() < 1e-3, "{:?}", sights.eye());
}

#[test]
fn the_g36_goes_through_its_carry_handle_s_two_sights_and_the_red_dot() {
    let Some((mut sights, weapon)) = sights_of(G36) else { return };
    // The carry handle's two bones, then the red dot's (by their items'
    // names); its iron sights are not used beside sights that look ahead.
    assert_eq!(sights.list().len(), 3);
    assert!(sights.straight());
    assert_eq!(bone(&sights, &weapon), "scope_g36_hensoldt_hkv_carry_handle_3x/mod_aim_camera_001");
    assert!(sights.optic());
    assert_eq!(sights.optic_field_of_view(), Some(8.6667));
    assert!((sights.aim_sensitivity(1.0) - 0.1517).abs() < 1e-6);
    // Its backup above: the carry handle's scope 1, an iron sight's field
    // of view and sensitivity.
    assert!(sights.change_aim_scope(false));
    assert_eq!(bone(&sights, &weapon), "scope_g36_hensoldt_hkv_carry_handle_3x/mod_aim_camera_002");
    assert_eq!(sights.scope_and_mode(), Some((1, 0)));
    assert!(!sights.optic());
    assert_eq!(sights.aimed_field_of_view(SETTING), SETTING - 15.0);
    assert!((sights.aim_sensitivity(1.0) - 0.65).abs() < 1e-6);
    // The red dot on top.
    assert!(sights.change_aim_scope(false));
    assert_eq!(bone(&sights, &weapon), "scope_base_hensoldt_hkv_red_dot_sight/mod_aim_camera_002");
    assert!(!sights.optic());
    // And round to the 3x, whose scope the key sets back to 0.
    assert!(sights.change_aim_scope(false));
    assert_eq!(sights.aim_index(), 0);
    assert_eq!(sights.scope_and_mode(), Some((0, 0)));
    assert!(sights.optic());
}

#[test]
fn the_m60e6_s_specter_switches_between_4x_and_1x() {
    let Some((mut sights, weapon)) = sights_of(M60E6) else { return };
    // The Specter's optic and its backup on top; the rear sight is left
    // out, and folds.
    assert_eq!(sights.list().len(), 2);
    assert_eq!(bone(&sights, &weapon), "scope_all_elcan_specter_dr_1-4/mod_aim_camera_001");
    assert!(sights.optic());
    assert_eq!(sights.optic_field_of_view(), Some(5.75));
    assert!((sights.aim_sensitivity(1.0) - 0.1138).abs() < 1e-6);
    for &(node, shown) in sights.folding() {
        let name = &weapon.hierarchy.nodes[node].name;
        assert_eq!(shown, name.contains("_folded_"), "{name}");
    }
    // The magnification key: its 1x, an optic still at 35 degrees.
    assert!(sights.change_magnification());
    assert_eq!(sights.scope_and_mode(), Some((0, 1)));
    assert!(sights.optic());
    assert_eq!(sights.optic_field_of_view(), Some(23.0));
    assert!((sights.aim_sensitivity(1.0) - 0.455).abs() < 1e-6);
    assert!(sights.change_magnification());
    assert_eq!(sights.optic_field_of_view(), Some(5.75));
    // The backup: the game counts the rear sight's scope first in its walk
    // of the items and so leaves the Specter on its scope 0, whose figure
    // the sensitivity keeps (kept, as the game has it).
    assert!(sights.change_aim_scope(false));
    assert_eq!(bone(&sights, &weapon), "scope_all_elcan_specter_dr_1-4/mod_aim_camera_002");
    assert!(!sights.optic());
    assert_eq!(sights.aimed_field_of_view(SETTING), SETTING - 15.0);
    assert_eq!(sights.scope_and_mode(), Some((0, 0)));
}

/// What the SPEAR's Tango6T gives step 2 (optics spec, "The lens"): its
/// lens's figures and rings (as the shader reading found them), its back
/// lens and its glass (its body's second material), its reticle mesh and
/// its picture's aberration; the reticle's blend whole at the widest and
/// none at the narrowest.
#[test]
fn the_tango_s_lens_and_reticle_are_read() {
    use trp_data::sight::LensKind;
    let Some((mut sights, _)) = sights_of(SPEAR) else { return };
    let tango = sights.parts()[0].clone();
    let optic = tango.lenses.iter().find_map(|piece| match &piece.kind {
        LensKind::Optic(lens) => Some(*lens),
        _ => None,
    });
    let lens = optic.expect("its lens");
    assert_eq!((lens.scales, lens.shifts, lens.normal_hideness), ([40.0, 0.94], [-0.15, 0.0469], 5.0));
    assert!((lens.inner_radius - 0.01332).abs() < 1e-5 && (lens.outer_radius - 0.02059).abs() < 1e-5, "{lens:?}");
    assert!(tango.lenses.iter().any(|piece| piece.kind == LensKind::Back));
    assert!(tango.lenses.iter().any(|piece| piece.kind == LensKind::Glass && piece.slot == 1));
    assert!(tango.lenses.iter().any(|piece| piece.pictures.iter().any(|(name, _)| name == "_MaskTex2")));
    let mode = tango.modes[0].optic.as_ref().expect("its optic");
    assert!((mode.aberration - 0.00325).abs() < 1e-6);
    let reticle = mode.reticle.as_ref().expect("its reticle mesh");
    assert_eq!((reticle.positions.len(), reticle.triangles.len() / 3), (3028, 2756));
    assert_eq!((reticle.position, reticle.rotation, reticle.scale), ([0.0, 0.0, 0.5], [0.0, 180.0, 0.0], 0.013));
    assert!(reticle.shape.is_some());
    assert!(sights.live_optic().is_some());
    assert!((sights.reticle_weight() - 1.0).abs() < 1e-6, "at the widest");
    assert!(sights.change_magnification());
    assert!(sights.reticle_weight().abs() < 1e-6, "at the narrowest");
}

/// The wheel's input to the zoom with the install's bindings (`LeftAlt`
/// and the wheel, `ScopeZoomIn` and `ScopeZoomOut`): either way with the
/// key held, nothing without it, turned about by the inverted switch.
#[test]
fn the_wheel_zooms_with_left_alt_held() {
    let Some(install) = crate::fixture::install() else { return };
    let controls = trp_sim::bindings::install_controls(&install);
    let alt = trp_sim::bindings::code("LeftAlt").expect("LeftAlt");
    let held = |code: trp_sim::bindings::Code| code == alt;
    let none = |_: trp_sim::bindings::Code| false;
    let input = trp_sim::sights::zoom_input;
    assert_eq!(input(&controls, &held, 1.0, 1.0, false), 1.0);
    assert_eq!(input(&controls, &held, -1.0, 2.0, false), -2.0);
    assert_eq!(input(&controls, &none, 1.0, 1.0, false), 0.0);
    assert_eq!(input(&controls, &held, 0.0, 1.0, false), 0.0);
    assert_eq!(input(&controls, &held, 1.0, 1.0, true), -1.0);
}

/// The user's M4A1 of `courses4` (recorder 0.11.0, its equipment), the
/// parts by their templates: a 3x on the receiver and a Belomo PK-06 on a
/// 45 degree mount on the handguard's side rail.
fn canted_m4a1() -> trp_data::database::Preset {
    let part = |id: &str, template: &str, parent: &str, slot: &str| trp_data::database::PresetItem {
        id: id.into(),
        template: template.into(),
        parent: Some(parent.into()),
        slot: Some(slot.into()),
    };
    let items = vec![
        trp_data::database::PresetItem { id: "m4a1".into(), template: "5447a9cd4bdc2dbd208b4567".into(), parent: None, slot: None },
        part("grip", "55d4b9964bdc2d1d4e8b456e", "m4a1", "mod_pistol_grip"),
        part("magazine", "5c6592372e221600133e47d7", "m4a1", "mod_magazine"),
        part("receiver", "55d355e64bdc2d962f8b4569", "m4a1", "mod_reciever"),
        part("scope", "626bb8532c923541184624b4", "receiver", "mod_scope"),
        part("barrel", "55d35ee94bdc2d61338b4568", "receiver", "mod_barrel"),
        part("muzzle", "56ea8180d2720bf2698b456a", "barrel", "mod_muzzle"),
        part("gas", "56eabcd4d2720b66698b4574", "barrel", "mod_gas_block"),
        part("handguard", "5d123102d7ad1a004e475fe5", "receiver", "mod_handguard"),
        part("mount1", "5d123a3cd7ad1a004e476058", "handguard", "mod_mount_001"),
        part("front", "5bc09a30d4351e00367fb7c8", "handguard", "mod_sight_front"),
        part("canted", "5649a2464bdc2d91118b45a8", "handguard", "mod_tactical_000"),
        part("pk06", "57ae0171245977343c27bfcf", "canted", "mod_scope"),
        part("rear", "5bc09a18d4351e003562b68e", "receiver", "mod_sight_rear"),
        part("buffer", "638de3603a1a4031d8260b8c", "m4a1", "mod_stock"),
        part("stock", "56eabf3bd2720b75698b4569", "buffer", "mod_stock_000"),
        part("pad", "58d2912286f7744e27117493", "stock", "mod_stock"),
        part("charge", "55d44fd14bdc2d962f8b456e", "m4a1", "mod_charge"),
    ];
    trp_data::database::Preset { id: "canted".into(), name: "canted".into(), root: "m4a1".into(), items, encyclopedia: None }
}

/// A canted sight turns the weapon about its barrel while aimed through
/// (`UpdateCurrentScopeRotation`, `ApplyComplexRotation`): the PK-06 on its
/// mount stands 45 degrees from straight ahead, and the weapon goes to it
/// by a share `CameraSmoothTime * aiming speed * dt` of what is left each
/// frame, and back. Against the recording frame for frame: `courses4`'s
/// three aimed scripts within 0.007 degrees (`docs/status.md`).
#[test]
fn a_canted_sight_turns_the_weapon_about_its_barrel() {
    use trp_data::{engine::EngineSettings, hard_settings::HardSettings, player::HandsData};
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble_preset(&mut assets, db, &canted_m4a1()).expect("the weapon");
    assert!(weapon.skipped.is_empty(), "{:?}", weapon.skipped);
    let data = HandsData::load(&mut assets).expect("player prefab");
    let mut hands = trp_sim::Hands::new(trp_sim::hands_settings(data, db, &hard, &engine, &weapon, trp_sim::ViewSettings::default()).expect("its motion"));
    let sights = hands.sights().expect("its sights");
    // As recorded: two ways to aim, the first the PK-06, turned.
    assert_eq!(sights.list().len(), 2);
    let aimed_by = bone(sights, &weapon);
    assert!(aimed_by.contains("pk_06"), "{aimed_by}");
    // Its window is a red dot's glass (`EFT/CollimatorGlass`), which the app
    // leaves undrawn as a scope's front glass, besides its three modes'
    // collimators.
    let pk06 = sights.parts().iter().find(|part| part.template == "57ae0171245977343c27bfcf").expect("the PK-06");
    let kinds: Vec<&trp_data::sight::LensKind> = pk06.lenses.iter().map(|piece| &piece.kind).collect();
    assert!(kinds.iter().any(|kind| **kind == trp_data::sight::LensKind::Glass), "{kinds:?}");
    assert_eq!(kinds.iter().filter(|kind| matches!(***kind, trp_data::sight::LensKind::Collimator(_))).count(), 3, "{kinds:?}");
    let turn = sights.scope_rotation();
    assert!((turn + 45.0).abs() < 0.1, "{turn}");
    let twist = |hands: &trp_sim::Hands| {
        let q = hands.weapon_transform().1;
        2.0 * q[1].atan2(q[3]).to_degrees()
    };
    // The first frame aimed takes the share of the frame, the quaternions
    // blended and renormalised (`Quaternion.Lerp`).
    hands.aim_changed(true);
    hands.blend_poses(DT);
    let share = 8.0 * hands.aiming_speed * DT;
    let half = (turn / 2.0).to_radians();
    let expected = 2.0 * (share * half.sin()).atan2(1.0 - share + share * half.cos()).to_degrees();
    assert!((twist(&hands) - expected).abs() < 1e-3, "{} against {expected}", twist(&hands));
    for _ in 0..240 {
        hands.blend_poses(DT);
    }
    assert!((twist(&hands) - turn).abs() < 0.01, "aimed: {}", twist(&hands));
    // Down again, back to straight.
    hands.aim_changed(false);
    for _ in 0..240 {
        hands.blend_poses(DT);
    }
    assert!(twist(&hands).abs() < 0.01, "lowered: {}", twist(&hands));
}
