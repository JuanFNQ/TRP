//! Cases flown each frame and spawned.

use super::*;

/// The cases' frame: the calls taken, the case at the port made or thrown,
/// a cylinder's or barrels' cases at their ports made, taken away or
/// thrown, those in flight moved on, turned, and taken away when their
/// second is up.
#[allow(clippy::too_many_arguments)]
pub fn fly_cases(
    mut commands: Commands,
    mut cases: ResMut<Cases>,
    mut controller: ResMut<Controller>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    game: Res<crate::GameData>,
    mut loose: ResMut<Loose>,
    weapon: Option<Res<WeaponRig>>,
    range: Res<Range>,
    time: Res<Time>,
    mut transforms: ParamSet<(TransformHelper, Query<&mut Transform>)>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    (hands, kinds): (Option<Res<crate::weapons::weapon::HandsRig>>, Res<crate::weapons::magazine_rounds::ChamberKinds>),
) {
    let cases = &mut *cases;
    let calls = std::mem::take(&mut controller.case_calls);

    let Some(settings) = cases.settings else { return };
    take_sounds(cases);
    let port = weapon.as_ref().and_then(|rig| rig.shell_port);
    // The round's case's model asked for ahead of the first shot (the
    // game's pool has it made).
    if port.is_some() && (held.is_changed() || carried.is_changed()) {
        if let Some(round) = fired_round(&carried, &held, &game.db) {
            loose.model_for(&format!("{}{round}", crate::world::loose::CASE_MODEL));
        }
    }
    for call in calls {
        match call {
            // The round to the shell port, a quarter turn about x
            // (`ParentShellToTransform`), a case.
            CaseCall::Shot => {
                // One left at the port goes (with its rig, if that is gone).
                if let Some(old) = cases.at_port.take() {
                    commands.entity(old).try_despawn();
                }
                let (Some(port), Some(round)) = (port, fired_round(&carried, &held, &game.db)) else { continue };
                let Some(ready) = loose.model_for(&format!("{}{round}", crate::world::loose::CASE_MODEL)) else { continue };
                let turn = convert::rotation(trp_sim::base::math::quat_euler([90.0, 0.0, 0.0]));
                let case = spawn_case(&mut commands, &ready, Transform::from_rotation(turn));
                commands.entity(port.entity).add_child(case);
                cases.at_port = Some(case);
            }
            // Thrown from where the port has carried it, with the weapon's
            // force and turn (`SpawnExtractingCartridge`).
            CaseCall::Eject => {
                let (Some(case), Some(port), Some(rig)) = (cases.at_port.take(), port, weapon.as_ref()) else { continue };
                commands.entity(case).try_despawn();
                let (pose, drawn) = {
                    let helper = transforms.p0();
                    (unsqueezed(&helper, rig.root, port.entity), helper.compute_global_transform(port.entity).ok())
                };
                let local = transforms.p1().get(port.entity).map(|transform| transform.translation);
                let (Some(pose), Ok(local)) = (pose, local) else { continue };
                let Some(round) = fired_round(&carried, &held, &game.db) else { continue };
                let Some(ready) = loose.model_for(&format!("{}{round}", crate::world::loose::CASE_MODEL)) else { continue };
                let parent = controller.movement.velocity.map(|v| v * PLAYER_SHARE);
                launch(&mut commands, cases, settings, &range, &ready, pose, drawn, (pose.1, local), &port, parent);
            }
            // A chamber of its own's case thrown from its port, where one
            // is drawn there (`SpawnShell`: a port with none throws none).
            CaseCall::Thrown { chamber, share } => {
                let Some((case, round)) = cases.in_ports.get_mut(chamber).and_then(Option::take) else { continue };
                commands.entity(case).try_despawn();
                let (Some(port), Some(rig)) = (port, weapon.as_ref()) else { continue };
                let Some(&from) = rig.shell_ports.get(chamber) else { continue };
                let (pose, first, drawn) = {
                    let helper = transforms.p0();
                    (unsqueezed(&helper, rig.root, from), unsqueezed(&helper, rig.root, port.entity), helper.compute_global_transform(from).ok())
                };
                let local = transforms.p1().get(port.entity).map(|transform| transform.translation);
                let (Some(pose), Some((_, first_turn)), Ok(local)) = (pose, first, local) else { continue };
                let Some(ready) = loose.model_for(&format!("{}{round}", crate::world::loose::CASE_MODEL)) else { continue };
                let parent = share.map_or([0.0, -1.0, 0.0], |share| controller.movement.velocity.map(|v| v * share));
                debug!("the case of chamber {chamber} ({round}) thrown from {:?} with {parent:?} besides", pose.0);
                launch(&mut commands, cases, settings, &range, &ready, pose, drawn, (first_turn, local), &port, parent);
            }
            // An underbarrel launcher's case thrown from its own port, where one
            // is drawn there, with the launcher's own throw and its port's move
            // and forward (`UnderbarrelContainer.SpawnExtractingCartridge`).
            CaseCall::LauncherThrown(share) => {
                let Some((case, round)) = cases.launcher_case.take() else { continue };
                commands.entity(case).try_despawn();
                let Some((port, rig)) = weapon.as_ref().and_then(|rig| Some((rig.launcher_shell_port?, rig))) else { continue };
                let (pose, drawn) = {
                    let helper = transforms.p0();
                    (unsqueezed(&helper, rig.root, port.entity), helper.compute_global_transform(port.entity).ok())
                };
                let local = transforms.p1().get(port.entity).map(|transform| transform.translation);
                let (Some(pose), Ok(local)) = (pose, local) else { continue };
                let Some(ready) = loose.model_for(&format!("{}{round}", crate::world::loose::CASE_MODEL)) else { continue };
                let parent = controller.movement.velocity.map(|v| v * share);
                debug!("the launcher's case ({round}) thrown from {:?} with {parent:?} besides", pose.0);
                launch(&mut commands, cases, settings, &range, &ready, pose, drawn, (pose.1, local), &port, parent);
            }
        }
    }
    // A cylinder's or barrels' cases at their ports while the model holds
    // them, each of the kind kept for it (`magazine_rounds::ChamberKinds`:
    // the item's, or its round's), a quarter turn about x
    // (`SetPatronInShellPort`); on another rig made afresh from what its
    // model holds.
    let rig_root = weapon.as_ref().and_then(|rig| rig.weapon_root);
    if cases.ports_rig != rig_root {
        for (case, _) in cases.in_ports.drain(..).chain(cases.launcher_case.take().map(Some)).flatten() {
            commands.entity(case).try_despawn();
        }
        cases.ports_rig = rig_root;
    }
    // An underbarrel launcher's case at its own port while its model holds
    // one (`Firearm::launcher_models`: from the shot, `MoveAmmoFromChamberTo
    // ShellPort`, a quarter turn about x, `ParentAmmoOrShellToTransform`), of
    // the kind of the round last seen in its chamber, whose model is asked
    // for ahead.
    if let Some(round) = crate::weapons::magazine_rounds::launcher_round(&carried, &held) {
        if cases.launcher_kind.as_deref() != Some(round.as_str()) {
            loose.model_for(&format!("{}{round}", crate::world::loose::CASE_MODEL));
            cases.launcher_kind = Some(round);
        }
    }
    let launcher_port = weapon.as_ref().and_then(|rig| rig.launcher_shell_port);
    let case_kept = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).and_then(trp_sim::Firearm::launcher_models).is_some_and(|(_, case)| case);
    match (&cases.launcher_case, launcher_port.filter(|_| case_kept)) {
        (Some((case, _)), None) => {
            commands.entity(*case).try_despawn();
            cases.launcher_case = None;
        }
        (None, Some(port)) => {
            if let Some(round) = cases.launcher_kind.clone() {
                if let Some(ready) = loose.model_for(&format!("{}{round}", crate::world::loose::CASE_MODEL)) {
                    let turn = convert::rotation(trp_sim::base::math::quat_euler([90.0, 0.0, 0.0]));
                    let case = spawn_case(&mut commands, &ready, Transform::from_rotation(turn));
                    commands.entity(port.entity).add_child(case);
                    cases.launcher_case = Some((case, round));
                }
            }
        }
        _ => {}
    }
    let shown = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).filter(|firearm| !firearm.single_chamber()).map_or(&[][..], |firearm| firearm.chambers_shown());
    if cases.in_ports.len() < shown.len() {
        cases.in_ports.resize(shown.len(), None);
    }
    for index in 0..cases.in_ports.len() {
        let wanted: Option<std::borrow::Cow<str>> = shown.get(index).filter(|shown| shown.case).and_then(|_| kinds.case(index).or_else(|| kinds.kind(index)).map(std::borrow::Cow::Borrowed).or_else(|| fired_round(&carried, &held, &game.db).map(std::borrow::Cow::Owned)));
        if cases.in_ports[index].as_ref().map(|(_, round)| round.as_str()) == wanted.as_deref() {
            continue;
        }
        if let Some((case, _)) = cases.in_ports[index].take() {
            commands.entity(case).try_despawn();
        }
        let (Some(round), Some(&port)) = (wanted, weapon.as_ref().and_then(|rig| rig.shell_ports.get(index))) else { continue };
        let Some(ready) = loose.model_for(&format!("{}{round}", crate::world::loose::CASE_MODEL)) else { continue };
        let turn = convert::rotation(trp_sim::base::math::quat_euler([90.0, 0.0, 0.0]));
        let case = spawn_case(&mut commands, &ready, Transform::from_rotation(turn));
        commands.entity(port).add_child(case);
        cases.in_ports[index] = Some((case, round.into_owned()));
    }
    // Those in flight.
    let dt = time.delta_secs();
    let cast = |from: [f32; 3], way: [f32; 3], reach: f32| range.cast(from, way, reach);
    let seed = &mut cases.seed;
    let hearing = mixer.as_deref().zip(camera.single().ok());
    let overall = levels.as_deref().map_or(1.0, Levels::overall_gain);
    let sounds = &mut cases.sounds;
    cases.flying.retain_mut(|case| {
        case.since += dt;
        if case.since >= LASTS {
            commands.entity(case.entity).despawn();
            return false;
        }
        let mut random = || draw(seed);
        let struck = case.flight.frame(dt, &cast, &mut random);
        if let (Some(surface), Some(caliber), Some(sounds), Some((mixer, camera))) = (struck, case.caliber, sounds.as_mut(), hearing) {
            play_bounce(sounds, caliber, surface, case.flight.place(), camera, mixer, overall);
        }
        // Its turn each frame (`Shell.Update`: its turn times a hundred
        // times the frame's time, in degrees, about its own axes).
        let step = trp_sim::base::math::quat_euler(case.flight.turn.map(|t| t * dt * 100.0));
        case.turn = mul(case.turn, step);
        if let Ok(mut transform) = transforms.p1().get_mut(case.entity) {
            transform.translation = convert::position(case.flight.place());
            transform.rotation = convert::rotation(case.turn);
        }
        true
    });
}

/// A case's drawn model: the round's own, its `shell` objects on and its
/// `full` ones off (`AmmoPoolObject.SetUsed`), at that place.
pub(super) fn spawn_case(commands: &mut Commands, ready: &crate::world::loose::Ready, at: Transform) -> Entity {
    let root = commands.spawn((Name::new("Spent case"), at, Visibility::Inherited)).id();
    let mut entities = Vec::with_capacity(ready.nodes.len());
    for (index, (parent, own, on)) in ready.nodes.iter().enumerate() {
        let name = ready.names.get(index).map(String::as_str).unwrap_or_default();
        let shown = if name.contains("full") {
            false
        } else {
            name.contains("shell") || *on
        };
        let entity = commands.spawn((*own, if shown { Visibility::Inherited } else { Visibility::Hidden })).id();
        commands.entity(parent.map_or(root, |p| entities[p])).add_child(entity);
        entities.push(entity);
    }
    let mut pieces = Vec::with_capacity(ready.drawn.len());
    for (k, (node, mesh, material)) in ready.drawn.iter().enumerate() {
        let piece = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id();
        commands.entity(entities[*node]).add_child(piece);
        pieces.push((k, piece));
    }
    // By its round's own levels of detail (`lod_view`, the world's things'):
    // the game's case is a pooled round object, its group kept.
    if crate::assets::lod_view::world_on() {
        ready.spawn_levels(commands, &entities, &pieces, None);
    }
    root
}
