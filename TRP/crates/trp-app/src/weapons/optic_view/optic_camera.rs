//! The optic's camera spawned and placed, the lenses dressed and the sights turned.

use super::*;

/// The optic camera, made once with the main camera and left off: its
/// picture HDR (half floats) where the main camera's is, drawn before the
/// main camera's, of the world (not the lenses), as the main camera draws
/// it (the same sampling, tonemapping, exposure and environment).
///
/// It draws the world's effects as the game's optic camera does: the
/// effects' objects are all of the world's layer (the effects' bundle's 1,006
/// objects on layer 0 but four smoke volumes' colliders, `probe
/// bundle-layers`; the jets' mesh made on layer 0), which its mask takes.
/// So it has the screens of the flash's jets, the fumes, the sparks and the
/// explosions' and hits' systems, and the scene's depth readable for their
/// copy (`muzzle_view`). Not the gun smoke's trail, which the game draws for
/// the main camera alone (`MuzzleSmoke.OnRenderObject`), nor the heat haze,
/// drawn by the main camera's distortion alone (`DistortCameraFX`, which
/// this camera has not), nor the screen's blood. Each screen is filled only
/// while the camera is on.
#[allow(clippy::type_complexity)]
pub(super) fn spawn_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    light: Option<Res<crate::rendering::lighting::LevelLighting>>,
    main: Query<(Entity, &Camera, Option<&Hdr>, Option<&Tonemapping>, Option<&Exposure>, Option<&bevy::light::EnvironmentMapLight>, Option<&AmbientLight>, Option<&Msaa>), With<MainCamera>>,
) {
    let Ok((main, camera, hdr, tonemapping, exposure, environment, ambient, msaa)) = main.single() else {
        warn!("no main camera: no optic camera");
        return;
    };
    let format = if hdr.is_some() { TextureFormat::Rgba16Float } else { TextureFormat::Rgba8UnormSrgb };
    let mut picture = Image::new_uninit(Extent3d { width: PICTURE_SIZE, height: PICTURE_SIZE, depth_or_array_layers: 1 }, TextureDimension::D2, format, RenderAssetUsages::default());
    picture.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    picture.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::ClampToEdge, address_mode_v: ImageAddressMode::ClampToEdge, ..ImageSamplerDescriptor::linear() });
    let picture = images.add(picture);
    let mut optic = commands.spawn((
        Name::new("Optic camera"),
        OpticCamera,
        // Its depth readable after the main pass, for the effects' copy.
        Camera3d { depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING).into(), ..default() },
        Camera { is_active: false, order: -1, clear_color: camera.clear_color, ..default() },
        RenderTarget::Image(picture.clone().into()),
        Projection::Perspective(PerspectiveProjection { fov: 20f32.to_radians(), aspect_ratio: 1.0, near: NEAR, near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -NEAR), ..default() }),
        Transform::default(),
        RenderLayers::from_layers(&[0, RETICLE_LAYER, crate::weapons::weapon::HANDS_LAYER]),
        (
            crate::weapons::muzzle_view::JetScreen::default(),
            crate::effects::particles_view::FumeScreen::default(),
            crate::effects::particles_view::SparkScreen::default(),
            crate::weapons::tracer_view::TracerScreen::default(),
            crate::weapons::flare_view::FlareScreen::default(),
            crate::effects::explosion_view::ExplosionScreen::default(),
        ),
    ));
    if hdr.is_some() {
        optic.insert(Hdr);
    }
    // With a level's light its picture stays in the scene's figures around
    // its effects (`gamma_ui`): the main camera's PRISM takes it on.
    if light.is_some() {
        optic.insert(crate::ui::gamma_ui::KeepFigures);
    }
    if let Some(tonemapping) = tonemapping {
        optic.insert(*tonemapping);
    }
    if let Some(exposure) = exposure {
        optic.insert(*exposure);
    }
    if let Some(environment) = environment {
        optic.insert(environment.clone());
    }
    if let Some(ambient) = ambient {
        optic.insert(ambient.clone());
    }
    if let Some(msaa) = msaa {
        optic.insert(*msaa);
    }
    // Its lights in one cluster, the renderer's own sorting of them (each
    // still ends at its range): the range's few lamps and its hall's probe
    // need no more. Bevy's default clusters cost the scope's view 180 µs a
    // frame in the profiling build, one cluster 141 (2026-11-11); the scoped
    // captures were the same pixel for pixel.
    optic.insert(bevy::light::cluster::ClusterConfig::Single);
    commands.insert_resource(OpticPicture(picture));
    // The main camera sees the lenses besides the world.
    commands.entity(main).insert(RenderLayers::from_layers(&[0, LENS_LAYER, crate::weapons::weapon::HANDS_LAYER]));
}

/// The entity a lens piece is drawn by: its object's own where its mesh
/// has one part, else that part's among the object's drawn children
/// (`weapon::spawn_nodes`: one child a submesh, in order, with no name).
fn piece_entity(object: Entity, slot: usize, meshes: &Query<(), With<Mesh3d>>, named: &Query<(), With<Name>>, children: &Query<&Children>) -> Option<Entity> {
    if meshes.contains(object) {
        return (slot == 0).then_some(object);
    }
    let kids = children.get(object).ok()?;
    kids.iter().filter(|&kid| meshes.contains(kid) && !named.contains(kid)).nth(slot)
}

/// A lens picture as an image, its sampler clamping where the game's does.
fn lens_picture(pictures: &mut LensPictures, images: &mut Assets<Image>, texture: &Arc<TextureData>) -> Option<Handle<Image>> {
    let key = Arc::as_ptr(texture) as usize;
    if let Some(handle) = pictures.0.get(&key) {
        return Some(handle.clone());
    }
    let mut image = match crate::assets::render_data::build_image(texture, crate::assets::render_data::Role::Color) {
        Ok(image) => image,
        Err(e) => {
            warn!("the lens's picture {}: {e:#}", texture.name);
            return None;
        }
    };
    if let ImageSampler::Descriptor(sampler) = &mut image.sampler {
        let mode = |clamp: bool| if clamp { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
        (sampler.address_mode_u, sampler.address_mode_v) = (mode(texture.clamp[0]), mode(texture.clamp[1]));
    }
    let handle = images.add(image);
    pictures.0.insert(key, handle.clone());
    Some(handle)
}

/// Gives the lens pieces of the weapon in hand their own drawing, once each:
/// a scope's lens the lens's (with the optic camera's picture: none made,
/// the part's own material), a collimator's its dot, the front glass
/// nothing.
#[allow(clippy::too_many_arguments)]
pub(super) fn dress_lenses(
    mut commands: Commands,
    rig: Option<Res<WeaponRig>>,
    hands: Option<Res<HandsRig>>,
    picture: Option<Res<OpticPicture>>,
    light: Option<Res<crate::rendering::lighting::LevelLighting>>,
    mut pictures: ResMut<LensPictures>,
    mut images: ResMut<Assets<Image>>,
    mut lenses: ResMut<Assets<LensMaterial>>,
    mut collimators: ResMut<Assets<CollimatorMaterial>>,
    meshes: Query<(), With<Mesh3d>>,
    named: Query<(), With<Name>>,
    children: Query<&Children>,
    dressed: Query<(), With<Dressed>>,
) {
    let (Some(rig), Some(hands)) = (rig, hands) else { return };
    let Some(sights) = hands.hands.sights() else { return };
    let power = if light.is_some() { 2.2 } else { 1.0 };
    let pieces: Vec<&LensNode> = sights.parts().iter().flat_map(|part| &part.lenses).collect();
    for piece in pieces {
        let Some(&object) = rig.bones.get(piece.node) else { continue };
        let Some(entity) = piece_entity(object, piece.slot, &meshes, &named, &children) else { continue };
        if dressed.contains(entity) {
            continue;
        }
        let picture_of = |name: &str| piece.pictures.iter().find(|(n, _)| n == name).map(|(_, texture)| texture.clone());
        let mut e = commands.entity(entity);
        match &piece.kind {
            LensKind::Optic(lens) => {
                let Some(picture) = picture.as_ref().map(|picture| Some(picture.0.clone())) else {
                    e.insert(Dressed { node: piece.node, dimmed: DIMMED });
                    continue;
                };
                let mark = picture_of("_MarkTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t));
                // Its picture's aberration: its mode's.
                let aberration = sights.parts().iter().flat_map(|part| &part.modes).filter_map(|mode| mode.optic.as_ref()).find(|optic| optic.lens == Some(piece.node)).map_or(0.0, |optic| optic.aberration);
                let material = LensMaterial {
                    lens: LensUniform {
                        scales_shifts: Vec4::new(lens.scales[0], lens.scales[1], lens.shifts[0], lens.shifts[1]),
                        direction: Vec4::new(lens.shift_direction[0], lens.shift_direction[1], lens.shift_direction[2], lens.normal_hideness),
                        figures: Vec4::new(lens.inner_radius, lens.outer_radius, lens.mark_lightness, 1.0 - DIMMED),
                        space: Vec4::new(power, if mark.is_some() { 1.0 } else { 0.0 }, aberration, 0.0),
                    },
                    picture,
                    mark,
                    mask: picture_of("_MaskTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t)),
                    mask2: picture_of("_MaskTex2").and_then(|t| lens_picture(&mut pictures, &mut images, &t)),
                };
                e.remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(lenses.add(material)), NotShadowCaster, Dressed { node: piece.node, dimmed: DIMMED }, RenderLayers::layer(LENS_LAYER)));
            }
            LensKind::Collimator(dot) => {
                let noise = picture_of("_NoiseTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t));
                let fade = picture_of("_FadeTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t));
                let material = CollimatorMaterial {
                    collimator: CollimatorUniform {
                        color: Vec4::new(dot.color[0], dot.color[1], dot.color[2], dot.hdr),
                        mark: Vec4::new(dot.mark_shift[0], dot.mark_shift[1], dot.mark_shift[2], dot.mark_scale),
                        space: Vec4::new(power, if noise.is_some() { 1.0 } else { 0.0 }, if fade.is_some() { 1.0 } else { 0.0 }, 0.0),
                    },
                    mark: picture_of("_MarkTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t)),
                    noise,
                    fade,
                };
                e.remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(collimators.add(material)), NotShadowCaster, Dressed { node: piece.node, dimmed: 0.0 }));
            }
            LensKind::Glass => {
                e.insert((Visibility::Hidden, Dressed { node: piece.node, dimmed: 0.0 }));
            }
            LensKind::Back => {
                e.insert(Dressed { node: piece.node, dimmed: 0.0 });
            }
        }
    }
}

/// The zeroing's turns (`trp_sim::sights::Sights::turns`; optics spec,
/// "Zeroing"): each optic's camera object, each collimator and a part a
/// prefab turns by distance, as the simulation last left them, and a
/// variable scope's ring as its zoom turns it (`Sights::ring_turns`; optics
/// spec, "The ring"), put on the rig's bones after the animator's pose,
/// which writes every bone each frame (`animation::pose_rig`).
pub(super) fn turn_sights(rig: Option<Res<WeaponRig>>, hands: Option<Res<HandsRig>>, mut transforms: Query<&mut Transform, Without<OpticCamera>>) {
    let (Some(rig), Some(hands)) = (rig, hands) else { return };
    let Some(sights) = hands.hands.sights() else { return };
    for (node, turn) in sights.turns().chain(sights.ring_turns()) {
        let Some(mut transform) = rig.bones.get(node).and_then(|&entity| transforms.get_mut(entity).ok()) else { continue };
        let rotation = crate::platform::convert::rotation(turn);
        if transform.rotation != rotation {
            transform.rotation = rotation;
        }
    }
}

/// Each frame: the optic camera on while aiming through an optic in first
/// person, at the current mode's `optic_camera` object (turned to the
/// zeroing point: `turn_sights`), at the scope's field of view; the lenses
/// dimmed but the live one's; a scope's modes' objects shown as its
/// prefab's mode is.
#[allow(clippy::too_many_arguments)]
pub(super) fn place_camera(
    mut commands: Commands,
    view: Res<View>,
    rig: Option<Res<WeaponRig>>,
    hands: Option<Res<HandsRig>>,
    mut camera: Query<(Entity, &mut Camera, &mut Transform, &mut Projection, Option<&ChildOf>), With<OpticCamera>>,
    transforms: Query<&Transform, Without<OpticCamera>>,
    parents: Query<&ChildOf>,
    mut lenses: Query<(&mut Dressed, &MeshMaterial3d<LensMaterial>)>,
    mut materials: ResMut<Assets<LensMaterial>>,
    mut visibility: Query<&mut Visibility>,
    mut shown_modes: Local<HashMap<Entity, usize>>,
    (warmup, main, mut warming): (Option<Res<crate::Warmup>>, Query<(&GlobalTransform, &Projection), (With<MainCamera>, Without<OpticCamera>)>, Local<bool>),
) {
    if pre_warm(warmup.is_some_and(|warmup| warmup.0), &mut warming, &mut camera, &main) {
        return;
    }
    let (Some(rig), Some(hands)) = (rig, hands) else { return };
    let Some(sights) = hands.hands.sights() else { return };
    let first_person = *view == View::FirstPerson;
    // The optic whose camera is on, and its lens.
    let live = sights.live_optic().filter(|_| first_person && hands.optic_seen());
    let optic = live.and_then(|(part, mode)| sights.parts().get(part)?.modes.get(mode)?.optic.as_ref());
    // A scope's modes' objects as its prefab's mode (`ScopePrefabCache.
    // SetMode`): the first object kept as the prefab has it until the mode
    // changes.
    for (part, piece) in sights.parts().iter().enumerate() {
        if piece.modes.len() < 2 {
            continue;
        }
        let Some(&root) = rig.bones.get(piece.root) else { continue };
        let mode = sights.state(part).prefab_mode;
        let was = *shown_modes.get(&root).unwrap_or(&0);
        if was == mode {
            continue;
        }
        for (index, info) in piece.modes.iter().enumerate() {
            if let Some(mut seen) = info.node.and_then(|node| rig.bones.get(node)).and_then(|&e| visibility.get_mut(e).ok()) {
                *seen = if index == mode { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
        shown_modes.insert(root, mode);
    }
    // The lenses' dimming: none on the live one.
    let live_lens = optic.and_then(|optic| optic.lens);
    for (mut dressed, material) in &mut lenses {
        let dimmed = if Some(dressed.node) == live_lens { 0.0 } else { DIMMED };
        if dressed.dimmed != dimmed {
            dressed.dimmed = dimmed;
            if let Some(material) = materials.get_mut(&material.0) {
                material.lens.figures.w = 1.0 - dimmed;
            }
        }
    }
    let Ok((entity, mut camera, mut transform, mut projection, parent)) = camera.single_mut() else { return };
    // Its place in the pivot's frame: the rig's root as it stands, without
    // the squeeze of a wide field of view (the game's camera is placed
    // before the squeeze is put on), then down to the object (whose own
    // scale is nought on the Tango: only its place and turn are taken).
    let place = optic.and_then(|optic| {
        let object = *rig.bones.get(optic.camera?)?;
        let root = transforms.get(rig.root).ok()?;
        let within = crate::player::hands_view::in_root(object, rig.root, |e| transforms.get(e).ok().copied(), &parents)?;
        let mut place = Transform { scale: Vec3::ONE, ..*root } * within;
        place.scale = Vec3::ONE;
        Some((place, sights.optic_field_of_view()?, optic.near.unwrap_or(NEAR)))
    });
    let Some((place, fov, near)) = place else {
        if camera.is_active {
            camera.is_active = false;
        }
        return;
    };
    if parent.map(ChildOf::parent) != Some(rig.pivot) {
        commands.entity(rig.pivot).add_child(entity);
    }
    // Its levels of detail are picked by its own view's pixel (`lod_view`,
    // TRP's own rule); the game's scope's factor on its bias
    // (`OpticComponentUpdater.CopyComponentFromOptic`, `OpticMode::
    // lod_bias_factor`) is the game's rule's and not read.
    *transform = place;
    if let Projection::Perspective(perspective) = &mut *projection {
        perspective.fov = fov.to_radians();
        perspective.near = near;
        perspective.near_clip_plane = Vec4::new(0.0, 0.0, -1.0, -near);
    }
    if !camera.is_active {
        camera.is_active = true;
    }
}

/// The optic camera through the warm-up (`crate::Warmup`): on, at the main
/// camera's place and as wide as its view, so that the renderer builds what
/// the scope will draw with behind the curtain. The game's own rule
/// (`OpticCameraManager.Init`: the new camera on for one frame, then off);
/// the app's renderer builds in the background over frames, so the camera
/// stays on until the curtain goes. Off as the warm-up ends. Whether it
/// took the frame: the rest of `place_camera` waits for the warm-up's end.
#[allow(clippy::type_complexity)]
fn pre_warm(
    on: bool,
    warming: &mut bool,
    camera: &mut Query<(Entity, &mut Camera, &mut Transform, &mut Projection, Option<&ChildOf>), With<OpticCamera>>,
    main: &Query<(&GlobalTransform, &Projection), (With<MainCamera>, Without<OpticCamera>)>,
) -> bool {
    let Ok((_, mut camera, mut transform, mut projection, parent)) = camera.single_mut() else { return false };
    if !on {
        if *warming {
            *warming = false;
            camera.is_active = false;
        }
        return false;
    }
    // Already placed on a rig: left as the rest of the frame has it.
    if parent.is_some() {
        return false;
    }
    let Ok((place, Projection::Perspective(view))) = main.single() else { return false };
    // Its square picture as wide as the main camera's view across.
    let across = 2.0 * ((view.fov / 2.0).tan() * view.aspect_ratio).atan();
    *transform = place.compute_transform().with_scale(Vec3::ONE);
    if let Projection::Perspective(perspective) = &mut *projection {
        perspective.fov = across.min(170f32.to_radians());
    }
    if !camera.is_active {
        camera.is_active = true;
    }
    if !*warming {
        *warming = true;
        info!("the optic camera on through the warm-up, behind the curtain, so that what it draws with is ready for the first aimed frame");
    }
    true
}
