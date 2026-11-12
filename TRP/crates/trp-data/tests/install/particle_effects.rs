//! Tests that read muzzle flashes, blasts, explosions, sparks, tracers and fumes.

use super::*;

/// The AKM's muzzle flash (firing spec, "The muzzle flash"): its manager's
/// figures, its material's, and its jets on the muzzle device.
#[test]
fn the_akm_s_muzzle_flash_is_read() {
    let Some((_assets, weapon)) = akm() else { return };
    let muzzle = weapon.muzzle.as_ref().expect("the AKM's muzzle flash");
    let m = &muzzle.material;
    println!(
        "{} s, atlas {:?}, under {:?}; material tint {:?}, window {}, phase {}, HDR {}, z {}, picture {} {}x{} format {}; jets {:?}",
        muzzle.shot_length,
        muzzle.atlas,
        weapon.hierarchy.nodes[muzzle.mesh_parent].name,
        m.tint,
        m.inv_window,
        m.randomize_phase,
        m.hdr,
        m.z_shift,
        m.texture.name,
        m.texture.width,
        m.texture.height,
        m.texture.format,
        muzzle.jets.iter().map(|jet| (weapon.hierarchy.path(jet.node), jet.particles.len(), jet.bounds, jet.chance)).collect::<Vec<_>>()
    );
    let level = trp_data::unity::texture::mips(&m.texture).expect("its level")[0];
    let pixels = trp_data::unity::texture::decode_rgba(&m.texture, level).expect("decoded");
    let mut stats = [(255u8, 0u8, 0u64); 4];
    for px in pixels.chunks_exact(4) {
        for c in 0..4 {
            stats[c] = (stats[c].0.min(px[c]), stats[c].1.max(px[c]), stats[c].2 + px[c] as u64);
        }
    }
    let n = (pixels.len() / 4) as u64;
    println!("the jets' picture (least, most, mean): {:?}, sRGB {}; its first quarter's middle {:?}", stats.map(|(a, b, s)| (a, b, s / n)), m.texture.srgb, &pixels[(256 * 1024 + 256) * 4..][..4]);
    for smoke in &muzzle.smokes {
        println!("smoke on {}: end {}, brake {}, drag {}, gravity {}, length {} + {}, by shot {}, drop {}; material width {}, uv {}, diffusion {}, fades {} {} {}, picture {} {}x{}", weapon.hierarchy.nodes[smoke.node].name, smoke.smoke_end, smoke.brake_distance, smoke.drag, smoke.gravity, smoke.smoke_length, smoke.smoke_length_randomness, smoke.increasing_by_shot, smoke.drop_time, smoke.material.width, smoke.material.uv_y_scale, smoke.material.diffusion_strength, smoke.material.start_fade, smoke.material.end, smoke.material.end_fade, smoke.material.texture.name, smoke.material.texture.width, smoke.material.texture.height);
    }
    assert_eq!(muzzle.smokes.len(), 1);
    println!("the jets' material's render queue: {}", m.render_queue);
    assert_eq!((muzzle.shot_length, muzzle.atlas), (0.12, [2, 2]));
    assert_eq!(weapon.hierarchy.nodes[muzzle.mesh_parent].name, "weapon");
    assert!(!muzzle.jets.is_empty() && muzzle.jets.iter().all(|jet| !jet.particles.is_empty()));
    // Its fumes: three on its muzzle device and the one of its ejection
    // port (`extractor_smoke`); the first one puff a shot of 0.4 at 5 for
    // 0.2 s (firing spec, "The muzzle's fumes").
    for fume in &muzzle.fumes {
        println!("fume on {}: start {}, radius {}, cone {}, count {} + {}, size {} ± {}, speed {}, life {}", weapon.hierarchy.path(fume.node), fume.start_pos, fume.emitter_radius, fume.conus_size, fume.count_min, fume.count_range, fume.size, fume.sizes_rnd, fume.speed, fume.lifetime);
    }
    let names: Vec<&str> = muzzle.fumes.iter().map(|fume| weapon.hierarchy.nodes[fume.node].name.as_str()).collect();
    assert_eq!(names.iter().filter(|name| name.starts_with("muzzleflash_")).count(), 3, "{names:?}");
    assert!(names.contains(&"extractor_smoke"), "{names:?}");
    let first = &muzzle.fumes[0];
    assert_eq!((first.count_min, first.count_range, first.size, first.speed, first.lifetime), (1, 1, 0.4, 5.0, 0.2));
    assert_eq!(first.color.alphas.len(), 3);
    // Its sparks: on `muzzleflash_000` and the port's, their tables of 128
    // kept with them (firing spec, "The muzzle's sparks").
    for s in &muzzle.sparks {
        println!("sparks on {}: count {} + {}, radius {}, cone {}, shift {}, life {:?}.., speed {:?}.., size {:?}..", weapon.hierarchy.nodes[s.node].name, s.count_min, s.count_range, s.emitter_radius, s.conus_size, s.emission_time_shift, &s.lifetime[..3], &s.speed[..3], &s.size[..3]);
    }
    assert_eq!(muzzle.sparks.len(), 2);
    assert!(muzzle.sparks.iter().all(|s| [s.lifetime.len(), s.speed.len(), s.gravity.len(), s.drag.len(), s.emission.len(), s.size.len(), s.turbulence.len(), s.frequency.len()] == [128; 8]));
    assert_eq!((muzzle.sparks[0].count_min, muzzle.sparks[0].count_range), (-2, 5));
}

/// The muzzles' heat haze: the shared system and its material, the AKM's
/// two emitters, and the camera's distortion it is drawn into (firing
/// spec, "The muzzle's heat haze").
#[test]
fn the_muzzles_heat_haze_is_read() {
    let Some((_assets, weapon)) = akm() else { return };
    let install = Install::locate().expect("install");
    let mut assets = Assets::new(install.clone()).expect("manifest");
    let haze = trp_data::drawing::particles::muzzle_heat_haze_system(&mut assets).expect("the haze system");
    let m = &haze.renderer.material;
    println!(
        "{}: world {}, most {}, size {:?}, colour {:?}; most size {}; material {} with {} ({:?}; {:?}; {:?}; {:?})",
        haze.name,
        haze.world_space,
        haze.max_particles,
        haze.size_over_lifetime.as_ref().map(|c| (c.mode, c.max_curve.keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>())),
        haze.color_over_lifetime.as_ref().map(|g| (g.mode, &g.max_gradient.colors, &g.max_gradient.alphas)),
        haze.renderer.max_size,
        m.name,
        m.shader,
        m.textures.iter().map(|(n, t)| (n, &t.name, t.width, t.height, t.format, t.clamp)).collect::<Vec<_>>(),
        m.floats,
        m.colors,
        m.scales
    );
    assert_eq!((haze.name.as_str(), haze.world_space, haze.max_particles), ("HeatHazeEffect", false, 12));
    assert!(haze.unbuilt.is_empty(), "{:?}", haze.unbuilt);
    assert_eq!(m.shader, "Particles/DistortAndBlur");
    assert!(m.texture("_DistortTex").is_some() && m.texture("_AlphaTex").is_some() && m.scale("_DistortTex").is_some());
    assert_eq!((m.float("_InvFade"), m.color("_TextureMovement")), (Some(88.0), Some([0.4, 0.4, -0.4, -0.4])));
    let muzzle = weapon.muzzle.as_ref().expect("the AKM's muzzle");
    for h in &muzzle.hazes {
        println!("haze on {}: {:?}", weapon.hierarchy.nodes[h.node].name, h);
    }
    assert_eq!(muzzle.hazes.len(), 2);
    // One with every shot, one with a quarter of them.
    let mut chances: Vec<f32> = muzzle.hazes.iter().map(|h| h.chance).collect();
    chances.sort_by(f32::total_cmp);
    assert_eq!(chances, vec![0.25, 1.0]);
    assert!(muzzle.hazes.iter().all(|h| (h.count, h.delay) == (3, 0.15)));
    let d = trp_data::drawing::camera_effects::read_distortion(&install).expect("the camera's distortion");
    println!("the camera's distortion: {d:?}");
    assert_eq!((d.pixels, d.blur_iterations), (8.0, 1));
}

#[test]
fn a_grenade_s_explosion_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let effect = trp_data::drawing::particles::named_effect(&mut assets, "Grenade_new").expect("the explosion");
    let c = |m: &trp_data::drawing::particles::MinMaxCurve| match m.mode {
        0 => format!("{}", m.scalar),
        3 => format!("{}..{}", m.min_scalar, m.scalar),
        mode => format!("m{mode}×{}", m.scalar),
    };
    for s in &effect.systems {
        let d = &s.data;
        let r = &d.renderer;
        println!(
            "{}{} at {:?} turn {:?}: {} s{} delay {}, life {}, speed {}, size {}, turn {}, gravity {}, most {}, {}; shape {:?}; emission {:?}; sheet {:?}; force {}; size/life {}, colour/life {}, limit {}, turn/speed {}; unbuilt {:?}; drawn {} (v {}, l {}, fudge {}) with {} {:?} {:?}",
            "  ".repeat(s.depth),
            d.name,
            s.position,
            s.rotation,
            d.duration,
            if d.looping { " looping" } else { "" },
            c(&d.start_delay),
            c(&d.start_lifetime),
            c(&d.start_speed),
            c(&d.start_size),
            c(&d.start_rotation),
            c(&d.gravity_modifier),
            d.max_particles,
            if d.world_space { "world" } else { "local" },
            d.shape.as_ref().map(|s| (s.kind, s.radius, s.radius_thickness, s.angle, s.arc, s.rotation, s.random_direction, s.spherical_direction)),
            d.emission.as_ref().map(|e| (c(&e.rate_over_time), e.bursts.iter().map(|b| (b.time, c(&b.count), b.cycles, b.interval, b.probability)).collect::<Vec<_>>())),
            d.texture_sheet.as_ref().map(|t| (t.tiles, t.single_row, t.random_row, c(&t.frame_over_time), c(&t.start_frame), t.cycles)),
            d.force.as_ref().map_or("-".into(), |f| format!("{} {} {} {}", c(&f.x), c(&f.y), c(&f.z), if f.world_space { "world" } else { "local" })),
            d.size_over_lifetime.as_ref().map_or("-".into(), c),
            d.color_over_lifetime.as_ref().map_or("-".into(), |g| format!("m{}", g.mode)),
            d.limit_velocity.as_ref().map_or("-".into(), |l| format!("{} d{} drag {}", c(&l.magnitude), l.dampen, c(&l.drag))),
            d.rotation_by_speed.as_ref().map_or("-".into(), |t| format!("{} {:?}", c(&t.curve), t.range)),
            d.unbuilt,
            r.render_mode,
            r.velocity_scale,
            r.length_scale,
            r.sorting_fudge,
            r.material.shader,
            r.material.floats,
            r.material.textures.iter().map(|(n, t)| (n.as_str(), t.name.as_str(), t.width)).collect::<Vec<_>>()
        );
        println!("    queue {}, keywords {:?}, sort {}", r.material.render_queue, r.material.keywords, r.sort_mode);
    }
    for l in &effect.lights {
        println!("light {} at {:?}: kind {}, colour {:?}, intensity {}, range {}, on {}", l.name, l.position, l.kind, l.color, l.intensity, l.range, l.enabled);
    }
    assert_eq!(effect.systems.len(), 17);
    // Its mediator's five copies, played in turn.
    assert_eq!(effect.copies, 5);
    // The dust a piece of concrete raises where it lands: set off by it, not
    // played with the rest.
    assert_eq!(effect.systems.iter().filter(|s| s.sub_emitter).map(|s| s.data.name.as_str()).collect::<Vec<_>>(), ["drop_dust"]);
    assert!(effect.systems.iter().all(|s| s.data.emission.as_ref().is_some_and(|e| !e.bursts.is_empty())));
    // The shock wave's mesh: Unity's own sphere, read from the install's
    // copy of the engine's resources (515 corners, half a metre out).
    let wave = effect.systems.iter().find(|s| s.data.name == "DistortionWave").expect("the shock wave");
    let mesh = wave.data.renderer.mesh.as_ref().expect("the shock wave's mesh");
    let farthest = mesh.positions.iter().map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()).fold(0.0f32, f32::max);
    println!("the shock wave's mesh {}: {} corners, {} triangles, {} normals, out to {farthest}", mesh.name, mesh.positions.len(), mesh.submeshes.iter().map(Vec::len).sum::<usize>() / 3, mesh.normals.len());
    assert_eq!((mesh.name.as_str(), mesh.positions.len(), mesh.normals.len()), ("Sphere", 515, 515));
    assert!((farthest - 0.5).abs() < 1e-3, "{farthest}");
}

/// The muzzles' shared blasts (the RShG-2's): the front's and the back's,
/// each a tree of systems; the back's light driven by its fire; the front's
/// fragments with trails and a trail material.
#[test]
fn the_muzzle_blasts_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let blasts = trp_data::drawing::particles::muzzle_blasts(&mut assets).expect("the muzzles' blasts");
    for (pivot, effect) in &blasts {
        println!("pivot {pivot}: {}", effect.name);
        for s in &effect.systems {
            let d = &s.data;
            let r = &d.renderer;
            println!(
                "  {}{} at {:?}: {} s delay {:?} life {:?} speed {:?} size {:?}, most {}, {}; emission {:?}; trails {:?}; unbuilt {:?}; drawn {} {} (fudge {}, sort {}) by {} queue {} {:?} {:?}; trail by {:?}",
                "  ".repeat(s.depth),
                d.name,
                s.position,
                d.duration,
                (d.start_delay.min_scalar, d.start_delay.scalar),
                (d.start_lifetime.min_scalar, d.start_lifetime.scalar),
                (d.start_speed.min_scalar, d.start_speed.scalar),
                (d.start_size.min_scalar, d.start_size.scalar),
                d.max_particles,
                if d.world_space { "world" } else { "local" },
                d.emission.as_ref().map(|e| (e.rate_over_time.scalar, e.bursts.iter().map(|b| (b.time, b.count.min_scalar, b.count.scalar)).collect::<Vec<_>>())),
                d.trails.as_ref().map(|t| (t.mode, t.ratio, t.lifetime.scalar, t.min_vertex_distance, t.texture_mode, t.world_space, t.size_affects_width, t.inherit_particle_color, t.width_over_trail.scalar)),
                d.unbuilt,
                r.render_mode,
                r.enabled,
                r.sorting_fudge,
                r.sort_mode,
                r.material.shader,
                r.material.render_queue,
                r.material.floats,
                r.material.textures.iter().map(|(n, t)| (n.as_str(), t.name.as_str(), t.width)).collect::<Vec<_>>(),
                r.trail_material.as_ref().map(|m| (m.shader.clone(), m.floats.clone(), m.colors.clone()))
            );
        }
        for l in &effect.lights {
            println!("  light {} at {:?}: kind {}, colour {:?}, intensity {}, range {}, on {}, driven {:?}", l.name, l.position, l.kind, l.color, l.intensity, l.range, l.enabled, l.driver);
        }
    }
    assert_eq!(blasts.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [0, 1]);
    let back = &blasts[1].1;
    assert!(back.lights.iter().any(|l| l.driver.is_some_and(|d| d.min_max == [0.0, 4.0] && d.time == 5.0)));
    let front = &blasts[0].1;
    assert!(front.systems.iter().any(|s| s.data.trails.is_some() && s.data.renderer.trail_material.is_some()));
    // The roots draw nothing.
    assert!(blasts.iter().all(|(_, e)| !e.systems[0].data.renderer.enabled));
}

/// The flashbang's effect: no mediator, seven older emitters (six onto
/// particle systems, one onto the shared sparks) and the pool's light.
#[test]
fn a_flashbang_s_effect_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let entry = trp_data::drawing::particles::effect_entry(&mut assets, "Flashbang").expect("the flashbang's entry");
    for e in &entry.emitters {
        match &e.kind {
            trp_data::drawing::particles::EmitterKind::System(a) => {
                let d = &a.data;
                let m = &d.renderer.material;
                println!(
                    "{}: within {:?} m, way {}, {} + <{}, scale {:?}; life {:?}, speed {:?}, size {:?}, turn {} {}, colour {:?}; {} s, most {}, {}; size/life {}, colour/life {}, limit {}, collision {}; unbuilt {:?}; drawn {} (fudge {}, sort {}) with {} queue {} {:?} {:?} {:?}",
                    d.name,
                    e.distance,
                    e.way,
                    e.min_count,
                    e.count_range,
                    e.random_scale,
                    a.lifetime,
                    a.speed,
                    a.size,
                    a.rotation_random,
                    a.rotation,
                    a.color,
                    d.duration,
                    d.max_particles,
                    if d.world_space { "world" } else { "local" },
                    d.size_over_lifetime.is_some(),
                    d.color_over_lifetime.is_some(),
                    d.limit_velocity.is_some(),
                    d.collision.is_some(),
                    d.unbuilt,
                    d.renderer.render_mode,
                    d.renderer.sorting_fudge,
                    d.renderer.sort_mode,
                    m.shader,
                    m.render_queue,
                    m.floats,
                    m.colors,
                    m.textures.iter().map(|(n, t)| (n.as_str(), t.name.as_str(), t.width)).collect::<Vec<_>>()
                );
            }
            trp_data::drawing::particles::EmitterKind::Sparks(s) => {
                println!("sparks: within {:?} m, way {}, {} + <{}, scale {:?}; {} places, lifetime {}..{}, speed {}..{}, time shift {}", e.distance, e.way, e.min_count, e.count_range, e.random_scale, s.lifetime.len(), s.lifetime[0], s.lifetime[127], s.speed[0], s.speed[127], s.time_shift);
            }
        }
    }
    println!("light {:?}, shift {}", entry.light, entry.particles_shift);
    assert_eq!(entry.emitters.len(), 7);
    assert_eq!(entry.emitters.iter().filter(|e| matches!(e.kind, trp_data::drawing::particles::EmitterKind::Sparks(_))).count(), 1);
    let light = entry.light.expect("the pool's light");
    assert_eq!((light.range, light.intensity, light.time, light.max_distance, light.shadows), (35.0, 12.0, 1.0, 350.0, true));
}

/// The game's own renderer all the muzzles' sparks are emitted into
/// (`SimpleSparksRenderer`; firing spec, "The muzzle's sparks").
#[test]
fn the_muzzles_sparks_renderer_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let s = trp_data::drawing::particles::muzzle_sparks_renderer(&mut assets).expect("the sparks' renderer");
    println!("{} segments, {} sparks, {}; size {}, fade {}, turbulence {:?}, tint {:?}, picture {} {}x{}", s.segments, s.count, s.shader, s.size, s.fade_out, s.turbulence, s.tint, s.texture.name, s.texture.width, s.texture.height);
    assert_eq!((s.segments, s.count, s.shader.as_str()), (12, 700, "Custom/SimpleParticleMoveAdd"));
    assert_eq!((s.size, s.fade_out, s.turbulence), (0.04, 8.0, Some([8.0, 5.0, 30.0, 1.0])));
    assert_eq!(s.tint[0], 2.0);
}

/// The game's own renderer of every tracer (`TracersLight`; ballistics
/// spec, "A tracer"), and the install's tracer rounds by their colours.
#[test]
fn the_tracers_renderer_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install.clone()).expect("manifest");
    let t = trp_data::drawing::particles::tracers_light(&mut assets).expect("the tracers' renderer");
    println!(
        "{}: {} s, grenade {} s by {}, speed by {} and {}, brightness {}; picture {} {}x{}; size keys {:?}; alpha keys {:?}",
        t.shader,
        t.max_flying_time,
        t.grenade_flying_max_time,
        t.grenade_size,
        t.speed_modifier,
        t.grenade_speed_modifier,
        t.brightness,
        t.texture.name,
        t.texture.width,
        t.texture.height,
        t.size_time.keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>(),
        t.color_alpha.keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>()
    );
    assert_eq!((t.shader.as_str(), t.max_flying_time, t.grenade_flying_max_time), ("Custom/TracerLight", 4.0, 0.25));
    assert_eq!((t.grenade_size, t.speed_modifier, t.grenade_speed_modifier, t.brightness), (0.06, 0.01, 0.003, 1.0));
    assert_eq!((t.texture.name.as_str(), t.texture.width, t.texture.height), ("Tracer", 128, 128));
    assert_eq!(t.size_time.keys.len(), 5);
    assert!((t.size_time.evaluate(0.05) - 0.5).abs() < 1e-6 && (t.size_time.evaluate(0.7) - 0.575).abs() < 1e-6 && t.size_time.evaluate(1.0) == 0.0);
    assert!(t.color_alpha.evaluate(0.5) == 1.0 && t.color_alpha.evaluate(1.0) == 0.0);
    // The rounds: 37 tracers, by their colours as the spec lists them.
    let db = trp_data::Database::load(&install).expect("the database");
    let mut colours: std::collections::BTreeMap<String, usize> = Default::default();
    for item in db.items.values().filter(|item| item.props.get("Tracer").and_then(|v| v.as_bool()) == Some(true)) {
        *colours.entry(item.props.get("TracerColor").and_then(|v| v.as_str()).unwrap_or("?").to_owned()).or_default() += 1;
    }
    println!("tracer rounds by colour: {colours:?}");
    assert_eq!(colours.values().sum::<usize>(), 37);
    assert_eq!((colours["tracerRed"], colours["tracerGreen"], colours["red"], colours["green"]), (3, 1, 9, 4));
}

/// The particle system all the muzzles' fumes are emitted into, as the
/// install has it (firing spec, "The muzzle's fumes"; particles spec).
#[test]
fn the_muzzles_fume_system_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let fume = trp_data::drawing::particles::muzzle_fume_system(&mut assets).expect("the fume system");
    let r = &fume.renderer;
    println!(
        "{}: world {}, most {}, size {:?}, turn {:?}, colour {:?}, limit {:?}; drawn {} aligned {} sorted {} sizes {}..{}, material {} with {} ({:?}; {:?}; {:?})",
        fume.name,
        fume.world_space,
        fume.max_particles,
        fume.size_over_lifetime.as_ref().map(|c| c.max_curve.keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>()),
        fume.rotation_over_lifetime.as_ref().map(|c| (c.mode, c.min_scalar, c.scalar)),
        fume.color_over_lifetime.as_ref().map(|g| (&g.max_gradient.colors, &g.max_gradient.alphas)),
        fume.limit_velocity.as_ref().map(|l| (l.magnitude.scalar, l.dampen, l.drag.scalar)),
        r.render_mode,
        r.alignment,
        r.sort_mode,
        r.min_size,
        r.max_size,
        r.material.name,
        r.material.shader,
        r.material.textures.iter().map(|(n, t)| (n, &t.name, t.width, t.height, t.format, t.clamp)).collect::<Vec<_>>(),
        r.material.floats,
        r.material.colors
    );
    assert_eq!((fume.name.as_str(), fume.world_space, fume.max_particles), ("Effect Muzzle Smoke", true, 100));
    assert!(fume.unbuilt.is_empty(), "{:?}", fume.unbuilt);
    let turn = fume.rotation_over_lifetime.as_ref().expect("its turning");
    assert_eq!(turn.mode, 3);
    assert!((turn.scalar - 5f32.to_radians()).abs() < 1e-4 && (turn.min_scalar + 5f32.to_radians()).abs() < 1e-4);
    let colour = &fume.color_over_lifetime.as_ref().expect("its colour").max_gradient;
    assert_eq!((colour.colors.len(), colour.alphas.len()), (2, 4));
    assert!((colour.alphas[1].0 - 7710.0 / 65535.0).abs() < 1e-6);
    assert_eq!(fume.limit_velocity.as_ref().map(|l| (l.magnitude.scalar, l.dampen)), Some((0.0, 0.2)));
    assert_eq!((r.render_mode, r.alignment, r.sort_mode), (0, 0, 0));
    assert_eq!(r.material.shader, "Particles/Smoke Distorted");
    assert!(r.material.texture("_MainTex").is_some() && r.material.texture("_NoiseTex").is_some());
    assert_eq!((r.material.float("_AnimationSpeed"), r.material.float("_InvFade")), (Some(6.0), Some(3.0)));
}
