//! Dressing and hanging parts, muzzles and magazines on a weapon.

use super::*;

/// Where the game puts a part's root on its slot's bone
/// (`crate::weapon::attach_at`): where its `ModPlacer` says, or at the
/// bone's origin a quarter turn about x.
fn slot_place(node: &Node) -> Placed {
    match node.placer {
        Some(placer) => (placer.position, crate::weapons::weapon::quat_of_euler(placer.rotation)),
        None => ([0.0; 3], crate::weapons::weapon::quat_of_euler([90.0, 0.0, 0.0])),
    }
}

/// A node made for a part of one's own, with no object of the install's.
fn push_node(hierarchy: &mut Hierarchy, name: &str, parent: usize, at: [f32; 3], turn: [f32; 4]) -> usize {
    let index = hierarchy.nodes.len();
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
        // Numbered apart from the weapon's own (`build`, `take_muzzle`).
        transform: ObjRef { file: usize::MAX, path_id: 1_000_000 + index as i64 },
        colliders: Vec::new(),
        bounded: None,
        lower: None,
        lod: None,
    });
    index
}

/// Makes the donor's prefab hung at `root` (that object and all below it,
/// the last of the tree) a part of one's own: nothing of the donor's is
/// drawn but the objects its file keeps, each at its size; its own slots'
/// bones are made where the model has them, unturned on the bone of the
/// slot the part is in as a weapon's slots' bones are (a donor's bone of
/// the same name is renamed, so that what goes in the slot goes on the
/// new one); and the model's pieces hang on a node at that slot's bone.
/// What is drawn of the model, with no edits to any pose.
pub fn dress_part(part: &PartDefinition, hierarchy: &mut Hierarchy, root: usize) -> Result<Build> {
    let scene = scene_of(&part.root, &part.model)?;
    let unit = scene.unit as f32;
    ensure!(hierarchy.nodes[root].placer.is_none_or(|placer| placer.scale.iter().all(|s| (s - 1.0).abs() < 1e-3)), "its donor is scaled on its slot");
    // The slot's bone in the root's axes.
    let (at, turn) = slot_place(&hierarchy.nodes[root]);
    let (bone_at, bone_turn) = (rotate(back(turn), at.map(|c| -c)), back(turn));
    let mut own = vec![false; hierarchy.nodes.len()];
    own[root] = true;
    for index in root + 1..hierarchy.nodes.len() {
        own[index] = hierarchy.nodes[index].parent.is_some_and(|parent| parent >= root && own[parent]);
    }
    for index in (root..hierarchy.nodes.len()).filter(|&index| own[index]) {
        let node = &mut hierarchy.nodes[index];
        match part.kept.iter().find(|kept| kept.name == node.name) {
            Some(kept) => node.scale = node.scale.map(|s| s * kept.scale),
            None => node.renderer = None,
        }
    }
    for slot in &part.slots {
        if let Some(theirs) = hierarchy.find_below(root, &slot.name) {
            hierarchy.nodes[theirs].name = format!("{} (donor)", slot.name);
        }
        let place = add(bone_at, rotate(bone_turn, part.on_slot(slot.at, unit)));
        push_node(hierarchy, &slot.name, root, place, bone_turn);
    }
    let node = push_node(hierarchy, &format!("{} (model)", part.name), root, bone_at, bone_turn);
    let mut build = Build { parts: Vec::new(), materials: Vec::new(), edits: Arc::new(PoseEdits::none()) };
    for pick in &part.meshes {
        let model = scene.models.iter().find(|model| model.name == pick.model).ok_or_else(|| anyhow!("the model has no part {}", pick.model))?;
        let source = model.mesh.as_ref().ok_or_else(|| anyhow!("the part {} has no mesh", pick.model))?;
        let taken = picked(source, &pick.pieces(source));
        ensure!(!taken.positions.is_empty(), "nothing of {} is taken", pick.model);
        let mesh = mesh_of(&model.name, &taken, |p| part.on_slot(p, unit));
        let materials = (0..mesh.submeshes.len())
            .map(|slot| {
                let material = material_of(&part.root, &part.model, model.materials.get(slot).map_or("", String::as_str));
                build.materials.iter().position(|kept| *kept == material).unwrap_or_else(|| {
                    build.materials.push(material);
                    build.materials.len() - 1
                })
            })
            .collect();
        build.parts.push(Part { node, mesh: Arc::new(mesh), materials });
    }
    Ok(build)
}

/// Hangs a part on a slot's bone as the game hangs one
/// (`crate::weapon::attach_at`), and makes a part of one's own its own
/// (`dress_part`): the index of its root, and what of a model is drawn.
pub fn hang(assets: &mut crate::unity::Assets, db: &Database, hierarchy: &mut Hierarchy, bone: usize, template: &str) -> Result<(usize, Option<Build>)> {
    let item = db.item(template)?;
    let key = item.prefab_key().ok_or_else(|| anyhow!("item has no prefab"))?.to_owned();
    let root = crate::weapons::weapon::attach_at(assets, hierarchy, bone, &key)?;
    match db.custom_parts.get(template) {
        Some(part) => Ok((root, Some(dress_part(part, hierarchy, root).with_context(|| format!("making {} its own", part.name))?))),
        None => Ok((root, None)),
    }
}

/// A part of one's own as a thing of its own (let go, or drawn in its
/// picture), with the parts its build has in its slots: its donor's
/// prefab at the origin and unturned, as an item's own object (a
/// picture's pose replaces its turn), made its own, and the rest hung
/// below as on a weapon.
pub fn part_model(assets: &mut crate::unity::Assets, db: &Database, preset: &Preset) -> Result<(Hierarchy, Build)> {
    let first = preset.items.iter().find(|item| item.id == preset.root).ok_or_else(|| anyhow!("{} has no root item", preset.id))?;
    let part = db.custom_parts.get(&first.template).ok_or_else(|| anyhow!("{} is not a part of one's own", first.template))?;
    let key = db.item(&first.template)?.prefab_key().ok_or_else(|| anyhow!("item has no prefab"))?.to_owned();
    let mut hierarchy = crate::weapons::weapon::prefab_tree(assets, &key)?;
    (hierarchy.nodes[0].translation, hierarchy.nodes[0].rotation) = ([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let mut build = dress_part(part, &mut hierarchy, 0)?;
    let mut root_of: HashMap<&str, usize> = HashMap::from([(first.id.as_str(), 0)]);
    let mut remaining: Vec<&PresetItem> = preset.items.iter().filter(|item| item.id != first.id).collect();
    while !remaining.is_empty() {
        let before = remaining.len();
        let mut later = Vec::new();
        for item in remaining {
            let (Some(&parent), Some(slot)) = (item.parent.as_deref().and_then(|parent| root_of.get(parent)), item.slot.as_deref()) else {
                later.push(item);
                continue;
            };
            let bone = hierarchy.find_below(parent, slot).ok_or_else(|| anyhow!("no bone named {slot}"))?;
            let (root, own) = hang(assets, db, &mut hierarchy, bone, &item.template)?;
            if let Some(own) = own {
                build.take(own);
            }
            root_of.insert(item.id.as_str(), root);
        }
        ensure!(later.len() < before, "a part's parent was not placed");
        remaining = later;
    }
    Ok((hierarchy, build))
}

/// Where an object is in the axes of an object above it, by the
/// hierarchy at rest; none is scaled on the way.
pub(super) fn below(hierarchy: &Hierarchy, above: usize, mut node: usize) -> Result<Placed> {
    let (mut place, mut turn) = ([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    while node != above {
        let n = &hierarchy.nodes[node];
        ensure!(n.scale.iter().all(|s| (s - 1.0).abs() < 1e-3), "{} is scaled", n.name);
        place = add(rotate(n.rotation, place), n.translation);
        turn = quat_mul(n.rotation, turn);
        node = n.parent.ok_or_else(|| anyhow!("{} is not below {}", hierarchy.nodes[node].name, hierarchy.nodes[above].name))?;
    }
    Ok((place, turn))
}

/// Another weapon's muzzle effects laid on this one (`muzzle_from`): the
/// objects the game takes them from on that weapon (`crate::muzzle::
/// effect_objects`), each put below this weapon's `fireport` where it is
/// from that weapon's fireport in the axes of its weapon bone, with its
/// scripts. The receiver's `extractor_smoke` is not taken: it is no
/// muzzle's, and its fume is not drawn.
pub fn take_muzzle(from: &Hierarchy, hierarchy: &mut Hierarchy) -> Result<()> {
    let bone_of = |h: &Hierarchy| (0..h.nodes.len()).find(|&i| h.path(i) == WEAPON_BONE).ok_or_else(|| anyhow!("no weapon bone"));
    let fireport_of = |h: &Hierarchy, weapon: usize| h.find_below(weapon, "fireport").ok_or_else(|| anyhow!("no fireport"));
    let (their_weapon, our_weapon) = (bone_of(from)?, bone_of(hierarchy)?);
    let their_fireport = below(from, their_weapon, fireport_of(from, their_weapon)?)?.0;
    let our_fireport = fireport_of(hierarchy, our_weapon)?;
    let turn = below(hierarchy, our_weapon, our_fireport)?.1;
    for (made, node) in crate::weapons::muzzle::effect_objects(from).into_iter().filter(|&node| from.nodes[node].name != "extractor_smoke").enumerate() {
        let (at, its_turn) = below(from, their_weapon, node)?;
        let source = &from.nodes[node];
        hierarchy.nodes.push(Node {
            name: source.name.clone(),
            parent: Some(our_fireport),
            translation: rotate(back(turn), sub(at, their_fireport)),
            rotation: quat_mul(back(turn), its_turn),
            scale: [1.0; 3],
            active: true,
            renderer: None,
            scripts: source.scripts.clone(),
            script_objects: source.script_objects.clone(),
            grip: None,
            placer: None,
            // No object of this weapon's (the build's own count up).
            transform: ObjRef { file: usize::MAX, path_id: -1 - made as i64 },
            colliders: Vec::new(),
            bounded: None,
            lower: None,
            lod: None,
        });
    }
    Ok(())
}

/// The weapon's magazine as a thing of its own (let go, or drawn in its
/// picture): its object, the first under its slot (`build`'s `hung`), as
/// the root of its own tree, with all below it (the model's magazine, its
/// rounds' points) and the model's parts among them. The root at the
/// origin and unturned, as an item's own object (a picture's pose
/// replaces its turn).
pub fn magazine_model(weapon: &crate::weapons::weapon::Weapon) -> Result<(Hierarchy, Build)> {
    let custom = weapon.custom.as_ref().ok_or_else(|| anyhow!("{} is one of the install's own", weapon.name))?;
    let h = &weapon.hierarchy;
    let slot = h.find_below(0, "mod_magazine").ok_or_else(|| anyhow!("{} has no magazine slot", weapon.name))?;
    let object = (0..h.nodes.len()).find(|&i| h.nodes[i].parent == Some(slot)).ok_or_else(|| anyhow!("{} has no magazine", weapon.name))?;
    // Its own and all below it, each after its parent as in the tree.
    let mut new_index = vec![None; h.nodes.len()];
    let mut nodes = Vec::new();
    for index in object..h.nodes.len() {
        let parent = h.nodes[index].parent.and_then(|parent| new_index[parent]);
        if index != object && parent.is_none() {
            continue;
        }
        let mut node = h.nodes[index].clone();
        node.parent = parent;
        if index == object {
            (node.translation, node.rotation) = ([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
        }
        new_index[index] = Some(nodes.len());
        nodes.push(node);
    }
    // Its parts, with only the materials they are drawn with (each a
    // picture of every kind to send).
    let mut materials = Vec::new();
    let mut parts = Vec::new();
    for part in custom.parts.iter().filter(|part| new_index[part.node].is_some()) {
        let own = part
            .materials
            .iter()
            .map(|&index| {
                let material = &custom.materials[index];
                materials.iter().position(|kept: &Material| kept.name == material.name).unwrap_or_else(|| {
                    materials.push(material.clone());
                    materials.len() - 1
                })
            })
            .collect();
        parts.push(Part { node: new_index[part.node].unwrap_or(0), mesh: part.mesh.clone(), materials: own });
    }
    Ok((Hierarchy { nodes, ..Hierarchy::default() }, Build { parts, materials, edits: custom.edits.clone() }))
}

/// The pictures a material of a model has, by its file's naming.
pub(super) fn material_of(root: &Path, model: &Model, name: &str) -> Material {
    let folder = root.join(&model.textures);
    let file = |map: &str| {
        let path = folder.join(model.texture.replace("{material}", name).replace("{map}", map));
        path.is_file().then_some(path)
    };
    Material {
        name: name.to_owned(),
        base_color: file("BaseColor"),
        normal: file("Normal"),
        roughness: file("Roughness"),
        metallic: file("Metallic"),
        occlusion: file("AO"),
        size: model.texture_size.max(1),
        normal_green_down: model.normal_green_down,
    }
}
