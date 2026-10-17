//! Checks against a real install. They pass trivially, with a note, on a
//! machine where no install is configured.

use std::collections::HashMap;

use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
use trp_data::unity::extract::{read_mesh, read_texture};
use trp_data::unity::Assets;
use trp_data::weapon::{assemble, Weapon};
use trp_data::{Database, Install};

const AKM: &str = "59d6088586f774275f37482f";

fn akm() -> Option<(Assets, Weapon)> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = assemble(&mut assets, &db, AKM).expect("assemble AKM");
    Some((assets, weapon))
}

#[test]
fn stock_akm_assembles_with_every_part() {
    let Some((assets, weapon)) = akm() else { return };
    assert!(weapon.skipped.is_empty(), "parts not placed: {:?}", weapon.skipped);

    let rendered: Vec<_> = weapon.hierarchy.nodes.iter().filter(|n| n.renderer.is_some()).collect();
    // Seven meshes on the base rifle and one for each of the eight parts.
    assert_eq!(rendered.len(), 15);
    assert!(rendered.iter().all(|n| n.name.ends_with("LOD0")), "a lower detail mesh was kept");

    for node in rendered {
        let renderer = node.renderer.as_ref().unwrap();
        let mesh = read_mesh(&assets, renderer.mesh).expect("mesh");
        assert!(!mesh.positions.is_empty(), "{} has no vertices", node.name);
        assert_eq!(mesh.normals.len(), mesh.positions.len());
        assert_eq!(mesh.uv0.len(), mesh.positions.len());
        let vertex_count = mesh.positions.len() as u32;
        assert!(mesh.submeshes.iter().flatten().all(|&i| i < vertex_count));
        for material in renderer.materials.iter().flatten() {
            let material = trp_data::unity::extract::read_material(&assets, *material).expect("material");
            let main = material.textures.get("_MainTex").expect("main texture");
            let texture = read_texture(&assets, *main).expect("texture");
            assert!(texture.width > 0 && !texture.data.is_empty());
        }
    }
}

/// A texture's levels read again from its bundle's file, only the blocks
/// they are in decompressed (`ResourcePlace`, for the streaming of
/// pictures), are the bytes the whole bundle gave.
#[test]
fn a_texture_read_again_from_its_bundle_is_the_same() {
    let Some((assets, weapon)) = akm() else { return };
    let mut checked = 0;
    for renderer in weapon.hierarchy.nodes.iter().filter_map(|n| n.renderer.as_ref()) {
        for material in renderer.materials.iter().flatten() {
            let material = trp_data::unity::extract::read_material(&assets, *material).expect("material");
            for texture in material.textures.values() {
                let (texture, place) = trp_data::unity::extract::read_texture_shared(&assets, *texture).expect("texture");
                let Some(place) = place else { continue };
                assert_eq!(place.read().expect("read again"), texture.data, "{}", texture.name);
                checked += 1;
            }
        }
    }
    assert!(checked > 10, "only {checked} textures kept beside their files");
}

/// The game's engine streams a texture by its levels only where it was
/// imported so (`m_StreamingMipmaps`), and draws the rest whole when
/// streaming is on. Every one of the AKM's 27 is (read 2026-10-15, each of
/// priority 0).
#[test]
fn the_weapon_s_textures_are_marked_for_streaming() {
    let Some((assets, weapon)) = akm() else { return };
    let mut seen = std::collections::BTreeMap::new();
    for renderer in weapon.hierarchy.nodes.iter().filter_map(|n| n.renderer.as_ref()) {
        for material in renderer.materials.iter().flatten() {
            let material = trp_data::unity::extract::read_material(&assets, *material).expect("material");
            for texture in material.textures.values() {
                let texture = read_texture(&assets, *texture).expect("texture");
                seen.insert(texture.name.clone(), texture.streaming_mipmaps);
            }
        }
    }
    assert_eq!(seen.len(), 27);
    assert!(seen.values().all(|streamed| *streamed), "not streamed: {:?}", seen.iter().filter(|(_, s)| !**s).collect::<Vec<_>>());
}

/// The idle clip starts in the pose the prefab is stored in, so its first
/// frame must reproduce the prefab's rotations. This is what pins down the
/// order Euler angles are applied in.
#[test]
fn idle_clip_starts_in_the_rest_pose() {
    let Some((assets, weapon)) = akm() else { return };
    let nodes: HashMap<u32, usize> =
        (0..weapon.hierarchy.nodes.len()).map(|i| (path_hash(&weapon.hierarchy.path(i)), i)).collect();
    let clips = weapon.clips(&assets).expect("clips");
    let (_, idle) = clips.iter().find(|(name, _)| name == "akm_idle_armed").expect("idle clip");
    let clip = Clip::read(&assets, *idle).expect("decode");

    let mut values = Vec::new();
    clip.sample(0.0, &mut values);
    let mut checked = 0;
    for track in clip.tracks.iter().filter(|t| t.property == Property::Euler) {
        let Some(&node) = nodes.get(&track.path) else { continue };
        let q = euler_to_quat([values[track.curve], values[track.curve + 1], values[track.curve + 2]]);
        let rest = weapon.hierarchy.nodes[node].rotation;
        let dot: f32 = q.iter().zip(&rest).map(|(a, b)| a * b).sum();
        let degrees = 2.0 * dot.abs().min(1.0).acos().to_degrees();
        assert!(degrees < 0.5, "{} is {degrees} degrees from rest", weapon.hierarchy.nodes[node].name);
        checked += 1;
    }
    assert!(checked > 50, "only {checked} rotation tracks landed on the rig");
}

/// What a character wears is read from the item's own bundle in one of
/// two forms: a skin for the player's skeleton (armour, vests, backpacks,
/// the last with a mesh for each thing it may be worn over) or a model
/// for the head bone (a helmet).
#[test]
fn worn_items_are_skins_or_models_for_the_head() {
    use trp_data::dress::{load, Under, Worn};
    let Some((mut assets, _)) = akm() else { return };
    const ARMOUR: &str = "assets/content/items/equipment/armor_paca/item_equipment_armor_paca.bundle";
    const BACKPACK: &str = "assets/content/items/equipment/backpack_baselard/item_equipment_backpack_baselard.bundle";
    const HELMET: &str = "assets/content/items/equipment/helmet_6b47/item_equipment_helmet_6b47.bundle";

    let Worn::Skinned(armour) = load(&mut assets, ARMOUR, Under::default()).expect("armour") else { panic!("armour is a skin") };
    assert!(armour.bone_paths.iter().any(|path| path.contains("Base HumanSpine")), "{:?}", armour.bone_paths);

    // A backpack has a mesh for the bare body and one for each of
    // armour, a vest and both under it.
    let mut meshes = Vec::new();
    for under in [(false, false), (true, false), (false, true), (true, true)] {
        let worn = load(&mut assets, BACKPACK, Under { armour: under.0, vest: under.1 }).expect("backpack");
        let Worn::Skinned(pack) = worn else { panic!("a backpack is a skin") };
        assert!(!pack.bone_paths.is_empty());
        meshes.push(pack.mesh);
    }
    println!("an armour on {} bones; a backpack's four meshes: {meshes:?}", armour.bone_paths.len());
    assert!(meshes.iter().enumerate().all(|(i, mesh)| meshes[..i].iter().all(|other| other != mesh)), "{meshes:?}");

    let Worn::Rigid(helmet) = load(&mut assets, HELMET, Under::default()).expect("helmet") else { panic!("a helmet is a model") };
    let drawn = helmet.nodes.iter().filter(|node| node.renderer.is_some()).count();
    println!("a helmet: {} objects, {drawn} of them drawn", helmet.nodes.len());
    assert!(drawn >= 1);
}

#[test]
fn arms_bind_to_bones_the_weapon_has() {
    let Some((mut assets, weapon)) = akm() else { return };
    let arms = trp_data::arms::load(&mut assets, "assets/content/hands/bear/bear_hands_skin.bundle").expect("arms");
    let mesh = read_mesh(&assets, arms.mesh).expect("arms mesh");
    assert_eq!(mesh.bind_poses.len(), arms.bone_paths.len());
    assert_eq!(mesh.weights.len(), mesh.positions.len());

    let rig: Vec<String> = (0..weapon.hierarchy.nodes.len()).map(|i| weapon.hierarchy.path(i)).collect();
    for path in arms.weapon_bone_paths() {
        assert!(rig.iter().any(|p| p == path), "weapon has no bone {path}");
    }
    let bone_count = arms.bone_paths.len() as u16;
    assert!(mesh.joints.iter().flatten().all(|&j| j < bone_count));
}

/// The body's animator controller reads in full: parameters, the base
/// layer's states with the movement state each stands for, and the layer
/// synchronised with it that poses the arms in a sprint.
#[test]
fn the_body_controller_reads_in_full() {
    let Some((mut assets, weapon)) = akm() else { return };
    // The stock AKM's width in inventory cells with its parts fitted,
    // which the body's animator takes as a parameter.
    assert_eq!(weapon.cell_width, 5);

    let table = trp_data::body::controller(&mut assets).expect("body controller");
    println!("{} parameters, {} states, {} layers", table.parameters.len(), table.states.len(), table.layers.len());
    let kind = |name: &str| table.parameters.iter().find(|p| p.name == name).map(|p| (p.kind, p.default));
    use trp_data::states::ParameterKind::{Bool, Float};
    assert_eq!(kind("Inert"), Some((Bool, 0.0)));
    assert_eq!(kind("Level"), Some((Float, 1.0)));
    assert_eq!(kind("FootStep"), Some((Float, 0.0)));

    // The game's numbers for standing, walking and sprinting.
    let movement = |name: &str| table.find(name).and_then(|s| s.movement.as_ref()).map(|m| m.state);
    assert_eq!(movement("Base Layer.Stand.Idle_Aim"), Some(1));
    assert_eq!(movement("Base Layer.StateMachine_Move.MOVE"), Some(4));
    assert_eq!(movement("Base Layer.StateMachine_Sprint.Sprint"), Some(5));
    let idle = table.find("Base Layer.Stand.Idle_Aim").unwrap();
    assert!(idle.movement.as_ref().unwrap().default_state);
    // Standing gives way to walking over a fixed 0.15 s.
    let to_walk = idle.transition_to("Base Layer.StateMachine_Move.MOVE").expect("a way from standing to walking");
    assert!(to_walk.fixed_duration && (to_walk.duration - 0.15).abs() < 1e-4);

    // The sprint has a tree for its own layer and one for the hands layer.
    let sprint = table.find("Base Layer.StateMachine_Sprint.Sprint").unwrap();
    let hands = table.layers.iter().find(|l| l.name == "Sync_SprintHands").expect("the hands layer");
    assert_eq!(hands.machine, sprint.machine);
    assert!(hands.sync_slot > 0);
    assert!(sprint.tree_of_layer[hands.sync_slot] >= 0);
    assert_eq!(idle.tree_of_layer[hands.sync_slot], -1, "standing has nothing on the hands layer");

    let bones = trp_data::body::bones(&mut assets, &["Root_Joint", "Root_Joint/Base HumanPelvis"]).expect("bones");
    println!("the pelvis rests {:?} above the root", bones[1].position);
    assert!((0.8..1.1).contains(&bones[1].position[1]));
}

/// The engine's three two-parameter blends, worked out here, against the
/// weight grids the install stores for the same trees.
#[test]
fn two_parameter_blends_match_the_stored_grids() {
    use trp_data::rootmotion::{BlendType, Node, RootMotionTable};
    let Some((mut assets, _)) = akm() else { return };
    let table = RootMotionTable::load(&mut assets).expect("root motion table");
    // Per kind of blend: how many trees were checked and the worst
    // difference in any weight anywhere on any grid.
    let mut found: std::collections::BTreeMap<String, (usize, f32, String)> = Default::default();
    fn visit(
        table: &RootMotionTable,
        state: &str,
        node: &Node,
        found: &mut std::collections::BTreeMap<String, (usize, f32, String)>,
    ) {
        let Node::Tree { blend, children, .. } = node else { return };
        // The freeform directional kind is only held to the engine's
        // figures among its children. Far outside them (ten times as far
        // out, or on the far side of the origin) the stored grids differ
        // from what is computed here, by up to a half in places; no state
        // the simulation uses takes its parameters there.
        let among_children = *blend == BlendType::FreeformDirectional2D;
        if let Some(mismatch) = table.grid_mismatch_in(node, among_children) {
            let entry = found.entry(format!("{blend:?}")).or_insert((0, 0.0, String::new()));
            entry.0 += 1;
            if mismatch > entry.1 {
                (entry.1, entry.2) = (mismatch, state.to_owned());
            }
        }
        for child in children {
            visit(table, state, child, found);
        }
    }
    for (name, node) in table.states.values() {
        visit(&table, name, node, &mut found);
    }
    for (kind, (count, worst, state)) in &found {
        println!("{kind}: {count} trees, largest weight difference {worst:.5} (in {state})");
    }
    for kind in [BlendType::FreeformCartesian2D, BlendType::FreeformDirectional2D, BlendType::SimpleDirectional2D] {
        let (count, worst, state) = found.get(&format!("{kind:?}")).unwrap_or_else(|| panic!("no {kind:?} tree in the table"));
        assert!(*count > 0);
        assert!(*worst < 2e-3, "{kind:?} differs by {worst} in {state}");
    }
}

/// The player's own settings for the game are read where the install has
/// them, and what is read lies in the game's own ranges for them.
#[test]
fn the_installs_view_settings_are_in_range() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let settings = install.player_settings();
    println!("the install's settings: {settings:?}");
    if let Some(field_of_view) = settings.field_of_view {
        assert!((50.0..=75.0).contains(&field_of_view), "field of view {field_of_view}");
    }
    if let Some(head_bobbing) = settings.head_bobbing {
        assert!((0.2..=1.0).contains(&head_bobbing), "head bobbing {head_bobbing}");
    }
    for sensitivity in [settings.mouse_sensitivity, settings.aiming_sensitivity].into_iter().flatten() {
        assert!(sensitivity > 0.0 && sensitivity <= 5.0, "sensitivity {sensitivity}");
    }
}

/// An outfit that is not the install's (`docs/spec/outfits.md`): the
/// Sunrise suit's model, read from the other game's folders where those
/// are configured, is fitted to the skeleton of the character's own
/// clothing with its own build kept. Skipped with a note where either
/// game is not there.
#[test]
fn an_outfit_of_another_game_is_fitted_to_the_body() {
    use trp_data::outfit::{self, Follows, Move, RestBone, ARMS, LEGS, UPPER};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let Some(definition) = db.outfits.values().find(|definition| definition.name == "outfit_stalker_sunrise") else {
        eprintln!("skipped: the outfit's file is not found from here ({:?})", db.custom_notes);
        return;
    };
    let stalker = trp_data::xray::Stalker::locate();
    let visual = definition.visual.clone().expect("the suit names a body");
    let Some(file) = stalker.find(&format!("meshes/{visual}")) else {
        eprintln!("skipped: the other game's folders are not configured, or have no {visual} (local/gamma_dir.txt, local/anomaly_dir.txt)");
        return;
    };
    // It is an item like its donor, and one piece.
    let item = db.item(&definition.id).expect("its item");
    assert_eq!(item.parent, db.item(&definition.donor).unwrap().parent);
    assert_eq!(item.props["Slots"].as_array().map(Vec::len), Some(0));

    // The skeleton as the default clothing has it at rest.
    let mut assets = Assets::new(install).expect("manifest");
    let mut skeleton: Vec<RestBone> = Vec::new();
    // The box about the install's own legs at rest: its top is the waist.
    let mut waist: Option<[[f32; 3]; 2]> = None;
    for key in ["tshirt_bear_turtleneck", "pants_bear_srvv", "bear_head_0"] {
        let part = trp_data::arms::load(&mut assets, &format!("assets/content/characters/character/prefabs/{key}.bundle")).expect("a piece of clothing");
        let mesh = read_mesh(&assets, part.mesh).expect("its mesh");
        for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
            if !skeleton.iter().any(|known| &known.path == path) {
                skeleton.push(RestBone { path: path.clone(), bind: *bind });
            }
        }
        if key.starts_with("pants") {
            waist = outfit::box_about(&mesh.positions);
        }
    }
    let waist = waist.expect("the legs have a box");
    println!("  the install's legs at rest: from {:.3} {:.3} {:.3} to {:.3} {:.3} {:.3}", waist[0][0], waist[0][1], waist[0][2], waist[1][0], waist[1][1], waist[1][2]);
    let fitted = outfit::build(definition, &stalker, &skeleton, Some(waist));
    for note in &fitted.notes {
        println!("  {note}");
    }
    let rig = &fitted.rig;
    assert!(fitted.has_body(), "the suit's body is drawn");
    // What its own eyes see of it is under its waist, which is as far
    // over its own hips as the top of the install's legs is over the
    // install's (the install's meshes stand along z); and it is not
    // nothing.
    let hips: Vec<_> = ["LThigh1", "RThigh1"].iter().map(|name| rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{name}"))).expect("a hip")).collect();
    let (ours_up, theirs_up) = ((hips[0].joint[2] + hips[1].joint[2]) / 2.0, (hips[0].their_joint[2] + hips[1].their_joint[2]) / 2.0);
    let top = ours_up + waist[1][2] - theirs_up;
    println!("  the waist: {:.3} m up on the install's own body ({:.3} over its hips at {theirs_up:.3}), {top:.3} on the suit (its hips at {ours_up:.3})", waist[1][2], waist[1][2] - theirs_up);
    let mut seen = 0;
    for piece in &fitted.pieces {
        for corner in &piece.mesh.submeshes[LEGS] {
            assert!(piece.mesh.positions[*corner as usize][2] <= top + 1e-5, "a corner of the legs' list is over the waist");
        }
        for triangle in piece.mesh.submeshes[UPPER].chunks_exact(3) {
            assert!(triangle.iter().any(|corner| piece.mesh.positions[*corner as usize][2] > top), "a triangle under the waist is the upper body's");
        }
        seen += piece.mesh.submeshes[LEGS].len() / 3;
    }
    assert!(seen > 500, "{seen} triangles under the waist");
    // Without its body's model it leaves the character's own body.
    let mut bodiless = (**definition).clone();
    bodiless.visual = Some("actors/no_such/model.ogf".to_owned());
    let left = outfit::build(&bodiless, &stalker, &skeleton, Some(waist));
    assert!(!left.has_body() && left.rig.bones.is_empty() && left.notes.iter().any(|note| note.contains("the character's own")));
    assert_eq!(left.has_arms(), fitted.has_arms());
    for piece in &fitted.pieces {
        let mesh = &piece.mesh;
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &mesh.positions {
            assert!(p.iter().all(|v| v.is_finite()));
            for axis in 0..3 {
                (low[axis], high[axis]) = (low[axis].min(p[axis]), high[axis].max(p[axis]));
            }
        }
        println!(
            "  {}: {} vertices; triangles: legs {}, arms {}, the rest {}; from {:.3} {:.3} {:.3} to {:.3} {:.3} {:.3}; its picture {}",
            piece.texture,
            mesh.positions.len(),
            mesh.submeshes[LEGS].len() / 3,
            mesh.submeshes[ARMS].len() / 3,
            mesh.submeshes[UPPER].len() / 3,
            low[0],
            low[1],
            low[2],
            high[0],
            high[1],
            high[2],
            piece.picture.as_ref().map_or("not found".to_owned(), |p| format!("{} x {} {:?}", p.width, p.height, p.packing)),
        );
        assert!(mesh.weights.iter().all(|shares| (shares.iter().sum::<f32>() - 1.0).abs() < 1e-4));
        assert!(mesh.joints.iter().flatten().all(|joint| (*joint as usize) < rig.bones.len()));
        assert_eq!((mesh.normals.len(), mesh.uv0.len(), mesh.bind_poses.len()), (mesh.positions.len(), mesh.positions.len(), rig.bones.len()));
    }

    // The model's own build is kept. Set beside the model as its file has
    // it (y up there, z up here): as tall, standing on the ground; its
    // hips and its shoulders as far apart; every limb's bone as long.
    let model = trp_data::xray::read(&std::fs::read(&file).unwrap()).unwrap();
    let ours = |name: &str| model.bones.iter().find(|bone| bone.name == name).unwrap().at;
    let joint = |name: &str| rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{name}"))).map(|bone| (bone.joint, bone.their_joint)).unwrap();
    let apart = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let body = &fitted.pieces[0];
    let heights: Vec<f32> = body.mesh.positions.iter().map(|p| p[2]).collect();
    let (lowest, highest) = (heights.iter().copied().fold(f32::MAX, f32::min), heights.iter().copied().fold(f32::MIN, f32::max));
    let tall = model.parts[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max) - model.parts[0].positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    println!("  the model is {tall:.3} m tall; fitted, from {lowest:.3} to {highest:.3}");
    assert!(lowest.abs() < 0.005 && (highest - lowest - tall).abs() < 0.005, "from {lowest} to {highest}, of a model {tall} tall");
    for (what, pair, their_pair) in [("hips", ["bip01_l_thigh", "bip01_r_thigh"], ["LThigh1", "RThigh1"]), ("shoulders", ["bip01_l_upperarm", "bip01_r_upperarm"], ["LUpperarm", "RUpperarm"])] {
        let (was, is, theirs) = (apart(ours(pair[0]), ours(pair[1])), apart(joint(their_pair[0]).0, joint(their_pair[1]).0), apart(joint(their_pair[0]).1, joint(their_pair[1]).1));
        println!("  the {what}: {was:.3} m apart on the model, {is:.3} on its rig, {theirs:.3} on the install's skeleton");
        assert!((was - is).abs() < 1e-4);
    }
    for (what, from, to, their_from, their_to) in [
        ("upper arm", "bip01_l_upperarm", "bip01_l_forearm", "LUpperarm", "LForearm1"),
        ("forearm", "bip01_l_forearm", "bip01_l_hand", "LForearm1", "LPalm"),
        ("thigh", "bip01_r_thigh", "bip01_r_calf", "RThigh1", "RCalf"),
        ("calf", "bip01_r_calf", "bip01_r_foot", "RCalf", "RFoot"),
    ] {
        let (was, is, theirs) = (apart(ours(from), ours(to)), apart(joint(their_from).0, joint(their_to).0), apart(joint(their_from).1, joint(their_to).1));
        println!("  the {what}: {was:.3} m long on the model, {is:.3} on its rig, {theirs:.3} on the install's skeleton");
        assert!((was - is).abs() < 1e-3, "the {what} is {is} long for {was}");
    }
    assert!(body.picture.is_some());
    assert!([LEGS, ARMS, UPPER].iter().all(|zone| !body.mesh.submeshes[*zone].is_empty()));

    // The rig: the hips, the hands and the feet are the install's own,
    // and at rest it moves nothing.
    for name in ["Pelvis", "LPalm", "RPalm", "LFoot", "RFoot"] {
        let bone = rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{name}"))).unwrap();
        assert_eq!(bone.follows, Follows::Theirs, "{name}");
    }
    let at_rest = rig.pose(&vec![Move::STILL; rig.bones.len()]);
    for (bone, moved) in rig.bones.iter().zip(&at_rest) {
        let off = moved.shift.iter().chain(moved.axes[0].iter().skip(1)).chain(moved.axes[1].iter().skip(2)).fold(0.0f32, |most, v| most.max(v.abs()));
        assert!(off < 1e-3, "{} is moved at rest: {moved:?}", bone.path);
    }
    // And the hands are on the install's hands, knuckle on knuckle.
    for (palm, knuckle) in [("LPalm", "LDigit31"), ("RPalm", "RDigit31")] {
        let (at, _) = joint(palm);
        assert!(apart(joint(knuckle).0, joint(knuckle).1) < 1e-4);
        let palm_bone = rig.bones.iter().position(|bone| bone.path.ends_with(palm)).unwrap() as u16;
        let of_hand: Vec<[f32; 3]> = body.mesh.positions.iter().zip(body.mesh.joints.iter().zip(&body.mesh.weights)).filter(|(_, (joints, weights))| (0..4).any(|slot| joints[slot] == palm_bone && weights[slot] > 0.9)).map(|(p, _)| *p).collect();
        let furthest = of_hand.iter().map(|p| apart(*p, at)).fold(0.0, f32::max);
        println!("  {palm}: {} vertices wholly the hand's, up to {furthest:.3} m off its joint", of_hand.len());
        assert!(!of_hand.is_empty() && furthest < 0.16, "the hand's vertices are up to {furthest} m from its joint");
    }
}

/// The same outfit's arms for first person (the other game's own model
/// for them), fitted to the install's arms: the hand's skeleton the size
/// of the install's (its knuckles' joints as far apart), every joint of
/// every finger on the install's own, the wrist on the install's wrist,
/// each arm lying as the install's lies, and at rest nothing moved.
/// Prints the palm's skin from edge to edge beside the install's own
/// hands, gloved and bare, and how far each joint of a hand is from the
/// install's own.
#[test]
fn an_outfit_s_arms_for_first_person_are_fitted_to_the_install_s() {
    use trp_data::outfit::{self, Follows, Move, RestBone, Skin, ARMS};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let Some(definition) = db.outfits.values().find(|definition| definition.name == "outfit_stalker_sunrise") else {
        eprintln!("skipped: the outfit's file is not found from here ({:?})", db.custom_notes);
        return;
    };
    let stalker = trp_data::xray::Stalker::locate();
    let Some(file) = definition.arms.as_ref().and_then(|visual| stalker.find(&format!("meshes/{visual}"))) else {
        eprintln!("skipped: the outfit names no arms, or the other game's folders have none");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let mut skeleton: Vec<RestBone> = Vec::new();
    for key in ["tshirt_bear_turtleneck", "pants_bear_srvv", "bear_head_0"] {
        let part = trp_data::arms::load(&mut assets, &format!("assets/content/characters/character/prefabs/{key}.bundle")).expect("a piece of clothing");
        let mesh = read_mesh(&assets, part.mesh).expect("its mesh");
        for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
            if !skeleton.iter().any(|known| &known.path == path) {
                skeleton.push(RestBone { path: path.clone(), bind: *bind });
            }
        }
    }
    let model = trp_data::xray::read(&std::fs::read(&file).unwrap()).unwrap();
    let (meshes, rig, size) = outfit::fit_arms(&model, &skeleton, definition.arms_size).expect("the arms are fitted");
    println!("  the arms' model is drawn at {size:.3} times its file's size");
    let apart = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let ours = |name: &str| model.bones.iter().find(|bone| bone.name == name || bone.name == format!("bip01_{name}")).unwrap().at;
    let joint = |name: &str| rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{name}"))).map(|bone| (bone.joint, bone.their_joint)).unwrap();
    // Left to itself, its hand's skeleton is the size of the install's:
    // the knuckles' joints as far apart, both hands' taken together.
    let (broad, their_broad) = (apart(ours("l_finger1"), ours("l_finger4")) + apart(ours("r_finger1"), ours("r_finger4")), apart(joint("LDigit21").1, joint("LDigit51").1) + apart(joint("RDigit21").1, joint("RDigit51").1));
    println!("  the hand across its knuckles' joints: {:.1} cm in the model's file, {:.1} on the install's skeleton", broad * 50.0, their_broad * 50.0);
    if definition.arms_size.is_none() {
        assert!((broad * size - their_broad).abs() < 1e-4, "{broad} times {size} for {their_broad}");
    }
    for (what, from, to, their_from, their_to) in [
        ("upper arm", "l_upperarm", "l_forearm", "LUpperarm", "LForearm1"),
        ("forearm", "l_forearm", "l_hand", "LForearm1", "LPalm"),
        ("palm, wrist to knuckle", "l_hand", "l_finger2", "LPalm", "LDigit31"),
        ("middle finger's first joint", "l_finger2", "l_finger21", "LDigit31", "LDigit32"),
        ("thumb's first joint", "l_finger0", "l_finger01", "LDigit11", "LDigit12"),
    ] {
        let (was, is, theirs) = (apart(ours(from), ours(to)), apart(joint(their_from).0, joint(their_to).0), apart(joint(their_from).1, joint(their_to).1));
        println!("  the {what}: {:.1} cm in the model's file ({:.1} at its size), {:.1} on its rig, {:.1} on the install's skeleton", was * 100.0, was * size * 100.0, is * 100.0, theirs * 100.0);
        // The upper arm is as long as the model's, at its size; a finger's
        // joint and the hand from its wrist are as long as the install's.
        match what {
            "upper arm" => assert!((was * size - is).abs() < 1e-4, "the {what} is {is} long for {was} times {size}"),
            "forearm" => {}
            _ => assert!((theirs - is).abs() < 1e-4, "the {what} is {is} long for the install's {theirs}"),
        }
    }
    // The wrist is the install's own: the arm bends there, wherever the
    // model has the joint, and each forearm bone is held there.
    for side in ["L", "R"] {
        let (at, their_at) = joint(&format!("{side}Palm"));
        assert!(apart(at, their_at) < 1e-4, "the {side} wrist is {} from the install's", apart(at, their_at));
        let palm = rig.bones.iter().position(|bone| bone.path.ends_with(&format!("Base Human{side}Palm"))).unwrap();
        assert_eq!(rig.bones[palm].follows, Follows::Theirs);
        for name in ["Forearm1", "Forearm2", "Forearm3"] {
            let bone = rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{side}{name}"))).unwrap();
            assert_eq!(bone.follows, Follows::Held { from: palm, at: palm }, "{side}{name}");
        }
    }
    println!("  the model has its wrist's joint {:.1} cm nearer its knuckles, at its size, than the install's skeleton has its own", (apart(joint("LPalm").1, joint("LDigit31").1) - apart(ours("l_hand"), ours("l_finger2")) * size) * 100.0);
    // The palm's skin from edge to edge and through, behind the knuckles
    // (`palm_box`): the model's as fitted, beside the install's own hands
    // (gloved, the default; bare, the Telnik's).
    let paths: Vec<String> = rig.bones.iter().map(|bone| bone.path.clone()).collect();
    let fitted = Skin::of(meshes.iter(), &paths);
    let mut their_skins = Vec::new();
    for (name, key) in [("the install's gloved hands", "assets/content/hands/bear/bear_hands_skin.bundle"), ("the install's bare hands", "assets/content/hands/hands_bear_telnik/hands_bear_telnik.skin.bundle")] {
        let part = trp_data::arms::load(&mut assets, key).expect("hands");
        let mesh = read_mesh(&assets, part.mesh).expect("their mesh");
        their_skins.push((name, Skin::of([&mesh], &part.bone_paths)));
    }
    let measured = |skin: &Skin| -> [f32; 2] {
        let mut sums = [0.0f32; 2];
        for side in ["L", "R"] {
            let [across, through] = outfit::palm_box(skin, &skeleton, side).expect("the hand's joints").expect("a palm");
            sums = [sums[0] + (across[1] - across[0]) * 0.5, sums[1] + (through[1] - through[0]) * 0.5];
        }
        sums
    };
    let [broad, thick] = measured(&fitted);
    println!("  the palm behind its knuckles, both hands' taken together: {:.1} cm from edge to edge and {:.1} through, as fitted", broad * 100.0, thick * 100.0);
    for (name, skin) in &their_skins {
        let [their_broad, their_thick] = measured(skin);
        println!("    {name}: {:.1} cm from edge to edge and {:.1} through (the fitted palm is {:.2} and {:.2} times that)", their_broad * 100.0, their_thick * 100.0, broad / their_broad, thick / their_thick);
    }
    for side in ["L", "R"] {
        let boxes: Vec<String> = [("fitted", &fitted)].into_iter().chain(their_skins.iter().map(|(name, skin)| (*name, skin))).map(|(name, skin)| {
            let [across, through] = outfit::palm_box(skin, &skeleton, side).unwrap().unwrap();
            format!("{name} {:.1} to {:.1} across, {:.1} to {:.1} through", across[0] * 100.0, across[1] * 100.0, through[0] * 100.0, through[1] * 100.0)
        }).collect();
        println!("    the {side} palm, cm from the middle of the knuckles' line: {}", boxes.join("; "));
    }
    // Left to itself, its palm is no narrower than the install's glove,
    // and not a fifth broader.
    if definition.arms_size.is_none() {
        let [their_broad, _] = measured(&their_skins[0].1);
        assert!((0.97..1.2).contains(&(broad / their_broad)), "the palm is {broad} broad for the install's {their_broad}");
    }
    println!("  each joint of the left hand, from the install's own:");
    for name in ["LPalm", "LDigit11", "LDigit12", "LDigit13", "LDigit21", "LDigit22", "LDigit23", "LDigit31", "LDigit32", "LDigit33", "LDigit41", "LDigit42", "LDigit43", "LDigit51", "LDigit52", "LDigit53", "LForearm1", "LUpperarm"] {
        let (at, their_at) = joint(name);
        println!("    {name:<10} {:5.1} cm   ({:6.3} {:6.3} {:6.3} for {:6.3} {:6.3} {:6.3})", apart(at, their_at) * 100.0, at[0], at[1], at[2], their_at[0], their_at[1], their_at[2]);
    }
    // Every joint of every finger is on the install's own.
    for side in ["L", "R"] {
        for finger in 1..=5 {
            for part in 1..=3 {
                let (at, their_at) = joint(&format!("{side}Digit{finger}{part}"));
                assert!(apart(at, their_at) < 1e-4, "{side}Digit{finger}{part} is {} from the install's", apart(at, their_at));
            }
        }
    }
    let at_rest = rig.pose(&vec![Move::STILL; rig.bones.len()]);
    assert!(at_rest.iter().all(|moved| *moved == Move::STILL));
    for mesh in &meshes {
        assert!(mesh.weights.iter().all(|shares| (shares.iter().sum::<f32>() - 1.0).abs() < 1e-4));
        assert!(mesh.submeshes[ARMS].len() == mesh.submeshes.iter().map(Vec::len).sum::<usize>() && !mesh.submeshes[ARMS].is_empty());
        println!("  a mesh of {} vertices, {} triangles", mesh.positions.len(), mesh.submeshes[ARMS].len() / 3);
    }
}

/// Every packed archive of the other game's is read: its table whole, and
/// the outfits' models of the item in it unpacked and checked against
/// their checksums. Skipped where the other game is not there.
#[test]
fn the_other_game_s_archives_are_read_whole() {
    let stalker = trp_data::xray::Stalker::locate();
    let Some(game) = stalker.game() else {
        eprintln!("skipped: the other game's folder is not known (local/anomaly_dir.txt)");
        return;
    };
    let began = std::time::Instant::now();
    let archives = stalker.archives();
    let files: usize = archives.iter().map(|archive| archive.len()).sum();
    println!("{} archives, {files} files, their tables read in {:.2} s", archives.len(), began.elapsed().as_secs_f32());
    assert_eq!(archives.len(), trp_data::xray_archive::archives_of(game).len(), "an archive's table could not be read");
    for name in ["meshes/dynamics/outfit/stalker_outfit.ogf", "meshes/dynamics/outfit/scientific_outfit.ogf"] {
        let archive = archives.iter().find(|archive| archive.has(name)).unwrap_or_else(|| panic!("{name} in no archive"));
        let bytes = archive.read(name).expect("read, unpacked and checked").expect("there");
        let model = trp_data::xray::read(&bytes).expect("a model");
        println!("  {name}: {} bytes, {} meshes, {} bones", bytes.len(), model.parts.len(), model.bones.len());
    }
}

/// Every outfit of the workspace's folder is an item of the database and
/// is built without failing: what of it the other game's files have,
/// loose or packed, is fitted (a body, arms for first person), and what
/// they have not is left to the character's own, with a note; its model
/// of the item, where it names one, is read with every picture. An outfit
/// made of the install's own clothing names bundles the install has.
/// Skipped with a note where the install is not there.
#[test]
fn every_outfit_of_the_folder_is_built() {
    use trp_data::outfit::{self, RestBone, ARMS, LEGS, UPPER};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    if db.outfits.is_empty() {
        eprintln!("skipped: no outfit's file is found from here ({:?})", db.custom_notes);
        return;
    }
    let stalker = trp_data::xray::Stalker::locate();
    let mut assets = Assets::new(install).expect("manifest");
    let mut skeleton: Vec<RestBone> = Vec::new();
    let mut waist = None;
    for key in ["tshirt_bear_turtleneck", "pants_bear_srvv", "bear_head_0"] {
        let part = trp_data::arms::load(&mut assets, &format!("assets/content/characters/character/prefabs/{key}.bundle")).expect("a piece of clothing");
        let mesh = read_mesh(&assets, part.mesh).expect("its mesh");
        for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
            if !skeleton.iter().any(|known| &known.path == path) {
                skeleton.push(RestBone { path: path.clone(), bind: *bind });
            }
        }
        if key.starts_with("pants") {
            waist = outfit::box_about(&mesh.positions);
        }
    }
    let own = trp_data::arms::load(&mut assets, "assets/content/hands/bear/bear_hands_skin.bundle").expect("the install's own arms");
    let their_skin = outfit::Skin::of([&read_mesh(&assets, own.mesh).expect("their mesh")], &own.bone_paths);
    let breadth = |skin: &outfit::Skin| -> f32 {
        ["L", "R"].iter().map(|side| outfit::palm_box(skin, &skeleton, side).expect("the hand's joints").expect("a palm")).map(|[across, _]| (across[1] - across[0]) * 0.5).sum()
    };
    let mut ids: Vec<&String> = db.outfits.keys().collect();
    ids.sort();
    for id in ids {
        let definition = &db.outfits[id];
        let item = db.item(&definition.id).expect("its item");
        assert_eq!(item.props["Slots"].as_array().map(Vec::len), Some(0), "{}", definition.name);
        let built = outfit::build(definition, &stalker, &skeleton, waist);
        println!("  {} ({}):", definition.name, definition.shown_name());
        for note in &built.notes {
            println!("    {note}");
        }
        let found = |name: &Option<String>| name.as_ref().is_some_and(|name| stalker.read(&format!("meshes/{name}")).is_ok());
        if let Some(world) = definition.world_visual.as_ref().filter(|_| stalker.known()) {
            let (bytes, found) = stalker.read(&format!("meshes/{world}")).expect("its model of the item");
            let model = trp_data::xray::read(&bytes).expect("its model of the item read");
            for part in &model.parts {
                let (bytes, _) = stalker.read(&format!("textures/{}.dds", part.texture)).expect("a picture of its model of the item");
                trp_data::xray::read_picture(&bytes).expect("a picture of its model of the item read");
            }
            println!("    its model of the item: {} meshes, every picture there; {found}", model.parts.len());
        }
        if found(&definition.visual) {
            assert!(built.has_body(), "{}: its body is among the other game's files and is not drawn", definition.name);
            let count = |list: usize| built.pieces.iter().filter(|piece| piece.picture.is_some()).map(|piece| piece.mesh.submeshes[list].len() / 3).sum::<usize>();
            println!("    its body: {} of {} meshes with their pictures, on {} bones; triangles under the waist {}, the arms' {}, the rest {}", built.pieces.iter().filter(|piece| piece.picture.is_some()).count(), built.pieces.len(), built.rig.bones.len(), count(LEGS), count(ARMS), count(UPPER));
            assert!(count(LEGS) > 300 && count(UPPER) > 300, "{}", definition.name);
        } else {
            assert!(!built.has_body());
        }
        if found(&definition.arms) {
            assert!(built.has_arms(), "{}: its arms are among the other game's files and are not drawn", definition.name);
            let arms = built.arms.as_ref().unwrap();
            let paths: Vec<String> = arms.rig.bones.iter().map(|bone| bone.path.clone()).collect();
            let (broad, their_broad) = (breadth(&outfit::Skin::of(arms.pieces.iter().map(|piece| &piece.mesh), &paths)), breadth(&their_skin));
            println!("    its arms for first person: {} meshes on {} bones, at {:.3} times their file's size; its palm {:.1} cm from edge to edge for the install's glove's {:.1}", arms.pieces.len(), arms.rig.bones.len(), arms.size, broad * 100.0, their_broad * 100.0);
            assert!((0.7..1.6).contains(&arms.size), "{}", arms.size);
            // Its wrists are the install's own.
            for bone in arms.rig.bones.iter().filter(|bone| bone.path.ends_with("Palm")) {
                let off = (0..3).map(|k| (bone.joint[k] - bone.their_joint[k]).powi(2)).sum::<f32>().sqrt();
                assert!(off < 1e-4, "{}: {} is {off} from the install's", definition.name, bone.path);
            }
            // Every finger's joints on the install's own.
            for bone in arms.rig.bones.iter().filter(|bone| bone.path.contains("Digit")) {
                let off = (0..3).map(|k| (bone.joint[k] - bone.their_joint[k]).powi(2)).sum::<f32>().sqrt();
                assert!(off < 1e-4, "{}: {} is {off} from the install's", definition.name, bone.path);
            }
        } else {
            assert!(!built.has_arms());
        }
        for (what, key) in [("top", &definition.their_top), ("hands", &definition.their_hands)] {
            if let Some(key) = key {
                let part = trp_data::arms::load(&mut assets, key).unwrap_or_else(|e| panic!("{}: its {what} {key}: {e:#}", definition.name));
                let mesh = read_mesh(&assets, part.mesh).expect("its mesh");
                println!("    the install's own {what}: {} vertices on {} bones", mesh.positions.len(), part.bone_paths.len());
            }
        }
    }
}

/// A weapon of one's own's pictures, compressed to be drawn with
/// (`custom::Picture::compressed`): how far each channel of the largest
/// level comes back from what was read, as a root mean square in 255 and
/// the share of pixels more than eight off. Passes with a note where no
/// weapon of one's own has its files here.
#[test]
fn a_weapon_of_one_s_own_s_pictures_compress_closely() {
    use trp_data::compress::{bc1_decoded, bc5_decoded};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let mut checked = 0;
    for template in db.custom.keys() {
        let Ok(weapon) = assemble(&mut assets, &db, template) else { continue };
        let Some(build) = weapon.custom.as_ref() else { continue };
        for material in &build.materials {
            let Ok(read) = trp_data::custom::pictures(material) else {
                eprintln!("skipped: the pictures of {} are not here", material.name);
                continue;
            };
            for (what, picture) in [("colour", read.colour), ("normal", read.normal), ("surface", read.surface)] {
                let Some(picture) = picture else { continue };
                let (width, height, channels) = (picture.width, picture.height, picture.channels);
                let first = width as usize * height as usize * channels;
                let original = picture.data[..first].to_vec();
                let packed = picture.compressed();
                let blocks = &packed.data[..trp_data::compress::level_bytes(width, height, if channels == 4 { 8 } else { 16 })];
                let back = if channels == 4 { bc1_decoded(width, height, blocks) } else { bc5_decoded(width, height, blocks) };
                let kept = if channels == 4 { 3 } else { 2 };
                let mut line = format!("{} {what} {width}x{height}:", material.name);
                for channel in 0..kept {
                    let (mut sum, mut off) = (0.0f64, 0usize);
                    for pixel in 0..(width * height) as usize {
                        let d = original[pixel * channels + channel] as f64 - back[pixel * channels + channel] as f64;
                        sum += d * d;
                        off += (d.abs() > 8.0) as usize;
                    }
                    let rms = (sum / (width * height) as f64).sqrt();
                    line += &format!(" channel {channel} {rms:.2} ({:.1}% over 8)", off as f64 * 100.0 / (width * height) as f64);
                    assert!(rms < 6.0, "{line}");
                }
                println!("{line}");
                checked += 1;
            }
        }
    }
    println!("{checked} pictures checked");
}

/// The inventory's health panel as the install's scene has it
/// (`docs/spec/health.md`, "The HEALTH tab"): its seven views and their
/// pieces named by the panel's own scripts, its figures, the left panel's
/// size on a screen 1920 by 1080, and its pictures read through trees
/// borrowed from a bundle; the views' glow hidden as their animator
/// stands, and the arrows' bob, by the scene's own clips.
#[test]
fn the_health_panel_is_read_as_the_scene_has_it() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let gear = trp_data::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::screen::HealthPanel::load(&install, &gear.file).expect("the health panel");
    let views = &health.views;
    assert_eq!(views.parts.len(), 7);
    let names: Vec<&str> = views.parts.iter().map(|view| health.panel.nodes[view.view].name.as_str()).collect();
    assert_eq!(names, ["Head", "Chest", "Stomach", "Left Arm", "Right Arm", "Left Leg", "Right Leg"]);
    for view in &views.parts {
        for piece in [view.value, view.bar, view.border, view.shape, view.broken, view.effects] {
            assert!(piece.is_some(), "{}: a piece of its view is not named", health.panel.nodes[view.view].name);
        }
    }
    for name in ["OverallHealth", "Poisoning", "Radiation", "BloodPressure", "Energy", "Hydration", "Temperature"] {
        assert!(views.parameter(name).and_then(|figure| figure.current).is_some(), "{name}");
    }
    assert!(views.parameter("Weight").is_none(), "the health tab has no weight");
    // The left side's width (a share of the screen and a size) shared by
    // its two children, with the space between them.
    let left = health.left_panel([1920.0, 1080.0]);
    assert!((left[0] - 604.163).abs() < 0.01 && (left[1] - 902.0).abs() < 0.01, "{left:?}");
    let [x, y, w, h] = health.gear_in(left);
    assert!((x - (left[0] - 494.0) / 2.0).abs() < 0.01 && (y - 100.0).abs() < 0.01 && w == 494.0 && h == 650.0, "{:?}", [x, y, w, h]);
    // The chest's view where its anchors and offset put it.
    let chest = views.parts[1].view;
    let [cx, cy, cw, ch] = health.panel.rect(chest, left);
    assert!((cx - (left[0] * 0.5 - 50.9)).abs() < 0.01 && (cy - (902.0 - (0.887_820_5 * 902.0 - 162.948_12))).abs() < 0.01 && cw == 130.0 && ch == 19.0, "{:?}", [cx, cy, cw, ch]);
    // The silhouette's picture and its texture.
    let mut pictures = trp_data::screen::ScenePictures::new(&install).expect("the trees to read pictures by");
    let silhouette = health.panel.find("Silhouette/Character Silhouette").expect("the silhouette");
    let (file, id) = health.panel.nodes[silhouette].image.and_then(|image| image.sprite).expect("its sprite");
    let name = health.panel.externals[file as usize - 1].clone();
    let cut = pictures.sprite(&name, id).expect("the sprite");
    let texture = pictures.texture(&cut.texture.0, cut.texture.1).expect("its texture");
    assert!(texture.width >= cut.rect[2] as u32 && texture.height >= cut.rect[3] as u32 && !texture.data.is_empty(), "{cut:?}");
    let covers = cut.covers();
    assert!(covers.iter().all(|c| (0.0..=1.0).contains(c)) && covers[0] < covers[2] && covers[1] < covers[3], "{covers:?}");
    // The scene's clips: the views' glow standing, and the arrows' bob.
    let shared: Vec<String> = health.panel.externals.iter().filter(|name| name.starts_with("sharedassets")).cloned().collect();
    let clips = trp_data::screen::scene_clips(&install, &mut pictures, &shared).expect("the scene's clips");
    let bound = |clip: &Clip, path: &str, property: &str| clip.others.iter().find(|b| b.path == path_hash(path) && b.attribute == path_hash(property)).map(|b| b.curve);
    let standing = clips.iter().find(|(_, _, clip)| clip.name == "Normal" && bound(clip, "BackGlowImage", "m_Color.a").is_some()).expect("the views' standing clip");
    let mut values = Vec::new();
    standing.2.sample(0.0, &mut values);
    assert_eq!(values[bound(&standing.2, "BackGlowImage", "m_Color.a").unwrap()], 0.0, "the glow is hidden standing");
    let bob = clips.iter().find(|(_, _, clip)| clip.name == "BuffPaneActive").expect("the arrows' bob");
    assert!(bob.2.looping && bound(&bob.2, "", "m_AnchoredPosition.y").is_some(), "{}", bob.2.name);
}

/// The GEAR tab's strip of figures as its row lays it out while the game
/// runs (`HorizontalLayoutGroup`: four panels asking 208 each in a strip
/// of 532.08 on a 16:9 screen, so 133.02 each from 36 units in), what its
/// script names, and the game's own fonts kept whole in `resources.assets`
/// (the panel's texts drawn in Bender).
#[test]
fn the_gear_tab_s_strip_and_the_game_s_fonts_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let gear = trp_data::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::screen::HealthPanel::load(&install, &gear.file).expect("the health panel");
    let strip = health.strip.as_ref().expect("the strip of figures");
    let left = health.left_panel([1920.0, 1080.0]);
    let names = ["Weight", "Overall Health", "Hydration", "Energy"];
    for (n, name) in names.iter().enumerate() {
        let index = strip.find(name).unwrap_or_else(|| panic!("{name}"));
        let [x, y, w, h] = strip.placed_in(index, left).rect;
        let width = (0.620_111_8 * left[0] + 157.430_18) / 4.0;
        assert!((w - width).abs() < 0.05 && (x - (left[0] * 0.189_944_1 - 157.430_18 / 2.0 + n as f32 * width)).abs() < 0.05, "{name}: {:?}", [x, y, w, h]);
        assert!((y - (902.0 - 20.6 - 46.8)).abs() < 0.05 && (h - 46.8).abs() < 0.01, "{name}: {:?}", [x, y, w, h]);
    }
    for name in ["Weight", "OverallHealth", "Hydration", "Energy"] {
        assert!(trp_data::screen::figure_named(&health.strip_figures, name).and_then(|f| f.current).is_some(), "{name}");
    }
    let weight = trp_data::screen::figure_named(&health.strip_figures, "Weight").unwrap();
    assert!(weight.alternative[3] > 0.0, "the weight has a colour of its own between its limits");
    // The fonts: Bender kept whole, and the panel's font assets made from it.
    let fonts = trp_data::screen::build_fonts(&install, "resources.assets").expect("the build's fonts");
    let bender = fonts.iter().find(|(name, _)| name == "Jovanny Lemonad - Bender").expect("Bender");
    assert_eq!(&bender.1[..4], b"OTTO");
    let header = health.panel.find("Chest/Header/Text").and_then(|text| health.panel.nodes[text].text.clone()).expect("a header's text");
    let file = health.panel.externals[header.font.0 as usize - 1].clone();
    let asset = trp_data::screen::script_name(&install, &file, header.font.1).expect("its font asset's name");
    assert_eq!(trp_data::screen::font_for(&asset, &fonts).map(|(name, _)| name.as_str()), Some("Jovanny Lemonad - Bender"), "{asset}");
}

/// The game's GEAR tab as `TARKOV UI` draws it (`docs/spec/inventory.md`,
/// "The game's own look"): the equipment panel's eleven slot views named
/// by its script with their borders, fills and pictures; the views of a
/// thing in a slot and in a grid read from the build's Resources; the
/// containers' column with its row template, its grid view and its rows'
/// spacing; a container's own layout of its grids; the slots' pictures;
/// and the database's colours and layouts of the items.
#[test]
fn the_game_s_gear_tab_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let gear = trp_data::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::screen::HealthPanel::load(&install, &gear.file).expect("the left panel");
    assert_eq!(health.slot_views.len(), 11, "{:?}", health.slot_views.iter().map(|(slot, _)| *slot).collect::<Vec<_>>());
    for (slot, parts) in &health.slot_views {
        for part in [parts.full_border, parts.back_image, parts.background, parts.empty_border, parts.header] {
            assert!(part.is_some(), "{slot}: a part of its view is not named");
        }
    }
    let gear_panel = health.gear_panel.as_ref().expect("the equipment panel's objects");
    assert!(gear_panel.find("CharacterGear").is_some(), "the character's outline behind the slots");
    // The views of a thing.
    let items = trp_data::screen::ItemViews::load(&install).expect("the views of a thing");
    let slot = items.slot.as_ref().expect("a slot's view");
    let grid = items.grid.as_ref().expect("a grid's view");
    assert!(slot.find("Info Panel/Name").is_some() && grid.find("Info Panel/Caption").is_some());
    let caption = grid.find("Info Panel/Caption").and_then(|index| grid.nodes[index].text.clone()).expect("the caption's text");
    assert_eq!(caption.size, 11.0);
    // The containers' column.
    let containers = trp_data::screen::ContainerViews::load(&install, &gear.file).expect("the containers' column");
    let column = containers.column.expect("its column of rows");
    assert!(column.vertical && column.spacing == 45.0 && column.padding[2] == 120.0, "{column:?}");
    assert!(containers.header.is_some() && containers.slot_panel.is_some());
    let row = containers.grids_row.expect("the default row of grids");
    assert_eq!(row.spacing, 6.0);
    let view = containers.grid.as_ref().expect("a grid's view");
    assert!(view.find("Background Tile").is_some() && view.find("Border").is_some());
    // A container's own layout: the AVS's twelve grids in their order.
    let (_, grids) = trp_data::screen::rig_layout(&install, "avs").expect("the AVS's layout");
    assert_eq!(grids.len(), 12);
    let layouts = trp_data::screen::rig_layouts(&install, &["avs".to_owned()]);
    assert_eq!(layouts["avs"].1[0], [0.0, -1.0, 64.0, 127.0]);
    assert!(trp_data::screen::resource_sprite(&install, "Slots/Equipment/TacticalVest").is_some());
    // The database's colours and layouts.
    let db = Database::load(&install).expect("database");
    assert!(db.items.values().filter(|item| item.background_color() != "default").count() > 100);
    assert!(db.items.values().filter(|item| !item.grid_layout_name().is_empty()).count() > 20);
    // The column's scroll view and its bar, by the pointers of their
    // scripts: a notch of the wheel 63 units, stopped at its ends, the bar
    // hidden while everything shows, from the bottom up, its handle tinted.
    let scrolling = containers.scrolling.expect("the column's scroll view");
    assert!(scrolling.vertical && !scrolling.across && scrolling.clamped && scrolling.hides && scrolling.sensitivity == 63.0, "{scrolling:?}");
    let at = |index: Option<usize>| index.map(|index| containers.panel.nodes[index].path.as_str());
    assert_eq!(at(scrolling.view), Some("Scrollview Parent/Containers Scrollview"));
    assert_eq!(at(scrolling.content), Some("Scrollview Parent/Containers Scrollview/Content"));
    let bar = scrolling.bar.expect("its bar");
    assert!(bar.bottom_up && bar.tinted == bar.handle, "{bar:?}");
    assert_eq!(at(bar.handle), Some("Scrollbar/Sliding Area/Handle"));
    let tints = bar.tints.expect("the bar's tints");
    assert!((tints.pressed[0] - 0.784).abs() < 1e-3 && (tints.highlighted[0] - 0.961).abs() < 1e-3 && (tints.fade - 0.1).abs() < 1e-6, "{tints:?}");
    // The handle up and down its sliding area, 10 in from the bar's ends,
    // 8 longer at each end than its share, and 6 wide.
    let handle = bar.handle_in(&containers.panel, [604.2, 902.0], 1.0, 0.5).expect("the handle's place");
    let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.05);
    assert!(near(handle, [596.2, 2.0, 6.0, 457.0]), "{handle:?}");
    // The screen's places at 1920 by 1080: the left side, the right's
    // panel, the left panel's ground and the left side's frame, whose
    // texture is sampled at its nearest pixel (its middle is clear).
    let screen = [1920.0, 1080.0];
    assert!(near(health.left_side_in(screen), [11.6, 48.0, 1213.3, 902.0]), "{:?}", health.left_side_in(screen));
    assert!(near(health.stash_in(screen).expect("the right's panel"), [1228.0, 75.0, 680.0, 873.0]), "{:?}", health.stash_in(screen));
    assert!(health.left_ground.is_some_and(|ground| (ground[3] - 0.498).abs() < 1e-3));
    let border = health.border.as_ref().expect("the left side's frame");
    let (outside, sprite) = border.nodes[0].image.and_then(|image| image.sprite).expect("the frame's sprite");
    let mut pictures = trp_data::screen::ScenePictures::new(&install).expect("the screens' pictures");
    let cut = pictures.sprite(&border.file_of(outside).expect("its file"), sprite).expect("the frame's sprite");
    assert!(pictures.texture(&cut.texture.0, cut.texture.1).expect("its texture").point);
    // The bar of tabs: eight tabs in the game's order, 26 over one another,
    // each its words' sample and 85 more; the widest is ACHIEVEMENTS.
    let mut bar = trp_data::screen::TabBar::load(&install, &gear.file).expect("the bar of tabs");
    assert_eq!(bar.gap, -26.0);
    let keys: Vec<&str> = bar.tabs.iter().map(|tab| tab.key.as_str()).collect();
    assert_eq!(keys, ["OVERALL", "GEAR", "HEALTH_TAB", "SKILLS", "MAP", "TASKS", "ACHIEVEMENTS", "Prestige"]);
    assert!(bar.tabs.iter().enumerate().all(|(order, tab)| tab.order == order as i32 && tab.width_adjustment == 85.0 && tab.normal.is_some() && tab.selected.is_some() && tab.hover.is_some()));
    // TextMeshPro's width of a text: the Bender asset's advances at its
    // sampling size of 48 and 2 hundredths of the size after every letter
    // but the last, as the scene keeps the tabs' samples (65.59).
    let sample = bar.panel.nodes[bar.tabs[0].size_definers[0]].text.clone().expect("the tab's sample");
    let font = trp_data::screen::TmpFont::read(&install, &bar.panel.file_of(sample.font.0).expect("its font's file"), sample.font.1).expect("Bender's asset");
    assert_eq!((font.point_size, font.normal_spacing), (48.0, 2.0));
    assert!(font.bold.is_some(), "its bold typeface");
    assert_eq!(font.width("OVERALL", 16.0, 0.0, false), 65.59);
    let texts = trp_data::inventory::texts(&install, "en", &keys).expect("the install's words");
    let say = |key: &str| texts.get(&key.to_lowercase()).cloned();
    let width = bar.width(&install, &say);
    assert!((width - (font.width("ACHIEVEMENTS", 16.0, 0.0, false) + 85.0)).abs() < 1e-3, "{width}");
    bar.lay_out(width);
    let places: Vec<f32> = bar.tabs.iter().map(|tab| bar.panel.placed_in(tab.object, screen).rect[0]).collect();
    assert!(places.iter().enumerate().all(|(order, x)| (x - (10.0 + order as f32 * (width - 26.0))).abs() < 1e-3), "{places:?}");
    // A line's height as TextMeshPro has it: Bender's ascent to descent at
    // 16, 17.61, which with its padding (4 and 6) is the 27.61 the scene
    // keeps for the corpse panel's box of a container's name.
    assert_eq!(font.height(16.0), 17.61);
    // The panel for a container opened: what its script names, its 18
    // filters in a column 34 apart (a vertical group laid out), its scroll
    // view elastic with no elasticity (back at once) and its bar.
    let mut stash = trp_data::screen::StashViews::load(&install, &gear.file).expect("the panel for a container opened");
    let path = |index: Option<usize>| index.map(|index| stash.panel.nodes[index].path.clone());
    assert_eq!(path(stash.name).as_deref(), Some("Simple Panel/Container Name/Text"));
    assert_eq!(path(stash.grids).as_deref(), Some("Simple Panel/Mask/Scroll Area/Content"));
    assert_eq!(stash.filter_tabs.len(), 18);
    let size = [680.0, 873.0];
    let tops: Vec<f32> = stash.filter_tabs.iter().filter_map(|tab| tab.normal).map(|normal| stash.panel.rect(normal, size)[1]).collect();
    assert!(tops.windows(2).all(|pair| (pair[1] - pair[0] - 34.0).abs() < 1e-3), "{tops:?}");
    let scrolling = stash.scrolling.expect("its scrolling");
    assert!(scrolling.elastic && scrolling.elasticity == 0.0 && scrolling.sensitivity == 63.0 && scrolling.bar.is_some_and(|bar| bar.bottom_up), "{scrolling:?}");
    // The quick-use panel: twelve views in the game's order of their
    // bindings, each with what its script names, 70 apart and 10 more
    // between the two groups, the panel where the game's screen has it.
    let mut quick = trp_data::screen::QuickViews::load(&install, &gear.file).expect("the quick-use panel");
    let keys: Vec<&str> = quick.slots.iter().map(|slot| slot.key.as_str()).collect();
    assert_eq!(keys, ["Grenade", "ItemV", "Item1", "Item2", "Item3", "Item4", "Item5", "Item6", "Item7", "Item8", "Item9", "Item10"]);
    assert!(quick.slots.iter().all(|slot| slot.install.is_some() && slot.hot_key.is_some() && slot.caption.is_some() && slot.border.is_some()));
    for slot in quick.slots.clone() {
        quick.panel.nodes[slot.object].active = true;
    }
    let place = quick.quick_in(screen);
    assert!(near(place, [558.5, 935.0, 854.2, 115.0]), "{place:?}");
    let lefts: Vec<f32> = quick.slots.iter().map(|slot| quick.panel.rect(slot.object, [place[2], place[3]])[0]).collect();
    let expected = [0.0, 70.0, 140.0, 210.0, 280.0, 354.0, 424.0, 494.0, 564.0, 634.0, 704.0, 774.0];
    assert!(lefts.iter().zip(expected).all(|(left, wanted)| (left - wanted).abs() < 1e-3), "{lefts:?}");
    // Its header on, its row sized to it (`ContentSizeFitter`): above the
    // panel's top, as the game shows it.
    let header = stash.header.expect("its header");
    stash.panel.nodes[header].active = true;
    let [_, panel_top, ..] = stash.panel.rect(stash.simple, size);
    let [_, top, _, height] = stash.panel.rect(header, size);
    assert!(height == 20.0 && top + height <= panel_top, "{top} {height} {panel_top}");
}

/// The things' sounds kept as Vorbis (`sound.md`, "Vorbis"): each names a
/// setup header the game's engine holds, and decodes to its own count of
/// frames. And an AI-2's hands have their own sound player, whose sounds
/// its animator's marks name (`hands_sounds`).
#[test]
fn vorbis_clips_decode_and_an_item_s_hands_have_their_sounds() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let clips = trp_data::sound::item_vorbis(&install).expect("the things' Vorbis clips");
    assert!(clips.len() >= 10, "{} Vorbis clips", clips.len());
    let mut setups = trp_data::sound::VorbisSetups::new(&install);
    for (name, sample) in &clips {
        let setup = setups.get(sample.setup_crc).unwrap_or_else(|e| panic!("{name}: {e:#}")).to_vec();
        let samples = trp_data::sound::decode_vorbis(sample, &setup).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        assert_eq!(samples.len() as u64, sample.frames * sample.channels as u64, "{name}: frames decoded");
        assert!(samples.iter().any(|s| s.unsigned_abs() > 1000), "{name}: silent");
    }
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let path = db.items["5755356824597772cb798962"].props["UsePrefab"]["path"].as_str().expect("the AI-2's hands").to_owned();
    let rig = trp_data::weapon::assemble_prefab(&mut assets, &path).expect("the AI-2's rig");
    let sounds = trp_data::sound::hands_sounds(&assets, &rig, &mut setups, true).expect("read").expect("a sound player");
    assert_eq!(sounds.player, "BaseSoundPlayer");
    assert!(sounds.shots.is_none(), "an item's hands have no shots");
    for mark in ["Draw", "Open", "Take", "CapRemove", "Inject", "Discard", "Holster"] {
        let element = sounds.for_mark(mark).unwrap_or_else(|| panic!("no sound for the mark {mark}"));
        assert!(!element.clips.is_empty() && element.clips.iter().all(|clip| !clip.samples.is_empty()), "{mark}");
    }
    println!("{} Vorbis clips decoded; the AI-2's {}: {} sounds", clips.len(), sounds.player, sounds.additional.len());
}

/// A weapon's banks of shots (`WeaponSoundPlayer`): the AKM's fire a loop
/// of sixteen beats at its rate of 600 a minute and a tail, with silenced
/// banks; the SV-98's each shot whole; the MP-43's both barrels' own bank.
#[test]
fn a_weapon_s_shots_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut setups = trp_data::sound::VorbisSetups::new(&install);
    let mut assets = Assets::new(install).expect("manifest");
    let mut shots_of = |template: &str| {
        let weapon = trp_data::weapon::assemble(&mut assets, &db, template).expect("the weapon");
        let sounds = trp_data::sound::hands_sounds(&assets, &weapon, &mut setups, true).expect("read").expect("a sound player");
        assert_eq!(sounds.player, "WeaponSoundPlayer", "{template}");
        sounds.shots.expect("its shots")
    };
    let close = |shots: &trp_data::sound::ShotSounds, bank: &str| -> f32 {
        let bank = shots.bank(bank).unwrap_or_else(|| panic!("no {bank}"));
        let clip = shots.clips[bank.lists[0][0]].1.as_ref().expect("the close clip read");
        clip.samples.len() as f32 / clip.channels as f32 / clip.frequency as f32
    };
    let akm = shots_of("59d6088586f774275f37482f");
    assert!(!akm.non_auto);
    let body = akm.bank("Body").expect("the AKM's body");
    // Sixteen beats of 60 / 600 s.
    assert!((body.clip_length - 1.6).abs() < 1e-4, "{}", body.clip_length);
    assert!((close(&akm, "Body") - 1.6).abs() < 1e-3);
    assert!(close(&akm, "Tail") > 3.0);
    assert!(akm.bank("BodySilenced").is_some() && akm.bank("Doublet").is_none());
    assert_eq!(body.blend.as_deref(), Some(&[5.0, 30.0, 55.0, 100.0][..]));
    let sv98 = shots_of("55801eed4bdc2d89578b4588");
    assert!(sv98.non_auto && sv98.bank("Tail").is_none());
    let mp43 = shots_of("5580223e4bdc2d1c128b457f");
    assert!(mp43.non_auto);
    assert_eq!(mp43.bank("Doublet").expect("both barrels").lists[0].len(), 2);
    let silenced = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM").silenced;
    assert!(!silenced, "the stock AKM has no silencer");
}
