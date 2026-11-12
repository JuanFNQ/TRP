//! Readying systems, preparing explosions, emitting entries and impacts.

use super::*;

/// A system made ready to run and draw: its shader, pictures and figures,
/// where it is from its effect's root, and what its collisions set off.
#[allow(clippy::too_many_arguments)]
fn ready_system(
    name: &str,
    data: ParticleSystemData,
    local: Placement,
    sub_emitter: bool,
    sets_off: Vec<(usize, bool, f32)>,
    images: &mut impl crate::assets::background::AddImage,
    pictures: &mut HashMap<(String, u32, u32), Handle<Image>>,
    white: &Handle<Image>,
) -> ReadySystem {
    let m = &data.renderer.material;
    let look = Look::of_material(m);
    if !data.renderer.enabled {
        info!("{name}: {}'s renderer is off: not drawn", data.name);
    } else if look.is_none() && data.renderer.render_mode != 5 {
        info!("{name}: {} is drawn by {}, not built: left out", data.name, m.shader);
    }
    for module in &data.unbuilt {
        info!("{name}: {}'s {module} is on and not built: left out", data.name);
    }
    if sub_emitter {
        info!("{name}: {} is another system's sub-emitter: not played with the effect", data.name);
    }
    // A cube where the material has one (`_Cube`), else a white one.
    let cube = match m.texture("_Cube").filter(|t| t.faces == 6) {
        Some(t) => match pictures.get(&(t.name.clone(), t.width, t.height)) {
            Some(made) => made.clone(),
            None => {
                let made = crate::effects::particles_view::cube_picture(t, images).unwrap_or_else(|| crate::effects::particles_view::white_cube(images));
                pictures.insert((t.name.clone(), t.width, t.height), made.clone());
                made
            }
        },
        None => match pictures.get(&("<white cube>".to_owned(), 1, 1)) {
            Some(made) => made.clone(),
            None => {
                let made = crate::effects::particles_view::white_cube(images);
                pictures.insert(("<white cube>".to_owned(), 1, 1), made.clone());
                made
            }
        },
    };
    let mut picture_of = |field: &str| -> Handle<Image> {
        let Some(t) = m.texture(field) else { return white.clone() };
        pictures.entry((t.name.clone(), t.width, t.height)).or_insert_with(|| crate::effects::particles_view::picture(t, images).unwrap_or_else(|| white.clone())).clone()
    };
    let drawn_pictures = [picture_of("_MainTex"), picture_of("_NoiseTex"), picture_of("_BumpMap"), cube.clone()];
    let figures = look.map(|look| figures_of(look, m)).unwrap_or_default();
    // Its trails, by the renderer's second material.
    let trail = match (&data.trails, &data.renderer.trail_material) {
        (Some(_), Some(tm)) => match Look::of_material(tm) {
            Some(trail_look) => {
                if (tm.float("_BlendSrc"), tm.float("_BlendDst")) != (Some(5.0), Some(10.0)) && trail_look == Look::AdditiveVertLit {
                    info!("{name}: {}'s trails' blend {:?} is not built: drawn as 5, 10", data.name, (tm.float("_BlendSrc"), tm.float("_BlendDst")));
                }
                let mut trail_picture = |field: &str| -> Handle<Image> {
                    let Some(t) = tm.texture(field) else { return white.clone() };
                    pictures.entry((t.name.clone(), t.width, t.height)).or_insert_with(|| crate::effects::particles_view::picture(t, images).unwrap_or_else(|| white.clone())).clone()
                };
                let trail_pictures = [trail_picture("_MainTex"), trail_picture("_NoiseTex"), trail_picture("_BumpMap"), cube.clone()];
                Some(ReadyTrail { look: trail_look, figures: figures_of(trail_look, tm), pictures: trail_pictures, queue: tm.render_queue })
            }
            None => {
                info!("{name}: {}'s trails are drawn by {}, not built: left out", data.name, tm.shader);
                None
            }
        },
        (Some(_), None) => {
            info!("{name}: {}'s trails have no material: left out", data.name);
            None
        }
        _ => None,
    };
    ReadySystem {
        trail,
        local,
        look: look.filter(|_| data.renderer.enabled),
        pictures: drawn_pictures,
        figures,
        queue: m.render_queue,
        fudge: data.renderer.sorting_fudge,
        render_mode: data.renderer.render_mode,
        sort_mode: data.renderer.sort_mode,
        sub_emitter,
        sets_off,
        data: Arc::new(data),
    }
}

/// The effects the database's grenades and explosive rounds name, read at
/// the start.
pub(crate) fn prepare_explosions(unity: &mut trp_data::unity::Assets, db: &trp_data::Database, impact_names: &[String], images: &mut impl crate::assets::background::AddImage) -> Option<Explosions> {
    // The grenades' and the explosive rounds' (`loose::blast_effects`).
    let mut names = crate::world::loose::blast_effects(db);
    // And the bullets' hits' entries (the effects' list's entries that name
    // materials), whose older emitters are of the same kind.
    let impact_names: Vec<String> = impact_names.to_vec();
    names.extend(impact_names.iter().cloned());
    names.sort();
    names.dedup();
    let mut fast = trp_sim::base::unity_random::FastRandom::new();
    let white = images.add_image(Image::new_fill(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    ));
    let mut pictures: HashMap<(String, u32, u32), Handle<Image>> = HashMap::new();
    let mut effects = HashMap::new();
    let mut entries = HashMap::new();
    let mut shared: Vec<(ReadySystem, ParticleSystem)> = Vec::new();
    let mut shared_of: HashMap<trp_data::unity::ObjRef, usize> = HashMap::new();
    let mut adapters = Vec::new();
    let mut adapter_of: HashMap<trp_data::unity::ObjRef, usize> = HashMap::new();
    let mut rng = Rng::new(0xb1a5);
    for name in names {
        // The entry's older emitters and its light (the flashbang's).
        match trp_data::drawing::particles::effect_entry(unity, &name) {
            Ok(entry) if !entry.emitters.is_empty() || entry.light.is_some() => {
                let mut emitters = Vec::new();
                for e in entry.emitters {
                    let kind = match e.kind {
                        trp_data::drawing::particles::EmitterKind::System(a) => {
                            let shared_index = *shared_of.entry(a.system).or_insert_with(|| {
                                // The effects' object at the world's origin
                                // (`PROVISIONAL`, as the haze system's).
                                let ready = ready_system(&name, a.data.clone(), Placement::default(), false, Vec::new(), images, &mut pictures, &white);
                                let system = ParticleSystem::seeded(ready.data.clone(), (rng.unit() * u32::MAX as f32) as u64);
                                shared.push((ready, system));
                                shared.len() - 1
                            });
                            let adapter = *adapter_of.entry(a.adapter).or_insert_with(|| {
                                adapters.push(trp_sim::effects::emitters::AdapterDraws::new(a.lifetime, a.speed, a.size, &mut fast));
                                adapters.len() - 1
                            });
                            ReadyKind::System { shared: shared_index, adapter, rotation_random: a.rotation_random, rotation: a.rotation, color: a.color }
                        }
                        trp_data::drawing::particles::EmitterKind::Sparks(s) => ReadyKind::Sparks(Arc::new(s)),
                    };
                    emitters.push(ReadyEmitter { distance: e.distance, way: e.way, min_count: e.min_count, count_range: e.count_range, scale: e.random_scale, kind });
                }
                info!("the explosion {name}: {} older emitters, the pool's light {:?}", emitters.len(), entry.light);
                entries.insert(name.clone(), ReadyEntry { emitters, shift: entry.particles_shift, light: entry.light });
            }
            Ok(_) => {}
            Err(e) => info!("the explosion {name}'s entry is not read: {e:#}"),
        }
        if impact_names.contains(&name) {
            // A hit's: no mediator's effect.
            continue;
        }
        let effect: ParticleEffect = match trp_data::drawing::particles::named_effect(unity, &name) {
            Ok(effect) => effect,
            Err(e) => {
                // The flash's (`Flashbang`) is its older emitters' alone; a
                // smoke grenade's is its own.
                info!("the explosion {name} has no mediator's effect: {e:#}");
                continue;
            }
        };
        let objects: Vec<trp_data::unity::ObjRef> = effect.systems.iter().map(|s| s.system).collect();
        let mut systems = Vec::new();
        for s in effect.systems {
            // Its sub-emitters set off by a collision; the others not built.
            let mut sets_off = Vec::new();
            for e in &s.data.sub_emitters {
                match (e.kind, e.system.and_then(|o| objects.iter().position(|&x| x == o))) {
                    (1, Some(target)) => sets_off.push((target, e.inherit & 16 != 0, e.probability)),
                    (kind, _) => info!("{name}: {}'s sub-emitter of kind {kind} is not built: left out", s.data.name),
                }
            }
            let local = Placement { position: s.position, rotation: s.rotation, scale: s.scale };
            systems.push(ready_system(&name, s.data, local, s.sub_emitter, sets_off, images, &mut pictures, &white));
        }
        info!("the explosion {name}: {} systems", systems.len());
        effects.insert(name, Arc::new(ReadyEffect { systems, copies: effect.copies.max(1) }));
    }
    // The muzzles' shared blasts (a launcher's: firing spec, "A launcher's
    // blasts").
    let mut blasts = Vec::new();
    match trp_data::drawing::particles::muzzle_blasts(unity) {
        Ok(read) => {
            for (kind, effect) in read {
                let name = effect.name.clone();
                let objects: Vec<trp_data::unity::ObjRef> = effect.systems.iter().map(|s| s.system).collect();
                let systems: Vec<ReadySystem> = effect
                    .systems
                    .into_iter()
                    .map(|s| {
                        let local = Placement { position: s.position, rotation: s.rotation, scale: s.scale };
                        ready_system(&name, s.data, local, s.sub_emitter, Vec::new(), images, &mut pictures, &white)
                    })
                    .collect();
                let lights = effect
                    .lights
                    .iter()
                    .filter_map(|l| {
                        let driver = l.driver?;
                        Some(DrivenLight {
                            color: [l.color[0], l.color[1], l.color[2]],
                            range: l.range,
                            local: l.position,
                            system: objects.iter().position(|&o| o == driver.system),
                            min_max: driver.min_max,
                            time: driver.time,
                            intensity: l.intensity,
                            velocity: 0.0,
                        })
                    })
                    .collect::<Vec<_>>();
                info!("the muzzles' blast {name} ({}): {} systems, {} driven lights", if kind == 0 { "front" } else { "back" }, systems.len(), lights.len());
                let ready = Arc::new(ReadyEffect { systems, copies: 1 });
                let running = ready.systems.iter().map(|s| ParticleSystem::seeded(s.data.clone(), (rng.unit() * u32::MAX as f32) as u64)).collect();
                blasts.push(MuzzleBlast { kind, effect: ready, systems: running, lights, root: Placement::default() });
            }
        }
        Err(e) => info!("the muzzles' blasts are not drawn: {e:#}"),
    }
    let unity = trp_sim::base::unity_random::UnityRandom::new(crate::fixed_seed(0xb1a5).map(|seed| seed as i32).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as i32)));
    Some(Explosions { effects, copies: HashMap::new(), rng, engine: None, entries, shared, adapters, pool: Vec::new(), blasts, shots_seen: [0; 2], fast, unity })
}

/// An entry's older emitters played (`Effect.Emit`'s particles): each
/// within its distances from the camera, its count and ways drawn from the
/// game's tables, from `at` (the place moved along the normal by the
/// entry's shift); the particles each gave.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_entry(
    entry: &ReadyEntry,
    at: [f32; 3],
    normal: [f32; 3],
    distance: f32,
    fast: &mut trp_sim::base::unity_random::FastRandom,
    unity: &mut trp_sim::base::unity_random::UnityRandom,
    shared: &mut [(ReadySystem, ParticleSystem)],
    adapters: &mut [trp_sim::effects::emitters::AdapterDraws],
    mut sparks: Option<&mut crate::effects::particles_view::Sparks>,
    now: f32,
) -> usize {
    let at = [at[0] + normal[0] * entry.shift, at[1] + normal[1] * entry.shift, at[2] + normal[2] * entry.shift];
    let mut emitted = 0;
    for e in entry.emitters.iter().filter(|e| trp_sim::combat::impacts::within(distance, e.distance)) {
        for _ in 0..trp_sim::effects::emitters::count(e.min_count, e.count_range, fast) {
            let (way, lift) = trp_sim::effects::emitters::way(e.way, normal, fast);
            let way = trp_sim::effects::emitters::scaled(way, e.scale);
            let place = [at[0], at[1] + lift, at[2]];
            match &e.kind {
                ReadyKind::System { shared: index, adapter, rotation_random, rotation, color } => {
                    let params = adapters[*adapter].emit(place, way, *rotation_random, *rotation, *color);
                    emitted += usize::from(shared[*index].1.emit(params));
                }
                ReadyKind::Sparks(adapter) => {
                    if let Some(sparks) = sparks.as_mut() {
                        sparks.emit(trp_sim::effects::emitters::spark(adapter, place, way, now, unity));
                        emitted += 1;
                    }
                }
            }
        }
    }
    emitted
}

/// The frame's bullets' hits played (`Effects.Update` playing `Effect.Emit`
/// for each queued, ten a frame: `bullets`): where seen, the material's
/// entry's older emitters along the normal and its light 5 cm out within
/// its distance. The decal is `decal_view`'s, the sound `impact_sound`'s.
pub fn impacts(
    bullets: Res<crate::weapons::bullets::Bullets>,
    game: Res<crate::GameData>,
    time: Res<Time>,
    mut explosions: ResMut<Explosions>,
    eyes: Query<&GlobalTransform, With<crate::MainCamera>>,
    mut sparks: Option<ResMut<crate::effects::particles_view::Sparks>>,
) {
    if bullets.effects.is_empty() {
        return;
    }
    let deferred_on = game.hard.float("DEFERRED_DECALS_ENABLED").is_ok_and(|on| on != 0.0);
    let Explosions { entries, shared, adapters, pool, fast, unity, .. } = &mut *explosions;
    let eye = eyes.iter().next().map_or(Vec3::ZERO, |e| e.translation());
    let eye = [eye.x, eye.y, -eye.z];
    for (impact, entry) in &bullets.effects {
        // The older decal system's decal is laid first, seen or not
        // (`Effect.Emit`): its draws taken (`emitters::old_decal`).
        // (A blade's hit lays none.)
        if let Some(draws) = bullets.entries.get(*entry).filter(|_| !impact.knife).and_then(|e| e.old_decal_laid(deferred_on)) {
            trp_sim::effects::emitters::old_decal(draws, fast);
        }
        if !impact.seen {
            continue;
        }
        let Some(name) = bullets.entries.get(*entry).map(|e| e.name.as_str()) else { continue };
        let Some(ready) = entries.get(name) else { continue };
        let p = impact.position;
        let distance = ((p[0] - eye[0]).powi(2) + (p[1] - eye[1]).powi(2) + (p[2] - eye[2]).powi(2)).sqrt();
        let emitted = emit_entry(ready, p, impact.normal, distance, fast, unity, shared, adapters, sparks.as_deref_mut(), time.elapsed_secs());
        if let Some(light) = ready.light.filter(|l| distance < l.max_distance) {
            let n = impact.normal;
            pool.push(trp_sim::effects::emitters::PoolLight {
                position: [p[0] + n[0] * 0.05, p[1] + n[1] * 0.05, p[2] + n[2] * 0.05],
                color: light.color,
                range: light.range,
                intensity: light.intensity,
                time: light.time,
                shadows: light.shadows,
                age: 0.0,
            });
        }
        debug!("a hit on {name}: {emitted} particles {distance:.1} m off");
    }
}
