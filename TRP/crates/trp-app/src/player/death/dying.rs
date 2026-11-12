//! Dying, and the corpse followed afterwards.

use super::*;

/// When the character has just died: the corpse takes the body's pose as
/// it is drawn this frame, and the game's engine is made with the ragdoll
/// on it, the weapon let go, and the range to fall against.
#[allow(clippy::too_many_arguments)]
pub fn die(
    mut commands: Commands,
    controller: Res<Controller>,
    mut dying: ResMut<Dying>,
    rigs: (Option<Res<Corpse>>, Option<Res<BodyParts>>, Option<Res<BodyRig>>, Option<Res<WeaponRig>>),
    (ragdoll, outfit_on, range): (Res<Ragdoll>, Res<OutfitOn>, Res<Range>),
    helper: TransformHelper,
    (parents, children, meshes, eye, locals_of): (Query<&ChildOf>, Query<&Children>, Query<&Aabb, (With<Mesh3d>, Without<SkinnedMesh>)>, Query<Entity, With<Eye>>, Query<&Transform>),
    originals: Query<&Visibility, (Without<CorpseTwin>, crate::platform::park::EvenParked)>,
    mut twins: Query<(&CorpseTwin, &mut Visibility), crate::platform::park::EvenParked>,
    (mut clock, time): (ResMut<PhysicsClock>, Res<Time>),
    (node_colliders, seen): (Query<&crate::weapons::weapon::NodeColliders>, Query<&InheritedVisibility>),
    (hands_held, carried, game): (Res<crate::screens::inventory_view::HandsHeld>, Res<crate::screens::inventory_view::Carried>, Res<crate::GameData>),
    mut fell: MessageWriter<crate::audio::step_sound::BodyFell>,
) {
    // Every frame counts towards the step, as the game's average does.
    let clock = &mut *clock;
    clock.now = clock.stepper.frame(time.delta_secs());
    let dead = controller.health.as_ref().is_some_and(|health| !health.alive);
    if !dead || dying.0.is_some() {
        return;
    }
    let (Some(corpse), Some(parts), Some(body), Some(weapon)) = rigs else { return };
    let (Some(data), Some(physics)) = (ragdoll.data.as_ref(), ragdoll.physics.as_ref()) else { return };
    // Drawn, as the body is this frame: what hangs from the weapon rig's
    // root with the first person's squeeze along the view taken off.
    let Ok(root) = helper.compute_global_transform(weapon.root) else { return };
    let (_, turn, at) = root.to_scale_rotation_translation();
    let unsqueezed = Affine3A::from_rotation_translation(turn, at) * root.affine().inverse();
    let on_rig = |entity: Entity| -> Option<Affine3A> {
        let now = helper.compute_global_transform(entity).ok()?.affine();
        Some(if under(entity, weapon.root, &parents) { unsqueezed * now } else { now })
    };
    // And for the character's own eyes, what hangs from the ribcage
    // (the arms, the neck and head, the weapon in hand) hangs from the
    // weapon rig's root, which stands for it where the view needs it:
    // the corpse is the body as others see it, so all that is moved to
    // where the body's own ribcage is, as it hangs there.
    let (animated, drawn_ribcage) = body.ribcages();
    let rebase = match (animated.and_then(|ribcage| helper.compute_global_transform(ribcage).ok()), on_rig(drawn_ribcage)) {
        (Some(ribcage), Some(drawn)) => ribcage.affine() * drawn.inverse(),
        _ => Affine3A::IDENTITY,
    };
    let drawn = |entity: Entity| -> Option<Affine3A> { Some(if under(entity, weapon.root, &parents) { rebase * on_rig(entity)? } else { on_rig(entity)? }) };
    // Each bone of the corpse where the body has it; one the body does not
    // draw stands where its parent is.
    let mut world: Vec<Affine3A> = Vec::with_capacity(corpse.bones.len());
    let mut locals = Vec::with_capacity(corpse.bones.len());
    for (index, path) in corpse.paths.iter().enumerate() {
        let parent = corpse.parents[index].map_or(Affine3A::IDENTITY, |parent| world[parent]);
        let here = body.drawn(&parts, &weapon, path).and_then(drawn).unwrap_or(parent);
        world.push(here);
        let local = transform_of(parent.inverse() * here);
        locals.push(local);
        commands.entity(corpse.bones[index]).insert(local);
    }
    let bone_of = |name: &str| corpse.paths.iter().position(|path| path.rsplit('/').next() == Some(name));
    let game_pose = |bone: usize| {
        let (_, turn, at) = world[bone].to_scale_rotation_translation();
        to_game(Vec3::from(at), turn)
    };

    // The engine, as the install sets it up, and the range: its boxes on the
    // layer the world's colliders are on, and the ground.
    let mut engine = crate::world::loose::range_world(physics, &range);
    let deadbody = physics.layer("Deadbody").unwrap_or(23);
    // The corpse.
    let poses: HashMap<String, ([f32; 3], [f32; 4])> = data.posed_bones().into_iter().filter_map(|bone| Some((bone.to_owned(), game_pose(bone_of(bone)?)))).collect();
    let bodies = trp_physics::corpse::build(&mut engine, data, &poses, controller.movement.velocity, ragdoll.max_depenetration, deadbody);
    let mut moved = vec![None; corpse.bones.len()];
    for (bone, body) in &bodies {
        if let Some(index) = bone_of(bone) {
            moved[index] = Some(*body);
        }
    }
    let mass_of = |bone: &str| data.bodies.iter().find(|b| b.bone == bone).map_or(1.0, |b| b.mass);

    // The weapon in hand, a body of its own: the box its meshes fill on the
    // rig, hung from the pelvis by a spring, or a pistol held fast to the
    // right forearm.
    let (mut held, mut weapon_rest) = (None, None);
    if let Some(weapon_root) = weapon.weapon_root.filter(|_| controller.armed) {
        if let Some(at) = drawn(weapon_root) {
            let into = at.inverse();
            // The weapon's own colliders, as `DropItemDead` switches them on:
            // every one of its parts' (switched off in the prefab, switched on
            // then) but triggers and those on the shells'
            // layer, in the weapon's axes as the game has them. A mesh
            // collider is not taken (`PROVISIONAL`: none on the weapons
            // looked at); a weapon with none falls as the box its meshes
            // fill (`PROVISIONAL`).
            let shells = physics.layer("Shells");
            let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            let mut colliders = Vec::new();
            let mut stack = vec![weapon_root];
            while let Some(entity) = stack.pop() {
                let shown = seen.get(entity).is_ok_and(|visible| visible.get());
                if let (true, Some(global)) = (shown, drawn(entity)) {
                    let to_weapon = into * global;
                    if let Ok(aabb) = meshes.get(entity) {
                        for corner in 0..8 {
                            let sign = Vec3::new(if corner & 1 == 0 { -1.0 } else { 1.0 }, if corner & 2 == 0 { -1.0 } else { 1.0 }, if corner & 4 == 0 { -1.0 } else { 1.0 });
                            let point = to_weapon.transform_point3(Vec3::from(aabb.center) + Vec3::from(aabb.half_extents) * sign);
                            low = low.min(point);
                            high = high.max(point);
                        }
                    }
                    if let Ok(own) = node_colliders.get(entity) {
                        let (scale, turn, place) = to_weapon.to_scale_rotation_translation();
                        let (at, turn) = to_game(Vec3::from(place), turn);
                        for c in own.0.iter().filter(|c| !c.trigger && Some(c.layer as u16) != shells) {
                            colliders.extend(trp_physics::loose::collider(c, at, turn, scale.abs().to_array(), &mut |_| None));
                        }
                    }
                }
                if let Ok(below) = children.get(entity) {
                    stack.extend(below.iter());
                }
            }
            if std::env::var("TRP_DEATH_DEBUG").is_ok() {
                let mut all = vec![weapon_root];
                let (mut with, mut shown_with) = (0, 0);
                while let Some(entity) = all.pop() {
                    if let Ok(own) = node_colliders.get(entity) {
                        with += own.0.len();
                        if seen.get(entity).is_ok_and(|visible| visible.get()) {
                            shown_with += own.0.len();
                        }
                    }
                    if let Ok(below) = children.get(entity) {
                        all.extend(below.iter());
                    }
                }
                info!("under the weapon's bone: {with} colliders, {shown_with} on shown objects; {} taken", colliders.len());
            }
            if colliders.is_empty() && low.cmple(high).all() {
                let half = ((high - low) / 2.0).max(Vec3::splat(0.01));
                let centre = (low + high) / 2.0;
                colliders.push(Collider { shape: Shape::Box { half: half.to_array() }, at: [centre.x, centre.y, -centre.z], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() });
            }
            if !colliders.is_empty() {
                let pistol = weapon.kind == trp_sim::locomotion::body::hands_kind::PISTOL;
                let mass = if pistol { PISTOL_MASS } else { WEAPON_MASS };
                let (_, turn, place) = at.to_scale_rotation_translation();
                let (weapon_at, weapon_turn) = to_game(Vec3::from(place), turn);
                let count = colliders.len();
                let body = engine.add_body(&BodyDesc {
                    at: weapon_at,
                    turn: weapon_turn,
                    colliders,
                    mass,
                    drag: 0.0,
                    angular_drag: UNITY_ANGULAR_DRAG,
                    // The character's velocity, not held as the corpse's is.
                    velocity: controller.movement.velocity,
                    spin: [0.0; 3],
                    max_depenetration: ragdoll.max_depenetration,
                    continuous: false,
                    layer: deadbody,
                });
                let palm = bone_of("Base HumanRPalm").map(|bone| game_pose(bone).0);
                match (pistol, bodies.get("Base HumanRForearm1"), bodies.get("Base HumanPelvis"), palm) {
                    (true, Some(forearm), ..) => engine.fixed_joint(body, *forearm, mass_of("Base HumanRForearm1") / mass),
                    (false, _, Some(pelvis), Some(palm)) => {
                        // The palm in the weapon's axes, and the strap's point
                        // by it.
                        let back = trp_physics::inverse(weapon_turn);
                        let palm = trp_physics::rotate(back, [palm[0] - weapon_at[0], palm[1] - weapon_at[1], palm[2] - weapon_at[2]]);
                        let on_weapon = [palm[0] + STRAP_BY_PALM[0], palm[1] + STRAP_BY_PALM[1], palm[2] + STRAP_BY_PALM[2]];
                        engine.spring_joint(body, on_weapon, *pelvis, STRAP_ON_PELVIS, STRAP_SPRING, STRAP_DAMPER, [0.0, 0.0], true, mass_of("Base HumanPelvis") / mass);
                    }
                    _ => {}
                }
                info!("the weapon let go on {count} colliders");
                engine.report_touches(body);
                held = Some((body, weapon_root));
                weapon_rest = locals_of.get(weapon_root).ok().map(|local| (weapon_root, *local));
            }
        }
    }

    // The eyes ride the chest, where the camera's place hangs: where they
    // are in the chest's bone now.
    let eyes = eye.single().ok().and_then(|eye| helper.compute_global_transform(eye).ok()).zip(bone_of("Base HumanSpine3").and_then(|bone| Some((moved[bone]?, bone)))).map(|(seen, (chest, bone))| {
        (chest, transform_of(world[bone].inverse() * seen.affine()))
    });

    // The corpse shows what the body has on, and the body itself goes.
    for (twin, mut visibility) in &mut twins {
        if let Ok(original) = originals.get(twin.0) {
            *visibility = *original;
        }
    }
    commands.entity(corpse.root).insert(Visibility::Inherited);
    commands.entity(body.root()).insert(Visibility::Hidden);
    let outfit = outfit_on.item.as_ref().filter(|_| outfit_on.body).and_then(|item| corpse.outfits.iter().position(|outfit| &outfit.item == item));
    info!("dead: {} bodies and {} joints in PhysX, the weapon {}; a step of {:.4} s a frame", bodies.len(), data.joints.len(), if held.is_some() { "let go" } else { "none" }, clock.now);
    // Its landing: the weapon in hand's sound, a body of the weapon's mass,
    // counted from the world's nought (`DropItemDead` keeps no place).
    let kind = hands_held
        .item
        .and_then(|id| carried.find(id))
        .and_then(|item| game.db.items.get(&item.kind.template))
        .and_then(|t| t.props.get("DropSoundType")?.as_str().map(trp_data::audio::sound::drop_kind))
        .unwrap_or(0);
    let mass = if weapon.kind == trp_sim::locomotion::body::hands_kind::PISTOL { PISTOL_MASS } else { WEAPON_MASS };
    let landing = trp_sim::sound::landing::Landing::new(kind, mass, 0.0);
    let surfaces = bodies.iter().filter_map(|(bone, body)| Some((*body, data.bodies.iter().find(|b| &b.bone == bone)?.colliders.first()?.surface))).collect();
    dying.0 = Some(Death { since: 0.0, locals, moved, outfit, eyes, weapon: held, weapon_rest, engine, landing, surfaces });
    // The body heard as it falls, as the corpse is made: at the character's
    // feet, on what the line down from them meets (`step_sound::play_falls`).
    fell.write(crate::audio::step_sound::BodyFell::at(controller.movement.position, &*range, true));
}

/// While dying: the engine stepped as the game steps it, each moved bone of
/// the corpse where its body is, its outfit on it, the weapon where its body
/// is, and the eyes riding the chest; the living body's hands out of sight.
#[allow(clippy::too_many_arguments)]
pub fn follow(
    mut dying: ResMut<Dying>,
    corpse: Option<Res<Corpse>>,
    weapon: Option<Res<WeaponRig>>,
    view: Res<View>,
    (time, clock): (Res<Time>, Res<PhysicsClock>),
    parents: Query<&ChildOf>,
    mut transforms: ParamSet<(TransformHelper, Query<&mut Transform>)>,
    camera: Query<Entity, With<MainCamera>>,
    mut hands: Query<&mut Visibility, Or<(With<FirstPersonHands>, With<OutfitArms>)>>,
    mut head: Query<&mut crate::rendering::shadow_only::ShadowOnly, (With<CorpseHead>, crate::platform::park::EvenParked)>,
    (range, mut landed): (Res<Range>, MessageWriter<crate::audio::landing_sound::Landed>),
) {
    let (Some(death), Some(corpse)) = (dying.0.as_mut(), corpse) else { return };
    death.since += time.delta_secs();
    death.engine.step(clock.now);
    // The weapon's touches heard (a body of the corpse by its surface).
    if let Some((body, _)) = death.weapon {
        let surfaces = &death.surfaces;
        landed.write_batch(crate::audio::landing_sound::hear(&mut death.engine, body, &mut death.landing, death.since, &range, |met| surfaces.get(&met).copied()));
    }
    let wish = crate::rendering::shadow_only::ShadowOnly(*view == View::FirstPerson);
    for mut shadow_only in &mut head {
        if *shadow_only != wish {
            *shadow_only = wish;
        }
    }
    for mut visibility in &mut hands {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
    // The bones, a parent before what hangs from it.
    let mut world: Vec<Affine3A> = Vec::with_capacity(corpse.bones.len());
    for (index, local) in death.locals.iter().enumerate() {
        let parent = corpse.parents[index].map_or(Affine3A::IDENTITY, |parent| world[parent]);
        let here = match death.moved[index] {
            Some(body) => {
                let (at, turn) = death.engine.pose(body);
                from_game(at, turn)
            }
            None => parent * local.compute_affine(),
        };
        world.push(here);
    }
    {
        let mut set = transforms.p1();
        for (index, moved) in death.moved.iter().enumerate() {
            if moved.is_some() {
                let parent = corpse.parents[index].map_or(Affine3A::IDENTITY, |parent| world[parent]);
                if let Ok(mut transform) = set.get_mut(corpse.bones[index]) {
                    *transform = transform_of(parent.inverse() * world[index]);
                }
            }
        }
        // The outfit, on the corpse's bones as they are.
        if let Some(outfit) = death.outfit.and_then(|at| corpse.outfits.get(at)) {
            use trp_data::character::outfit::Move;
            let theirs: Vec<Move> = outfit
                .theirs
                .iter()
                .zip(&outfit.rest)
                .map(|(bone, rest)| {
                    let moved = bone.map_or(Affine3A::IDENTITY, |bone| world[bone]) * *rest;
                    Move { axes: [moved.matrix3.x_axis.to_array(), moved.matrix3.y_axis.to_array(), moved.matrix3.z_axis.to_array()], shift: moved.translation.to_array() }
                })
                .collect();
            for (entity, moved) in outfit.ours.iter().zip(outfit.rig.pose(&theirs)) {
                let turn = Quat::from_mat3(&Mat3::from_cols(moved.axes[0].into(), moved.axes[1].into(), moved.axes[2].into())).normalize();
                if let Ok(mut transform) = set.get_mut(*entity) {
                    *transform = Transform { translation: moved.shift.into(), rotation: turn, scale: Vec3::ONE };
                }
            }
        }
        // The rig at its true size: no squeeze with nobody's eyes on it.
        if let Some(weapon) = weapon.as_ref() {
            if let Ok(mut root) = set.get_mut(weapon.root) {
                root.scale = Vec3::ONE;
            }
        }
    }
    // The weapon and the eyes: each in its parent's axes as those stand.
    let mut placed: Vec<(Entity, Transform)> = Vec::new();
    {
        let helper = transforms.p0();
        let in_parent = |entity: Entity, wanted: Affine3A| -> Option<Transform> {
            let parent = parents.get(entity).ok()?.parent();
            let parent = helper.compute_global_transform(parent).ok()?;
            Some(transform_of(parent.affine().inverse() * wanted))
        };
        if let Some((body, weapon_root)) = death.weapon {
            let (at, turn) = death.engine.pose(body);
            if let Some(local) = in_parent(weapon_root, from_game(at, turn)) {
                placed.push((weapon_root, local));
            }
        }
        if let (View::FirstPerson, Some((chest, eyes)), Ok(camera)) = (*view, death.eyes, camera.single()) {
            let (at, turn) = death.engine.pose(chest);
            if let Some(local) = in_parent(camera, from_game(at, turn) * eyes.compute_affine()) {
                placed.push((camera, local));
            }
        }
    }
    let mut set = transforms.p1();
    for (entity, local) in placed {
        if let Ok(mut transform) = set.get_mut(entity) {
            transform.translation = local.translation;
            transform.rotation = local.rotation;
        }
    }
}
