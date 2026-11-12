//! Tests that read bots' looks, watches, suits, worn items, armbands and holsters.

use super::*;

/// A PMC bot's looks of each side as SPT's server dresses one (`bots/types/
/// usec.json`, `bear.json`: the range's dummies, body-hits spec, "The dummy
/// on the range"): every head, body and legs a weight of nought or more
/// (nought: never picked but at a draw of nought, `bots::weighted`) and a
/// bundle that reads as a part of a body.
#[test]
fn a_pmc_bots_looks_are_read_and_each_part_reads() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let customizations = trp_data::character::suits::Customizations::load(&install).expect("the customizations");
    assert_eq!(customizations.bundle("5cde95d97d6c8b647a3769b0").as_deref(), Some("assets/content/characters/character/prefabs/tshirt_usec_cryeac.bundle"));
    let mut assets = Assets::new(install.clone()).expect("manifest");
    for side in ["usec", "bear"] {
        let looks = trp_data::character::suits::bot_looks(&install, side).expect("the side's looks");
        println!("{side}: {} heads, {} bodies, {} legs", looks.head.len(), looks.body.len(), looks.feet.len());
        assert!(looks.head.len() >= 4 && looks.body.len() >= 10 && looks.feet.len() >= 10);
        for (id, weight) in looks.head.iter().chain(&looks.body).chain(&looks.feet) {
            assert!(*weight >= 0.0, "{side} {id}: weight {weight}");
            let key = customizations.bundle(id).unwrap_or_else(|| panic!("{side} {id}: no bundle"));
            trp_data::character::arms::load(&mut assets, &key).unwrap_or_else(|e| panic!("{side} {id} {key}: {e:#}"));
        }
    }
}

/// The watches of the install's hands entries (body animator spec, "The
/// watch on the wrist"): the default BEAR hands' where the entry puts it,
/// and every watch's prefab read, on the left forearm's last bone, with
/// its three hands and a tick that ends on the next second.
#[test]
fn every_hands_watch_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let places = trp_data::character::watch::places(&install).expect("the watches");
    let bear = &places["assets/content/hands/bear/bear_hands_skin.bundle"];
    assert_eq!(bear.prefab, "assets/content/hands/bear/bear_watch.bundle");
    assert_eq!(bear.position, [-0.0376, 0.00526, 0.03789]);
    assert_eq!(bear.rotation, [-12.49942, -11.33493, 1.457913]);
    let mut assets = Assets::new(install).expect("manifest");
    // The BEAR hands' third material is the watch's glass, and only it.
    let hands = trp_data::character::arms::load(&mut assets, "assets/content/hands/bear/bear_hands_skin.bundle").expect("the BEAR hands");
    assert_eq!(hands.glass, [false, false, true]);
    assert!(hands.beside.is_empty());
    // Every hands with a watch has its glass, once, on the hands' own skin
    // or on a skin of the bundle's beside it (the watch's case and glass
    // apart on four: the Phoenix's two, the Nord's, the Instructor's).
    let mut apart = Vec::new();
    for key in places.keys() {
        let hands = trp_data::character::arms::load(&mut assets, key).unwrap_or_else(|e| panic!("{key}: {e:#}"));
        let glasses = std::iter::once(&hands).chain(&hands.beside).flat_map(|skin| &skin.glass).filter(|glass| **glass).count();
        assert_eq!(glasses, 1, "{key}");
        if !hands.beside.is_empty() {
            assert!(hands.beside.iter().all(|skin| skin.bone_paths.iter().all(|path| path.contains("Base HumanLForearm"))), "{key}: a skin beside the hands off the left forearm");
            apart.push(key.rsplit('/').next().unwrap_or(key).to_owned());
        }
    }
    apart.sort();
    println!("the watch's case a skin of its own: {apart:?}");
    assert_eq!(apart, ["hands_bear_instructor.skin.bundle", "hands_bear_nord.skin.bundle", "hands_bear_phoenix.skin.bundle", "hands_bear_phoenix_arena_bp_01.skin.bundle"]);
    // Two hands with no watch's hands draw a digital watch beside them: the
    // Clawgear's Suunto, the Special Ops' G-Shock (its glass not drawn).
    for (key, glasses) in [("assets/content/hands/hands_usec_clawgear/hands_usec_clawgear.skin.bundle", 0), ("assets/content/hands/hands_usec_special_ops/hands_usec_special_ops.skin.bundle", 1)] {
        let hands = trp_data::character::arms::load(&mut assets, key).unwrap_or_else(|e| panic!("{key}: {e:#}"));
        assert!(!hands.beside.is_empty(), "{key}");
        assert_eq!(std::iter::once(&hands).chain(&hands.beside).flat_map(|skin| &skin.glass).filter(|glass| **glass).count(), glasses, "{key}");
    }
    // A body's list is levels of detail: nothing beside its fullest.
    let shirt = trp_data::character::arms::load(&mut assets, "assets/content/characters/character/prefabs/tshirt_bear_turtleneck.bundle").expect("the BEAR shirt");
    assert!(shirt.beside.is_empty());
    let prefabs: std::collections::BTreeSet<&str> = places.values().map(|place| place.prefab.as_str()).collect();
    println!("{} hands with a watch, {} watches", places.len(), prefabs.len());
    assert!(prefabs.len() >= 5);
    for prefab in prefabs {
        let watch = trp_data::character::watch::load(&mut assets, prefab).unwrap_or_else(|e| panic!("{prefab}: {e:#}"));
        assert!(watch.bone_path.ends_with("Base HumanLForearm2/Base HumanLForearm3"), "{prefab}: {}", watch.bone_path);
        let names = watch.hands.map(|at| watch.hierarchy.nodes[at].name.to_lowercase());
        println!("{prefab}: {names:?}, {} keys of tick", watch.tick.keys.len());
        assert!(names[0].contains("hour") && names[1].contains("minute") && names[2].contains("sec"), "{prefab}: {names:?}");
        assert!(watch.hands.iter().all(|&at| watch.hierarchy.nodes[at].renderer.is_some()), "{prefab}: a hand draws nothing");
        assert!((watch.tick.evaluate(0.0)).abs() < 1e-3 && (watch.tick.evaluate(1.0) - 1.0).abs() < 1e-3, "{prefab}: its tick");
    }
}

/// Every clothing suit of the install, of every side, with its parts'
/// bundles, each of which reads (inventory spec, "The OVERALL tab").
#[test]
fn every_suit_and_its_parts_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let locale = trp_data::character::inventory::Locale::read(&install, "en").expect("the install's texts");
    let suits = trp_data::character::suits::suits(&install, &locale).expect("the suits");
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
        let part = trp_data::character::arms::load(&mut assets, key).unwrap_or_else(|e| panic!("{key}: {e:#}"));
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
        let part = trp_data::character::arms::load(&mut assets, key).expect("read above");
        paths.extend(part.bone_paths.into_iter().filter(|path| !paths.contains(path)).collect::<Vec<_>>());
    }
    let wanted: Vec<&str> = paths.iter().map(String::as_str).collect();
    let bones = trp_data::character::body::bones(&mut assets, &wanted).expect("every suit's bone on the player's skeleton");
    println!("{} bones under the suits' bodies and legs", bones.len());
    // And every suit's hands on the bones a weapon's rig has, as the
    // default hands are (`arms_bind_to_bones_the_weapon_has`).
    if let Some((mut assets, weapon)) = akm() {
        let rig: Vec<String> = (0..weapon.hierarchy.nodes.len()).map(|i| weapon.hierarchy.path(i)).collect();
        for key in suits.iter().filter_map(|suit| suit.hands.as_ref()) {
            let hands = trp_data::character::arms::load(&mut assets, key).unwrap_or_else(|e| panic!("{key}: {e:#}"));
            for path in hands.weapon_bone_paths() {
                assert!(rig.iter().any(|p| p == path), "{key}: the weapon has no bone {path}");
            }
        }
    }
    // The OVERALL tab's own figures, as the inventory's scene has them.
    let overall = trp_data::interface::screen::OverallPanel::load(&install, "level44").expect("the OVERALL panel");
    let camera = overall.camera.expect("the model view's camera");
    println!("the OVERALL tab: camera {camera:?}, turn {:?}, most seen {:?}, highlight {:?}, line tints {:?}, sides {:?}", overall.turn_a_pixel, overall.dropdown_most_seen, overall.dropdown_highlight, overall.line_tints, overall.side_sprites);
    assert_eq!((camera.fov, camera.near, camera.far), (28.0, 0.3, 25.0));
    assert!((camera.place[1] + 0.2).abs() < 1e-3 && (camera.place[2] - 0.49).abs() < 1e-3);
    assert_eq!(overall.turn_a_pixel, Some(0.3));
    // The game's eight lights its camera switches on (`PlayerMVObjectLights`:
    // six points about the figure and two directional), read once with the
    // panel, and its PRISM's filmic tonemap alone.
    for light in &overall.lights {
        println!("light {}: kind {} on {} colour {:?} intensity {} range {} at {:?} turned {:?}", light.name, light.kind, light.on, light.color, light.intensity, light.range, light.position, light.rotation);
    }
    assert_eq!(overall.lights.len(), 8, "the camera's eight lights");
    assert_eq!((overall.lights.iter().filter(|l| l.kind == 2).count(), overall.lights.iter().filter(|l| l.kind == 1).count()), (6, 2), "six points and two directional");
    assert!(overall.lights.iter().all(|l| l.on), "each on as the camera draws");
    let main = overall.lights.iter().find(|l| l.name == "Main Light").expect("the warm key");
    // At its own place scaled by its parent's (2, 2, 1): before the figure
    // standing 5 m on, at its head's height.
    assert!((main.position[0] + 0.5601).abs() < 1e-3 && (main.position[1] - 0.662).abs() < 1e-3 && (main.position[2] - 4.858).abs() < 1e-3, "{:?}", main.position);
    let prism = overall.prism.as_ref().expect("the camera's PRISM");
    println!("its PRISM: {prism:?}");
    assert!(prism.use_tonemap && prism.tonemap_type == 0 && !prism.use_exposure && !prism.use_vignette && !prism.use_bloom);
    assert_eq!((prism.tone, prism.secondary_tone), ([7.0, 0.3, 1.0], [0.004, 0.06, 0.0]));
    assert_eq!(overall.dropdown_most_seen, Some(120.0));
    assert!(overall.side_sprites.iter().all(Option::is_some));
    for (index, key, case) in &overall.localized {
        println!("localized {}: {key:?} case {case}, text {:?}", overall.panel.nodes[*index].path, overall.panel.nodes[*index].text.as_ref().map(|t| (&t.text, t.color, t.size)));
    }
}

/// What a character wears is read from the item's own bundle in one of
/// two forms: a skin for the player's skeleton (armour, vests, backpacks,
/// the last with a mesh for each thing it may be worn over) or a model
/// for the head bone (a helmet).
#[test]
fn worn_items_are_skins_or_models_for_the_head() {
    use trp_data::character::dress::{load, Under, Worn};
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

/// The bones a weapon not in hand hangs on (`PlayerBones`), and the default
/// legs' holster (`LegsView`): every slot's bone found in the player's
/// skeleton; the BEAR trousers' holster on a leg, its turn a unit one.
#[test]
fn the_slot_bones_and_the_legs_holster_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let bones = trp_data::character::body::slot_bones(&mut assets).expect("the slot bones");
    eprintln!("{bones:#?}");
    for (name, path) in [("primary", &bones.primary), ("secondary", &bones.secondary), ("pistol", &bones.pistol), ("pistol_left", &bones.pistol_left), ("scabbard", &bones.scabbard)] {
        assert!(path.as_deref().is_some_and(|path| path.starts_with("Root_Joint/")), "{name}: {path:?}");
    }
    let holster = trp_data::character::arms::legs_holster(&mut assets, "assets/content/characters/character/prefabs/pants_bear_srvv.bundle").expect("read");
    eprintln!("{holster:?}");
    let holster = holster.expect("the BEAR trousers carry a holster");
    let length = holster.rotation.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!((length - 1.0).abs() < 1e-3, "{length}");
}

/// The armband, the hood and a lower suit's holster (body animator spec,
/// "The armband, the hood and a suit's holster"). Every armband template's
/// bundle reads as an armband: one skin on the left upper arm, a mesh set
/// for each default upper body (BEAR's, USEC's), no id listed twice, every
/// mesh skinned to the skin's bones; the two upper bodies that share a
/// bundle have the same meshes in every armband. Of the upper bodies the
/// customization list names, the cultist top alone has a hood, its hooded
/// form a torso of the same bones and material with its three meshes its
/// own. The hood's rule on it: down under the USEC cap, up with nothing on
/// the head or a plain balaclava, down with a shemagh (conflicting items)
/// or the skull mask (it blocks the headwear slot). The starting legs'
/// holster on the right leg at its place; the lower suits without a
/// `LegsView`, on which no pistol is drawn.
#[test]
fn the_armbands_the_hood_and_the_suits_holsters_are_read() {
    use trp_data::character::dress::{hood, hood_up, load, Under, Worn};
    const BEAR: &str = "5cc0858d14c02e000c6bea66";
    const USEC: &str = "5cde95d97d6c8b647a3769b0";
    const CULTIST: &str = "assets/content/characters/character/prefabs/top_pmc_cultist.bundle";
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let body_ids = trp_data::character::suits::Customizations::load(&install).expect("the customizations").body_ids();
    let mut assets = Assets::new(install.clone()).expect("manifest");

    let mut keys: Vec<String> = db.items.values().filter(|item| db.is_a(item, "ArmBand")).filter_map(|item| item.prefab_key().map(str::to_owned)).collect();
    keys.sort();
    keys.dedup();
    let mut without = Vec::new();
    let (mut meshes_read, mut fewer) = (std::collections::HashSet::new(), 0);
    for key in &keys {
        let Worn::Band(band) = load(&mut assets, key, Under::default()).expect(key) else { panic!("{key} is an armband") };
        assert_eq!(band.skins.len(), 1, "{key}");
        assert!(band.skins[0].bone_paths.iter().any(|path| path.ends_with("Base HumanLUpperarm")), "{key}: {:?}", band.skins[0].bone_paths);
        let mut ids: Vec<&str> = band.sets.iter().map(|(id, _)| id.as_str()).collect();
        ids.sort();
        let listed = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), listed, "{key}: an id listed twice");
        for id in [BEAR, USEC] {
            assert!(band.sets.iter().any(|(of, _)| of == id), "{key}: no set for {id}");
        }
        // The first set, which a body with none takes: the `Body`
        // category's id.
        assert_eq!(band.sets.first().map(|(id, _)| id.as_str()), Some("5cc0868e14c02e000c6bea68"), "{key}");
        for (id, meshes) in &band.sets {
            assert_eq!(meshes.len(), band.skins.len(), "{key} {id}");
            for &mesh in meshes {
                if !meshes_read.insert((key.clone(), mesh)) {
                    continue;
                }
                let read = read_mesh(&assets, mesh).expect("an armband's mesh");
                // Some have a bind pose fewer than the skin has bones: no
                // point of theirs is moved by a bone past their poses.
                let poses = read.bind_poses.len();
                assert!(poses <= band.skins[0].bone_paths.len(), "{key} {id}");
                fewer += usize::from(poses < band.skins[0].bone_paths.len());
                let moved = read.joints.iter().zip(&read.weights).flat_map(|(joints, weights)| joints.iter().zip(weights).filter(|(_, w)| **w > 0.0).map(|(j, _)| *j as usize)).max().unwrap_or(0);
                assert!(moved < poses, "{key} {id}: bone {moved} of {poses}");
                assert_eq!(read.submeshes.len(), band.skins[0].materials.len(), "{key} {id}");
            }
        }
        // The one bundle two upper bodies share, the Black Knight's top:
        // the boss's own id has no set (the first's then), the quest suit's
        // has one. A body known by its bundle alone is taken as its first
        // id; a suit chosen carries its own.
        let shared: Vec<&Vec<String>> = body_ids.values().filter(|ids| ids.len() > 1).collect();
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].iter().map(|id| band.sets.iter().any(|(of, _)| of == id)).collect::<Vec<_>>(), [false, true], "{key}: {:?}", shared[0]);
        without.push(body_ids.values().flatten().filter(|id| !band.sets.iter().any(|(of, _)| of == *id)).count());
    }
    eprintln!("{} armband bundles, {} meshes ({fewer} with a bind pose fewer than the skin's bones); the upper bodies each has no set for (the first set's meshes then): {without:?}", keys.len(), meshes_read.len());
    assert_eq!((keys.len(), meshes_read.len(), fewer), (26, 1453, 338));
    assert_eq!(body_ids.values().map(Vec::len).sum::<usize>(), 151);
    assert!(without.iter().all(|&none| none == 63 || none == 76), "{without:?}");

    let mut hooded = Vec::new();
    let mut bundles: Vec<&String> = body_ids.keys().collect();
    bundles.sort();
    let manifest = install.manifest().expect("the manifest");
    for bundle in bundles.into_iter().filter(|key| manifest.entries.contains_key(*key)) {
        if hood(&mut assets, bundle).expect(bundle).is_some() {
            hooded.push(bundle.clone());
        }
    }
    assert_eq!(hooded, [CULTIST]);
    let part = trp_data::character::arms::load(&mut assets, CULTIST).expect("the cultist top");
    let up = hood(&mut assets, CULTIST).expect("read").expect("its hood");
    assert_eq!(up.bone_paths, part.bone_paths);
    assert_eq!(up.materials, part.materials);
    let (down_torso, up_torso) = (part.torso.expect("a torso of the newer kind"), up.torso.expect("its hood's too"));
    for case in trp_data::character::arms::Torso::ALL {
        let (down, up) = (down_torso.mesh(case).expect("the part's mesh"), up_torso.mesh(case).expect("the hood's mesh"));
        assert_ne!(down, up, "{case:?}");
    }

    let face_cover = |template: &str| -> (Vec<String>, Vec<String>) {
        let item = &db.items[template];
        let conflicting = item.props.get("ConflictingItems").and_then(|list| list.as_array()).into_iter().flatten().filter_map(|id| id.as_str().map(str::to_owned)).collect();
        (conflicting, item.blocked_slots())
    };
    let up_with = |template: &str| {
        let (conflicting, blocks) = face_cover(template);
        hood_up(false, Some((&conflicting, &blocks)))
    };
    assert!(!hood_up(true, None), "the USEC cap");
    assert!(db.is_a(&db.items["5aa2a7e8e5b5b00016327c16"], "Headwear"));
    assert!(hood_up(false, None), "nothing on the head");
    assert!(up_with("572b7f1624597762ae139822"), "a balaclava");
    assert!(!up_with("5ab8f85d86f7745cd93a1cf5"), "the shemagh");
    assert!(!up_with("5b432b6c5acfc4001a599bf0"), "the skull mask");
    let covers: Vec<&String> = db.items.iter().filter(|(_, item)| db.is_a(item, "FaceCover")).map(|(id, _)| id).collect();
    let lowering = covers.iter().filter(|id| !up_with(id)).count();
    eprintln!("{} face covers, {lowering} of which bring the hood down", covers.len());
    assert_eq!((covers.len(), lowering), (120, 65));

    let list: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(install.database_dir().join("templates/customization.json")).expect("the list")).expect("json");
    let mut legs: Vec<String> = list
        .as_object()
        .into_iter()
        .flat_map(|all| all.values())
        .filter(|entry| entry.pointer("/_props/BodyPart").and_then(|part| part.as_str()) == Some("Feet"))
        .filter_map(|entry| entry.pointer("/_props/Prefab/path").and_then(|path| path.as_str()).filter(|path| !path.is_empty()).map(str::to_owned))
        .filter(|key| manifest.entries.contains_key(key))
        .collect();
    legs.sort();
    legs.dedup();
    let (mut right, mut left, mut none) = (0, Vec::new(), Vec::new());
    for key in &legs {
        match trp_data::character::arms::legs_holster(&mut assets, key).expect(key) {
            Some(holster) if holster.right => right += 1,
            Some(_) => left.push(key.clone()),
            None => none.push(key.clone()),
        }
    }
    eprintln!("{} legs bundles: holster right {right}, left {left:?}, none {}: {none:?}", legs.len(), none.len());
    assert_eq!((legs.len(), right, left.len(), none.len()), (112, 57, 1, 54));
    assert!(none.iter().any(|key| key.ends_with("/pants_usec_shorts.bundle")));
    let start = trp_data::character::arms::legs_holster(&mut assets, "assets/content/characters/character/prefabs/pants_bear_srvv.bundle").expect("read").expect("the BEAR trousers carry a holster");
    assert!(start.right);
    for (got, want) in start.position.iter().zip([0.3258, 0.0947, 0.167]) {
        assert!((got - want).abs() < 5e-4, "{:?}", start.position);
    }
}
