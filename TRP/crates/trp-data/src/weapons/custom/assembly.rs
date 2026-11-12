//! Building a custom weapon's meshes and materials.

use super::*;

/// A picture file of each kind a material has.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Material {
    pub name: String,
    pub base_color: Option<PathBuf>,
    pub normal: Option<PathBuf>,
    pub roughness: Option<PathBuf>,
    pub metallic: Option<PathBuf>,
    pub occlusion: Option<PathBuf>,
    /// The largest side a picture is drawn with, and which way green
    /// points in the normal picture.
    pub size: u32,
    pub normal_green_down: bool,
}

/// One of the model's parts as it is drawn: a mesh on a node of the
/// weapon's hierarchy, in the axes of the weapon's own bone, with the
/// material of each of its submeshes.
#[derive(Debug, Clone)]
pub struct Part {
    pub node: usize,
    pub mesh: Arc<MeshData>,
    pub materials: Vec<usize>,
}

/// What a weapon of one's own has beyond a weapon of the install's.
#[derive(Debug, Clone)]
pub struct Build {
    pub parts: Vec<Part>,
    pub materials: Vec<Material>,
    pub edits: Arc<PoseEdits>,
}

/// A model's mesh on the bone it is placed on by `place` (a point of the
/// model to metres on the bone), in the manner of the install's own
/// meshes: left-handed, a submesh for each material, a tangent at every
/// vertex.
pub(super) fn mesh_of(name: &str, source: &fbx::Mesh, place: impl Fn([f32; 3]) -> [f32; 3]) -> MeshData {
    let mut mesh = MeshData { name: name.to_owned(), ..MeshData::default() };
    let slots = source.materials.iter().copied().max().map_or(1, |most| most as usize + 1);
    mesh.submeshes = vec![Vec::new(); slots];
    // Corners that are the same in place, normal and picture are one vertex.
    let mut seen: HashMap<[u32; 8], u32> = HashMap::new();
    let mut vertex = |corner: usize| -> u32 {
        let (p, n, uv) = (source.positions[corner], source.normals[corner], source.uvs[corner]);
        let key = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits(), n[0].to_bits(), n[1].to_bits(), n[2].to_bits(), uv[0].to_bits(), uv[1].to_bits()];
        let next = mesh.positions.len() as u32;
        *seen.entry(key).or_insert_with(|| {
            mesh.positions.push(place(p));
            // The same change of axes as a point's, without its place.
            mesh.normals.push([n[0], n[2], n[1]]);
            mesh.uv0.push(uv);
            next
        })
    };
    // The change of axes is a mirroring (the model's are right-handed,
    // the rig's left-handed), which turns every triangle's face the
    // other way: two of its corners change places to turn it back.
    let mut triangles: Vec<(usize, [u32; 3])> = Vec::with_capacity(source.positions.len() / 3);
    for triangle in 0..source.positions.len() / 3 {
        let corner = triangle * 3;
        triangles.push((source.materials[triangle] as usize, [vertex(corner), vertex(corner + 2), vertex(corner + 1)]));
    }
    for (slot, corners) in triangles {
        mesh.submeshes[slot].extend(corners);
    }
    // A tangent along each vertex's picture, as the engine keeps one: the
    // way the picture's u runs over the surface, square to the normal,
    // and a sign for which way its v then runs.
    let count = mesh.positions.len();
    let (mut along, mut across) = (vec![[0.0f32; 3]; count], vec![[0.0f32; 3]; count]);
    for triangle in mesh.submeshes.iter().flat_map(|indices| indices.chunks_exact(3)) {
        let [a, b, c] = [triangle[0] as usize, triangle[1] as usize, triangle[2] as usize];
        let (e1, e2) = (sub(mesh.positions[b], mesh.positions[a]), sub(mesh.positions[c], mesh.positions[a]));
        let (du1, dv1) = (mesh.uv0[b][0] - mesh.uv0[a][0], mesh.uv0[b][1] - mesh.uv0[a][1]);
        let (du2, dv2) = (mesh.uv0[c][0] - mesh.uv0[a][0], mesh.uv0[c][1] - mesh.uv0[a][1]);
        let det = du1 * dv2 - du2 * dv1;
        if det.abs() < 1e-12 {
            continue;
        }
        let t: [f32; 3] = std::array::from_fn(|k| (e1[k] * dv2 - e2[k] * dv1) / det);
        let s: [f32; 3] = std::array::from_fn(|k| (e2[k] * du1 - e1[k] * du2) / det);
        for vertex in [a, b, c] {
            along[vertex] = add(along[vertex], t);
            across[vertex] = add(across[vertex], s);
        }
    }
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    mesh.tangents = (0..count)
        .map(|vertex| {
            let n = mesh.normals[vertex];
            let flat = sub(along[vertex], n.map(|c| c * dot(n, along[vertex])));
            let length = dot(flat, flat).sqrt();
            // A vertex whose picture does not move over the surface has
            // none of its own: any way square to the normal.
            let t = if length > 1e-9 {
                flat.map(|c| c / length)
            } else {
                let other = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
                let side = cross(n, other);
                let length = dot(side, side).sqrt().max(1e-9);
                side.map(|c| c / length)
            };
            let sign = if dot(cross(n, t), across[vertex]) < 0.0 { -1.0 } else { 1.0 };
            [t[0], t[1], t[2], sign]
        })
        .collect();
    mesh
}

/// Makes the donor's bare rig this weapon's: nothing of the donor's is
/// drawn, the model's parts hang on its bones, the bones a weapon's
/// parts would bring are added, and every bone the edits move rests
/// where the edits have it. `hung` are objects of the donor's already
/// hung on its slots' bones (its magazine's prefab, `crate::weapon`): a
/// part on such a bone hangs below that object instead, where the object
/// is the thing the game goes by (a reload shows and hides a magazine's,
/// and finds its rounds' points below it), and no hand goes by anything
/// of theirs.
pub fn build(definition: &Definition, hierarchy: &mut Hierarchy, hung: &[usize]) -> Result<Build> {
    let scene = scene_of(&definition.root, &definition.model)?;
    let unit = scene.unit as f32;

    let weapon = (0..hierarchy.nodes.len()).find(|&i| hierarchy.path(i) == WEAPON_BONE).ok_or_else(|| anyhow!("the donor has no weapon bone"))?;
    let below = |hierarchy: &Hierarchy, mut node: usize| loop {
        if node == weapon {
            return true;
        }
        match hierarchy.nodes[node].parent {
            Some(parent) => node = parent,
            None => return false,
        }
    };
    // Nothing of the donor's own is drawn.
    for index in 0..hierarchy.nodes.len() {
        if below(hierarchy, index) {
            hierarchy.nodes[index].renderer = None;
        }
    }
    for &object in hung {
        for index in object..hierarchy.nodes.len() {
            let mut at = Some(index);
            while let Some(node) = at.filter(|&node| node >= object) {
                if node == object {
                    hierarchy.nodes[index].grip = None;
                    break;
                }
                at = hierarchy.nodes[node].parent;
            }
        }
    }
    let bone = |hierarchy: &Hierarchy, path: &str| -> Result<usize> {
        let full = if path == "weapon" { WEAPON_BONE.to_owned() } else { format!("{WEAPON_BONE}/{path}") };
        (0..hierarchy.nodes.len()).find(|&i| hierarchy.path(i) == full).ok_or_else(|| anyhow!("the donor has no bone {path}"))
    };

    // The donor's bones at rest, and the edits on them.
    let parents: Vec<Option<usize>> = hierarchy.nodes.iter().map(|node| node.parent).collect();
    let mut edits = PoseEdits { parents, weapon, moved: Vec::new(), hand: None, donor_rest: Vec::new() };
    let rest: Vec<Placed> = hierarchy.nodes.iter().map(|node| (node.translation, node.rotation)).collect();
    let at_rest = |edits: &PoseEdits, node: usize| if node == weapon { ([0.0; 3], [0.0, 0.0, 0.0, 1.0]) } else { edits.on_weapon(&rest, node) };
    for part in &definition.parts {
        let Some(pivot) = part.pivot else { continue };
        let on = bone(hierarchy, &part.bone)?;
        ensure!(on != weapon, "the weapon's own bone is not moved");
        let by = sub(definition.on_bone(pivot, unit), at_rest(&edits, on).0);
        match edits.moved.iter().position(|(moved, _)| *moved == on) {
            Some(already) => ensure!(edits.moved[already].1 == by, "two parts on {} turn about different points", part.bone),
            None => edits.moved.push((on, by)),
        }
    }
    if let Some(hands) = &definition.hands {
        let direct = |name: &str| -> Result<usize> {
            let node = bone(hierarchy, name)?;
            ensure!(hierarchy.nodes[node].parent == Some(weapon), "{name} does not hang from the weapon's bone");
            Ok(node)
        };
        let (marker, reference) = (direct(LEFT_MARKER)?, direct(LEFT_REFERENCE)?);
        let mut holds = Vec::new();
        let mut resting = [0.0; 3];
        for hold in &hands.holds {
            let on = bone(hierarchy, &hold.bone)?;
            let turn = at_rest(&edits, on).1;
            // Given on the weapon's bone at rest; kept in the bone's own
            // axes, so that it turns with the bone.
            let palms = hold.palms.iter().map(|palm| rotate(back(turn), of_centimetres(*palm))).collect();
            let moved_by = edits.moved.iter().find(|(moved, _)| *moved == on).map(|(_, by)| *by);
            let shift = hold.shift.map(of_centimetres).or(moved_by).ok_or_else(|| anyhow!("the hold on {} says no shift, and no part moves that bone", hold.bone))?;
            if hold.rest {
                resting = shift;
            }
            holds.push(Hold { bone: (on != weapon).then_some(on), palms, shift });
        }
        ensure!(!holds.is_empty(), "the hands have no holds");
        edits.hand = Some((marker, reference, holds, resting, hands.exponent));
    }
    // Every bone the edits move rests where they have it, and the
    // donor's own rest is kept for its animator.
    let mut edited = rest.clone();
    edits.apply(&mut edited);
    for (index, (before, after)) in rest.iter().zip(&edited).enumerate() {
        if before.0 != after.0 {
            edits.donor_rest.push((index, before.0));
            hierarchy.nodes[index].translation = after.0;
        }
    }
    let placed = |hierarchy: &Hierarchy, node: usize| -> Placed {
        if node == weapon {
            return ([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
        }
        let pose: Vec<Placed> = hierarchy.nodes.iter().map(|node| (node.translation, node.rotation)).collect();
        edits.on_weapon(&pose, node)
    };
    let mut made = 0i64;
    let mut new_node = |hierarchy: &mut Hierarchy, name: &str, parent: usize, at: [f32; 3], turn: [f32; 4]| -> usize {
        made += 1;
        hierarchy.nodes.push(Node {
            name: name.to_owned(),
            parent: Some(parent),
            translation: at,
            rotation: turn,
            scale: [1.0; 3],
            active: true,
            renderer: None,
            scripts: Vec::new(),
            script_objects: Vec::new(),
            grip: None,
            placer: None,
            // No object of the install's.
            transform: ObjRef { file: usize::MAX, path_id: made },
            colliders: Vec::new(),
            bounded: None,
            lower: None,
            lod: None,
        });
        hierarchy.nodes.len() - 1
    };

    // The bones a weapon's parts would bring.
    for added in &definition.bones {
        let parent = bone(hierarchy, &added.parent)?;
        let (parent_at, parent_turn) = placed(hierarchy, parent);
        let turn = match &added.turn_as {
            Some(other) => placed(hierarchy, bone(hierarchy, other)?).1,
            None => [0.0, 0.0, 0.0, 1.0],
        };
        let at = rotate(back(parent_turn), sub(definition.on_bone(added.at, unit), parent_at));
        new_node(hierarchy, &added.name, parent, at, quat_mul(back(parent_turn), turn));
    }
    // The bones of its own slots, on the weapon's bone and unturned, as
    // the donor's slots are there (a part hung a quarter turn from its
    // bone then stands upright, `crate::weapon::attach_at`).
    for own in &definition.slots {
        ensure!(hierarchy.find_below(weapon, &own.name).is_none(), "the donor has a bone {} already", own.name);
        new_node(hierarchy, &own.name, weapon, definition.on_bone(own.at, unit), [0.0, 0.0, 0.0, 1.0]);
    }

    // The model's parts, each a node on its bone that undoes the bone's
    // own place at rest: the mesh is in the axes of the weapon's bone.
    let mut build = Build { parts: Vec::new(), materials: Vec::new(), edits: Arc::new(edits.clone()) };
    for part in &definition.parts {
        let model = scene.models.iter().find(|model| model.name == part.model).ok_or_else(|| anyhow!("the model has no part {}", part.model))?;
        let source = model.mesh.as_ref().ok_or_else(|| anyhow!("the part {} has no mesh", part.model))?;
        let mesh = mesh_of(&model.name, source, |p| definition.on_bone(p, unit));
        let on = bone(hierarchy, &part.bone)?;
        let hung_on = hung.iter().copied().find(|&object| hierarchy.nodes[object].parent == Some(on));
        // The model's magazine is drawn with a magazine fitted alone, as an
        // install's weapon without one is built without its magazine.
        if part.bone == "mod_magazine" && hung_on.is_none() {
            continue;
        }
        let parent = hung_on.unwrap_or(on);
        let (parent_at, parent_turn) = placed(hierarchy, parent);
        let node = new_node(hierarchy, &part.model, parent, rotate(back(parent_turn), parent_at.map(|c| -c)), back(parent_turn));
        let materials = (0..mesh.submeshes.len())
            .map(|slot| {
                let name = model.materials.get(slot).cloned().unwrap_or_default();
                build.materials.iter().position(|material| material.name == name).unwrap_or_else(|| {
                    build.materials.push(material_of(&definition.root, &definition.model, &name));
                    build.materials.len() - 1
                })
            })
            .collect();
        build.parts.push(Part { node, mesh: Arc::new(mesh), materials });
    }
    Ok(build)
}

impl Build {
    /// Adds another build's parts, each material kept once.
    pub fn take(&mut self, other: Build) {
        let mut index = Vec::with_capacity(other.materials.len());
        for material in other.materials {
            match self.materials.iter().position(|kept| *kept == material) {
                Some(at) => index.push(at),
                None => {
                    self.materials.push(material);
                    index.push(self.materials.len() - 1);
                }
            }
        }
        for mut part in other.parts {
            part.materials = part.materials.iter().map(|&m| index[m]).collect();
            self.parts.push(part);
        }
    }
}
