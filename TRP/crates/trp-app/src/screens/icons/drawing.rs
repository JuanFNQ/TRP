//! Preparing and drawing icons, and the pictures captured.

use super::*;

/// The icon thread: one model at a time, in the order asked.
pub(super) fn prepare_icons(mut icons: ResMut<Icons>, mut carried: ResMut<Carried>, mut ready: ResMut<crate::screens::inventory_view::PicturesArrived>, mut images: ResMut<Assets<Image>>) {
    let _clock = crate::platform::render_timing::Clock::new("prepare_icons");
    let icons = &mut *icons;
    if let Some(task) = icons.task.as_mut() {
        let Some((worker, (template, _), made)) = block_on(poll_once(task)) else { return };
        icons.task = None;
        icons.worker = worker;
        match made {
            Ok(Made::Model(prepared)) => icons.ready.push_back(prepared),
            // Its prefab's own picture: put on the screen as it is, and
            // kept in the cache as a drawn one is.
            Ok(Made::Own(size, upright, turned)) => {
                if let Some(path) = icons.path(&template, size) {
                    if let Err(e) = trp_data::interface::icon::save_png(&path, size[0], size[1], &upright) {
                        warn!("the picture of {template} is not kept: {e:#}");
                    }
                }
                let picture = picture_of(&mut images, size, upright, turned);
                icons.drawn += 1;
                arrived(carried.bypass_change_detection(), &mut ready, template, Some(picture));
            }
            Err(e) => {
                info!("the picture of {template} is not made: {e:#}");
                icons.failed += 1;
                arrived(carried.bypass_change_detection(), &mut ready, template, None);
            }
        }
    }
    let Some(wanted) = icons.queue.pop_front() else {
        // Nothing left to read: the thread's reader and its bundles go,
        // let go of away from the frame, as the rigs' is (`weapon::
        // prepare_rigs`: freeing one in the frame had cost it 40 ms).
        if let Some(worker) = icons.worker.take() {
            AsyncComputeTaskPool::get().spawn(async move { drop(worker) }).detach();
        }
        return;
    };
    let (worker, install, db) = (icons.worker.take(), icons.install.clone(), icons.db.clone());
    icons.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        match worker.map_or_else(|| Worker::new(&install, db).map(Worker::for_pictures), Ok) {
            Ok(mut worker) => {
                let made = match worker.own_picture(&wanted.0, wanted.1) {
                    Ok(Some((size, upright, turned))) => Ok(Made::Own(size, upright, turned)),
                    Ok(None) => worker.prepare(&wanted.0, wanted.1),
                    Err(e) => Err(e),
                };
                (Some(worker), wanted, made)
            }
            Err(e) => (None, wanted, Err(e)),
        }
    }));
}

/// The studio: a model made ready is put in it and the camera aimed at it;
/// after a few frames the picture is read back (`icon_captured`).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_icons(
    mut commands: Commands,
    mut icons: ResMut<Icons>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut surfaces: ResMut<Assets<crate::rendering::game_surface::GameSurfaceMaterial>>,
    mut cameras: Query<(&mut Camera, &mut Projection, &mut RenderTarget)>,
    waiting: Res<crate::PipelinesWaiting>,
    mut carried: ResMut<Carried>,
    mut ready: ResMut<crate::screens::inventory_view::PicturesArrived>,
    uploads: Res<crate::weapons::weapon::UploadWatch>,
) {
    let _clock = crate::platform::render_timing::Clock::new("draw_icons");
    let icons = &mut *icons;
    // `TRP_ICON_TRACE` in the environment: where the pictures stand, every
    // 60 frames (with `main::count_waiting`'s pipelines still being built,
    // which a picture waits on).
    if std::env::var_os("TRP_ICON_TRACE").is_some() {
        static FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let frame = FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if frame % 60 == 0 {
            let drawing = icons.drawing.as_ref().map(|d| format!("{} frames {} wait {} asked {} tries {}", d.template, d.frames, d.wait, d.asked.is_some(), d.tries));
            info!("icon trace {frame}: queue {} task {} ready {} arriving {} drawing {:?} pipelines waiting {}", icons.queue.len(), icons.task.is_some(), icons.ready.len(), icons.arriving.len(), drawing, waiting.0.load(std::sync::atomic::Ordering::Relaxed));
        }
    }
    // The camera and its lights, once the icon camera is known.
    if icons.camera.is_none() {
        let Some(studio) = icons.ready.front_mut().and_then(|first| first.studio.take()).or_else(|| icons.studio.take()) else { return };
        let layer = RenderLayers::layer(LAYER);
        // The game's camera stands at (0, 0, -1) of its space looking
        // along z; in the engine's axes that is +1 looking along -z. It is
        // put further back: it is orthographic, and nothing of a large
        // thing is then cut off.
        let place = convert::position(studio.position).normalize_or(Vec3::Z) * 10.0;
        let camera = commands
            .spawn((
                Name::new("Icon camera"),
                Camera3d::default(),
                Camera { is_active: false, order: -5, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                Projection::Orthographic(OrthographicProjection { near: 0.0, far: 40.0, ..OrthographicProjection::default_3d() }),
                Transform { translation: place, rotation: convert::rotation(studio.rotation), ..default() },
                Msaa::Sample4,
                // Its samples in half floats, which take the game's numbers
                // as the shader hands them (`game_surface`) and are averaged
                // as they are, as the game's picture is (eight bits and no
                // sRGB in a gamma project); with no tonemap, put as they are
                // on the picture, itself not sRGB. In eight bits as sRGB they
                // were averaged in linear light, which lightened the edges
                // and every fine detail.
                bevy::render::view::Hdr,
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                // No light but its own (`IconRenderSettings`: ambient light
                // black), and an exposure at which a light's intensity is
                // the game's.
                // (Bevy 0.16 copies this into the render world, where it
                // asks for a camera of its own and warns, once, of an
                // entity with a camera and no render graph: that entity is
                // the render world's, and nothing is wrong with this one.)
                AmbientLight { color: Color::WHITE, brightness: 0.0, affects_lightmapped_meshes: false },
                Exposure { ev100: 0.0 },
                layer.clone(),
            ))
            .id();
        for light in &studio.lights {
            // Bevy's diffuse light has a 1/pi the game's has not, and the
            // exposure of ev100 0 takes 1/1.2 of it: so many lux give the
            // game's intensity. Each light's turn is in the camera's space.
            commands.spawn((
                Name::new("Icon light"),
                DirectionalLight { color: Color::srgb(light.color[0], light.color[1], light.color[2]), illuminance: light.intensity * std::f32::consts::PI * 1.2, shadows_enabled: false, ..default() },
                Transform::from_rotation(convert::rotation(studio.rotation) * convert::rotation(light.rotation)),
                layer.clone(),
            ));
        }
        info!("the icon studio: its camera {camera} and {} lights", studio.lights.len());
        // The same lights for the game's own surface, in its numbers (the
        // colour times the intensity), and its eye: the icon camera's place,
        // 1 m before the model's middle (`ItemIconCreator`: the camera at
        // (0, -10, -1), the bounds' middle put at (0, -10, 0)), which the
        // `_Icon` shader takes the way to the eye from.
        icons.surface = Some(crate::rendering::game_surface::SurfaceView {
            way: crate::rendering::game_surface::Way::Picture,
            eye: Some(convert::position(studio.position)),
            lights: studio.lights.iter().map(|light| crate::rendering::game_surface::SurfaceView::light(convert::rotation(studio.rotation) * convert::rotation(light.rotation), light.color, light.intensity)).collect(),
            ambient: [0.0; 3],
        });
        icons.camera = Some(camera);
        icons.studio = Some(studio);
        // The camera is there from the frame's end: aimed in this frame it
        // would not be, and the first picture would be read from a target
        // nothing drew to.
        return;
    }
    let Some(camera) = icons.camera else { return };
    // A model in the studio: once it has stood long enough, its picture is
    // read back.
    if let Some(drawing) = icons.drawing.as_mut() {
        drawing.frames += 1;
        // Not until every mesh and picture of it is on the card: a part
        // whose own are not is left out of the drawing, and the picture
        // kept without it.
        if !drawing.on_card {
            let name = format!("icon {}", drawing.model);
            if uploads.take(|waited| waited == name).is_empty() {
                return;
            }
            if drawing.frames > drawing.old_rule {
                info!("the picture of {}: its model all on the graphics card {} frames after it was put in the studio, where {} had been waited", drawing.template, drawing.frames, drawing.old_rule);
            }
            (drawing.on_card, drawing.frames) = (true, 0);
        }
        if drawing.asked.is_none() && drawing.frames >= drawing.wait && waiting.0.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            drawing.asked = Some(commands.spawn(Screenshot::image(drawing.target.clone())).observe(icon_captured).id());
            drawing.frames = 0;
        } else if drawing.asked.is_some() && drawing.frames > ANSWER_FRAMES_AT_MOST {
            if drawing.tries < 3 {
                warn!("the picture of {} was not read back in {ANSWER_FRAMES_AT_MOST} frames: asked again", drawing.template);
                (drawing.asked, drawing.frames, drawing.tries) = (None, 0, drawing.tries + 1);
            } else {
                warn!("the picture of {} was not read back three times: not made", drawing.template);
                give_up(&mut commands, icons, &mut images, carried.bypass_change_detection(), &mut ready);
                if let Ok((mut camera, ..)) = cameras.get_mut(camera) {
                    camera.is_active = false;
                }
            }
        }
        return;
    }
    let Some(prepared) = icons.ready.pop_front() else { return };
    let PreparedIcon { template, size, hierarchy, keeps_own, looks, parts, extra, turn, place, half_height, picture_bytes, .. } = prepared;
    // (Its pictures whole, as the game draws an item for its picture.)
    let games = looks.game_figures();
    let (mesh_handles, material_handles, pictures) = looks.add_with_pictures(&mut meshes, &mut images, &mut materials);
    // An install's material drawn with the game's own surface, the shader's
    // `_Icon` twin (`game_surface`); the app's own (a weapon of one's own's,
    // the other game's suits) with the standard one's light, in the same
    // numbers. The dithered glass (with `SEE-THROUGH GLASS`) with its own
    // twin, blended over what is behind, and its mesh drawn a second time
    // for the twin's alpha (`cover`).
    let glasses: Vec<bool> = material_handles.iter().map(|standard| crate::effects::glass_view::glass_note(standard).is_some()).collect();
    let surface_handles: Vec<Option<Handle<crate::rendering::game_surface::GameSurfaceMaterial>>> = games
        .iter()
        .zip(&material_handles)
        .map(|(game, handle)| {
            let (standard, view) = (materials.get(handle)?, icons.surface.as_ref()?);
            Some(surfaces.add(match (game, crate::effects::glass_view::glass_note(handle)) {
                (Some((figures, _)), Some(note)) => crate::rendering::game_surface::GameSurface::glass(standard, figures, &note, view),
                (Some((figures, cube)), None) => crate::rendering::game_surface::GameSurface::material(standard, figures, cube.map(|at| pictures[at].clone()), view),
                (None, _) => crate::rendering::game_surface::GameSurface::standard(standard),
            }))
        })
        .collect();
    let cover = glasses.iter().any(|glass| *glass).then(|| surfaces.add(crate::rendering::game_surface::GameSurface::cover()));
    let covered = |material: usize| cover.as_ref().filter(|_| glasses[material] && surface_handles[material].is_some()).cloned();
    let layer = RenderLayers::layer(LAYER);
    let model = commands.spawn((Name::new(format!("Icon of {template}")), Transform { translation: place, rotation: turn, ..default() }, Visibility::Inherited)).id();
    uploads.wait(format!("icon {model}"), pictures.iter().map(Handle::id).collect(), mesh_handles.iter().map(Handle::id).collect());
    let mut entities: Vec<Entity> = Vec::with_capacity(hierarchy.nodes.len());
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        let own = if index == 0 && !keeps_own { Transform::from_scale(Vec3::from(node.scale)) } else { convert::transform(node.translation, node.rotation, node.scale) };
        let visibility = if node.active { Visibility::Inherited } else { Visibility::Hidden };
        let entity = commands.spawn((own, visibility)).id();
        commands.entity(node.parent.map_or(model, |parent| entities[parent])).add_child(entity);
        entities.push(entity);
        for (mesh, material) in &parts[index] {
            let drawn = commands.spawn((Mesh3d(mesh_handles[*mesh].clone()), Transform::default(), layer.clone())).id();
            match &surface_handles[*material] {
                Some(surface) => commands.entity(drawn).insert(MeshMaterial3d(surface.clone())),
                None => commands.entity(drawn).insert(MeshMaterial3d(material_handles[*material].clone())),
            };
            commands.entity(entity).add_child(drawn);
            if let Some(cover) = covered(*material) {
                let alpha = commands.spawn((Mesh3d(mesh_handles[*mesh].clone()), MeshMaterial3d(cover), Transform::default(), layer.clone())).id();
                commands.entity(entity).add_child(alpha);
            }
        }
    }
    for (node, mesh, material) in &extra {
        let drawn = commands.spawn((Mesh3d(mesh_handles[*mesh].clone()), Transform::default(), layer.clone())).id();
        match &surface_handles[*material] {
            Some(surface) => commands.entity(drawn).insert(MeshMaterial3d(surface.clone())),
            None => commands.entity(drawn).insert(MeshMaterial3d(material_handles[*material].clone())),
        };
        commands.entity(entities[*node]).add_child(drawn);
        if let Some(cover) = covered(*material) {
            let alpha = commands.spawn((Mesh3d(mesh_handles[*mesh].clone()), MeshMaterial3d(cover), Transform::default(), layer.clone())).id();
            commands.entity(entities[*node]).add_child(alpha);
        }
    }
    // Drawn at twice its size (`IconCreatorBase`: a texture of double
    // size), onto a picture of its own.
    let (width, height) = (size[0] * 2, size[1] * 2);
    // With nothing in it to send: a picture with something to send waits
    // its turn behind every other's (`main::UPLOAD_BUDGET`), and one not on
    // the card when it is read back is not read at all. The camera clears
    // it before drawing.
    // (Not sRGB: the camera's figures are the game's numbers, put as they
    // are.)
    let mut target = Image::new_uninit(Extent3d { width, height, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8Unorm, bevy::asset::RenderAssetUsages::default());
    target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let target = images.add(target);
    if let Ok((mut aim, mut projection, mut render_target)) = cameras.get_mut(camera) {
        *render_target = RenderTarget::Image(target.clone().into());
        aim.is_active = true;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = ScalingMode::Fixed { width: 2.0 * half_height * size[0] as f32 / size[1] as f32, height: 2.0 * half_height };
        }
    }
    let old_rule = SETTLE_FRAMES + (picture_bytes / OLD_RULE_BYTES_A_FRAME) as u32;
    icons.drawing = Some(Drawing { template, size, model, target, frames: 0, wait: SETTLE_FRAMES, asked: None, tries: 0, on_card: false, old_rule });
}

/// A picture that cannot be made: its model and its target go, and its
/// box keeps the plain shape of its sort.
fn give_up(commands: &mut Commands, icons: &mut Icons, images: &mut Assets<Image>, carried: &mut Carried, ready: &mut crate::screens::inventory_view::PicturesArrived) {
    let Some(drawing) = icons.drawing.take() else { return };
    commands.entity(drawing.model).despawn();
    images.remove(&drawing.target);
    icons.failed += 1;
    arrived(carried, ready, drawing.template, None);
}

/// The picture read back: finished as the game finishes it, put on the
/// screen, written to the cache; the model goes.
fn icon_captured(
    trigger: On<ScreenshotCaptured>,
    mut commands: Commands,
    mut icons: ResMut<Icons>,
    mut carried: ResMut<Carried>,
    mut ready: ResMut<crate::screens::inventory_view::PicturesArrived>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<&mut Camera>,
) {
    let icons = &mut *icons;
    // (An answer to a reading back asked before, and asked again since, is
    // not this picture's.)
    let Some(drawing) = icons.drawing.as_mut().filter(|drawing| drawing.asked == Some(trigger.event().entity)) else { return };
    let captured = &trigger.event().image;
    let Some(data) = captured.data.as_ref() else { return };
    let [width, height] = drawing.size;
    // Nothing drawn yet (a picture of the model's not yet on the card):
    // tried again a few frames on, three times at the most, and then given
    // up, the box keeping the plain shape of its sort. An empty picture is
    // never kept.
    let empty = !data.chunks_exact(4).any(|pixel| pixel[3] > 0);
    if empty && drawing.tries < 3 {
        (drawing.tries, drawing.frames, drawing.asked, drawing.wait) = (drawing.tries + 1, 0, None, SETTLE_FRAMES * 2);
        return;
    }
    if empty {
        warn!("the picture of {} came out empty three times: not made", drawing.template);
        give_up(&mut commands, icons, &mut images, carried.bypass_change_detection(), &mut ready);
        if let Some(mut camera) = icons.camera.and_then(|camera| cameras.get_mut(camera).ok()) {
            camera.is_active = false;
        }
        return;
    }
    // Finished, kept and turned away from the frame: the shadow's blur and
    // the PNG had cost the frame they were made in.
    let shadow = icons.studio.as_ref().map_or_else(|| Studio::as_read(), Clone::clone);
    let (template, model, target) = (drawing.template.clone(), drawing.model, drawing.target.clone());
    let (path, data, cells) = (icons.path(&template, [width, height]), data.clone(), [(width - 1) / CELL_PIXELS, (height - 1) / CELL_PIXELS]);
    icons.arriving.push(AsyncComputeTaskPool::get().spawn(async move {
        let rgba = trp_data::interface::icon::finish(&data, (width * 2) as usize, (height * 2) as usize, shadow.shadow, shadow.shadow_offset);
        if let Some(path) = path {
            if let Err(e) = trp_data::interface::icon::save_png(&path, width, height, &rgba) {
                warn!("the picture of {template} is not kept: {e:#}");
            }
        }
        let turned = trp_data::interface::icon::turned(&rgba, width as usize, height as usize);
        Arrived { template, cells, size: [width, height], drawn: true, made: Ok((rgba, turned)) }
    }));
    commands.entity(model).despawn();
    images.remove(&target);
    icons.drawing = None;
    if let Some(mut camera) = icons.camera.and_then(|camera| cameras.get_mut(camera).ok()) {
        camera.is_active = false;
    }
}
