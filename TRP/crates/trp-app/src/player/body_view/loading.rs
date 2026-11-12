//! Loading the body's parts from the install.

use super::*;

impl BodyParts {
    /// Reads the parts in the bundles named: the upper body's, the
    /// legs', then the head's. One that cannot be read is left out with a
    /// note. All but the legs are the upper body's for drawing: seen from
    /// outside, a shadow in first person.
    ///
    /// `worn` is what the character has on over that, each an item's
    /// template id or a bundle: armour, vests and backpacks are skinned to
    /// the skeleton with the clothing, and what goes on the head is kept
    /// as a model for the head bone. All of it is the upper body's for
    /// drawing (body animator spec, "Worn items"). It is what is worn as
    /// the run begins, by the character and by the dummies: a thing put on
    /// later is read when it is (`worn_reads`): only
    /// what the player sees at start-up is loaded.
    ///
    /// `custom_hands` is the options' switch `CUSTOM FIRST PERSON HANDS` as
    /// the run begins: off, no outfit's arms are read or fitted at all.
    ///
    /// `present` is every thing fitted to what is worn, by its template:
    /// of the parts a model on the head may have fitted, those are read (a
    /// part fitted later is read then: `fit_head_parts`).
    ///
    /// The bones the install's other suits hang on (`SUIT_BONES`) and the
    /// head's are in the skeleton whatever is worn, for what is put on
    /// later.
    ///
    /// `extra` are more bones the skeleton is to have, by their paths (the
    /// slot bones of the weapons on the body: `slung_view`).
    ///
    /// `dummy_looks` are the dummies' own looks, each its upper body's,
    /// legs' and head's bundles as `keys` (none where empty: the
    /// character's is drawn on it there).
    ///
    /// `head_fitting` is the options' `HEAD FITTING` as the run begins:
    /// off, no thing's second mesh, no part's changer and no hood is read.
    ///
    /// `armband` is the options' `ARMBAND` as the run begins: off, an
    /// armband worn is not read, and not drawn.
    ///
    /// With `DISTANCE DETAIL` on (`lod_view::on`), every part's and every
    /// skinned worn thing's levels of detail are read, each level a part of
    /// its own (`lods`, `fullest`).
    #[allow(clippy::too_many_arguments)]
    pub fn load(unity: &mut UnityAssets, db: &trp_data::Database, keys: &[&str], worn: &[String], present: &HashSet<String>, custom_hands: bool, extra: &[String], dummy_looks: &[[String; 3]], head_fitting: bool, armband: bool) -> Self {
        let (mut parts, mut upper, mut items) = (Vec::new(), Vec::new(), Vec::new());
        let (mut of_outfit, mut top, mut suit) = (Vec::new(), Vec::new(), Vec::new());
        let (mut legs, mut head) = (None, None);
        // The levels of detail, where read: each part's group and levels,
        // and its fullest's place.
        let levelled = crate::assets::lod_view::on();
        let mut specs = LevelSpecs { mesh: None, made: Vec::new() };
        if levelled {
            specs.mesh = trp_data::character::player::mesh_transform(unity).map_err(|e| eprintln!("the player's mesh transform could not be read; no part of the body is drawn by its levels of detail: {e:#}")).ok();
        }
        let (mut lods, mut fullest) = (Vec::new(), Vec::new());
        // A part's levels: all of them where read (the fullest first), or
        // the fullest alone.
        let levels_of = |unity: &mut UnityAssets, key: &str| -> anyhow::Result<Vec<trp_data::character::arms::Arms>> {
            if levelled {
                trp_data::character::arms::load_levels(unity, key)
            } else {
                trp_data::character::arms::load(unity, key).map(|part| vec![part])
            }
        };
        // An upper body's hooded form, where it has a hood (`HoodedDress`).
        let mut hoods = Vec::new();
        let hood_of = |unity: &mut UnityAssets, key: &str| -> Option<trp_data::character::arms::Arms> {
            if !head_fitting {
                return None;
            }
            trp_data::character::dress::hood(unity, key).unwrap_or_else(|e| {
                eprintln!("the body's part {key}: its hood could not be read: {e:#}");
                None
            })
        };
        // And each level's hooded form, where the levels are read.
        let hood_levels_of = |unity: &mut UnityAssets, key: &str| -> Vec<trp_data::character::arms::Arms> {
            if !head_fitting || !levelled {
                return Vec::new();
            }
            trp_data::character::dress::hood_levels(unity, key).unwrap_or_else(|e| {
                eprintln!("the body's part {key}: its hood's levels could not be read: {e:#}");
                Vec::new()
            })
        };
        // The hooded form of a level: the one drawn at the same levels.
        let hood_for = |hooded: &[trp_data::character::arms::Arms], part: &trp_data::character::arms::Arms| -> Option<trp_data::character::arms::Arms> {
            let levels = part.lod.as_ref()?.levels;
            hooded.iter().find(|form| form.lod.as_ref().is_some_and(|lod| lod.levels == levels)).cloned()
        };
        let mut bands = Vec::new();
        // The bits a part of a look adds to the head things' fittings
        // (`PlayerBody.SetSkin`'s changer), where it carries one.
        let bits_of = |unity: &mut UnityAssets, key: &str| -> u8 {
            if !head_fitting {
                return 0;
            }
            trp_data::character::dress::part_clipping(unity, key).unwrap_or_else(|e| {
                eprintln!("the body's part {key}: its changer could not be read: {e:#}");
                0
            })
        };
        // A head's mesh for under a face cover (`HeadSkin`; body animator
        // spec, "The head's own mesh under a face cover"): a second form of
        // the head's part, by the head's own bit (`fit::FACE_COVERED`).
        let covered_of = |part: &trp_data::character::arms::Arms| -> Option<trp_data::character::dress::Fitting> {
            let custom = part.covered.filter(|_| head_fitting)?;
            Some(trp_data::character::dress::Fitting { kind: trp_data::character::dress::fit::FACE_COVERED, base: part.mesh, custom })
        };
        let mut head_fittings = Vec::new();
        let mut suit_bits = [0; 3];
        for (index, key) in keys.iter().enumerate().filter(|(_, key)| !key.is_empty() && **key != "none") {
            if let Some(bits) = suit_bits.get_mut(index) {
                *bits = bits_of(unity, key);
            }
            match levels_of(unity, key) {
                Ok(levels) => {
                    let first = parts.len();
                    if index == 1 {
                        legs = Some(first);
                    }
                    if index == 2 {
                        head = Some(first);
                    }
                    let hooded = if index == 0 { hood_levels_of(unity, key) } else { Vec::new() };
                    for (at, part) in levels.into_iter().enumerate() {
                        let hood = if at == 0 { (index == 0).then(|| hood_of(unity, key)).flatten() } else { hood_for(&hooded, &part) };
                        if let Some(hood) = hood {
                            if at == 0 {
                                println!("the body's part {key}: a hood, up or down by what is worn on the head");
                            }
                            hoods.push((parts.len(), hood));
                        }
                        if let Some(fitting) = covered_of(&part) {
                            if at == 0 {
                                println!("the body's part {key}: a head with a mesh for under a face cover");
                            }
                            head_fittings.push((parts.len(), fitting));
                        }
                        lods.push(specs.of(&part));
                        fullest.push(first);
                        parts.push(part);
                        upper.push(index != 1);
                        items.push(None);
                        of_outfit.push(None);
                        top.push(index == 0);
                        suit.push(match index {
                            0 => Some(SuitRole::Upper),
                            1 => Some(SuitRole::Lower),
                            _ => None,
                        });
                    }
                }
                Err(e) => eprintln!("the body's part {key} could not be read: {e:#}"),
            }
        }
        // A vest sits out over armour and a backpack over either, by a
        // mesh of its own for each case. What is worn can change while
        // running, so each item is read for every case, once for each
        // mesh: a backpack read with no armour on is drawn by its mesh over
        // armour once some is put on.
        let of_class = |parent: &str| -> Vec<String> { db.items.iter().filter(|(_, template)| template.parent == parent).map(|(id, _)| id.clone()).collect() };
        // Which things are armour and which vests, of the whole database
        // (what is put on later is told by these); and which armour worn
        // now is worn like a vest, its bundle read (later, as it is read).
        let (armour, vests) = (of_class(ARMOUR), of_class(VEST));
        let vestlike: Vec<String> = worn
            .iter()
            .filter(|item| armour.contains(item))
            .filter(|item| db.items.get(*item).and_then(|template| template.prefab_key()).is_some_and(|key| trp_data::character::dress::vestlike(unity, key).unwrap_or(false)))
            .cloned()
            .collect();
        let cases = ALL_UNDER;
        // An outfit that is not the install's is fitted to the skeleton
        // as the character's own clothing has it at rest: every bone one
        // of those meshes hangs on, with that mesh's own matrix for it.
        // The box about the legs' own mesh at rest is given with it: its
        // top is the waist, under which the character's own eyes see the
        // body (the install draws the legs' mesh for them and no more).
        // Whatever of an outfit cannot be drawn is left to the
        // character's own body and arms.
        let mut outfits = Vec::new();
        let mut their_hands = Vec::new();
        if !db.outfits.is_empty() {
            let mut skeleton: Vec<trp_data::character::outfit::RestBone> = Vec::new();
            let mut waist = None;
            for (index, part) in parts.iter().enumerate() {
                match trp_data::unity::extract::read_mesh(unity, part.mesh) {
                    Ok(mesh) => {
                        for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses).filter(|(path, _)| !skeleton.iter().any(|known| &known.path == *path)).collect::<Vec<_>>() {
                            skeleton.push(trp_data::character::outfit::RestBone { path: path.clone(), bind: *bind });
                        }
                        if legs == Some(index) {
                            waist = trp_data::character::outfit::box_about(&mesh.positions);
                        }
                    }
                    Err(e) => eprintln!("a mesh of the body could not be read for the outfits: {e:#}"),
                }
            }
            let stalker = trp_data::formats::xray::Stalker::locate();
            let mut ids: Vec<&String> = db.outfits.keys().collect();
            ids.sort();
            for id in ids {
                let mut definition = (*db.outfits[id]).clone();
                // (Both kinds of hands: the other game's arms, the install's
                // own bare ones. `|`, that both are taken.)
                if !custom_hands && (definition.arms.take().is_some() | definition.their_hands.take().is_some()) {
                    eprintln!("the outfit {}: its arms for first person are not read (CUSTOM FIRST PERSON HANDS is off)", definition.name);
                }
                let definition = &definition;
                let outfit = trp_data::character::outfit::build(definition, &stalker, &skeleton, waist);
                for note in &outfit.notes {
                    eprintln!("the outfit {}: {note}", definition.name);
                }
                let mut has = Vec::new();
                if outfit.has_body() {
                    has.push(format!("a body of {} meshes on a rig of {} of the body's bones", outfit.pieces.len(), outfit.rig.bones.len()));
                }
                if let Some(arms) = outfit.arms.as_ref().filter(|_| outfit.has_arms()) {
                    has.push(format!("arms for first person, {} meshes on {} bones, at {:.2} times their file's size", arms.pieces.len(), arms.rig.bones.len(), arms.size));
                }
                // The install's own clothing, where the outfit is made of
                // that: skinned to the skeleton as the character's own.
                if let Some(key) = definition.their_top.as_ref().filter(|_| !outfit.has_body()) {
                    match levels_of(unity, key) {
                        Ok(levels) => {
                            let first = parts.len();
                            for part in levels {
                                lods.push(specs.of(&part));
                                fullest.push(first);
                                parts.push(part);
                                upper.push(true);
                                items.push(None);
                                of_outfit.push(Some(id.clone()));
                                top.push(false);
                                suit.push(None);
                            }
                            has.push("one of the install's own tops".to_owned());
                        }
                        Err(e) => eprintln!("the outfit {}: its top {key} could not be read, and the character's own is drawn: {e:#}", definition.name),
                    }
                }
                if let Some(key) = definition.their_hands.as_ref().filter(|_| !outfit.has_arms()) {
                    match trp_data::character::arms::load(unity, key) {
                        Ok(mut hands) => {
                            // What the bundle draws beside them (a
                            // watch's case and glass), each a skin of its own.
                            let beside = std::mem::take(&mut hands.beside);
                            their_hands.push((id.clone(), hands));
                            their_hands.extend(beside.into_iter().map(|skin| (id.clone(), skin)));
                            has.push("the install's own hands for first person".to_owned());
                        }
                        Err(e) => eprintln!("the outfit {}: its hands {key} could not be read, and the character's own are drawn: {e:#}", definition.name),
                    }
                }
                if has.is_empty() {
                    eprintln!("the outfit {} has nothing to draw: worn, the character is as without it", definition.name);
                } else {
                    println!("the outfit {}: {}", definition.name, has.join("; "));
                }
                outfits.push((id.clone(), outfit));
            }
        }
        let mut on_head = Vec::new();
        let mut fittings = head_fittings;
        // (An outfit of one's own has its donor's prefab for a name only:
        // none of the install's meshes is drawn for it.)
        let listed_once = worn.iter().enumerate().filter(|(index, item)| !worn[..*index].contains(item) && !db.outfits.contains_key(*item)).map(|(_, item)| item);
        for item in listed_once {
            let bundle = db.items.get(item).and_then(|template| template.prefab_key()).unwrap_or(item.as_str()).to_owned();
            let first = parts.len();
            // A skin's second form, on the part whose mesh is its first
            // (each level's, where the levels are read).
            let skin_fittings: Vec<trp_data::character::dress::Fitting> = match (head_fitting, levelled) {
                (false, _) => Vec::new(),
                (true, false) => trp_data::character::dress::skin_fitting(unity, &bundle).ok().flatten().into_iter().collect(),
                (true, true) => trp_data::character::dress::skin_fittings(unity, &bundle).unwrap_or_default(),
            };
            for under in &cases {
                match trp_data::character::dress::load(unity, &bundle, *under) {
                    Ok(Worn::Skinned(fullest_skin)) => {
                        // Its levels, where read: the fullest's first.
                        let levels = match levelled {
                            true => trp_data::character::dress::load_levels(unity, &bundle, *under).ok().flatten().unwrap_or_else(|| vec![fullest_skin]),
                            false => vec![fullest_skin],
                        };
                        let mut level_first = None;
                        for part in levels {
                            // The same mesh as for a case before: that piece's too.
                            let same = (first..parts.len()).find(|at| parts[*at].mesh == part.mesh);
                            if let Some(at) = same {
                                level_first.get_or_insert(at);
                            }
                            match same.and_then(|at| items[at].as_mut()) {
                                Some(WornPiece { over, .. }) => over.push(*under),
                                None => {
                                    if let Some(fitting) = skin_fittings.iter().find(|fitting| fitting.base == part.mesh) {
                                        fittings.push((parts.len(), *fitting));
                                    }
                                    let at = parts.len();
                                    lods.push(specs.of(&part));
                                    fullest.push(*level_first.get_or_insert(at));
                                    parts.push(part);
                                    upper.push(true);
                                    items.push(Some(WornPiece { item: item.clone(), over: vec![*under] }));
                                    of_outfit.push(None);
                                    top.push(false);
                                    suit.push(None);
                                }
                            }
                        }
                    }
                    Ok(Worn::Rigid(model)) => {
                        let (model, fitted, hinges) = trp_data::character::dress::head_model(unity, db, item, model, &|part| present.contains(part));
                        if !fitted.is_empty() || !hinges.is_empty() {
                            let named: String = hinges.iter().map(|hinge| format!(", a hinge on {}{}", model.nodes[hinge.node].name, hinge.owner.last().map_or(String::new(), |(_, of)| format!(" (its part {of}'s own)")))).collect();
                            println!("worn on the head, {item}: {} parts it may have fitted{named}", fitted.len());
                        }
                        let fitted_meshes = if head_fitting { trp_data::character::dress::fittings(unity, &model) } else { Vec::new() };
                        if !fitted_meshes.is_empty() {
                            println!("worn on the head, {item}: {} meshes fitted to what else is worn", fitted_meshes.iter().filter(|(node, _)| model.nodes[*node].renderer.is_some()).count());
                        }
                        on_head.push(HeadModel { item: item.clone(), model, parts: fitted, hinges, fittings: fitted_meshes });
                        break;
                    }
                    // An armband: each skin a piece of the body, worn over
                    // anything, its mesh by the upper body (`fit_armbands`).
                    Ok(Worn::Band(band)) => {
                        if !armband {
                            eprintln!("{item} is an armband, not drawn (ARMBAND is off)");
                            break;
                        }
                        println!("worn on the left arm, {item}: an armband of {} skins, a mesh set for {} upper bodies", band.skins.len(), band.sets.len());
                        for (at, skin) in band.skins.into_iter().enumerate() {
                            bands.push((parts.len(), band.sets.iter().filter_map(|(id, meshes)| Some((id.clone(), *meshes.get(at)?))).collect()));
                            // (Its `Skin` group lists no renderer: drawn at
                            // any distance, `trp_data::lod`.)
                            lods.push(None);
                            fullest.push(parts.len());
                            parts.push(skin);
                            upper.push(true);
                            items.push(Some(WornPiece { item: item.clone(), over: Vec::new() }));
                            of_outfit.push(None);
                            top.push(false);
                            suit.push(None);
                        }
                        break;
                    }
                    Err(e) => {
                        eprintln!("{item} is not worn: {e:#}");
                        break;
                    }
                }
            }
        }
        // Each dummy's own look, drawn on it alone.
        let mut dummy_own = vec![None; parts.len()];
        let mut dummy_has = vec![[false; 3]; dummy_looks.len()];
        let mut dummy_bits = vec![[0; 3]; dummy_looks.len()];
        for (which, look) in dummy_looks.iter().enumerate() {
            for (role, key) in look.iter().enumerate().filter(|(_, key)| !key.is_empty()) {
                dummy_bits[which][role] = bits_of(unity, key);
                match levels_of(unity, key) {
                    Ok(levels) => {
                        let first = parts.len();
                        let hooded = if role == LOOK_UPPER { hood_levels_of(unity, key) } else { Vec::new() };
                        for (at, part) in levels.into_iter().enumerate() {
                            let hood = if at == 0 { (role == LOOK_UPPER).then(|| hood_of(unity, key)).flatten() } else { hood_for(&hooded, &part) };
                            if let Some(hood) = hood {
                                if at == 0 {
                                    println!("dummy {which}'s own {key}: a hood, up or down by what it wears on the head");
                                }
                                hoods.push((parts.len(), hood));
                            }
                            if let Some(fitting) = covered_of(&part) {
                                if at == 0 {
                                    println!("dummy {which}'s own {key}: a head with a mesh for under a face cover");
                                }
                                fittings.push((parts.len(), fitting));
                            }
                            lods.push(specs.of(&part));
                            fullest.push(first);
                            parts.push(part);
                            upper.push(role != LOOK_LOWER);
                            items.push(None);
                            of_outfit.push(None);
                            top.push(role == LOOK_UPPER);
                            suit.push(None);
                            dummy_own.push(Some((which, role)));
                        }
                        dummy_has[which][role] = true;
                    }
                    Err(e) => eprintln!("dummy {which}'s own {key} could not be read, and the character's is drawn on it: {e:#}"),
                }
            }
        }
        // Every bone a mesh hangs on that is not an arm's, and every bone
        // above one of those. The collarbones are wanted whatever the
        // meshes say: the arms' own mesh hangs on them too.
        let mut bones: Vec<String> = Vec::new();
        let ribcage = parts.iter().flat_map(|part| &part.bone_paths).find_map(|path| path.find(RIBCAGE).map(|at| &path[..at]));
        let mut wanted: Vec<String> =
            ribcage.into_iter().flat_map(|above| COLLARBONES.iter().map(move |collarbone| format!("{above}{collarbone}"))).collect();
        // What rides on the head needs the head's bone, whether or not a
        // head is drawn: the neck hangs beside the ribcage. Wanted whatever
        // is worn now, as the suits' bones are: a thing put on later hangs
        // on them (`worn_reads`).
        wanted.extend(ribcage.map(|above| format!("{above}/Base HumanNeck/{HEAD}")));
        wanted.extend(SUIT_BONES.map(str::to_owned));
        wanted.extend(extra.iter().cloned());
        let of_outfits = outfits.iter().flat_map(|(_, outfit)| outfit.rig.bones.iter().chain(outfit.arms.iter().flat_map(|arms| &arms.rig.bones)).map(|bone| &bone.path));
        let of_hands = their_hands.iter().flat_map(|(_, hands)| &hands.bone_paths);
        for path in parts.iter().flat_map(|part| &part.bone_paths).chain(&wanted).chain(of_outfits).chain(of_hands).filter(|path| !of_an_arm(path)) {
            for (end, _) in path.match_indices('/').chain(std::iter::once((path.len(), ""))) {
                if !bones.iter().any(|known| known == &path[..end]) {
                    bones.push(path[..end].to_owned());
                }
            }
        }
        bones.sort_by_key(|path| path.matches('/').count());
        // The character model's: the same and the arms, less what only the
        // character's own eyes see (an outfit's arms for them).
        let mut standing: Vec<String> = Vec::new();
        let of_bodies = outfits.iter().flat_map(|(_, outfit)| &outfit.rig.bones).map(|bone| &bone.path);
        for path in parts.iter().flat_map(|part| &part.bone_paths).chain(&wanted).chain(of_bodies) {
            for (end, _) in path.match_indices('/').chain(std::iter::once((path.len(), ""))) {
                if !standing.iter().any(|known| known == &path[..end]) {
                    standing.push(path[..end].to_owned());
                }
            }
        }
        standing.sort_by_key(|path| path.matches('/').count());
        Self {
            parts,
            upper,
            bones,
            on_head,
            item: items,
            armour,
            vests,
            vestlike,
            suit,
            outfits,
            of_outfit,
            top,
            their_hands,
            standing,
            standing_pose: None,
            overall: None,
            dummy_pose: None,
            head,
            dummy_own,
            dummy_has,
            fitting: head_fitting,
            fittings,
            suit_bits,
            dummy_bits,
            hoods,
            armband,
            bands,
            look_keys: std::array::from_fn(|role| keys.get(role).map_or(String::new(), |key| key.to_string())),
            dummy_keys: dummy_looks.to_vec(),
            body_ids: HashMap::new(),
            lods,
            fullest,
        }
    }
}
