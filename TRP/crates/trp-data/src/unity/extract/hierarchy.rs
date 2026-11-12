//! A prefab's hierarchy of nodes, renderers, levels of detail and grips.

use super::*;

#[derive(Debug, Clone)]
pub struct Renderer {
    pub mesh: ObjRef,
    pub materials: Vec<Option<ObjRef>>,
    /// For skinned meshes: node index of each bone, in bind pose order.
    pub bones: Vec<Option<usize>>,
    pub skinned: bool,
    /// It casts a shadow and draws nothing (`m_CastShadows` 3,
    /// `ShadowCastingMode.ShadowsOnly`): an item's shadow mesh, as the
    /// repair kits' `shadow_LOD0`.
    pub shadows_only: bool,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub active: bool,
    pub renderer: Option<Renderer>,
    /// Class names of the scripts on this object.
    pub scripts: Vec<String>,
    /// Those scripts' own objects, in the same order (their fields read
    /// where wanted: the muzzle's, `crate::muzzle`).
    pub script_objects: Vec<ObjRef>,
    /// Set on a part's marker for where a hand holds it.
    pub grip: Option<Grip>,
    /// Set on a part that says for itself where it goes on its bone.
    pub placer: Option<Placer>,
    pub transform: ObjRef,
    /// The colliders on this object, in its own axes.
    pub colliders: Vec<NodeCollider>,
    /// The mesh of a renderer here that its LOD group keeps from being
    /// drawn near (a lower level's, `AR_bagarii_lod1`): never drawn here,
    /// but switched on, so the engine counts it in the renderer's bounds
    /// (`Renderer.bounds`), which frame an item's picture
    /// (`WeaponPreview.GetBounds`).
    pub bounded: Option<ObjRef>,
    /// That renderer whole, where this object has one its group draws at
    /// a coarser level only (`renderer` is then none): drawn by a reader
    /// that picks the levels by distance (`lod`; third-person spec,
    /// "Levels of detail"), and by no other, which draws the fullest level
    /// alone at any distance as before.
    pub lower: Option<Renderer>,
    /// The levels of detail the renderer here (`renderer` or `lower`) is
    /// drawn at, where a group lists it: the group's place among
    /// `Hierarchy::lods` and the levels, a bit each. None for a renderer no
    /// group lists, drawn at any distance.
    pub lod: Option<NodeLod>,
}

/// A renderer's levels of detail (`Node::lod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeLod {
    pub group: usize,
    pub levels: u8,
}

/// A group of levels of detail in a tree (`lod::LodGroup`) and the object
/// it is on, by its index.
#[derive(Debug, Clone)]
pub struct HierarchyLod {
    pub node: usize,
    pub group: std::sync::Arc<crate::drawing::lod::LodGroup>,
}

/// Where a part goes on the bone it is put on, when that is not the
/// game's usual place (`ModPlacer`): its place, its turn (degrees about
/// each axis, as `Quaternion.Euler` takes them) and its size on the bone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placer {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

/// A part's marker for a hand: the palm goes to the marker's own place and
/// turn, and the fingers take the stored pose.
#[derive(Debug, Clone)]
pub struct Grip {
    pub left_hand: bool,
    /// 0 the ordinary grip, 1 the alternative one, 2 for an underbarrel weapon.
    pub kind: i32,
    /// Local rotations (`xyzw`) for the palm and every bone below it, in
    /// the order of the hand's own bones, palm first.
    pub fingers: Vec<[f32; 4]>,
    /// Its finger pose is the bones below it as they stand each time it is
    /// read, never stored (`DontCache` on one not cached: `GripPose`'s
    /// indexer reads `FingerTransforms[i].localRotation`): an underbarrel
    /// launcher's, whose own clips move them.
    pub live: bool,
}

/// A GameObject tree, parents before children. Node 0 is the root. With
/// its groups of levels of detail (`Node::lod` numbers them), those that
/// govern nothing left out.
#[derive(Debug, Clone, Default)]
pub struct Hierarchy {
    pub nodes: Vec<Node>,
    pub lods: Vec<HierarchyLod>,
}

impl Hierarchy {
    /// Path of a node from the root, in the form animation bindings use.
    pub fn path(&self, mut index: usize) -> String {
        let mut parts = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            parts.push(self.nodes[index].name.as_str());
            index = parent;
        }
        parts.reverse();
        parts.join("/")
    }

    /// Whether the node is mirrored where it stands: its own scale and its
    /// parents', to the root's and with it, make a reflection (an odd
    /// number of axes turned negative). The engine draws such a renderer
    /// with its culling turned round (the Keymod 6" CASV rail's mesh at
    /// scale −1, −1, −1). Rotations never mirror.
    pub fn mirrored(&self, mut index: usize) -> bool {
        let mut mirrored = false;
        loop {
            let [x, y, z] = self.nodes[index].scale;
            mirrored ^= x * y * z < 0.0;
            match self.nodes[index].parent {
                Some(parent) => index = parent,
                None => return mirrored,
            }
        }
    }

    /// Rest-pose rotation of a node relative to the root node, as `xyzw`.
    pub fn rotation_from_root(&self, mut index: usize) -> [f32; 4] {
        let mut chain = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            chain.push(self.nodes[index].rotation);
            index = parent;
        }
        chain.iter().rev().fold([0.0, 0.0, 0.0, 1.0], |acc, q| quat_mul(acc, *q))
    }

    /// First node called `name` at or below `start`, nearest first.
    pub fn find_below(&self, start: usize, name: &str) -> Option<usize> {
        let mut depth_of = vec![usize::MAX; self.nodes.len()];
        depth_of[start] = 0;
        let mut best: Option<(usize, usize)> = None;
        for i in start..self.nodes.len() {
            if i != start {
                let Some(parent) = self.nodes[i].parent else { continue };
                if depth_of[parent] == usize::MAX {
                    continue;
                }
                depth_of[i] = depth_of[parent] + 1;
            }
            if self.nodes[i].name == name && best.map_or(true, |(d, _)| depth_of[i] < d) {
                best = Some((depth_of[i], i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// First node called `name` below `start`, in the tree's order (depth
    /// first, as `TransformTools.FindTransformRecursive` looks within one
    /// part's tree).
    pub fn find_first_below(&self, start: usize, name: &str) -> Option<usize> {
        let below = |mut i: usize| loop {
            match self.nodes[i].parent {
                Some(parent) if parent == start => return true,
                Some(parent) => i = parent,
                None => return false,
            }
        };
        (start + 1..self.nodes.len()).find(|&i| self.nodes[i].name == name && below(i))
    }

    /// Appends another hierarchy under `parent` and returns the index of its
    /// root, its groups of levels of detail with it.
    pub fn attach(&mut self, parent: usize, other: Hierarchy) -> usize {
        let base = self.nodes.len();
        let groups = self.lods.len();
        for mut node in other.nodes {
            node.parent = Some(node.parent.map_or(parent, |p| p + base));
            for r in node.renderer.iter_mut().chain(node.lower.iter_mut()) {
                for bone in r.bones.iter_mut().flatten() {
                    *bone += base;
                }
            }
            if let Some(lod) = &mut node.lod {
                lod.group += groups;
            }
            self.nodes.push(node);
        }
        self.lods.extend(other.lods.into_iter().map(|lod| HierarchyLod { node: lod.node + base, ..lod }));
        base
    }

    /// The renderer an object draws at some level of detail: its own, or
    /// a coarser level's (`Node::lower`), for a reader that picks levels.
    pub fn drawn_renderer(&self, index: usize) -> Option<&Renderer> {
        let node = &self.nodes[index];
        node.renderer.as_ref().or(node.lower.as_ref())
    }
}

/// Hamilton product of two `xyzw` quaternions: `b` applied first, then `a`.
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let ([ax, ay, az, aw], [bx, by, bz, bw]) = (a, b);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// Reads the tree below a GameObject. A renderer its group of levels of
/// detail draws at a coarser level only is kept apart (`Node::lower`), so
/// that a reader that does not pick levels draws the fullest alone; each
/// renderer a group lists is given its levels (`Node::lod`).
pub fn read_hierarchy(assets: &Assets, game_object: ObjRef) -> Result<Hierarchy> {
    let go = assets.read(game_object)?;
    let root_transform = components(assets, game_object, &go)
        .into_iter()
        .find(|c| assets.class_id(*c) == Some(class::TRANSFORM))
        .ok_or_else(|| anyhow!("game object has no transform"))?;

    let mut nodes = Vec::new();
    let mut node_of_transform: HashMap<ObjRef, usize> = HashMap::new();
    let mut pending_bones: Vec<(usize, Vec<Option<ObjRef>>)> = Vec::new();
    let mut hidden: HashSet<ObjRef> = HashSet::new();
    let mut groups: Vec<HierarchyLod> = Vec::new();
    let mut renderer_objects: Vec<(usize, ObjRef)> = Vec::new();
    let mut stack = vec![(root_transform, None::<usize>)];

    while let Some((transform, parent)) = stack.pop() {
        let t = assets.read(transform)?;
        let go_ref = t
            .req("m_GameObject")?
            .as_pptr()
            .and_then(|p| assets.resolve_loaded(transform.file, p))
            .ok_or_else(|| anyhow!("transform without a game object"))?;
        let go = assets.read(go_ref)?;
        let index = nodes.len();
        node_of_transform.insert(transform, index);

        let mut mesh = None;
        let mut renderer = None;
        let mut scripts = Vec::new();
        let mut script_objects = Vec::new();
        let mut grip = None;
        let mut placer = None;
        let mut colliders = Vec::new();
        let layer = go.get("m_Layer").and_then(Value::as_i64).unwrap_or(0) as u32;
        for comp in components(assets, go_ref, &go) {
            match assets.class_id(comp) {
                Some(class_id @ (BOX_COLLIDER | MESH_COLLIDER | SPHERE_COLLIDER | CAPSULE_COLLIDER)) => {
                    colliders.extend(node_collider(assets, comp, class_id, layer)?);
                }
                Some(class::MESH_FILTER) => {
                    let c = assets.read(comp)?;
                    mesh = c.req("m_Mesh")?.as_pptr().and_then(|p| assets.resolve_loaded(comp.file, p));
                }
                Some(class::MESH_RENDERER) => {
                    let c = assets.read(comp)?;
                    if c.req("m_Enabled")?.as_bool().unwrap_or(true) {
                        renderer = Some((comp, materials(assets, comp, &c)?, None, shadows_only(&c)));
                    }
                }
                Some(class::SKINNED_MESH_RENDERER) => {
                    let c = assets.read(comp)?;
                    if c.req("m_Enabled")?.as_bool().unwrap_or(true) {
                        let skinned_mesh =
                            c.req("m_Mesh")?.as_pptr().and_then(|p| assets.resolve_loaded(comp.file, p));
                        let bones = c
                            .req("m_Bones")?
                            .as_array()
                            .unwrap_or(&[])
                            .iter()
                            .map(|b| b.as_pptr().and_then(|p| assets.resolve_loaded(comp.file, p)))
                            .collect();
                        pending_bones.push((index, bones));
                        renderer = Some((comp, materials(assets, comp, &c)?, skinned_mesh, shadows_only(&c)));
                    }
                }
                Some(class::LOD_GROUP) => {
                    let c = assets.read(comp)?;
                    let lods = c.req("m_LODs")?.as_array().unwrap_or(&[]);
                    let level = |lod: &Value| -> Vec<ObjRef> {
                        lod.at("renderers")
                            .and_then(Value::as_array)
                            .unwrap_or(&[])
                            .iter()
                            .filter_map(|r| r.get("renderer")?.as_pptr())
                            .filter_map(|p| assets.resolve_loaded(comp.file, p))
                            .collect()
                    };
                    let kept: HashSet<ObjRef> = lods.first().map(level).unwrap_or_default().into_iter().collect();
                    for lod in lods.iter().skip(1) {
                        hidden.extend(level(lod).into_iter().filter(|r| !kept.contains(r)));
                    }
                    // The group itself, where it governs something.
                    if let Some(group) = crate::drawing::lod::read(assets, comp).ok().filter(|group| !group.governs_nothing()) {
                        groups.push(HierarchyLod { node: index, group: std::sync::Arc::new(group) });
                    }
                }
                Some(class::MONO_BEHAVIOUR) => {
                    let class_name = script_class(assets, comp);
                    if class_name.as_deref() == Some("GripPose") {
                        let c = assets.read(comp)?;
                        grip = Some(Grip {
                            left_hand: c.req("Hand")?.as_i64() == Some(0),
                            kind: c.req("GripType")?.as_i64().unwrap_or(0) as i32,
                            fingers: c
                                .req("Fingers")?
                                .as_array()
                                .unwrap_or(&[])
                                .iter()
                                .filter_map(Value::as_vec4)
                                .collect(),
                            live: c.get("DontCache").and_then(Value::as_i64).unwrap_or(0) != 0 && c.get("_cached").and_then(Value::as_i64).unwrap_or(0) == 0,
                        });
                    }
                    if class_name.as_deref() == Some("ModPlacer") {
                        let c = assets.read(comp)?;
                        placer = Some(Placer {
                            position: c.req("ModPosition")?.as_vec3().context("ModPosition")?,
                            rotation: c.req("ModRotation")?.as_vec3().context("ModRotation")?,
                            scale: c.req("ModScale")?.as_vec3().context("ModScale")?,
                        });
                    }
                    if let Some(class_name) = class_name {
                        scripts.push(class_name);
                        script_objects.push(comp);
                    }
                }
                _ => {}
            }
        }
        let renderer = renderer.and_then(|(comp, materials, skinned_mesh, shadows_only)| {
            let skinned = skinned_mesh.is_some();
            let mesh = skinned_mesh.or(mesh)?;
            renderer_objects.push((index, comp));
            Some(Renderer { mesh, materials, bones: Vec::new(), skinned, shadows_only })
        });

        nodes.push(Node {
            name: go.req("m_Name")?.as_str().unwrap_or("").to_owned(),
            parent,
            translation: t.req("m_LocalPosition")?.as_vec3().context("position")?,
            rotation: t.req("m_LocalRotation")?.as_vec4().context("rotation")?,
            scale: t.req("m_LocalScale")?.as_vec3().context("scale")?,
            active: go.req("m_IsActive")?.as_bool().unwrap_or(true),
            renderer,
            scripts,
            script_objects,
            grip,
            placer,
            transform,
            colliders,
            bounded: None,
            lower: None,
            lod: None,
        });
        // Reversed so that children come off the stack in file order.
        for child in t.req("m_Children")?.as_array().unwrap_or(&[]).iter().rev() {
            if let Some(obj) = child.as_pptr().and_then(|p| assets.resolve_loaded(transform.file, p)) {
                stack.push((obj, Some(index)));
            }
        }
    }

    let measure = crate::drawing::lod::measuring() && !groups.is_empty();
    let renderers_of = if measure { renderer_objects.clone() } else { Vec::new() };
    for (index, comp) in renderer_objects {
        if hidden.contains(&comp) {
            let lower = nodes[index].renderer.take();
            nodes[index].bounded = lower.as_ref().filter(|renderer| !renderer.skinned).map(|renderer| renderer.mesh);
            nodes[index].lower = lower;
        }
        // Its levels, by the first group that lists it.
        nodes[index].lod = groups.iter().enumerate().find_map(|(group, lod)| Some(NodeLod { group, levels: Some(lod.group.levels_of(comp)).filter(|levels| *levels != 0)? }));
    }
    for (index, bones) in pending_bones {
        let node = &mut nodes[index];
        for r in node.renderer.iter_mut().chain(node.lower.iter_mut()) {
            r.bones = bones.iter().map(|b| b.and_then(|t| node_of_transform.get(&t).copied())).collect();
        }
    }
    // TRP's measure of each group (`lod::measured`), with a picker of levels
    // on: the meshes of the objects its levels list (not on bones, not
    // shadows alone), placed in the axes of the object it is on.
    if measure {
        let mut meshes: HashMap<ObjRef, Option<Arc<crate::drawing::lod::MeshPoints>>> = HashMap::new();
        for lod in &mut groups {
            let group = lod.group.clone();
            let mut drawn = Vec::new();
            for &(index, comp) in &renderers_of {
                let bits = group.levels_of(comp);
                let Some(renderer) = nodes[index].renderer.as_ref().or(nodes[index].lower.as_ref()).filter(|r| bits != 0 && !r.skinned && !r.shadows_only) else { continue };
                let points = meshes
                    .entry(renderer.mesh)
                    .or_insert_with(|| read_mesh(assets, renderer.mesh).ok().map(|data| Arc::new(crate::drawing::lod::MeshPoints { points: data.positions, triangles: data.submeshes })))
                    .clone();
                if let Some(points) = points {
                    drawn.push((index, bits, false, points));
                }
            }
            let parent = |n: usize| nodes[n].parent;
            let placing = |n: usize| (nodes[n].translation, nodes[n].rotation, nodes[n].scale);
            lod.group = Arc::new(crate::drawing::lod::measured(Some(assets), &group, || crate::drawing::lod::tree_shapes(group.levels.len(), lod.node, &parent, &placing, &drawn)));
        }
    }
    Ok(Hierarchy { nodes, lods: groups })
}
