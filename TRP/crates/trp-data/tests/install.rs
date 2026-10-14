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
