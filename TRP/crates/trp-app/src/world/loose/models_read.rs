//! Blasted things, blast effects, and the loose models read and prepared.

use super::*;

/// A blast (`Grenade.Explosion`), in the game's axes: a thrown grenade's at
/// its fuse (`blast_view`), or an explosive round's at a hit that sets it off
/// (`ClientGameWorld.ShotDelegate`, `bullets`; underbarrel-launcher spec, "The
/// grenade").
#[derive(Message, Clone)]
pub struct Blasted {
    /// The grenade's item or the round's, whose figures the blast takes.
    pub template: String,
    /// The grenade's place, or the hit's.
    pub at: [f32; 3],
    /// Up for a thrown grenade, the surface's normal for a round's hit: the
    /// way its effect faces and its scorch lies, along which its blast is
    /// moved off (8 cm) and its sound (5 cm).
    pub normal: [f32; 3],
    /// The effect played and its sound's volume (`Effects.EmitGrenade`): a
    /// thrown grenade's `ExplosionEffectType` at 1; a round's `ExplosionType`
    /// at its shot's first hit only, at 1 going in and 0 (no sound) coming
    /// out; none where nothing is played.
    pub effect: Option<(String, f32)>,
    /// It blasts: not a dummy's (`GrenadeComponentIsDummy`: the big rounds'
    /// extra effect alone).
    pub blasts: bool,
    /// A round's, made in the bullets' frame after their tick, where its
    /// fragments are made at once (`bullets::fly_bullets`).
    pub shot: bool,
}

impl Blasted {
    /// A thrown grenade's at its fuse: up, its own effect at 1.
    pub fn thrown(db: &trp_data::Database, template: &str, at: [f32; 3]) -> Self {
        let effect = db.items.get(template).and_then(|item| item.props.get("ExplosionEffectType")?.as_str()).filter(|name| !name.is_empty()).map(|name| (name.to_owned(), 1.0));
        Self { template: template.to_owned(), at, normal: [0.0, 1.0, 0.0], effect, blasts: true, shot: false }
    }

    /// The blast's own place (`Grenade.Explosion`'s shift: 8 cm along the
    /// normal).
    pub fn blast_point(&self) -> [f32; 3] {
        trp_sim::combat::blast::blast_point_along(self.at, self.normal)
    }

    /// Where its effect's sound and light are (`Effect.Emit`: 5 cm along the
    /// normal).
    pub fn sound_point(&self) -> [f32; 3] {
        std::array::from_fn(|k| self.at[k] + self.normal[k] * 0.05)
    }
}

/// The effects a blast can play, read at the start: every grenade's
/// `ExplosionEffectType` and every explosive round's `ExplosionType`, not a
/// class's (`ThrowWeap` names one no grenade has).
pub fn blast_effects(db: &trp_data::Database) -> Vec<String> {
    let classes: std::collections::HashSet<&str> = db.items.values().map(|item| item.parent.as_str()).collect();
    let mut names: Vec<String> = db
        .items
        .values()
        .filter(|item| !classes.contains(item.id.as_str()))
        .filter_map(|item| {
            let grenade = item.props.get("ExplosionEffectType").and_then(|v| v.as_str());
            let round = trp_sim::combat::ballistics::Fuse::of(item).and_then(|fuse| fuse.effect);
            grenade.map(str::to_owned).or(round)
        })
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// A place and turn of the game's as Bevy's.
/// Whether a model's piece drawn with the sights' own shader `lens` is left
/// out: drawn as a plain material it would be an opaque pane of its colour
/// (a red dot's window white, its dot red). In the inspect window's preview
/// (`preview`; `CameraImage` turns the `WeaponPreview` keyword on) the
/// game's optic lens draws nothing; everywhere a collimator's mark seen
/// from off its line falls outside its picture, and the glasses add only a
/// faint glint (`inspect-window.md`, "The scopes' glass"). An optic's lens
/// in the world stays (the game draws it dark), and so does a back lens
/// (black). `ESTIMATE`: the glasses' glint left out for nought.
/// `PROVISIONAL`: the edit build screen is given the preview's rule, where
/// the game draws an optic's lens dark there (its camera has no
/// `CameraImage`).
pub(crate) fn left_out(lens: Option<&str>, preview: bool) -> bool {
    match lens {
        Some("CW FX/Collimator" | "Custom/OpticGlass" | "EFT/CollimatorGlass") => true,
        Some("CW FX/OpticSight") => preview,
        _ => false,
    }
}

pub(super) fn drawn(at: V3, turn: [f32; 4]) -> Transform {
    Transform { translation: convert::position(at), rotation: convert::rotation(turn), ..default() }
}

/// The thread: one model at a time, in the order asked; its colliders
/// with it, a convex mesh's points read with the model.
pub fn read_models(
    mut loose: ResMut<Loose>,
    carried: Res<Carried>,
    (mut meshes, mut images, mut materials, mut shared): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>, ResMut<crate::assets::render_data::SharedPictures>),
) {
    let loose = &mut *loose;
    // A model nothing on the ground draws with, and none on its way, is let
    // go of: its materials, its meshes and its share of the pictures.
    // Looked for only on a frame after something that uses a model has
    // changed (`Uses`), the ground things' keys made again only when what
    // is carried or lies about has.
    loose.uses.keep_keys(carried.last_changed().get(), carried.ground.iter().map(|(item, _)| item));
    if std::mem::take(&mut loose.uses.stale) {
        let ready = loose.models.iter().filter(|(_, asked)| matches!(asked, Asked::Ready(_))).map(|(template, _)| template.as_str());
        let in_use = loose
            .flights
            .iter()
            .map(|f| f.template.as_str())
            .chain(loose.uses.lying(|id| loose.places.contains_key(&id)))
            .chain(loose.held.as_deref())
            .chain(loose.foreseen.key())
            .chain(loose.kept.iter().map(String::as_str));
        for template in uses::unused(ready, in_use) {
            if let Some(Asked::Ready(ready)) = loose.models.remove(&template) {
                shared.release(&ready.marks);
                info!("the model of {template} let go: nothing on the ground draws with it");
            }
        }
    }
    if let Some(task) = loose.task.as_mut() {
        let Some(Read { worker, reader_made, model }) = block_on(poll_once(task)) else { return };
        loose.task = None;
        loose.worker = worker;
        let Some((template, made, took)) = model else {
            // The reader alone, made ahead (`Foreseen`).
            match reader_made {
                Some(made) => info!("the loose models' reader made in {made:.2} s as the inventory opened, and kept while it is open"),
                None => warn!("the loose models' reader could not be made ahead: a model is read when it is asked for"),
            }
            return;
        };
        // (A model come, ready or not to be had: what uses the models is
        // looked through on the next frame.)
        loose.uses.stale = true;
        let read_in = match reader_made {
            Some(reader) => format!("read in {took:.2} s after its reader's {reader:.2} s"),
            None => format!("read in {took:.2} s by the reader kept"),
        };
        let asked = match made {
            Ok(Prepared { template, model, colliders, caliber, convex, hot }) => {
                let Model { hierarchy, looks, parts, extra, shadows, devices, lower, .. } = model;
                // Its levels of detail (`lod_view`), where drawn by.
                let lower_lens: Vec<Option<&'static str>> = lower.iter().flat_map(|own| own.iter().map(|(_, material)| looks.lens(*material))).collect();
                let node_lods: Vec<Option<(usize, u8)>> = (0..hierarchy.nodes.len())
                    .map(|index| hierarchy.nodes[index].lod.filter(|_| crate::assets::lod_view::any_on() && hierarchy.drawn_renderer(index).is_some_and(|renderer| !renderer.skinned && !renderer.shadows_only)).map(|lod| (lod.group, lod.levels)))
                    .collect();
                // Its groups switch together (a weapon's parts with their
                // weapon: `lod_view::LodSpec::bundle`).
                let lod_groups: Vec<(usize, Arc<crate::assets::lod_view::LodSpec>)> = if crate::assets::lod_view::any_on() { hierarchy.lods.iter().map(|lod| lod.node).zip(crate::assets::lod_view::LodSpec::bundle(hierarchy.lods.iter().map(|lod| &*lod.group))).collect() } else { Vec::new() };
                let names: Vec<String> = hierarchy.nodes.iter().map(|node| node.name.clone()).collect();
                let placed = loose::placed(&hierarchy);
                let mut chambers: Vec<(String, usize)> = Vec::new();
                for node in &hierarchy.nodes {
                    if node.name.starts_with("patron_in_weapon") && !chambers.iter().any(|(name, _)| *name == node.name) {
                        chambers.extend(hierarchy.find_below(0, &node.name).map(|at| (node.name.clone(), at)));
                    }
                }
                let magazine = hierarchy
                    .nodes
                    .iter()
                    .position(|node| node.name == "mod_magazine")
                    .and_then(|slot| hierarchy.nodes.iter().position(|node| node.parent == Some(slot)))
                    .unwrap_or(0);
                let points = trp_sim::items::magazine_view::points(hierarchy.nodes.iter().map(|node| (node.parent, node.name.as_str())), magazine);
                let bytes = looks.picture_bytes();
                let games = looks.game_figures();
                let mut lens: Vec<Option<&'static str>> = parts.iter().flat_map(|own| own.iter().map(|(_, material)| looks.lens(*material))).collect();
                lens.extend(extra.iter().map(|_| None));
                let (mesh_handles, material_handles, pictures, kept) = looks.add_sharing(&mut meshes, &mut images, &mut materials, Some(&mut shared));
                let game_of = |material: usize| games.get(material).copied().flatten().map(|(figures, cube)| (figures, cube.map(|at| pictures[at].clone())));
                let nodes = hierarchy
                    .nodes
                    .iter()
                    .enumerate()
                    .map(|(index, node)| {
                        // The first object's own place, turn and scale are
                        // the game's to set (`CreateLootPrefab`).
                        let own = if index == 0 { Transform::IDENTITY } else { convert::transform(node.translation, node.rotation, node.scale) };
                        (node.parent.filter(|_| index > 0), own, node.active)
                    })
                    .collect();
                let mut drawn = Vec::new();
                let mut game = Vec::new();
                let mut slots = Vec::new();
                for (index, own) in parts.iter().enumerate() {
                    drawn.extend(own.iter().map(|(mesh, material)| (index, mesh_handles[*mesh].clone(), material_handles[*material].clone())));
                    game.extend(own.iter().map(|(_, material)| game_of(*material)));
                    slots.extend((0..own.len()).map(Some));
                }
                drawn.extend(extra.iter().map(|(node, mesh, material)| (*node, mesh_handles[*mesh].clone(), material_handles[*material].clone())));
                game.extend(extra.iter().map(|(_, _, material)| game_of(*material)));
                slots.extend(extra.iter().map(|_| None));
                let shadows = shadows.iter().map(|(node, mesh, material)| (*node, mesh_handles[*mesh].clone(), material_handles[*material].clone())).collect();
                let lower: Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>, Option<&'static str>)> = lower
                    .iter()
                    .enumerate()
                    .flat_map(|(index, own)| own.iter().map(move |(mesh, material)| (index, *mesh, *material)))
                    .zip(lower_lens)
                    .map(|((index, mesh, material), lens)| (index, mesh_handles[mesh].clone(), material_handles[material].clone(), lens))
                    .collect();
                info!(
                    "the model of {template} for the world: {} colliders, {} materials, {} of {} pictures already on the card; {:.1} MB of pictures sent; {read_in}",
                    colliders.len(),
                    material_handles.len(),
                    kept.already,
                    kept.marks.len(),
                    bytes.saturating_sub(kept.bytes_spared) as f64 / 1e6
                );
                let round = template.starts_with(ROUND_MODEL).then(|| RoundBody { hierarchy, convex });
                let ready = Ready { nodes, names, drawn, games: game, shadows, lens, slots, hot, caliber, points, chambers, placed, colliders, round, marks: kept.marks, devices: Arc::new(devices), lower, node_lods, lod_groups };
                if crate::assets::lod_view::any_on() {
                    info!("the model of {template}: {}", ready.levels_said());
                }
                Asked::Ready(Arc::new(ready))
            }
            Err(e) => {
                info!("{template} lies as a box: its model could not be had: {e:#}");
                Asked::Failed
            }
        };
        loose.models.insert(template, asked);
    }
    let Some(template) = loose.queue.pop_front() else {
        // Nothing left to read: the thread's reader goes, away from the
        // frame (as the icons' does); but not while a screen that asks for
        // one model after another is open (`keep_reader`: the edit build
        // screen). Made again, the reader took 0.43 s of a pick's 0.70 s
        // there (the AKM's stock), the model itself 0.27 s. Nor while the
        // inventory screen is open (`Foreseen`), which may put a thing
        // down: there it is made as the screen opens, ahead of any model.
        if loose.foreseen.open {
            if loose.worker.is_none() && !loose.foreseen.reader_asked {
                loose.foreseen.reader_asked = true;
                let (install, db) = (loose.install.clone(), loose.db.clone());
                loose.task = Some(AsyncComputeTaskPool::get().spawn(async move {
                    let begun = std::time::Instant::now();
                    let worker = Worker::new(&install, db).ok();
                    Read { reader_made: worker.as_ref().map(|_| begun.elapsed().as_secs_f32()), worker, model: None }
                }));
            }
        } else if !loose.keep_reader {
            if let Some(worker) = loose.worker.take() {
                info!("the loose models' reader let go: nothing left to read, and no screen open that keeps it");
                AsyncComputeTaskPool::get().spawn(async move { drop(worker) }).detach();
            }
        }
        return;
    };
    let (worker, install, db) = (loose.worker.take(), loose.install.clone(), loose.db.clone());
    loose.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        let (begun, kept) = (std::time::Instant::now(), worker.is_some());
        let mut worker = match worker.map_or_else(|| Worker::new(&install, db), Ok) {
            Ok(worker) => worker,
            Err(e) => return Read { worker: None, reader_made: None, model: Some((template, Err(e), 0.0)) },
        };
        let reader_made = (!kept).then(|| begun.elapsed().as_secs_f32());
        let begun = std::time::Instant::now();
        let made = prepare(&mut worker, &template);
        Read { worker: Some(worker), reader_made, model: Some((template, made, begun.elapsed().as_secs_f32())) }
    }));
}

/// A thing's model for the world, and its colliders: its prefab's; or, for
/// a model with none (an outfit of the other game's), the box its meshes
/// fill (`PROVISIONAL`).
/// The key a thrown grenade's model is asked by: its own object's
/// (`GrenadeItself`), not its item's.
pub(super) const GRENADE_MODEL: &str = "thrown grenade ";

/// The key a spent case's model is asked by: its round's, with the round's
/// `shell` objects on and its `full` ones off (`AmmoPoolObject.SetUsed`).
pub(crate) const CASE_MODEL: &str = "spent case ";

/// The key a round's model in a magazine is asked by: its round's, with
/// its `full` objects on and its `shell` ones off (`SetUsed(false)`).
pub(crate) const ROUND_MODEL: &str = "round in a magazine ";

/// The key a thing on the ground is drawn and falls by: a weapon's build
/// (`trp_sim::inventory::rig_key`); rounds as an unspent round, their
/// `shell` objects off (the game's loose 5.45 BT has its case's collider
/// off where the prefab has that object on: `courses2`'s course 1,
/// `docs/spec/physics.md`, "Loose things").
pub(crate) fn model_key(item: &trp_sim::items::inventory::Item) -> String {
    if trp_sim::items::inventory::is(item, "Ammo") {
        format!("{ROUND_MODEL}{}", item.kind.template)
    } else {
        trp_sim::items::inventory::rig_key(item)
    }
}

/// The rounds a thing on the ground shows (`ObjectsFactory.CreateItem`),
/// each on its object of the thing's model, with its kind and whether it
/// is a chamber's: a magazine's as its visual fills its points
/// (`magazine_view::round_at`, as `magazine_rounds` draws them), a
/// weapon's magazine's so, and a weapon's chambered rounds on their
/// slots' objects (`AttachMods`: `SlotView.InsertItem`). A belt's and a
/// cylinder's are not built.
pub(crate) fn shown_rounds(item: &trp_sim::items::inventory::Item, ready: &Ready) -> Vec<(usize, String, bool)> {
    let mut out = Vec::new();
    let magazine = if !item.kind.visible_rounds.is_empty() { Some(item) } else { item.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT).filter(|magazine| !magazine.kind.visible_rounds.is_empty()) };
    if let Some(magazine) = magazine {
        let count = trp_sim::items::inventory::magazine_rounds(magazine);
        for (place, &(node, number)) in ready.points.iter().enumerate() {
            if let Some(kind) = trp_sim::items::magazine_view::round_at(place, number, count).and_then(|from_top| trp_sim::items::inventory::magazine_round_at(magazine, from_top)) {
                out.push((node, kind.template.clone(), false));
            }
        }
    }
    if trp_sim::items::inventory::is(item, "Weapon") {
        for (slot, kind) in item.slots.iter().zip(&item.kind.slots) {
            let Some(round) = slot.as_ref().filter(|_| kind.name.starts_with("patron_in_weapon")) else { continue };
            if let Some((_, node)) = ready.chambers.iter().find(|(name, _)| name.eq_ignore_ascii_case(&kind.name)) {
                out.push((*node, round.kind.template.clone(), true));
            }
        }
    }
    out
}

fn prepare(worker: &mut Worker, template: &str) -> anyhow::Result<Prepared> {
    let mut model = match (template.strip_prefix(GRENADE_MODEL), template.strip_prefix(CASE_MODEL), template.strip_prefix(ROUND_MODEL)) {
        (Some(item), ..) => worker.thrown_grenade(item)?.0,
        (_, Some(round), _) => worker.round_model(round, true)?,
        (.., Some(round)) => worker.round_model(round, false)?,
        _ => worker.model(template, false, &[])?,
    };
    model.looks.unskinned();
    // Marked, to draw with a picture already on the card where it is the
    // same (a rig's, or another thing's).
    model.looks.mark_pictures();
    let unity = &worker.unity;
    let mut convex: HashMap<trp_data::unity::assets::ObjRef, Arc<[V3]>> = HashMap::new();
    let mut points = |mesh| {
        let read: Option<Arc<[V3]>> = trp_data::unity::extract::read_mesh(unity, mesh).ok().filter(|m| !m.positions.is_empty()).map(|m| Arc::from(m.positions));
        if let Some(read) = &read {
            convex.insert(mesh, read.clone());
        }
        read
    };
    let mut colliders = loose::colliders(&model.hierarchy, &mut points);
    if colliders.is_empty() {
        // Each drawn mesh's corners in the first object's axes (Bevy's,
        // then mirrored to the game's).
        let mut world: Vec<Mat4> = Vec::with_capacity(model.hierarchy.nodes.len());
        for (index, node) in model.hierarchy.nodes.iter().enumerate() {
            let own = if index == 0 { Mat4::IDENTITY } else { convert::transform(node.translation, node.rotation, node.scale).to_matrix() };
            world.push(node.parent.filter(|_| index > 0).map_or(Mat4::IDENTITY, |parent| world[parent]) * own);
        }
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let drawn = model.parts.iter().enumerate().flat_map(|(node, own)| own.iter().map(move |(mesh, _)| (node, *mesh))).chain(model.extra.iter().map(|(node, mesh, _)| (*node, *mesh)));
        for (node, mesh) in drawn {
            if let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) = model.looks.meshes[mesh].attribute(Mesh::ATTRIBUTE_POSITION) {
                for p in points {
                    let at = world[node].transform_point3(Vec3::from(*p));
                    (low, high) = (low.min(at), high.max(at));
                }
            }
        }
        anyhow::ensure!(low.x <= high.x, "it has no collider and draws nothing");
        let (centre, half) = ((low + high) / 2.0, ((high - low) / 2.0).max(Vec3::splat(0.005)));
        colliders.push(Collider { shape: Shape::Box { half: half.to_array() }, at: [centre.x, centre.y, -centre.z], turn: [0.0, 0.0, 0.0, 1.0], material: Material::default() });
    }
    let caliber = worker.round_caliber(template.strip_prefix(CASE_MODEL).unwrap_or(template));
    // Its parts that glow with its heat, read as the rig's are (a weapon's
    // `HotObject`s; nothing else has one).
    let hot = trp_data::weapons::heat::read(&worker.unity, &model.hierarchy);
    Ok(Prepared { template: template.to_owned(), model, colliders, caliber, convex, hot })
}
