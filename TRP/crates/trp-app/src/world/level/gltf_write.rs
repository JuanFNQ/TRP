//! The range's layout written out as glTF.

use super::*;

/// What `--export-level` writes, as the range gives it (`Range::layout`).
pub struct Layout {
    pub boxes: Vec<LevelBox>,
    pub props: Vec<PropLayout>,
    /// Each start, with the prop it goes with.
    pub starts: Vec<(String, Start, Option<String>)>,
    pub lamps: Vec<Vec3>,
    pub indoors: Option<LevelBox>,
}

/// A prop as it stands, and its parts drawn under it (in the world).
pub struct PropLayout {
    pub name: String,
    pub stand: Stand,
    pub parts: Vec<LevelBox>,
}

/// A cube a metre on a side about its middle, a face's four corners each
/// with the face's normal, and its two triangles each (anticlockwise seen
/// from outside).
pub(super) fn unit_cube() -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u16>) {
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for normal in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
        // Two ways across the face with `u × v` along the normal.
        let u = if normal.y.abs() > 0.5 { Vec3::X } else { Vec3::Y };
        let v = normal.cross(u);
        let first = positions.len() as u16;
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push((normal * 0.5 + u * a * 0.5 + v * b * 0.5).to_array());
            normals.push(normal.to_array());
        }
        // u × v is the normal's way when (u, v, normal) are right-handed.
        if u.cross(v).dot(normal) > 0.0 {
            indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        } else {
            indices.extend([first, first + 2, first + 1, first, first + 3, first + 2]);
        }
    }
    (positions, normals, indices)
}

pub(super) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A turn about the upright as glTF's quaternion.
fn turn_array(yaw: f32) -> [f32; 4] {
    Quat::from_rotation_y(yaw).to_array()
}

/// The layout as a glTF's JSON, its one buffer (the unit cube) in it.
pub fn to_gltf(layout: &Layout) -> serde_json::Value {
    use serde_json::json;
    let (positions, normals, indices) = unit_cube();
    let mut buffer: Vec<u8> = Vec::new();
    for p in positions.iter().chain(&normals) {
        for value in p {
            buffer.extend(value.to_le_bytes());
        }
    }
    for index in &indices {
        buffer.extend(index.to_le_bytes());
    }
    let vertex_bytes = positions.len() * 12;
    let mut materials: Vec<serde_json::Value> = Vec::new();
    let mut material_of: HashMap<(u8, [u8; 4]), usize> = HashMap::new();
    let mut material = |color: Color, surface: u8, named: Option<&str>, materials: &mut Vec<serde_json::Value>| -> usize {
        let srgb = color.to_srgba().to_u8_array();
        *material_of.entry((surface, srgb)).or_insert_with(|| {
            let linear = color.to_linear();
            let surface_name = trp_data::audio::sound::SURFACES.get(surface as usize).copied().unwrap_or("Concrete");
            let name = named.map_or_else(|| format!("{surface_name} {:02x}{:02x}{:02x}", srgb[0], srgb[1], srgb[2]), str::to_owned);
            let mut made = json!({ "name": name, "pbrMetallicRoughness": { "baseColorFactor": [linear.red, linear.green, linear.blue, linear.alpha], "metallicFactor": 0.0, "roughnessFactor": 0.95 } });
            if linear.alpha < 1.0 {
                made["alphaMode"] = json!("BLEND");
                made["doubleSided"] = json!(true);
            }
            materials.push(made);
            materials.len() - 1
        })
    };
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut meshes: Vec<serde_json::Value> = Vec::new();
    // A box's node under a parent standing at `parent` (its world place
    // and turn), with a mesh of its own (Blender gives each its own).
    let mut box_node = |name: &str, centre: Vec3, size: Vec3, yaw: f32, material: usize, parent: Stand, nodes: &mut Vec<serde_json::Value>| -> usize {
        meshes.push(json!({ "name": name, "primitives": [{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": 2, "material": material }] }));
        let local = Quat::from_rotation_y(-parent.yaw) * (centre - parent.at);
        nodes.push(json!({ "name": name, "mesh": meshes.len() - 1, "translation": local.to_array(), "rotation": turn_array(yaw - parent.yaw), "scale": size.to_array() }));
        nodes.len() - 1
    };
    let world = Stand::made(Vec3::ZERO);
    let group = |name: &str, children: Vec<usize>, nodes: &mut Vec<serde_json::Value>| -> usize {
        nodes.push(json!({ "name": name, "children": children }));
        nodes.len() - 1
    };
    let start_node = |name: &str, start: Start, parent: Stand, nodes: &mut Vec<serde_json::Value>, box_node: &mut dyn FnMut(&str, Vec3, Vec3, f32, usize, Stand, &mut Vec<serde_json::Value>) -> usize, arrow: usize| -> usize {
        // Its arrow, 80 cm long ahead of the feet, and its head across
        // its tip, under it.
        let turn = -start.yaw.to_radians();
        let ahead = Quat::from_rotation_y(turn) * Vec3::NEG_Z;
        let at = Stand { at: start.feet, yaw: turn };
        let shaft = box_node(&format!("start {name} arrow"), start.feet + ahead * 0.4 + Vec3::Y * 0.025, Vec3::new(0.15, 0.05, 0.8), turn, arrow, at, nodes);
        let head = box_node(&format!("start {name} arrow head"), start.feet + ahead * 0.75 + Vec3::Y * 0.025, Vec3::new(0.45, 0.05, 0.1), turn, arrow, at, nodes);
        let local = Quat::from_rotation_y(-parent.yaw) * (start.feet - parent.at);
        nodes.push(json!({ "name": format!("start {name}"), "translation": local.to_array(), "rotation": turn_array(turn - parent.yaw), "children": [shaft, head] }));
        nodes.len() - 1
    };
    let arrow = material(Color::srgb(0.95, 0.75, 0.1), trp_sim::sound::steps::CONCRETE, Some("start arrow"), &mut materials);
    let mut roots = Vec::new();
    // The ground, for the eye.
    let ground = material(Color::srgb(0.35, 0.35, 0.35), trp_sim::sound::steps::CONCRETE, Some("ground (not read)"), &mut materials);
    roots.push(box_node("ground (not read)", Vec3::new(0.0, -0.05, 0.0), Vec3::new(120.0, 0.1, 120.0), 0.0, ground, world, &mut nodes));
    // The boxes, numbered where two have one name.
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut numbered = |name: &str| -> String {
        let n = seen.entry(name.to_owned()).or_insert(0);
        *n += 1;
        if *n == 1 { name.to_owned() } else { format!("{name} {n}") }
    };
    let boxes: Vec<usize> = layout.boxes.iter().map(|b| {
        let m = material(b.color, b.surface, None, &mut materials);
        box_node(&numbered(&b.name), b.centre, b.size, b.yaw, m, world, &mut nodes)
    }).collect();
    roots.push(group("boxes", boxes, &mut nodes));
    let mut props = Vec::new();
    for prop in &layout.props {
        let mut children: Vec<usize> = prop.parts.iter().enumerate().map(|(n, part)| {
            let m = material(part.color, part.surface, None, &mut materials);
            // Named by the range where it says (a square of the sampler by
            // its surface), else numbered.
            let name = if part.name.is_empty() { format!("{} part {}", prop.name, n + 1) } else { part.name.clone() };
            box_node(&name, part.centre, part.size, part.yaw, m, prop.stand, &mut nodes)
        }).collect();
        for (name, start, _) in layout.starts.iter().filter(|(_, _, of)| of.as_deref() == Some(prop.name.as_str())) {
            children.push(start_node(name, *start, prop.stand, &mut nodes, &mut box_node, arrow));
        }
        nodes.push(json!({ "name": format!("prop {}", prop.name), "translation": prop.stand.at.to_array(), "rotation": turn_array(prop.stand.yaw), "children": children }));
        props.push(nodes.len() - 1);
    }
    roots.push(group("props", props, &mut nodes));
    let starts: Vec<usize> = layout.starts.iter().filter(|(_, _, of)| of.is_none()).map(|(name, start, _)| start_node(name, *start, world, &mut nodes, &mut box_node, arrow)).collect();
    roots.push(group("starts", starts, &mut nodes));
    let housing = material(Color::srgb(0.18, 0.18, 0.19), crate::world::range::METAL_SURFACE, Some("lamp"), &mut materials);
    let lamps: Vec<usize> = layout.lamps.iter().enumerate().map(|(n, at)| box_node(&format!("lamp {}", n + 1), *at, Vec3::new(0.7, 0.14, 0.7), 0.0, housing, world, &mut nodes)).collect();
    roots.push(group("lamps", lamps, &mut nodes));
    if let Some(indoors) = &layout.indoors {
        let glass = material(Color::srgba(0.3, 0.6, 1.0, 0.15), trp_sim::sound::steps::CONCRETE, Some("indoors (not drawn)"), &mut materials);
        roots.push(box_node("indoors", indoors.centre, indoors.size, indoors.yaw, glass, world, &mut nodes));
    }
    let count = positions.len();
    json!({
        "asset": { "version": "2.0", "generator": format!("TRP {}", env!("TRP_VERSION")) },
        "scene": 0,
        "scenes": [{ "name": "range", "nodes": roots }],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": count, "type": "VEC3", "min": [-0.5, -0.5, -0.5], "max": [0.5, 0.5, 0.5] },
            { "bufferView": 1, "componentType": 5126, "count": count, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5123, "count": indices.len(), "type": "SCALAR" },
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": vertex_bytes, "target": 34962 },
            { "buffer": 0, "byteOffset": vertex_bytes, "byteLength": vertex_bytes, "target": 34962 },
            { "buffer": 0, "byteOffset": vertex_bytes * 2, "byteLength": indices.len() * 2, "target": 34963 },
        ],
        "buffers": [{ "byteLength": buffer.len(), "uri": format!("data:application/octet-stream;base64,{}", base64(&buffer)) }],
    })
}

/// Writes the layout to a `.gltf`, its folder made if it is not there.
pub fn write(layout: &Layout, path: &Path) -> anyhow::Result<()> {
    if let Some(folder) = path.parent().filter(|folder| !folder.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&to_gltf(layout))?)?;
    Ok(())
}
