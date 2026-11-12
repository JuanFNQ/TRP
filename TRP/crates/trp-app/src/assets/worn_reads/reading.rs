//! What is worn now, the worn pieces read in, and the next one asked.

use super::*;

/// The templates worn in the slots the game draws, by the character (and
/// what `--wear` named) and by each dummy; and every thing fitted to them.
fn worn_now(carried: &crate::screens::inventory_view::Carried) -> (Vec<String>, HashSet<String>) {
    let bodies = (0..carried.bodies.len()).filter_map(|which| carried.body(which));
    let mut worn: Vec<String> = carried.fixed_wear.clone();
    let mut fitted = HashSet::new();
    for gear in std::iter::once(&carried.equipment.item).chain(bodies) {
        for item in trp_sim::items::inventory::WORN.iter().filter_map(|slot| gear.in_slot(slot)) {
            if !worn.contains(&item.kind.template) {
                worn.push(item.kind.template.clone());
            }
            let mut below: Vec<&trp_sim::items::inventory::Item> = item.slots.iter().flatten().collect();
            while let Some(part) = below.pop() {
                fitted.insert(part.kind.template.clone());
                below.extend(part.slots.iter().flatten());
            }
        }
    }
    (worn, fitted)
}

/// Reads each thing worn that the run has not read, one at a time, and
/// hangs each on the body and its copies once it is on the graphics card.
#[allow(clippy::too_many_arguments)]
pub fn read_worn(
    mut commands: Commands,
    reads: Option<ResMut<WornReads>>,
    carried: Res<crate::screens::inventory_view::Carried>,
    mut parts: ResMut<BodyParts>,
    (rig, weapon, uploads, options): (Option<Res<BodyRig>>, Option<Res<crate::weapons::weapon::WeaponRig>>, Res<crate::weapons::weapon::UploadWatch>, Res<crate::Options>),
    (mut figure, mut corpse, mut dummies, mut models): (
        Option<ResMut<crate::screens::figure_view::Figure>>,
        Option<ResMut<crate::screens::figure_view::CorpseSkeleton>>,
        Option<ResMut<crate::world::dummy::DummySkeletons>>,
        Option<ResMut<HeadModels>>,
    ),
    (mut meshes, mut images, mut materials, mut bindposes): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>, ResMut<Assets<SkinnedMeshInverseBindposes>>),
) {
    let (Some(mut reads), Some(rig), Some(weapon), Some(mut models)) = (reads, rig, weapon, models.take()) else { return };
    let reads = &mut *reads;
    if !reads.begun {
        reads.begun = true;
        reads.fitting = parts.fitting;
        reads.armband = parts.armband;
        let read: Vec<String> = parts.item.iter().flatten().map(|piece| piece.item.clone()).chain(models.items().map(str::to_owned)).collect();
        reads.known.extend(read);
        reads.known.extend(reads.db.outfits.keys().cloned());
    }

    // A run played reads one thing at a time, away from the frame, and takes
    // it on the frame it comes; an unattended run (a capture, a scripted
    // replay) reads every thing on the frame it is put on, one after the
    // other (`background`), so that what it shows never hangs on the thread.
    let wait = options.unattended();
    // What an unattended run read on this frame: hung on it, not waited for
    // on the card, as a thing read at the start was shown on the frame it
    // was put on (drawn once the renderer has it).
    let mut hung_now: Vec<String> = Vec::new();
    loop {
        if reads.task.is_none() && (carried.is_changed() || !reads.loaded.is_empty() || reads.worker.is_some()) {
            ask_next(reads, &carried);
        }
        // A thing made ready: into the stores, and waited for on the card.
        let Some((worker, template, made)) = reads.task.as_mut().and_then(|task| crate::assets::background::take(task, wait, "a worn thing")) else { break };
        reads.task = None;
        reads.worker = worker;
        match made {
            Ok(Prepared { shape, vestlike, looks }) => {
                let (made_meshes, made_materials, _, _) = looks.add_sharing(&mut meshes, &mut images, &mut materials, None);
                let pair = |(mesh, material): (usize, usize)| (made_meshes[mesh].clone(), made_materials[material].clone());
                let loaded = match shape {
                    PreparedShape::Skinned(pieces) => Loaded::Skinned(
                        pieces
                            .into_iter()
                            .map(|piece| LoadedPiece {
                                drawn: piece.meshes.iter().copied().zip(piece.materials.iter().copied()).map(pair).collect(),
                                poses: bindposes.add(SkinnedMeshInverseBindposes::from(piece.poses)),
                                bone_paths: piece.bone_paths,
                                over: piece.over,
                                lod: piece.lod.map(|(group, levels)| crate::assets::lod_view::LodPiece { group, anchor: None, levels }),
                                second: piece.second.map(|(kind, seconds, poses)| (kind, seconds.into_iter().map(|at| made_meshes[at].clone()).collect(), bindposes.add(SkinnedMeshInverseBindposes::from(poses)))),
                                band: piece.band.map(|(sets, forms)| {
                                    let base = forms.first().map_or(0, |(meshes, _)| meshes.len());
                                    let forms = forms
                                        .into_iter()
                                        .map(|(at, poses)| {
                                            let made: Vec<Handle<Mesh>> = at.into_iter().map(|at| made_meshes[at].clone()).collect();
                                            (crate::player::body_view::second_form(base, &made), bindposes.add(SkinnedMeshInverseBindposes::from(poses)))
                                        })
                                        .collect();
                                    Arc::new(BandMeshes { sets, forms })
                                }),
                            })
                            .collect(),
                        vestlike,
                    ),
                    PreparedShape::Rigid(model, nodes, seconds) => Loaded::Rigid(
                        model,
                        nodes.into_iter().map(|node| node.into_iter().map(pair).collect()).collect(),
                        seconds.into_iter().map(|node| node.into_iter().map(|at| made_meshes[at].clone()).collect()).collect(),
                    ),
                };
                let drawn: Vec<&(Handle<Mesh>, Handle<StandardMaterial>)> = match &loaded {
                    Loaded::Skinned(pieces, _) => pieces.iter().flat_map(|piece| &piece.drawn).collect(),
                    Loaded::Rigid(_, nodes, _) => nodes.iter().flatten().collect(),
                };
                // The second forms are waited for with the first.
                let second_shapes: Vec<AssetId<Mesh>> = match &loaded {
                    Loaded::Skinned(pieces, _) => pieces.iter().flat_map(|piece| piece.second.iter().flat_map(|(_, meshes, _)| meshes.iter().map(Handle::id))).collect(),
                    Loaded::Rigid(_, _, seconds) => seconds.iter().flatten().map(Handle::id).collect(),
                };
                let shapes: Vec<AssetId<Mesh>> = drawn.iter().map(|(mesh, _)| mesh.id()).chain(second_shapes).collect();
                let pictures: Vec<AssetId<Image>> = drawn
                    .iter()
                    .filter_map(|(_, material)| materials.get(material))
                    .flat_map(|m| [&m.base_color_texture, &m.normal_map_texture, &m.metallic_roughness_texture, &m.occlusion_texture, &m.emissive_texture].into_iter().flatten().map(Handle::id).collect::<Vec<_>>())
                    .collect();
                info!("a worn thing, {template}: read away from the frame ({} meshes), {}", shapes.len(), if wait { "hung on this frame" } else { "waited for on the graphics card" });
                if wait {
                    hung_now.push(template.clone());
                } else {
                    uploads.wait(format!("{WAITED}{template}"), pictures, shapes);
                }
                reads.loaded.push((template, loaded));
            }
            Err(e) => warn!("a worn thing, {template}: not read, and not drawn: {e:#}"),
        }
        if !wait {
            break;
        }
    }

    // Each on the card: hung on the body and every copy.
    let ready = uploads.take(|name| name.starts_with(WAITED)).into_iter().map(|name| name[WAITED.len()..].to_owned());
    for template in ready.chain(hung_now) {
        let template = &template;
        let Some(at) = reads.loaded.iter().position(|(of, _)| of == template) else { continue };
        let (template, loaded) = reads.loaded.swap_remove(at);
        let dummy_skeletons: &mut [crate::screens::figure_view::Skeleton] = match dummies.as_mut() {
            Some(dummies) => &mut dummies.0,
            None => &mut [],
        };
        let made: Vec<(Handle<Mesh>, Handle<StandardMaterial>)> = match loaded {
            Loaded::Skinned(pieces, vestlike) => {
                let mut made = Vec::new();
                parts.note_bones(&template, pieces.iter().flat_map(|piece| &piece.bone_paths));
                for piece in pieces {
                    let joints: Vec<Entity> = piece.bone_paths.iter().map(|path| crate::player::clothing::joint_of(Some(&rig), &parts, &weapon, path)).collect();
                    let worn = WornPiece { item: template.clone(), over: piece.over.clone() };
                    // (An armband's every mesh waited for by the figure.)
                    if let (Some(band), Some((_, material))) = (&piece.band, piece.drawn.first()) {
                        made.extend(band.meshes().map(|mesh| (mesh.clone(), material.clone())));
                    }
                    for (slot, (mesh, material)) in piece.drawn.into_iter().enumerate() {
                        let fitted = piece.second.as_ref().and_then(|(kind, seconds, poses)| {
                            seconds.get(slot).map(|other| Fitted { wearer: Wearer::Character, kind: *kind, meshes: [mesh.clone(), other.clone()], poses: Some([piece.poses.clone(), poses.clone()]) })
                        });
                        let banded = piece.band.as_ref().map(|band| Banded { wearer: Wearer::Character, slot, meshes: band.clone() });
                        let entity = commands
                            .spawn((
                                Name::new("Clothing, a thing first worn"),
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(material.clone()),
                                SkinnedMesh { inverse_bindposes: piece.poses.clone(), joints: joints.clone() },
                                NoFrustumCulling,
                                crate::platform::park::Park,
                                UpperBody,
                                RenderLayers::layer(SEEN),
                                worn.clone(),
                                // Shown by what is worn (`show_worn`).
                                Visibility::Hidden,
                            ))
                            .id();
                        commands.entity(rig.root()).add_child(entity);
                        if let Some(fitted) = &fitted {
                            commands.entity(entity).insert(fitted.clone());
                        }
                        // Its levels of detail, on it and every copy.
                        let lod = piece.lod.clone();
                        let tag = |commands: &mut Commands, copy: Entity| {
                            if let Some(lod) = &lod {
                                commands.entity(copy).insert(lod.clone());
                            }
                        };
                        tag(&mut commands, entity);
                        if let Some(banded) = &banded {
                            commands.entity(entity).insert(banded.clone());
                        }
                        // The gear page's figure, and the OVERALL tab's own
                        // (`OVERALL FIGURE`).
                        for skeleton in figure.as_ref().map(|figure| std::iter::once(&figure.skeleton).chain(figure.overall_skeleton())).into_iter().flatten() {
                            let copy = skeleton.skinned(&mut commands, entity, (&mesh, &material), &piece.poses, &piece.bone_paths, false);
                            tag(&mut commands, copy);
                            if let Some(fitted) = &fitted {
                                commands.entity(copy).insert(fitted.of(Wearer::Figure));
                            }
                            if let Some(banded) = &banded {
                                commands.entity(copy).insert(banded.of(Wearer::Figure));
                            }
                        }
                        if let Some(corpse) = corpse.as_ref() {
                            let copy = corpse.0.skinned(&mut commands, entity, (&mesh, &material), &piece.poses, &piece.bone_paths, false);
                            tag(&mut commands, copy);
                            if let Some(fitted) = &fitted {
                                commands.entity(copy).insert(fitted.of(Wearer::Corpse));
                            }
                            if let Some(banded) = &banded {
                                commands.entity(copy).insert(banded.of(Wearer::Corpse));
                            }
                        }
                        for (which, dummy) in dummy_skeletons.iter().enumerate() {
                            let copy = dummy.skinned(&mut commands, entity, (&mesh, &material), &piece.poses, &piece.bone_paths, true);
                            tag(&mut commands, copy);
                            commands.entity(copy).insert(crate::world::dummy::DummyShows(crate::world::dummy::Shows::Worn { item: template.clone(), over: piece.over.clone() }));
                            if let Some(fitted) = &fitted {
                                commands.entity(copy).insert(fitted.of(Wearer::Dummy(which)));
                            }
                            if let Some(banded) = &banded {
                                commands.entity(copy).insert(banded.of(Wearer::Dummy(which)));
                            }
                        }
                        if let Some(fitted) = &fitted {
                            made.push((fitted.meshes[1].clone(), material.clone()));
                        }
                        made.push((mesh, material));
                    }
                }
                if vestlike {
                    parts.vestlike.push(template.clone());
                }
                made
            }
            Loaded::Rigid(model, looks, seconds) => {
                let head = rig.bone(&parts, "Base HumanHead");
                let (figure_skeleton, overall_skeleton) = match figure.as_mut() {
                    Some(figure) => {
                        let (gear, overall) = figure.skeletons();
                        (Some(gear), overall)
                    }
                    None => (None, None),
                };
                let copies = (figure_skeleton, overall_skeleton, corpse.as_mut().map(|corpse| &mut corpse.0), dummy_skeletons);
                crate::player::body_view::hang_read_model(&mut commands, &mut models, model, looks, seconds, head, copies)
            }
        };
        info!("a worn thing, {template}: hung on the body and its copies ({} meshes)", made.len());
        if let Some(figure) = figure.as_mut() {
            let (meshes, materials): (Vec<_>, Vec<_>) = made.into_iter().unzip();
            figure.wait_for(&meshes, &materials);
        }
    }
}

/// The next worn that the run has not read: asked for, if there is one;
/// else the thread's reader goes, away from the frame.
fn ask_next(reads: &mut WornReads, carried: &crate::screens::inventory_view::Carried) {
    let (worn, fitted) = worn_now(carried);
    // (An armband with `ARMBAND` off is not read at all.)
    let left_out = |template: &str| !reads.armband && reads.db.items.get(template).is_some_and(|item| reads.db.is_a(item, "ArmBand"));
    match worn.into_iter().find(|template| !reads.known.contains(template) && !left_out(template)) {
        Some(template) => {
            reads.known.insert(template.clone());
            let (worker, install, db, fitting, armband) = (reads.worker.take(), reads.install.clone(), reads.db.clone(), reads.fitting, reads.armband);
            reads.task = Some(AsyncComputeTaskPool::get().spawn(async move {
                // (A face shield's glass's cube, `glass_view`; the OVERALL
                // figure's surface's, `figure_surface`.)
                let reader = || UnityAssets::new(install).map(|mut unity| {
                    crate::effects::glass_view::open_cubes(&mut unity);
                    unity
                });
                match worker.map_or_else(reader, Ok) {
                    Ok(mut unity) => {
                        let made = prepare(&mut unity, &db, &template, &fitted, fitting, armband);
                        (Some(unity), template, made)
                    }
                    Err(e) => (None, template, Err(e)),
                }
            }));
        }
        // Nothing left to read: the thread's reader goes, away from the
        // frame.
        None => {
            if let Some(worker) = reads.worker.take() {
                AsyncComputeTaskPool::get().spawn(async move { drop(worker) }).detach();
            }
        }
    }
}
