//! Spawning the body with its rig.

use super::*;

/// Spawns the skeleton and the clothing skinned to it. Runs after the
/// weapon is spawned: the arms' bones are the weapon rig's.
pub fn spawn_body(
    mut commands: Commands,
    mut game: ResMut<GameData>,
    parts: Res<BodyParts>,
    rig: Option<Res<WeaponRig>>,
    mut skinned: Query<&mut SkinnedMesh>,
    mut converted: ResMut<Converted>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    let (Some(rig), false) = (rig, parts.parts.is_empty()) else { return };
    let unity = &mut game.unity;
    let root = commands.spawn((Name::new("Body"), Transform::default(), Visibility::default())).id();
    let ribcage_drawn = commands.spawn((Name::new("Ribcage as drawn"), Transform::default(), Visibility::default())).id();
    commands.entity(rig.root).add_child(ribcage_drawn);
    let mut by_path: HashMap<&str, Entity> = HashMap::new();
    let mut bones = Vec::with_capacity(parts.bones.len());
    for path in &parts.bones {
        let name = path.rsplit('/').next().unwrap_or(path);
        let entity = commands.spawn((Name::new(name.to_owned()), Transform::default(), Visibility::default())).id();
        // What hangs from the ribcage turns with it to the view, so it
        // hangs from the ribcage as drawn; the collarbones do not (they
        // are kept where the animation has them), so the animated ones
        // stay under the animated ribcage.
        let above = path.rfind('/').map(|at| &path[..at]);
        let parent = match above {
            Some(above) if above.ends_with(RIBCAGE) && !a_collarbone(path) => ribcage_drawn,
            Some(above) => by_path.get(above).copied().unwrap_or(root),
            None => root,
        };
        commands.entity(parent).add_child(entity);
        by_path.insert(path.as_str(), entity);
        bones.push(entity);
    }

    // A collarbone that is kept where the animated one is, for each.
    let mut collarbones = Vec::new();
    let mut kept: HashMap<&str, Entity> = HashMap::new();
    for path in parts.bones.iter().filter(|path| a_collarbone(path)) {
        let entity = commands.spawn((Name::new("Collarbone as kept"), Transform::default(), Visibility::default())).id();
        commands.entity(rig.root).add_child(entity);
        collarbones.push((by_path[path.as_str()], entity));
        kept.insert(path.as_str(), entity);
        // The arms' own mesh hangs its shoulders on the weapon rig's
        // collarbone; it goes on the kept one too.
        let on_rig = &path[path.find(RIBCAGE).map_or(0, |at| at + RIBCAGE.len() + 1)..];
        if let Some(rigs) = rig.by_hash.get(&path_hash(on_rig)) {
            for mut mesh in &mut skinned {
                for joint in mesh.joints.iter_mut().filter(|joint| *joint == rigs) {
                    *joint = entity;
                }
            }
        }
    }

    // The character model on the gear page, where the options have it: the
    // same meshes on a skeleton of its own, standing (`figure_view`).
    let mut figure = parts.standing_pose.as_ref().map(|pose| crate::screens::figure_view::Builder::new(&mut commands, &parts.standing, pose));
    // And with `OVERALL FIGURE`, the OVERALL tab's own: the same again on
    // the menu player's skeleton, which the game's menu figure moves
    // (`figure_view::Overall`).
    let mut overall = parts.overall.as_ref().filter(|_| figure.is_some()).map(|data| crate::screens::figure_view::Builder::overall(&mut commands, data));
    // And the corpse the character becomes, likewise, hidden until then
    // (`death`).
    let mut corpse = crate::screens::figure_view::Builder::corpse(&mut commands, &parts.standing);
    // And the dummies on the range, with their switch on: the same again
    // for each, standing as the model does where it stands until its
    // character poses it (`dummy`).
    let mut dummies: Vec<crate::screens::figure_view::Builder> = parts
        .dummy_pose
        .as_ref()
        .map(|pose| (0..crate::world::dummy::places().len()).map(|which| crate::screens::figure_view::Builder::dummy(&mut commands, &parts.standing, pose, crate::world::dummy::place(which), which)).collect())
        .unwrap_or_default();

    // The entity a mesh's bone is tied to.
    let joint = |path: &str| -> Entity {
        if path.ends_with(RIBCAGE) {
            return ribcage_drawn;
        }
        if let Some(entity) = kept.get(path) {
            return *entity;
        }
        if of_an_arm(path) {
            // Named on the weapon's rig from the collarbone down. A bone
            // the rig lacks is taken as the nearest one above it.
            let mut on_rig = &path[path.find(RIBCAGE).map_or(0, |at| at + RIBCAGE.len() + 1)..];
            loop {
                if let Some(entity) = rig.by_hash.get(&path_hash(on_rig)) {
                    return *entity;
                }
                match on_rig.rfind('/') {
                    Some(at) => on_rig = &on_rig[..at],
                    None => return rig.root,
                }
            }
        }
        by_path.get(path).copied().unwrap_or(root)
    };

    let mut drawn = 0;
    let mut noted: HashSet<&str> = HashSet::new();
    for (index, (((part, upper), item), (of_outfit, top))) in parts.parts.iter().zip(&parts.upper).zip(&parts.item).zip(parts.of_outfit.iter().zip(&parts.top)).enumerate() {
        // A dummy's own part, drawn on it alone; and which part of a look
        // a part of the character's own is (its suit's top or legs, its
        // head), which a dummy with its own does not take.
        let own = parts.dummy_own.get(index).copied().flatten();
        // (A coarser level is its fullest's part: `BodyParts::fullest`.)
        let of_head = parts.head.is_some_and(|head| parts.fullest.get(index).copied().unwrap_or(index) == head);
        let role = match parts.suit.get(index).copied().flatten() {
            Some(SuitRole::Upper) => Some(LOOK_UPPER),
            Some(SuitRole::Lower) => Some(LOOK_LOWER),
            None => of_head.then_some(LOOK_HEAD),
        };
        // Its levels of detail, on every copy of it (`lod_view`).
        let lod = parts.lod_piece(index);
        // A torso of the newer kind has a mesh for each case of what is
        // worn, each drawn by itself (`show_torso`).
        let meshes_of: Vec<(Option<trp_data::character::arms::Torso>, trp_data::unity::ObjRef)> = match &part.torso {
            Some(torso) => trp_data::character::arms::Torso::ALL.iter().filter_map(|&case| Some((Some(case), torso.mesh(case)?))).collect(),
            None => vec![(None, part.mesh)],
        };
        let joints: Vec<Entity> = part.bone_paths.iter().map(|path| joint(path)).collect();
        // Each thing worn as a skin: whether the skeleton has its bones, said once (`BodyParts::note_bones`).
        if let Some(worn) = item.as_ref().filter(|worn| noted.insert(worn.item.as_str())) {
            parts.note_bones(&worn.item, parts.parts.iter().zip(&parts.item).filter(|(_, of)| of.as_ref().is_some_and(|of| of.item == worn.item)).flat_map(|(part, _)| &part.bone_paths));
        }
        // An armband's skin: its mesh for every upper body (`fit_armbands`), each read once.
        let band = parts.bands.iter().find(|(at, _)| *at == index).map(|(_, sets)| {
            std::sync::Arc::new(BandMeshes::of(sets, |mesh| match converted.mesh(unity, mesh, false, &mut meshes) {
                Ok(entry) => Some((entry.parts.clone(), bindposes.add(SkinnedMeshInverseBindposes::from(entry.bind_poses.clone())))),
                Err(e) => {
                    warn!("an armband's mesh: {e:#}");
                    None
                }
            }))
        });
        // A hood's: its hooded form (`fit_head_things`, `fit::HOOD_UP`).
        let hood = parts.hoods.iter().find(|(at, _)| *at == index).map(|(_, hood)| hood);
        for (case, mesh_ref) in meshes_of {
            let entry = match converted.mesh(unity, mesh_ref, false, &mut meshes) {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("a mesh of the body: {e:#}");
                    continue;
                }
            };
            let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(entry.bind_poses.clone()));
            // Its second form, where it has one (a face cover's skin, a
            // hood: `fit_head_things`), with its own bind poses.
            let hooded = hood.and_then(|hood| match (&hood.torso, case) {
                (Some(torso), Some(case)) => torso.mesh(case),
                (None, None) => Some(hood.mesh),
                _ => None,
            });
            let second_of = parts.fittings.iter().find(|(at, fitting)| *at == index && fitting.base == mesh_ref).map(|(_, fitting)| (fitting.kind, fitting.custom)).or(hooded.map(|mesh| (trp_data::character::dress::fit::HOOD_UP, mesh)));
            let second = second_of.and_then(|(kind, custom)| match converted.mesh(unity, custom, false, &mut meshes) {
                Ok(custom) => Some((kind, second_form(entry.parts.len(), &custom.parts), bindposes.add(SkinnedMeshInverseBindposes::from(custom.bind_poses.clone())))),
                Err(e) => {
                    warn!("a second mesh of the body: {e:#}");
                    None
                }
            });
            for (slot, mesh) in entry.parts.iter().enumerate() {
                let material_ref = part.materials.get(slot).copied().flatten();
                let material = converted.material(unity, material_ref, &mut images, &mut materials);
                let fitted = second.as_ref().and_then(|(kind, seconds, poses)| {
                    seconds.get(slot).map(|other| Fitted { wearer: Wearer::Character, kind: *kind, meshes: [mesh.clone(), other.clone()], poses: Some([inverse_bindposes.clone(), poses.clone()]) })
                });
                let banded = band.as_ref().map(|band| Banded { wearer: Wearer::Character, slot, meshes: band.clone() });
                // What a dummy's copy is shown by, on the dummy's own gear
                // (`dummy::show_dummies`).
                let shows = match (item, of_outfit) {
                    (Some(worn), _) => crate::world::dummy::Shows::Worn { item: worn.item.clone(), over: worn.over.clone() },
                    (None, Some(outfit)) => crate::world::dummy::Shows::Outfit { item: outfit.clone() },
                    (None, None) => crate::world::dummy::Shows::Own { top: *top, torso: case },
                };
                if let Some((which, _)) = own {
                    if let Some(dummy) = dummies.get_mut(which) {
                        let copy = dummy.skinned(&mut commands, Entity::PLACEHOLDER, (mesh, &material), &inverse_bindposes, &part.bone_paths, true);
                        commands.entity(copy).insert(crate::world::dummy::DummyShows(shows));
                        if let Some(lod) = &lod {
                            commands.entity(copy).insert(lod.clone());
                        }
                        // (Its own top's hood.)
                        if let Some(fitted) = &fitted {
                            commands.entity(copy).insert(fitted.of(Wearer::Dummy(which)));
                        }
                        drawn += 1;
                    }
                    continue;
                }
                let piece = commands
                    .spawn((
                        Name::new("Clothing"),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: joints.clone() },
                        // Skinned bounds are not tracked.
                        NoFrustumCulling,
                        // Out of the engine's work while hidden (`park`).
                        Park,
                    ))
                    .id();
                if *upper {
                    commands.entity(piece).insert((UpperBody, RenderLayers::layer(SEEN)));
                } else {
                    commands.entity(piece).insert((LowerBody, RenderLayers::layer(SEEN)));
                }
                match (item, of_outfit) {
                    (Some(worn), _) => commands.entity(piece).insert(worn.clone()),
                    // One of the install's own tops, worn as an outfit.
                    (None, Some(outfit)) => commands.entity(piece).insert((OutfitPiece { item: outfit.clone() }, Visibility::Hidden)),
                    (None, None) => commands.entity(piece).insert(OwnBody { top: *top }),
                };
                if let Some(role) = parts.suit.get(index).copied().flatten() {
                    commands.entity(piece).insert(SuitPiece(role));
                }
                if let Some(case) = case {
                    commands.entity(piece).insert((TorsoPiece(case), Visibility::Hidden));
                }
                if let Some(fitted) = &fitted {
                    commands.entity(piece).insert(fitted.clone());
                }
                if let Some(banded) = &banded {
                    commands.entity(piece).insert(banded.clone());
                }
                if let Some(lod) = &lod {
                    commands.entity(piece).insert(lod.clone());
                }
                commands.entity(root).add_child(piece);
                let hidden = (item.is_none() && of_outfit.is_some()) || case.is_some();
                for figure in [figure.as_mut(), overall.as_mut()].into_iter().flatten() {
                    let copy = figure.skinned(&mut commands, piece, (mesh, &material), &inverse_bindposes, &part.bone_paths, hidden);
                    if let Some(lod) = &lod {
                        commands.entity(copy).insert(lod.clone());
                    }
                    if let Some(fitted) = &fitted {
                        figure.keep_mesh(&fitted.meshes[1]);
                        commands.entity(copy).insert(fitted.of(Wearer::Figure));
                    }
                    if let Some(banded) = &banded {
                        banded.meshes.meshes().for_each(|mesh| figure.keep_mesh(mesh));
                        commands.entity(copy).insert(banded.of(Wearer::Figure));
                    }
                }
                let corpse_piece = corpse.skinned(&mut commands, piece, (mesh, &material), &inverse_bindposes, &part.bone_paths, hidden);
                // (A top with a hood is the head's too: up, it closes round
                // the eyes, which ride the chest; the game's own body, kept
                // as its corpse, draws its whole top a shadow for them.
                // `PROVISIONAL`: the rest of the corpse's upper body is still
                // drawn for those eyes, the head and a hooded top alone not.)
                if of_head || hood.is_some() {
                    commands.entity(corpse_piece).insert(crate::player::death::CorpseHead);
                }
                if let Some(lod) = &lod {
                    commands.entity(corpse_piece).insert(lod.clone());
                }
                if let Some(fitted) = &fitted {
                    commands.entity(corpse_piece).insert(fitted.of(Wearer::Corpse));
                }
                if let Some(banded) = &banded {
                    commands.entity(corpse_piece).insert(banded.of(Wearer::Corpse));
                }
                for (which, dummy) in dummies.iter_mut().enumerate() {
                    if role.is_some_and(|role| parts.dummy_has.get(which).is_some_and(|has| has[role])) {
                        continue;
                    }
                    let copy = dummy.skinned(&mut commands, piece, (mesh, &material), &inverse_bindposes, &part.bone_paths, true);
                    commands.entity(copy).insert(crate::world::dummy::DummyShows(shows.clone()));
                    if let Some(lod) = &lod {
                        commands.entity(copy).insert(lod.clone());
                    }
                    if let Some(fitted) = &fitted {
                        commands.entity(copy).insert(fitted.of(Wearer::Dummy(which)));
                    }
                    if let Some(banded) = &banded {
                        commands.entity(copy).insert(banded.of(Wearer::Dummy(which)));
                    }
                }
                drawn += 1;
            }
        }
    }
    // The outfits that are not the install's (`docs/spec/outfits.md`).
    // Each has a skeleton of its own to be drawn on: an entity for each
    // bone of its rig, which every frame is given the move, from rest, of
    // what is on that bone (`see_outfit`). Its meshes hang on those, each
    // in its three parts, and are not drawn until its item is on. What
    // of it lies under the waist is seen by the character's own eyes too;
    // the rest, its arms with it, is the upper body's, a shadow there and
    // no more. Its arms for those eyes are a model of their own (the
    // other game's for first person, on a second such skeleton), seen in
    // first person and nowhere else; an outfit without one leaves the
    // install's own arms there.
    let mut skeletons = Vec::new();
    let weapon_root = rig.root;
    let mut skeleton_of = |commands: &mut Commands, item: &str, rig: &trp_data::character::outfit::Rig| -> Vec<Entity> {
        // What the game squeezes with the weapon hangs, through two
        // entities, from the weapon rig's root: the first is that root
        // again, the second undoes its place and its turn.
        let squeezed_root = commands.spawn((Name::new("Outfit, as the weapon is squeezed"), Transform::default(), Visibility::default())).id();
        let unsqueeze = commands.spawn((Name::new("Outfit, the root undone"), Transform::default(), Visibility::default())).id();
        commands.entity(weapon_root).add_child(squeezed_root);
        commands.entity(squeezed_root).add_child(unsqueeze);
        let squeezed: Vec<bool> = rig.bones.iter().map(|bone| of_an_arm(&bone.path) || a_collarbone(&bone.path)).collect();
        let ours: Vec<Entity> = rig
            .bones
            .iter()
            .zip(&squeezed)
            .map(|(bone, squeezed)| {
                let name = bone.path.rsplit('/').next().unwrap_or(&bone.path);
                let entity = commands.spawn((Name::new(format!("{name}, the outfit's")), Transform::default(), Visibility::default())).id();
                if *squeezed {
                    commands.entity(unsqueeze).add_child(entity);
                }
                entity
            })
            .collect();
        skeletons.push(OutfitSkeleton {
            item: item.to_owned(),
            // The rig's points in the engine's axes, as the meshes are.
            rig: rig.in_space(|p| convert::position(p).to_array()),
            theirs: rig.bones.iter().map(|bone| joint(&bone.path)).collect(),
            their_rest: rig.bones.iter().map(|bone| bevy::math::Affine3A::from_mat4(convert::matrix(bone.bind))).collect(),
            squeezed,
            ours: ours.clone(),
            unsqueeze,
        });
        ours
    };
    for (item, outfit) in &parts.outfits {
        let ours = if outfit.has_body() { skeleton_of(&mut commands, item, &outfit.rig) } else { Vec::new() };
        // The model's own skeleton for it, stood once.
        let rest: Vec<bevy::math::Affine3A> = outfit.rig.bones.iter().map(|bone| bevy::math::Affine3A::from_mat4(convert::matrix(bone.bind))).collect();
        let model_ours = figure.as_mut().filter(|_| outfit.has_body()).map(|figure| figure.outfit(&mut commands, item, &outfit.rig.in_space(|p| convert::position(p).to_array()), &rest));
        let overall_ours = overall.as_mut().filter(|_| outfit.has_body()).map(|overall| overall.outfit(&mut commands, item, &outfit.rig.in_space(|p| convert::position(p).to_array()), &rest));
        let corpse_ours = outfit.has_body().then(|| corpse.outfit(&mut commands, item, &outfit.rig.in_space(|p| convert::position(p).to_array()), &rest));
        let dummies_ours: Vec<Vec<Entity>> = if outfit.has_body() {
            dummies.iter_mut().map(|dummy| dummy.outfit(&mut commands, item, &outfit.rig.in_space(|p| convert::position(p).to_array()), &rest)).collect()
        } else {
            Vec::new()
        };
        for piece in outfit.pieces.iter().filter(|_| outfit.has_body()) {
            // A mesh whose picture was not found is not drawn at all: it
            // would have no colour of its own.
            let Some(picture) = &piece.picture else { continue };
            let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(piece.mesh.bind_poses.iter().map(|rows| convert::matrix(*rows)).collect::<Vec<_>>()));
            let material = converted.outfit_material(picture, &mut images, &mut materials);
            for (zone, mesh) in converted.custom_mesh(&piece.mesh, &mut meshes).into_iter().enumerate() {
                if piece.mesh.submeshes[zone].is_empty() {
                    continue;
                }
                let entity = commands
                    .spawn((
                        Name::new("Outfit"),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: ours.clone() },
                        NoFrustumCulling,
                        Visibility::Hidden,
                        OutfitPiece { item: item.clone() },
                        Park,
                    ))
                    .id();
                if let (Some(figure), Some(model_ours)) = (figure.as_mut(), model_ours.as_ref()) {
                    figure.skinned_on(&mut commands, entity, (&mesh, &material), &inverse_bindposes, model_ours.clone(), true);
                }
                if let (Some(overall), Some(overall_ours)) = (overall.as_mut(), overall_ours.as_ref()) {
                    overall.skinned_on(&mut commands, entity, (&mesh, &material), &inverse_bindposes, overall_ours.clone(), true);
                }
                if let Some(corpse_ours) = corpse_ours.as_ref() {
                    corpse.skinned_on(&mut commands, entity, (&mesh, &material), &inverse_bindposes, corpse_ours.clone(), true);
                }
                for (dummy, ours) in dummies.iter_mut().zip(&dummies_ours) {
                    let copy = dummy.skinned_on(&mut commands, entity, (&mesh, &material), &inverse_bindposes, ours.clone(), true);
                    commands.entity(copy).insert(crate::world::dummy::DummyShows(crate::world::dummy::Shows::Outfit { item: item.clone() }));
                }
                if zone != trp_data::character::outfit::LEGS {
                    commands.entity(entity).insert((UpperBody, RenderLayers::layer(SEEN)));
                } else {
                    commands.entity(entity).insert((LowerBody, RenderLayers::layer(SEEN)));
                }
                commands.entity(root).add_child(entity);
                drawn += 1;
            }
        }
        let Some(arms) = outfit.arms.as_ref().filter(|_| outfit.has_arms()) else { continue };
        let ours = skeleton_of(&mut commands, item, &arms.rig);
        for piece in &arms.pieces {
            let Some(picture) = &piece.picture else { continue };
            let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(piece.mesh.bind_poses.iter().map(|rows| convert::matrix(*rows)).collect::<Vec<_>>()));
            let material = converted.outfit_material(picture, &mut images, &mut materials);
            for (zone, mesh) in converted.custom_mesh(&piece.mesh, &mut meshes).into_iter().enumerate() {
                if piece.mesh.submeshes[zone].is_empty() {
                    continue;
                }
                // The shadow is the body's own arms': these cast none.
                let entity = commands
                    .spawn((
                        Name::new("Outfit, its arms for its own eyes"),
                        Mesh3d(mesh),
                        MeshMaterial3d(material.clone()),
                        SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: ours.clone() },
                        NoFrustumCulling,
                        bevy::light::NotShadowCaster,
                        Visibility::Hidden,
                        OutfitArms { item: item.clone() },
                    ))
                    .id();
                commands.entity(root).add_child(entity);
                drawn += 1;
            }
        }
    }
    // The install's own hands as an outfit's arms: skinned to the arms'
    // bones as the character's own hands are, and drawn in their place
    // in first person while the item is on.
    for (item, hands) in &parts.their_hands {
        let entry = match converted.mesh(unity, hands.mesh, false, &mut meshes) {
            Ok(entry) => entry,
            Err(e) => {
                warn!("the hands of an outfit: {e:#}");
                continue;
            }
        };
        let joints: Vec<Entity> = hands.bone_paths.iter().map(|path| joint(path)).collect();
        let inverse_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(entry.bind_poses.clone()));
        // A watch's glass is not drawn (`trp_data::arms::is_glass`).
        for (slot, mesh) in entry.parts.iter().enumerate().filter(|(slot, _)| !hands.glass.get(*slot).copied().unwrap_or(false)) {
            let material_ref = hands.materials.get(slot).copied().flatten();
            let material = converted.material(unity, material_ref, &mut images, &mut materials);
            let entity = commands
                .spawn((
                    Name::new("Outfit, the install's own hands"),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    SkinnedMesh { inverse_bindposes: inverse_bindposes.clone(), joints: joints.clone() },
                    NoFrustumCulling,
                    bevy::light::NotShadowCaster,
                    Visibility::Hidden,
                    OutfitArms { item: item.clone() },
                ))
                .id();
            commands.entity(root).add_child(entity);
            drawn += 1;
        }
    }
    commands.insert_resource(OutfitSkeletons(skeletons));
    // What rides on the head: the item's own model, parented to the head
    // bone with no offset and the one fixed turn the game gives it.
    let head = parts.bones.iter().position(|path| path.ends_with(HEAD)).map(|at| bones[at]);
    let heads = Heads {
        body: Some(head.unwrap_or(root)),
        figure: figure.as_ref().and_then(|figure| figure.bone(HEAD)),
        overall: overall.as_ref().and_then(|overall| overall.bone(HEAD)),
        corpse: corpse.bone(HEAD),
        dummies: dummies.iter().map(|dummy| dummy.bone(HEAD)).collect(),
    };
    let mut kept_models = Vec::new();
    for HeadModel { item, model, parts: fitted, hinges, fittings } in parts.on_head.iter().filter(|_| head.is_some()) {
        let mut kept = HeadKept {
            item: item.clone(),
            model: model.clone(),
            parts: fitted.clone(),
            fittings: fittings.clone(),
            none: HashSet::new(),
            body: Vec::new(),
            figure: Vec::new(),
            overall: Vec::new(),
            corpse: Vec::new(),
            dummies: vec![Vec::new(); dummies.len()],
        };
        let copies = Copies { figure: figure.as_mut(), overall: overall.as_mut(), corpse: Some(&mut corpse), dummies: &mut dummies };
        let mut looks = converted_looks(&*unity, &mut converted, &mut meshes, &mut images, &mut materials);
        let made = hang_head_model(&mut commands, &mut looks, &mut kept, 0, hinges, &heads, copies);
        drawn += made.len();
        kept_models.push(kept);
    }
    commands.insert_resource(HeadModels(kept_models));
    info!("the body: {} bones posed by its animator, {} meshes", bones.len(), drawn);
    if let Some(figure) = figure {
        figure.finish(&mut commands, overall.zip(parts.overall.clone()));
    }
    corpse.finish_corpse(&mut commands);
    if !dummies.is_empty() {
        let (rigs, skeletons): (Vec<_>, Vec<_>) = dummies.into_iter().map(|dummy| dummy.finish_dummy()).unzip();
        commands.insert_resource(crate::world::dummy::DummyRigs(rigs));
        commands.insert_resource(crate::world::dummy::DummySkeletons(skeletons));
    }
    let ribcage_animated = parts.bones.iter().position(|path| path.ends_with(RIBCAGE)).map(|at| bones[at]);
    commands.insert_resource(BodyRig { root, bones, collarbones, ribcage: rig.root, ribcage_animated, ribcage_drawn });
}
