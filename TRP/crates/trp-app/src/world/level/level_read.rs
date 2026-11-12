//! The level's boxes, stands and starts, and a level read from glTF.

use super::*;

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
pub(super) const PLAIN: Color = Color::srgb(0.55, 0.55, 0.55);

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
pub(super) fn clean(name: &str) -> String {
    let name = name.trim().to_lowercase();
    match name.rsplit_once('.') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => head.trim().to_owned(),
        _ => name,
    }
}

/// What a material's name says a box sounds of: its first word, one of the
/// game's surfaces (concrete if none is).
pub(super) fn surface_named(name: &str) -> u8 {
    let first = name.split(|c: char| c.is_whitespace() || c == '_' || c == '-').next().unwrap_or("");
    trp_data::audio::sound::SURFACES.iter().position(|surface| surface.eq_ignore_ascii_case(first)).map_or(trp_sim::sound::steps::CONCRETE, |at| at as u8)
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
                    let square = named.strip_prefix("sampler part ").and_then(|n| n.trim().parse::<usize>().ok()).filter(|n| (1..=trp_data::audio::sound::SURFACES.len()).contains(n));
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
