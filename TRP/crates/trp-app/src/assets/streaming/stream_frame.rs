//! Made pictures taken, and the streaming decided each frame.

use super::*;

/// Copies made away from the frame, taken as they come within what may be
/// on its way to the card at once (`ON_THEIR_WAY`), and the materials
/// drawing with a picture pointed at its copy once the copy is on the card.
///
/// A material changed is readied afresh by the engine, which lets go of
/// the one it had first, and a material that names a picture not yet on
/// the card is not ready and is not drawn (`unready`). The pictures
/// go up to the card a few a frame (`main::warm_up`, `UPLOAD_BUDGET`), so a
/// material pointed at its copy as the copy was added was not drawn until
/// the copy was up: the models flashed out as one walked up to them.
/// Until it is there the material draws with
/// the copy it had, as Unity's streaming draws with the levels it has.
/// Pointed in `PostUpdate`, before the stores tell the renderer what
/// changed (`AssetEventSystems`).
pub fn take_made(mut streaming: ResMut<MipStreaming>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>, warmup: Option<Res<crate::Warmup>>) {
    let streaming = &mut *streaming;
    if !streaming.settings.streaming || streaming.pictures.is_empty() {
        return;
    }
    // Copies on the card now: their materials pointed at them.
    let there: Vec<AssetId<Image>> = ON_CARD.lock().map(|mut lists| std::mem::take(&mut lists.1)).unwrap_or_default();
    if !there.is_empty() {
        let up: Vec<(AssetId<Image>, u32, Handle<Image>)> = streaming
            .pictures
            .iter_mut()
            .filter(|(_, streamed)| streamed.arriving.as_ref().is_some_and(|(_, copy)| there.contains(&copy.id())))
            .filter_map(|(id, streamed)| streamed.arriving.take().map(|(level, copy)| (*id, level, copy)))
            .collect();
        for (id, level, copy) in up {
            // (How long since its raise was decided, with
            // `RUST_LOG=info,trp::assets::streaming=debug`.)
            let clock = streaming.clock;
            if let Some(at) = streaming.pictures.get_mut(&id).and_then(|streamed| streamed.raised_at.take()) {
                debug!("mip streaming at {clock:.2} s: a raise to level {level} on the card {:.3} s after it was decided", clock - at);
            }
            streaming.point_to(&mut materials, id, level, Some(copy));
        }
    }
    // The copies made, taken in while those on their way to the card stay
    // within `ON_THEIR_WAY` with them (one larger alone), the rest on a
    // later frame; all as they come during the warm-up, whose frames are
    // long and not the run's (paced there, the start's pictures of the
    // rifle came 0.1 to 0.3 s later), and with `TRP_STREAM_UNPACED`.
    static UNPACED: OnceLock<bool> = OnceLock::new();
    let paced = !*UNPACED.get_or_init(|| std::env::var_os("TRP_STREAM_UNPACED").is_some()) && !warmup.is_some_and(|warmup| warmup.0);
    let mut on_their_way: usize = streaming.pictures.values().filter_map(|streamed| streamed.arriving.as_ref().map(|(level, _)| streamed.bytes_from(*level))).sum();
    let made: Vec<(AssetId<Image>, u32, Option<Image>)> = streaming
        .pictures
        .iter_mut()
        .filter_map(|(id, streamed)| {
            let level = streamed.pending.as_ref().filter(|(_, task)| task.is_finished())?.0;
            let bytes = streamed.bytes_from(level);
            if paced && on_their_way > 0 && on_their_way + bytes > ON_THEIR_WAY {
                return None;
            }
            let (_, task) = streamed.pending.as_mut()?;
            let image = block_on(poll_once(task))?;
            on_their_way += bytes;
            streamed.pending = None;
            Some((*id, level, image))
        })
        .collect();
    for (id, level, image) in made {
        match image {
            Some(image) => {
                let copy = images.add(image);
                // `TRP_STREAM_AT_ONCE` in the environment points the
                // materials at it at once, as was once done, to set the two
                // beside each other.
                if std::env::var_os("TRP_STREAM_AT_ONCE").is_some() {
                    streaming.point_to(&mut materials, id, level, Some(copy));
                    continue;
                }
                if let Ok(mut lists) = ON_CARD.lock() {
                    lists.0.push(copy.id());
                }
                if let Some(streamed) = streaming.pictures.get_mut(&id) {
                    streamed.arriving = Some((level, copy));
                }
            }
            None => {
                warn!("mip streaming: a picture's levels could not be had again from its bundle; it is left as it is");
                if let Some(streamed) = streaming.pictures.get_mut(&id) {
                    streamed.stuck = true;
                }
            }
        }
    }
}

/// The levels the pictures want, worked out every `DECIDE_EVERY`, and the
/// card's copies brought to them: a picture raised at once (within the
/// upload a decision, the nearest first; its copy made away from the frame
/// and taken as it is ready and there is room on the way to the card,
/// `take_made`), brought down once it has wanted
/// that for `LOWER_AFTER`. A thing drawn is counted whether it is in a
/// camera's sight or not, as long as it is shown (`PROVISIONAL`: Unity's
/// streaming is said to go by distance alone).
pub fn stream(
    time: Res<Time<Real>>,
    mut streaming: ResMut<MipStreaming>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<(&Camera, &GlobalTransform, &Projection, Has<HeldView>, Option<&RenderLayers>)>,
    drawn: Query<(Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform, Option<&Aabb>, &InheritedVisibility, Option<&RenderLayers>)>,
    glass: Query<(Entity, &Mesh3d, &crate::effects::glass_view::GlassDrawn, &GlobalTransform, Option<&Aabb>, &InheritedVisibility, Option<&RenderLayers>)>,
    figure: Query<(Entity, &Mesh3d, &crate::screens::figure_surface::SurfaceDrawn, &GlobalTransform, Option<&Aabb>, &InheritedVisibility, Option<&RenderLayers>)>,
    (names, parents): (Query<&Name>, Query<&ChildOf>),
) {
    let streaming = &mut *streaming;
    if !streaming.settings.streaming || streaming.pictures.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    streaming.clock += dt;
    streaming.since_decided += dt;
    if streaming.since_decided < DECIDE_EVERY {
        return;
    }
    streaming.since_decided = 0.0;
    // Pictures nothing holds or draws with (one made and never drawn).
    let gone: Vec<AssetId<Image>> = streaming.pictures.iter().filter(|(id, streamed)| !streamed.held() && streaming.users.get(*id).is_none_or(HashSet::is_empty)).map(|(id, _)| *id).collect();
    for id in gone {
        streaming.forget(id);
    }
    let settings = streaming.settings;
    let most = settings.most_reduction();
    // Each camera that draws, where it is, its field of view and height;
    // a held view's (`HeldView`) whether it draws or not, for what is on
    // its layers alone, every other for all that is shown.
    let eyes: Vec<(Vec3, f32, f32, Option<RenderLayers>)> = cameras
        .iter()
        .filter(|(camera, _, _, held, _)| camera.is_active || *held)
        .filter_map(|(camera, place, projection, held, layers)| {
            let Projection::Perspective(perspective) = projection else { return None };
            let height = camera.physical_viewport_size()?.y as f32;
            Some((place.translation(), (perspective.fov * 0.5).tan(), height, held.then(|| layers.cloned().unwrap_or_default())))
        })
        .collect();
    // The level each picture wants, and how near what wants it is.
    let mut wanted: HashMap<AssetId<Image>, (f32, f32)> = HashMap::new();
    // What wants each picture most, for the log of what is raised
    // (`RUST_LOG=info,trp::assets::streaming=debug`): the mesh, its
    // distance from the camera beyond its bounds, and its bounds' reach.
    let telling = bevy::log::tracing::enabled!(bevy::log::Level::DEBUG);
    let mut cause: HashMap<AssetId<Image>, (Entity, f32, f32)> = HashMap::new();
    // (A glass drawn with its twin, `glass_view`, and the OVERALL figure's
    // pieces with theirs, `figure_surface`, by the standard material whose
    // pictures the twin draws with.)
    let all = drawn
        .iter()
        .map(|(entity, mesh, material, place, bounds, shown, on)| (entity, mesh, &material.0, place, bounds, shown, on))
        .chain(glass.iter().map(|(entity, mesh, drawn, place, bounds, shown, on)| (entity, mesh, drawn.material(), place, bounds, shown, on)))
        .chain(figure.iter().map(|(entity, mesh, drawn, place, bounds, shown, on)| (entity, mesh, drawn.material(), place, bounds, shown, on)));
    for (entity, mesh, material, place, bounds, shown, on) in all {
        if !shown.get() {
            continue;
        }
        let (Some(density), Some(material)) = (streaming.density.get(&mesh.id()), materials.get(material)) else { continue };
        let (scale, _, _) = place.to_scale_rotation_translation();
        let grow = scale.abs().max_element();
        let centre = bounds.map_or(place.translation(), |bounds| place.transform_point(Vec3::from(bounds.center)));
        let reach = bounds.map_or(0.0, |bounds| Vec3::from(bounds.half_extents).length() * grow);
        let density = density * grow * grow;
        let pictures = [&material.base_color_texture, &material.normal_map_texture, &material.metallic_roughness_texture, &material.occlusion_texture, &material.emissive_texture];
        for (eye, tan_half, height, held) in &eyes {
            if held.as_ref().is_some_and(|layers| !layers.intersects(on.unwrap_or(&RenderLayers::default()))) {
                continue;
            }
            let distance = (centre.distance(*eye) - reach).max(0.0);
            for picture in pictures.iter().copied().flatten() {
                let Some(first) = streaming.family.get(&picture.id()).copied() else { continue };
                let size = streaming.pictures[&first].levels[0].1;
                let texels = size.x as f32 * size.y as f32;
                // Half the log2 of the picture's texels a square world unit
                // over the screen's pixels a square world unit at this
                // distance.
                let level = if distance <= 0.0 {
                    f32::NEG_INFINITY
                } else {
                    0.5 * (texels * distance * distance * tan_half * tan_half / (density * (height * 0.5) * (height * 0.5))).log2()
                } + settings.bias();
                let entry = wanted.entry(first).or_insert((f32::INFINITY, f32::INFINITY));
                if telling && level < entry.0 {
                    cause.insert(first, (entity, distance, reach));
                }
                entry.0 = entry.0.min(level);
                entry.1 = entry.1.min(distance);
            }
        }
    }
    // Whole levels, within what each may be; what nothing draws with goes
    // down to the most it may.
    let mut levels: HashMap<AssetId<Image>, (u32, f32)> = streaming
        .pictures
        .iter()
        .map(|(id, streamed)| {
            let floor = most.min(streamed.deepest);
            let (level, near) = wanted.get(id).map_or((floor, f32::INFINITY), |(level, near)| ((level.floor().max(0.0) as u32).min(floor), *near));
            (*id, (level, near))
        })
        .collect();
    // Within the budget: the farthest given up a level at a time.
    let mut total: u64 = levels.iter().map(|(id, (level, _))| streaming.pictures[id].bytes_from(*level) as u64).sum();
    if total > BUDGET {
        let mut farthest: Vec<AssetId<Image>> = levels.keys().copied().collect();
        farthest.sort_by(|a, b| levels[b].1.total_cmp(&levels[a].1));
        'down: loop {
            let mut moved = false;
            for id in &farthest {
                let streamed = &streaming.pictures[id];
                let (level, _) = levels.get_mut(id).expect("a level");
                if *level < most.min(streamed.deepest) {
                    total -= (streamed.bytes_from(*level) - streamed.bytes_from(*level + 1)) as u64;
                    *level += 1;
                    moved = true;
                    if total <= BUDGET {
                        break 'down;
                    }
                }
            }
            if !moved {
                break;
            }
        }
    }
    // The card's copies brought to them: raised the nearest first, within
    // the upload a frame; brought down after their wait.
    let mut order: Vec<(AssetId<Image>, u32, f32)> = levels.iter().map(|(id, (level, near))| (*id, *level, *near)).collect();
    order.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut sent = 0usize;
    let clock = streaming.clock;
    let mut changed = Vec::new();
    for (id, level, _) in order {
        let streamed = streaming.pictures.get_mut(&id).expect("a picture");
        if streamed.pending.is_some() || streamed.arriving.is_some() || streamed.stuck {
            continue;
        }
        if level < streamed.loaded {
            if sent > 0 && sent + streamed.bytes_from(level) > UPLOAD_A_FRAME {
                continue;
            }
            sent += streamed.bytes_from(level);
            streamed.lower_since = None;
            streamed.raised_at.get_or_insert(clock);
            changed.push((id, level));
        } else if level > streamed.loaded {
            streamed.raised_at = None;
            let since = *streamed.lower_since.get_or_insert(clock);
            if clock - since >= LOWER_AFTER {
                streamed.lower_since = None;
                changed.push((id, level));
            }
        } else {
            streamed.raised_at = None;
            streamed.lower_since = None;
        }
    }

    for (id, level) in changed {
        // Down to the first copy's level, the first again at once; else a
        // new copy, made away from the frame.
        let streamed = streaming.pictures.get_mut(&id).expect("a picture");
        if telling {
            let size = streamed.levels[0].1;
            let (by, distance, reach) = cause.get(&id).map_or(("nothing drawn".to_owned(), f32::INFINITY, 0.0), |(entity, distance, reach)| (said_of(*entity, &names, &parents), *distance, *reach));
            debug!(
                "mip streaming at {clock:.2} s: a picture of {}x{} from level {} to {level} ({:.1} MB on the card from {:.1}), wanted most by {by:?} at {distance:.2} m beyond its bounds of {reach:.2} m",
                size.x,
                size.y,
                streamed.loaded,
                streamed.bytes_from(level) as f64 / (1 << 20) as f64,
                streamed.bytes_from(streamed.loaded) as f64 / (1 << 20) as f64
            );
        }
        if level == most.min(streamed.deepest) {
            streaming.point_to(&mut materials, id, level, None);
        } else {
            streamed.pending = Some((level, streamed.make(level)));
        }
    }

    // (With the first copies kept beside the copies raised from them.)
    let on_card: usize = streaming
        .pictures
        .values()
        .map(|streamed| streamed.bytes_from(streamed.loaded) + if streamed.current.is_some() { streamed.bytes_from(most.min(streamed.deepest)) } else { 0 })
        .sum();
    let whole: usize = streaming.pictures.values().map(|streamed| streamed.bytes_from(0)).sum();
    let kept: usize = streaming.pictures.values().map(|streamed| if let Whole::Kept(data) = &streamed.whole { data.len() } else { 0 }).sum();
    let report = (streaming.pictures.len(), on_card >> 22, whole >> 22);
    if report != streaming.reported {
        streaming.reported = report;
        info!(
            "mip streaming: {} pictures, {} MB of them on the graphics card of {} MB whole ({} MB kept in memory, the rest read again from their bundles); {} added whole, the game not streaming them",
            streaming.pictures.len(),
            on_card >> 20,
            whole >> 20,
            kept >> 20,
            ADDED_WHOLE.load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}

/// A mesh named for the log, with its two nearest named forebears.
fn said_of(entity: Entity, names: &Query<&Name>, parents: &Query<&ChildOf>) -> String {
    let mut said = Vec::new();
    let mut at = Some(entity);
    while let (Some(here), true) = (at, said.len() < 3) {
        said.push(names.get(here).map_or_else(|_| format!("{here}"), |name| name.as_str().to_owned()));
        at = parents.get(here).ok().map(ChildOf::parent);
    }
    said.join(" < ")
}
