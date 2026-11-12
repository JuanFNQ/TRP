//! Turning the preview and drawing it each frame.

use super::*;

/// A drag across the preview turns it (`DragTrigger`, `TranslateAxes`,
/// `Rotate`): begun once the mouse has moved past the drag's threshold
/// with its button down on the preview (a click is no turn), then by the
/// mouse's own travel, which the cursor's place (held in the window's
/// middle while it turns, `cursor`) has no part in (a click on another part
/// of the screen had flung the weapon about: the cursor's jump to the
/// middle was taken for a move); its yaw less the
/// sideways move, its tilt the upward move, flipped while the weapon faces
/// away, held within ±30°; the arrows turn it too.
#[allow(clippy::too_many_arguments)]
pub(super) fn turn(
    mut state: ResMut<EditBuild>,
    options: Res<crate::Options>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    picture: Query<&Interaction, With<PreviewPicture>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut list: Query<&mut ScrollPosition, With<DropdownList>>,
    (menu, windows): (Res<crate::screens::item_menu_view::ItemMenu>, Query<&RelativeCursorPosition, With<crate::screens::inventory_view::WinRoot>>),
) {
    if !state.open {
        return;
    }
    // While the menu of a right click is up, or the mouse is over a window,
    // the mouse is theirs: nothing of the screen hears it.
    let taken = menu.0.is_some() || windows.iter().any(RelativeCursorPosition::cursor_over);
    if state.mouse_taken != taken {
        state.mouse_taken = taken;
    }
    // The wheel scrolls the app's dropdown (a line a notch: the app's);
    // with none open it zooms.
    if taken {
    } else if state.menu.is_some() && wheel.delta.y != 0.0 {
        let s = window.single().map_or(1.0, |window| window.height() / HIGH);
        state.scroll = (state.scroll - wheel.delta.y * (CELL + 2.0) * s).max(0.0);
        for mut position in &mut list {
            position.0.y = state.scroll;
        }
    } else if wheel.delta.y != 0.0 {
        let notches = match wheel.unit {
            bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
            bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y / PIXELS_A_NOTCH,
        };
        state.zoom += notches * ZOOM_PER_NOTCH;
    }
    let cursor = window.single().ok().and_then(Window::cursor_position);
    if buttons.just_pressed(MouseButton::Left) && state.menu.is_none() && !taken && picture.iter().any(|interaction| *interaction == Interaction::Pressed) {
        state.pressed_at = Some(cursor.unwrap_or_default());
    }
    if !buttons.pressed(MouseButton::Left) {
        state.pressed_at = None;
        state.dragging = false;
    }
    if let (Some(at), false, Some(now)) = (state.pressed_at, state.dragging, cursor) {
        if now.distance(at) >= DRAG_THRESHOLD {
            state.dragging = true;
            state.cursor = Some(now);
        }
    }
    TURNING.store(state.dragging, std::sync::atomic::Ordering::Relaxed);
    let rotate = |state: &mut EditBuild, angle: f32, tilt: f32| {
        state.yaw -= angle;
        if state.yaw >= 360.0 {
            state.yaw -= 360.0;
        } else if state.yaw < -360.0 {
            state.yaw += 360.0;
        }
        let facing = if state.yaw.to_radians().cos() >= 0.0 { 1.0 } else { -1.0 };
        state.tilt = (state.tilt + tilt * facing).clamp(-30.0, 30.0);
    };
    if state.dragging {
        let delta = if options.unattended() {
            match (cursor, state.cursor) {
                (Some(now), Some(before)) => now - before,
                _ => Vec2::ZERO,
            }
        } else {
            motion.delta
        };
        if delta != Vec2::ZERO {
            rotate(&mut state, delta.x * TURN_PER_COUNT, delta.y * TURN_PER_COUNT);
        }
    }
    state.cursor = cursor;
    if keys.pressed(KeyCode::ArrowRight) {
        rotate(&mut state, ARROW_TURN * 0.3, 0.0);
    } else if keys.pressed(KeyCode::ArrowLeft) {
        rotate(&mut state, -ARROW_TURN * 0.3, 0.0);
    }
}

/// The turning parent's place and turn: Unity's `Euler(tilt, yaw, 0)` at
/// the preview's height.
pub(crate) fn rotator_place(scene: &trp_data::interface::edit_build::EditBuildScene, yaw: f32, tilt: f32) -> Transform {
    let turn = Quat::from_rotation_y(yaw.to_radians()) * Quat::from_rotation_x(tilt.to_radians());
    Transform { translation: convert::position([0.0, scene.height, 0.0]), rotation: convert::rotation(turn.to_array()), scale: Vec3::ONE }
}

/// The preview: its camera and lights made once, its picture the window's
/// size; the build's model put in hidden once it is ready and shown on the
/// next frame, when its objects' places are had, the old one kept until
/// then; the turning parent turned. The camera draws only for a few frames
/// after something it sees changes, its picture kept otherwise.
#[allow(clippy::too_many_arguments)]
pub(super) fn preview(
    mut commands: Commands,
    mut state: ResMut<EditBuild>,
    mut loose: ResMut<crate::world::loose::Loose>,
    mut images: ResMut<Assets<Image>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut transforms: Query<&mut Transform>,
    globals: Query<&GlobalTransform>,
    mut cameras: Query<&mut Camera>,
    meshes: Res<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    (streaming, mut swaps_seen): (Option<Res<crate::assets::streaming::MipStreaming>>, Local<u64>),
) {
    let state = &mut *state;
    // A picture of the model's brought to another level: drawn afresh
    // with it (`streaming::HeldView`).
    let swaps = streaming.as_ref().map_or(0, |streaming| streaming.swaps);
    if swaps != *swaps_seen {
        *swaps_seen = swaps;
        state.preview.frames = state.preview.frames.max(DRAWN_TURNED);
    }
    if !state.open {
        // Shut: the models it asked for let go of, and the reader with them.
        for key in std::mem::take(&mut state.released) {
            loose.let_go(&key);
        }
        if loose.keep_reader {
            loose.keep_reader = false;
        }
        return;
    }
    // Open, the models' reader is kept between picks.
    if !loose.keep_reader {
        loose.keep_reader = true;
    }
    let (Some(scene), Ok(window), Some(build)) = (state.scene.clone(), window.single(), state.build.as_ref()) else { return };
    let layer = RenderLayers::layer(LAYER);
    let size = UVec2::new(window.physical_width().max(1), window.physical_height().max(1));
    if state.preview.camera.is_none() {
        let image = target(&mut images, size);
        // The camera as the scene's (`Weapon Camera`): its field of view
        // and planes; under the turning parent's place, a metre behind it,
        // looking along it (`ResetRotator(-1)`); drawing the preview's
        // layer at an exposure at which a light's intensity is the game's
        // (as the icons' studio), onto nothing, so that the screen's
        // ground shows round the weapon. Its depth readable where the glass
        // is drawn, for the camera's undithering (`glass_view`), which the
        // scene's camera runs as the game's main camera does.
        let camera = commands
            .spawn((
                Name::new("Edit build preview's camera"),
                crate::assets::streaming::HeldView,
                Camera3d { depth_texture_usages: crate::effects::glass_view::depth_usages(), ..default() },
                Camera { order: -4, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                RenderTarget::Image(image.clone().into()),
                AmbientLight { color: Color::WHITE, brightness: 0.0, affects_lightmapped_meshes: false },
                Exposure { ev100: 0.0 },
                Projection::Perspective(PerspectiveProjection { fov: scene.fov.to_radians(), near: scene.near, far: scene.far, ..default() }),
                Transform::from_translation(convert::position([0.0, scene.height, CAMERA_START])),
                layer.clone(),
            ))
            .id();
        // The inspect window's two lights and photographic tonemap (chosen
        // over the screen's own thirteen and its tonemap at 2.5 with its
        // contrast); the
        // screen's own where that preview could not be read.
        match state.inspect.clone() {
            Some(inspect) => {
                state.preview.lights = crate::screens::info_preview::inspect_lights(&mut commands, &inspect, &layer);
                if let Some(exposure) = inspect.exposure {
                    commands.entity(camera).insert((bevy::render::view::Hdr, bevy::core_pipeline::tonemapping::Tonemapping::None, crate::screens::preview_tonemap::PreviewTonemap::of(exposure)));
                }
            }
            None => state.preview.lights = preview_lights(&mut commands, &scene, &layer),
        }
        // The outline's mask (`HighLightMesh`: a picture of the screen's
        // size with no depth, cleared black, the outlined part drawn white
        // into it), drawn by a camera of the same sight before the
        // preview's; and the outline's pass on the preview's camera.
        let mask = mask_target(&mut images, size);
        let mask_camera = commands
            .spawn((
                Name::new("Edit build preview's outline mask"),
                Camera3d::default(),
                Camera { order: -5, clear_color: ClearColorConfig::Custom(Color::BLACK), is_active: false, ..default() },
                RenderTarget::Image(mask.clone().into()),
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                bevy::core_pipeline::tonemapping::DebandDither::Disabled,
                bevy::render::view::Msaa::Off,
                Projection::Perspective(PerspectiveProjection { fov: scene.fov.to_radians(), near: scene.near, far: scene.far, ..default() }),
                Transform::IDENTITY,
                RenderLayers::layer(MASK_LAYER),
                ChildOf(camera),
            ))
            .id();
        commands.entity(camera).insert((crate::screens::preview_outline::PreviewOutline::of(None, size), crate::screens::preview_outline::OutlineMask(mask.clone())));
        state.preview.mask_material = Some(materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() }));
        (state.preview.mask_camera, state.preview.mask_image) = (Some(mask_camera), Some(mask));
        let rotator = commands.spawn((Name::new("Edit build preview"), rotator_place(&scene, state.yaw, state.tilt), Visibility::Inherited, layer.clone())).id();
        (state.preview.camera, state.preview.image, state.preview.size, state.preview.rotator) = (Some(camera), Some(image), size, Some(rotator));
        state.preview.camera_z = CAMERA_START;
        state.zoom = 0.0;
        state.preview.frames = DRAWN_MODEL;
        return;
    }
    // The picture made again for the window's size.
    if state.preview.size != size {
        let image = target(&mut images, size);
        if let Some(camera) = state.preview.camera {
            commands.entity(camera).insert(RenderTarget::Image(image.clone().into()));
        }
        if let Some(old) = state.preview.image.replace(image) {
            images.remove(&old);
        }
        let mask = mask_target(&mut images, size);
        if let (Some(camera), Some(mask_camera)) = (state.preview.camera, state.preview.mask_camera) {
            commands.entity(mask_camera).insert(RenderTarget::Image(mask.clone().into()));
            commands.entity(camera).insert(crate::screens::preview_outline::OutlineMask(mask.clone()));
        }
        if let Some(old) = state.preview.mask_image.replace(mask) {
            images.remove(&old);
        }
        state.preview.size = size;
        state.preview.frames = state.preview.frames.max(DRAWN_TURNED);
    }
    let Some(rotator) = state.preview.rotator else { return };
    if let Ok(mut place) = transforms.get_mut(rotator) {
        let wanted = rotator_place(&scene, state.yaw, state.tilt);
        if place.rotation != wanted.rotation {
            place.rotation = wanted.rotation;
            state.preview.frames = state.preview.frames.max(DRAWN_TURNED);
        }
    }
    // The model made last frame: its objects' places are had, so each
    // slot's bone is found in it and it is shown in the old one's place,
    // and its slots are made (`RefreshWeapon`: `CreateModSlotViews`).
    if let Some(model) = state.preview.pending.take() {
        let turned = globals.get(rotator).map(|global| global.affine().inverse()).unwrap_or(Affine3A::IDENTITY);
        let found = bones(&model.build, &model.put.names, &model.put.parents);
        state.preview.bones = found.iter().filter_map(|(slot, node)| Some((*slot, turned * globals.get(*model.put.nodes.get(*node)?).ok()?.affine()))).collect();
        // The gear's own armour slots, which its model has no object for:
        // each at a place of the app's on the model.
        if state.gear {
            if let Some(root) = model.put.nodes.first().and_then(|root| globals.get(*root).ok()).map(GlobalTransform::affine) {
                let from_root = root.inverse();
                let mut bounds: Option<(Vec3, Vec3)> = None;
                for (node, mesh) in &model.put.bounded {
                    let (Some(aabb), Ok(global)) = (meshes.get(mesh).and_then(MeshAabb::compute_aabb), globals.get(model.put.nodes[*node])) else { continue };
                    let affine = from_root * global.affine();
                    for corner in 0..8 {
                        let sign = Vec3A::new(if corner & 1 == 0 { -1.0 } else { 1.0 }, if corner & 2 == 0 { -1.0 } else { 1.0 }, if corner & 4 == 0 { -1.0 } else { 1.0 });
                        let p = Vec3::from(affine.transform_point3a(aabb.center + aabb.half_extents * sign));
                        bounds = Some(bounds.map_or((p, p), |(low, high)| (low.min(p), high.max(p))));
                    }
                }
                if let Some((low, high)) = bounds {
                    for (slot, point) in gear_points(&model.build, &found, low, high) {
                        state.preview.bones.insert(slot, turned * root * Affine3A::from_translation(point));
                    }
                }
            }
        }
        state.preview.bounds = model_bounds(&model.put, &meshes, &globals);
        // Armour and a vest with the camera fitted to them, as the screen's
        // first model is shown (`RefreshWeapon`: `setAsClosest`,
        // `PlaceInClosestPosition`, its place kept for the template).
        if seen_whole(&model.build) && state.preview.key.is_none() {
            if let (Some(bounds), Some(camera)) = (state.preview.bounds, state.preview.camera) {
                let aspect = size.x as f32 / size.y.max(1) as f32;
                state.preview.camera_z = crate::screens::info_preview::closest(&scene, aspect, bounds);
                if let Ok(mut place) = transforms.get_mut(camera) {
                    place.translation = convert::position([0.0, scene.height, state.preview.camera_z]);
                }
            }
        }
        // (The old model's outline goes with it.)
        state.preview.bone_nodes = found;
        state.preview.nodes = model.put.nodes.clone();
        state.preview.parents = model.put.parents.clone();
        state.preview.drawn = model.put.drawn.clone();
        state.preview.copies.clear();
        if let Some(old) = state.preview.holder.replace(model.put.holder) {
            commands.entity(old).despawn();
        }
        commands.entity(model.put.holder).insert(Visibility::Inherited);
        // (The model shown before, no longer wanted here.)
        if let Some(old) = state.preview.key.as_ref().filter(|old| **old != model.key) {
            loose.let_go(old);
        }
        state.preview.key = Some(model.key);
        state.preview.model += 1;
        state.shown = Some(model.build);
        state.menu = None;
        state.menu_seen = false;
        state.preview.frames = DRAWN_MODEL;
    }
    // The build's model, by its key, once ready: put in hidden.
    if state.preview.key.as_deref() != Some(state.build_key.as_str()) {
        let key = state.build_key.clone();
        // A model asked for and no longer the build's (a pick made again
        // before it came) is let go of (`SetupItemPreview` cancels the wait
        // for the one before).
        if let Some(stale) = state.preview.asked.replace(key.clone()).filter(|asked| *asked != key && state.preview.key.as_ref() != Some(asked)) {
            loose.let_go(&stale);
        }
        if let Some(ready) = loose.model_for(&key) {
            // The pivot turned (0, −90, 0) and scaled by the `PreviewPivot`,
            // the container at minus its pivot, the weapon's object at its
            // place in the container (`PositionGameObject`); with no
            // `PreviewPivot`, about the model's own first object
            // (`PROVISIONAL`: the game's bounds' middle).
            // The pivot turned (0, −90, 0) (`ItemObserveScreen`).
            let place = state.places.get(&build.kind.template).copied().flatten();
            info!("the edit build screen's model {key}: placed by {place:?}; {} objects", ready.nodes.len());
            // (Armour and a vest (−90, 180, 0), seen from the front:
            // `RefreshWeapon`.)
            let pivot_turn = if seen_whole(build) {
                convert::rotation((Quat::from_rotation_y(180.0f32.to_radians()) * Quat::from_rotation_x((-90.0f32).to_radians())).to_array())
            } else {
                convert::rotation(Quat::from_rotation_y((-90.0f32).to_radians()).to_array())
            };
            let put = put_model(&mut commands, &ready, rotator, &layer, pivot_turn, place, None);
            state.preview.pending = Some(Model { put, key, build: build.clone() });
        }
    }
    // The wheel's zoom, once the model's bounds are had (`Zoom`: none
    // before).
    let zoom = std::mem::take(&mut state.zoom);
    if let (true, Some(bounds), Some(camera)) = (zoom != 0.0, state.preview.bounds, state.preview.camera) {
        let aspect = size.x as f32 / size.y.max(1) as f32;
        let z = zoomed(&scene, state.preview.camera_z, zoom, aspect, bounds);
        if z != state.preview.camera_z {
            state.preview.camera_z = z;
            if let Ok(mut place) = transforms.get_mut(camera) {
                place.translation = convert::position([0.0, scene.height, z]);
            }
            state.preview.frames = state.preview.frames.max(DRAWN_TURNED);
        }
    }
    if let Some(Ok(mut camera)) = state.preview.camera.map(|camera| cameras.get_mut(camera)) {
        let drawing = state.preview.frames > 0;
        if camera.is_active != drawing {
            camera.is_active = drawing;
        }
    }
    state.preview.frames = state.preview.frames.saturating_sub(1);
}
