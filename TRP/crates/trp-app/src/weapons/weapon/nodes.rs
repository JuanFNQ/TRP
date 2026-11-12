//! Trees and nodes spawned, with their glows, colliders and skins.

use super::*;

/// One rig's entities: every node in the order of its prefab, and each by
/// its path from the animated root.
pub(crate) struct Tree {
    pub entities: Vec<Entity>,
    pub by_path: HashMap<String, Entity>,
    pub rendered: usize,
    /// The entity that draws each node's mesh of each slot: the node, the
    /// slot and the entity.
    pub drawn: Vec<(usize, usize, Entity)>,
}

/// A rig's parts that glow with its heat, given their twins
/// (`heat_view::spawn_glows`) once its tree is in the world, their owner its
/// `Weapon_root` (given the weapon in hand's heat while it is the
/// `WeaponRig`'s).
pub(super) fn glows_of(commands: &mut Commands, weapon: &trp_data::weapons::weapon::Weapon, tree: &Tree, parts: &[Vec<(Handle<Mesh>, Handle<StandardMaterial>)>], skins: &[Skin]) {
    let Some(&owner) = tree.by_path.get("Weapon_root").filter(|_| !weapon.hot.is_empty()) else { return };
    let skinned = |node: usize| skins.get(node).is_some_and(Option::is_some);
    crate::weapons::heat_view::spawn_glows(commands, owner, &weapon.hot, &tree.drawn, parts, &skinned, HANDS_LAYER);
    commands.entity(owner).insert(crate::weapons::heat_view::HeatOwner::hands());
}

/// The entity of a rig already there that a node of another's is, where
/// the two prefabs have it alike outside the weapon's own tree.
fn shared_node(hierarchy: &Hierarchy, index: usize, shared: Option<&HashMap<String, Entity>>) -> Option<Entity> {
    let path = hierarchy.path(index);
    shared.and_then(|shared| shared.get(&path).copied()).filter(|_| !path.starts_with("Weapon_root"))
}

/// Spawns a rig's nodes under the view's pivot, with their meshes. With
/// `shared`, the nodes of a rig already there are used for every path the
/// two have alike outside the weapon's own tree (the arms' bones and the
/// camera's: the prefabs name them alike, and the body's meshes are
/// skinned to them once), so that they are posed by whichever rig is in
/// the hands; the weapon's own tree is this rig's alone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_tree(
    commands: &mut Commands,
    unity: &mut trp_data::unity::Assets,
    converted: &mut Converted,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
    hierarchy: &Hierarchy,
    pivot: Entity,
    shared: Option<&HashMap<String, Entity>>,
    custom: Option<&trp_data::weapons::custom::Build>,
) -> Tree {
    // What each node draws, made here, on the frame's own thread.
    let mut parts: Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>> = vec![Vec::new(); hierarchy.nodes.len()];
    let mut skins: Vec<Skin> = vec![None; hierarchy.nodes.len()];
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let Some(renderer) = drawn_renderer(node).filter(|_| shared_node(hierarchy, index, shared).is_none()) else { continue };
        match converted.mesh(unity, renderer.mesh, !renderer.skinned && hierarchy.mirrored(index), meshes) {
            Ok(entry) => {
                let jointed = entry.parts.iter().all(|mesh| meshes.get(mesh).is_some_and(|mesh| mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX).is_some()));
                match skin_bones(hierarchy, index).filter(|bones| jointed && bones.len() == entry.bind_poses.len()) {
                    Some(bones) => skins[index] = Some((bones, bindposes.add(SkinnedMeshInverseBindposes::from(entry.bind_poses.clone())))),
                    None => unskin_all(meshes, &entry.parts),
                }
                for (slot, mesh) in entry.parts.iter().enumerate() {
                    let material = converted.material(unity, renderer.materials.get(slot).copied().flatten(), images, materials);
                    parts[index].push((mesh.clone(), material));
                }
            }
            Err(e) => warn!("mesh on {}: {e:#}", node.name),
        }
    }
    // A weapon that is not the install's: the parts of its own model,
    // each on the node made for it, with its own pictures.
    let mut extra = Vec::new();
    if let Some(custom) = custom {
        let looks = converted.custom_materials(&custom.materials, images, materials);
        for part in &custom.parts {
            let made = converted.custom_mesh(&part.mesh, meshes);
            unskin_all(meshes, &made);
            for (slot, mesh) in made.into_iter().enumerate() {
                let Some(material) = part.materials.get(slot).and_then(|index| looks.get(*index)) else { continue };
                extra.push((part.node, mesh, material.clone()));
            }
        }
    }
    spawn_nodes(commands, hierarchy, pivot, shared, &parts, &skins, &extra)
}

/// What a node of a rig draws: its renderer, or with the levels of detail
/// drawn by (`lod_view::on`) a coarser level's that is not on bones (`Node::
/// lower`). `PROVISIONAL`: a part on bones (a weapon's spring) keeps its
/// fullest at any distance, as before, where the game's group switches it.
pub(super) fn drawn_renderer(node: &trp_data::unity::extract::Node) -> Option<&trp_data::unity::extract::Renderer> {
    node.renderer.as_ref().or(node.lower.as_ref().filter(|lower| !lower.skinned && crate::assets::lod_view::on()))
}

/// Each drawn piece of a rig tagged with its levels of detail (`lod_view`),
/// placed by its group's object: a renderer its group lists, not on bones.
/// Its groups switch together: a weapon's parts with their weapon
/// (`lod_view::LodSpec::bundle`).
fn tag_levels(commands: &mut Commands, hierarchy: &Hierarchy, entities: &[Entity], drawn: &[(usize, usize, Entity)]) {
    if !crate::assets::lod_view::on() || hierarchy.lods.is_empty() {
        return;
    }
    let specs: Vec<std::sync::Arc<crate::assets::lod_view::LodSpec>> = crate::assets::lod_view::LodSpec::bundle(hierarchy.lods.iter().map(|lod| &*lod.group));
    for &(index, _, entity) in drawn {
        let node = &hierarchy.nodes[index];
        let (Some(lod), Some(renderer)) = (node.lod, drawn_renderer(node)) else { continue };
        if renderer.skinned {
            continue;
        }
        let (Some(group), Some(anchor)) = (specs.get(lod.group), hierarchy.lods.get(lod.group).and_then(|at| entities.get(at.node))) else { continue };
        commands.entity(entity).insert(crate::assets::lod_view::LodPiece { group: group.clone(), anchor: Some(*anchor), levels: lod.levels });
    }
}

/// The colliders a node of a rig has in its prefab, in its own axes (the
/// game's): what a weapon let fall falls on (`death`).
#[derive(Component, Clone)]
pub struct NodeColliders(pub Vec<trp_data::unity::extract::NodeCollider>);

/// The bones a node's skinned mesh is drawn on, by their places in the
/// tree, where it is skinned and every one of them is there (a weapon's
/// part on bones of its own: the AS VAL's and the VSS's hammer spring,
/// stretched between two). None for any other: it is drawn without bones,
/// as it lies.
pub(super) fn skin_bones(hierarchy: &Hierarchy, index: usize) -> Option<Vec<usize>> {
    let renderer = hierarchy.nodes[index].renderer.as_ref().filter(|renderer| renderer.skinned && !renderer.bones.is_empty())?;
    renderer.bones.iter().map(|bone| bone.filter(|bone| *bone < hierarchy.nodes.len())).collect()
}

/// A node's mesh drawn on bones: the bones' places in the tree, and the
/// mesh's inverse bind poses.
pub(crate) type Skin = Option<(Vec<usize>, Handle<SkinnedMeshInverseBindposes>)>;

/// Spawns a rig's nodes under the view's pivot, each drawing what `parts`
/// has for it (a mesh with its material, one or more), on the bones
/// `skins` names for it, and `extra` on the nodes it names (a weapon of
/// one's own's parts). Shared nodes as for `spawn_tree`.
pub(crate) fn spawn_nodes(
    commands: &mut Commands,
    hierarchy: &Hierarchy,
    pivot: Entity,
    shared: Option<&HashMap<String, Entity>>,
    parts: &[Vec<(Handle<Mesh>, Handle<StandardMaterial>)>],
    skins: &[Skin],
    extra: &[(usize, Handle<Mesh>, Handle<StandardMaterial>)],
) -> Tree {
    let mut entities: Vec<Entity> = Vec::with_capacity(hierarchy.nodes.len());
    let mut by_path = HashMap::new();
    let mut rendered = 0;
    // The entities a node's meshes are drawn by, for its bones once every
    // node is there.
    let mut drawn_by: Vec<(usize, Entity)> = Vec::new();
    let mut drawn = Vec::new();
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let path = hierarchy.path(index);
        if let Some(entity) = shared_node(hierarchy, index, shared) {
            entities.push(entity);
            by_path.insert(path, entity);
            continue;
        }
        let visibility = if node.active { Visibility::Inherited } else { Visibility::Hidden };
        let mut transform = convert::transform(node.translation, node.rotation, node.scale);
        if index == 0 {
            // The rig's own origin is the view pivot's: the game puts it
            // at the body's ribcage every frame.
            transform.translation = Vec3::ZERO;
        }
        let entity = commands.spawn((Name::new(node.name.clone()), transform, visibility)).id();
        commands.entity(node.parent.map_or(pivot, |p| entities[p])).add_child(entity);
        // Its colliders, for the weapon let fall (`death`): the game's own
        // on its parts.
        if !node.colliders.is_empty() {
            commands.entity(entity).insert(NodeColliders(node.colliders.clone()));
        }
        entities.push(entity);
        by_path.insert(path, entity);
        let own = &parts[index];
        // A renderer set to cast its shadow alone draws nothing but it
        // (`ShadowCastingMode.ShadowsOnly`: `shadow_only`).
        let shadow = crate::rendering::shadow_only::ShadowOnly(node.renderer.as_ref().is_some_and(|renderer| renderer.shadows_only));
        for (slot, (mesh, material)) in own.iter().enumerate() {
            let bundle = (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), bevy::camera::visibility::RenderLayers::layer(HANDS_LAYER), shadow);
            if own.len() == 1 {
                commands.entity(entity).insert(bundle);
                drawn_by.push((index, entity));
                drawn.push((index, slot, entity));
            } else {
                let part = commands.spawn((bundle, Transform::default())).id();
                commands.entity(entity).add_child(part);
                drawn_by.push((index, part));
                drawn.push((index, slot, part));
            }
            rendered += 1;
        }
    }
    for (index, entity) in drawn_by {
        let Some(Some((bones, inverse_bindposes))) = skins.get(index) else { continue };
        let joints = bones.iter().map(|bone| entities[*bone]).collect();
        // Skinned bounds are not tracked.
        commands.entity(entity).insert((SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints }, NoFrustumCulling));
    }
    for (node, mesh, material) in extra {
        let drawn = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), bevy::camera::visibility::RenderLayers::layer(HANDS_LAYER))).id();
        commands.entity(entities[*node]).add_child(drawn);
        rendered += 1;
    }
    tag_levels(commands, hierarchy, &entities, &drawn);
    Tree { entities, by_path, rendered, drawn }
}
