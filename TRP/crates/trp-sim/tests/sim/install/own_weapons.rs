//! Weapons of one's own built and worked on a donor's rig and animator.

use super::*;

/// The Sturmgewehr 44 of `custom/stg44.json`: a weapon that is not the
/// install's, whose model lies in `local/` (`docs/spec/custom-weapons.md`).
const STG44: &str = "66f0a5740000000000000044";

/// Its donor, the FN SCAR-L, and the AKM it takes its round and recoil from.
const SCAR_L: &str = "6184055050224f204c1da540";

const AKM: &str = "59d6088586f774275f37482f";

/// A weapon of one's own is an item of the database like any other, and
/// is assembled by the game's rules on its donor's rig: what it weighs,
/// what it holds and how it kicks come from its template, its muzzle and
/// its sights from bones as a weapon's parts would bring them, and what
/// is drawn is the model's own parts, each the right way out. Passes
/// with a note where the model is not there (it is never in the
/// repository).
#[test]
fn a_weapon_of_ones_own_is_built_on_its_donors_rig() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    if !db.custom.contains_key(STG44) {
        eprintln!("skipped: the StG 44 is not built here ({:?})", db.custom_notes);
        return;
    }
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let engine = trp_data::setup::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let data = trp_data::character::player::HandsData::load(&mut assets).expect("weapon motion data");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, STG44).expect("the StG 44");
    let akm = trp_data::weapons::weapon::assemble(&mut assets, &db, AKM).expect("the AKM");
    let custom = weapon.custom.as_ref().expect("its own parts");

    // The template: 4.6 kg with its empty magazine, and 31 rounds of the
    // AKM's; five cells; a fixed stock; no catch for the bolt.
    let template = db.item(STG44).expect("its template");
    let round = db.item(db.item(AKM).unwrap().props["defAmmo"].as_str().unwrap()).unwrap().prop_f32("Weight").unwrap();
    assert!((weapon.total_weight - (4.6 + 31.0 * round)).abs() < 1e-3, "{}", weapon.total_weight);
    assert_eq!((weapon.magazine_capacity, weapon.cell_width, weapon.held_closer, weapon.fold), (Some(30), 5, false, None));
    assert_eq!(template.props["ammoCaliber"], db.item(AKM).unwrap().props["ammoCaliber"]);
    let logic = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its logic's settings");
    assert_eq!((logic.bolt_catch, logic.foldable, logic.fire_rate, logic.magazine_capacity), (false, false, 540.0, 30));
    assert_eq!(logic.modes, [trp_sim::FireMode::Single, trp_sim::FireMode::FullAuto]);
    // It kicks and handles as the AKM's stock build does: that build's
    // own figures, its parts' shares in them (below).
    assert!((weapon.total_ergonomics - akm.total_ergonomics).abs() < 1e-4, "{} {}", weapon.total_ergonomics, akm.total_ergonomics);
    let kick = |weapon: &trp_data::weapons::weapon::Weapon, template: &str| db.item(template).unwrap().prop_f32("RecoilForceUp").unwrap() * (1.0 + weapon.recoil_delta);
    assert!((kick(&weapon, STG44) - kick(&akm, AKM)).abs() < 1e-3);
    let view = trp_sim::ViewSettings::default();
    let (ours, theirs) = (
        trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &weapon, view).expect("its motion"),
        trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &akm, view).expect("the AKM's motion"),
    );
    for (mine, its) in [(ours.recoil.rotation_strength, theirs.recoil.rotation_strength), (ours.recoil.position_strength, theirs.recoil.position_strength)] {
        assert!((mine[0] - its[0]).abs() < 1e-4 && (mine[1] - its[1]).abs() < 1e-4, "{mine:?} {its:?}");
    }
    println!(
        "the StG 44: {:.2} kg loaded, ergonomics {}, {} rounds a minute, the line that feels for obstacles {:.3} m (the AKM's {:.3})",
        weapon.total_weight, weapon.total_ergonomics, logic.fire_rate, ours.weapon_length, theirs.weapon_length
    );

    // The muzzle and the sights are bones of its own, as a weapon's parts
    // bring them: the model's muzzle 88.5 cm ahead of the weapon's bone,
    // and a line through the notch and the post that is all but level.
    assert!((ours.weapon_length - 0.885).abs() < 0.005, "{}", ours.weapon_length);
    let sights = trp_sim::base::rig::SightNodes::find(&weapon.hierarchy).expect("its sights");
    let rig = trp_sim::base::rig::WeaponRig::new(&weapon.hierarchy);
    let line = sights.line(&rig).expect("the line through its two sights");
    assert_eq!(weapon.hierarchy.nodes[sights.bone].name, "mod_aim_camera");
    assert!(line.angle.abs() < 0.2 && line.shift[1].abs() < 0.002, "{line:?}");

    // Every part is drawn the right way out: a triangle's face, by the
    // order of its corners, looks the way its corners' normals do. (The
    // model's axes are mirrored into the rig's, which turns every
    // triangle over unless two corners change places.)
    assert_eq!(custom.parts.len(), 12);
    for part in &custom.parts {
        let mesh = &part.mesh;
        let (mut agree, mut count) = (0usize, 0usize);
        for triangle in mesh.submeshes.iter().flat_map(|indices| indices.chunks_exact(3)) {
            let [a, b, c] = [0, 1, 2].map(|k| mesh.positions[triangle[k] as usize]);
            let (u, v): ([f32; 3], [f32; 3]) = (std::array::from_fn(|k| b[k] - a[k]), std::array::from_fn(|k| c[k] - a[k]));
            let face = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let normal: [f32; 3] = std::array::from_fn(|k| triangle.iter().map(|&i| mesh.normals[i as usize][k]).sum());
            let along: f32 = (0..3).map(|k| face[k] * normal[k]).sum();
            if face.iter().map(|f| f * f).sum::<f32>() > 1e-16 {
                count += 1;
                agree += (along > 0.0) as usize;
            }
        }
        assert!(agree as f32 > 0.98 * count as f32, "{}: {agree} of {count} triangles face their normals' way", mesh.name);
        assert_eq!(mesh.tangents.len(), mesh.positions.len());
    }

    // Its muzzle is the SKS's (its file's `muzzle_from`): that barrel's
    // one jet and its smoke, below its own fireport where they are from
    // the SKS's, half a centimetre ahead along the barrel (-y).
    let h = &weapon.hierarchy;
    let muzzle = weapon.muzzle.as_ref().expect("its muzzle effects");
    assert_eq!((muzzle.jets.len(), muzzle.smokes.len()), (1, 1));
    let jet = &h.nodes[muzzle.jets[0].node];
    assert_eq!(jet.parent.map(|parent| h.nodes[parent].name.as_str()), Some("fireport"));
    assert!(jet.translation[0].abs() < 1e-3 && (jet.translation[1] + 0.005).abs() < 1e-3 && jet.translation[2].abs() < 1e-3, "{:?}", jet.translation);
    assert_eq!(muzzle.smokes[0].node, muzzle.jets[0].node);

    // Its magazine's object is its donor's magazine's prefab, the first
    // under the slot as the game hangs it (what a reload shows and hides,
    // and what is let go): below it the points its rounds are drawn at and
    // the model's magazine, nothing of the donor's drawn and no hand going
    // by it. Every object comes after its parent.
    let slot = h.find_below(0, "mod_magazine").expect("its magazine's slot");
    let magazine = (0..h.nodes.len()).find(|&i| h.nodes[i].parent == Some(slot)).expect("a magazine");
    assert!(h.nodes[magazine].name.starts_with("mag_"), "{}", h.nodes[magazine].name);
    assert!(h.nodes.iter().enumerate().all(|(i, node)| node.parent.is_none_or(|parent| parent < i)));
    let under = |mut node: usize| loop {
        match h.nodes[node].parent {
            Some(parent) if parent == magazine => return true,
            Some(parent) => node = parent,
            None => return false,
        }
    };
    let points: Vec<usize> = (0..h.nodes.len()).filter(|&i| h.nodes[i].name.starts_with("patron_") && h.nodes[i].name != "patron_in_weapon").collect();
    assert_eq!(points.len(), 5);
    assert!(points.iter().all(|&point| under(point)));
    let part = custom.parts.iter().find(|part| part.mesh.name == "Magazine").expect("the model's magazine");
    assert_eq!(h.nodes[part.node].parent, Some(magazine));
    let hung = (0..h.nodes.len()).filter(|&i| under(i)).collect::<Vec<_>>();
    assert!(hung.iter().all(|&i| h.nodes[i].renderer.is_none() && h.nodes[i].grip.is_none()));

    // Let go or in its picture, the magazine is that object with what is
    // below it: the model's magazine drawn, its five points.
    let (tree, own) = trp_data::weapons::custom::magazine_model(&weapon).expect("its magazine as a thing");
    assert_eq!(tree.nodes[0].name, h.nodes[magazine].name);
    assert_eq!(tree.nodes.len(), hung.len() + 1);
    assert_eq!((own.parts.len(), own.materials.len()), (1, 1));
    assert_eq!(own.materials[0].name, "Magazine");
    assert_eq!(tree.nodes[own.parts[0].node].name, "Magazine");
    assert_eq!(tree.nodes.iter().filter(|node| node.name.starts_with("patron_")).count(), 5);
}

/// The StG 44's ZF4 Steel Mount and ZF4 (`custom/parts/zf4.json`), parts
/// of one's own: the StG's `mod_mount` takes the mount alone (the B-18 it
/// once took, no more), the mount's `mod_scope` the ZF4 alone. On the
/// rifle each is its donor's prefab hung as the game hangs a part, nothing
/// of the donors drawn but the PU's lens and its back, fitted to the
/// ZF4's eyepiece; the ZF4 is a sight with the PU's optic and the
/// magnification the ZF4 has; their model's pieces are drawn the right way
/// out. Alone, each is its own model too. Passes with a note where the
/// model is not there (it is never in the repository).
#[test]
fn the_stg_44_s_zf4_is_a_scope_of_one_s_own_on_a_mount_of_one_s_own() {
    const MAGAZINE: &str = "66f0a5740000000000000045";
    const MOUNT: &str = "66f0a5740000000000000046";
    const ZF4: &str = "66f0a5740000000000000047";
    const PU: &str = "5b3f7c1c5acfc40dc5296b1d";
    const B18: &str = "57ffb0062459777a045af529";
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    if !db.custom.contains_key(STG44) || !db.custom_parts.contains_key(ZF4) || !db.custom_parts.contains_key(MOUNT) {
        eprintln!("skipped: the StG 44's ZF4 is not built here ({:?})", db.custom_notes);
        return;
    }
    let mut assets = Assets::new(install).expect("manifest");

    // The templates: the slots, the classes and figures of the donors, the
    // ZF4's magnification.
    let takes = |template: &str, slot: &str| -> Vec<String> {
        let item = db.item(template).unwrap();
        let slot = item.props["Slots"].as_array().unwrap().iter().find(|s| s["_name"] == slot).unwrap_or_else(|| panic!("{} has no {slot}", item.name));
        slot["_props"]["filters"][0]["Filter"].as_array().unwrap().iter().map(|t| t.as_str().unwrap().to_owned()).collect()
    };
    assert_eq!(takes(STG44, "mod_mount"), [MOUNT]);
    assert!(!takes(STG44, "mod_mount").iter().any(|t| t == B18));
    assert_eq!(takes(MOUNT, "mod_scope"), [ZF4]);
    let (mount, zf4, pu) = (db.item(MOUNT).unwrap(), db.item(ZF4).unwrap(), db.item(PU).unwrap());
    assert!(db.is_a(mount, "Mount") && db.is_a(zf4, "OpticScope"));
    assert_eq!(zf4.props["Zooms"], serde_json::json!([[4]]));
    assert_eq!((zf4.prop_f32("Weight"), zf4.props["AimSensitivity"].clone()), (pu.prop_f32("Weight"), pu.props["AimSensitivity"].clone()));

    let key = format!("{STG44}[mod_magazine={MAGAZINE}[],mod_mount={MOUNT}[mod_scope={ZF4}[]]]");
    let weapon = trp_data::weapons::weapon::assemble_key(&mut assets, &db, &key).expect("the StG 44 with its ZF4");
    assert!(weapon.skipped.is_empty(), "{:?}", weapon.skipped);
    let h = &weapon.hierarchy;
    let custom = weapon.custom.as_ref().expect("its own parts");
    // The rifle's twelve, the mount's two (its share of the scope's mesh,
    // the rail of the receiver's) and the ZF4's one.
    assert_eq!(custom.parts.len(), 15);
    assert!(h.nodes.iter().enumerate().all(|(i, node)| node.parent.is_none_or(|parent| parent < i)));

    // The ZF4 is a sight with the PU's optic: one mode, its field of view,
    // its eye's bone below the ZF4's root, its lens and its back (the
    // donor's front glass, a submesh of its body, not drawn).
    let sight = weapon.sights.iter().find(|part| part.template == ZF4).expect("the ZF4 as a sight");
    assert_eq!(sight.scopes[0].zooms, [4.0]);
    let optic = sight.modes[0].optic.as_ref().expect("its optic");
    assert!((optic.field_of_view.unwrap() - 4.7143).abs() < 1e-3, "{:?}", optic.field_of_view);
    assert_eq!(h.nodes[optic.bone.unwrap()].name, "mod_aim_camera");
    let kinds: Vec<&trp_data::weapons::sight::LensKind> = sight.lenses.iter().map(|lens| &lens.kind).collect();
    assert!(matches!(kinds.as_slice(), [trp_data::weapons::sight::LensKind::Optic(_), trp_data::weapons::sight::LensKind::Back]), "{kinds:?}");
    // Its reticle is its own (`custom::MarkDef`), not the PU's.
    let mark = sight.lenses[0].pictures.iter().find(|(name, _)| name == "_MarkTex").expect("its reticle");
    assert_eq!((mark.1.width, mark.1.height), (1024, 1024));
    // Below the two parts' roots, nothing drawn but the lens and its back,
    // each 1.071 times its size (1.19 at the model's size, the parts at 0.9).
    let below = |node: usize, root: usize| {
        let mut at = Some(node);
        while let Some(n) = at {
            if n == root {
                return true;
            }
            at = h.nodes[n].parent;
        }
        false
    };
    let mount_root = h.nodes.iter().position(|node| node.name.starts_with("mount_svt40")).expect("the mount's root");
    let drawn: Vec<&str> = (0..h.nodes.len()).filter(|&i| below(i, mount_root) && h.nodes[i].renderer.is_some()).map(|i| h.nodes[i].name.as_str()).collect();
    assert_eq!(drawn, ["linza", "backLens"]);
    let lens = sight.lenses[0].node;
    assert!(h.nodes[lens].scale.iter().all(|s| (s - 1.071).abs() < 1e-5), "{:?}", h.nodes[lens].scale);

    // The lens is at the bottom of the ZF4's eyepiece: on the weapon's bone
    // (metres right, back and up), the second model's (0, 9.53, 20.40)
    // drawn at 0.9 of its size about the mount's point (1.40, 3.24,
    // 11.745), so (0.14, 8.901, 19.5345), as that model lies on the first
    // (14.145 cm higher), the first's origin 42.05 cm behind the bone and
    // 13.0 above it.
    let weapon_bone = (0..h.nodes.len()).find(|&i| h.path(i) == trp_data::weapons::custom::WEAPON_BONE).unwrap();
    let mut place = [0.0f32; 3];
    let mut at = lens;
    while at != weapon_bone {
        let node = &h.nodes[at];
        let p = trp_sim::base::math::quat_rotate(node.rotation, [place[0] * node.scale[0], place[1] * node.scale[1], place[2] * node.scale[2]]);
        place = [p[0] + node.translation[0], p[1] + node.translation[1], p[2] + node.translation[2]];
        at = node.parent.unwrap();
    }
    let (bottom, at, scale) = ([0.0f32, 9.53, 20.40], [1.40f32, 3.24, 11.745], 0.9f32);
    let eyepiece: [f32; 3] = std::array::from_fn(|k| at[k] + scale * (bottom[k] - at[k]));
    let wanted = [eyepiece[0] * 0.01, (eyepiece[2] - 42.05) * 0.01, (eyepiece[1] + 14.145 - 13.0) * 0.01];
    assert!((0..3).all(|k| (place[k] - wanted[k]).abs() < 5e-4), "{place:?} {wanted:?}");

    // Every piece of the model drawn the right way out (the file is read in
    // its node's axes and turned: neither mirrors it).
    for part in &custom.parts {
        let mesh = &part.mesh;
        let (mut agree, mut count) = (0usize, 0usize);
        for triangle in mesh.submeshes.iter().flat_map(|indices| indices.chunks_exact(3)) {
            let [a, b, c] = [0, 1, 2].map(|k| mesh.positions[triangle[k] as usize]);
            let (u, v): ([f32; 3], [f32; 3]) = (std::array::from_fn(|k| b[k] - a[k]), std::array::from_fn(|k| c[k] - a[k]));
            let face = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let normal: [f32; 3] = std::array::from_fn(|k| triangle.iter().map(|&i| mesh.normals[i as usize][k]).sum());
            if face.iter().map(|f| f * f).sum::<f32>() > 1e-16 {
                count += 1;
                agree += ((0..3).map(|k| face[k] * normal[k]).sum::<f32>() > 0.0) as usize;
            }
        }
        assert!(agree as f32 > 0.95 * count as f32, "{}: {agree} of {count} triangles face their normals' way", mesh.name);
    }

    // Alone, each is its own model: the ZF4 its one piece of the model, the
    // mount with the ZF4 in it the three.
    let (alone, own) = trp_data::weapons::custom::part_model(&mut assets, &db, &trp_data::weapons::weapon::alone(ZF4)).expect("the ZF4 alone");
    assert_eq!((own.parts.len(), own.materials.len()), (1, 1));
    assert_eq!(own.materials[0].name, "Scope");
    assert_eq!((alone.nodes[0].translation, alone.nodes[0].rotation), ([0.0; 3], [0.0, 0.0, 0.0, 1.0]));
    let fitted = trp_data::weapons::weapon::preset_of_key(&format!("{MOUNT}[mod_scope={ZF4}[]]")).unwrap();
    let (_, both) = trp_data::weapons::custom::part_model(&mut assets, &db, &fitted).expect("the mount with its ZF4");
    assert_eq!(both.parts.len(), 3);
    assert_eq!(both.materials.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Scope", "Receiver"]);
}

/// The StG 44 is worked by its donor's animator, and by nothing else:
/// its states, their times and their marks are the SCAR-L's as that
/// weapon would have them with no catch for its bolt. What differs is
/// where the bones are in the pose given out (`trp_data::custom::
/// PoseEdits`): the left hand rests on this rifle's handguard, stays on
/// its magazine wherever that is carried, and takes its cocking handle
/// where this rifle has it; the right hand's marker, which the
/// simulation goes by, is the donor's own.
#[test]
fn a_weapon_of_ones_own_is_worked_by_its_donors_animator() {
    use trp_sim::base::math::{quat_mul, quat_rotate};
    use trp_sim::base::rig::WeaponRig;
    use trp_sim::Firearm;
    const FRAME: f32 = 1.0 / 120.0;
    let built = crate::fixture::install().ok_or(()).ok().and_then(|install| Database::load(&install).ok()).is_some_and(|db| db.custom.contains_key(STG44));
    if !built {
        eprintln!("skipped: no install, or the StG 44 is not built here");
        return;
    }
    let Some((ours, ours_data, ours_settings, ..)) = weapon_of(STG44) else { return };
    let Some((theirs, theirs_data, mut theirs_settings, ..)) = weapon_of(SCAR_L) else { return };
    // The donor as a weapon with no catch, at the same rate of fire and
    // with the same round, its magazine's clips the StG's (the 40-round's:
    // `custom/stg44.json`'s `magAnimationIndex`).
    theirs_settings.bolt_catch = false;
    theirs_settings.fire_rate = ours_settings.fire_rate;
    theirs_settings.shot_strength = ours_settings.shot_strength;
    assert_eq!(ours_settings.magazine_variant, 3, "the StG 44's magazine plays the SCAR-L's 40-round's clips");
    theirs_settings.magazine_variant = ours_settings.magazine_variant;
    assert_eq!(ours_data.state_names(), theirs_data.state_names());

    let bone = |weapon: &trp_data::weapons::weapon::Weapon, name: &str| {
        let path = format!("Weapon_root/Weapon_root_anim/weapon{name}");
        (0..weapon.hierarchy.nodes.len()).find(|&i| weapon.hierarchy.path(i) == path).unwrap_or_else(|| panic!("no bone {name}"))
    };
    // A bone on the weapon's own bone: centimetres ahead, up and to the right.
    let on_weapon = |rig: &WeaponRig, weapon: usize, bone: usize| -> ([f32; 3], [f32; 4]) {
        let ((at, turn), (there, turned)) = (rig.placed(weapon), rig.placed(bone));
        let back = [-turn[0], -turn[1], -turn[2], turn[3]];
        let p = quat_rotate(back, std::array::from_fn(|k| there[k] - at[k]));
        ([-p[1] * 100.0, p[2] * 100.0, p[0] * 100.0], quat_mul(back, turned))
    };
    let apart = |a: [f32; 3], b: [f32; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt();

    struct Side<'a> {
        weapon: &'a trp_data::weapons::weapon::Weapon,
        rig: WeaponRig,
        logic: Firearm,
    }
    let side = |weapon, settings: &trp_sim::FirearmSettings, data| Side { weapon, rig: WeaponRig::new(&weapon.hierarchy), logic: Firearm::new(settings.clone(), data) };
    let (mut a, mut b) = (side(&ours, &ours_settings, &ours_data), side(&theirs, &theirs_settings, &theirs_data));
    // What is read off both rigs each frame.
    struct Read {
        state: String,
        left: [f32; 3],
        right: [f32; 3],
        magazine: [f32; 3],
        handle: [f32; 3],
    }
    let read = |s: &mut Side| -> Read {
        let moved = s.rig.moved_bone();
        s.rig.pose(s.logic.animator().pose(), moved);
        let weapon = bone(s.weapon, "");
        let at = |name: &str| on_weapon(&s.rig, weapon, bone(s.weapon, name)).0;
        let (state, through, next) = s.logic.animator().state();
        Read {
            state: format!("{state} {through:.4} {next:?}"),
            left: at("/weapon_L_IK_marker"),
            right: at("/weapon_R_IK_marker"),
            magazine: at("/mod_magazine"),
            handle: at("/weapon_bolt/mod_charge"),
        }
    };
    // How many frames were set side by side, on how many of them the
    // hand had the magazine and how far from the donor's hold at worst,
    // and where the palm was from the donor's at the handle's fullest
    // pull.
    let tally = std::cell::RefCell::new((0usize, 0usize, 0.0f32, None::<([f32; 3], f32)>));
    let step = |a: &mut Side, b: &mut Side, seconds: f32| {
        let mut tally = tally.borrow_mut();
        let (frames, on_magazine, worst_on_magazine, pulled) = &mut *tally;
        for _ in 0..(seconds / FRAME).round() as usize {
            // The same things said by both, frame for frame.
            assert_eq!(a.logic.step(FRAME), b.logic.step(FRAME));
            let (ours, theirs) = (read(a), read(b));
            assert_eq!(ours.state, theirs.state);
            *frames += 1;
            // The right hand's marker is the donor's own.
            assert!(apart(ours.right, theirs.right) < 1e-3, "{:?} {:?}", ours.right, theirs.right);
            // The handle travels the donor's path.
            assert!(apart(ours.handle, theirs.handle) < 1e-3);
            // While the donor's hand has its magazine (the palm's marker
            // within 3 cm of where it holds one), ours is as far from
            // ours.
            let rest = [40.8f32, -0.8, 0.0];
            let holding = [[-8.4f32, -20.7, -8.6], [-7.8, -23.3, -7.1]].iter().any(|palm| apart(theirs.left, std::array::from_fn(|k| theirs.magazine[k] + palm[k])) < 3.0);
            if holding && apart(theirs.magazine, rest) > 1.0 {
                let (mine, donor): ([f32; 3], [f32; 3]) = (std::array::from_fn(|k| ours.left[k] - ours.magazine[k]), std::array::from_fn(|k| theirs.left[k] - theirs.magazine[k]));
                *on_magazine += 1;
                *worst_on_magazine = worst_on_magazine.max(apart(mine, donor));
            }
            // And at the handle's fullest pull, where the hand is from
            // the donor's.
            let travel = 47.6 - theirs.handle[0];
            if pulled.is_none_or(|(_, most)| travel > most) {
                *pulled = Some((std::array::from_fn(|k| ours.left[k] - theirs.left[k]), travel));
            }
        }
    };

    // At rest in the hands: the left hand on the handguard, 5.6 cm ahead
    // of where the donor has it, 2 cm higher and 0.9 cm nearer the bore.
    for s in [&mut a, &mut b] {
        s.logic.start_drawn();
    }
    step(&mut a, &mut b, 0.5);
    let (ours_at, theirs_at) = (read(&mut a), read(&mut b));
    let rested: [f32; 3] = std::array::from_fn(|k| ours_at.left[k] - theirs_at.left[k]);
    assert!(apart(rested, [5.6, 2.0, 0.9]) < 0.02, "{rested:?}");
    // Its magazine's bone is at this rifle's magazine well.
    let moved: [f32; 3] = std::array::from_fn(|k| ours_at.magazine[k] - theirs_at.magazine[k]);
    assert!(apart(moved, [5.30, 0.48, 0.0]) < 0.05, "{moved:?}");

    // A reload from empty: the magazine changed, the handle pulled.
    for s in [&mut a, &mut b] {
        (s.logic.chamber, s.logic.magazine) = (false, Some(0));
        s.logic.reload();
    }
    step(&mut a, &mut b, 5.0);
    assert!(a.logic.is_idle() && a.logic.chamber && a.logic.magazine == Some(29));
    let (frames, on_magazine, worst_on_magazine, pulled) = *tally.borrow();
    let (shift, travel) = pulled.expect("the handle was pulled");
    println!(
        "the StG 44's reload from empty: {frames} frames in the donor's states; the hand on the magazine for {on_magazine} of them, within {worst_on_magazine:.2} cm of the donor's hold; at the handle's full pull ({travel:.1} cm) the palm is {shift:.2?} cm from the donor's"
    );
    assert!(on_magazine > 40 && worst_on_magazine < 1.0, "{on_magazine} {worst_on_magazine}");
    assert!((travel - 10.6).abs() < 0.2 && apart(shift, [-6.9, 0.0, 1.1]) < 0.3, "{travel} {shift:?}");

    // A shot, a switch to automatic, a look at the magazine and one into
    // the chamber: all the donor's, frame for frame.
    for s in [&mut a, &mut b] {
        s.logic.set_trigger(true, true);
    }
    step(&mut a, &mut b, 0.1);
    for s in [&mut a, &mut b] {
        s.logic.set_trigger(false, true);
    }
    step(&mut a, &mut b, 0.5);
    assert_eq!(a.logic.magazine, Some(28));
    // The button that is this rifle's selector turns in place with the
    // donor's: it is round, and its middle stays where it is.
    let button = |s: &mut Side| {
        let moved = s.rig.moved_bone();
        s.rig.pose(s.logic.animator().pose(), moved);
        let (weapon, node) = (bone(s.weapon, ""), bone(s.weapon, "/mod_trigger_group/mod_selector"));
        on_weapon(&s.rig, weapon, node)
    };
    let switch = |a: &mut Side, b: &mut Side| {
        a.logic.next_fire_mode();
        b.logic.next_fire_mode();
        step(a, b, 2.0);
        button(a)
    };
    let automatic = switch(&mut a, &mut b);
    assert_eq!(a.logic.mode(), trp_sim::FireMode::FullAuto);
    // The donor's selector turns about the axis across the weapon (45
    // degrees between its two modes, by its clips), and this rifle's
    // selector is a round button across the weapon: its bone is at the
    // button's own middle, so the turn leaves the button where it is.
    let custom = ours.custom.as_ref().expect("its own parts");
    for part in custom.parts.iter().filter(|part| part.mesh.name.starts_with("Body_FiremodeButton")) {
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &part.mesh.positions {
            (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
        }
        // Ahead and up, in centimetres, as the bone is given.
        let middle = [-(low[1] + high[1]) * 50.0, (low[2] + high[2]) * 50.0];
        assert!((middle[0] - automatic.0[0]).abs() < 0.06 && (middle[1] - automatic.0[1]).abs() < 0.06, "{} {middle:?} {:?}", part.mesh.name, automatic.0);
    }
    assert!(apart(automatic.0, [26.52, -1.67, 0.0]) < 0.05 && quat_mul(automatic.1, [0.0, 0.0, 0.0, 1.0])[1].abs() < 1e-3, "{automatic:?}");
    for check in [0, 1] {
        for s in [&mut a, &mut b] {
            if check == 0 {
                s.logic.check_magazine();
            } else {
                s.logic.check_chamber();
            }
        }
        step(&mut a, &mut b, 4.0);
        assert!(a.logic.is_idle());
    }
    // Held, it fires at its own rate: 540 rounds a minute.
    for s in [&mut a, &mut b] {
        s.logic.set_trigger(true, true);
    }
    step(&mut a, &mut b, 1.0);
    let fired = 28 - a.logic.magazine.unwrap_or(0);
    println!("held for a second on automatic it fired {fired} rounds");
    assert!((8..=10).contains(&fired), "{fired}");
}
