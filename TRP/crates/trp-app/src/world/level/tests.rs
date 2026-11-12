//! Tests of the range written as a file for Blender and read back.
use super::*;

#[test]
fn names_are_matched_without_a_copys_number() {
    assert_eq!(clean("Prop Rifles Table.003"), "prop rifles table");
    assert_eq!(clean("lamp 4"), "lamp 4");
    assert_eq!(clean("v1.5"), "v1");
    assert_eq!(surface_named("Metal 6b6b6b"), 1);
    assert_eq!(surface_named("woodthick_old"), 12);
    assert_eq!(surface_named("Material.002"), trp_sim::sound::steps::CONCRETE);
}

#[test]
fn base64_is_the_standard_one() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}

/// A box Blender turned about its up comes back turned, its size its
/// own; one tilted comes back as the upright box about it, with a
/// warning; a copy's name is matched without its number, and a box
/// under a group keeps the group's move.
#[test]
fn a_turned_box_and_a_tilted_one_are_read() {
    let turn = Quat::from_rotation_y(30f32.to_radians()).to_array();
    let tilt = Quat::from_rotation_x(30f32.to_radians()).to_array();
    let json = serde_json::json!({
        "scene": 0,
        "scenes": [{ "nodes": [0, 3] }],
        "nodes": [
            { "name": "walls", "translation": [1.0, 0.0, 0.0], "children": [1, 2] },
            { "name": "Wall.001", "mesh": 0, "translation": [0.0, 1.0, 0.0], "rotation": turn, "scale": [2.0, 2.0, 4.0] },
            { "name": "ramp", "mesh": 0, "rotation": tilt },
            { "name": "Lamp.002", "translation": [0.0, 7.8, 0.0] },
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "material": 0 }] }],
        "accessors": [{ "min": [-0.5, -0.5, -0.5], "max": [0.5, 0.5, 0.5] }],
        "materials": [{ "name": "Metal rusty", "pbrMetallicRoughness": { "baseColorFactor": [0.5, 0.25, 0.125, 1.0] } }],
    });
    let mut warnings = Vec::new();
    let level = Level::of_gltf(&json, &mut |line| warnings.push(line));
    assert_eq!(level.boxes.len(), 2);
    let wall = &level.boxes[0];
    assert!(wall.centre.distance(Vec3::new(1.0, 1.0, 0.0)) < 1e-5 && wall.size.distance(Vec3::new(2.0, 2.0, 4.0)) < 1e-5, "{wall:?}");
    assert!((wall.yaw - 30f32.to_radians()).abs() < 1e-5 && wall.surface == 1 && wall.color == Color::linear_rgba(0.5, 0.25, 0.125, 1.0));
    let ramp = &level.boxes[1];
    let (sin, cos) = 30f32.to_radians().sin_cos();
    assert!(ramp.yaw == 0.0 && ramp.size.distance(Vec3::new(1.0, cos + sin, sin + cos)) < 1e-5, "{ramp:?}");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(level.lamps, vec![Vec3::new(0.0, 7.8, 0.0)]);
}


/// A start is read by its arrow, moved and turned alone as by the
/// empty: the feet at the arrow's tail, the view along it; without an
/// arrow, by the empty. A box with no material is the range's grey.
#[test]
fn a_start_goes_by_its_arrow() {
    let quarter = Quat::from_rotation_y(-90f32.to_radians()).to_array();
    let json = serde_json::json!({
        "scenes": [{ "nodes": [0, 2, 3] }],
        "nodes": [
            { "name": "start surfaces", "translation": [1.0, 0.0, 1.0], "children": [1] },
            { "name": "start surfaces arrow", "mesh": 0, "translation": [20.0, 0.025, -3.0], "rotation": quarter, "scale": [0.15, 0.05, 0.8] },
            { "name": "start lanes", "translation": [3.0, 0.0, -9.0], "rotation": quarter },
            { "name": "block", "mesh": 1 },
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "material": 0 }] }, { "primitives": [{ "attributes": { "POSITION": 0 } }] }],
        "accessors": [{ "min": [-0.5, -0.5, -0.5], "max": [0.5, 0.5, 0.5] }],
        "materials": [{ "name": "start arrow" }],
    });
    let level = Level::of_gltf(&json, &mut |line| panic!("{line}"));
    // The arrow, a quarter turned to the right, about (21, -2): the
    // feet at its tail, 40 cm back, looking 90 degrees right.
    let start = level.starts["surfaces"];
    assert!(start.feet.xz().distance(Vec2::new(20.6, -2.0)) < 1e-5 && (start.yaw - 90.0).abs() < 1e-3, "{start:?}");
    let lanes = level.starts["lanes"];
    assert!(lanes.feet.distance(Vec3::new(3.0, 0.0, -9.0)) < 1e-5 && (lanes.yaw - 90.0).abs() < 1e-3, "{lanes:?}");
    assert_eq!(level.boxes[0].color, PLAIN);
}


/// Each face's triangles wind anticlockwise seen from outside.
#[test]
fn the_cube_faces_out() {
    let (positions, normals, indices) = unit_cube();
    for triangle in indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(positions[triangle[k] as usize]));
        let normal = Vec3::from(normals[triangle[0] as usize]);
        assert!((b - a).cross(c - a).dot(normal) > 0.0);
    }
}
