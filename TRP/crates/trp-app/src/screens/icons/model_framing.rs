//! Models for rounds, grenades and STALKER items, and the camera's framing.

use super::*;

impl Worker {
    /// A round's model, as a fired case (`AmmoPoolObject.SetUsed(true)`:
    /// its objects named `shell` on, those named `full` off) or as a round
    /// in a magazine (`SetUsed(false)`: the other way about).
    pub(crate) fn round_model(&mut self, template: &str, used: bool) -> anyhow::Result<Model> {
        let item = self.db.item(template)?;
        let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("it has no model"))?.to_owned();
        let (mut hierarchy, pose) = trp_data::interface::icon::model_of(&mut self.unity, &key)?;
        for node in &mut hierarchy.nodes {
            if node.name.contains("shell") {
                node.active = used;
            } else if node.name.contains("full") {
                node.active = !used;
            }
        }
        Ok(self.model_from(hierarchy, false, pose, None))
    }

    /// A round's calibre's class for its case's sounds (`AmmoPoolObject.
    /// _caliber`); none for anything else.
    pub(crate) fn round_caliber(&mut self, template: &str) -> Option<u8> {
        let item = self.db.items.get(template)?;
        if !self.db.is_a(item, "Ammo") {
            return None;
        }
        let key = item.prefab_key()?.to_owned();
        let files = self.unity.load(&key).ok()?;
        trp_data::weapons::weapon::round_caliber(&self.unity, &files)
    }

    /// A thrown grenade's own model (`GrenadeItself`, grenades-and-knife
    /// spec, "The throw"), by its item, and its prefab's figures for the
    /// throw.
    pub(crate) fn thrown_grenade(&mut self, template: &str) -> anyhow::Result<(Model, trp_data::weapons::grenade::GrenadeData)> {
        let item = self.db.item(template)?;
        let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("it has no prefab"))?.to_owned();
        let rig = trp_data::weapons::weapon::assemble_prefab(&mut self.unity, &key)?;
        let data = trp_data::weapons::grenade::GrenadeData::read(&self.unity, &rig)?;
        Ok((self.model_from(data.thrown.clone(), false, None, None), data))
    }

    /// A model from a tree: what each node draws, the parts of one's own
    /// weapon besides.
    pub(super) fn model_from(&mut self, mut hierarchy: Hierarchy, keeps_own: bool, pose: Option<Pose>, custom: Option<&trp_data::weapons::custom::Build>) -> Model {
        // The model's own top object is on whatever its prefab saved: the
        // game's pool sets it as it makes the object and switches it on to
        // show it (`AbstractAssetsPool.RunInitialStageBody`, `PopOrCreate`).
        // The M995 round's prefab is saved with it off, and was drawn as
        // nothing, in its picture and on the ground, without this.
        for node in hierarchy.nodes.iter_mut().filter(|node| node.parent.is_none()) {
            node.active = true;
        }
        // (With its materials' cubes, which the game's own surface reflects:
        // `game_surface`.)
        let mut looks = PreparedLooks::default().with_cubes();
        if self.pictures {
            looks = looks.raw_colours();
        }
        let mut parts = vec![Vec::new(); hierarchy.nodes.len()];
        let mut shadows = Vec::new();
        // Each node's place in the tree as it lies, for a mesh drawn on
        // bones.
        let mut world: Vec<Mat4> = Vec::with_capacity(hierarchy.nodes.len());
        for node in &hierarchy.nodes {
            let own = convert::transform(node.translation, node.rotation, node.scale).to_matrix();
            world.push(node.parent.map_or(own, |parent| world[parent] * own));
        }
        // What is on: a node and every node above it.
        let mut on = vec![false; hierarchy.nodes.len()];
        for (index, node) in hierarchy.nodes.iter().enumerate() {
            on[index] = node.active && node.parent.is_none_or(|parent| on[parent]);
            let Some(renderer) = node.renderer.as_ref().filter(|_| on[index]) else { continue };
            // A mesh on bones is drawn where they put it, not by its node.
            match looks.mesh_with_poses(&self.unity, renderer.mesh, !renderer.skinned && hierarchy.mirrored(index)) {
                Ok((meshes, poses)) => {
                    // A skinned mesh where its bones put it.
                    let bones: Option<Vec<usize>> = renderer.bones.iter().map(|bone| bone.filter(|bone| *bone < hierarchy.nodes.len())).collect();
                    if let Some(bones) = bones.filter(|bones| renderer.skinned && !bones.is_empty() && bones.len() == poses.len()) {
                        let into = world[index].inverse();
                        let matrices: Vec<Mat4> = bones.iter().zip(&poses).map(|(bone, pose)| into * world[*bone] * *pose).collect();
                        meshes.iter().for_each(|mesh| looks.pose_on_bones(*mesh, &matrices));
                    }
                    // A part whose material cannot be read is left out of
                    // the picture: drawn, it is the stand-in's magenta.
                    for (slot, mesh) in meshes.into_iter().enumerate() {
                        let material = looks.material(&self.unity, renderer.materials.get(slot).copied().flatten());
                        if renderer.shadows_only {
                            shadows.push((index, mesh, material));
                        } else if !looks.is_fallback(material) {
                            parts[index].push((mesh, material));
                        } else {
                            // (Said, for the sweep of every picture.)
                            debug!("{}: its submesh {slot} is left out, its material ({:?}) cannot be read", node.name, renderer.materials.get(slot).copied().flatten());
                        }
                    }
                }
                Err(e) => warn!("mesh on {}: {e:#}", node.name),
            }
        }
        let mut extra = Vec::new();
        if let Some(custom) = custom {
            let materials = looks.custom_materials(&custom.materials);
            for part in &custom.parts {
                for (slot, mesh) in looks.custom_mesh(&part.mesh).into_iter().enumerate() {
                    if let Some(material) = part.materials.get(slot).and_then(|index| materials.get(*index)) {
                        extra.push((part.node, mesh, *material));
                    }
                }
            }
        }
        // For the world, the coarser levels of detail too (`lod_view`), with
        // either the characters' or the world's things' on (a model is
        // shared by a hung weapon and a thing on the ground): a picture's
        // model draws the fullest alone, as before.
        let mut lower = vec![Vec::new(); hierarchy.nodes.len()];
        if !self.pictures && crate::assets::lod_view::any_on() {
            for (index, node) in hierarchy.nodes.iter().enumerate() {
                let Some(renderer) = node.lower.as_ref().filter(|renderer| on[index] && !renderer.skinned && !renderer.shadows_only) else { continue };
                match looks.mesh_with_poses(&self.unity, renderer.mesh, hierarchy.mirrored(index)) {
                    Ok((meshes, _)) => {
                        for (slot, mesh) in meshes.into_iter().enumerate() {
                            let material = looks.material(&self.unity, renderer.materials.get(slot).copied().flatten());
                            if !looks.is_fallback(material) {
                                lower[index].push((mesh, material));
                            }
                        }
                    }
                    Err(e) => warn!("a coarser level's mesh on {}: {e:#}", node.name),
                }
            }
        }
        Model { hierarchy, keeps_own, looks, parts, extra, shadows, pose, devices: Vec::new(), lower }
    }

    /// An outfit of the other game's, by its own model of the item as it
    /// lies in the world (`docs/spec/icons.md`, "an item of the other
    /// game's"): its meshes with their pictures, read loose or from the
    /// game's packed archives, hung from one node of no turn.
    pub(super) fn stalker_model(&mut self, template: &str, world: &str) -> anyhow::Result<Model> {
        let (bytes, _) = self.stalker.read(&format!("meshes/{world}"))?;
        let model = trp_data::formats::xray::read(&bytes)?;
        let mut looks = PreparedLooks::default();
        let mut extra = Vec::new();
        for part in &model.parts {
            let picture = match self.stalker.read(&format!("textures/{}.dds", part.texture)).and_then(|(bytes, _)| trp_data::formats::xray::read_picture(&bytes)) {
                Ok(picture) => picture,
                Err(e) => {
                    warn!("the picture of {template}: a mesh of {world} is left out: {e:#}");
                    continue;
                }
            };
            let mesh = MeshData { name: part.texture.clone(), positions: part.positions.clone(), normals: part.normals.clone(), uv0: part.uv.clone(), submeshes: vec![part.triangles.clone()], ..default() };
            let material = looks.stalker_material(&picture);
            for mesh in looks.custom_mesh(&mesh) {
                extra.push((0, mesh, material));
            }
        }
        let node = Node {
            name: world.to_owned(),
            parent: None,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            active: true,
            renderer: None,
            scripts: Vec::new(),
            script_objects: Vec::new(),
            grip: None,
            placer: None,
            transform: ObjRef { file: usize::MAX, path_id: 0 },
            colliders: Vec::new(),
            bounded: None,
            lower: None,
            lod: None,
        };
        let hierarchy = Hierarchy { nodes: vec![node], ..Hierarchy::default() };
        Ok(Model { hierarchy, keeps_own: false, looks, parts: Vec::new(), extra, shadows: Vec::new(), pose: Some(LAID_FACE_UP), devices: Vec::new(), lower: Vec::new() })
    }

    /// A model posed and framed for its picture of `cells`: each node in
    /// the studio with the pose's turn on the whole, and the camera's half
    /// height from the bounds of what is drawn. None for a model with no
    /// renderer switched on: the game's bounds of it are of no size
    /// (`WeaponPreview.GetBounds`, `default(Bounds)`), and its camera draws
    /// nothing, an empty picture that it keeps and shows.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn framed(
        &mut self,
        template: &str,
        cells: [u32; 2],
        hierarchy: Hierarchy,
        keeps_own: bool,
        mut looks: PreparedLooks,
        (mut parts, extra, shadows): (Vec<Vec<(usize, usize)>>, Vec<(usize, usize, usize)>, &[(usize, usize, usize)]),
        pose: Option<Pose>,
    ) -> anyhow::Result<Option<PreparedIcon>> {
        parts.resize(hierarchy.nodes.len(), Vec::new());
        let pose = pose.unwrap_or(Pose { rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3], has_offset: false, bounds_scale: 1.0 });
        let turn = convert::rotation(pose.rotation);
        let mut world: Vec<Mat4> = Vec::with_capacity(hierarchy.nodes.len());
        for (index, node) in hierarchy.nodes.iter().enumerate() {
            let own = if index == 0 && !keeps_own { Transform::from_scale(Vec3::from(node.scale)) } else { convert::transform(node.translation, node.rotation, node.scale) };
            let above = node.parent.map_or(Mat4::from_quat(turn), |parent| world[parent]);
            world.push(above * own.to_matrix());
        }
        // The bounds (`WeaponPreview.GetBounds`): each renderer's own box
        // (`Renderer.bounds`: its whole mesh's, every submesh's together,
        // its corners through its object's place), on objects that are on,
        // but lenses and the muzzle's flash. A renderer that is switched on
        // counts whether or not it draws here: one that casts its shadow
        // alone (a backpack's `_SHADOW`), and a lower level of an LOD group
        // (`AR_bagarii_lod1`). Boxed by submesh, a turned
        // model's bounds were tighter than the game's: the Bagariy's 0.62 m
        // across for its 0.75, its picture a fifth too large and cut.
        let mut boxes: Vec<Option<(Vec3, Vec3)>> = vec![None; hierarchy.nodes.len()];
        let grow = |boxes: &mut Vec<Option<(Vec3, Vec3)>>, node: usize, (from, to): (Vec3, Vec3)| {
            if from.x <= to.x {
                boxes[node] = Some(boxes[node].map_or((from, to), |(f, t)| (f.min(from), t.max(to))));
            }
        };
        let mesh_box = |mesh: usize| -> (Vec3, Vec3) {
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) = looks.meshes[mesh].attribute(Mesh::ATTRIBUTE_POSITION) else { return (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)) };
            points.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(from, to), p| (from.min(Vec3::from(*p)), to.max(Vec3::from(*p))))
        };
        for (index, meshes) in parts.iter().enumerate() {
            for (mesh, _) in meshes {
                grow(&mut boxes, index, mesh_box(*mesh));
            }
        }
        for (node, mesh, _) in extra.iter().chain(shadows) {
            grow(&mut boxes, *node, mesh_box(*mesh));
        }
        let mut on = vec![false; hierarchy.nodes.len()];
        let mut renderers_on = !extra.is_empty();
        for (index, node) in hierarchy.nodes.iter().enumerate() {
            on[index] = node.active && node.parent.is_none_or(|parent| on[parent]);
            renderers_on |= on[index] && (node.renderer.is_some() || node.bounded.is_some());
            let Some(mesh) = node.bounded.filter(|_| on[index]) else { continue };
            // Its mesh's own box as the asset keeps it (`m_LocalAABB`).
            let aabb = self.unity.read(mesh).ok().and_then(|value| {
                let aabb = value.get("m_LocalAABB")?;
                Some((aabb.get("m_Center")?.as_vec3()?, aabb.get("m_Extent")?.as_vec3()?))
            });
            if let Some((center, extent)) = aabb {
                let (a, b) = (convert::position([center[0] - extent[0], center[1] - extent[1], center[2] - extent[2]]), convert::position([center[0] + extent[0], center[1] + extent[1], center[2] + extent[2]]));
                grow(&mut boxes, index, (a.min(b), a.max(b)));
            }
        }
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (index, node) in hierarchy.nodes.iter().enumerate() {
            let Some((from, to)) = boxes[index].filter(|_| !node.name.contains("linza") && node.name != "MuzzleJetCombinedMesh") else { continue };
            for corner in 0..8 {
                let c = Vec3::new(if corner & 1 == 0 { from.x } else { to.x }, if corner & 2 == 0 { from.y } else { to.y }, if corner & 4 == 0 { from.z } else { to.z });
                let at = world[index].transform_point3(c);
                (low, high) = (low.min(at), high.max(at));
            }
        }
        // (A renderer switched on with nothing bounded is the app's own
        // failure, a mesh it could not read: said, and not kept.)
        if !renderers_on {
            return Ok(None);
        }
        anyhow::ensure!(low.x <= high.x, "nothing of it is drawn");
        looks.unskinned();
        // The camera's half height: the bounds' over the bounds' scale, or
        // their half width over the picture's shape where they are wider
        // than it (`ItemIconCreator.CenterByBounds`).
        let size = [cells[0] * CELL_PIXELS + 1, cells[1] * CELL_PIXELS + 1];
        let aspect = size[0] as f32 / size[1] as f32;
        let half = (high - low) / 2.0;
        let half_height = if half.x / half.y.max(1e-6) > aspect { half.x / aspect } else { half.y } / pose.bounds_scale;
        debug!("the picture of {template}: bounds {low} to {high}, half {half}, its camera's half height {half_height} ({cells:?} cells, {:?})", pose.bounds_scale);
        let offset = if pose.has_offset { convert::position(pose.position) } else { Vec3::ZERO };
        let place = -(low + high) / 2.0 + offset;
        let picture_bytes = looks.picture_bytes();
        Ok(Some(PreparedIcon { template: template.to_owned(), size, hierarchy, keeps_own, looks, parts, extra, turn, place, half_height, picture_bytes, studio: self.studio.take() }))
    }
}
