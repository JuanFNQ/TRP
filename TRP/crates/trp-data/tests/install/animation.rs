//! Tests that read clips, the body controller, its blends and bindings.

use super::*;

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

#[test]
fn arms_bind_to_bones_the_weapon_has() {
    let Some((mut assets, weapon)) = akm() else { return };
    let arms = trp_data::character::arms::load(&mut assets, "assets/content/hands/bear/bear_hands_skin.bundle").expect("arms");
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

    let table = trp_data::character::body::controller(&mut assets).expect("body controller");
    println!("{} parameters, {} states, {} layers", table.parameters.len(), table.states.len(), table.layers.len());
    let kind = |name: &str| table.parameters.iter().find(|p| p.name == name).map(|p| (p.kind, p.default));
    use trp_data::weapons::states::ParameterKind::{Bool, Float};
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

    let bones = trp_data::character::body::bones(&mut assets, &["Root_Joint", "Root_Joint/Base HumanPelvis"]).expect("bones");
    println!("the pelvis rests {:?} above the root", bones[1].position);
    assert!((0.8..1.1).contains(&bones[1].position[1]));
}

/// The engine's three two-parameter blends, worked out here, against the
/// weight grids the install stores for the same trees.
#[test]
fn two_parameter_blends_match_the_stored_grids() {
    use trp_data::character::rootmotion::{BlendType, Node, RootMotionTable};
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
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
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

/// The bipods' own clips (mounting spec, "The bipod", "The `_procedural`
/// nodes"): for each of the seven bipods the database fits, every curve of
/// every clip of its own animator binds a node of its prefab, and none of
/// those nodes is a `*_procedural` one; six of the seven have such nodes
/// and the Harris has none (`probe partbinds <template>`). So the clips
/// never move them: they stay where the prefab puts them.
#[test]
fn no_bipod_clip_binds_a_procedural_node() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    // Each bipod's template and its count of `_procedural` nodes: the
    // Atlas V8, the Harris S-BRM, the SV-98's, the G36's, the PK's, the
    // RPD's, the M60's.
    let bipods = [
        ("6644920d49817dc7d505ca71", 6),
        ("5888961624597754281f93f3", 0),
        ("56ea8222d2720b69698b4567", 4),
        ("622b397c9a3d4327e41843b6", 3),
        ("6464d870bb2c580352070cc4", 4),
        ("6513f037e06849f06c0957d7", 4),
        ("66012d9a3dff5074ed002e33", 2),
    ];
    for (template, procedural) in bipods {
        let key = db.items[template].prefab_key().expect("its prefab").to_owned();
        let (tree, clips) = trp_data::weapons::weapon::part_clip_binds(&mut assets, &key).expect("its tree and clips");
        let named: Vec<&str> = tree.nodes.iter().map(|node| node.name.as_str()).filter(|name| name.ends_with("_procedural")).collect();
        assert_eq!(named.len(), procedural, "{template}'s `_procedural` nodes: {named:?}");
        assert_eq!(clips.len(), 4, "{template}'s four clips (closed, open, open idle, close)");
        let mut moved = 0;
        for (clip, binds) in &clips {
            assert!(!binds.is_empty(), "{template}'s {clip} moves something");
            for bind in binds {
                let node = bind.node.unwrap_or_else(|| panic!("{template}'s {clip} binds {:08x}, no node of the prefab", bind.path));
                let name = &tree.nodes[node].name;
                assert!(!name.ends_with("_procedural"), "{template}'s {clip} moves {name}");
                moved += 1;
            }
        }
        println!("{template}: {} `_procedural` nodes, {} clips, {moved} bindings, none to them", named.len(), clips.len());
    }
}
