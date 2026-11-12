//! Tests that read shadows, textures, camera effects and levels of detail.

use super::*;

/// A grenade's explosion (`Effects`' `Grenade_new`, the F-1's): every one of
/// its systems read, each one's figures printed (firing spec, "A grenade's
/// explosion").
/// The body armour repair kit's shadow meshes cast their shadow alone
/// (`m_CastShadows` 3), its model's own do not: drawn, the shadow meshes
/// cover the kit in white.
#[test]
fn a_shadow_mesh_is_read_as_shadows_only() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let key = "assets/content/items/spec/item_spec_armorrepair/item_spec_armorrepair.bundle";
    let (hierarchy, _) = trp_data::interface::icon::model_of(&mut assets, key).expect("the kit's model");
    let shadows_only = |name: &str| hierarchy.nodes.iter().find(|node| node.name == name).and_then(|node| node.renderer.as_ref()).map(|renderer| renderer.shadows_only);
    assert_eq!(shadows_only("shadow_LOD0"), Some(true));
    assert_eq!(shadows_only("item_spec_armorrepair_LOD0"), Some(false));
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
/// streaming is on. Every one of the AKM's 27 is (each of priority 0).
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

/// The eyes' burn's figures and pictures (`Cam2`'s `EyeBurn`): three spots
/// and a trail, 0.2 to 1.1 in size, 100 s at most.
#[test]
fn the_eyes_burn_is_read() {
    let Ok(install) = Install::locate() else { return };
    let burn = trp_data::drawing::eye_burn::read(&install).expect("the eyes' burn");
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
    let read = trp_data::drawing::camera_effects::read(&install).expect("the camera's effects");
    let h = &read.health;
    let keys = |c: &trp_data::formats::curve::Curve| c.keys.iter().map(|k| (k.time, k.value, k.in_slope, k.out_slope)).collect::<Vec<_>>();
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
    let level = trp_data::level::tod::TimeOfDayLevel::customs(&install).expect("Customs");
    let file = level.file_of(level.settings.camera).expect("the camera's file");
    let s = &trp_data::drawing::camera_effects::SharpenData::of_camera(&install, &file, level.settings.camera.1).expect("the level camera's sharpening");
    println!("sharpen {} clamp {} desaturate {}; ramp {}x{} {:?}; offset {:?}; radius {:?} falloff {}", s.strength, s.clamp, s.desaturate, s.ramp.width, s.ramp.height, s.ramp.format, s.ramp_offset, s.min_max_radius, s.radius_falloff);
    assert_eq!((s.clamp, s.desaturate), (0.406, true));
    assert_eq!((s.ramp_offset, s.min_max_radius, s.radius_falloff), ([0.0; 3], [0.325, 1.0], 0.425));
    assert_eq!((h.radial_most, h.vignette_darkness, h.near_death_most), (0.035, 74.9, 1.0));
    println!("the berserk's sharpening: {}", h.sharpen_most);
    assert_eq!(h.sharpen_most, 4.11);
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
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("the hard settings");
    println!("bleeding centres {:?}", hard.vec3s("BleedingCenters").expect("the bleeding centres"));
}

/// The levels of detail (third-person spec, "Levels of detail"; the brief's
/// figures, `local\distance_brief_s266.md`, part 1a): the bear body's two
/// skins and its group, the bear head's, the hooded top's hood apart, an
/// armband's empty `Skin` group dropped, the Altyn's group on its root with
/// its coarser mesh kept apart, a rig's skins with their group placed by
/// their meshes, the player's mesh transform they hang under, the bias.
#[test]
fn the_levels_of_detail_are_read() {
    use trp_data::character::dress::{load, load_levels, Under, Worn};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let points = |assets: &Assets, mesh| read_mesh(assets, mesh).map(|mesh| (mesh.positions.len(), mesh.submeshes.iter().map(Vec::len).sum::<usize>() / 3)).expect("a level's mesh");
    // The bear body: size 2, its levels down to 0.6 and 0.015 of the view;
    // 2,198 points and 3,458 triangles, then 1,054 and 1,458; one material.
    let body = "assets/content/characters/character/prefabs/bear_body.bundle";
    let levels = trp_data::character::arms::load_levels(&mut assets, body).expect(body);
    assert_eq!(levels.len(), 2);
    let lod = levels[0].lod.as_ref().expect("the body's levels");
    assert_eq!((lod.group.size, lod.group.heights(), lod.group.fade_mode), (2.0, vec![0.6, 0.015], 0));
    assert_eq!(lod.levels, 0b01);
    assert_eq!(levels[1].lod.as_ref().map(|lod| lod.levels), Some(0b10));
    assert_eq!(points(&assets, levels[0].mesh), (2198, 3458));
    assert_eq!(points(&assets, levels[1].mesh), (1054, 1458));
    assert_eq!(levels[0].materials, levels[1].materials);
    // Its point as saved, beside the feet once under the mesh transform.
    assert!((lod.group.reference[0] + 0.388).abs() < 0.001 && (lod.group.reference[1] - 0.213).abs() < 0.001, "{:?}", lod.group.reference);
    // The bear head: down to 0.035; 2,526 points, then 765.
    let head = "assets/content/characters/character/prefabs/bear_head_0.bundle";
    let levels = trp_data::character::arms::load_levels(&mut assets, head).expect(head);
    assert_eq!(levels.len(), 2);
    assert_eq!(levels[0].lod.as_ref().expect("the head's levels").group.heights(), vec![0.6, 0.035]);
    assert_eq!((points(&assets, levels[0].mesh).0, points(&assets, levels[1].mesh).0), (2526, 765));
    // The hooded top: its two hooded forms are the hood's, not levels.
    let cultist = "assets/content/characters/character/prefabs/top_pmc_cultist.bundle";
    let levels = trp_data::character::arms::load_levels(&mut assets, cultist).expect(cultist);
    let hoods = trp_data::character::dress::hood_levels(&mut assets, cultist).expect("its hood's levels");
    assert_eq!((levels.len(), hoods.len()), (2, 2));
    assert_eq!(levels.iter().map(|level| level.lod.as_ref().map(|lod| lod.levels)).collect::<Vec<_>>(), [Some(0b01), Some(0b10)]);
    assert_eq!(hoods.iter().map(|hood| hood.lod.as_ref().map(|lod| lod.levels)).collect::<Vec<_>>(), [Some(0b01), Some(0b10)]);
    // An armband's `Skin` group (size 0.5) lists no renderer: dropped, the
    // loot model's (size 3) kept.
    let band = "assets/content/items/equipment/armband/item_equipment_armband_alpha.bundle";
    let files = assets.load(band).expect(band);
    let raw: Vec<trp_data::drawing::lod::LodGroup> = files.iter().flat_map(|&file| assets.objects_of(file, trp_data::unity::class::LOD_GROUP)).map(|group| trp_data::drawing::lod::read(&assets, group).expect("a group")).collect();
    assert!(raw.iter().any(|group| group.size == 0.5 && group.governs_nothing()), "{raw:?}");
    let kept = trp_data::drawing::lod::of_files(&assets, &files);
    assert!(!kept.is_empty() && kept.iter().all(|group| !group.governs_nothing() && group.size != 0.5), "{kept:?}");
    // The Altyn: a rigid model, its group on its root (size 0.5; 0.15 and
    // 0.015); its coarser mesh apart, at level 1 alone.
    let altyn = "assets/content/items/equipment/helmet_altyn/item_equipment_helmet_altyn.bundle";
    let Worn::Rigid(model) = load(&mut assets, altyn, Under::default()).expect(altyn) else { panic!("the Altyn is a model") };
    assert_eq!(model.lods.len(), 1);
    assert_eq!(model.lods[0].node, 0);
    assert_eq!((model.lods[0].group.size, model.lods[0].group.heights()), (0.5, vec![0.15, 0.015]));
    let node = |name: &str| model.nodes.iter().find(|node| node.name == name).expect(name);
    let (fine, coarse) = (node("helmet_altyn_LOD0"), node("helmlet_altyn_LOD1"));
    assert!(fine.renderer.is_some() && fine.lower.is_none());
    assert_eq!(fine.lod.map(|lod| (lod.group, lod.levels)), Some((0, 0b01)));
    assert!(coarse.renderer.is_none() && coarse.lower.is_some() && coarse.bounded.is_some());
    assert_eq!(coarse.lod.map(|lod| (lod.group, lod.levels)), Some((0, 0b10)));
    // A rig: its `Skin` group's levels (0.6, 0.04), placed by its meshes'
    // bounds as it is dressed: at the chest, not where it is saved.
    let rig = "assets/content/items/equipment/armor_6b5/item_equipment_rig_6b5_flora.bundle";
    let levels = load_levels(&mut assets, rig, Under::default()).expect(rig).expect("a skinned thing");
    assert_eq!(levels.len(), 2);
    let lod = levels[0].lod.as_ref().expect("the rig's levels");
    assert_eq!(lod.group.heights(), vec![0.6, 0.04]);
    eprintln!("the rig's point as dressed: {:?}", lod.group.reference);
    assert!(lod.group.reference[2] > 0.8, "{:?}", lod.group.reference);
    // The mesh transform the body's parts hang under: the player's `Mesh`,
    // at its origin, turned -90 degrees about x.
    let (place, turn) = trp_data::character::player::mesh_transform(&mut assets).expect("the mesh transform");
    assert_eq!(place, [0.0; 3]);
    assert!((turn[0] + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5 && (turn[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5, "{turn:?}");
    // The AKM assembled carries each part's group and coarser meshes.
    let Some((_, akm)) = akm() else { return };
    let coarser = akm.hierarchy.nodes.iter().filter(|node| node.lower.is_some()).count();
    let tagged = akm.hierarchy.nodes.iter().filter(|node| node.lod.is_some()).count();
    eprintln!("the AKM: {} groups, {tagged} renderers tagged, {coarser} coarser apart; reaches {:?}", akm.hierarchy.lods.len(), akm.hierarchy.lods.iter().map(|lod| (akm.hierarchy.nodes[lod.node].name.as_str(), lod.group.size, lod.group.heights())).collect::<Vec<_>>());
    assert!(akm.hierarchy.lods.len() >= 5 && coarser > 0 && tagged > coarser);
    assert!(akm.hierarchy.lods.iter().all(|lod| lod.node < akm.hierarchy.nodes.len()));
    // The install's bias (`Graphics.ini`): 2, the game's clamp 2 to 4.
    assert_eq!(assets.install.display_settings().lod_bias, Some(2.0));
    assert_eq!((trp_data::setup::install::lod_bias(1.0), trp_data::setup::install::lod_bias(5.0), trp_data::setup::install::lod_bias(2.5)), (2.0, 4.0, 2.5));
}

/// TRP's own measure of the levels (third-person spec, "TRP's own
/// rule"): each coarser level's deviation from the fullest, both ways, its
/// openings closed, of the bear body, legs and head, the Scav's head, a rig,
/// the Altyn, the range's magazine, round and key, the popper and the AKM's
/// every group; the bear body's open waist alone 7.3 cm, which the closing
/// takes out; each group measured once a run (read again, the kept measure).
/// The closing's loops by the triangles' winding and its caps not sampled:
/// the same figure every time (a walk by a map's order of the bear body's
/// open sides gave 13.6, 23.7 or 24.7 mm).
#[test]
fn trps_measure_of_the_levels() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    trp_data::drawing::lod::set_measuring(true);
    let mut assets = Assets::new(install.clone()).expect("manifest");
    let started = std::time::Instant::now();
    // (Each: its coarser level's deviation, metres, as measured.)
    let near = |got: f32, want: f32| (got - want).abs() < 0.0005;
    for (key, want) in [
        ("assets/content/characters/character/prefabs/bear_body.bundle", 0.01363),
        ("assets/content/characters/character/prefabs/bear_feet.bundle", 0.01708),
        ("assets/content/characters/character/prefabs/bear_head_0.bundle", 0.00652),
        ("assets/content/characters/character/prefabs/wild_head_misha.bundle", 0.01772),
    ] {
        let levels = trp_data::character::arms::load_levels(&mut assets, key).expect(key);
        let lod = levels[0].lod.as_ref().expect(key);
        eprintln!("{key}: {:?}", lod.group.deviation);
        assert_eq!(lod.group.deviation.len(), 2, "{key}");
        assert!(lod.group.deviation[0] == 0.0 && near(lod.group.deviation[1], want), "{key}: {:?}", lod.group.deviation);
        assert!(levels.iter().all(|level| level.lod.as_ref().is_some_and(|other| other.group.deviation == lod.group.deviation)), "{key}");
    }
    // The bear body's levels with their openings left open: its fullest's
    // cap across the waist, hidden by the legs, 7.3 cm from the coarser.
    let body = trp_data::character::arms::load_levels(&mut assets, "assets/content/characters/character/prefabs/bear_body.bundle").expect("the body");
    let meshes: Vec<_> = body.iter().map(|level| (level.mesh, level.lod.as_ref().map_or(0, |lod| lod.levels))).collect();
    let shapes = trp_data::drawing::lod::skin_shapes(&assets, 2, &meshes).expect("the body's meshes");
    let open = trp_data::drawing::lod::deviation(&shapes[0], &shapes[1]);
    let far = trp_data::drawing::lod::far_samples(&shapes[0], &shapes[1], 0.04);
    eprintln!("the bear body open: {open} m; farther than 4 cm: {:?}", &far[..far.len().min(4)]);
    assert!(near(open, 0.07285), "{open}");
    // Closed, the same to the bit however a map orders the open sides (each
    // closing's maps seeded anew): the right wrist's coarser opening, two
    // loops meeting at a corner, closed as two, their caps not sampled.
    let each: Vec<Vec<f32>> = (0..8).map(|_| trp_data::drawing::lod::deviations(&shapes)).collect();
    assert!(each.iter().all(|got| got == &each[0]) && near(each[0][1], 0.01363), "{each:?}");
    let loops: Vec<(usize, usize)> = shapes.iter().map(|shape| shape.closed()).zip(&shapes).map(|(closed, open)| (closed.points.len() - open.points.len(), closed.caps)).collect();
    assert_eq!(loops, vec![(3, 16), (5, 44)]);
    assert!(!far.is_empty() && far.iter().all(|(at, _)| (at[2] - 1.09).abs() < 0.03 && at[0].abs() < 0.15 && at[1].abs() < 0.15), "{far:?}");
    // A worn skin and a rigid model.
    let rig = "assets/content/items/equipment/armor_6b5/item_equipment_rig_6b5_flora.bundle";
    let levels = trp_data::character::dress::load_levels(&mut assets, rig, trp_data::character::dress::Under::default()).expect(rig).expect("skinned");
    let deviation = levels[0].lod.as_ref().map(|lod| lod.group.deviation.clone()).unwrap_or_default();
    assert!(deviation.len() == 2 && near(deviation[1], 0.03280), "{deviation:?}");
    let altyn = "assets/content/items/equipment/helmet_altyn/item_equipment_helmet_altyn.bundle";
    let trp_data::character::dress::Worn::Rigid(model) = trp_data::character::dress::load(&mut assets, altyn, trp_data::character::dress::Under::default()).expect(altyn) else { panic!("rigid") };
    assert!(near(model.lods[0].group.deviation[1], 0.01121), "{:?}", model.lods[0].group.deviation);
    // The range's loose things: the magazine, a round, the key of one level.
    for (key, want) in [
        ("assets/content/items/mods/magazines/mag_ak_izhmash_ak_std_55_762x39_30.bundle", vec![0.0, 0.01232]),
        ("assets/content/items/ammo/patrons/patron_762x39_fmj.bundle", vec![0.0, 0.00101]),
        ("assets/content/items/spec/keys/item_key_14.bundle", vec![0.0]),
    ] {
        let (hierarchy, _) = trp_data::interface::icon::model_of(&mut assets, key).expect(key);
        let got = &hierarchy.lods[0].group.deviation;
        assert!(got.len() == want.len() && got.iter().zip(&want).all(|(a, b)| near(*a, *b)), "{key}: {got:?}");
    }
    let popper = trp_data::level::range_targets::popper(&install, true).expect("the popper");
    assert!(near(popper.props.lods[0].group.deviation[1], 0.00558), "{:?}", popper.props.lods[0].group.deviation);
    eprintln!("measured in {:.2} s", started.elapsed().as_secs_f64());
    // The AKM's every group, 2.3 to 17.2 mm (the rear sight to the
    // weapon's body); its magazine's the magazine's own (the kept measure).
    let Some((_, akm)) = akm() else { return };
    let all: Vec<(String, f32)> = akm.hierarchy.lods.iter().map(|lod| (akm.hierarchy.nodes[lod.node].name.clone(), lod.group.deviation.get(1).copied().unwrap_or(f32::NAN))).collect();
    eprintln!("the AKM: {all:?}");
    assert_eq!(all.len(), 9);
    assert!(all.iter().all(|(_, d)| (0.0023..0.0176).contains(d)), "{all:?}");
    assert!(all.iter().any(|(name, d)| name.starts_with("mag_ak") && near(*d, 0.01232)), "{all:?}");
}

/// The levels of detail of the range's loose things and props (third-person spec, "Levels of detail"): what lies on the range at the
/// start (two AK magazines, 7.62x39 rounds, the yard's key) with each
/// renderer's group and levels and its coarser meshes apart, as a thing on
/// the ground is read (`icon::model_of`); a popper and the gun bench read
/// with every level (`scene_props::read`), and without, as before.
#[test]
fn the_loose_things_and_props_levels_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install.clone()).expect("manifest");
    let near = |a: f32, b: f32| (a - b).abs() < 0.0005;
    // (Each: its bundle, its group's size and levels, how many renderers
    // tagged and how many of them at a coarser level only.)
    let things: [(&str, f32, &[f32], usize, usize); 3] = [
        ("assets/content/items/mods/magazines/mag_ak_izhmash_ak_std_55_762x39_30.bundle", 0.2013, &[0.0971, 0.01], 2, 1),
        ("assets/content/items/ammo/patrons/patron_762x39_fmj.bundle", 0.0556, &[0.2005, 0.01], 4, 2),
        ("assets/content/items/spec/keys/item_key_14.bundle", 0.5268, &[0.0453], 1, 0),
    ];
    let mut groups = 0;
    for (key, size, heights, tagged, coarser) in things {
        let (hierarchy, _) = trp_data::interface::icon::model_of(&mut assets, key).expect(key);
        eprintln!("{key}: {} groups {:?}", hierarchy.lods.len(), hierarchy.lods.iter().map(|lod| (hierarchy.nodes[lod.node].name.as_str(), lod.group.size, lod.group.heights(), lod.group.fade_mode)).collect::<Vec<_>>());
        assert_eq!(hierarchy.lods.len(), 1, "{key}");
        let group = &hierarchy.lods[0].group;
        assert!(near(group.size, size) && group.heights().len() == heights.len() && group.heights().iter().zip(heights).all(|(a, b)| near(*a, *b)), "{key}: {group:?}");
        assert_eq!(group.fade_mode, 0, "{key}");
        assert_eq!(hierarchy.nodes.iter().filter(|node| node.lod.is_some()).count(), tagged, "{key}");
        assert_eq!(hierarchy.nodes.iter().filter(|node| node.lower.is_some()).count(), coarser, "{key}");
        // Every coarser renderer at the coarser levels alone.
        assert!(hierarchy.nodes.iter().filter(|node| node.lower.is_some()).all(|node| node.lod.is_some_and(|lod| lod.levels & 1 == 0)), "{key}");
        groups += hierarchy.lods.len();
    }
    // The round's levels hold its unspent and spent forms alike (a case is
    // the round's object: `AmmoPoolObject.SetUsed`).
    let (round, _) = trp_data::interface::icon::model_of(&mut assets, things[1].0).expect("the round");
    let levels_of = |word: &str| round.nodes.iter().filter(|node| node.name.contains(word)).filter_map(|node| node.lod.map(|lod| lod.levels)).collect::<Vec<_>>();
    assert_eq!((levels_of("full"), levels_of("shell")), (vec![0b01, 0b10], vec![0b01, 0b10]));
    // A popper: one group on its own object (size 1.2005; 0.19, 0.03075;
    // cross-fading), its three parts at each level; without every level
    // asked for, only the nearest read and nothing tagged.
    let popper = trp_data::level::range_targets::popper(&install, true).expect("the popper");
    assert_eq!(popper.props.lods.len(), 1);
    let group = &popper.props.lods[0];
    assert_eq!((group.node, group.group.fade_mode), (0, 1));
    assert!(near(group.group.size, 1.2005) && near(group.group.heights()[0], 0.19) && near(group.group.heights()[1], 0.03075), "{:?}", group.group);
    let tags: Vec<u8> = popper.props.nodes.iter().filter_map(|node| node.lod.map(|lod| lod.levels)).collect();
    assert_eq!((tags.iter().filter(|&&l| l == 0b01).count(), tags.iter().filter(|&&l| l == 0b10).count(), tags.len()), (3, 3, 6));
    let fullest = trp_data::level::range_targets::popper(&install, false).expect("the popper, its nearest level");
    assert!(fullest.props.lods.is_empty() && fullest.props.nodes.iter().all(|node| node.lod.is_none()));
    assert_eq!(fullest.props.nodes.iter().filter(|node| !node.drawn.is_empty()).count(), 3);
    assert_eq!(popper.props.nodes.iter().filter(|node| !node.drawn.is_empty()).count(), 6);
    groups += popper.props.lods.len();
    // The gun bench as the range reads it (its table and vise, the AKS-74U
    // in the vise left out): the vise's group alone (0.6737; 0.3988,
    // 0.0408), the table none; the rifle's groups gone with it.
    let bench = trp_data::level::scene_props::read(&install, "bunker_2.unity", "10_workbench/level3", &["StaticDeferredDecal"], &["aksu"], &["highlight_transform/metal_table2 (1)", "highlight_transform/Vise"], true).expect("the gun bench");
    eprintln!("the gun bench: {:?}", bench.lods.iter().map(|lod| (bench.nodes[lod.node].name.as_str(), lod.group.size, lod.group.heights())).collect::<Vec<_>>());
    assert_eq!(bench.lods.len(), 1);
    assert_eq!(bench.nodes[bench.lods[0].node].name, "Vise");
    assert!(near(bench.lods[0].group.size, 0.6737) && near(bench.lods[0].group.heights()[1], 0.0408));
    assert_eq!(bench.nodes.iter().filter(|node| node.lod.is_some()).count(), 4);
    assert!(bench.nodes.iter().filter(|node| node.name.starts_with("metal_table")).all(|node| node.lod.is_none()));
    groups += bench.lods.len();
    eprintln!("the range's loose things at the start and its props, one of each: {groups} groups");
    assert_eq!(groups, 5);
}

/// The shadow at the menu figure's feet (`MenuPlayerPoser.BottomShadow`;
/// inventory spec, "The shadow at the feet"): five planes below its own
/// object, each Unity's own plane (10 by 10 m, flat, its normal up), drawn
/// by `Unlit/Transparent Colored` with one of two 256 by 256 pictures.
#[test]
fn the_menu_player_s_boot_shadow_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    // (Timed: the menu player's bundle, the shaders', the rest.)
    let began = std::time::Instant::now();
    assets.load(trp_data::character::body::MENU_PLAYER_BUNDLE).expect("the menu player");
    let player = began.elapsed();
    assets.load("shaders").expect("the shaders");
    let shaders = began.elapsed() - player;
    let shadow = trp_data::character::body::boot_shadow(&mut assets).expect("the boot shadow");
    eprintln!("read in {player:?} (the menu player), {shaders:?} (the shaders), {:?} (the shadow)", began.elapsed() - player - shaders);
    for node in &shadow.nodes {
        eprintln!("{} under {:?}: at {:?}, turned {:?}, scaled {:?}, on {}, {:?}", node.name, node.parent, node.translation, node.rotation, node.scale, node.active, node.plane);
    }
    let planes: Vec<_> = shadow.nodes.iter().filter_map(|node| node.plane).collect();
    assert_eq!(planes.len(), 5, "five planes");
    assert_eq!(shadow.nodes[shadow.switched].name, "BootShadow");
    assert_eq!(shadow.meshes.len(), 1, "one mesh for all");
    let plane = &shadow.meshes[0];
    let (low, high) = plane.positions.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(low, high), p| ([low[0].min(p[0]), low[1].min(p[1]), low[2].min(p[2])], [high[0].max(p[0]), high[1].max(p[1]), high[2].max(p[2])]));
    eprintln!("the plane: {} points, from {low:?} to {high:?}, normal {:?}", plane.positions.len(), plane.normals.first());
    assert!((high[0] - low[0] - 10.0).abs() < 1e-3 && (high[2] - low[2] - 10.0).abs() < 1e-3 && high[1] - low[1] < 1e-6, "10 by 10 m, flat");
    assert_eq!(shadow.pictures.len(), 2);
    for picture in &shadow.pictures {
        eprintln!("{}: {}x{} format {} sRGB {}", picture.name, picture.width, picture.height, picture.format, picture.srgb);
        assert_eq!((picture.width, picture.height), (256, 256));
    }
}
