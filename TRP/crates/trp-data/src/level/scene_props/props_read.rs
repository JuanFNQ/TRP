//! Reading a scene's props: their nodes, meshes and materials.

use super::*;

/// Reads the object at `path` (its names from the scene's top, joined by
/// `/`) of the scene whose path in the build ends with `scene`, and all
/// under it that is on, skipping an object with a script of a class in
/// `skip`, or whose name begins with one of `left_out` (and all under it);
/// with `keep` named, only those objects (their paths under the one read)
/// and all under them, and the objects on the way to them. With `levels`,
/// every level of detail's renderers, tagged (`PropNode::lod`); else the
/// nearest level's alone.
pub fn read(install: &Install, scene: &str, path: &str, skip: &[&str], left_out: &[&str], keep: &[&str], levels: bool) -> Result<Props> {
    let index = scene_paths(install)?.iter().position(|p| p.ends_with(scene)).with_context(|| format!("no scene {scene} in the build"))?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true).with_context(|| file.clone())?;
    let top = (0..scene.nodes.len()).find(|&node| scene.path(node) == path).with_context(|| format!("no object {path} in {file}"))?;
    let mesh_tree = tree_of(install, TREES, class::MESH).context("a mesh's tree")?;
    let material_tree = tree_of(install, TREES, class::MATERIAL).context("a material's tree")?;
    let mut pictures = ScenePictures::new(install)?;
    let folder = install.root.join("EscapeFromTarkov_Data");
    let stream = |name: &str, path: &str, offset: usize, size: usize| -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let beside = folder.join(path.rsplit('/').next().unwrap_or(path));
        let mut file = std::fs::File::open(&beside).with_context(|| format!("{name}: {}", beside.display()))?;
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut data = vec![0; size];
        file.read_exact(&mut data).with_context(|| format!("{name}: {}", beside.display()))?;
        Ok(data)
    };
    let mut props = Props { root: (scene.world_position(top), scene.world_rotation(top)), ..Default::default() };
    // Levels of detail: every group under the object read (the object
    // itself among them), by the scene's object it is on. Without every
    // level asked for, the renderers of every level but the nearest are not
    // drawn.
    let mut groups: Vec<(usize, crate::drawing::lod::LodGroup)> = Vec::new();
    let mut farther: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for node in (0..scene.nodes.len()).filter(|&node| scene.path(node).starts_with(path)) {
        if let Some(bytes) = scene.component(node, LOD_GROUP) {
            match lod_group(bytes) {
                Ok(group) => {
                    if !group.governs_nothing() {
                        farther.extend(group.levels.iter().skip(1).flat_map(|level| &level.renderers).filter(|pointer| pointer.file == 0).map(|pointer| pointer.path_id));
                        groups.push((node, group));
                    }
                }
                Err(e) => props.skipped.push(format!("{}'s levels of detail: {e:#}", scene.path(node))),
            }
        }
    }
    // Each renderer's group and levels (the scene's own renderers only).
    let levels_of = |id: i64| -> Option<(usize, u8)> {
        groups.iter().enumerate().find_map(|(at, (_, group))| {
            let bits = group.levels_of(crate::unity::ObjRef { file: 0, path_id: id });
            (bits != 0).then_some((at, bits))
        })
    };
    // The scene's objects read, by their places in `props.nodes`.
    let mut read_at: HashMap<usize, usize> = HashMap::new();
    // Meshes read whole by their file and number (a joined mesh is many
    // objects' own), materials and pictures by theirs.
    let mut whole: HashMap<(String, i64), MeshData> = HashMap::new();
    let mut materials: HashMap<(String, i64), usize> = HashMap::new();
    let mut pictures_read: HashMap<(String, i64), Option<usize>> = HashMap::new();
    let file_of = |pointer: (i32, i64)| -> Result<String> {
        if pointer.0 == 0 {
            return Ok(file.clone());
        }
        let listed = scene.externals.get(pointer.0 as usize - 1).with_context(|| format!("no file {} among {file}'s", pointer.0))?;
        Ok(listed.rsplit('/').next().unwrap_or(listed).to_owned())
    };
    // What is kept, where only some is: each kept object's path under the
    // one read, and the paths on the way to it.
    let top_path = scene.path(top);
    let kept = |node: usize| -> bool {
        if keep.is_empty() || node == top {
            return true;
        }
        let under = scene.path(node);
        let under = under.strip_prefix(&top_path).map(|rest| rest.trim_start_matches('/')).unwrap_or("");
        keep.iter().any(|wanted| under == *wanted || under.starts_with(&format!("{wanted}/")) || wanted.starts_with(&format!("{under}/")))
    };
    let mut stack = vec![(top, None)];
    while let Some((node, parent)) = stack.pop() {
        let own = &scene.nodes[node];
        if !own.active || !kept(node) || skip.iter().any(|class| scene.behaviour(node, class).is_some()) || left_out.iter().any(|name| own.name.starts_with(name)) {
            continue;
        }
        let local = scene.local(node).unwrap_or(crate::interface::screen::Local { rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3], scale: [1.0; 3] });
        let at = props.nodes.len();
        let (translation, rotation) = if parent.is_none() { ([0.0; 3], [0.0, 0.0, 0.0, 1.0]) } else { (local.position, local.rotation) };
        let boxes = scene.component(node, BOX_COLLIDER).and_then(box_collider).into_iter().collect();
        props.nodes.push(PropNode { name: own.name.clone(), parent, translation, rotation, scale: if parent.is_none() { [1.0; 3] } else { local.scale }, drawn: Vec::new(), in_root: false, boxes, lod: None });
        read_at.insert(node, at);
        for &child in own.children.iter().rev() {
            stack.push((child, Some(at)));
        }
        let (Some(filter), Some(renderer)) = (scene.component(node, MESH_FILTER), scene.component(node, class::MESH_RENDERER)) else { continue };
        let what = scene.path(node);
        let made = (|| -> Result<()> {
            let renderer = Renderer::read(renderer).context("its renderer")?;
            // Off, one that casts only a shadow: not drawn; one of a farther
            // level of detail only with every level asked for.
            let id = scene.component_id(node, class::MESH_RENDERER);
            let farther_level = id.is_some_and(|id| farther.contains(&id));
            if !renderer.enabled || renderer.shadow_only || (farther_level && !levels) {
                return Ok(());
            }
            if levels {
                props.nodes[at].lod = id.and_then(levels_of).map(|(group, levels)| crate::unity::extract::NodeLod { group, levels });
            }
            let mut r = Reader::new(filter.get(12..).context("a short mesh filter")?);
            let mesh = (r.i32()?, r.i64()?);
            let mesh_key = (file_of(mesh)?, mesh.1);
            if !whole.contains_key(&mesh_key) {
                let value = pictures.read_by(&mesh_key.0, mesh_key.1, class::MESH, &mesh_tree)?;
                whole.insert(mesh_key.clone(), mesh_streamed(&value, &stream)?);
            }
            let data = &whole[&mesh_key];
            // A joined mesh's run of parts, in the world's axes; else the
            // mesh's own parts.
            let joined = renderer.batch_count > 0;
            let parts: Vec<usize> = if joined { (renderer.batch_first as usize..(renderer.batch_first + renderer.batch_count) as usize).collect() } else { (0..data.submeshes.len()).collect() };
            for (n, part) in parts.into_iter().enumerate() {
                let Some(material) = renderer.materials.get(n).or(renderer.materials.last()).copied() else { continue };
                let Some(indices) = data.submeshes.get(part) else { bail!("part {part} of a mesh of {}", data.submeshes.len()) };
                let mut mesh = part_of(data, indices);
                if joined {
                    to_root(&mut mesh, props.root);
                }
                let material_key = (file_of(material)?, material.1);
                let material = match materials.get(&material_key) {
                    Some(&at) => at,
                    None => {
                        let value = pictures.read_by(&material_key.0, material_key.1, class::MATERIAL, &material_tree)?;
                        let mut picture = |name: &str| -> Option<usize> {
                            let pointer = texture_pointer(&value, name)?;
                            let picture_file = pictures.file_name(&material_key.0, pointer.0).ok()?;
                            let key = (picture_file, pointer.1);
                            if let Some(read) = pictures_read.get(&key) {
                                return *read;
                            }
                            let read = match pictures.texture(&key.0, key.1) {
                                Ok(texture) => {
                                    props.pictures.push(texture);
                                    Some(props.pictures.len() - 1)
                                }
                                Err(e) => {
                                    props.skipped.push(format!("{name} of {}: {e:#}", value.get("m_Name").and_then(Value::as_str).unwrap_or("?")));
                                    None
                                }
                            };
                            pictures_read.insert(key, read);
                            read
                        };
                        let (base, normal, gloss) = (picture("_MainTex"), picture("_BumpMap"), picture("_SpecMap"));
                        props.materials.push(material_of(&value, base, normal, gloss));
                        materials.insert(material_key, props.materials.len() - 1);
                        props.materials.len() - 1
                    }
                };
                props.meshes.push(mesh);
                props.nodes[at].drawn.push((props.meshes.len() - 1, material));
            }
            props.nodes[at].in_root = joined;
            Ok(())
        })();
        if let Err(e) = made {
            props.skipped.push(format!("{what}: {e:#}"));
        }
    }
    // The groups kept, each by the object read it is on; a group whose
    // object was not read (left out, or off) governs nothing drawn here:
    // its renderers' tags are dropped and they are drawn at any distance.
    if levels {
        let mut kept: Vec<Option<usize>> = Vec::with_capacity(groups.len());
        for (node, group) in groups {
            kept.push(read_at.get(&node).map(|&at| {
                props.lods.push(crate::unity::extract::HierarchyLod { node: at, group: std::sync::Arc::new(group) });
                props.lods.len() - 1
            }));
        }
        for node in &mut props.nodes {
            node.lod = node.lod.and_then(|lod| Some(crate::unity::extract::NodeLod { group: kept.get(lod.group).copied().flatten()?, levels: lod.levels }));
        }
        // TRP's measure of each group (`lod::measured`): the meshes its
        // objects draw, placed in the axes of the object it is on.
        for g in 0..props.lods.len() {
            let (group_node, group) = (props.lods[g].node, props.lods[g].group.clone());
            let mut drawn = Vec::new();
            for (index, node) in props.nodes.iter().enumerate() {
                let Some(lod) = node.lod.filter(|lod| lod.group == g) else { continue };
                for &(mesh, _) in &node.drawn {
                    let data = &props.meshes[mesh];
                    drawn.push((index, lod.levels, node.in_root, std::sync::Arc::new(crate::drawing::lod::MeshPoints { points: data.positions.clone(), triangles: data.submeshes.clone() })));
                }
            }
            let parent = |n: usize| props.nodes[n].parent;
            let placing = |n: usize| (props.nodes[n].translation, props.nodes[n].rotation, props.nodes[n].scale);
            let measured = crate::drawing::lod::measured(None, &group, || crate::drawing::lod::tree_shapes(group.levels.len(), group_node, &parent, &placing, &drawn));
            props.lods[g].group = std::sync::Arc::new(measured);
        }
    }
    Ok(props)
}
