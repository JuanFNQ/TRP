//! Loose things flown each frame and placed.

use super::*;

/// Every thing on its way down, a step of the game's physics this frame
/// (`death::PhysicsClock`): its body made once its model is there and
/// stepped through what it missed, its watch kept, and its model drawn
/// where the body is; done, its body goes and it lies where it is.
#[allow(clippy::too_many_arguments)]
pub fn fly(
    mut commands: Commands,
    mut loose: ResMut<Loose>,
    clock: Res<crate::player::death::PhysicsClock>,
    time: Res<Time>,
    range: Res<Range>,
    carried: Res<Carried>,
    mut models: Query<(&LooseModel, &mut Transform, &mut Visibility)>,
    mut blasts: MessageWriter<Blasted>,
    mut landed: MessageWriter<crate::audio::landing_sound::Landed>,
    game: Res<crate::GameData>,
) {
    let loose = &mut *loose;
    let Some(physics) = loose.physics.clone() else { return };
    let (step, frame) = (clock.now(), time.delta_secs());
    let layer = physics.layer("Loot").unwrap_or(15);
    let mut done = Vec::new();
    let mut blown = Vec::new();
    for (index, flight) in loose.flights.iter_mut().enumerate() {
        flight.since += frame;
        let ready = match loose.models.get(&flight.template) {
            Some(Asked::Ready(ready)) => ready.clone(),
            Some(Asked::Failed) => {
                done.push(index);
                continue;
            }
            _ => {
                flight.missed.push((step, frame));
                continue;
            }
        };
        if flight.moving.is_none() {
            // The rounds it shows (`MAGAZINE ROUNDS`), each with its model:
            // its body is made once they are all there, with their
            // colliders (`CreateLootPrefab` switches on every collider in
            // it: `docs/spec/physics.md`, "Loose things").
            let shown = match carried.ground.iter().find(|(item, _)| item.id == flight.id) {
                Some((item, _)) if loose.rounds && flight.grenade.is_none() => shown_rounds(item, &ready),
                _ => Vec::new(),
            };
            let mut hung: Vec<(usize, Arc<Ready>, bool)> = Vec::with_capacity(shown.len());
            let mut waiting = false;
            for (node, round, chamber) in shown {
                let key = format!("{ROUND_MODEL}{round}");
                // Kept as `model_for` keeps it: a chamber's round is drawn
                // with it.
                if !loose.kept.contains(&key) {
                    loose.kept.push(key.clone());
                    loose.uses.stale = true;
                }
                match loose.models.get(&key) {
                    Some(Asked::Ready(model)) => hung.push((node, model.clone(), chamber)),
                    Some(Asked::Failed) => {}
                    Some(Asked::Waiting) => waiting = true,
                    None => {
                        loose.models.insert(key.clone(), Asked::Waiting);
                        loose.queue.push_back(key);
                        waiting = true;
                    }
                }
            }
            if waiting {
                flight.missed.push((step, frame));
                continue;
            }
            let mut colliders = ready.colliders.clone();
            let names: Vec<&str> = ready.names.iter().map(String::as_str).collect();
            for (node, round, _) in &hung {
                if let Some(body) = &round.round {
                    colliders.extend(loose::rounds_colliders(&ready.placed, &names, &[(*node, &body.hierarchy)], &mut |mesh| body.convex.get(&mesh).cloned()).into_iter().map(|(_, c)| c));
                }
            }
            let mut world = range_world(&physics, &range);
            let mut start = flight.start;
            if flight.placed {
                // Set down: its colliders' underside a centimetre over the
                // ground where it is put.
                let lowest = loose::bounds(&colliders).map_or(0.0, |(low, _)| low[1]);
                start.at[1] += 0.01 - lowest;
            }
            let (body, swept) = match &flight.grenade {
                // A thrown grenade on its own body, on the `Default` layer
                // (`GrenadeFactory.Create`), set going by the throw's impulse.
                Some(grenade) => {
                    let desc = trp_physics::BodyDesc {
                        at: start.at,
                        turn: start.turn,
                        colliders: grenade.colliders.clone(),
                        mass: grenade.mass,
                        drag: grenade.drag,
                        angular_drag: grenade.angular_drag,
                        velocity: start.velocity,
                        spin: start.spin,
                        max_depenetration: world.settings().max_depenetration,
                        continuous: grenade.continuous,
                        layer: physics.layer("Default").unwrap_or(0),
                    };
                    (world.add_body(&desc), true)
                }
                None => {
                    let swept = loose::bounds(&colliders).is_some_and(|(low, high)| (high[0] - low[0]) * (high[1] - low[1]) * (high[2] - low[2]) <= loose.swept_under);
                    if !hung.is_empty() {
                        info!("{}: its body with the colliders of the {} rounds it shows, {} in all", flight.template, hung.len(), colliders.len());
                    }
                    let (body, _) = loose::add(&mut world, colliders, flight.mass, &start, loose.swept_under, layer);
                    (body, swept)
                }
            };
            if flight.landing.is_some() {
                world.report_touches(body);
            }
            let mut moving = Moving { world, body, swept, watch: Watch::default() };
            // How long its model was in coming, for a thing that left the
            // character (the game's object is there in the throw itself,
            // and seen once the throw's delay is over).
            if !flight.placed {
                let late = flight.missed.iter().fold(0.0, |late, (_, frame)| late + frame);
                info!(
                    "{}: its model was there {} frames ({late:.3} s) after it set off; seen from {:.3} s on (the game's delay {:.2} s)",
                    flight.template,
                    flight.missed.len(),
                    late.max(flight.hidden_for),
                    flight.hidden_for
                );
            }
            // What it missed while its model was read, heard as it would
            // have been. (A grenade is not watched to a stop: it flies on
            // to its fuse.)
            let mut stopped = false;
            let mut then = 0.0;
            for (missed, frame) in std::mem::take(&mut flight.missed) {
                moving.world.step(missed);
                then += frame;
                if let Some(landing) = flight.landing.as_mut() {
                    landed.write_batch(crate::audio::landing_sound::hear(&mut moving.world, moving.body, landing, then, &range, |_| None));
                }
                if flight.grenade.is_none() && moving.watch.frame(&moving.world, moving.body, moving.swept, frame) {
                    stopped = true;
                    break;
                }
            }
            let (at, turn) = moving.world.pose(moving.body);
            let object = drawn(at, turn);
            let id = flight.id;
            let root = commands
                .spawn((Name::new(format!("Loose {}", flight.template)), LooseModel(id), object, if flight.since >= flight.hidden_for { Visibility::Inherited } else { Visibility::Hidden }))
                .id();
            let mut entities = Vec::with_capacity(ready.nodes.len());
            for (parent, own, on) in &ready.nodes {
                let entity = commands.spawn((*own, if *on { Visibility::Inherited } else { Visibility::Hidden })).id();
                commands.entity(parent.map_or(root, |p| entities[p])).add_child(entity);
                entities.push(entity);
            }
            let mut pieces = Vec::with_capacity(ready.drawn.len());
            for (k, (node, mesh, material)) in ready.drawn.iter().enumerate() {
                if left_out(ready.lens.get(k).copied().flatten(), false) {
                    continue;
                }
                let part = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id();
                commands.entity(entities[*node]).add_child(part);
                pieces.push((k, part));
            }
            // Its levels of detail (`lod_view`), with the world's things'
            // on: every level spawned and tagged for the picker.
            if crate::assets::lod_view::world_on() {
                let (tagged, made) = ready.spawn_levels(&mut commands, &entities, &pieces, None);
                if tagged + made > 0 {
                    debug!("levels of detail: {} on the ground with {tagged} pieces tagged and {made} coarser made", flight.template);
                }
            }
            for (node, mesh, material) in &ready.shadows {
                let part = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), crate::rendering::shadow_only::ShadowOnly(true))).id();
                commands.entity(entities[*node]).add_child(part);
            }
            // A weapon keeps glowing and shimmering on the ground as it
            // cools, by its item's heat (`ObjectsFactory` gives every weapon
            // object it makes its weapon: `InitHotObjects`).
            ready.spawn_glows(&mut commands, root, id, &pieces, false);
            // A weapon's chambered round, on its slot's object as the game
            // puts it there as the weapon's model is made (`magazine_rounds`
            // draws a magazine's).
            for (node, round, chamber) in &hung {
                if *chamber {
                    let drawn = crate::weapons::magazine_rounds::spawn_round(&mut commands, round, crate::assets::lod_view::world_on());
                    commands.entity(entities[*node]).add_child(drawn);
                }
            }
            // A magazine's points, for its rounds (`magazine_rounds`).
            if !ready.points.is_empty() {
                let points = ready.points.iter().map(|&(node, number)| (entities[node], number)).collect();
                commands.entity(root).insert(crate::weapons::magazine_rounds::MagazinePoints::new(crate::weapons::magazine_rounds::Owner::Ground(id), points));
            }
            let centre = place_of(&moving.world, moving.body);
            loose.places.insert(id, Place { centre });
            // (It has a place now: its model is a lying thing's.)
            loose.uses.stale = true;
            flight.moving = Some(moving);
            if stopped {
                done.push(index);
            }
            continue;
        }
        let moving = flight.moving.as_mut().expect("moving");
        moving.world.step(step);
        if let Some(landing) = flight.landing.as_mut() {
            landed.write_batch(crate::audio::landing_sound::hear(&mut moving.world, moving.body, landing, flight.since, &range, |_| None));
        }
        // A grenade is gone at its fuse; anything else when the game's
        // watch on loot would take its body away.
        let stop = match &flight.grenade {
            Some(grenade) => {
                let fused = flight.since >= grenade.fuse;
                if fused {
                    blown.push(flight.id);
                    let at = moving.world.pose(moving.body).0;
                    info!("a grenade's fuse ran out {:.3} s after the throw, at {at:?}", flight.since);
                    blasts.write(Blasted::thrown(&game.db, &grenade.item, at));
                }
                fused
            }
            None => moving.watch.frame(&moving.world, moving.body, moving.swept, frame),
        };
        let (at, turn) = moving.world.pose(moving.body);
        let object = drawn(at, turn);
        if loose.places.insert(flight.id, Place { centre: place_of(&moving.world, moving.body) }).is_none() {
            loose.uses.stale = true;
        }
        for (model, mut transform, mut visibility) in &mut models {
            if model.0 == flight.id {
                *transform = object;
                if flight.since >= flight.hidden_for {
                    *visibility = Visibility::Inherited;
                }
            }
        }
        if stop {
            done.push(index);
        }
    }
    loose.blown.extend(blown);
    for index in done.into_iter().rev() {
        let flight = loose.flights.remove(index);
        // (A flight over: its model is in use by what lies, or by nothing.)
        loose.uses.stale = true;
        if flight.moving.is_some() {
            info!("{} lies still {:.2} s after it set off, at {:?}", flight.template, flight.since, loose.places.get(&flight.id).map(|place| place.centre));
        }
    }
}

/// A body's centre of mass in the engine's axes.
fn place_of(world: &World, body: Body) -> Vec3 {
    let (at, turn) = world.pose(body);
    let (centre, _) = world.mass_properties(body);
    let c = trp_physics::rotate(turn, centre);
    convert::position([at[0] + c[0], at[1] + c[1], at[2] + c[2]])
}
