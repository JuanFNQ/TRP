//! Setting loose things going.

use super::*;

/// What the inventory has let go this frame is set going as the game sets
/// it going; a thing on the ground with nowhere yet (the range's, or what
/// a restart put back) is set down; a thing taken up goes. And the weapon
/// in hand draws its magazine or not as its reload says.
#[allow(clippy::too_many_arguments)]
pub fn set_going(
    mut commands: Commands,
    mut loose: ResMut<Loose>,
    mut carried: ResMut<Carried>,
    mut controller: ResMut<Controller>,
    time: Res<Time>,
    weapon: Option<Res<WeaponRig>>,
    (hands, game, skills): (Option<Res<crate::weapons::weapon::HandsRig>>, Res<crate::GameData>, Option<Res<crate::player::skills::Skills>>),
    helper: TransformHelper,
    (names, children, models, locals): (Query<&Name>, Query<&Children>, Query<(Entity, &LooseModel)>, Query<&Transform>),
    mut again: MessageReader<crate::player::death::BeginAgain>,
    (pointed, dragging, menu): (Option<Res<crate::screens::inventory_view::Pointed>>, Option<Res<crate::screens::inventory_view::Dragging>>, Option<Res<crate::screens::item_menu_view::ItemMenu>>),
) {
    let buffs = skills.map(|skills| skills.buffs()).unwrap_or_default();
    let loose = &mut *loose;
    // Grenades gone at their fuse leave the ground.
    if !loose.blown.is_empty() {
        let blown = std::mem::take(&mut loose.blown);
        carried.ground.retain(|(item, _)| !blown.contains(&item.id));
    }
    // A run begun again puts back what lay on the ground at its start, each
    // set down afresh.
    if again.read().count() > 0 {
        loose.flights.clear();
        loose.places.clear();
        loose.uses.stale = true;

        for (entity, _) in &models {
            commands.entity(entity).despawn();
        }
    }
    // The weapon's magazine, as the game finds it (`GetModTransform`: the
    // first object under the weapon's magazine slot, looked for from its
    // root), drawn as the rig is, the first person's squeeze along the view
    // taken off.
    let magazine = weapon.as_ref().and_then(|rig| {
        let from = rig.weapon_root?;
        let mut stack = vec![from];
        while let Some(entity) = stack.pop() {
            if names.get(entity).is_ok_and(|name| name.as_str() == "mod_magazine") {
                return children.get(entity).ok().and_then(|below| below.first().copied());
            }
            stack.extend(children.get(entity).into_iter().flatten());
        }
        None
    });
    let unsqueezed = |entity: Entity| -> Option<(V3, [f32; 4])> {
        let rig = weapon.as_ref()?;
        let root = helper.compute_global_transform(rig.root).ok()?;
        let (_, turn, at) = root.to_scale_rotation_translation();
        let now = helper.compute_global_transform(entity).ok()?;
        let (_, turn_now, at_now) = (Affine3A::from_rotation_translation(turn, at) * root.affine().inverse() * now.affine()).to_scale_rotation_translation();
        Some(([at_now.x, at_now.y, -at_now.z], convert::rotation_back(turn_now)))
    };
    if let (Some(shown), Some(entity)) = (controller.magazine_drawn.take(), magazine) {
        commands.entity(entity).insert(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
    // With the options' `ITEM PHYSICS` off nothing more: what is put down
    // lies as a box where it is put (`inventory_view::show_ground`).
    if loose.physics.is_none() {
        if !carried.thrown.is_empty() {
            carried.bypass_change_detection().thrown.clear();
        }
        return;
    }
    let m = &controller.movement;
    let [height, _] = m.capsule_figures();
    // A thing put down or dropped is turned as the look, not as the hands'
    // root: the game's `WeaponRoot` at a throw is the look turned back 90
    // about its x in every recorded throw (`loose::look_root`).
    let thrown_by = Thrower {
        yaw: m.yaw,
        pitch: m.pitch,
        chest: [m.position[0], m.position[1] + height * 0.65, m.position[2]],
        velocity: m.velocity,
        dt: time.delta_secs(),
        weapon_root: loose::look_root(m.pitch, m.yaw),
    };
    let magazine_at = magazine.and_then(|entity| unsqueezed(entity));
    // The weapon's first shell port (`_shellPortsTransforms[0]`) as the rig
    // draws it, the squeeze off (the game throws the round at the hands'
    // mark, heard in `Update`, before the squeeze of `LateUpdate`), with its
    // move in its parent from where it was as the weapon was made
    // (`_initialShellPortPosition`) and the round's throw.
    let port_at = weapon.as_ref().and_then(|rig| {
        let port = rig.shell_port.as_ref()?;
        let local = locals.get(port.entity).ok()?.translation;
        let moved = [local.x - port.rest[0], local.y - port.rest[1], -local.z - port.rest[2]];
        Some((unsqueezed(port.entity)?, moved, port.patron))
    });
    // What a throw of the grenade in hand goes by.
    let thrown_data = hands.as_ref().and_then(|hands| hands.thrown.as_ref()).map(|(_, data)| data.clone());
    // The grenade in hand's flying model, read before it is thrown: its key
    // made, and the model asked for, as the grenade in hand changes.
    let in_hand = hands.as_ref().and_then(|hands| hands.thrown.as_ref()).map(|(template, _)| template.as_str());
    if loose.held.as_deref().and_then(|held| held.strip_prefix(GRENADE_MODEL)) != in_hand {
        loose.held = in_hand.map(|template| format!("{GRENADE_MODEL}{template}"));
        loose.uses.stale = true;
        if let Some(held) = loose.held.clone() {
            loose.ask(&held);
        }
    }
    // What the inventory screen may put down next: the models' reader kept
    // while it is open, and a weapon's model asked for at the first sign
    // (`Foreseen`).
    let signs = Signs {
        open: controller.inventory_open,
        pointed: pointed.and_then(|pointed| pointed.0),
        taken: dragging.and_then(|dragging| dragging.0),
        menu: menu.and_then(|menu| menu.0.as_ref().map(|menu| menu.id)),
    };
    loose.foresee(time.elapsed_secs(), signs, &carried, carried.last_changed().get());
    // Where the hand let the grenade go, worked out on the simulation's
    // frame of the throw from the rig as that frame drew it
    // (`controller::GrenadeRelease`), not from what is drawn now, which lies
    // between the simulation's frames.
    let release = controller.grenade_release.take();
    // (Taken without telling of a change to what is carried: the screen
    // is drawn afresh on one.)
    let thrown = if carried.thrown.is_empty() { Vec::new() } else { std::mem::take(&mut carried.bypass_change_detection().thrown) };
    for (id, how) in thrown {
        let Some((item, at)) = carried.ground.iter().find(|(item, _)| item.id == id) else { continue };
        let mut grenade = None;
        // A weapon's model is its build's, a round's an unspent round's
        // (`model_key`).
        let mut template = model_key(item);
        let start = match (how, magazine_at) {
            (How::LetFall, Some((at, turn))) => loose::let_fall(at, turn, &mut loose.dice),
            // No weapon to let it fall from: where it would lie.
            (How::LetFall, None) => loose::let_fall([at.x, at.y + 1.0, -at.z], [0.0, 0.0, 0.0, 1.0], &mut loose.dice),
            (How::Thrown, _) => loose::thrown(&thrown_by, false, &mut loose.dice),
            (How::FromPort, _) => match port_at {
                Some(((at, turn), moved, throw)) => loose::from_port(at, turn, moved, &throw, &mut loose.dice),
                // No port drawn to throw from: let fall where it would lie.
                None => loose::let_fall([at.x, at.y + 1.0, -at.z], [0.0, 0.0, 0.0, 1.0], &mut loose.dice),
            },
            (How::Grenade { low }, _) => match (thrown_data.as_ref(), release) {
                (Some(data), Some(release)) => {
                    // Where it leaves and how it is turned: the hand's
                    // grenade as the frame of the throw drew it, and the
                    // offset turned with it (`GrenadeRelease`). The line back
                    // to the body's front is not tried against the world
                    // (`CheckHandsToBodyObstacles`: PROVISIONAL).
                    let leaves = release.at;
                    // A direction on the unit sphere for the throw's wobble.
                    let (z, around) = (loose.dice.range(-1.0, 1.0), loose.dice.range(0.0, std::f32::consts::TAU));
                    let across = (1.0 - z * z).max(0.0).sqrt();
                    let input = trp_sim::held::grenade_hands::ThrowInput {
                        force: loose.grenade_force,
                        low,
                        // Strength's throw (a high throw's strength) and
                        // Throwing's elite (`skills`).
                        strength_buff: buffs.strength_throw,
                        elite: buffs.throwing_elite,
                        hands_stamina: controller.movement.stamina.arms.level(),
                        // The body's `WeaponRoot` on the frame of the throw,
                        // whose down is the look (recorded, within 0.02
                        // degrees: `bin/grenade_knife`).
                        drop_up: trp_physics::rotate(loose::look_root(release.pitch, release.yaw), [0.0, 1.0, 0.0]),
                        random: [across * around.cos(), across * around.sin(), z],
                        velocity: controller.movement.velocity,
                    };
                    let force = trp_sim::held::grenade_hands::throw_force(&input);
                    let body = data.body;
                    // Its fuse from its item (`explDelay`, the field the
                    // game reads), with no time spent on it: the player's
                    // throw.
                    let fuse = game.db.items.get(&template).and_then(|item| item.props["explDelay"].as_f64()).unwrap_or(0.0) as f32;
                    grenade = Some(GrenadeBody {
                        mass: body.mass,
                        drag: body.drag,
                        angular_drag: body.angular_drag,
                        continuous: body.collision_detection != 0,
                        colliders: loose::colliders(&data.thrown, &mut |_| None),
                        fuse,
                        item: template.clone(),
                    });
                    template = format!("{GRENADE_MODEL}{template}");
                    // Where it left the hand from the eye, in the level
                    // look's axes (right, up, forward), as `bin/grenade_knife`
                    // gives the recorded throws'.
                    let (s, c) = release.yaw.to_radians().sin_cos();
                    let from_eye: [f32; 3] = std::array::from_fn(|k| release.at[k] - release.eye[k]);
                    let seen = [from_eye[0] * c - from_eye[2] * s, from_eye[1], from_eye[0] * s + from_eye[2] * c];
                    info!(
                        "a grenade thrown ({}): force {force:?}, fuse {fuse} s; it left the hand {seen:.3?} from the eye (right, up, forward), the look {:.2} down",
                        if low { "from below" } else { "from above" },
                        release.pitch
                    );
                    Start { at: leaves, turn: release.turn, velocity: force.map(|f| f / body.mass.max(1e-6)), spin: [0.0; 3] }
                }
                _ => {
                    warn!("{template}: no grenade in the hand to throw from; it is put down instead");
                    loose::thrown(&thrown_by, false, &mut loose.dice)
                }
            },
        };
        let hidden_for = if how == How::Thrown { loose.hidden_for } else { 0.0 };
        let mass = item.weight();
        // Its landing sound by its template (`DropSoundType`), counted from
        // where it sets off (`ThrowItem`'s `CacheParameters`); its body's
        // mass is the thing's whole weight (`CreateLootWithRigidbody`).
        let kind = game.db.items.get(&item.kind.template).and_then(|t| t.props.get("DropSoundType")?.as_str().map(trp_data::audio::sound::drop_kind)).unwrap_or(0);
        let landing = (kind != 0 && grenade.is_none()).then(|| trp_sim::sound::landing::Landing::new(kind, mass, start.at[1]));
        loose.flights.retain(|f| f.id != id);
        loose.places.remove(&id);
        info!("{template} set going ({how:?}) at {:?}, turned {:?}, at {:?} m/s; the character at {:?}", start.at, start.turn, start.velocity, controller.movement.position);
        loose.flights.push(Flight { id, template: template.clone(), mass, start, placed: false, hidden_for, since: 0.0, missed: Vec::new(), moving: None, grenade, landing });
        loose.uses.stale = true;
        loose.ask(&template);
    }
    // Set down: the rest of what lies on the ground with nowhere yet.
    let unplaced: Vec<(Id, String, f32, Vec3)> = carried
        .ground
        .iter()
        .filter(|(item, _)| !loose.places.contains_key(&item.id) && !loose.flights.iter().any(|f| f.id == item.id))
        .map(|(item, at)| (item.id, model_key(item), item.weight(), *at))
        .collect();
    for (id, template, mass, at) in unplaced {
        if matches!(loose.models.get(&template), Some(Asked::Failed)) {
            continue;
        }
        let start = Start { at: [at.x, at.y, -at.z], turn: [0.0, 0.0, 0.0, 1.0], velocity: [0.0; 3], spin: [0.0; 3] };
        // Set down by this work's layout: not a fall of the game's, and
        // not heard.
        loose.flights.push(Flight { id, template: template.clone(), mass, start, placed: true, hidden_for: 0.0, since: 0.0, missed: Vec::new(), moving: None, grenade: None, landing: None });
        loose.uses.stale = true;
        loose.ask(&template);
    }
    // Taken up, or gone with a restart: its flight, place and model go.
    let lying: std::collections::HashSet<Id> = carried.ground.iter().map(|(item, _)| item.id).collect();
    let had = (loose.flights.len(), loose.places.len());
    loose.flights.retain(|f| lying.contains(&f.id));
    loose.places.retain(|id, _| lying.contains(id));
    if had != (loose.flights.len(), loose.places.len()) {
        loose.uses.stale = true;
    }

    for (entity, model) in &models {
        if !lying.contains(&model.0) {
            commands.entity(entity).despawn();
        }
    }
}
