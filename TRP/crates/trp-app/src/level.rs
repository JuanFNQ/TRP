//! The range as a file to edit in Blender and read back (the user,
//! 2026-10-19: "Is it possible to provide me an FTB file that I can edit
//! inside blender so I can edit the level to how I want then give it
//! back"). The file is glTF 2.0, which Blender imports and exports as it
//! ships and Bevy reads; `--export-level` writes the range as it stands,
//! and `--level` reads one back at the start, to try an edit for a run
//! (`docs/manual.md`, "The range in Blender"); the range itself is the
//! code's (`range`), an edit taken into it as the user's of 2026-10-19 was
//! ("just replace our range with this one ... I don't want a level
//! toggle"). The app's own; nothing of the game's is in it.
//!
//! What the file holds, each by its name (Blender keeps an object's name,
//! adding `.001` to a copy's):
//!
//! - **boxes**: every wall, floor, step and block, each an upright box
//!   (a mesh's bounds, by its place, turn about the upright and size),
//!   its colour its material's and what it sounds of underfoot the first
//!   word of the material's name (one of the game's twenty surfaces,
//!   `trp_data::sound::SURFACES`; concrete if none). Moved, sized, turned,
//!   copied or deleted at will. A box named `roof` (or `roof ...`) is a
//!   roof: drawn and met by sight and blasts, but no part of the
//!   character's own collision, whose ground is the highest top over its
//!   feet (`range::Solid::overhead`).
//! - **`prop <name>`**: each thing the hands work with what it stands on
//!   (the doors, the crates, the tables with their cases and boxes, the
//!   storage, the car, the barbed wire): moved and turned as one, its
//!   parts drawn under it for the eye only. But the sampler of surfaces'
//!   parts, `sampler part <n>`: each the square of the game's `n`th
//!   surface, where it is and as large as it is drawn.
//! - **`start <name>`**: where `--start` puts the character and which way
//!   it looks, by its arrow (`start <name> arrow`, under it, from the feet
//!   ahead; moved or turned alone, it still says), or the empty's own
//!   place and turn where it has no arrow; a prop's own under it, to go
//!   with it.
//! - **`lamp <n>`**: the hall's ceiling lamps (`HALL LAMPS`).
//! - **`indoors`**: the box inside which the camera takes the level's
//!   indoor exposure and the sky's ambient is taken away (`lighting`,
//!   "The range hall"); none, and the range is all outdoors.
//! - **`ground`**: the ground, drawn for the eye; not read.

use bevy::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Whether a box of that name is a roof (`range::Solid::overhead`).
pub fn is_roof(name: &str) -> bool {
    let name = clean(name);
    name == "roof" || name.starts_with("roof ")
}

/// A box of the level's: upright, turned about the upright by `yaw`
/// (radians, the right hand's way about Bevy's up), in Bevy's axes.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelBox {
    pub name: String,
    pub centre: Vec3,
    pub size: Vec3,
    pub yaw: f32,
    pub color: Color,
    pub surface: u8,
}

/// Where a prop stands: its anchor's place and its turn about the upright
/// from the way it is made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stand {
    pub at: Vec3,
    pub yaw: f32,
}

impl Stand {
    /// As made: at its anchor, unturned.
    pub fn made(anchor: Vec3) -> Self {
        Self { at: anchor, yaw: 0.0 }
    }

    /// A place of the prop as made about `anchor`, where it now stands.
    pub fn point(&self, anchor: Vec3, p: Vec3) -> Vec3 {
        self.at + Quat::from_rotation_y(self.yaw) * (p - anchor)
    }

    /// A way of the prop as made, as it now stands.
    pub fn way(&self, v: Vec3) -> Vec3 {
        Quat::from_rotation_y(self.yaw) * v
    }

}

/// A start: where the feet are (Bevy's axes) and the way looked, in
/// degrees right of the lane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Start {
    pub feet: Vec3,
    pub yaw: f32,
}

/// The level as read from a file.
#[derive(Clone, Debug, Default)]
pub struct Level {
    pub boxes: Vec<LevelBox>,
    pub props: HashMap<String, Stand>,
    pub starts: HashMap<String, Start>,
    pub lamps: Vec<Vec3>,
    pub indoors: Option<LevelBox>,
    /// The sampler's squares, each by its surface's number, where the file
    /// has any (`sampler part <n>`).
    pub squares: Option<Vec<(u8, LevelBox)>>,
}

/// A box with no material: the range's concrete grey (glTF's own default
/// is white, which no one means by leaving a cube as it comes).
const PLAIN: Color = Color::srgb(0.55, 0.55, 0.55);

static LEVEL: OnceLock<Level> = OnceLock::new();

/// The level read at the start, for the rest of the run.
pub fn set(level: Level) {
    let _ = LEVEL.set(level);
}

/// The level read at the start, if one was.
pub fn get() -> Option<&'static Level> {
    LEVEL.get()
}


/// A node's name as matched: small letters, without Blender's `.001` for a
/// copy.
fn clean(name: &str) -> String {
    let name = name.trim().to_lowercase();
    match name.rsplit_once('.') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => head.trim().to_owned(),
        _ => name,
    }
}

/// What a material's name says a box sounds of: its first word, one of the
/// game's surfaces (concrete if none is).
fn surface_named(name: &str) -> u8 {
    let first = name.split(|c: char| c.is_whitespace() || c == '_' || c == '-').next().unwrap_or("");
    trp_data::sound::SURFACES.iter().position(|surface| surface.eq_ignore_ascii_case(first)).map_or(trp_sim::steps::CONCRETE, |at| at as u8)
}

/// The turn about the upright that takes x to `x` (level part).
fn yaw_of(x: Vec3) -> f32 {
    (-x.z).atan2(x.x)
}

fn floats<const N: usize>(value: Option<&serde_json::Value>) -> Option<[f32; N]> {
    let list = value?.as_array()?;
    (list.len() == N).then(|| std::array::from_fn(|k| list[k].as_f64().unwrap_or(0.0) as f32))
}

/// A node's own matrix: its `matrix`, or its place, turn and scale.
fn node_matrix(node: &serde_json::Value) -> Mat4 {
    if let Some(m) = floats::<16>(node.get("matrix")) {
        return Mat4::from_cols_array(&m);
    }
    let t = floats::<3>(node.get("translation")).map_or(Vec3::ZERO, Vec3::from);
    let r = floats::<4>(node.get("rotation")).map_or(Quat::IDENTITY, Quat::from_array);
    let s = floats::<3>(node.get("scale")).map_or(Vec3::ONE, Vec3::from);
    Mat4::from_scale_rotation_translation(s, r.normalize(), t)
}

/// The JSON of a `.glb` (its first chunk) or a `.gltf`.
fn gltf_json(bytes: &[u8]) -> anyhow::Result<serde_json::Value> {
    if !bytes.starts_with(b"glTF") {
        return Ok(serde_json::from_slice(bytes)?);
    }
    let word = |at: usize| -> anyhow::Result<u32> { Ok(u32::from_le_bytes(bytes.get(at..at + 4).ok_or_else(|| anyhow::anyhow!("a .glb cut short"))?.try_into()?)) };
    let (length, kind) = (word(12)? as usize, word(16)?);
    anyhow::ensure!(kind == 0x4E4F_534A, "a .glb whose first chunk is not its JSON");
    let json = bytes.get(20..20 + length).ok_or_else(|| anyhow::anyhow!("a .glb cut short"))?;
    Ok(serde_json::from_slice(json)?)
}

impl Level {
    /// Reads a level from a `.glb` or a `.gltf`; what it cannot take is
    /// said, and left out.
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(Self::of_gltf(&gltf_json(&bytes)?, &mut |line| eprintln!("level file: {line}")))
    }

    /// The level a glTF's JSON holds (only the JSON is read: a mesh's
    /// bounds are its positions' `min` and `max`, which glTF requires).
    pub fn of_gltf(json: &serde_json::Value, warn: &mut dyn FnMut(String)) -> Self {
        let empty = Vec::new();
        let list = |key: &str| json.get(key).and_then(|v| v.as_array()).unwrap_or(&empty);
        let (nodes, meshes, accessors, materials) = (list("nodes"), list("meshes"), list("accessors"), list("materials"));
        let scene = json.get("scene").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let roots: Vec<usize> = match list("scenes").get(scene).and_then(|s| s.get("nodes")).and_then(|v| v.as_array()) {
            Some(roots) => roots.iter().filter_map(|v| v.as_u64()).map(|v| v as usize).collect(),
            None => {
                // No scene: every node no other has as a child.
                let children: Vec<usize> = nodes.iter().flat_map(|n| n.get("children").and_then(|c| c.as_array()).into_iter().flatten()).filter_map(|v| v.as_u64()).map(|v| v as usize).collect();
                (0..nodes.len()).filter(|n| !children.contains(n)).collect()
            }
        };
        let mut level = Level::default();
        let mut stack: Vec<(usize, Mat4)> = roots.into_iter().rev().map(|n| (n, Mat4::IDENTITY)).collect();
        while let Some((index, parent)) = stack.pop() {
            let Some(node) = nodes.get(index) else { continue };
            let world = parent * node_matrix(node);
            let raw = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let name = clean(raw);
            let (place, x_way) = (world.transform_point3(Vec3::ZERO), world.transform_vector3(Vec3::X));
            let children = || node.get("children").and_then(|c| c.as_array()).into_iter().flatten().filter_map(|v| v.as_u64()).map(|v| v as usize);
            let child_named = |child: usize| nodes.get(child).and_then(|n| n.get("name")).and_then(|v| v.as_str()).map(clean).unwrap_or_default();
            if let Some(start) = name.strip_prefix("start ") {
                let start = start.trim();
                // By its arrow where it has one: the arrow runs from the
                // feet ahead, along its own -z.
                let arrow = children().find(|&child| child_named(child) == format!("start {start} arrow"));
                let found = arrow.and_then(|child| nodes.get(child)).map(|node| world * node_matrix(node)).map(|arrow| {
                    let (middle, along) = (arrow.transform_point3(Vec3::ZERO), arrow.transform_vector3(Vec3::NEG_Z));
                    let ahead = Vec3::new(along.x, 0.0, along.z).normalize_or(Vec3::NEG_Z);
                    (middle - ahead * along.length() / 2.0, ahead)
                });
                let (feet, ahead) = found.unwrap_or((place, world.transform_vector3(Vec3::NEG_Z)));
                level.starts.insert(start.to_owned(), Start { feet: Vec3::new(feet.x, place.y, feet.z), yaw: ahead.x.atan2(-ahead.z).to_degrees() });
                continue;
            }
            if let Some(prop) = name.strip_prefix("prop ") {
                if level.props.insert(prop.trim().to_owned(), Stand { at: place, yaw: yaw_of(x_way) }).is_some() {
                    warn(format!("{raw}: a second of that prop, taken in place of the first"));
                }
                for child in children() {
                    let named = child_named(child);
                    // Its starts go with it.
                    if named.starts_with("start ") {
                        stack.push((child, world));
                    }
                    // The sampler's squares, each where it is.
                    let square = named.strip_prefix("sampler part ").and_then(|n| n.trim().parse::<usize>().ok()).filter(|n| (1..=trp_data::sound::SURFACES.len()).contains(n));
                    let Some(n) = square else { continue };
                    let Some(node) = nodes.get(child) else { continue };
                    let Some(mesh) = node.get("mesh").and_then(|v| v.as_u64()) else { continue };
                    if let Some((mut found, _)) = mesh_box(mesh as usize, meshes, accessors, materials, world * node_matrix(node)) {
                        found.name = named.clone();
                        level.squares.get_or_insert_with(Vec::new).push(((n - 1) as u8, found));
                    }
                }
                continue;
            }
            if name == "lamp" || name.starts_with("lamp ") {
                level.lamps.push(place);
                continue;
            }
            if name.starts_with("ground") {
                continue;
            }
            let indoors = name.starts_with("indoors");
            if let Some(mesh) = node.get("mesh").and_then(|v| v.as_u64()) {
                match mesh_box(mesh as usize, meshes, accessors, materials, world) {
                    Some((mut found, tilted)) => {
                        if tilted {
                            warn(format!("{raw} is tilted: taken as the upright box about it"));
                        }
                        found.name = raw.to_owned();
                        if indoors {
                            level.indoors = Some(found);
                            continue;
                        }
                        level.boxes.push(found);
                    }
                    None => warn(format!("{raw}: its mesh has no positions' bounds, left out")),
                }
            } else if indoors {
                warn(format!("{raw} has no mesh: no indoors"));
            }
            for child in children().collect::<Vec<_>>().into_iter().rev() {
                stack.push((child, world));
            }
        }
        level
    }
}

/// A mesh's box in the world (an upright box about its positions' bounds
/// by the node's matrix), with its first material's colour and surface;
/// and whether it was tilted, and so taken as the upright box about it.
fn mesh_box(mesh: usize, meshes: &[serde_json::Value], accessors: &[serde_json::Value], materials: &[serde_json::Value], world: Mat4) -> Option<(LevelBox, bool)> {
    let primitives = meshes.get(mesh)?.get("primitives")?.as_array()?;
    let (mut low, mut high) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for primitive in primitives {
        let accessor = accessors.get(primitive.get("attributes")?.get("POSITION")?.as_u64()? as usize)?;
        low = low.min(Vec3::from(floats::<3>(accessor.get("min"))?));
        high = high.max(Vec3::from(floats::<3>(accessor.get("max"))?));
    }
    if !low.is_finite() {
        return None;
    }
    let material = primitives.first().and_then(|p| p.get("material")).and_then(|v| v.as_u64()).and_then(|m| materials.get(m as usize));
    let material_name = material.and_then(|m| m.get("name")).and_then(|v| v.as_str()).unwrap_or("");
    let factor = material.map(|m| m.get("pbrMetallicRoughness").and_then(|p| floats::<4>(p.get("baseColorFactor"))).unwrap_or([1.0; 4]));
    let color = factor.map_or(PLAIN, |factor| Color::linear_rgba(factor[0], factor[1], factor[2], factor[3]));
    let surface = surface_named(material_name);
    let (middle, half) = ((low + high) / 2.0, (high - low) / 2.0);
    let axes = [world.transform_vector3(Vec3::X * half.x), world.transform_vector3(Vec3::Y * half.y), world.transform_vector3(Vec3::Z * half.z)];
    let centre = world.transform_point3(middle);
    let upright = axes[1].try_normalize().is_some_and(|up| up.y.abs() > 2f32.to_radians().cos()) || half.y == 0.0;
    let level_x = Vec3::new(axes[0].x, 0.0, axes[0].z);
    if upright && level_x.length() > 1e-6 {
        let size = Vec3::new(axes[0].length(), axes[1].length(), axes[2].length()) * 2.0;
        return Some((LevelBox { name: String::new(), centre, size, yaw: yaw_of(axes[0]), color, surface }, false));
    }
    // Tilted: the upright box about its corners.
    let reach = axes.iter().fold(Vec3::ZERO, |sum, axis| sum + axis.abs());
    Some((LevelBox { name: String::new(), centre, size: reach * 2.0, yaw: 0.0, color, surface }, true))
}

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
fn unit_cube() -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u16>) {
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

fn base64(bytes: &[u8]) -> String {
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
            let surface_name = trp_data::sound::SURFACES.get(surface as usize).copied().unwrap_or("Concrete");
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
    let arrow = material(Color::srgb(0.95, 0.75, 0.1), trp_sim::steps::CONCRETE, Some("start arrow"), &mut materials);
    let mut roots = Vec::new();
    // The ground, for the eye.
    let ground = material(Color::srgb(0.35, 0.35, 0.35), trp_sim::steps::CONCRETE, Some("ground (not read)"), &mut materials);
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
    let housing = material(Color::srgb(0.18, 0.18, 0.19), crate::range::METAL_SURFACE, Some("lamp"), &mut materials);
    let lamps: Vec<usize> = layout.lamps.iter().enumerate().map(|(n, at)| box_node(&format!("lamp {}", n + 1), *at, Vec3::new(0.7, 0.14, 0.7), 0.0, housing, world, &mut nodes)).collect();
    roots.push(group("lamps", lamps, &mut nodes));
    if let Some(indoors) = &layout.indoors {
        let glass = material(Color::srgba(0.3, 0.6, 1.0, 0.15), trp_sim::steps::CONCRETE, Some("indoors (not drawn)"), &mut materials);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_matched_without_a_copys_number() {
        assert_eq!(clean("Prop Rifles Table.003"), "prop rifles table");
        assert_eq!(clean("lamp 4"), "lamp 4");
        assert_eq!(clean("v1.5"), "v1");
        assert_eq!(surface_named("Metal 6b6b6b"), 1);
        assert_eq!(surface_named("woodthick_old"), 12);
        assert_eq!(surface_named("Material.002"), trp_sim::steps::CONCRETE);
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
}
