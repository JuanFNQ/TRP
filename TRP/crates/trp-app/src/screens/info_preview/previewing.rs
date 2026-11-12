//! The inspect window's preview turned and drawn.

use super::*;

/// The drag and the wheel on a preview (`ItemInfoWindowLabels.Rotate` and
/// `Zoom`): a drag begun on the preview of the window on top there turns
/// its model by the pointer's travel (`Rotate(dx, dy, 0, 0)`: the yaw back
/// by 0.3 of a degree a pixel across, the tilt by as much up, its sense as
/// the model faces, its tilt not held); the wheel over it zooms. The turn
/// takes each frame's travel from the frame the drag begins, the drag
/// threshold from the press (`DragTrigger.onDrag`: each `OnDrag`'s own
/// `delta`, the last frame's move): what the pointer went before that
/// never turns it (`courses10`'s `weapon-inspect-menu`: 3 pixels at frame
/// 781, the drag begun at 782, 7 out).
pub(super) fn turn(
    mut previews: ResMut<InfoPreviews>,
    controller: Res<crate::player::controller::Controller>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    window: Query<&Window, With<PrimaryWindow>>,
    pictures: Query<(&InfoPicture, &RelativeCursorPosition)>,
    covers: Query<(&crate::screens::inventory_view::Cover, &RelativeCursorPosition)>,
) {
    // (Written past its change's mark: the page is drawn afresh as a
    // model comes in, not as one turns.)
    let previews = previews.bypass_change_detection();
    if previews.views.is_empty() || !crate::screens::inventory_view::windows_up(&controller) {
        previews.dragging = None;
        return;
    }
    let Ok(window) = window.single() else { return };
    let cursor = window.cursor_position();
    let top = crate::screens::inventory_view::cover_under(covers.iter());
    let under = pictures.iter().find(|(picture, at)| at.cursor_over() && picture.1 == top).map(|(picture, _)| picture.0);
    if buttons.just_pressed(MouseButton::Left) {
        if let (Some(item), Some(at)) = (under, cursor) {
            previews.dragging = Some((item, at, at, false));
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        previews.dragging = None;
    }
    if let (Some((item, from, last, begun)), Some(now)) = (previews.dragging, cursor) {
        let begun = begun || now.distance(from) * window.scale_factor() >= crate::screens::inventory_view::DRAG_THRESHOLD;
        // (The pointer's travel in the screen's pixels, up for y.)
        let travel = (now - last) * window.scale_factor();
        previews.dragging = Some((item, from, now, begun));
        if begun && travel != Vec2::ZERO {
            if let Some(view) = previews.views.iter_mut().find(|view| view.item == item) {
                view.yaw -= travel.x * TURN_PER_PIXEL;
                if view.yaw >= 360.0 {
                    view.yaw -= 360.0;
                } else if view.yaw < -360.0 {
                    view.yaw += 360.0;
                }
                let facing = if view.yaw.to_radians().cos() >= 0.0 { 1.0 } else { -1.0 };
                view.tilt += -travel.y * TURN_PER_PIXEL * facing;
            }
        }
    }
    if wheel.delta.y != 0.0 {
        if let Some(view) = under.and_then(|item| previews.views.iter_mut().find(|view| view.item == item)) {
            let notches = match wheel.unit {
                bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
                bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y / PIXELS_A_NOTCH,
            };
            view.zoom += notches * ZOOM_PER_NOTCH;
        }
    }
}

/// Each window's preview: made as its window opens (its camera on a layer
/// of its own, at the turning parent's height a metre behind it,
/// `ResetRotator(-1)`; its picture twice its room), let go as it shuts;
/// the thing's model, once ready, put in hidden and shown on the next
/// frame with the camera fitted to its bounds (`PlaceInClosestPosition`,
/// kept for its template); turned and zoomed; the camera drawing only for
/// a few frames after something it sees changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn preview(
    mut commands: Commands,
    mut marked: ResMut<InfoPreviews>,
    carried: Res<Carried>,
    controller: Res<crate::player::controller::Controller>,
    look: Res<crate::screens::health_view::HealthLook>,
    mut game: ResMut<crate::GameData>,
    (mut loose, mut images, meshes): (ResMut<crate::world::loose::Loose>, ResMut<Assets<Image>>, Res<Assets<Mesh>>),
    mut transforms: Query<&mut Transform>,
    globals: Query<&GlobalTransform>,
    mut cameras: Query<&mut Camera>,
    (held, said): (Res<crate::screens::inventory_view::HandsHeld>, Res<crate::screens::health_view::Words>),
    (streaming, mut swaps_seen): (Option<Res<crate::assets::streaming::MipStreaming>>, Local<u64>),
    (mut surfaces, standards, level): (ResMut<Assets<crate::rendering::game_surface::GameSurfaceMaterial>>, Res<Assets<StandardMaterial>>, Option<Res<crate::rendering::lighting::LevelLighting>>),
) {
    // A picture brought to another level: each view drawn afresh with it
    // (`streaming::HeldView`).
    let swaps = streaming.as_ref().map_or(0, |streaming| streaming.swaps);
    let swapped = swaps != *swaps_seen;
    *swaps_seen = swaps;
    // (A repair window's preview as a window's that shows a thing.)
    let open: Vec<(Id, String, String)> = if look.info.is_some() || look.repair.is_some() {
        carried
            .windows
            .iter()
            .filter(|window| window.kind == WinKind::Info || (window.kind == WinKind::Repair && look.repair.is_some()))
            .filter_map(|window| carried.find(window.item).map(|item| (window.item, trp_sim::items::inventory::rig_key(item), item.kind.template.clone())))
            .collect()
    } else {
        Vec::new()
    };
    if open.is_empty() && marked.views.is_empty() {
        return;
    }
    // (Its change marked only as a model comes in or a preview comes or
    // goes, which the page is drawn afresh for.)
    let previews = marked.bypass_change_detection();
    let mut changed = false;
    // A window shut: its preview let go of, and its model if no other
    // window shows it.
    let mut kept = Vec::new();
    for view in std::mem::take(&mut previews.views) {
        if open.iter().any(|(item, ..)| *item == view.item) {
            kept.push(view);
            continue;
        }
        commands.entity(view.camera).despawn();
        commands.entity(view.rotator).despawn();
        images.remove(&view.image);
        if !open.iter().any(|(_, key, _)| *key == view.key) {
            loose.let_go(&view.key);
        }
        changed = true;
    }
    previews.views = kept;
    if previews.scene.is_none() && !previews.no_scene && !open.is_empty() {
        previews.scene = look.edit_build().map(|eb| eb.scene.clone());
        if previews.scene.is_none() {
            match trp_data::interface::edit_build::EditBuildScene::load(&game.unity.install) {
                Ok(scene) => previews.scene = Some(Arc::new(scene)),
                Err(e) => {
                    warn!("the inspect window draws no model: the edit build screen's scene could not be read: {e:#}");
                    previews.no_scene = true;
                }
            }
        }
    }
    let Some(scene) = previews.scene.clone() else { return };
    if previews.inspect.is_none() && !open.is_empty() {
        // (Read with the rest of the screens: `HealthLook::inspect`; where it
        // could not be, the edit build screen's lights light it.)
        previews.inspect = Some(look.inspect.clone());
    }
    let inspect = previews.inspect.clone().flatten();
    if previews.lights.is_empty() && !open.is_empty() {
        let layers = RenderLayers::from_layers(&LAYERS);
        previews.lights = match &inspect {
            Some(inspect) => inspect_lights(&mut commands, inspect, &layers),
            None => preview_lights(&mut commands, &scene, &layers),
        };
    }
    // The picture's size for a window: twice its preview's room as the
    // window is laid out now (its rows, slots, buttons and stretch).
    let picture_size = |item: Id| -> UVec2 {
        let repair = carried.windows.iter().any(|window| window.kind == WinKind::Repair && window.item == item) && !carried.windows.iter().any(|window| window.kind == WinKind::Info && window.item == item);
        let repair_room = repair.then(|| crate::screens::health_view::repair_built(&look, &crate::screens::health_view::RepairWords::default()).map(|built| built.preview)).flatten();
        let room = repair_room.or_else(|| carried.windows.iter().find(|window| window.kind == WinKind::Info && window.item == item).zip(carried.find(item)).and_then(|(window, thing)| {
            let figures = trp_sim::items::item_attributes::figures(thing, &game.db, &|key| carried.texts.say(key));
            let (parts, _) = crate::screens::inventory_view::info_parts(&carried, thing, &held, controller.health.as_ref(), &game, &said, window.stretched);
            crate::screens::health_view::info_built(&look, &crate::screens::health_view::InfoWords::default(), Some(&figures), &parts, false).and_then(|built| built.preview())
        }));
        let room = room.map_or([670.0, 300.0], |rect| [rect[2], rect[3]]);
        UVec2::new((room[0] * PICTURE_SCALE).round().max(1.0) as u32, (room[1] * PICTURE_SCALE).round().max(1.0) as u32)
    };
    // A window laid out afresh (stretched, a part put in or taken out):
    // its picture made again at its new size.
    if carried.is_changed() || look.is_changed() {
        for view in &mut previews.views {
            let size = picture_size(view.item);
            if size != view.size {
                let image = target(&mut images, size);
                commands.entity(view.camera).insert(RenderTarget::Image(image.clone().into()));
                images.remove(&std::mem::replace(&mut view.image, image));
                view.size = size;
                view.frames = view.frames.max(DRAWN_TURNED);
                changed = true;
            }
        }
    }
    let new_sizes: Vec<(Id, UVec2)> = open.iter().filter(|(item, ..)| !previews.views.iter().any(|view| view.item == *item)).map(|(item, ..)| (*item, picture_size(*item))).collect();
    // A window opened: its preview made.
    for (item, key, template) in &open {
        if previews.views.iter().any(|view| view.item == *item) {
            continue;
        }
        let Some(layer) = LAYERS.iter().copied().find(|layer| !previews.views.iter().any(|view| view.layer == *layer)) else { continue };
        let Some(size) = new_sizes.iter().find(|(own, _)| own == item).map(|(_, size)| *size) else { continue };
        let image = target(&mut images, size);
        let layers = RenderLayers::layer(layer);
        let camera = commands
            .spawn((
                Name::new("Inspect window's preview camera"),
                crate::assets::streaming::HeldView,
                // Its depth readable where the glass is drawn, for the
                // undithering the pool's camera runs (`glass_view`).
                Camera3d { depth_texture_usages: crate::effects::glass_view::depth_usages(), ..default() },
                Camera { order: -5 - (layer - LAYERS[0]) as isize, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                RenderTarget::Image(image.clone().into()),
                AmbientLight { color: Color::WHITE, brightness: 0.0, affects_lightmapped_meshes: false },
                Exposure { ev100: 0.0 },
                Projection::Perspective(PerspectiveProjection { fov: scene.fov.to_radians(), near: inspect.as_ref().map_or(scene.near, |inspect| inspect.near), far: scene.far, ..default() }),
                Transform::from_translation(convert::position([0.0, scene.height, CAMERA_START])),
                layers.clone(),
            ))
            .id();
        // The pool's camera's photographic tonemap (Standard Assets'
        // `Tonemapping`, its exposure 2), on its picture in half floats
        // (`HdrModeSwitcher`), the engine's own tonemapping left out.
        if let Some(exposure) = inspect.as_ref().and_then(|inspect| inspect.exposure) {
            commands.entity(camera).insert((bevy::render::view::Hdr, bevy::core_pipeline::tonemapping::Tonemapping::None, crate::screens::preview_tonemap::PreviewTonemap::of(exposure)));
        }
        let rotator = commands.spawn((Name::new("Inspect window's preview"), rotator_place(&scene, 0.0, 0.0), Visibility::Inherited, layers)).id();
        if !previews.places.contains_key(template) {
            let prefab = game.db.items.get(template).and_then(|kind| kind.prefab_key()).map(str::to_owned);
            let place = prefab.and_then(|prefab| {
                trp_data::interface::icon::preview_place(&mut game.unity, &prefab).unwrap_or_else(|e| {
                    warn!("{template}'s place in the inspect window's preview: {e:#}");
                    None
                })
            });
            previews.places.insert(template.clone(), place);
        }
        // (Not marked: the window draws nothing of it until its model is
        // in. Drawn afresh for it, the page knew the mouse a frame late: the
        // slot under the mouse lit two frames after the window opened, the
        // game's one, `courses10`'s `weapon-inspect-menu`, frame 260.)
        previews.views.push(View { item: *item, key: key.clone(), template: template.clone(), layer, camera, image, size, rotator, holder: None, pending: None, put_key: None, ready: false, yaw: 0.0, tilt: 0.0, camera_z: CAMERA_START, bounds: None, zoom: 0.0, frames: 0 });
    }
    for view in &mut previews.views {
        let Some((_, key, template)) = open.iter().find(|(item, ..)| *item == view.item) else { continue };
        if *key != view.key || *template != view.template {
            (view.key, view.template) = (key.clone(), template.clone());
        }
        // The model put in last frame: its bounds had, it is shown in the
        // old one's place and the camera fitted to it.
        if let Some(put) = view.pending.take() {
            view.bounds = model_bounds(&put, &meshes, &globals);
            if let Some(old) = view.holder.replace(put.holder) {
                commands.entity(old).despawn();
            }
            commands.entity(put.holder).insert(Visibility::Inherited);
            let aspect = view.size.x as f32 / view.size.y.max(1) as f32;
            let z = *previews.closest.entry(view.template.clone()).or_insert_with(|| view.bounds.map_or(CAMERA_START, |bounds| closest(&scene, aspect, bounds)));
            view.camera_z = z;
            if let Ok(mut place) = transforms.get_mut(view.camera) {
                place.translation = convert::position([0.0, scene.height, z]);
            }
            if !view.ready {
                view.ready = true;
                changed = true;
            }
            view.frames = DRAWN_MODEL;
        }
        // Its model, by its key, once ready: put in hidden, its pivot
        // turned by the pose of its icon (`PreviewPivot.Icon.rotation`).
        if view.pending.is_none() && view.put_key.as_deref() != Some(view.key.as_str()) {
            if let Some(ready) = loose.model_for(&view.key) {
                let place = previews.places.get(&view.template).copied().flatten();
                let turn = place.map_or(Quat::IDENTITY, |place| convert::rotation(place.icon_rotation));
                // Its surfaces as the game's deferred pass and light draw
                // them under the template's two lights (`game_surface`), where
                // they and its tonemap were read; else the standard surface
                // under the edit build screen's.
                let top_ambient = level.as_ref().map_or([0.0; 3], |level| level.top_ambient);
                let surface = inspect.as_ref().filter(|inspect| inspect.exposure.is_some()).map(|inspect| preview_surface(inspect, top_ambient));
                let with = surface.as_ref().map(|view| (&mut *surfaces, &*standards, view));
                view.pending = Some(put_model(&mut commands, &ready, view.rotator, &RenderLayers::layer(view.layer), turn, place, with));
                view.put_key = Some(view.key.clone());
            }
        }
        if let Ok(mut place) = transforms.get_mut(view.rotator) {
            let wanted = rotator_place(&scene, view.yaw, view.tilt);
            if place.rotation != wanted.rotation {
                place.rotation = wanted.rotation;
                view.frames = view.frames.max(DRAWN_TURNED);
            }
        }
        let zoom = std::mem::take(&mut view.zoom);
        if let (true, Some(bounds)) = (zoom != 0.0, view.bounds) {
            let aspect = view.size.x as f32 / view.size.y.max(1) as f32;
            let z = zoomed(&scene, view.camera_z, zoom, aspect, bounds);
            if z != view.camera_z {
                view.camera_z = z;
                if let Ok(mut place) = transforms.get_mut(view.camera) {
                    place.translation = convert::position([0.0, scene.height, z]);
                }
                view.frames = view.frames.max(DRAWN_TURNED);
            }
        }
        if swapped {
            view.frames = view.frames.max(DRAWN_TURNED);
        }
        if let Ok(mut camera) = cameras.get_mut(view.camera) {
            let drawing = view.frames > 0 && crate::screens::inventory_view::windows_up(&controller);
            if camera.is_active != drawing {
                camera.is_active = drawing;
            }
        }
        view.frames = view.frames.saturating_sub(1);
    }
    if changed {
        marked.set_changed();
    }
}
