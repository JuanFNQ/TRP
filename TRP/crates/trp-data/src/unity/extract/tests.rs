//! Tests of the typed views: half floats, hierarchy paths and a part attached with its levels of detail.

use super::*;

#[test]
fn half_floats() {
    assert_eq!(half_to_f32(0x3c00), 1.0);
    assert_eq!(half_to_f32(0xc000), -2.0);
    assert_eq!(half_to_f32(0x0000), 0.0);
    assert_eq!(half_to_f32(0x0001), 2.0f32.powi(-24));
    assert_eq!(half_to_f32(0x7c00), f32::INFINITY);
}

fn node(name: &str, parent: Option<usize>, rotation: [f32; 4]) -> Node {
    Node {
        name: name.to_owned(),
        parent,
        translation: [0.0; 3],
        rotation,
        scale: [1.0; 3],
        active: true,
        renderer: None,
        scripts: Vec::new(),
        script_objects: Vec::new(),
        grip: None,
        placer: None,
        transform: ObjRef { file: 0, path_id: 0 },
        colliders: Vec::new(),
        bounded: None,
        lower: None,
        lod: None,
    }
}

#[test]
fn hierarchy_paths_search_and_attach() {
    let quarter_x = [-0.70710677, 0.0, 0.0, 0.70710677];
    let identity = [0.0, 0.0, 0.0, 1.0];
    let mut h = Hierarchy {
        nodes: vec![
            node("root", None, identity),
            node("Weapon_root", Some(0), quarter_x),
            node("weapon", Some(1), identity),
            node("mod_stock", Some(2), identity),
        ],
        ..Hierarchy::default()
    };
    assert_eq!(h.path(0), "");
    assert_eq!(h.path(3), "Weapon_root/weapon/mod_stock");
    assert_eq!(h.rotation_from_root(3), quarter_x);

    // A part with a slot of the same name: the search prefers the nearer one.
    let part = Hierarchy { nodes: vec![node("stock", None, identity), node("mod_stock", Some(0), identity)], ..Hierarchy::default() };
    let part_root = h.attach(3, part);
    assert_eq!(h.nodes[part_root].parent, Some(3));
    assert_eq!(h.find_below(0, "mod_stock"), Some(3));
    assert_eq!(h.find_below(part_root, "mod_stock"), Some(part_root + 1));
    assert_eq!(h.find_below(0, "missing"), None);
}

#[test]
fn a_part_attached_brings_its_levels_of_detail() {
    let identity = [0.0, 0.0, 0.0, 1.0];
    let group = std::sync::Arc::new(crate::drawing::lod::LodGroup { id: None, object: None, size: 0.5, reference: [0.0; 3], fade_mode: 0, animate_cross_fading: false, levels: Vec::new(), deviation: Vec::new() });
    let mut h = Hierarchy { nodes: vec![node("root", None, identity)], lods: vec![HierarchyLod { node: 0, group: group.clone() }] };
    let mut drawn = node("mesh_lod1", Some(0), identity);
    drawn.lod = Some(NodeLod { group: 0, levels: 0b10 });
    let part = Hierarchy { nodes: vec![node("part", None, identity), drawn], lods: vec![HierarchyLod { node: 0, group }] };
    let at = h.attach(0, part);
    assert_eq!(h.lods.len(), 2);
    assert_eq!(h.lods[1].node, at);
    assert_eq!(h.nodes[at + 1].lod, Some(NodeLod { group: 1, levels: 0b10 }));
}
