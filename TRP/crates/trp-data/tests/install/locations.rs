//! Tests that read the install's versions, blacklist and locations' floors and scenes.

use super::*;

/// The versions the install states for itself (`trp_data::versions`): the client's product and
/// SPT's server and client plugin, read from their programs' version resources, and the client
/// SPT's configuration names, all of the releases TRP was built for (0.16.9, SPT 4.1).
#[test]
fn the_install_states_its_versions() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let game = install.game_version().expect("the client's version");
    let spt = install.spt_version().expect("SPT's version");
    let client = install.spt_client_version().expect("SPT's client plugin's version");
    let made_for = install.spt_made_for().expect("the client SPT was made for");
    eprintln!("the client {game}, SPT {spt} (its client plugin {client}), made for the client {made_for}");
    assert!(trp_data::setup::versions::same_release(&game, "0.16.9"), "{game}");
    assert!(trp_data::setup::versions::same_release(&spt, "4.1"), "{spt}");
    assert!(trp_data::setup::versions::same_release(&client, "4.1"), "{client}");
    assert!(trp_data::setup::versions::same_release(&made_for, "0.16.9"), "{made_for}");
}

/// Factory's colliders as its scenes lay them (`level_colliders`): under the
/// player's start of `courses3`'s `1-f1-concrete` (its recorder's origin,
/// [13.33, 0.1075, -15.97]) the floor whose face the recorded ground check
/// met 0.0764 m below (`ground_hit`), at 0.0311; and the particles' longest
/// step, 0.03 s.
#[test]
fn factory_s_floor_lies_where_the_recorded_player_stood() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let at = [13.331_352, 0.107_545_7, -15.974_579];
    let level = trp_data::level::level_colliders::read(&install, trp_data::level::level_colliders::FACTORY_DAY, at, 1.0).expect("Factory's colliders");
    let mut heights = Vec::new();
    for c in level.colliders.iter().filter(|c| c.path.ends_with("Factory_area_03_inside_floor_COLLIDER")) {
        let trp_data::level::level_colliders::Shape::Mesh { mesh, .. } = &c.shape else { continue };
        for t in &mesh.triangles {
            let [a, b, d] = t.map(|i| mesh.positions[i as usize]);
            // Where the line down through the point crosses the triangle.
            let side = |p: [f32; 3], q: [f32; 3]| (q[0] - p[0]) * (at[2] - p[2]) - (q[2] - p[2]) * (at[0] - p[0]);
            let (s1, s2, s3) = (side(a, b), side(b, d), side(d, a));
            if !((s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0) || (s1 <= 0.0 && s2 <= 0.0 && s3 <= 0.0)) {
                continue;
            }
            let n = [(b[1] - a[1]) * (d[2] - a[2]) - (b[2] - a[2]) * (d[1] - a[1]), (b[2] - a[2]) * (d[0] - a[0]) - (b[0] - a[0]) * (d[2] - a[2]), (b[0] - a[0]) * (d[1] - a[1]) - (b[1] - a[1]) * (d[0] - a[0])];
            heights.push(a[1] - (n[0] * (at[0] - a[0]) + n[2] * (at[2] - a[2])) / n[1]);
        }
    }
    assert!(heights.iter().any(|h| (h - (at[1] - 0.076_408_68)).abs() < 1e-3), "the floor's heights there {heights:?}");
    let engine = trp_data::setup::engine::EngineSettings::load(&install).expect("the engine's settings");
    assert_eq!(engine.maximum_particle_timestep, 0.03);
}

/// A location's scenes are those of the preset its base in the database
/// names (`Scene`), its child presets' after it: Factory by day's fourteen,
/// less the AI's, the sound's and the culling's, are `FACTORY_DAY`;
/// Reserve's 49 (46 kept) and Customs' 29 (26 kept), as read.
#[test]
fn a_location_s_scenes_are_its_preset_s() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    use trp_data::level::level_colliders::{location_scenes, with_colliders, FACTORY_DAY};
    let factory = location_scenes(&install, "factory4_day").expect("Factory's scenes");
    assert_eq!(factory.len(), 14, "{factory:?}");
    assert!(factory[0].ends_with("Factory_Rework_Day_Scripts.unity"), "the day's own first: {factory:?}");
    let kept: Vec<&String> = factory.iter().filter(|s| with_colliders(s)).collect();
    assert_eq!(kept.len(), FACTORY_DAY.len(), "{kept:?}");
    for want in FACTORY_DAY {
        assert!(kept.iter().any(|s| s.ends_with(want)), "{want} not among {kept:?}");
    }
    assert!(factory.iter().any(|s| s.ends_with("Factory_Sound_Rework.unity")) && !with_colliders("Factory_Rework/Factory_Sound_Rework.unity"));
    let reserve = location_scenes(&install, "RezervBase").expect("Reserve's scenes");
    assert_eq!((reserve.len(), reserve.iter().filter(|s| with_colliders(s)).count()), (49, 46));
    let customs = location_scenes(&install, "bigmap").expect("Customs' scenes");
    assert_eq!((customs.len(), customs.iter().filter(|s| with_colliders(s)).count()), (29, 26));
}

/// Reserve's colliders lie where `courses3`'s `2-surfaces` found them: a
/// bullet's recorded entry into a red shipping container's wall (frame
/// 1859, `MetalThick_MedPen`, at (-3.21831, 0.52640, 5.48477) less the
/// recorder's origin (-29.84963, -6.98217, -155.32394), its normal
/// (-0.20778, 0, -0.97818)) lies on a triangle of that mesh as read, within
/// a millimetre, facing the same way within a hundredth of a degree.
#[test]
fn reserve_s_container_lies_where_the_recorded_bullet_entered_it() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let at = [-3.218_311_3 - 29.849_628, 0.526_404_86 - 6.982_165, 5.484_771_7 - 155.323_94];
    let normal = [-0.207_779_47f32, 0.0, -0.978_175_8];
    let level = trp_data::level::level_colliders::read(&install, &["Reserve_Base/Reserve_Base_outside_BLOCK_7.unity"], at, 0.1).expect("Reserve's colliders");
    let sub = |p: [f32; 3], q: [f32; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let cross = |u: [f32; 3], v: [f32; 3]| [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let dot = |u: [f32; 3], v: [f32; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let mut found = false;
    for c in level.colliders.iter().filter(|c| c.path.ends_with("container_6m_red_close (1)/MetalThick_MedPen")) {
        let trp_data::level::level_colliders::Shape::Mesh { mesh, .. } = &c.shape else { continue };
        for t in &mesh.triangles {
            let [a, b, d] = t.map(|i| mesh.positions[i as usize]);
            let n = cross(sub(b, a), sub(d, a));
            let length = dot(n, n).sqrt();
            if length < 1e-9 {
                continue;
            }
            let n = n.map(|x| x / length);
            let inside = [(a, b), (b, d), (d, a)].iter().all(|&(p, q)| dot(cross(sub(q, p), sub(at, p)), n) >= -1e-4);
            // The angle between by its sine: a hundredth of a degree.
            let apart = cross(n, normal);
            if inside && dot(sub(at, a), n).abs() < 1e-3 && dot(n, normal) > 0.0 && dot(apart, apart).sqrt() < 1.75e-4 {
                found = true;
            }
        }
    }
    assert!(found, "no face of the red container's wall at the bullet's entry ({} colliders read)", level.colliders.len());
}

/// SPT's list of what it never hands out (`item.json`'s `blacklist`, which
/// `HIDE UNFINISHED ITEMS` hides) names the unfinished items, which the live game's wiki says
/// cannot be had (the GK-01, the KAC QDC 6.5x35, the NXS 2.5 and the Sparrow 22), the
/// infected's arms (the knife without a picture) and the NSV's belt, and not
/// the run's own weapon.
#[test]
fn spt_s_item_blacklist_names_the_unfinished_items() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let list = trp_data::level::loot::item_blacklist(&install).expect("item.json's blacklist");
    let named = ["5580239d4bdc2de7118b4583", "55d617094bdc2d89028b4568", "544a3d0a4bdc2d1b388b4567", "54490a4d4bdc2dbc018b4573", "670ad7f1ad195290cd00da7a", "5cffa483d7ad1a049e54ef1c"];
    for id in named {
        assert!(list.iter().any(|on| on == id), "{id} is not on SPT's blacklist");
    }
    assert!(!list.iter().any(|on| on == "59d6088586f774275f37482f"), "the AKM is on it");
    eprintln!("SPT's item blacklist: {} things", list.len());
}
