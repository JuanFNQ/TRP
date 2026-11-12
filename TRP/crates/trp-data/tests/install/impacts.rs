//! Tests that read ballistic presets, impacts and a level's ballistic scripts.

use super::*;

/// The ballistic presets (`ColliderPresets`) are read, each by its
/// material's number, and the terrain's give what Customs' ground was
/// recorded with (`courses3`, `2-surfaces`: soil, asphalt, gravel and tall
/// grass, which the terrain takes from the presets).
#[test]
fn the_ballistic_presets_are_read() {
    let Ok(install) = Install::locate() else { return };
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("the hard settings");
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
    let entries = trp_data::drawing::particles::material_entries(&mut assets).expect("the entries");
    for e in &entries {
        let read = trp_data::drawing::particles::effect_entry(&mut assets, &e.name).expect("the entry");
        let systems: Vec<String> = read
            .emitters
            .iter()
            .map(|em| match &em.kind {
                trp_data::drawing::particles::EmitterKind::System(a) => format!("{} ({}, way {}, {}+{}, within {:?} m)", a.data.name, a.data.renderer.material.shader, em.way, em.min_count, em.count_range, em.distance),
                trp_data::drawing::particles::EmitterKind::Sparks(_) => format!("sparks (way {}, {}+{}, within {:?} m)", em.way, em.min_count, em.count_range, em.distance),
            })
            .collect();
        println!("{} {:?}: deferred {}, old {}, light {:?}, shift {}\n  {}", e.name, e.materials, e.deferred_decal, e.old_decal, read.light, read.particles_shift, systems.join("\n  "));
    }
    // Rubber, generic soft, fabric, cardboard and snow are the first's.
    assert_eq!(entries[0].materials, vec![30, 9, 6, 3, 38]);
    assert!(entries.iter().any(|e| e.name == "Concrete" && e.materials == vec![1, 5, 21] && e.deferred_decal));
    let decals = trp_data::drawing::particles::deferred_bullet_decals(&mut assets).expect("the decals");
    for (materials, d) in decals.by_material.iter().map(|(m, d)| (m.clone(), d)).chain([(Vec::new(), &decals.generic)]) {
        println!("decal {materials:?}: {} {}x{} by {}, {}x{} tiles, sizes {:?}, turned {}, colour {:?}, specular {:?}, normals {}, {} m deep, at most {}", d.picture.name, d.picture.width, d.picture.height, d.shader, d.columns, d.rows, d.sizes, d.randomize_rotation, d.color, d.specular_color, d.normal_power, d.projector_height, d.max_decals);
    }
    assert_eq!(decals.by_material.len(), 6);
    // The two of blood (body hits spec, "What a body's hit shows").
    let blood = trp_data::drawing::particles::deferred_blood_decals(&mut assets).expect("the blood decals");
    for (name, d) in [("environment", &blood.environment), ("bleeding", &blood.bleeding)] {
        println!("blood {name}: {} {}x{} ({:?}) by {}, {}x{} tiles, sizes {:?}, turned {}, colour {:?}, specular {:?}, normals {}, {} m deep, at most {}", d.picture.name, d.picture.width, d.picture.height, d.picture.format, d.shader, d.columns, d.rows, d.sizes, d.randomize_rotation, d.color, d.specular_color, d.normal_power, d.projector_height, d.max_decals);
    }
    println!("a bleeding's drops every {:?} s", blood.bleeding_spawn_delta);
    // Red pictures, each a sheet of two by two, laid as a still thing's hole.
    assert_eq!(blood.environment.color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(blood.bleeding.color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!((blood.environment.columns, blood.environment.rows, blood.bleeding.columns, blood.bleeding.rows), (2, 2, 2, 2));
    assert!(blood.bleeding_spawn_delta[0] > 0.0 && blood.bleeding_spawn_delta[1] >= blood.bleeding_spawn_delta[0]);
    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    let sounds = trp_data::audio::sound::effect_sounds(&install, &names, false).expect("the sounds");
    for (name, at) in &sounds.effects {
        if let Some(b) = at.map(|at| &sounds.banks[at]) {
            println!("sound {name}: {} rolloff {} physical {} limited {} clip length {} source {}", b.name, b.rolloff, b.physical, b.limited, b.clip_length, b.source_type);
        }
    }
    let ricochet = sounds.additional.first().copied().flatten().map(|at| &sounds.banks[at]).expect("the ricochet's bank");
    println!("ricochet: {} rolloff {} clip length {} source {}", ricochet.name, ricochet.rolloff, ricochet.clip_length, ricochet.source_type);
    // The hit sounds of a melee weapon shown on the character's model
    // (`PlayKnifeHitEffect`): the fourth on a body, the fifth on a thing.
    for at in [3, 4] {
        let bank = sounds.additional.get(at).copied().flatten().map(|at| &sounds.banks[at]).expect("a shown blade's bank");
        println!("additional {at}: {} rolloff {} physical {} limited {} clip length {} source {}", bank.name, bank.rolloff, bank.physical, bank.limited, bank.clip_length, bank.source_type);
    }
    println!("{} additional banks", sounds.additional.len());
}

/// The impacts' systems drawn by shaders not yet ported, as read: render
/// mode, mesh, the material's figures (for the ports).
#[test]
fn the_impacts_unported_systems() {
    let Ok(install) = Install::locate() else { return };
    let mut assets = trp_data::unity::Assets::new(install.clone()).expect("the assets");
    for name in ["Concrete", "Wood", "Plastic", "Glass", "Soil"] {
        let read = trp_data::drawing::particles::effect_entry(&mut assets, name).expect("the entry");
        for em in &read.emitters {
            let trp_data::drawing::particles::EmitterKind::System(a) = &em.kind else { continue };
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
    let entries = trp_data::drawing::particles::material_entries(&mut assets).expect("the entries");
    for e in &entries {
        let read = trp_data::drawing::particles::effect_entry(&mut assets, &e.name).expect("the entry");
        for em in &read.emitters {
            let trp_data::drawing::particles::EmitterKind::System(a) = &em.kind else { continue };
            if !a.data.unbuilt.iter().any(|m| m == "VelocityModule") {
                continue;
            }
            let raw = assets.read(a.system).expect("the system");
            let m = raw.get("VelocityModule").expect("the module");
            println!("{} / {}: world {:?}", e.name, a.data.name, m.get("inWorldSpace"));
            for field in ["x", "y", "z", "orbitalX", "orbitalY", "orbitalZ", "orbitalOffsetX", "orbitalOffsetY", "orbitalOffsetZ", "radial", "speedModifier"] {
                if let Some(v) = m.get(field) {
                    println!("  {field}: {:?}", trp_data::drawing::particles::MinMaxCurve::read(v).ok());
                }
            }
        }
    }
}

/// Factory's ballistic scripts as its scenes keep them
/// (`level_colliders::ballistics`; ballistics spec, "Surfaces"): the walls
/// `courses2`'s rounds met, each with the figures the game wrote at a hit on
/// it in a later recording (wall D in `m700-bipod`; `MetalThick_LowPen`,
/// a preset's, 43); and the cable reels' two sets of thick wood, the four
/// outside's, whose one `courses2`'s three hits were on, and the ten
/// inside's.
#[test]
fn factory_s_ballistic_scripts_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let all = trp_data::level::level_colliders::ballistics(&install, trp_data::level::level_colliders::FACTORY_DAY).expect("Factory's ballistic scripts");
    println!("  {} ballistic scripts", all.len());
    let of = |name: &str| -> Vec<&trp_data::level::level_colliders::LevelBallistic> { all.iter().filter(|b| b.on && b.name == name).collect() };
    let wall = of("Factory_area_04_outside_wall_D_BALLISTIC_concrete");
    assert_eq!(wall.len(), 1);
    assert_eq!((wall[0].material, wall[0].preset, wall[0].figures), (5, -900, [100.0, 0.0, 0.41, 0.16, 0.23, 0.037]));
    let thick = of("MetalThick_LowPen");
    assert!(!thick.is_empty() && thick.iter().all(|b| b.material == 17 && b.preset == 43 && b.figures == [7.0, 0.63, 0.33, 0.13, 0.21, 0.043]));
    let reels: Vec<_> = of("wood").into_iter().filter(|b| b.material == 28 && b.path.contains("kabel_pallet")).collect();
    let outside: Vec<_> = reels.iter().filter(|b| b.figures == [25.0, 0.074, 0.338, 0.119, 0.289, 0.485]).collect();
    let inside = reels.iter().filter(|b| b.figures == [25.0, 0.35, 0.27, 0.11, 0.2, 0.028]).count();
    for b in &reels {
        println!("  {} {}: {:?}", b.scene, b.path, b.figures);
    }
    assert_eq!((outside.len(), inside, reels.len()), (4, 10, 14));
    assert!(outside.iter().all(|b| b.scene.ends_with("Factory_Rework_Background.unity")));
}
