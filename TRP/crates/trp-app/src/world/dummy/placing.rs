//! Placing dummies, standing in the way, blasts on them and their falls.

use super::*;

/// Which frame their colliders are of from here to the next call (the
/// frame before's kept as well, `Dummy::before_frame`), their health
/// stepped where their characters did not follow it (they bleed: the dead
/// and fallen, or one with no character), and last frame's hits forgotten.
/// A standing dummy's colliders go by its character's newest frame, not by
/// what is drawn: what a bullet or a blast meets goes by the simulation
/// alone; they are placed only where a reader asks
/// (`Dummy::placement`), with the economy off here at once, as every frame
/// before. A fallen one's are placed here, as its ragdoll has its bones;
/// one whose character has had no frame yet, as its skeleton stands.
pub fn place_dummies(dummies: Option<ResMut<Dummies>>, rigs: Option<Res<DummyRigs>>, helper: TransformHelper, time: Res<Time>) {
    let (Some(mut dummies), Some(rigs)) = (dummies, rigs) else { return };
    let dummies = &mut *dummies;
    let began = std::time::Instant::now();
    for dummy in &mut dummies.list {
        let rig = &rigs.0[dummy.which];
        dummy.taken.clear();
        if dummy.dressed {
            if !std::mem::take(&mut dummy.followed) {
                dummy.health.step(time.delta_secs());
            }
            // A heavy bleeding's start and end, and its death, for the
            // drops on the ground; all for its voice.
            dummy.drain(dummies.listened, &mut dummies.bleeding_drops);
        }
        dummy.before_frame = dummy.colliders_frame;
        dummy.colliders_frame = dummy.newest_standing();
        match dummy.colliders_frame {
            Some(frame) => {
                dummy.placed.clear();
                dummy.objects.clear();
                if !dummies.economy {
                    let _ = dummy.placement(frame);
                }
            }
            None => {
                let bones: HashMap<usize, Affine3A> = dummy.colliders.iter().filter_map(|c| Some((c.bone, dummy.bone_now(rig, c.bone, &helper)?))).collect();
                dummy.placed = dummy.colliders.iter().map(|c| placed(c, bones.get(&c.bone).copied().unwrap_or(Affine3A::IDENTITY))).collect();
                dummy.objects = dummy
                    .colliders
                    .iter()
                    .map(|c| {
                        let at = (bones.get(&c.bone).copied().unwrap_or(Affine3A::IDENTITY) * c.local).translation;
                        [at.x, at.y, -at.z]
                    })
                    .collect();
            }
        }
        if std::env::var_os("TRP_DUMMY_DEBUG").is_some() && dummy.which == 0 && every_second() {
            let colliders = dummy.colliders_on(dummy.colliders_frame);
            for (c, p) in dummy.colliders.iter().zip(colliders.placed()).take(40).filter(|(c, _)| c.data.object.contains("Spine3") || c.data.object.contains("Head")) {
                let at = match p {
                    Placed::Sphere { centre, .. } | Placed::Box { centre, .. } => *centre,
                    Placed::Capsule { a, b, .. } => (*a + *b) / 2.0,
                };
                info!("dummy {} collider {} at {:.3?}", dummy.which, c.data.object, at);
            }
        }
    }
    dummies.posing_log.placing += began.elapsed().as_secs_f64();
}

/// The living dummies' capsules given to the range, which the character's
/// collision meets, so that the player collides with the bodies rather
/// than going through them: the
/// game's characters' controllers meet one another's (`Player` with
/// `Player` in the install's collision matrix) and never a dead body
/// (`Deadbody`), so a fallen dummy is walked over. Each as its character
/// stands (its stance's capsule, `capsule_figures`), the character's
/// radius. Before the character's move; the dummies move after it (since
/// 0.238.0, `run_dummies` after `controller::step`; before, as their
/// systems ran; `PROVISIONAL`: the game's order of characters not read).
/// The character's own capsule is in the list too, so that a
/// dummy's move meets it as a bot's meets the player's; each mover's own
/// is left out of its move (`Site::settle`).
pub fn stand_in_the_way(dummies: Option<Res<Dummies>>, controller: Res<crate::player::controller::Controller>, mut range: ResMut<crate::world::range::Range>) {
    let radius = range.radius;
    let character = controller.health.as_ref().is_none_or(|health| health.alive).then(|| {
        let [height, centre] = controller.movement.capsule_figures();
        trp_sim::locomotion::collision::Upright { feet: controller.movement.position, height, centre, radius }
    });
    let others: Vec<trp_sim::locomotion::collision::Upright> = dummies
        .iter()
        .flat_map(|dummies| &dummies.list)
        .filter(|dummy| dummy.dressed && dummy.health.alive && dummy.fall.is_none())
        .filter_map(|dummy| dummy.puppet.as_ref())
        .map(|puppet| {
            let [height, centre] = puppet.movement.capsule_figures();
            trp_sim::locomotion::collision::Upright { feet: puppet.movement.position, height, centre, radius }
        })
        .chain(character)
        .collect();
    if std::env::var_os("TRP_OTHERS_DEBUG").is_some() && every_second() {
        info!("others: {:?}", others);
    }
    range.set_others(others);
}

/// Each grenade's blast told of this frame (`loose::Blasted`) on every
/// living dummy (grenade spec, "The blast"; `trp_sim::blast`), as on any
/// character the game's blast finds: the lines from the blast to its hit
/// colliders as the last frame posed them and its plates in, from where it
/// stands now (the game's bots so: `courses3`'s `3-bot-blast`, the bones a
/// frame behind, the place the blast's frame's), what of the range they
/// meet (`Range::entered`, `PROVISIONAL`: nothing of the range pierced),
/// each part's damage given to its health as `GrenadeFragment` in the order
/// the game's colliders give the parts, and its armour's share
/// (`Wearer::take_blast`). Never contused (a bot is not); a death by it
/// pushes nothing (the blast's damage has no way: `Grenade.GetDamageInfo`).
/// A thrown grenade's here, before the dummies' colliders are said to be
/// of this frame (`place_dummies`); an explosive round's in the bullets'
/// frame (`shot_blast_dummies`). With the economy on, a dummy whose feet
/// the blast does not reach (`trp_sim::blast::reaches`) is not posed for
/// it: the blast does nothing to it either way.
pub fn blast_dummies(mut blasts: MessageReader<crate::world::loose::Blasted>, game: Res<crate::GameData>, range: Res<crate::world::range::Range>, dummies: Option<ResMut<Dummies>>) {
    match dummies {
        Some(mut dummies) => blast_on_dummies(blasts.read().filter(|blasted| !blasted.shot), &game, &range, &mut dummies, false),
        None => blasts.clear(),
    }
}

/// An explosive round's blast on the dummies, in the bullets' frame after
/// their tick (`bullets::fly_bullets`): the lines to their
/// colliders as the frame before posed them (`Dummy::before_frame`), from
/// where they stand now, as a thrown grenade's (`blast_dummies`), where
/// before the dummies were met as the frame's bullets meet them. The game's
/// order as read: a round's blast is the
/// grenade's own (`Grenade.Explosion`), run in the ballistics' tick, which
/// the game world's update runs after every character's update and before
/// the engine's animation writes the bones, so it meets each bot's bones as
/// its last late update left them, as a thrown grenade's does (whose
/// recording shows the bones a frame behind and the place the blast's
/// frame's). `PROVISIONAL`: no recording of a round's blast on a bot.
pub fn shot_blast_dummies(mut blasts: MessageReader<crate::world::loose::Blasted>, game: Res<crate::GameData>, range: Res<crate::world::range::Range>, dummies: Option<ResMut<Dummies>>) {
    match dummies {
        Some(mut dummies) => blast_on_dummies(blasts.read().filter(|blasted| blasted.shot), &game, &range, &mut dummies, true),
        None => blasts.clear(),
    }
}

/// The blasts on the living dummies, each dummy's colliders on the frame
/// before's (`after_placing`: the frame before `place_dummies`'s, a
/// round's; else the frame `place_dummies` last said, which is the frame
/// before's until it runs again, a grenade's).
fn blast_on_dummies<'a>(blasts: impl Iterator<Item = &'a crate::world::loose::Blasted>, game: &crate::GameData, range: &crate::world::range::Range, dummies: &mut Dummies, after_placing: bool) {
    let mut dummies_killed = Vec::new();
    let mut marks = Vec::new();
    let economy = dummies.economy;
    let (listened, drops) = (dummies.listened, &mut dummies.bleeding_drops);
    for blasted in blasts.filter(|blasted| blasted.blasts) {
        let Some(grenade) = crate::effects::blast_view::grenade_of(&game.db, &blasted.template) else { continue };
        let at = blasted.blast_point();
        let wall = crate::effects::blast_view::wall_absorption(&game.db);
        for dummy in dummies.list.iter_mut().filter(|dummy| dummy.dressed && dummy.health.alive && dummy.fall.is_none()) {
            let Some(feet) = dummy.puppet.as_ref().map(|puppet| puppet.movement.position) else { continue };
            if economy && !trp_sim::combat::blast::reaches(&grenade.explosive, at, feet) {
                continue;
            }
            let frame = if after_placing { dummy.before_frame.or(dummy.colliders_frame) } else { dummy.colliders_frame };
            let points: Vec<trp_sim::combat::blast::HitPoint> = {
                let colliders = dummy.colliders_on(frame);
                let objects = colliders.objects();
                let plates = dummy.wearer.plates_on();
                dummy
                    .colliders
                    .iter()
                    .zip(objects)
                    .filter(|(c, _)| c.body.plate == 0)
                    .chain(dummy.colliders.iter().zip(objects).filter(|(c, _)| c.body.plate != 0 && plates & c.body.plate == c.body.plate))
                    .filter_map(|(c, at)| Some(trp_sim::combat::blast::HitPoint { part: *Part::ALL.get(c.body.part as usize)?, at: *at, collider: c.body.collider, plate: c.body.plate }))
                    .collect()
            };
            let line = |from: [f32; 3], to: [f32; 3]| range.entered(from, to).map_or(trp_sim::combat::blast::Line::Through(0.0), trp_sim::combat::blast::Line::Stopped);
            let outcome = trp_sim::combat::blast::blast(&grenade.explosive, at, feet, &points, &line, wall);
            if outcome.damage.is_empty() && outcome.armor.is_empty() {
                continue;
            }
            // Each part's damage as a hit of its own (`ApplyDamageInfo`), its
            // part first for the voice; none once dead.
            for (part, damage) in &outcome.damage {
                dummy.drain(listened, drops);
                if listened && dummy.health.alive {
                    dummy.heard.push(Heard::Struck(*part, *damage, trp_sim::condition::health::DamageKind::GrenadeFragment));
                }
                dummy.health.apply_damage(*part, *damage, trp_sim::condition::health::DamageKind::GrenadeFragment);
                dummy.drain(listened, drops);
            }
            // A death by the player's grenade: a kill (`OnEnemyKill` takes a
            // thrown weapon's), not a hit (`ManageAggressor` leaves them out).
            if !dummy.health.alive {
                let whole = Part::ALL.iter().map(|p| dummy.health.part(*p).maximum).sum();
                dummies_killed.push(DummyHit { killed: true, head: false, damage: 0.0, heavy: false, whole, bullet: false, melee: false });
            }
            let taken = dummy.wearer.take_blast(&outcome.armor);
            // Its armour's wear for its things (`keep_dummy_wear`).
            dummy.worn |= !taken.is_empty();
            marks.extend(dummy.wearer.drain().into_iter().map(|mark| (dummy.which, mark)));
            let far = (0..3).map(|k| (feet[k] - at[k]).powi(2)).sum::<f32>().sqrt();
            let parts: Vec<String> = Part::ALL.iter().map(|p| format!("{:.1}", dummy.health.part(*p).current)).collect();
            info!(
                "dummy {}: a blast {far:.2} m off, damage {:?}, its armour took {:?}; health [{}]{}",
                dummy.which,
                outcome.damage.iter().map(|(part, damage)| format!("{} {damage:.1}", part.name())).collect::<Vec<_>>(),
                taken.iter().map(|(id, damage)| format!("{id}: {damage:.2}")).collect::<Vec<_>>(),
                parts.join(", "),
                if dummy.health.alive { "" } else { ": dead" }
            );
        }
    }
    dummies.experience.extend(dummies_killed);
    dummies.marks.extend(marks);
}

/// For `TRP_DUMMY_DEBUG`'s lines: true on every sixtieth call.
fn every_second() -> bool {
    static FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 60 == 0
}

/// Their deaths (health spec, "Death"; `Corpse.CreateCorpse`,
/// `DropItemDead`, `ApplyCorpseImpulse`): a body made the game's ragdoll
/// where it stands, on the range's engine, the weapon in its hands let go
/// on a spring from the pelvis (the thing staying in its slot, kept as the
/// one in hand: `clear_dummy_weapons`), pushed by the killing hit the frame
/// after; then each frame stepped as the game steps it, its bones where its
/// bodies are and the weapon where its own is.
#[allow(clippy::too_many_arguments)]
pub fn fall_dummies(
    dummies: Option<ResMut<Dummies>>,
    rigs: Option<Res<DummyRigs>>,
    source: Option<Res<DummySource>>,
    ragdoll: Res<crate::player::death::Ragdoll>,
    range: Res<crate::world::range::Range>,
    clock: Res<crate::player::death::PhysicsClock>,
    (time, game, mut landed, mut fell, carried): (Res<Time>, Res<crate::GameData>, MessageWriter<crate::audio::landing_sound::Landed>, MessageWriter<crate::audio::step_sound::BodyFell>, Res<crate::screens::inventory_view::Carried>),
    (parents, children, meshes, node_colliders, seen): (
        Query<&ChildOf>,
        Query<&Children>,
        Query<&Aabb, (With<Mesh3d>, Without<SkinnedMesh>)>,
        Query<&crate::weapons::weapon::NodeColliders>,
        Query<&InheritedVisibility>,
    ),
    mut set: ParamSet<(TransformHelper, Query<&mut Transform>)>,
) {
    let (Some(mut dummies), Some(rigs), Some(source)) = (dummies, rigs, source) else { return };
    for dummy in &mut dummies.list {
        let rig = &rigs.0[dummy.which];
        if dummy.fall.is_none() && dummy.dressed && !dummy.health.alive {
            // A cut: drawn at its character's newest frame, which the
            // ragdoll and the weapon let go start from (as when that frame
            // was what was drawn), nothing drawn between it
            // and what was drawn the frame before (`Puppet::cut`); on the
            // animator's straight pose, whatever is drawn (a drawing option
            // moves no result: `SMOOTH ANIMATIONS` moved
            // its arms' start by up to 8.9 mm, the weapon's with them), and
            // whatever its size on screen left drawn. Its colliders go on
            // its skeleton from the next `place_dummies`.
            let newest = dummy.puppet.as_mut().and_then(|puppet| {
                puppet.cut();
                Some((puppet.straight_pose()?, puppet.firearm().is_some()))
            });
            if let Some((pose, firearm)) = newest {
                pose_dummy(dummy, rig, &pose, firearm, &mut set.p1());
            }
            dummy.held_frame = None;
            let helper = set.p0();
            let (Some(data), Some(physics)) = (ragdoll.data.as_ref(), ragdoll.physics.as_ref()) else { return };
            let mut engine = crate::world::loose::range_world(physics, &range);
            let deadbody = physics.layer("Deadbody").unwrap_or(23);
            let global = |entity: Entity| helper.compute_global_transform(entity).ok().map(|g| g.affine());
            let game_pose = |affine: Affine3A| {
                let (_, turn, at) = affine.to_scale_rotation_translation();
                ([at.x, at.y, -at.z], convert::rotation_back(turn))
            };
            let poses: HashMap<String, ([f32; 3], [f32; 4])> = data.posed_bones().into_iter().filter_map(|bone| Some((bone.to_owned(), game_pose(global(rig.bones[rig.bone(bone)?])?)))).collect();
            let bodies = trp_physics::corpse::build(&mut engine, data, &poses, [0.0; 3], ragdoll.max_depenetration, deadbody);
            let mut moved = vec![None; rig.bones.len()];
            for (bone, body) in &bodies {
                if let Some(index) = rig.bone(bone) {
                    moved[index] = Some(*body);
                }
            }
            // The weapon let go (`death::die`'s): its own colliders, else the
            // box its meshes fill, a body of 1 kg on the strap's spring from
            // the pelvis, from a point by the right palm; none where its
            // hands were empty. The thing in its hands is kept in its slot
            // (`Corpse.ItemInHands`).
            let in_hands = dummy.holds.filter(|&held| still_in_hands(carried.body(dummy.which), held));
            let mut weapon = None;
            if let Some(weapon_root) = dummy.model.as_ref().filter(|_| in_hands.is_some()).and_then(|m| m.weapon_root) {
                if let Some(at) = global(weapon_root) {
                    let into = at.inverse();
                    let shells = physics.layer("Shells");
                    let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                    let mut colliders: Vec<PhysicsCollider> = Vec::new();
                    let mut stack = vec![weapon_root];
                    while let Some(entity) = stack.pop() {
                        let shown = seen.get(entity).is_ok_and(|visible| visible.get());
                        if let (true, Some(g)) = (shown, global(entity)) {
                            let to_weapon = into * g;
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
                                let (at, turn) = ([place.x, place.y, -place.z], convert::rotation_back(turn));
                                for c in own.0.iter().filter(|c| !c.trigger && Some(c.layer as u16) != shells) {
                                    colliders.extend(trp_physics::loose::collider(c, at, turn, scale.abs().to_array(), &mut |_| None));
                                }
                            }
                        }
                        if let Ok(below) = children.get(entity) {
                            stack.extend(below.iter());
                        }
                    }
                    if colliders.is_empty() && low.cmple(high).all() {
                        let half = ((high - low) / 2.0).max(Vec3::splat(0.01));
                        let centre = (low + high) / 2.0;
                        colliders.push(PhysicsCollider { shape: PhysicsShape::Box { half: half.to_array() }, at: [centre.x, centre.y, -centre.z], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() });
                    }
                    if !colliders.is_empty() {
                        let (weapon_at, weapon_turn) = game_pose(at);
                        let body = engine.add_body(&BodyDesc {
                            at: weapon_at,
                            turn: weapon_turn,
                            colliders,
                            mass: crate::player::death::WEAPON_MASS,
                            drag: 0.0,
                            angular_drag: crate::player::death::UNITY_ANGULAR_DRAG,
                            velocity: [0.0; 3],
                            spin: [0.0; 3],
                            max_depenetration: ragdoll.max_depenetration,
                            continuous: false,
                            layer: deadbody,
                        });
                        let palm = rig.bone("Base HumanRPalm").and_then(|bone| global(rig.bones[bone])).map(|g| game_pose(g).0);
                        let mass_of = |bone: &str| data.bodies.iter().find(|b| b.bone == bone).map_or(1.0, |b| b.mass);
                        if let (Some(pelvis), Some(palm)) = (bodies.get("Base HumanPelvis"), palm) {
                            let back = trp_physics::inverse(weapon_turn);
                            let palm = trp_physics::rotate(back, [palm[0] - weapon_at[0], palm[1] - weapon_at[1], palm[2] - weapon_at[2]]);
                            let by = crate::player::death::STRAP_BY_PALM;
                            let on_weapon = [palm[0] + by[0], palm[1] + by[1], palm[2] + by[2]];
                            engine.spring_joint(body, on_weapon, *pelvis, crate::player::death::STRAP_ON_PELVIS, crate::player::death::STRAP_SPRING, crate::player::death::STRAP_DAMPER, [0.0, 0.0], true, mass_of("Base HumanPelvis") / crate::player::death::WEAPON_MASS);
                        }
                        engine.report_touches(body);
                        weapon = Some(body);
                    }
                }
            }
            // Its landing: the AKM's sound, a body of the weapon's mass,
            // counted from the world's nought (`DropItemDead` keeps no place).
            let kind = game.db.items.get(WEAPON).and_then(|t| t.props.get("DropSoundType")?.as_str().map(trp_data::audio::sound::drop_kind)).unwrap_or(0);
            let landing = trp_sim::sound::landing::Landing::new(kind, crate::player::death::WEAPON_MASS, 0.0);
            let surfaces = bodies.iter().filter_map(|(bone, body)| Some((*body, data.bodies.iter().find(|b| &b.bone == bone)?.colliders.first()?.surface))).collect();
            // The push: on the body whose own collider (its bone's) was hit.
            let push = dummy.killing.and_then(|k| {
                let c = dummy.colliders.get(k.collider)?;
                if c.data.local.is_some() || c.data.plate != 0 {
                    return None;
                }
                let body = bodies.get(&c.data.object)?;
                let force = source.hit_force * (0.3 + 0.7 * inverse_lerp(50.0, 20.0, k.penetration));
                Some((*body, std::array::from_fn(|i| k.direction[i] * force), k.point))
            });
            info!(
                "dummy {} dead: {} bodies in PhysX, the weapon {}{}",
                dummy.which,
                bodies.len(),
                if weapon.is_some() { "let go" } else { "none" },
                if push.is_some() { ", pushed by the killing hit" } else { "" }
            );
            // (Nothing hung where its hands were empty.)
            dummy.fall = Some(Fall { engine, moved, push, weapon, landing, since: 0.0, surfaces, in_hands, hung: in_hands.is_some() });
            // The body heard as it falls, as the corpse is made: at its feet
            // (its character's, else its rig's root), on what the line down
            // from them meets (`step_sound::play_falls`).
            let root = dummy.puppet.as_ref().map(|puppet| puppet.movement.position).or_else(|| global(rig.root).map(|g| game_pose(g).0));
            if let Some(root) = root {
                fell.write(crate::audio::step_sound::BodyFell::at(root, &*range, false));
            }
            continue;
        }
        let Some(fall) = dummy.fall.as_mut() else { continue };
        if let Some((body, impulse, point)) = fall.push.take() {
            fall.engine.impulse_at(body, impulse, point);
        }
        fall.engine.step(clock.now());
        fall.since += time.delta_secs();
        // The weapon's touches heard: a body of the corpse by its hit
        // colliders' surface (`BodyPartCollider.SurfaceSound`).
        if let Some(body) = fall.weapon {
            let surfaces = &fall.surfaces;
            landed.write_batch(crate::audio::landing_sound::hear(&mut fall.engine, body, &mut fall.landing, fall.since, &range, |met| surfaces.get(&met).copied()));
        }
        static FALL_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if std::env::var_os("TRP_DUMMY_DEBUG").is_some() && FALL_FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 60 == 0 {
            let pelvis = rig.bone("Base HumanPelvis").and_then(|b| fall.moved[b]).map(|b| fall.engine.pose(b).0);
            let weapon = fall.weapon.map(|b| fall.engine.pose(b).0);
            info!("dummy {} falling: the pelvis at {pelvis:.2?}, the weapon at {weapon:.2?}", dummy.which);
        }
        // The weapon in its parent where its body is.
        let weapon_local = match (fall.weapon, dummy.model.as_ref().and_then(|m| m.weapon_root)) {
            (Some(body), Some(entity)) => {
                let (at, turn) = fall.engine.pose(body);
                let wanted = Affine3A::from_rotation_translation(convert::rotation(turn), convert::position(at));
                let helper = set.p0();
                parents.get(entity).ok().and_then(|p| helper.compute_global_transform(p.parent()).ok()).map(|parent| (entity, Transform::from_matrix(Mat4::from(parent.affine().inverse() * wanted))))
            }
            _ => None,
        };
        let mut transforms = set.p1();
        let root = transforms.get(rig.root).map_or(Affine3A::IDENTITY, |t| t.compute_affine());
        let world = world_of(&rig.parents, &dummy.locals, &fall.moved, Some(&fall.engine), root);
        for (index, moved) in fall.moved.iter().enumerate() {
            if moved.is_some() {
                let parent = rig.parents[index].map_or(Affine3A::IDENTITY, |p| world[p]);
                if let Ok(mut transform) = transforms.get_mut(rig.bones[index]) {
                    *transform = Transform::from_matrix(Mat4::from(parent.inverse() * world[index]));
                }
            }
        }
        if let Some((entity, local)) = weapon_local {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                transform.translation = local.translation;
                transform.rotation = local.rotation;
            }
        }
        pose_outfits(rig, &world, &mut transforms);
    }
}

/// Unity's `Mathf.InverseLerp`.
fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a != b { ((value - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 }
}
