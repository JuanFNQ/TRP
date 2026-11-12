//! Clothing put on the body.

use super::*;

/// Reads the suits asked for, one at a time, and puts on each as it is had.
#[allow(clippy::too_many_arguments)]
pub(super) fn wear(
    mut commands: Commands,
    clothing: Option<ResMut<Clothing>>,
    parts: Res<BodyParts>,
    rig: Option<Res<BodyRig>>,
    weapon: Option<Res<WeaponRig>>,
    mut figure: Option<ResMut<crate::screens::figure_view::Figure>>,
    (corpse, uploads): (Option<Res<crate::screens::figure_view::CorpseSkeleton>>, Res<crate::weapons::weapon::UploadWatch>),
    (mut meshes, mut images, mut materials, mut bindposes): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>, ResMut<Assets<SkinnedMeshInverseBindposes>>),
    pieces: Query<(Entity, &SuitPiece), crate::platform::park::EvenParked>,
    twins: Query<(Entity, &crate::screens::figure_view::Twin), crate::platform::park::EvenParked>,
    corpse_twins: Query<(Entity, &crate::player::death::CorpseTwin), crate::platform::park::EvenParked>,
    hands: Query<(Entity, &ChildOf), With<FirstPersonHands>>,
) {
    let (Some(mut clothing), Some(weapon)) = (clothing, weapon) else { return };
    let clothing = &mut *clothing;
    // A suit made ready: into the stores.
    let done = clothing.task.as_mut().and_then(|(_, task)| block_on(poll_once(task)));
    if let Some((worker, made)) = done {
        let (at, _) = clothing.task.take().expect("a task was polled");
        clothing.worker = worker;
        match made {
            Ok(prepared) => {
                let loaded = into_stores(prepared, &mut meshes, &mut images, &mut materials, &mut bindposes);
                info!("the suit {}: read", clothing.suits[at].name);
                // Waited for on the graphics card before it is put on: the
                // pictures go up a few a frame (`UPLOAD_BUDGET`), and a piece
                // whose pictures are not there is not drawn, which left the
                // body, its copies and the dummies without the suit for those
                // frames.
                let parts: Vec<&LoadedPart> = loaded.body.iter().chain(&loaded.hands).chain(&loaded.feet).collect();
                let shapes: Vec<AssetId<Mesh>> = parts.iter().flat_map(|part| &part.meshes).flat_map(|(_, submeshes, _)| submeshes.iter().map(Handle::id)).collect();
                let pictures: Vec<AssetId<Image>> = parts
                    .iter()
                    .flat_map(|part| &part.materials)
                    .filter_map(|material| materials.get(material))
                    .flat_map(|m| [&m.base_color_texture, &m.normal_map_texture, &m.metallic_roughness_texture, &m.occlusion_texture, &m.emissive_texture].into_iter().flatten().map(Handle::id).collect::<Vec<_>>())
                    .collect();
                uploads.wait(format!("{SUIT_WAITED}{at}"), pictures, shapes);
                clothing.loaded.insert(at, loaded);
            }
            Err(e) => {
                warn!("the suit {} could not be read, and is not worn: {e:#}", clothing.suits[at].name);
                clothing.failed.push(at);
                for role in [SuitRole::Upper, SuitRole::Lower] {
                    if clothing.wanted[slot(role)] == Some(at) {
                        clothing.wanted[slot(role)] = clothing.worn[slot(role)];
                    }
                }
            }
        }
    }
    for name in uploads.take(|name| name.starts_with(SUIT_WAITED)) {
        if let Some(at) = name.strip_prefix(SUIT_WAITED).and_then(|at| at.parse().ok()) {
            clothing.on_card.insert(at);
        }
    }
    // Each suit asked for, had and on the card: put on.
    let mut changed = false;
    for role in [SuitRole::Upper, SuitRole::Lower] {
        let Some(at) = clothing.wanted[slot(role)].filter(|at| clothing.worn[slot(role)] != Some(*at)) else { continue };
        let Some(loaded) = clothing.loaded.get(&at).filter(|_| clothing.on_card.contains(&at)) else { continue };
        // What it takes the place of goes, with its copies on the models
        // and the corpse.
        let gone: Vec<Entity> = pieces.iter().filter(|(_, piece)| piece.0 == role).map(|(entity, _)| entity).collect();
        for (entity, twin) in &twins {
            if gone.contains(&twin.0) {
                commands.entity(entity).despawn();
            }
        }
        for (entity, twin) in &corpse_twins {
            if gone.contains(&twin.0) {
                commands.entity(entity).despawn();
            }
        }
        for entity in gone {
            commands.entity(entity).despawn();
        }
        let levels = match role {
            SuitRole::Upper => &loaded.body,
            SuitRole::Lower => &loaded.feet,
        };
        let root = rig.as_ref().map(|rig| rig.root());
        let mut made_meshes: Vec<Handle<Mesh>> = Vec::new();
        let mut made_materials: Vec<Handle<StandardMaterial>> = Vec::new();
        for (part, root) in levels.iter().filter_map(|part| Some((part, root?))) {
            let joints: Vec<Entity> = part.bone_paths.iter().map(|path| joint_of(rig.as_deref(), &parts, &weapon, path)).collect();
            for (case, submeshes, poses) in &part.meshes {
                // Its hooded form's mesh for the same case, a second form
                // (`body_view::fit_head_things`, `fit::HOOD_UP`).
                let hooded = part.hood.as_ref().and_then(|hood| hood.iter().find(|(of, _, _)| of == case)).map(|(_, seconds, hood_poses)| (crate::player::body_view::second_form(submeshes.len(), seconds), hood_poses));
                for (slot, mesh) in submeshes.iter().enumerate() {
                    let Some(material) = part.materials.get(slot) else { continue };
                    let fitted = hooded.as_ref().and_then(|(seconds, hood_poses)| {
                        let other = seconds.get(slot)?.clone();
                        Some(crate::player::body_view::Fitted { wearer: crate::player::body_view::Wearer::Character, kind: trp_data::character::dress::fit::HOOD_UP, meshes: [mesh.clone(), other], poses: Some([poses.clone(), (*hood_poses).clone()]) })
                    });
                    let piece = commands
                        .spawn((
                            Name::new("Clothing, a suit chosen"),
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            SkinnedMesh { inverse_bindposes: poses.clone(), joints: joints.clone() },
                            NoFrustumCulling,
                            RenderLayers::layer(SEEN),
                            OwnBody { top: role == SuitRole::Upper },
                            SuitPiece(role),
                            crate::platform::park::Park,
                        ))
                        .id();
                    match role {
                        SuitRole::Upper => commands.entity(piece).insert(UpperBody),
                        SuitRole::Lower => commands.entity(piece).insert(LowerBody),
                    };
                    if let Some(case) = case {
                        commands.entity(piece).insert((TorsoPiece(*case), Visibility::Hidden));
                    }
                    commands.entity(root).add_child(piece);
                    let figure_copy = figure.as_ref().map(|figure| figure.skeleton.skinned(&mut commands, piece, (mesh, material), poses, &part.bone_paths, true));
                    // (And the OVERALL tab's own figure's, `OVERALL FIGURE`.)
                    let overall_copy = figure.as_ref().and_then(|figure| figure.overall_skeleton()).map(|overall| overall.skinned(&mut commands, piece, (mesh, material), poses, &part.bone_paths, true));
                    let corpse_copy = corpse.as_ref().map(|corpse| corpse.0.skinned(&mut commands, piece, (mesh, material), poses, &part.bone_paths, true));
                    // Its levels of detail, on it and its copies (`lod_view`).
                    if let Some(lod) = &part.lod {
                        for tagged in [Some(piece), figure_copy, overall_copy, corpse_copy].into_iter().flatten() {
                            commands.entity(tagged).insert(lod.clone());
                        }
                    }
                    if let Some(fitted) = &fitted {
                        commands.entity(piece).insert(fitted.clone());
                        for copy in [figure_copy, overall_copy].into_iter().flatten() {
                            commands.entity(copy).insert(fitted.of(crate::player::body_view::Wearer::Figure));
                        }
                        if let Some(copy) = corpse_copy {
                            // (A hood closes round the corpse's eyes: as its
                            // head, a shadow for them, `body_view::spawn_body`.)
                            commands.entity(copy).insert((fitted.of(crate::player::body_view::Wearer::Corpse), crate::player::death::CorpseHead));
                        }
                        made_meshes.push(fitted.meshes[1].clone());
                    }
                    made_meshes.push(mesh.clone());
                    made_materials.push(material.clone());
                }
            }
        }
        // An upper suit's hands, for the character's own eyes, in place of
        // those it had (where it names any).
        if let (SuitRole::Upper, false) = (role, loaded.hands.is_empty()) {
            let parent = hands.iter().next().map(|(_, child_of)| child_of.parent()).unwrap_or(weapon.root);
            for (entity, _) in &hands {
                commands.entity(entity).despawn();
            }
            clothing.hands = clothing.suits[at].hands.clone();
            for new in &loaded.hands {
                let joints: Vec<Entity> = new.bone_paths.iter().map(|path| joint_of(rig.as_deref(), &parts, &weapon, path)).collect();
                for (_, submeshes, poses) in &new.meshes {
                    for (slot, mesh) in submeshes.iter().enumerate().filter(|(slot, _)| !new.glass.get(*slot).copied().unwrap_or(false)) {
                        let Some(material) = new.materials.get(slot) else { continue };
                        let piece = commands
                            .spawn((
                                Name::new("Arms, a suit chosen"),
                                FirstPersonHands,
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(material.clone()),
                                SkinnedMesh { inverse_bindposes: poses.clone(), joints: joints.clone() },
                                NoFrustumCulling,
                                // As the game's hands: no shadow (`weapon::spawn_weapon`).
                                bevy::light::NotShadowCaster,
                            ))
                            .id();
                        commands.entity(parent).add_child(piece);
                    }
                }
            }
        }
        if let Some(figure) = figure.as_mut() {
            figure.wait_for(&made_meshes, &made_materials);
        }
        info!("the suit {} put on", clothing.suits[at].name);
        clothing.worn[slot(role)] = Some(at);
        clothing.bits[slot(role)] = Some(loaded.bits);
        // A lower suit's legs bring their own holster, or none
        // (`PlayerBody.Init` with the new legs: `slung_view::hang`).
        if role == SuitRole::Lower {
            clothing.holster = Some(loaded.holster);
        }
        changed = true;
    }
    if changed {
        if let Some(file) = &clothing.file {
            let id = |role: SuitRole| clothing.worn[slot(role)].map(|at| clothing.suits[at].id.as_str());
            keep(file, [id(SuitRole::Upper), id(SuitRole::Lower)]);
        }
    }
    // The next asked for that is not had: read, if nothing is being read.
    if clothing.task.is_none() {
        let next = [SuitRole::Upper, SuitRole::Lower]
            .into_iter()
            .filter_map(|role| clothing.wanted[slot(role)].filter(|at| clothing.worn[slot(role)] != Some(*at)))
            .find(|at| !clothing.loaded.contains_key(at) && !clothing.failed.contains(at));
        match next {
            Some(at) => {
                let (worker, install, suit, fitting) = (clothing.worker.take(), clothing.install.clone(), clothing.suits[at].clone(), clothing.fitting);
                let task = AsyncComputeTaskPool::get().spawn(async move {
                    // (The cubemaps' bundle open for the OVERALL figure's
                    // own surface: `glass_view::open_cubes`.)
                    let reader = || {
                        UnityAssets::new(install).map(|mut unity| {
                            crate::effects::glass_view::open_cubes(&mut unity);
                            unity
                        })
                    };
                    match worker.map_or_else(reader, Ok) {
                        Ok(mut unity) => {
                            let made = prepare(&mut unity, &suit, fitting);
                            (Some(unity), made)
                        }
                        Err(e) => (None, Err(e)),
                    }
                });
                clothing.task = Some((at, task));
            }
            // Nothing left to read: the thread's reader goes, away from the
            // frame.
            None => {
                if let Some(worker) = clothing.worker.take() {
                    AsyncComputeTaskPool::get().spawn(async move { drop(worker) }).detach();
                }
            }
        }
    }
}
