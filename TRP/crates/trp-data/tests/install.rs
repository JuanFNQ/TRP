//! Checks against a real install. They pass trivially, with a note, on a
//! machine where no install is configured.

use std::collections::HashMap;

use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
use trp_data::unity::extract::{read_mesh, read_texture};
use trp_data::unity::Assets;
use trp_data::weapon::{assemble, Weapon};
use trp_data::{Database, Install};

const AKM: &str = "59d6088586f774275f37482f";

fn akm() -> Option<(Assets, Weapon)> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = assemble(&mut assets, &db, AKM).expect("assemble AKM");
    Some((assets, weapon))
}

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
    let haze = trp_data::particles::muzzle_heat_haze_system(&mut assets).expect("the haze system");
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
    let d = trp_data::camera_effects::read_distortion(&install).expect("the camera's distortion");
    println!("the camera's distortion: {d:?}");
    assert_eq!((d.pixels, d.blur_iterations), (8.0, 1));
}

/// A grenade's explosion (`Effects`' `Grenade_new`, the F-1's): every one of
/// its systems read, each one's figures printed (firing spec, "A grenade's
/// explosion").
/// The body armour repair kit's shadow meshes cast their shadow alone
/// (`m_CastShadows` 3), its model's own do not: drawn, the shadow meshes
/// cover the kit in white (0.143.1).
#[test]
fn a_shadow_mesh_is_read_as_shadows_only() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let key = "assets/content/items/spec/item_spec_armorrepair/item_spec_armorrepair.bundle";
    let (hierarchy, _) = trp_data::icon::model_of(&mut assets, key).expect("the kit's model");
    let shadows_only = |name: &str| hierarchy.nodes.iter().find(|node| node.name == name).and_then(|node| node.renderer.as_ref()).map(|renderer| renderer.shadows_only);
    assert_eq!(shadows_only("shadow_LOD0"), Some(true));
    assert_eq!(shadows_only("item_spec_armorrepair_LOD0"), Some(false));
}

#[test]
fn a_grenade_s_explosion_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let effect = trp_data::particles::named_effect(&mut assets, "Grenade_new").expect("the explosion");
    let c = |m: &trp_data::particles::MinMaxCurve| match m.mode {
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
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let blasts = trp_data::particles::muzzle_blasts(&mut assets).expect("the muzzles' blasts");
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
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let entry = trp_data::particles::effect_entry(&mut assets, "Flashbang").expect("the flashbang's entry");
    for e in &entry.emitters {
        match &e.kind {
            trp_data::particles::EmitterKind::System(a) => {
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
            trp_data::particles::EmitterKind::Sparks(s) => {
                println!("sparks: within {:?} m, way {}, {} + <{}, scale {:?}; {} places, lifetime {}..{}, speed {}..{}, time shift {}", e.distance, e.way, e.min_count, e.count_range, e.random_scale, s.lifetime.len(), s.lifetime[0], s.lifetime[127], s.speed[0], s.speed[127], s.time_shift);
            }
        }
    }
    println!("light {:?}, shift {}", entry.light, entry.particles_shift);
    assert_eq!(entry.emitters.len(), 7);
    assert_eq!(entry.emitters.iter().filter(|e| matches!(e.kind, trp_data::particles::EmitterKind::Sparks(_))).count(), 1);
    let light = entry.light.expect("the pool's light");
    assert_eq!((light.range, light.intensity, light.time, light.max_distance, light.shadows), (35.0, 12.0, 1.0, 350.0, true));
}

/// The game's own renderer all the muzzles' sparks are emitted into
/// (`SimpleSparksRenderer`; firing spec, "The muzzle's sparks").
#[test]
fn the_muzzles_sparks_renderer_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let s = trp_data::particles::muzzle_sparks_renderer(&mut assets).expect("the sparks' renderer");
    println!("{} segments, {} sparks, {}; size {}, fade {}, turbulence {:?}, tint {:?}, picture {} {}x{}", s.segments, s.count, s.shader, s.size, s.fade_out, s.turbulence, s.tint, s.texture.name, s.texture.width, s.texture.height);
    assert_eq!((s.segments, s.count, s.shader.as_str()), (12, 700, "Custom/SimpleParticleMoveAdd"));
    assert_eq!((s.size, s.fade_out, s.turbulence), (0.04, 8.0, Some([8.0, 5.0, 30.0, 1.0])));
    assert_eq!(s.tint[0], 2.0);
}

/// Every clothing suit of the install, of every side, with its parts'
/// bundles, each of which reads (inventory spec, "The OVERALL tab").
#[test]
fn every_suit_and_its_parts_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let suits = trp_data::suits::suits(&install, "en").expect("the suits");
    let (upper, lower): (Vec<_>, Vec<_>) = suits.iter().partition(|suit| suit.upper);
    println!("{} upper suits, {} lower", upper.len(), lower.len());
    assert!(upper.len() >= 90 && lower.len() >= 60);
    let bear = upper.iter().find(|suit| suit.id == "5cd945d71388ce000a659dfb").expect("the BEAR's own upper suit");
    assert_eq!(bear.name, "BEAR Standard");
    assert_eq!(bear.body.as_deref(), Some("assets/content/characters/character/prefabs/tshirt_bear_turtleneck.bundle"));
    assert_eq!(bear.hands.as_deref(), Some("assets/content/hands/bear/bear_hands_skin.bundle"));
    let mut assets = Assets::new(install.clone()).expect("manifest");
    let (mut torsos, mut whole) = (0, 0);
    for key in suits.iter().flat_map(|suit| [&suit.body, &suit.feet].into_iter().flatten()) {
        let part = trp_data::arms::load(&mut assets, key).unwrap_or_else(|e| panic!("{key}: {e:#}"));
        torsos += usize::from(part.torso.is_some());
        if part.torso.is_some() {
            let suit = suits.iter().find(|suit| suit.body.as_ref() == Some(key) || suit.feet.as_ref() == Some(key)).expect("its suit");
            println!("a torso of the newer kind: {} {} {:?}", suit.id, suit.name, suit.sides);
        }
        whole += usize::from(part.torso.is_some_and(|torso| torso.armor.is_some() && torso.vest.is_some()));
    }
    println!("{torsos} torsos of the newer kind, {whole} with all three meshes");
    assert!(torsos > 0);
    // Every bone a suit's part hangs on is one of the player's skeleton's,
    // which the body's animator poses.
    let mut paths: Vec<String> = Vec::new();
    for key in suits.iter().flat_map(|suit| [&suit.body, &suit.feet].into_iter().flatten()) {
        let part = trp_data::arms::load(&mut assets, key).expect("read above");
        paths.extend(part.bone_paths.into_iter().filter(|path| !paths.contains(path)).collect::<Vec<_>>());
    }
    let wanted: Vec<&str> = paths.iter().map(String::as_str).collect();
    let bones = trp_data::body::bones(&mut assets, &wanted).expect("every suit's bone on the player's skeleton");
    println!("{} bones under the suits' bodies and legs", bones.len());
    // And every suit's hands on the bones a weapon's rig has, as the
    // default hands are (`arms_bind_to_bones_the_weapon_has`).
    if let Some((mut assets, weapon)) = akm() {
        let rig: Vec<String> = (0..weapon.hierarchy.nodes.len()).map(|i| weapon.hierarchy.path(i)).collect();
        for key in suits.iter().filter_map(|suit| suit.hands.as_ref()) {
            let hands = trp_data::arms::load(&mut assets, key).unwrap_or_else(|e| panic!("{key}: {e:#}"));
            for path in hands.weapon_bone_paths() {
                assert!(rig.iter().any(|p| p == path), "{key}: the weapon has no bone {path}");
            }
        }
    }
    // The OVERALL tab's own figures, as the inventory's scene has them.
    let overall = trp_data::screen::OverallPanel::load(&install, "level44").expect("the OVERALL panel");
    let camera = overall.camera.expect("the model view's camera");
    println!("the OVERALL tab: camera {camera:?}, turn {:?}, most seen {:?}, highlight {:?}, line tints {:?}, sides {:?}", overall.turn_a_pixel, overall.dropdown_most_seen, overall.dropdown_highlight, overall.line_tints, overall.side_sprites);
    assert_eq!((camera.fov, camera.near, camera.far), (28.0, 0.3, 25.0));
    assert!((camera.place[1] + 0.2).abs() < 1e-3 && (camera.place[2] - 0.49).abs() < 1e-3);
    assert_eq!(overall.turn_a_pixel, Some(0.3));
    assert_eq!(overall.dropdown_most_seen, Some(120.0));
    assert!(overall.side_sprites.iter().all(Option::is_some));
    for (index, key, case) in &overall.localized {
        println!("localized {}: {key:?} case {case}, text {:?}", overall.panel.nodes[*index].path, overall.panel.nodes[*index].text.as_ref().map(|t| (&t.text, t.color, t.size)));
    }
}

/// The particle system all the muzzles' fumes are emitted into, as the
/// install has it (firing spec, "The muzzle's fumes"; particles spec).
#[test]
fn the_muzzles_fume_system_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let fume = trp_data::particles::muzzle_fume_system(&mut assets).expect("the fume system");
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

#[test]
fn stock_akm_assembles_with_every_part() {
    let Some((assets, weapon)) = akm() else { return };
    assert!(weapon.skipped.is_empty(), "parts not placed: {:?}", weapon.skipped);

    let rendered: Vec<_> = weapon.hierarchy.nodes.iter().filter(|n| n.renderer.is_some()).collect();
    // Seven meshes on the base rifle and one for each of the eight parts.
    assert_eq!(rendered.len(), 15);
    assert!(rendered.iter().all(|n| n.name.ends_with("LOD0")), "a lower detail mesh was kept");

    for node in rendered {
        let renderer = node.renderer.as_ref().unwrap();
        let mesh = read_mesh(&assets, renderer.mesh).expect("mesh");
        assert!(!mesh.positions.is_empty(), "{} has no vertices", node.name);
        assert_eq!(mesh.normals.len(), mesh.positions.len());
        assert_eq!(mesh.uv0.len(), mesh.positions.len());
        let vertex_count = mesh.positions.len() as u32;
        assert!(mesh.submeshes.iter().flatten().all(|&i| i < vertex_count));
        for material in renderer.materials.iter().flatten() {
            let material = trp_data::unity::extract::read_material(&assets, *material).expect("material");
            let main = material.textures.get("_MainTex").expect("main texture");
            let texture = read_texture(&assets, *main).expect("texture");
            assert!(texture.width > 0 && !texture.data.is_empty());
        }
    }
}

/// A texture's levels read again from its bundle's file, only the blocks
/// they are in decompressed (`ResourcePlace`, for the streaming of
/// pictures), are the bytes the whole bundle gave.
#[test]
fn a_texture_read_again_from_its_bundle_is_the_same() {
    let Some((assets, weapon)) = akm() else { return };
    let mut checked = 0;
    for renderer in weapon.hierarchy.nodes.iter().filter_map(|n| n.renderer.as_ref()) {
        for material in renderer.materials.iter().flatten() {
            let material = trp_data::unity::extract::read_material(&assets, *material).expect("material");
            for texture in material.textures.values() {
                let (texture, place) = trp_data::unity::extract::read_texture_shared(&assets, *texture).expect("texture");
                let Some(place) = place else { continue };
                assert_eq!(place.read().expect("read again"), texture.data, "{}", texture.name);
                checked += 1;
            }
        }
    }
    assert!(checked > 10, "only {checked} textures kept beside their files");
}

/// The game's engine streams a texture by its levels only where it was
/// imported so (`m_StreamingMipmaps`), and draws the rest whole when
/// streaming is on. Every one of the AKM's 27 is (read 2026-10-15, each of
/// priority 0).
#[test]
fn the_weapon_s_textures_are_marked_for_streaming() {
    let Some((assets, weapon)) = akm() else { return };
    let mut seen = std::collections::BTreeMap::new();
    for renderer in weapon.hierarchy.nodes.iter().filter_map(|n| n.renderer.as_ref()) {
        for material in renderer.materials.iter().flatten() {
            let material = trp_data::unity::extract::read_material(&assets, *material).expect("material");
            for texture in material.textures.values() {
                let texture = read_texture(&assets, *texture).expect("texture");
                seen.insert(texture.name.clone(), texture.streaming_mipmaps);
            }
        }
    }
    assert_eq!(seen.len(), 27);
    assert!(seen.values().all(|streamed| *streamed), "not streamed: {:?}", seen.iter().filter(|(_, s)| !**s).collect::<Vec<_>>());
}

/// The idle clip starts in the pose the prefab is stored in, so its first
/// frame must reproduce the prefab's rotations. This is what pins down the
/// order Euler angles are applied in.
#[test]
fn idle_clip_starts_in_the_rest_pose() {
    let Some((assets, weapon)) = akm() else { return };
    let nodes: HashMap<u32, usize> =
        (0..weapon.hierarchy.nodes.len()).map(|i| (path_hash(&weapon.hierarchy.path(i)), i)).collect();
    let clips = weapon.clips(&assets).expect("clips");
    let (_, idle) = clips.iter().find(|(name, _)| name == "akm_idle_armed").expect("idle clip");
    let clip = Clip::read(&assets, *idle).expect("decode");

    let mut values = Vec::new();
    clip.sample(0.0, &mut values);
    let mut checked = 0;
    for track in clip.tracks.iter().filter(|t| t.property == Property::Euler) {
        let Some(&node) = nodes.get(&track.path) else { continue };
        let q = euler_to_quat([values[track.curve], values[track.curve + 1], values[track.curve + 2]]);
        let rest = weapon.hierarchy.nodes[node].rotation;
        let dot: f32 = q.iter().zip(&rest).map(|(a, b)| a * b).sum();
        let degrees = 2.0 * dot.abs().min(1.0).acos().to_degrees();
        assert!(degrees < 0.5, "{} is {degrees} degrees from rest", weapon.hierarchy.nodes[node].name);
        checked += 1;
    }
    assert!(checked > 50, "only {checked} rotation tracks landed on the rig");
}

/// What a character wears is read from the item's own bundle in one of
/// two forms: a skin for the player's skeleton (armour, vests, backpacks,
/// the last with a mesh for each thing it may be worn over) or a model
/// for the head bone (a helmet).
#[test]
fn worn_items_are_skins_or_models_for_the_head() {
    use trp_data::dress::{load, Under, Worn};
    let Some((mut assets, _)) = akm() else { return };
    const ARMOUR: &str = "assets/content/items/equipment/armor_paca/item_equipment_armor_paca.bundle";
    const BACKPACK: &str = "assets/content/items/equipment/backpack_baselard/item_equipment_backpack_baselard.bundle";
    const HELMET: &str = "assets/content/items/equipment/helmet_6b47/item_equipment_helmet_6b47.bundle";

    let Worn::Skinned(armour) = load(&mut assets, ARMOUR, Under::default()).expect("armour") else { panic!("armour is a skin") };
    assert!(armour.bone_paths.iter().any(|path| path.contains("Base HumanSpine")), "{:?}", armour.bone_paths);

    // A backpack has a mesh for the bare body and one for each of
    // armour, a vest and both under it.
    let mut meshes = Vec::new();
    for under in [(false, false), (true, false), (false, true), (true, true)] {
        let worn = load(&mut assets, BACKPACK, Under { armour: under.0, vest: under.1 }).expect("backpack");
        let Worn::Skinned(pack) = worn else { panic!("a backpack is a skin") };
        assert!(!pack.bone_paths.is_empty());
        meshes.push(pack.mesh);
    }
    println!("an armour on {} bones; a backpack's four meshes: {meshes:?}", armour.bone_paths.len());
    assert!(meshes.iter().enumerate().all(|(i, mesh)| meshes[..i].iter().all(|other| other != mesh)), "{meshes:?}");

    let Worn::Rigid(helmet) = load(&mut assets, HELMET, Under::default()).expect("helmet") else { panic!("a helmet is a model") };
    let drawn = helmet.nodes.iter().filter(|node| node.renderer.is_some()).count();
    println!("a helmet: {} objects, {drawn} of them drawn", helmet.nodes.len());
    assert!(drawn >= 1);
}

#[test]
fn arms_bind_to_bones_the_weapon_has() {
    let Some((mut assets, weapon)) = akm() else { return };
    let arms = trp_data::arms::load(&mut assets, "assets/content/hands/bear/bear_hands_skin.bundle").expect("arms");
    let mesh = read_mesh(&assets, arms.mesh).expect("arms mesh");
    assert_eq!(mesh.bind_poses.len(), arms.bone_paths.len());
    assert_eq!(mesh.weights.len(), mesh.positions.len());

    let rig: Vec<String> = (0..weapon.hierarchy.nodes.len()).map(|i| weapon.hierarchy.path(i)).collect();
    for path in arms.weapon_bone_paths() {
        assert!(rig.iter().any(|p| p == path), "weapon has no bone {path}");
    }
    let bone_count = arms.bone_paths.len() as u16;
    assert!(mesh.joints.iter().flatten().all(|&j| j < bone_count));
}

/// The body's animator controller reads in full: parameters, the base
/// layer's states with the movement state each stands for, and the layer
/// synchronised with it that poses the arms in a sprint.
#[test]
fn the_body_controller_reads_in_full() {
    let Some((mut assets, weapon)) = akm() else { return };
    // The stock AKM's width in inventory cells with its parts fitted,
    // which the body's animator takes as a parameter.
    assert_eq!(weapon.cell_width, 5);

    let table = trp_data::body::controller(&mut assets).expect("body controller");
    println!("{} parameters, {} states, {} layers", table.parameters.len(), table.states.len(), table.layers.len());
    let kind = |name: &str| table.parameters.iter().find(|p| p.name == name).map(|p| (p.kind, p.default));
    use trp_data::states::ParameterKind::{Bool, Float};
    assert_eq!(kind("Inert"), Some((Bool, 0.0)));
    assert_eq!(kind("Level"), Some((Float, 1.0)));
    assert_eq!(kind("FootStep"), Some((Float, 0.0)));

    // The game's numbers for standing, walking and sprinting.
    let movement = |name: &str| table.find(name).and_then(|s| s.movement.as_ref()).map(|m| m.state);
    assert_eq!(movement("Base Layer.Stand.Idle_Aim"), Some(1));
    assert_eq!(movement("Base Layer.StateMachine_Move.MOVE"), Some(4));
    assert_eq!(movement("Base Layer.StateMachine_Sprint.Sprint"), Some(5));
    let idle = table.find("Base Layer.Stand.Idle_Aim").unwrap();
    assert!(idle.movement.as_ref().unwrap().default_state);
    // Standing gives way to walking over a fixed 0.15 s.
    let to_walk = idle.transition_to("Base Layer.StateMachine_Move.MOVE").expect("a way from standing to walking");
    assert!(to_walk.fixed_duration && (to_walk.duration - 0.15).abs() < 1e-4);

    // The sprint has a tree for its own layer and one for the hands layer.
    let sprint = table.find("Base Layer.StateMachine_Sprint.Sprint").unwrap();
    let hands = table.layers.iter().find(|l| l.name == "Sync_SprintHands").expect("the hands layer");
    assert_eq!(hands.machine, sprint.machine);
    assert!(hands.sync_slot > 0);
    assert!(sprint.tree_of_layer[hands.sync_slot] >= 0);
    assert_eq!(idle.tree_of_layer[hands.sync_slot], -1, "standing has nothing on the hands layer");

    let bones = trp_data::body::bones(&mut assets, &["Root_Joint", "Root_Joint/Base HumanPelvis"]).expect("bones");
    println!("the pelvis rests {:?} above the root", bones[1].position);
    assert!((0.8..1.1).contains(&bones[1].position[1]));
}

/// The engine's three two-parameter blends, worked out here, against the
/// weight grids the install stores for the same trees.
#[test]
fn two_parameter_blends_match_the_stored_grids() {
    use trp_data::rootmotion::{BlendType, Node, RootMotionTable};
    let Some((mut assets, _)) = akm() else { return };
    let table = RootMotionTable::load(&mut assets).expect("root motion table");
    // Per kind of blend: how many trees were checked and the worst
    // difference in any weight anywhere on any grid.
    let mut found: std::collections::BTreeMap<String, (usize, f32, String)> = Default::default();
    fn visit(
        table: &RootMotionTable,
        state: &str,
        node: &Node,
        found: &mut std::collections::BTreeMap<String, (usize, f32, String)>,
    ) {
        let Node::Tree { blend, children, .. } = node else { return };
        // The freeform directional kind is only held to the engine's
        // figures among its children. Far outside them (ten times as far
        // out, or on the far side of the origin) the stored grids differ
        // from what is computed here, by up to a half in places; no state
        // the simulation uses takes its parameters there.
        let among_children = *blend == BlendType::FreeformDirectional2D;
        if let Some(mismatch) = table.grid_mismatch_in(node, among_children) {
            let entry = found.entry(format!("{blend:?}")).or_insert((0, 0.0, String::new()));
            entry.0 += 1;
            if mismatch > entry.1 {
                (entry.1, entry.2) = (mismatch, state.to_owned());
            }
        }
        for child in children {
            visit(table, state, child, found);
        }
    }
    for (name, node) in table.states.values() {
        visit(&table, name, node, &mut found);
    }
    for (kind, (count, worst, state)) in &found {
        println!("{kind}: {count} trees, largest weight difference {worst:.5} (in {state})");
    }
    for kind in [BlendType::FreeformCartesian2D, BlendType::FreeformDirectional2D, BlendType::SimpleDirectional2D] {
        let (count, worst, state) = found.get(&format!("{kind:?}")).unwrap_or_else(|| panic!("no {kind:?} tree in the table"));
        assert!(*count > 0);
        assert!(*worst < 2e-3, "{kind:?} differs by {worst} in {state}");
    }
}

/// The player's own settings for the game are read where the install has
/// them, and what is read lies in the game's own ranges for them.
#[test]
fn the_installs_view_settings_are_in_range() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let settings = install.player_settings();
    println!("the install's settings: {settings:?}");
    if let Some(field_of_view) = settings.field_of_view {
        assert!((50.0..=75.0).contains(&field_of_view), "field of view {field_of_view}");
    }
    if let Some(head_bobbing) = settings.head_bobbing {
        assert!((0.2..=1.0).contains(&head_bobbing), "head bobbing {head_bobbing}");
    }
    for sensitivity in [settings.mouse_sensitivity, settings.aiming_sensitivity].into_iter().flatten() {
        assert!(sensitivity > 0.0 && sensitivity <= 5.0, "sensitivity {sensitivity}");
    }
}

/// An outfit that is not the install's (`docs/spec/outfits.md`): the
/// Sunrise suit's model, read from the other game's folders where those
/// are configured, is fitted to the skeleton of the character's own
/// clothing with its own build kept. Skipped with a note where either
/// game is not there.
#[test]
fn an_outfit_of_another_game_is_fitted_to_the_body() {
    use trp_data::outfit::{self, Follows, Move, RestBone, ARMS, LEGS, UPPER};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let Some(definition) = db.outfits.values().find(|definition| definition.name == "outfit_stalker_sunrise") else {
        eprintln!("skipped: the outfit's file is not found from here ({:?})", db.custom_notes);
        return;
    };
    let stalker = trp_data::xray::Stalker::locate();
    let visual = definition.visual.clone().expect("the suit names a body");
    let Some(file) = stalker.find(&format!("meshes/{visual}")) else {
        eprintln!("skipped: the other game's folders are not configured, or have no {visual} (local/gamma_dir.txt, local/anomaly_dir.txt)");
        return;
    };
    // It is an item like its donor, and one piece.
    let item = db.item(&definition.id).expect("its item");
    assert_eq!(item.parent, db.item(&definition.donor).unwrap().parent);
    assert_eq!(item.props["Slots"].as_array().map(Vec::len), Some(0));

    // The skeleton as the default clothing has it at rest.
    let mut assets = Assets::new(install).expect("manifest");
    let mut skeleton: Vec<RestBone> = Vec::new();
    // The box about the install's own legs at rest: its top is the waist.
    let mut waist: Option<[[f32; 3]; 2]> = None;
    for key in ["tshirt_bear_turtleneck", "pants_bear_srvv", "bear_head_0"] {
        let part = trp_data::arms::load(&mut assets, &format!("assets/content/characters/character/prefabs/{key}.bundle")).expect("a piece of clothing");
        let mesh = read_mesh(&assets, part.mesh).expect("its mesh");
        for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
            if !skeleton.iter().any(|known| &known.path == path) {
                skeleton.push(RestBone { path: path.clone(), bind: *bind });
            }
        }
        if key.starts_with("pants") {
            waist = outfit::box_about(&mesh.positions);
        }
    }
    let waist = waist.expect("the legs have a box");
    println!("  the install's legs at rest: from {:.3} {:.3} {:.3} to {:.3} {:.3} {:.3}", waist[0][0], waist[0][1], waist[0][2], waist[1][0], waist[1][1], waist[1][2]);
    let fitted = outfit::build(definition, &stalker, &skeleton, Some(waist));
    for note in &fitted.notes {
        println!("  {note}");
    }
    let rig = &fitted.rig;
    assert!(fitted.has_body(), "the suit's body is drawn");
    // What its own eyes see of it is under its waist, which is as far
    // over its own hips as the top of the install's legs is over the
    // install's (the install's meshes stand along z); and it is not
    // nothing.
    let hips: Vec<_> = ["LThigh1", "RThigh1"].iter().map(|name| rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{name}"))).expect("a hip")).collect();
    let (ours_up, theirs_up) = ((hips[0].joint[2] + hips[1].joint[2]) / 2.0, (hips[0].their_joint[2] + hips[1].their_joint[2]) / 2.0);
    let top = ours_up + waist[1][2] - theirs_up;
    println!("  the waist: {:.3} m up on the install's own body ({:.3} over its hips at {theirs_up:.3}), {top:.3} on the suit (its hips at {ours_up:.3})", waist[1][2], waist[1][2] - theirs_up);
    let mut seen = 0;
    for piece in &fitted.pieces {
        for corner in &piece.mesh.submeshes[LEGS] {
            assert!(piece.mesh.positions[*corner as usize][2] <= top + 1e-5, "a corner of the legs' list is over the waist");
        }
        for triangle in piece.mesh.submeshes[UPPER].chunks_exact(3) {
            assert!(triangle.iter().any(|corner| piece.mesh.positions[*corner as usize][2] > top), "a triangle under the waist is the upper body's");
        }
        seen += piece.mesh.submeshes[LEGS].len() / 3;
    }
    assert!(seen > 500, "{seen} triangles under the waist");
    // Without its body's model it leaves the character's own body.
    let mut bodiless = (**definition).clone();
    bodiless.visual = Some("actors/no_such/model.ogf".to_owned());
    let left = outfit::build(&bodiless, &stalker, &skeleton, Some(waist));
    assert!(!left.has_body() && left.rig.bones.is_empty() && left.notes.iter().any(|note| note.contains("the character's own")));
    assert_eq!(left.has_arms(), fitted.has_arms());
    for piece in &fitted.pieces {
        let mesh = &piece.mesh;
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &mesh.positions {
            assert!(p.iter().all(|v| v.is_finite()));
            for axis in 0..3 {
                (low[axis], high[axis]) = (low[axis].min(p[axis]), high[axis].max(p[axis]));
            }
        }
        println!(
            "  {}: {} vertices; triangles: legs {}, arms {}, the rest {}; from {:.3} {:.3} {:.3} to {:.3} {:.3} {:.3}; its picture {}",
            piece.texture,
            mesh.positions.len(),
            mesh.submeshes[LEGS].len() / 3,
            mesh.submeshes[ARMS].len() / 3,
            mesh.submeshes[UPPER].len() / 3,
            low[0],
            low[1],
            low[2],
            high[0],
            high[1],
            high[2],
            piece.picture.as_ref().map_or("not found".to_owned(), |p| format!("{} x {} {:?}", p.width, p.height, p.packing)),
        );
        assert!(mesh.weights.iter().all(|shares| (shares.iter().sum::<f32>() - 1.0).abs() < 1e-4));
        assert!(mesh.joints.iter().flatten().all(|joint| (*joint as usize) < rig.bones.len()));
        assert_eq!((mesh.normals.len(), mesh.uv0.len(), mesh.bind_poses.len()), (mesh.positions.len(), mesh.positions.len(), rig.bones.len()));
    }

    // The model's own build is kept. Set beside the model as its file has
    // it (y up there, z up here): as tall, standing on the ground; its
    // hips and its shoulders as far apart; every limb's bone as long.
    let model = trp_data::xray::read(&std::fs::read(&file).unwrap()).unwrap();
    let ours = |name: &str| model.bones.iter().find(|bone| bone.name == name).unwrap().at;
    let joint = |name: &str| rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{name}"))).map(|bone| (bone.joint, bone.their_joint)).unwrap();
    let apart = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let body = &fitted.pieces[0];
    let heights: Vec<f32> = body.mesh.positions.iter().map(|p| p[2]).collect();
    let (lowest, highest) = (heights.iter().copied().fold(f32::MAX, f32::min), heights.iter().copied().fold(f32::MIN, f32::max));
    let tall = model.parts[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max) - model.parts[0].positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    println!("  the model is {tall:.3} m tall; fitted, from {lowest:.3} to {highest:.3}");
    assert!(lowest.abs() < 0.005 && (highest - lowest - tall).abs() < 0.005, "from {lowest} to {highest}, of a model {tall} tall");
    for (what, pair, their_pair) in [("hips", ["bip01_l_thigh", "bip01_r_thigh"], ["LThigh1", "RThigh1"]), ("shoulders", ["bip01_l_upperarm", "bip01_r_upperarm"], ["LUpperarm", "RUpperarm"])] {
        let (was, is, theirs) = (apart(ours(pair[0]), ours(pair[1])), apart(joint(their_pair[0]).0, joint(their_pair[1]).0), apart(joint(their_pair[0]).1, joint(their_pair[1]).1));
        println!("  the {what}: {was:.3} m apart on the model, {is:.3} on its rig, {theirs:.3} on the install's skeleton");
        assert!((was - is).abs() < 1e-4);
    }
    for (what, from, to, their_from, their_to) in [
        ("upper arm", "bip01_l_upperarm", "bip01_l_forearm", "LUpperarm", "LForearm1"),
        ("forearm", "bip01_l_forearm", "bip01_l_hand", "LForearm1", "LPalm"),
        ("thigh", "bip01_r_thigh", "bip01_r_calf", "RThigh1", "RCalf"),
        ("calf", "bip01_r_calf", "bip01_r_foot", "RCalf", "RFoot"),
    ] {
        let (was, is, theirs) = (apart(ours(from), ours(to)), apart(joint(their_from).0, joint(their_to).0), apart(joint(their_from).1, joint(their_to).1));
        println!("  the {what}: {was:.3} m long on the model, {is:.3} on its rig, {theirs:.3} on the install's skeleton");
        assert!((was - is).abs() < 1e-3, "the {what} is {is} long for {was}");
    }
    assert!(body.picture.is_some());
    assert!([LEGS, ARMS, UPPER].iter().all(|zone| !body.mesh.submeshes[*zone].is_empty()));

    // The rig: the hips, the hands and the feet are the install's own,
    // and at rest it moves nothing.
    for name in ["Pelvis", "LPalm", "RPalm", "LFoot", "RFoot"] {
        let bone = rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{name}"))).unwrap();
        assert_eq!(bone.follows, Follows::Theirs, "{name}");
    }
    let at_rest = rig.pose(&vec![Move::STILL; rig.bones.len()]);
    for (bone, moved) in rig.bones.iter().zip(&at_rest) {
        let off = moved.shift.iter().chain(moved.axes[0].iter().skip(1)).chain(moved.axes[1].iter().skip(2)).fold(0.0f32, |most, v| most.max(v.abs()));
        assert!(off < 1e-3, "{} is moved at rest: {moved:?}", bone.path);
    }
    // And the hands are on the install's hands, knuckle on knuckle.
    for (palm, knuckle) in [("LPalm", "LDigit31"), ("RPalm", "RDigit31")] {
        let (at, _) = joint(palm);
        assert!(apart(joint(knuckle).0, joint(knuckle).1) < 1e-4);
        let palm_bone = rig.bones.iter().position(|bone| bone.path.ends_with(palm)).unwrap() as u16;
        let of_hand: Vec<[f32; 3]> = body.mesh.positions.iter().zip(body.mesh.joints.iter().zip(&body.mesh.weights)).filter(|(_, (joints, weights))| (0..4).any(|slot| joints[slot] == palm_bone && weights[slot] > 0.9)).map(|(p, _)| *p).collect();
        let furthest = of_hand.iter().map(|p| apart(*p, at)).fold(0.0, f32::max);
        println!("  {palm}: {} vertices wholly the hand's, up to {furthest:.3} m off its joint", of_hand.len());
        assert!(!of_hand.is_empty() && furthest < 0.16, "the hand's vertices are up to {furthest} m from its joint");
    }
}

/// The same outfit's arms for first person (the other game's own model
/// for them), fitted to the install's arms: the hand's skeleton the size
/// of the install's (its knuckles' joints as far apart), every joint of
/// every finger on the install's own, the wrist on the install's wrist,
/// each arm lying as the install's lies, and at rest nothing moved.
/// Prints the palm's skin from edge to edge beside the install's own
/// hands, gloved and bare, and how far each joint of a hand is from the
/// install's own.
#[test]
fn an_outfit_s_arms_for_first_person_are_fitted_to_the_install_s() {
    use trp_data::outfit::{self, Follows, Move, RestBone, Skin, ARMS};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let Some(definition) = db.outfits.values().find(|definition| definition.name == "outfit_stalker_sunrise") else {
        eprintln!("skipped: the outfit's file is not found from here ({:?})", db.custom_notes);
        return;
    };
    let stalker = trp_data::xray::Stalker::locate();
    let Some(file) = definition.arms.as_ref().and_then(|visual| stalker.find(&format!("meshes/{visual}"))) else {
        eprintln!("skipped: the outfit names no arms, or the other game's folders have none");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let mut skeleton: Vec<RestBone> = Vec::new();
    for key in ["tshirt_bear_turtleneck", "pants_bear_srvv", "bear_head_0"] {
        let part = trp_data::arms::load(&mut assets, &format!("assets/content/characters/character/prefabs/{key}.bundle")).expect("a piece of clothing");
        let mesh = read_mesh(&assets, part.mesh).expect("its mesh");
        for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
            if !skeleton.iter().any(|known| &known.path == path) {
                skeleton.push(RestBone { path: path.clone(), bind: *bind });
            }
        }
    }
    let model = trp_data::xray::read(&std::fs::read(&file).unwrap()).unwrap();
    let (meshes, rig, size) = outfit::fit_arms(&model, &skeleton, definition.arms_size).expect("the arms are fitted");
    println!("  the arms' model is drawn at {size:.3} times its file's size");
    let apart = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let ours = |name: &str| model.bones.iter().find(|bone| bone.name == name || bone.name == format!("bip01_{name}")).unwrap().at;
    let joint = |name: &str| rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{name}"))).map(|bone| (bone.joint, bone.their_joint)).unwrap();
    // Left to itself, its hand's skeleton is the size of the install's:
    // the knuckles' joints as far apart, both hands' taken together.
    let (broad, their_broad) = (apart(ours("l_finger1"), ours("l_finger4")) + apart(ours("r_finger1"), ours("r_finger4")), apart(joint("LDigit21").1, joint("LDigit51").1) + apart(joint("RDigit21").1, joint("RDigit51").1));
    println!("  the hand across its knuckles' joints: {:.1} cm in the model's file, {:.1} on the install's skeleton", broad * 50.0, their_broad * 50.0);
    if definition.arms_size.is_none() {
        assert!((broad * size - their_broad).abs() < 1e-4, "{broad} times {size} for {their_broad}");
    }
    for (what, from, to, their_from, their_to) in [
        ("upper arm", "l_upperarm", "l_forearm", "LUpperarm", "LForearm1"),
        ("forearm", "l_forearm", "l_hand", "LForearm1", "LPalm"),
        ("palm, wrist to knuckle", "l_hand", "l_finger2", "LPalm", "LDigit31"),
        ("middle finger's first joint", "l_finger2", "l_finger21", "LDigit31", "LDigit32"),
        ("thumb's first joint", "l_finger0", "l_finger01", "LDigit11", "LDigit12"),
    ] {
        let (was, is, theirs) = (apart(ours(from), ours(to)), apart(joint(their_from).0, joint(their_to).0), apart(joint(their_from).1, joint(their_to).1));
        println!("  the {what}: {:.1} cm in the model's file ({:.1} at its size), {:.1} on its rig, {:.1} on the install's skeleton", was * 100.0, was * size * 100.0, is * 100.0, theirs * 100.0);
        // The upper arm is as long as the model's, at its size; a finger's
        // joint and the hand from its wrist are as long as the install's.
        match what {
            "upper arm" => assert!((was * size - is).abs() < 1e-4, "the {what} is {is} long for {was} times {size}"),
            "forearm" => {}
            _ => assert!((theirs - is).abs() < 1e-4, "the {what} is {is} long for the install's {theirs}"),
        }
    }
    // The wrist is the install's own: the arm bends there, wherever the
    // model has the joint, and each forearm bone is held there.
    for side in ["L", "R"] {
        let (at, their_at) = joint(&format!("{side}Palm"));
        assert!(apart(at, their_at) < 1e-4, "the {side} wrist is {} from the install's", apart(at, their_at));
        let palm = rig.bones.iter().position(|bone| bone.path.ends_with(&format!("Base Human{side}Palm"))).unwrap();
        assert_eq!(rig.bones[palm].follows, Follows::Theirs);
        for name in ["Forearm1", "Forearm2", "Forearm3"] {
            let bone = rig.bones.iter().find(|bone| bone.path.ends_with(&format!("Base Human{side}{name}"))).unwrap();
            assert_eq!(bone.follows, Follows::Held { from: palm, at: palm }, "{side}{name}");
        }
    }
    println!("  the model has its wrist's joint {:.1} cm nearer its knuckles, at its size, than the install's skeleton has its own", (apart(joint("LPalm").1, joint("LDigit31").1) - apart(ours("l_hand"), ours("l_finger2")) * size) * 100.0);
    // The palm's skin from edge to edge and through, behind the knuckles
    // (`palm_box`): the model's as fitted, beside the install's own hands
    // (gloved, the default; bare, the Telnik's).
    let paths: Vec<String> = rig.bones.iter().map(|bone| bone.path.clone()).collect();
    let fitted = Skin::of(meshes.iter(), &paths);
    let mut their_skins = Vec::new();
    for (name, key) in [("the install's gloved hands", "assets/content/hands/bear/bear_hands_skin.bundle"), ("the install's bare hands", "assets/content/hands/hands_bear_telnik/hands_bear_telnik.skin.bundle")] {
        let part = trp_data::arms::load(&mut assets, key).expect("hands");
        let mesh = read_mesh(&assets, part.mesh).expect("their mesh");
        their_skins.push((name, Skin::of([&mesh], &part.bone_paths)));
    }
    let measured = |skin: &Skin| -> [f32; 2] {
        let mut sums = [0.0f32; 2];
        for side in ["L", "R"] {
            let [across, through] = outfit::palm_box(skin, &skeleton, side).expect("the hand's joints").expect("a palm");
            sums = [sums[0] + (across[1] - across[0]) * 0.5, sums[1] + (through[1] - through[0]) * 0.5];
        }
        sums
    };
    let [broad, thick] = measured(&fitted);
    println!("  the palm behind its knuckles, both hands' taken together: {:.1} cm from edge to edge and {:.1} through, as fitted", broad * 100.0, thick * 100.0);
    for (name, skin) in &their_skins {
        let [their_broad, their_thick] = measured(skin);
        println!("    {name}: {:.1} cm from edge to edge and {:.1} through (the fitted palm is {:.2} and {:.2} times that)", their_broad * 100.0, their_thick * 100.0, broad / their_broad, thick / their_thick);
    }
    for side in ["L", "R"] {
        let boxes: Vec<String> = [("fitted", &fitted)].into_iter().chain(their_skins.iter().map(|(name, skin)| (*name, skin))).map(|(name, skin)| {
            let [across, through] = outfit::palm_box(skin, &skeleton, side).unwrap().unwrap();
            format!("{name} {:.1} to {:.1} across, {:.1} to {:.1} through", across[0] * 100.0, across[1] * 100.0, through[0] * 100.0, through[1] * 100.0)
        }).collect();
        println!("    the {side} palm, cm from the middle of the knuckles' line: {}", boxes.join("; "));
    }
    // Left to itself, its palm is no narrower than the install's glove,
    // and not a fifth broader.
    if definition.arms_size.is_none() {
        let [their_broad, _] = measured(&their_skins[0].1);
        assert!((0.97..1.2).contains(&(broad / their_broad)), "the palm is {broad} broad for the install's {their_broad}");
    }
    println!("  each joint of the left hand, from the install's own:");
    for name in ["LPalm", "LDigit11", "LDigit12", "LDigit13", "LDigit21", "LDigit22", "LDigit23", "LDigit31", "LDigit32", "LDigit33", "LDigit41", "LDigit42", "LDigit43", "LDigit51", "LDigit52", "LDigit53", "LForearm1", "LUpperarm"] {
        let (at, their_at) = joint(name);
        println!("    {name:<10} {:5.1} cm   ({:6.3} {:6.3} {:6.3} for {:6.3} {:6.3} {:6.3})", apart(at, their_at) * 100.0, at[0], at[1], at[2], their_at[0], their_at[1], their_at[2]);
    }
    // Every joint of every finger is on the install's own.
    for side in ["L", "R"] {
        for finger in 1..=5 {
            for part in 1..=3 {
                let (at, their_at) = joint(&format!("{side}Digit{finger}{part}"));
                assert!(apart(at, their_at) < 1e-4, "{side}Digit{finger}{part} is {} from the install's", apart(at, their_at));
            }
        }
    }
    let at_rest = rig.pose(&vec![Move::STILL; rig.bones.len()]);
    assert!(at_rest.iter().all(|moved| *moved == Move::STILL));
    for mesh in &meshes {
        assert!(mesh.weights.iter().all(|shares| (shares.iter().sum::<f32>() - 1.0).abs() < 1e-4));
        assert!(mesh.submeshes[ARMS].len() == mesh.submeshes.iter().map(Vec::len).sum::<usize>() && !mesh.submeshes[ARMS].is_empty());
        println!("  a mesh of {} vertices, {} triangles", mesh.positions.len(), mesh.submeshes[ARMS].len() / 3);
    }
}

/// Every packed archive of the other game's is read: its table whole, and
/// the outfits' models of the item in it unpacked and checked against
/// their checksums. Skipped where the other game is not there.
#[test]
fn the_other_game_s_archives_are_read_whole() {
    let stalker = trp_data::xray::Stalker::locate();
    let Some(game) = stalker.game() else {
        eprintln!("skipped: the other game's folder is not known (local/anomaly_dir.txt)");
        return;
    };
    let began = std::time::Instant::now();
    let archives = stalker.archives();
    let files: usize = archives.iter().map(|archive| archive.len()).sum();
    println!("{} archives, {files} files, their tables read in {:.2} s", archives.len(), began.elapsed().as_secs_f32());
    assert_eq!(archives.len(), trp_data::xray_archive::archives_of(game).len(), "an archive's table could not be read");
    for name in ["meshes/dynamics/outfit/stalker_outfit.ogf", "meshes/dynamics/outfit/scientific_outfit.ogf"] {
        let archive = archives.iter().find(|archive| archive.has(name)).unwrap_or_else(|| panic!("{name} in no archive"));
        let bytes = archive.read(name).expect("read, unpacked and checked").expect("there");
        let model = trp_data::xray::read(&bytes).expect("a model");
        println!("  {name}: {} bytes, {} meshes, {} bones", bytes.len(), model.parts.len(), model.bones.len());
    }
}

/// Every outfit of the workspace's folder is an item of the database and
/// is built without failing: what of it the other game's files have,
/// loose or packed, is fitted (a body, arms for first person), and what
/// they have not is left to the character's own, with a note; its model
/// of the item, where it names one, is read with every picture. An outfit
/// made of the install's own clothing names bundles the install has.
/// Skipped with a note where the install is not there.
#[test]
fn every_outfit_of_the_folder_is_built() {
    use trp_data::outfit::{self, RestBone, ARMS, LEGS, UPPER};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    if db.outfits.is_empty() {
        eprintln!("skipped: no outfit's file is found from here ({:?})", db.custom_notes);
        return;
    }
    let stalker = trp_data::xray::Stalker::locate();
    let mut assets = Assets::new(install).expect("manifest");
    let mut skeleton: Vec<RestBone> = Vec::new();
    let mut waist = None;
    for key in ["tshirt_bear_turtleneck", "pants_bear_srvv", "bear_head_0"] {
        let part = trp_data::arms::load(&mut assets, &format!("assets/content/characters/character/prefabs/{key}.bundle")).expect("a piece of clothing");
        let mesh = read_mesh(&assets, part.mesh).expect("its mesh");
        for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
            if !skeleton.iter().any(|known| &known.path == path) {
                skeleton.push(RestBone { path: path.clone(), bind: *bind });
            }
        }
        if key.starts_with("pants") {
            waist = outfit::box_about(&mesh.positions);
        }
    }
    let own = trp_data::arms::load(&mut assets, "assets/content/hands/bear/bear_hands_skin.bundle").expect("the install's own arms");
    let their_skin = outfit::Skin::of([&read_mesh(&assets, own.mesh).expect("their mesh")], &own.bone_paths);
    let breadth = |skin: &outfit::Skin| -> f32 {
        ["L", "R"].iter().map(|side| outfit::palm_box(skin, &skeleton, side).expect("the hand's joints").expect("a palm")).map(|[across, _]| (across[1] - across[0]) * 0.5).sum()
    };
    let mut ids: Vec<&String> = db.outfits.keys().collect();
    ids.sort();
    for id in ids {
        let definition = &db.outfits[id];
        let item = db.item(&definition.id).expect("its item");
        assert_eq!(item.props["Slots"].as_array().map(Vec::len), Some(0), "{}", definition.name);
        let built = outfit::build(definition, &stalker, &skeleton, waist);
        println!("  {} ({}):", definition.name, definition.shown_name());
        for note in &built.notes {
            println!("    {note}");
        }
        let found = |name: &Option<String>| name.as_ref().is_some_and(|name| stalker.read(&format!("meshes/{name}")).is_ok());
        if let Some(world) = definition.world_visual.as_ref().filter(|_| stalker.known()) {
            let (bytes, found) = stalker.read(&format!("meshes/{world}")).expect("its model of the item");
            let model = trp_data::xray::read(&bytes).expect("its model of the item read");
            for part in &model.parts {
                let (bytes, _) = stalker.read(&format!("textures/{}.dds", part.texture)).expect("a picture of its model of the item");
                trp_data::xray::read_picture(&bytes).expect("a picture of its model of the item read");
            }
            println!("    its model of the item: {} meshes, every picture there; {found}", model.parts.len());
        }
        if found(&definition.visual) {
            assert!(built.has_body(), "{}: its body is among the other game's files and is not drawn", definition.name);
            let count = |list: usize| built.pieces.iter().filter(|piece| piece.picture.is_some()).map(|piece| piece.mesh.submeshes[list].len() / 3).sum::<usize>();
            println!("    its body: {} of {} meshes with their pictures, on {} bones; triangles under the waist {}, the arms' {}, the rest {}", built.pieces.iter().filter(|piece| piece.picture.is_some()).count(), built.pieces.len(), built.rig.bones.len(), count(LEGS), count(ARMS), count(UPPER));
            assert!(count(LEGS) > 300 && count(UPPER) > 300, "{}", definition.name);
        } else {
            assert!(!built.has_body());
        }
        if found(&definition.arms) {
            assert!(built.has_arms(), "{}: its arms are among the other game's files and are not drawn", definition.name);
            let arms = built.arms.as_ref().unwrap();
            let paths: Vec<String> = arms.rig.bones.iter().map(|bone| bone.path.clone()).collect();
            let (broad, their_broad) = (breadth(&outfit::Skin::of(arms.pieces.iter().map(|piece| &piece.mesh), &paths)), breadth(&their_skin));
            println!("    its arms for first person: {} meshes on {} bones, at {:.3} times their file's size; its palm {:.1} cm from edge to edge for the install's glove's {:.1}", arms.pieces.len(), arms.rig.bones.len(), arms.size, broad * 100.0, their_broad * 100.0);
            assert!((0.7..1.6).contains(&arms.size), "{}", arms.size);
            // Its wrists are the install's own.
            for bone in arms.rig.bones.iter().filter(|bone| bone.path.ends_with("Palm")) {
                let off = (0..3).map(|k| (bone.joint[k] - bone.their_joint[k]).powi(2)).sum::<f32>().sqrt();
                assert!(off < 1e-4, "{}: {} is {off} from the install's", definition.name, bone.path);
            }
            // Every finger's joints on the install's own.
            for bone in arms.rig.bones.iter().filter(|bone| bone.path.contains("Digit")) {
                let off = (0..3).map(|k| (bone.joint[k] - bone.their_joint[k]).powi(2)).sum::<f32>().sqrt();
                assert!(off < 1e-4, "{}: {} is {off} from the install's", definition.name, bone.path);
            }
        } else {
            assert!(!built.has_arms());
        }
        for (what, key) in [("top", &definition.their_top), ("hands", &definition.their_hands)] {
            if let Some(key) = key {
                let part = trp_data::arms::load(&mut assets, key).unwrap_or_else(|e| panic!("{}: its {what} {key}: {e:#}", definition.name));
                let mesh = read_mesh(&assets, part.mesh).expect("its mesh");
                println!("    the install's own {what}: {} vertices on {} bones", mesh.positions.len(), part.bone_paths.len());
            }
        }
    }
}

/// A weapon of one's own's pictures, compressed to be drawn with
/// (`custom::Picture::compressed`): how far each channel of the largest
/// level comes back from what was read, as a root mean square in 255 and
/// the share of pixels more than eight off. Passes with a note where no
/// weapon of one's own has its files here.
#[test]
fn a_weapon_of_one_s_own_s_pictures_compress_closely() {
    use trp_data::compress::{bc1_decoded, bc5_decoded};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let mut checked = 0;
    for template in db.custom.keys() {
        let Ok(weapon) = assemble(&mut assets, &db, template) else { continue };
        let Some(build) = weapon.custom.as_ref() else { continue };
        for material in &build.materials {
            let Ok(read) = trp_data::custom::pictures(material) else {
                eprintln!("skipped: the pictures of {} are not here", material.name);
                continue;
            };
            for (what, picture) in [("colour", read.colour), ("normal", read.normal), ("surface", read.surface)] {
                let Some(picture) = picture else { continue };
                let (width, height, channels) = (picture.width, picture.height, picture.channels);
                let first = width as usize * height as usize * channels;
                let original = picture.data[..first].to_vec();
                let packed = picture.compressed();
                let blocks = &packed.data[..trp_data::compress::level_bytes(width, height, if channels == 4 { 8 } else { 16 })];
                let back = if channels == 4 { bc1_decoded(width, height, blocks) } else { bc5_decoded(width, height, blocks) };
                let kept = if channels == 4 { 3 } else { 2 };
                let mut line = format!("{} {what} {width}x{height}:", material.name);
                for channel in 0..kept {
                    let (mut sum, mut off) = (0.0f64, 0usize);
                    for pixel in 0..(width * height) as usize {
                        let d = original[pixel * channels + channel] as f64 - back[pixel * channels + channel] as f64;
                        sum += d * d;
                        off += (d.abs() > 8.0) as usize;
                    }
                    let rms = (sum / (width * height) as f64).sqrt();
                    line += &format!(" channel {channel} {rms:.2} ({:.1}% over 8)", off as f64 * 100.0 / (width * height) as f64);
                    assert!(rms < 6.0, "{line}");
                }
                println!("{line}");
                checked += 1;
            }
        }
    }
    println!("{checked} pictures checked");
}

/// The inventory's health panel as the install's scene has it
/// (`docs/spec/health.md`, "The HEALTH tab"): its seven views and their
/// pieces named by the panel's own scripts, its figures, the left panel's
/// size on a screen 1920 by 1080, and its pictures read through trees
/// borrowed from a bundle; the views' glow hidden as their animator
/// stands, and the arrows' bob, by the scene's own clips.
#[test]
fn the_health_panel_is_read_as_the_scene_has_it() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let gear = trp_data::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::screen::HealthPanel::load(&install, &gear.file).expect("the health panel");
    let views = &health.views;
    assert_eq!(views.parts.len(), 7);
    let names: Vec<&str> = views.parts.iter().map(|view| health.panel.nodes[view.view].name.as_str()).collect();
    assert_eq!(names, ["Head", "Chest", "Stomach", "Left Arm", "Right Arm", "Left Leg", "Right Leg"]);
    for view in &views.parts {
        for piece in [view.value, view.bar, view.border, view.shape, view.broken, view.effects] {
            assert!(piece.is_some(), "{}: a piece of its view is not named", health.panel.nodes[view.view].name);
        }
    }
    for name in ["OverallHealth", "Poisoning", "Radiation", "BloodPressure", "Energy", "Hydration", "Temperature"] {
        assert!(views.parameter(name).and_then(|figure| figure.current).is_some(), "{name}");
    }
    assert!(views.parameter("Weight").is_none(), "the health tab has no weight");
    // The left side's width (a share of the screen and a size) shared by
    // its two children, with the space between them.
    let left = health.left_panel([1920.0, 1080.0]);
    assert!((left[0] - 604.163).abs() < 0.01 && (left[1] - 902.0).abs() < 0.01, "{left:?}");
    let [x, y, w, h] = health.gear_in(left);
    assert!((x - (left[0] - 494.0) / 2.0).abs() < 0.01 && (y - 100.0).abs() < 0.01 && w == 494.0 && h == 650.0, "{:?}", [x, y, w, h]);
    // The chest's view where its anchors and offset put it.
    let chest = views.parts[1].view;
    let [cx, cy, cw, ch] = health.panel.rect(chest, left);
    assert!((cx - (left[0] * 0.5 - 50.9)).abs() < 0.01 && (cy - (902.0 - (0.887_820_5 * 902.0 - 162.948_12))).abs() < 0.01 && cw == 130.0 && ch == 19.0, "{:?}", [cx, cy, cw, ch]);
    // The silhouette's picture and its texture.
    let mut pictures = trp_data::screen::ScenePictures::new(&install).expect("the trees to read pictures by");
    let silhouette = health.panel.find("Silhouette/Character Silhouette").expect("the silhouette");
    let (file, id) = health.panel.nodes[silhouette].image.and_then(|image| image.sprite).expect("its sprite");
    let name = health.panel.externals[file as usize - 1].clone();
    let cut = pictures.sprite(&name, id).expect("the sprite");
    let texture = pictures.texture(&cut.texture.0, cut.texture.1).expect("its texture");
    assert!(texture.width >= cut.rect[2] as u32 && texture.height >= cut.rect[3] as u32 && !texture.data.is_empty(), "{cut:?}");
    let covers = cut.covers();
    assert!(covers.iter().all(|c| (0.0..=1.0).contains(c)) && covers[0] < covers[2] && covers[1] < covers[3], "{covers:?}");
    // The scene's clips: the views' glow standing, and the arrows' bob.
    let shared: Vec<String> = health.panel.externals.iter().filter(|name| name.starts_with("sharedassets")).cloned().collect();
    let clips = trp_data::screen::scene_clips(&install, &mut pictures, &shared).expect("the scene's clips");
    let bound = |clip: &Clip, path: &str, property: &str| clip.others.iter().find(|b| b.path == path_hash(path) && b.attribute == path_hash(property)).map(|b| b.curve);
    let standing = clips.iter().find(|(_, _, clip)| clip.name == "Normal" && bound(clip, "BackGlowImage", "m_Color.a").is_some()).expect("the views' standing clip");
    let mut values = Vec::new();
    standing.2.sample(0.0, &mut values);
    assert_eq!(values[bound(&standing.2, "BackGlowImage", "m_Color.a").unwrap()], 0.0, "the glow is hidden standing");
    let bob = clips.iter().find(|(_, _, clip)| clip.name == "BuffPaneActive").expect("the arrows' bob");
    assert!(bob.2.looping && bound(&bob.2, "", "m_AnchoredPosition.y").is_some(), "{}", bob.2.name);
}

/// The GEAR tab's strip of figures as its row lays it out while the game
/// runs (`HorizontalLayoutGroup`: four panels asking 208 each in a strip
/// of 532.08 on a 16:9 screen, so 133.02 each from 36 units in), what its
/// script names, and the game's own fonts kept whole in `resources.assets`
/// (the panel's texts drawn in Bender).
#[test]
fn the_gear_tab_s_strip_and_the_game_s_fonts_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let gear = trp_data::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::screen::HealthPanel::load(&install, &gear.file).expect("the health panel");
    let strip = health.strip.as_ref().expect("the strip of figures");
    let left = health.left_panel([1920.0, 1080.0]);
    let names = ["Weight", "Overall Health", "Hydration", "Energy"];
    for (n, name) in names.iter().enumerate() {
        let index = strip.find(name).unwrap_or_else(|| panic!("{name}"));
        let [x, y, w, h] = strip.placed_in(index, left).rect;
        let width = (0.620_111_8 * left[0] + 157.430_18) / 4.0;
        assert!((w - width).abs() < 0.05 && (x - (left[0] * 0.189_944_1 - 157.430_18 / 2.0 + n as f32 * width)).abs() < 0.05, "{name}: {:?}", [x, y, w, h]);
        assert!((y - (902.0 - 20.6 - 46.8)).abs() < 0.05 && (h - 46.8).abs() < 0.01, "{name}: {:?}", [x, y, w, h]);
    }
    for name in ["Weight", "OverallHealth", "Hydration", "Energy"] {
        assert!(trp_data::screen::figure_named(&health.strip_figures, name).and_then(|f| f.current).is_some(), "{name}");
    }
    let weight = trp_data::screen::figure_named(&health.strip_figures, "Weight").unwrap();
    assert!(weight.alternative[3] > 0.0, "the weight has a colour of its own between its limits");
    // The fonts: Bender kept whole, and the panel's font assets made from it.
    let fonts = trp_data::screen::build_fonts(&install, "resources.assets").expect("the build's fonts");
    let bender = fonts.iter().find(|(name, _)| name == "Jovanny Lemonad - Bender").expect("Bender");
    assert_eq!(&bender.1[..4], b"OTTO");
    let header = health.panel.find("Chest/Header/Text").and_then(|text| health.panel.nodes[text].text.clone()).expect("a header's text");
    let file = health.panel.externals[header.font.0 as usize - 1].clone();
    let asset = trp_data::screen::script_name(&install, &file, header.font.1).expect("its font asset's name");
    assert_eq!(trp_data::screen::font_for(&asset, &fonts).map(|(name, _)| name.as_str()), Some("Jovanny Lemonad - Bender"), "{asset}");
}

/// The game's GEAR tab as `TARKOV UI` draws it (`docs/spec/inventory.md`,
/// "The game's own look"): the equipment panel's eleven slot views named
/// by its script with their borders, fills and pictures; the views of a
/// thing in a slot and in a grid read from the build's Resources; the
/// containers' column with its row template, its grid view and its rows'
/// spacing; a container's own layout of its grids; the slots' pictures;
/// and the database's colours and layouts of the items.
#[test]
fn the_game_s_gear_tab_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let gear = trp_data::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::screen::HealthPanel::load(&install, &gear.file).expect("the left panel");
    assert_eq!(health.slot_views.len(), 11, "{:?}", health.slot_views.iter().map(|(slot, _)| *slot).collect::<Vec<_>>());
    for (slot, parts) in &health.slot_views {
        for part in [parts.full_border, parts.back_image, parts.background, parts.empty_border, parts.header] {
            assert!(part.is_some(), "{slot}: a part of its view is not named");
        }
    }
    let gear_panel = health.gear_panel.as_ref().expect("the equipment panel's objects");
    assert!(gear_panel.find("CharacterGear").is_some(), "the character's outline behind the slots");
    // The views of a thing.
    let items = trp_data::screen::ItemViews::load(&install).expect("the views of a thing");
    let slot = items.slot.as_ref().expect("a slot's view");
    let grid = items.grid.as_ref().expect("a grid's view");
    assert!(slot.find("Info Panel/Name").is_some() && grid.find("Info Panel/Caption").is_some());
    let caption = grid.find("Info Panel/Caption").and_then(|index| grid.nodes[index].text.clone()).expect("the caption's text");
    assert_eq!(caption.size, 11.0);
    // The containers' column.
    let containers = trp_data::screen::ContainerViews::load(&install, &gear.file).expect("the containers' column");
    let column = containers.column.expect("its column of rows");
    assert!(column.vertical && column.spacing == 45.0 && column.padding[2] == 120.0, "{column:?}");
    assert!(containers.header.is_some() && containers.slot_panel.is_some());
    let row = containers.grids_row.expect("the default row of grids");
    assert_eq!(row.spacing, 6.0);
    let view = containers.grid.as_ref().expect("a grid's view");
    assert!(view.find("Background Tile").is_some() && view.find("Border").is_some());
    // A container's own layout: the AVS's twelve grids in their order.
    let (_, grids) = trp_data::screen::rig_layout(&install, "avs").expect("the AVS's layout");
    assert_eq!(grids.len(), 12);
    let layouts = trp_data::screen::rig_layouts(&install, &["avs".to_owned()]);
    assert_eq!(layouts["avs"].1[0], [0.0, -1.0, 64.0, 127.0]);
    assert!(trp_data::screen::resource_sprite(&install, "Slots/Equipment/TacticalVest").is_some());
    // The database's colours and layouts.
    let db = Database::load(&install).expect("database");
    assert!(db.items.values().filter(|item| item.background_color() != "default").count() > 100);
    assert!(db.items.values().filter(|item| !item.grid_layout_name().is_empty()).count() > 20);
    // The column's scroll view and its bar, by the pointers of their
    // scripts: a notch of the wheel 63 units, stopped at its ends, the bar
    // hidden while everything shows, from the bottom up, its handle tinted.
    let scrolling = containers.scrolling.expect("the column's scroll view");
    assert!(scrolling.vertical && !scrolling.across && scrolling.clamped && scrolling.hides && scrolling.sensitivity == 63.0, "{scrolling:?}");
    let at = |index: Option<usize>| index.map(|index| containers.panel.nodes[index].path.as_str());
    assert_eq!(at(scrolling.view), Some("Scrollview Parent/Containers Scrollview"));
    assert_eq!(at(scrolling.content), Some("Scrollview Parent/Containers Scrollview/Content"));
    let bar = scrolling.bar.expect("its bar");
    assert!(bar.bottom_up && bar.tinted == bar.handle, "{bar:?}");
    assert_eq!(at(bar.handle), Some("Scrollbar/Sliding Area/Handle"));
    let tints = bar.tints.expect("the bar's tints");
    assert!((tints.pressed[0] - 0.784).abs() < 1e-3 && (tints.highlighted[0] - 0.961).abs() < 1e-3 && (tints.fade - 0.1).abs() < 1e-6, "{tints:?}");
    // The handle up and down its sliding area, 10 in from the bar's ends,
    // 8 longer at each end than its share, and 6 wide.
    let handle = bar.handle_in(&containers.panel, [604.2, 902.0], 1.0, 0.5).expect("the handle's place");
    let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.05);
    assert!(near(handle, [596.2, 2.0, 6.0, 457.0]), "{handle:?}");
    // The screen's places at 1920 by 1080: the left side, the right's
    // panel, the left panel's ground and the left side's frame, whose
    // texture is sampled at its nearest pixel (its middle is clear).
    let screen = [1920.0, 1080.0];
    assert!(near(health.left_side_in(screen), [11.6, 48.0, 1213.3, 902.0]), "{:?}", health.left_side_in(screen));
    assert!(near(health.stash_in(screen).expect("the right's panel"), [1228.0, 75.0, 680.0, 873.0]), "{:?}", health.stash_in(screen));
    assert!(health.left_ground.is_some_and(|ground| (ground[3] - 0.498).abs() < 1e-3));
    let border = health.border.as_ref().expect("the left side's frame");
    let (outside, sprite) = border.nodes[0].image.and_then(|image| image.sprite).expect("the frame's sprite");
    let mut pictures = trp_data::screen::ScenePictures::new(&install).expect("the screens' pictures");
    let cut = pictures.sprite(&border.file_of(outside).expect("its file"), sprite).expect("the frame's sprite");
    assert!(pictures.texture(&cut.texture.0, cut.texture.1).expect("its texture").point);
    // The bar of tabs: eight tabs in the game's order, 26 over one another,
    // each its words' sample and 85 more; the widest is ACHIEVEMENTS.
    let mut bar = trp_data::screen::TabBar::load(&install, &gear.file).expect("the bar of tabs");
    assert_eq!(bar.gap, -26.0);
    let keys: Vec<&str> = bar.tabs.iter().map(|tab| tab.key.as_str()).collect();
    assert_eq!(keys, ["OVERALL", "GEAR", "HEALTH_TAB", "SKILLS", "MAP", "TASKS", "ACHIEVEMENTS", "Prestige"]);
    assert!(bar.tabs.iter().enumerate().all(|(order, tab)| tab.order == order as i32 && tab.width_adjustment == 85.0 && tab.normal.is_some() && tab.selected.is_some() && tab.hover.is_some()));
    // TextMeshPro's width of a text: the Bender asset's advances at its
    // sampling size of 48 and 2 hundredths of the size after every letter
    // but the last, as the scene keeps the tabs' samples (65.59).
    let sample = bar.panel.nodes[bar.tabs[0].size_definers[0]].text.clone().expect("the tab's sample");
    let font = trp_data::screen::TmpFont::read(&install, &bar.panel.file_of(sample.font.0).expect("its font's file"), sample.font.1).expect("Bender's asset");
    assert_eq!((font.point_size, font.normal_spacing), (48.0, 2.0));
    assert!(font.bold.is_some(), "its bold typeface");
    assert_eq!(font.width("OVERALL", 16.0, 0.0, false), 65.59);
    let texts = trp_data::inventory::texts(&install, "en", &keys).expect("the install's words");
    let say = |key: &str| texts.get(&key.to_lowercase()).cloned();
    let width = bar.width(&install, &say);
    assert!((width - (font.width("ACHIEVEMENTS", 16.0, 0.0, false) + 85.0)).abs() < 1e-3, "{width}");
    bar.lay_out(width);
    let places: Vec<f32> = bar.tabs.iter().map(|tab| bar.panel.placed_in(tab.object, screen).rect[0]).collect();
    assert!(places.iter().enumerate().all(|(order, x)| (x - (10.0 + order as f32 * (width - 26.0))).abs() < 1e-3), "{places:?}");
    // A line's height as TextMeshPro has it: Bender's ascent to descent at
    // 16, 17.61, which with its padding (4 and 6) is the 27.61 the scene
    // keeps for the corpse panel's box of a container's name.
    assert_eq!(font.height(16.0), 17.61);
    // The panel for a container opened: what its script names, its 18
    // filters in a column 34 apart (a vertical group laid out), its scroll
    // view elastic with no elasticity (back at once) and its bar.
    let mut stash = trp_data::screen::StashViews::load(&install, &gear.file).expect("the panel for a container opened");
    let path = |index: Option<usize>| index.map(|index| stash.panel.nodes[index].path.clone());
    assert_eq!(path(stash.name).as_deref(), Some("Simple Panel/Container Name/Text"));
    assert_eq!(path(stash.grids).as_deref(), Some("Simple Panel/Mask/Scroll Area/Content"));
    assert_eq!(stash.filter_tabs.len(), 18);
    let size = [680.0, 873.0];
    let tops: Vec<f32> = stash.filter_tabs.iter().filter_map(|tab| tab.normal).map(|normal| stash.panel.rect(normal, size)[1]).collect();
    assert!(tops.windows(2).all(|pair| (pair[1] - pair[0] - 34.0).abs() < 1e-3), "{tops:?}");
    let scrolling = stash.scrolling.expect("its scrolling");
    assert!(scrolling.elastic && scrolling.elasticity == 0.0 && scrolling.sensitivity == 63.0 && scrolling.bar.is_some_and(|bar| bar.bottom_up), "{scrolling:?}");
    // The quick-use panel: twelve views in the game's order of their
    // bindings, each with what its script names, 70 apart and 10 more
    // between the two groups, the panel where the game's screen has it.
    let mut quick = trp_data::screen::QuickViews::load(&install, &gear.file).expect("the quick-use panel");
    let keys: Vec<&str> = quick.slots.iter().map(|slot| slot.key.as_str()).collect();
    assert_eq!(keys, ["Grenade", "ItemV", "Item1", "Item2", "Item3", "Item4", "Item5", "Item6", "Item7", "Item8", "Item9", "Item10"]);
    assert!(quick.slots.iter().all(|slot| slot.install.is_some() && slot.hot_key.is_some() && slot.caption.is_some() && slot.border.is_some()));
    for slot in quick.slots.clone() {
        quick.panel.nodes[slot.object].active = true;
    }
    let place = quick.quick_in(screen);
    assert!(near(place, [558.5, 935.0, 854.2, 115.0]), "{place:?}");
    let lefts: Vec<f32> = quick.slots.iter().map(|slot| quick.panel.rect(slot.object, [place[2], place[3]])[0]).collect();
    let expected = [0.0, 70.0, 140.0, 210.0, 280.0, 354.0, 424.0, 494.0, 564.0, 634.0, 704.0, 774.0];
    assert!(lefts.iter().zip(expected).all(|(left, wanted)| (left - wanted).abs() < 1e-3), "{lefts:?}");
    // Its header on, its row sized to it (`ContentSizeFitter`): above the
    // panel's top, as the game shows it.
    let header = stash.header.expect("its header");
    stash.panel.nodes[header].active = true;
    let [_, panel_top, ..] = stash.panel.rect(stash.simple, size);
    let [_, top, _, height] = stash.panel.rect(header, size);
    assert!(height == 20.0 && top + height <= panel_top, "{top} {height} {panel_top}");
}

/// The things' sounds kept as Vorbis (`sound.md`, "Vorbis"): each names a
/// setup header the game's engine holds, and decodes to its own count of
/// frames. And an AI-2's hands have their own sound player, whose sounds
/// its animator's marks name (`hands_sounds`).
#[test]
fn vorbis_clips_decode_and_an_item_s_hands_have_their_sounds() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let clips = trp_data::sound::item_vorbis(&install).expect("the things' Vorbis clips");
    assert!(clips.len() >= 10, "{} Vorbis clips", clips.len());
    let mut setups = trp_data::sound::VorbisSetups::new(&install);
    for (name, sample) in &clips {
        let setup = setups.get(sample.setup_crc).unwrap_or_else(|e| panic!("{name}: {e:#}")).to_vec();
        let samples = trp_data::sound::decode_vorbis(sample, &setup).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        assert_eq!(samples.len() as u64, sample.frames * sample.channels as u64, "{name}: frames decoded");
        assert!(samples.iter().any(|s| s.unsigned_abs() > 1000), "{name}: silent");
    }
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let path = db.items["5755356824597772cb798962"].props["UsePrefab"]["path"].as_str().expect("the AI-2's hands").to_owned();
    let rig = trp_data::weapon::assemble_prefab(&mut assets, &path).expect("the AI-2's rig");
    let sounds = trp_data::sound::hands_sounds(&assets, &rig, &mut setups, true).expect("read").expect("a sound player");
    assert_eq!(sounds.player, "BaseSoundPlayer");
    assert!(sounds.shots.is_none(), "an item's hands have no shots");
    for mark in ["Draw", "Open", "Take", "CapRemove", "Inject", "Discard", "Holster"] {
        let element = sounds.for_mark(mark).unwrap_or_else(|| panic!("no sound for the mark {mark}"));
        assert!(!element.clips.is_empty() && element.clips.iter().all(|clip| !clip.samples.is_empty()), "{mark}");
    }
    println!("{} Vorbis clips decoded; the AI-2's {}: {} sounds", clips.len(), sounds.player, sounds.additional.len());
}

/// A weapon's banks of shots (`WeaponSoundPlayer`): the AKM's fire a loop
/// of sixteen beats at its rate of 600 a minute and a tail, with silenced
/// banks; the SV-98's each shot whole; the MP-43's both barrels' own bank.
#[test]
fn a_weapon_s_shots_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut setups = trp_data::sound::VorbisSetups::new(&install);
    let mut assets = Assets::new(install).expect("manifest");
    let mut shots_of = |template: &str| {
        let weapon = trp_data::weapon::assemble(&mut assets, &db, template).expect("the weapon");
        let sounds = trp_data::sound::hands_sounds(&assets, &weapon, &mut setups, true).expect("read").expect("a sound player");
        assert_eq!(sounds.player, "WeaponSoundPlayer", "{template}");
        sounds.shots.expect("its shots")
    };
    let close = |shots: &trp_data::sound::ShotSounds, bank: &str| -> f32 {
        let bank = shots.bank(bank).unwrap_or_else(|| panic!("no {bank}"));
        let clip = shots.clips[bank.lists[0][0]].1.as_ref().expect("the close clip read");
        clip.samples.len() as f32 / clip.channels as f32 / clip.frequency as f32
    };
    let akm = shots_of("59d6088586f774275f37482f");
    assert!(!akm.non_auto);
    let body = akm.bank("Body").expect("the AKM's body");
    // Sixteen beats of 60 / 600 s.
    assert!((body.clip_length - 1.6).abs() < 1e-4, "{}", body.clip_length);
    assert!((close(&akm, "Body") - 1.6).abs() < 1e-3);
    assert!(close(&akm, "Tail") > 3.0);
    assert!(akm.bank("BodySilenced").is_some() && akm.bank("Doublet").is_none());
    assert_eq!(body.blend.as_deref(), Some(&[5.0, 30.0, 55.0, 100.0][..]));
    let sv98 = shots_of("55801eed4bdc2d89578b4588");
    assert!(sv98.non_auto && sv98.bank("Tail").is_none());
    let mp43 = shots_of("5580223e4bdc2d1c128b457f");
    assert!(mp43.non_auto);
    assert_eq!(mp43.bank("Doublet").expect("both barrels").lists[0].len(), 2);
    let silenced = trp_data::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM").silenced;
    assert!(!silenced, "the stock AKM has no silencer");
}

/// A helmet's face shield and hinge (`ObjectsFactory.AttachMods`,
/// `HelmetDress`): the ZSh-1-2M's shield hangs on its `mod_equipment`,
/// hidden until fitted; its armour plates have no model and are not hung;
/// its hinge is the object its dress names, with the turns and curves it
/// keeps.
#[test]
fn a_helmet_s_face_shield_and_hinge_are_read() {
    use trp_data::dress::{head_model, load, Under, Worn};
    const ZSH: &str = "5aa7e454e5b5b0214e506fa2";
    const SHIELD: &str = "5aa7e3abe5b5b000171d064d";
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let key = db.items[ZSH].prefab_key().expect("its prefab").to_owned();
    let Worn::Rigid(model) = load(&mut assets, &key, Under::default()).expect("the helmet") else { panic!("a helmet is a model") };
    let (model, parts, hinges) = head_model(&mut assets, &db, ZSH, model, &|_| true);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].path, vec![("mod_equipment".to_owned(), SHIELD.to_owned())]);
    let shield = &model.nodes[parts[0].root];
    assert!(!shield.active);
    assert_eq!(hinges.len(), 1, "the helmet's, and none on its shield");
    let hinge = &hinges[0];
    assert!(hinge.owner.is_empty(), "the helmet's own");
    let on = model.nodes[parts[0].root].parent.expect("hung");
    println!(
        "hinge {} (rest {:?}), the shield on {}; off {:?} on {:?}; forward {:?}; backward {:?}",
        model.nodes[hinge.node].name, model.nodes[hinge.node].rotation, model.nodes[on].name, hinge.off, hinge.on, hinge.forward.keys, hinge.backward.keys
    );
    assert!(hinge.forward.keys.len() >= 2 && hinge.backward.keys.len() >= 2);

    // The Ops-Core FAST's visor carries its own hinge (its bundle's
    // `HelmetDress`), the helmet none: it is read on the visor's model,
    // below where the visor hangs.
    const FAST: &str = "5a154d5cfcdbcb001a3b00da";
    const VISOR: &str = "5a16b672fcdbcb001912fa83";
    let key = db.items[FAST].prefab_key().expect("its prefab").to_owned();
    let Worn::Rigid(model) = load(&mut assets, &key, Under::default()).expect("the FAST") else { panic!("a helmet is a model") };
    let (model, parts, hinges) = head_model(&mut assets, &db, FAST, model, &|part| part == VISOR);
    let visor = parts.iter().find(|part| part.path.last().is_some_and(|(_, of)| of == VISOR)).expect("the visor hung");
    assert_eq!(hinges.len(), 1, "{hinges:?}");
    assert_eq!(hinges[0].owner, visor.path, "the visor's own");
    let mut at = Some(hinges[0].node);
    while at.is_some_and(|node| node != visor.root) {
        at = model.nodes[at.unwrap()].parent;
    }
    assert_eq!(at, Some(visor.root), "the hinge is in the visor's model");
    println!("the FAST's visor: its hinge {} on {:?}", model.nodes[hinges[0].node].name, visor.path);
}

/// The wish list's menu and icons (`InteractionButtonsContainer.
/// SubMenuTemplate`, `StaticIcons.WishlistSprites`, `WishlistSpritesOutline`):
/// the item's menu's menu of lines found with its three templates, and the
/// five groups' icons twice over, the outlined ones others.
#[test]
fn the_wish_list_s_menu_and_icons_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let sub = trp_data::screen::context_submenu(&install).expect("the menu of lines");
    for template in ["Button Template", "Wishlist Template", "Dispose Template"] {
        assert!(sub.find(&format!("InteractionButtonsContainer/{template}")).is_some(), "{template}");
    }
    let [plain, outlined] = trp_data::screen::wishlist_icons(&install).expect("the icons");
    println!("plain {plain:?}\noutlined {outlined:?}");
    assert_eq!((plain.len(), outlined.len()), (5, 5));
    assert!(plain.iter().zip(&outlined).all(|((a, at), (b, bt))| a == b && at != bt));
}

/// A spent case's figures (`ShellExtractionData`, `shellport`,
/// `AmmoPoolObject._caliber`): the AKM throws its cases by its shot
/// settings from a shell port of its own; its 5.45 BP round is of the
/// 5.56 mm class.
#[test]
fn a_weapon_s_case_throw_and_a_round_s_calibre_are_read() {
    let Some((mut assets, akm)) = akm() else { return };
    let extraction = akm.extraction.expect("the AKM's shell extraction");
    let port = akm.shell_port.expect("the AKM's shell port");
    println!("extraction {extraction:?}, shell port {} at {:?}", akm.hierarchy.nodes[port].name, akm.hierarchy.nodes[port].translation);
    assert!(extraction.force.iter().any(|range| range[0] != 0.0 || range[1] != 0.0));
    let files = assets.load("assets/content/items/ammo/patrons/patron_545x39_bp.bundle").expect("the round");
    assert_eq!(trp_data::weapon::round_caliber(&assets, &files), Some(2));
}

/// The eyes' burn's figures and pictures (`Cam2`'s `EyeBurn`): three spots
/// and a trail, 0.2 to 1.1 in size, 100 s at most.
#[test]
fn the_eyes_burn_is_read() {
    let Ok(install) = Install::locate() else { return };
    let burn = trp_data::eye_burn::read(&install).expect("the eyes' burn");
    println!(
        "spots {:?}, trail {:?}, scale {:?}, {} s, a trail spot each {} of the turn shifted {}, at most {}; trail curve {:?}; scale curve {:?}",
        burn.spots.iter().map(|t| (t.width, t.height, t.format)).collect::<Vec<_>>(),
        burn.trail.iter().map(|t| (t.width, t.height, t.format)).collect::<Vec<_>>(),
        burn.scale,
        burn.max_time,
        burn.trail_every,
        burn.trail_shift,
        burn.max_trail,
        burn.trail_curve.keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>(),
        burn.scale_curve.keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>()
    );
    assert_eq!((burn.spots.len(), burn.trail.len()), (3, 1));
    assert_eq!(burn.scale, [0.2, 1.1]);
    assert_eq!(burn.max_time, 100.0);
}

/// Health's screen effects' figures and near death's picture are read from
/// the camera and its effects prefab.
#[test]
fn the_camera_effects_are_read() {
    let Ok(install) = Install::locate() else { return };
    let read = trp_data::camera_effects::read(&install).expect("the camera's effects");
    let h = &read.health;
    let keys = |c: &trp_data::curve::Curve| c.keys.iter().map(|k| (k.time, k.value, k.in_slope, k.out_slope)).collect::<Vec<_>>();
    println!(
        "hit: {} s over {:?}, squares {:?}, {} passes; radial {} at {:?}; vignette at {:?}, sharpness {}, darkness {}, flicker {:?}; near death {} ({}x{} {:?})",
        h.hit_time,
        keys(&h.hit_curve),
        h.hit_squares,
        h.hit_passes,
        h.radial_most,
        h.radial_center,
        h.vignette_center,
        h.vignette_sharpness,
        h.vignette_darkness,
        keys(&h.flicker),
        h.near_death_most,
        read.near_death.width,
        read.near_death.height,
        read.near_death.format
    );
    assert_eq!((h.hit_time, h.hit_squares, h.hit_passes), (0.65, [1024, 512], 3));
    // The sharpening and its desaturation, as the recorded camera has them
    // (`courses6\0-medical`'s first frame: 0.6, 0.406, on; no offset; the
    // ring from 0.325 to 1, its falloff 0.425).
    let level = trp_data::tod::TimeOfDayLevel::customs(&install).expect("Customs");
    let file = level.file_of(level.settings.camera).expect("the camera's file");
    let s = &trp_data::camera_effects::SharpenData::of_camera(&install, &file, level.settings.camera.1).expect("the level camera's sharpening");
    println!("sharpen {} clamp {} desaturate {}; ramp {}x{} {:?}; offset {:?}; radius {:?} falloff {}", s.strength, s.clamp, s.desaturate, s.ramp.width, s.ramp.height, s.ramp.format, s.ramp_offset, s.min_max_radius, s.radius_falloff);
    assert_eq!((s.clamp, s.desaturate), (0.406, true));
    assert_eq!((s.ramp_offset, s.min_max_radius, s.radius_falloff), ([0.0; 3], [0.325, 1.0], 0.425));
    assert_eq!((h.radial_most, h.vignette_darkness, h.near_death_most), (0.035, 74.9, 1.0));
    assert_eq!((read.near_death.width, read.near_death.height), (1920, 1080));
    // Near death's picture decoded: its middle and its corner.
    let level = trp_data::unity::texture::mips(&read.near_death).expect("its level")[0];
    let pixels = trp_data::unity::texture::decode_rgba(&read.near_death, level).expect("decoded");
    let at = |x: usize, y: usize| &pixels[(y * 1920 + x) * 4..][..4];
    println!(
        "near death: middle {:?}, a corner {:?}, the left edge's middle {:?}, the first rows' middle {:?}, the last rows' {:?}, sRGB {}",
        at(960, 540),
        at(0, 0),
        at(0, 540),
        at(960, 100),
        at(960, 980),
        read.near_death.srgb
    );
    assert_eq!(read.eye_burn.spots.len(), 3);
    let b = &read.blood;
    println!("blood: {b:?}; the drop {} ({}x{} format {} sRGB {})", read.blood_drop.name, read.blood_drop.width, read.blood_drop.height, read.blood_drop.format, read.blood_drop.srgb);
    assert_eq!((b.refraction, b.downsampling, b.max_time, b.initial_drops, b.dimension), (0.046, 3, 8.0, 4, 11));
    assert_eq!(b.levels[0], [48.0 / 255.0, 48.0 / 255.0, 48.0 / 255.0, 1.0]);
    let level = trp_data::unity::texture::mips(&read.blood_drop).expect("its level")[0];
    let pixels = trp_data::unity::texture::decode_rgba(&read.blood_drop, level).expect("decoded");
    let mut stats = [(255u8, 0u8, 0u64); 4];
    for px in pixels.chunks_exact(4) {
        for c in 0..4 {
            stats[c] = (stats[c].0.min(px[c]), stats[c].1.max(px[c]), stats[c].2 + px[c] as u64);
        }
    }
    let n = (pixels.len() / 4) as u64;
    println!("the drop's channels (least, most, mean): {:?}; its middle {:?}", stats.map(|(a, b, s)| (a, b, s / n)), &pixels[(128 * 256 + 128) * 4..][..4]);
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("the hard settings");
    println!("bleeding centres {:?}", hard.vec3s("BleedingCenters").expect("the bleeding centres"));
}

/// A contusion's ringing (`Sounds.TinnitusSound`) is read whole.
#[test]
fn the_tinnitus_sound_is_read() {
    let Ok(install) = Install::locate() else { return };
    let clip = trp_data::sound::tinnitus_sound(&install).expect("the tinnitus");
    println!("{}: {} channels at {} Hz, {:.2} s", clip.name, clip.channels, clip.frequency, clip.samples.len() as f32 / clip.channels as f32 / clip.frequency as f32);
    assert!(clip.samples.len() > clip.frequency as usize / 2);
}

/// Every grenade's blast's sound (`ExplosionEffectType`'s effect's bank):
/// the F-1's `Grenade_new` has one, with clips.
#[test]
fn the_grenades_blasts_sounds_are_read() {
    let Ok(install) = Install::locate() else { return };
    let db = trp_data::Database::load(&install).expect("database");
    let mut names: Vec<String> = db.items.values().filter_map(|item| item.props.get("ExplosionEffectType")?.as_str().map(str::to_owned)).filter(|name| !name.is_empty()).collect();
    names.sort();
    names.dedup();
    let sounds = trp_data::sound::effect_sounds(&install, &names, false).expect("the effects' sounds");
    for (name, at) in &sounds.effects {
        match at.map(|at| &sounds.banks[at]) {
            Some(bank) => println!(
                "{name}: {} volume {} -{} pitch {} ±{} rolloff {} source {} physical {} limited {} blend {:?} clips {:?}",
                bank.name,
                bank.base_volume,
                bank.delta_volume,
                bank.base_pitch,
                bank.delta_pitch,
                bank.rolloff,
                bank.source_type,
                bank.physical,
                bank.limited,
                bank.blend,
                bank.lists.iter().map(|list| list.iter().map(|&clip| sounds.clips[clip].0.as_str()).collect::<Vec<_>>()).collect::<Vec<_>>()
            ),
            None => println!("{name}: no sound"),
        }
    }
    let f1 = sounds.bank("Grenade_new").expect("the F-1's blast's bank");
    assert!(f1.lists.iter().any(|list| !list.is_empty()));
}

/// The spent cases' sounds: a bank for every calibre's class and surface,
/// each with clips.
#[test]
fn the_spent_cases_sounds_are_read() {
    let Ok(install) = Install::locate() else { return };
    let sounds = trp_data::sound::shell_sounds(&install, false).expect("the spent cases' sounds");
    for (c, caliber) in trp_data::sound::SHELL_CALIBERS.iter().enumerate() {
        for (s, surface) in trp_data::sound::SHELL_SURFACES.iter().enumerate() {
            let bank = &sounds.banks[sounds.bank(c as u8, s).unwrap_or_else(|| panic!("no bank for {caliber} on {surface}"))];
            println!(
                "{caliber} {surface}: {} volume {} -{} pitch {} ±{} rolloff {} source {} blend {:?} clips {:?}",
                bank.name,
                bank.base_volume,
                bank.delta_volume,
                bank.base_pitch,
                bank.delta_pitch,
                bank.rolloff,
                bank.source_type,
                bank.blend,
                bank.lists.iter().map(Vec::len).collect::<Vec<_>>()
            );
            assert!(bank.lists.iter().any(|list| !list.is_empty()));
        }
    }
}

/// The ballistic presets (`ColliderPresets`) are read, each by its
/// material's number, and the terrain's give what Customs' ground was
/// recorded with (`courses3`, `2-surfaces`: soil, asphalt, gravel and tall
/// grass, which the terrain takes from the presets).
#[test]
fn the_ballistic_presets_are_read() {
    let Ok(install) = Install::locate() else { return };
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("the hard settings");
    let presets = hard.ballistic_presets().expect("the presets");
    for (at, p) in presets.iter().enumerate() {
        println!("{at:2} {:16} material {:2} {:?} by scale {} ({} keys)", p.name, p.material, p.figures, p.depend_on_scale, p.scale_curve.keys.len());
    }
    let recorded: [(i32, [f32; 6]); 4] = [
        (22, [100.0, 0.0, 0.125, 0.06, 0.14, 0.013]),
        (1, [100.0, 0.0, 0.32, 0.17, 0.18, 0.038]),
        (15, [69.0, 0.35, 0.31, 0.1, 0.11, 0.027]),
        (13, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ];
    for (material, figures) in recorded {
        let preset = &presets[material as usize];
        assert_eq!(preset.material, material, "{}", preset.name);
        for k in 0..6 {
            assert!((preset.figures[k] - figures[k]).abs() < 1e-4, "{}: {:?}", preset.name, preset.figures);
        }
    }
    println!("decal shift {}", hard.float("DECAL_SHIFT").expect("the decal shift"));
}

/// The impacts' entries by material and what each plays (ballistics spec,
/// "What a hit shows and sounds"), the bullets' decals, and the effects'
/// sounds for them with the ricochet's.
#[test]
fn the_impacts_are_read() {
    let Ok(install) = Install::locate() else { return };
    let mut assets = trp_data::unity::Assets::new(install.clone()).expect("the assets");
    let entries = trp_data::particles::material_entries(&mut assets).expect("the entries");
    for e in &entries {
        let read = trp_data::particles::effect_entry(&mut assets, &e.name).expect("the entry");
        let systems: Vec<String> = read
            .emitters
            .iter()
            .map(|em| match &em.kind {
                trp_data::particles::EmitterKind::System(a) => format!("{} ({}, way {}, {}+{}, within {:?} m)", a.data.name, a.data.renderer.material.shader, em.way, em.min_count, em.count_range, em.distance),
                trp_data::particles::EmitterKind::Sparks(_) => format!("sparks (way {}, {}+{}, within {:?} m)", em.way, em.min_count, em.count_range, em.distance),
            })
            .collect();
        println!("{} {:?}: deferred {}, old {}, light {:?}, shift {}\n  {}", e.name, e.materials, e.deferred_decal, e.old_decal, read.light, read.particles_shift, systems.join("\n  "));
    }
    // Rubber, generic soft, fabric, cardboard and snow are the first's.
    assert_eq!(entries[0].materials, vec![30, 9, 6, 3, 38]);
    assert!(entries.iter().any(|e| e.name == "Concrete" && e.materials == vec![1, 5, 21] && e.deferred_decal));
    let decals = trp_data::particles::deferred_bullet_decals(&mut assets).expect("the decals");
    for (materials, d) in decals.by_material.iter().map(|(m, d)| (m.clone(), d)).chain([(Vec::new(), &decals.generic)]) {
        println!("decal {materials:?}: {} {}x{} by {}, {}x{} tiles, sizes {:?}, turned {}, colour {:?}, specular {:?}, normals {}, {} m deep, at most {}", d.picture.name, d.picture.width, d.picture.height, d.shader, d.columns, d.rows, d.sizes, d.randomize_rotation, d.color, d.specular_color, d.normal_power, d.projector_height, d.max_decals);
    }
    assert_eq!(decals.by_material.len(), 6);
    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    let sounds = trp_data::sound::effect_sounds(&install, &names, false).expect("the sounds");
    for (name, at) in &sounds.effects {
        if let Some(b) = at.map(|at| &sounds.banks[at]) {
            println!("sound {name}: {} rolloff {} physical {} limited {} clip length {} source {}", b.name, b.rolloff, b.physical, b.limited, b.clip_length, b.source_type);
        }
    }
    let ricochet = sounds.additional.first().copied().flatten().map(|at| &sounds.banks[at]).expect("the ricochet's bank");
    println!("ricochet: {} rolloff {} clip length {} source {}", ricochet.name, ricochet.rolloff, ricochet.clip_length, ricochet.source_type);
}

/// The impacts' systems drawn by shaders not yet ported, as read: render
/// mode, mesh, the material's figures (for the ports).
#[test]
fn the_impacts_unported_systems() {
    let Ok(install) = Install::locate() else { return };
    let mut assets = trp_data::unity::Assets::new(install.clone()).expect("the assets");
    for name in ["Concrete", "Wood", "Plastic", "Glass", "Soil"] {
        let read = trp_data::particles::effect_entry(&mut assets, name).expect("the entry");
        for em in &read.emitters {
            let trp_data::particles::EmitterKind::System(a) = &em.kind else { continue };
            let r = &a.data.renderer;
            let m = &r.material;
            if !matches!(m.shader.as_str(), "Standard" | "Legacy Shaders/Transparent/Diffuse" | "EFT/BumpedGlass") {
                continue;
            }
            println!(
                "{name}: {} by {} (mode {}, normals {}, mesh {:?}, queue {}), keywords {:?}\n  floats {:?}\n  colors {:?}\n  textures {:?}\n  unbuilt {:?}",
                a.data.name,
                m.shader,
                r.render_mode,
                r.normal_direction,
                r.mesh.as_ref().map(|mesh| (mesh.name.clone(), mesh.positions.len())),
                m.render_queue,
                m.keywords,
                m.floats,
                m.colors,
                m.textures.iter().map(|(n, t)| (n.clone(), t.name.clone(), t.width, t.height, t.faces, t.format, t.mip_count, t.data.len())).collect::<Vec<_>>(),
                a.data.unbuilt
            );
        }
    }
}

/// The velocity-over-lifetime modules of the impacts' systems, raw.
#[test]
fn the_impacts_velocity_modules() {
    let Ok(install) = Install::locate() else { return };
    let mut assets = trp_data::unity::Assets::new(install.clone()).expect("the assets");
    let entries = trp_data::particles::material_entries(&mut assets).expect("the entries");
    for e in &entries {
        let read = trp_data::particles::effect_entry(&mut assets, &e.name).expect("the entry");
        for em in &read.emitters {
            let trp_data::particles::EmitterKind::System(a) = &em.kind else { continue };
            if !a.data.unbuilt.iter().any(|m| m == "VelocityModule") {
                continue;
            }
            let raw = assets.read(a.system).expect("the system");
            let m = raw.get("VelocityModule").expect("the module");
            println!("{} / {}: world {:?}", e.name, a.data.name, m.get("inWorldSpace"));
            for field in ["x", "y", "z", "orbitalX", "orbitalY", "orbitalZ", "orbitalOffsetX", "orbitalOffsetY", "orbitalOffsetZ", "radial", "speedModifier"] {
                if let Some(v) = m.get(field) {
                    println!("  {field}: {:?}", trp_data::particles::MinMaxCurve::read(v).ok());
                }
            }
        }
    }
}

/// The inspect window's rows (inspect-window spec, "A row"): its three
/// templates, by its script's pointers, and the attributes' icons.
#[test]
fn the_inspect_window_s_rows_and_icons_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let window = trp_data::screen::info_window(&install).expect("the window");
    let rows = trp_data::screen::info_rows(&install, &window).expect("its rows");
    for (panel, parts) in [(&rows.bar, ["Icon", "Name", "Value", "bars/Full"]), (&rows.compact, ["Icon", "Name", "Value", "Icon"]), (&rows.dropdown, ["Icon", "Name", "Hover", "Hover/Arrow"])] {
        for part in parts {
            assert!(panel.find(part).is_some(), "{part}");
        }
    }
    let icons = trp_data::screen::attribute_icons(&install).expect("the icons");
    for id in ["Ergonomics", "CenterOfImpact", "RecoilUp", "Velocity", "ArmorClass", "ArmorPoints", "MaxAmmoDamage", "AmmoPenetrationPower"] {
        assert!(icons.iter().any(|(key, _)| key == id), "{id}");
    }
    // Its parts' panel's slot, from another file, 64 square with its name,
    // its picture and its filters' row; how far it stretches; the slots'
    // pictures by their names.
    let slot = rows.mod_slot.as_ref().expect("the parts' slot");
    for part in ["CanvasGroup/Background Image", "CanvasGroup/Text", "CanvasGroup/filters", "CanvasGroup/Full Border", "CanvasGroup/Empty Border"] {
        assert!(slot.find(part).is_some(), "{part}");
    }
    let limit = trp_data::screen::stretch_limit(&window).expect("its stretch limit");
    println!("the window stretches to {limit:?}; its slot is in {}", slot.file);
    assert!(limit[0] >= 670.0 && limit[1] >= 500.0, "{limit:?}");
    // The armour classes' marks a text names (`<sprite name="armor_classes_N">`),
    // each the engine's sprite its glyph points to: class 1 the first glyph's
    // (`armor_classes_0`), class 5 the third's.
    let classes = trp_data::screen::tmp_sprite_names(&install, "Sprite Assets/armor_classes").expect("the classes' marks");
    let of = |name: &str| classes.iter().find(|(own, _)| own == name).map(|(_, (_, id))| *id);
    assert_eq!(classes.len(), 6, "{classes:?}");
    assert!(of("armor_classes_1").is_some() && of("armor_classes_5").is_some() && of("armor_classes_1") != of("armor_classes_5"), "{classes:?}");
    let sprites = trp_data::screen::resource_sprites(&install, "Slots/").expect("the slots' pictures");
    for name in ["mod_magazine", "mod_muzzle", "mod_stock", "patron_in_weapon", "armor_plate"] {
        assert!(sprites.iter().any(|(path, _)| path == name), "{name} among {} pictures", sprites.len());
    }
}

/// The rows' colours and pictures from their scripts.
#[test]
fn the_inspect_window_s_rows_colours_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let window = trp_data::screen::info_window(&install).expect("the window");
    let rows = trp_data::screen::info_rows(&install, &window).expect("its rows");
    let bar = trp_data::screen::row_look(&rows.bar, "CharacteristicPanel").expect("the bar's look");
    let figures = bar.bar.expect("a bar's own");
    assert!((figures.sub[0] - 0.758).abs() < 0.01 && (figures.add[2] - 0.622).abs() < 0.01, "{figures:?}");
    assert!(figures.add_sprite.is_some() && figures.sub_sprite.is_some());
    let compact = trp_data::screen::row_look(&rows.compact, "CompactCharacteristicPanel").expect("the compact row's look");
    assert!(compact.bar.is_none() && (compact.decreasing[0] - 0.769).abs() < 0.01, "{compact:?}");
}

/// The face shield's mask (`VisorEffect` on `Cam2`, and the camera's
/// `HysteresisFilter`): the figures as read by hand in session 126
/// (`local\visor_shader_s126.md`), the three kinds' masks, and the four
/// pictures read.
#[test]
fn the_face_shield_s_mask_is_read() {
    use trp_data::visor::MaskKind;
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return;
    };
    let visor = trp_data::visor::read(&install).expect("the visor's mask");
    assert!((visor.scratches_intensity - 0.45).abs() < 1e-6 && (visor.min_light - 0.6).abs() < 1e-6 && !visor.affected_by_light);
    assert_eq!((visor.downsample, visor.blur_iterations), (1, 2));
    assert!((visor.blur_size - 1.5).abs() < 1e-6 && (visor.distort_intensity - 0.0075).abs() < 1e-6);
    assert_eq!(visor.edge_fade.keys.len(), 4);
    assert_eq!((visor.dx.keys.len(), visor.dy.keys.len(), visor.walk_rotation.keys.len()), (9, 9, 6));
    let kinds: Vec<(MaskKind, bool)> = visor.masks.iter().map(|(kind, picture)| (*kind, picture.is_some())).collect();
    assert_eq!(kinds, [(MaskKind::Wide, true), (MaskKind::Narrow, true), (MaskKind::None, false)]);
    assert!(visor.mask_of(MaskKind::None).is_none(), "no frame without a mask");
    assert_eq!((visor.mask.width, visor.mask.height), (512, 256));
    assert_eq!((visor.scratches.width, visor.scratches.height), (2048, 1024));
    // The camera's filter: down over 0.2 s after 0.6 s, the forward
    // curve's last key at 0.8017 s.
    let last = visor.forward.keys.last().expect("its keys").time;
    assert!((last - 0.80171776).abs() < 1e-5, "{last}");
    assert!(visor.forward.evaluate(0.5) == 0.0 && (visor.forward.evaluate(0.70171777) - 0.5).abs() < 0.01);
    println!("the visor: edge fade {:?}; forward {:?}; backward {:?}", visor.edge_fade.keys, visor.forward.keys, visor.backward.keys);
}
