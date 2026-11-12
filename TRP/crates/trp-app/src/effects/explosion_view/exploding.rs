//! Exploding, placing the lights and composing each frame's explosions.

use super::*;

/// Each grenade's blast plays its effect where the grenade was, its z axis
/// up (`EmitGrenade(..., position, Vector3.up)`, the mediator turning it to
/// `LookRotation(normal)`): the mediator's next copy moved there and played
/// with all its systems. A system of that copy still playing is only moved:
/// `Play` does nothing to a system already playing (the 2022.3 scripting
/// reference; the engine's `Play` resets a system only when stopped).
/// `PROVISIONAL`: one with particles alive after its emission ended taken as
/// still playing. A system another sets off (a sub-emitter) is not played
/// with the rest: the engine marks it so that it never emits on its own
/// (`ParticleSystem::MarkAllSubEmitters`, `IsEmitting`; grenades spec).
#[allow(clippy::too_many_arguments)]
pub fn explode(
    mut blasts: MessageReader<crate::world::loose::Blasted>,
    frame: Res<bevy::diagnostic::FrameCount>,
    time: Res<Time>,
    mut explosions: ResMut<Explosions>,
    range: Res<crate::world::range::Range>,
    loose: Option<Res<crate::world::loose::Loose>>,
    eyes: Query<&GlobalTransform, With<crate::MainCamera>>,
    mut sparks: Option<ResMut<crate::effects::particles_view::Sparks>>,
) {
    let Explosions { effects, copies, rng, engine, entries, shared, adapters, pool, fast, unity, .. } = &mut *explosions;
    for blasted in blasts.read() {
        // Its entry's older emitters, each within its distances from the
        // camera, and its light (`Effect.Emit`; along its normal: a thrown
        // grenade's up, a round's hit's surface's).
        let Some((name, _)) = blasted.effect.clone() else { continue };
        let normal = blasted.normal;
        if let Some(entry) = entries.get(&name) {
            let eye = eyes.iter().next().map_or(Vec3::ZERO, |e| e.translation());
            let distance = (Vec3::new(eye.x, eye.y, -eye.z) - Vec3::from(blasted.at)).length();
            let emitted = emit_entry(entry, blasted.at, normal, distance, fast, unity, shared, adapters, sparks.as_deref_mut(), time.elapsed_secs());
            if let Some(light) = entry.light.filter(|l| distance < l.max_distance) {
                pool.push(trp_sim::effects::emitters::PoolLight {
                    position: blasted.sound_point(),
                    color: light.color,
                    range: light.range,
                    intensity: light.intensity,
                    time: light.time,
                    shadows: light.shadows,
                    age: 0.0,
                });
            }
            info!("the explosion {name}'s older emitters: {emitted} particles {distance:.1} m off, the pool's light {}", entry.light.is_some_and(|l| distance < l.max_distance));
        }
        // The range in the game's engine, once: its ground and boxes, as
        // the things thrown meet it (stepped once, for its queries).
        if engine.is_none() {
            if let Some(physics) = loose.as_ref().and_then(|l| l.physics()) {
                let mut world = crate::world::loose::range_world(physics, &range);
                world.step(1.0 / 60.0);
                *engine = Some(world);
            }
        }
        let Some(effect) = effects.get(&name) else { continue };
        let copies = copies.entry(name.clone()).or_insert_with(|| Copies { effect: effect.clone(), shown: (0..effect.copies).map(|_| None).collect(), next: 0 });
        let index = copies.next;
        copies.next = (index + 1) % copies.shown.len();
        // Its z along the normal (`LookRotation(normal)`): up a quarter turn
        // about x, taking z to y.
        let root = Placement { position: blasted.at, rotation: trp_sim::recordings::particle_trace::look_rotation(normal), scale: [1.0; 3] };
        let systems = copies.shown[index].get_or_insert_with(|| {
            copies.effect.systems.iter().map(|s| ParticleSystem::seeded(s.data.clone(), (rng.unit() * u32::MAX as f32) as u64)).collect()
        });
        for (system, ready) in systems.iter_mut().zip(&copies.effect.systems) {
            let placement = compose(root, ready.local);
            if ready.sub_emitter {
                system.placement = placement;
            } else if system.alive() {
                system.placement = placement;
            } else {
                system.play(placement);
            }
        }
        info!("the explosion {name} (copy {}) at {:?} on frame {}", index + 1, blasted.at, frame.0);
    }
}

/// The time of day's harmonics, packed, with `CUSTOMS LIGHT`; none
/// without it (`ESTIMATE`: the flat ambient's constant alone).
fn effect_harmonics(level: Option<&crate::rendering::lighting::LevelLighting>) -> [Vec4; 7] {
    match level {
        Some(l) => {
            let p = l.harmonics.packed();
            [p.a[0], p.a[1], p.a[2], p.b[0], p.b[1], p.b[2], p.c].map(Vec4::from_array)
        }
        None => {
            let mut sh = [Vec4::ZERO; 7];
            for (i, row) in sh.iter_mut().take(3).enumerate() {
                row[3] = [AMBIENT; 3][i];
            }
            sh
        }
    }
}

/// The lights module's lights this frame as Bevy's point lights, from a
/// pool kept for them (more made as needed, the rest hidden). Each as the
/// hall's lamps are given (`lighting::hall_lamp`): its colour the linear of
/// the game's, and its power such that at a fifth of its range (where the
/// engine's fall-off has halved it) a surface square to it takes what the
/// game's would. `PROVISIONAL`: elsewhere Bevy's fall-off with the square of
/// the distance; the effects' own particles are not lit by them (the
/// game's G-buffer things are: the concrete). With the game's shading
/// (`shading`), each as the game gives it: its colour, its intensity
/// (times 4π, which Bevy divides by) and its range, the engine's fall-off.
fn place_lights(commands: &mut Commands, pool: &mut Query<(&mut PointLight, &mut Transform, &mut Visibility), With<ExplosionLight>>, lights: &[FrameLight]) {
    let lamp = |l: &FrameLight| {
        let reach = 0.2 * l.range;
        let at = l.intensity / (1.0 + 25.0 * 0.2 * 0.2);
        let pi = std::f32::consts::PI;
        let (color, intensity) = if crate::rendering::shading::on() {
            (Color::linear_rgb(l.color[0], l.color[1], l.color[2]), 4.0 * pi * l.intensity)
        } else {
            (
                Color::linear_rgb(crate::rendering::lighting::linear(l.color[0]), crate::rendering::lighting::linear(l.color[1]), crate::rendering::lighting::linear(l.color[2])),
                4.0 * pi * pi * crate::rendering::lighting::EXPOSURE_SCALE * reach * reach * crate::rendering::lighting::linear(at),
            )
        };
        PointLight {
            color,
            intensity,
            range: l.range,
            radius: 0.0,
            // The pool's light with shadows where its entry asks (hard, the
            // game's: `PROVISIONAL`, Bevy's own soft filtering).
            shadows_enabled: l.shadows,
            ..default()
        }
    };
    let mut wanted = lights.iter();
    for (mut light, mut transform, mut visibility) in pool.iter_mut() {
        match wanted.next() {
            Some(l) => {
                *light = lamp(l);
                transform.translation = crate::platform::convert::position(l.position);
                *visibility = Visibility::Inherited;
            }
            None => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
    for l in wanted {
        commands.spawn((lamp(l), Transform::from_translation(crate::platform::convert::position(l.position)), Visibility::Inherited, ExplosionLight));
    }
}

pub(super) fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A placement under another.
fn compose(parent: Placement, local: Placement) -> Placement {
    let q = Quat::from_array(parent.rotation);
    let at = q * (Vec3::from(local.position) * Vec3::from(parent.scale));
    Placement {
        position: (Vec3::from(parent.position) + at).to_array(),
        rotation: (q * Quat::from_array(local.rotation)).to_array(),
        scale: (Vec3::from(parent.scale) * Vec3::from(local.scale)).to_array(),
    }
}

/// A frame, once everything is placed: each copy's systems that still play
/// stepped once, and each one's corners handed to every camera that draws
/// them, made for that camera in the engine's order (`camera_draws`).
#[allow(clippy::too_many_arguments)]
pub fn explosion_frame(
    mut commands: Commands,
    time: Res<Time>,
    mut explosions: ResMut<Explosions>,
    suns: Query<&GlobalTransform, With<crate::rendering::lighting::RangeSun>>,
    level: Option<Res<crate::rendering::lighting::LevelLighting>>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut ExplosionScreen)>,
    mut pool: Query<(&mut PointLight, &mut Transform, &mut Visibility), With<ExplosionLight>>,
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    controller: Res<crate::player::controller::Controller>,
    places: Query<&GlobalTransform>,
    lamps: Query<&GlobalTransform, With<SpotLight>>,
    game: Option<Res<crate::GameData>>,
) {
    let dt = time.delta_secs();
    // A frame longer than the particles' longest step is made in equal
    // steps (`Time.maximumParticleDeltaTime`; particles spec, "A long frame").
    let most = game.as_ref().map_or(f32::MAX, |g| g.engine.maximum_particle_timestep);
    let mut shown = false;
    let mut lights = Vec::new();
    let Explosions { copies, rng, engine, shared, pool: pool_lights, blasts, shots_seen, .. } = &mut *explosions;
    let world = engine.as_ref().map(Engine);
    // A shot of the weapon in hand plays, at each of its particle pivots,
    // the shared blast of that pivot's kind: moved there, its systems
    // stopped and played again (`IMuzzleParticlePivot.Play`).
    // The weapon's muzzle's and its launcher's, each by its own shots.
    for (parts, shots) in crate::weapons::muzzle_view::fired_muzzles(rig.as_deref(), &controller, shots_seen) {
        if shots > 0 {
            for &(entity, kind) in &parts.pivots {
                let Some(place) = entity.and_then(|e| places.get(e).ok()) else { continue };
                let Some(blast) = blasts.iter_mut().find(|b| b.kind == kind) else { continue };
                let (_, turn, at) = place.to_scale_rotation_translation();
                blast.root = Placement { position: [at.x, at.y, -at.z], rotation: crate::platform::convert::rotation_back(turn), scale: [1.0; 3] };
                for (system, ready) in blast.systems.iter_mut().zip(&blast.effect.systems) {
                    system.play(compose(blast.root, ready.local));
                }
                info!("the muzzles' blast of kind {kind} played at {:?}", blast.root.position);
            }
        }
    }
    // The frame's rays shared among every system before any steps: the
    // blasts', the shared, the effects' copies (`PROVISIONAL`: in that
    // order, the copies by their effect's name and number).
    {
        let mut names: Vec<&String> = copies.keys().collect();
        names.sort();
        let names: Vec<String> = names.into_iter().cloned().collect();
        let mut all: Vec<&mut ParticleSystem> = blasts.iter_mut().flat_map(|b| b.systems.iter_mut()).chain(shared.iter_mut().map(|(_, s)| s)).collect();
        let mut by_name: HashMap<&String, &mut Copies> = copies.iter_mut().collect();
        for name in &names {
            if let Some(copy) = by_name.remove(name) {
                all.extend(copy.shown.iter_mut().flatten().flat_map(|systems| systems.iter_mut()));
            }
        }
        trp_sim::effects::particles::assign_ray_budgets(&mut all, PARTICLE_RAYS);
    }
    // Each blast's systems stepped; its driven lights eased toward their
    // system's playing (`MuzzleParticleLight`), lit while above nought.
    for blast in blasts.iter_mut() {
        for system in blast.systems.iter_mut().filter(|s| s.alive()) {
            system.step_frame(dt, most, world.as_ref().map(|w| w as &dyn trp_sim::effects::particles::World));
            shown |= !system.particles.is_empty();
        }
        for light in &mut blast.lights {
            let playing = light.system.and_then(|i| blast.systems.get(i)).is_some_and(ParticleSystem::alive);
            let target = if playing { light.min_max[1] } else { light.min_max[0] };
            light.intensity = trp_sim::effects::particles::smooth_damp(light.intensity, target, &mut light.velocity, dt * light.time, dt);
            if (light.intensity - target).abs() < 0.01 {
                light.intensity = target;
            }
            if light.intensity > 0.0 {
                let at = compose(blast.root, Placement { position: light.local, rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] }).position;
                lights.push(FrameLight { position: at, color: light.color, range: light.range, intensity: light.intensity, shadows: false });
            }
        }
        for system in blast.systems.iter() {
            lights.extend(system.lights().into_iter().map(|l| FrameLight { position: l.position, color: l.color, range: l.range, intensity: l.intensity, shadows: false }));
        }
    }
    // The shared systems the older emitters emit into, each stepped in the
    // world while it has particles; the pool's lights, each falling.
    for (_, system) in shared.iter_mut().filter(|(_, s)| s.alive()) {
        system.step_frame(dt, most, world.as_ref().map(|w| w as &dyn trp_sim::effects::particles::World));
        shown |= !system.particles.is_empty();
    }
    for light in pool_lights.iter_mut() {
        light.step(dt);
    }
    pool_lights.retain(|l| l.now().is_some());
    for l in pool_lights.iter() {
        if let Some((range, intensity)) = l.now() {
            lights.push(FrameLight { position: l.position, color: [l.color[0], l.color[1], l.color[2]], range, intensity, shadows: l.shadows });
        }
    }
    for copy in copies.values_mut() {
        let effect = copy.effect.clone();
        for systems in copy.shown.iter_mut().flatten() {
            // Each system stepped in the world; what its particles' meetings
            // set off born after. `PROVISIONAL`: a long frame's (`step_frame`)
            // born after its last step, the engine's after each.
            let mut set_off: Vec<(usize, [f32; 3], f32)> = Vec::new();
            for (system, ready) in systems.iter_mut().zip(&effect.systems) {
                if !system.alive() {
                    continue;
                }
                let met = system.step_frame(dt, most, world.as_ref().map(|w| w as &dyn trp_sim::effects::particles::World));
                for p in &met {
                    for &(target, takes_duration, chance) in &ready.sets_off {
                        if rng.unit() <= chance {
                            set_off.push((target, system.world_place(p), if takes_duration { system.cycle() } else { 0.0 }));
                        }
                    }
                }
                shown |= !system.particles.is_empty();
            }
            for (target, at, cycle) in set_off {
                if let Some(system) = systems.get_mut(target) {
                    system.emit_at(at, cycle);
                }
            }
            for system in systems.iter() {
                lights.extend(system.lights().into_iter().map(|l| FrameLight { position: l.position, color: l.color, range: l.range, intensity: l.intensity, shadows: false }));
            }
        }
    }
    place_lights(&mut commands, &mut pool, &lights);
    if !shown {
        for (_, _, _, mut screen) in &mut cameras {
            if !screen.0.is_empty() {
                screen.0.clear();
            }
        }
        return;
    }
    // The way to the app's sun.
    let sun = suns.iter().next().map_or(Vec3::Y, |sun| sun.back().as_vec3());
    let light = effect_light(level.as_deref());
    let harmonics = effect_harmonics(level.as_deref());
    // The engine's vertex lights of the frame (for the vertex-lit shaders):
    // the sun, the effects' lights, the hall's lamps.
    let game = crate::platform::convert::position;
    let mut frame_lights: Vec<VertexLight> = Vec::new();
    if let Some(sun_way) = suns.iter().next().map(|s| s.back().as_vec3()) {
        frame_lights.push(VertexLight { position: sun_way.extend(0.0), color: Vec4::new(light[0].x, light[0].y, light[0].z, 1.0), atten: Vec4::new(-1.0, 1.0, 0.0, 1.0), spot: Vec4::ZERO });
    }
    for l in &lights {
        frame_lights.push(VertexLight::point(game(l.position), l.color, l.intensity, l.range));
    }
    for lamp in lamps.iter() {
        let (color, intensity, range, cone) = crate::rendering::lighting::hall_lamp_figures();
        let (cos_half, cos_quarter) = ((cone * 0.5).to_radians().cos(), (cone * 0.25).to_radians().cos());
        let mut l = VertexLight::point(lamp.translation(), color, intensity, range);
        l.atten.x = cos_half;
        l.atten.y = 1.0 / (cos_quarter - cos_half).max(1e-3);
        l.spot = (-lamp.forward().as_vec3()).extend(0.0);
        frame_lights.push(l);
    }
    let frame = FrameFigures { light, harmonics, smoke_globals: crate::rendering::lighting::smoke_globals(level.as_deref()), frame_lights, sun, time: time.elapsed_secs() };
    for (camera, eye, projection, mut screen) in &mut cameras {
        if !camera.is_active {
            if !screen.0.is_empty() {
                screen.0.clear();
            }
            continue;
        }
        screen.0 = camera_draws(&explosions, &frame, camera, eye, crate::weapons::muzzle_view::near_of(projection));
    }
}
