//! Tests that read head things' fitting, face shields, hinges, masks and glass.

use super::*;

/// The head things' fittings to one another (body animator spec, "The
/// head's things fitted to one another"; `probe fittings`): of the 568
/// equipment bundles, 160 carry `CustomItem` scripts and 57
/// `CustomItemSkin`; every one of the 217 is read as worn but the 13
/// ballistic and airsoft masks', whose script names neither mesh (the game
/// leaves those as saved); every skin's is the face cover's, every drawn
/// fitting's two meshes have as many bind poses, and two drawn meshes
/// change their count of submeshes (the Grom glasses', a ULACH's). Of the
/// 317 body, feet and head bundles the customization list names that the
/// install has, 88 carry a `ClippingRuleChanger`, each on the part's own
/// object: 84 give the face cover's bit, 4 that and a bit no kind answers
/// to (32). The USEC cap is a hat with both meshes, the ComTac II
/// headphones.
#[test]
fn every_head_thing_s_fitting_is_read() {
    use trp_data::character::dress::{fit, fittings, load, survey, Under, Worn};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let (bundles, parts, missing) = survey(&install).expect("the survey");
    let with = |test: &dyn Fn(&trp_data::character::dress::SurveyedBundle) -> bool| bundles.iter().filter(|bundle| test(bundle)).count();
    assert_eq!(bundles.len(), 568);
    assert_eq!(with(&|b| b.plain_scripts > 0), 160);
    assert_eq!(with(&|b| b.skin_scripts > 0), 57);
    assert_eq!(with(&|b| b.plain_scripts > 0 || b.skin_scripts > 0), 217);
    let unread: Vec<&str> = bundles.iter().filter(|b| (b.plain_scripts > 0 || b.skin_scripts > 0) && !b.read.iter().any(|f| f.drawn)).map(|b| b.key.as_str()).collect();
    assert_eq!(unread.len(), 13, "{unread:?}");
    assert!(unread.iter().all(|key| key.contains("/facecover_ballistic_mask/")), "{unread:?}");
    assert!(bundles.iter().filter(|b| b.read.iter().any(|f| f.drawn)).all(|b| b.plain_scripts > 0 || b.skin_scripts > 0));
    let drawn: Vec<&trp_data::character::dress::SurveyedFitting> = bundles.iter().flat_map(|b| &b.read).filter(|f| f.drawn).collect();
    let of_kind = |kind: u8| drawn.iter().filter(|f| f.kind == kind).count();
    assert_eq!(drawn.len(), 204);
    assert_eq!([of_kind(fit::FACE_COVER), of_kind(fit::VISOR), of_kind(fit::HAT), of_kind(fit::HEADPHONES), of_kind(fit::HELMET)], [57, 33, 42, 14, 58]);
    assert!(bundles.iter().filter(|b| b.skin_scripts > 0).all(|b| b.read.len() == 1 && b.read[0].kind == fit::FACE_COVER));
    assert!(drawn.iter().all(|f| f.bind_poses[0] == f.bind_poses[1]));
    assert_eq!(drawn.iter().filter(|f| f.submeshes[0] != f.submeshes[1]).count(), 2);

    assert_eq!((parts.len(), missing), (317, 1));
    let changers: Vec<&trp_data::character::dress::SurveyedPart> = parts.iter().filter(|part| part.any_changer).collect();
    assert_eq!(changers.len(), 88);
    assert_eq!(changers.iter().filter(|part| part.bits == fit::FACE_COVER).count(), 84);
    assert_eq!(changers.iter().filter(|part| part.bits == fit::FACE_COVER | 32).count(), 4);
    assert!(parts.iter().all(|part| part.any_changer == (part.bits != 0)), "every changer on the part's own object");

    // The capture's cap and headset, each drawn object's fitting with both
    // meshes.
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    for (template, kind) in [("5aa2a7e8e5b5b00016327c16", fit::HAT), ("5645bcc04bdc2d363b8b4572", fit::HEADPHONES)] {
        let key = db.items[template].prefab_key().expect("its prefab").to_owned();
        let Worn::Rigid(model) = load(&mut assets, &key, Under::default()).expect("the model") else { panic!("{template} is a model") };
        let found = fittings(&assets, &model);
        let drawn: Vec<_> = found.iter().filter(|(node, _)| model.nodes[*node].renderer.is_some()).collect();
        assert_eq!(drawn.len(), 1, "{template}: {found:?}");
        let (node, fitting) = drawn[0];
        assert_eq!(fitting.kind, kind);
        assert_eq!(model.nodes[*node].renderer.as_ref().map(|r| r.mesh), Some(fitting.base), "the prefab saves its first mesh");
        assert_ne!(fitting.base, fitting.custom);
    }
}

/// A helmet's face shield and hinge (`ObjectsFactory.AttachMods`,
/// `HelmetDress`): the ZSh-1-2M's shield hangs on its `mod_equipment`,
/// hidden until fitted; its armour plates have no model and are not hung;
/// its hinge is the object its dress names, with the turns and curves it
/// keeps.
#[test]
fn a_helmet_s_face_shield_and_hinge_are_read() {
    use trp_data::character::dress::{head_model, load, Under, Worn};
    const ZSH: &str = "5aa7e454e5b5b0214e506fa2";
    const SHIELD: &str = "5aa7e3abe5b5b000171d064d";
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
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

/// A part fitted after the model was made (`fit_part`; the armor bench
/// offers parts the run did not have): the FAST made with nothing, then
/// its visor hung as `head_model` would have hung it, at the same object
/// with its own hinge; a mount on its NVG slot, and an adapter on the
/// mount's own slot below it; nothing for an armour plate, nor for a slot
/// the model has no object of.
#[test]
fn a_part_fitted_later_hangs_as_one_made_with_the_model() {
    use trp_data::character::dress::{fit_part, head_model, load, Under, Worn};
    const FAST: &str = "5a154d5cfcdbcb001a3b00da";
    const VISOR: &str = "5a16b672fcdbcb001912fa83";
    const MOUNT: &str = "5a16b8a9fcdbcb00165aa6ca";
    const ADAPTER: &str = "5c0695860db834001b735461";
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let key = db.items[FAST].prefab_key().expect("its prefab").to_owned();
    let Worn::Rigid(bare) = load(&mut assets, &key, Under::default()).expect("the FAST") else { panic!("a helmet is a model") };
    let (whole, made, made_hinges) = head_model(&mut assets, &db, FAST, bare.clone(), &|part| part == VISOR);
    let (mut model, none, _) = head_model(&mut assets, &db, FAST, bare, &|_| false);
    assert!(none.is_empty());
    let visor_way = made.iter().find(|part| part.path.last().is_some_and(|(_, of)| of == VISOR)).expect("made with it").path.clone();
    let (visor, hinge) = fit_part(&mut assets, &db, &mut model, 0, &visor_way).expect("the visor hung");
    let name_of_parent = |model: &trp_data::unity::extract::Hierarchy, node: usize| model.nodes[model.nodes[node].parent.expect("hung")].name.clone();
    let made_visor = made.iter().find(|part| part.path == visor_way).expect("made with it");
    assert_eq!(name_of_parent(&model, visor.root), name_of_parent(&whole, made_visor.root), "on the same object");
    assert!(!model.nodes[visor.root].active, "hidden until shown");
    let hinge = hinge.expect("the visor's own hinge");
    assert_eq!((hinge.owner.clone(), hinge.off, hinge.on), (made_hinges[0].owner.clone(), made_hinges[0].off, made_hinges[0].on));
    assert_eq!(model.nodes[hinge.node].name, whole.nodes[made_hinges[0].node].name);
    // The mount, and an adapter on it.
    let slot = db.items[FAST].slots().into_iter().find(|slot| slot.takes.iter().any(|taken| taken == MOUNT)).expect("a slot takes the mount").name;
    let mount_way = vec![(slot, MOUNT.to_owned())];
    let (mount, _) = fit_part(&mut assets, &db, &mut model, 0, &mount_way).expect("the mount hung");
    let below = db.items[MOUNT].slots().into_iter().find(|slot| slot.takes.iter().any(|taken| taken == ADAPTER)).expect("the mount takes the adapter").name;
    let mut adapter_way = mount_way.clone();
    adapter_way.push((below, ADAPTER.to_owned()));
    let (adapter, _) = fit_part(&mut assets, &db, &mut model, mount.root, &adapter_way).expect("the adapter hung");
    let mut at = model.nodes[adapter.root].parent;
    while at.is_some_and(|node| node != mount.root) {
        at = at.and_then(|node| model.nodes[node].parent);
    }
    assert_eq!(at, Some(mount.root), "the adapter in the mount's model");
    // An armour plate hangs nothing; nor a slot with no object.
    let plate = db.items[FAST].slots().into_iter().find_map(|slot| slot.takes.iter().find(|taken| db.items.get(*taken).is_some_and(|item| db.is_a(item, "ArmorPlate"))).map(|taken| (slot.name.clone(), taken.clone())));
    if let Some(plate) = plate {
        assert!(fit_part(&mut assets, &db, &mut model, 0, &[plate]).is_none(), "an armour plate");
    }
    assert!(fit_part(&mut assets, &db, &mut model, 0, &[("no_such_slot".to_owned(), MOUNT.to_owned())]).is_none());
}

/// The face shield's mask (`VisorEffect` on `Cam2`, and the camera's
/// `HysteresisFilter`): the figures as read by hand
/// (`local\visor_shader_s126.md`), the three kinds' masks, and the four
/// pictures read.
#[test]
fn the_face_shield_s_mask_is_read() {
    use trp_data::character::visor::MaskKind;
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let visor = trp_data::character::visor::read(&install, true).expect("the visor's mask");
    // The cracks' material and its picture (`GlassDamageMaterial`'s
    // `_MainTex`): 1024 square, clamped at its edges, the star in its alpha.
    let (material, crack) = visor.crack.as_ref().expect("the cracks' material");
    assert_eq!((crack.name.as_str(), crack.width, crack.height, crack.clamp), ("glassshield_damage2", 1024, 1024, [true, true]), "{material}");
    println!("the cracks: material {material}, picture {} ({}x{}, format {}, {} levels)", crack.name, crack.width, crack.height, crack.format, crack.mip_count);
    assert!(trp_data::character::visor::read(&install, false).expect("without the cracks").crack.is_none());
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

/// A face shield's glass (body-animator spec, "The shield's glass"): the
/// ZSh-1-2M's shield's second material is the dithered glass with the
/// figures as read (`local\visor_glass_s126.md`, its table), its
/// specular picture and its cube (in the `cubemaps` bundle); its frame is
/// not the glass. The welding mask's glass is half see-through everywhere
/// (`_OpacityScale` 0); the window magazine's glass is the same shader.
#[test]
fn the_face_shield_s_glass_is_read() {
    use trp_data::character::dress::{load, Under, Worn};
    use trp_data::drawing::glass::GlassFigures;
    use trp_data::unity::extract::read_material;
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    assets.load("cubemaps").expect("the cubemaps");
    let mut glasses = |template: &str| -> Vec<(String, Option<GlassFigures>, bool, bool)> {
        let key = db.items[template].prefab_key().expect("its prefab").to_owned();
        let Worn::Rigid(model) = load(&mut assets, &key, Under::default()).expect("the model") else { panic!("a model") };
        let mut seen: Vec<(String, Option<GlassFigures>, bool, bool)> = Vec::new();
        for material in model.nodes.iter().filter_map(|node| node.renderer.as_ref()).flat_map(|renderer| renderer.materials.iter().flatten()) {
            let data = read_material(&assets, *material).expect("its material");
            if seen.iter().any(|(name, ..)| *name == data.name) {
                continue;
            }
            seen.push((data.name.clone(), GlassFigures::of(&data), data.textures.contains_key("_SpecTex"), data.textures.contains_key("_Cube")));
        }
        seen
    };
    let zsh = glasses("5aa7e3abe5b5b000171d064d");
    println!("the ZSh-1-2M's shield: {zsh:?}");
    let glass: Vec<_> = zsh.iter().filter(|(_, figures, ..)| figures.is_some()).collect();
    assert_eq!(glass.len(), 1, "one glass");
    let (name, figures, spec, cube) = glass[0];
    let figures = figures.expect("the glass");
    assert_eq!(name, "helmet_zsh_1-2m_glass");
    assert!(*spec && *cube, "its specular picture and its cube");
    let near = |a: f32, b: f32| (a - b).abs() < 0.0006;
    assert!(near(figures.color[0], 0.449) && near(figures.color[3], 1.0), "{:?}", figures.color);
    assert!(near(figures.opacity_scale, 1.0) && near(figures.spec_power, 1.72) && near(figures.glossness, 1.0) && near(figures.global_reflection_strength, 1.13));
    assert!(near(figures.reflect_color[0], 0.301) && near(figures.spec_color[0], 1.0) && figures.cull == 2, "{figures:?}");
    assert!(zsh.iter().any(|(name, figures, ..)| name == "helmet_zsh_1-2m_v1" && figures.is_none()), "the frame is not the glass");
    let welding = glasses("60a7ad3a0c5cb24b0134664a");
    println!("the Gorilla welding mask: {welding:?}");
    let figures = welding.iter().find_map(|(_, figures, ..)| *figures).expect("its glass");
    assert!(near(figures.opacity_scale, 0.0) && near(figures.color[0], 0.726), "{figures:?}");
}

/// A head's mesh under a face cover (`HeadSkin`'s `Data.FaceCover`; body
/// animator spec, "The head's own mesh under a face cover"): of the 55 head
/// bundles the customization list names that the install has, 25 name one,
/// at both their levels of detail (the 10 bosses', the mannequin's, the
/// dealmaker's, `wild_head_nonmesh` and the 17 zombies' name none); each
/// level's `Base` is the mesh its renderer saves, and its two meshes have
/// the same bind poses and as many submeshes. The default BEAR head's, at
/// its fullest: as many vertices, the ears drawn in (98 mm to each side at
/// their height, 91 under a cover) and the neck (70 mm to each side, 64).
#[test]
fn every_head_s_mesh_under_a_face_cover_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let manifest = install.manifest().expect("the manifest");
    let path = install.database_dir().join("templates/customization.json");
    let list: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the customization list")).expect("its json");
    let mut keys: Vec<String> = list
        .as_object()
        .into_iter()
        .flat_map(|all| all.values())
        .filter(|entry| entry.pointer("/_props/BodyPart").and_then(|part| part.as_str()) == Some("Head"))
        .filter_map(|entry| entry.pointer("/_props/Prefab/path").and_then(|path| path.as_str()).filter(|path| !path.is_empty()).map(str::to_owned))
        .collect();
    keys.sort();
    keys.dedup();
    keys.retain(|key| manifest.entries.contains_key(key));
    let mut assets = Assets::new(install).expect("manifest");
    let (mut with, mut without) = (Vec::new(), Vec::new());
    for key in &keys {
        let levels = trp_data::character::arms::load_levels(&mut assets, key).unwrap_or_else(|e| panic!("{key}: {e:#}"));
        let covered = levels.iter().filter(|level| level.covered.is_some()).count();
        assert!(covered == 0 || covered == levels.len(), "{key}: {covered} of {} levels name one", levels.len());
        (if covered == 0 { &mut without } else { &mut with }).push(key.as_str());
        for level in &levels {
            let saved = assets.read(level.renderer).ok().and_then(|r| r.get("m_Mesh").and_then(trp_data::unity::Value::as_pptr)).and_then(|p| assets.resolve_loaded(level.renderer.file, p));
            assert_eq!(saved, Some(level.mesh), "{key}: its Base is the mesh its renderer saves");
            let Some(covered) = level.covered else { continue };
            assert_ne!(covered, level.mesh, "{key}");
            let (base, under) = (read_mesh(&assets, level.mesh).expect("its mesh"), read_mesh(&assets, covered).expect("its mesh under a cover"));
            assert_eq!(base.bind_poses, under.bind_poses, "{key}");
            assert_eq!(base.submeshes.len(), under.submeshes.len(), "{key}");
        }
    }
    println!("{} head bundles: {} with a mesh for under a face cover; without: {without:?}", keys.len(), with.len());
    assert_eq!((keys.len(), with.len()), (55, 25));
    assert!(without.iter().all(|key| ["head_boss_", "head_mannequin", "wild_dealmaker_head", "wild_head_nonmesh", "zombie"].iter().any(|word| key.contains(word))), "{without:?}");

    // The default BEAR head: how far to each side its points lie between
    // two heights (the mesh's own axes: across, back, up), on each mesh.
    let head = trp_data::character::arms::load(&mut assets, "assets/content/characters/character/prefabs/bear_head_0.bundle").expect("the head");
    let (base, under) = (read_mesh(&assets, head.mesh).expect("its mesh"), read_mesh(&assets, head.covered.expect("its other mesh")).expect("its mesh under a cover"));
    assert_eq!((base.positions.len(), under.positions.len()), (2526, 2526));
    let widest = |mesh: &trp_data::unity::extract::MeshData, from: f32, to: f32| mesh.positions.iter().filter(|p| p[2] >= from && p[2] < to).map(|p| p[0].abs()).fold(0.0, f32::max);
    let (ears, neck) = ((1.687, 1.737), (1.56, 1.61));
    println!("the ears' height: {:.4} and {:.4} under a cover; the neck's: {:.4} and {:.4}", widest(&base, ears.0, ears.1), widest(&under, ears.0, ears.1), widest(&base, neck.0, neck.1), widest(&under, neck.0, neck.1));
    assert!(widest(&base, ears.0, ears.1) > 0.097 && widest(&under, ears.0, ears.1) < 0.091);
    assert!(widest(&base, neck.0, neck.1) > 0.069 && widest(&under, neck.0, neck.1) < 0.064);
}
