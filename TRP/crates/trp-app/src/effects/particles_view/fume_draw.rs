//! The fumes' light, uniform and pictures, prepared and drawn each frame.

use super::*;

/// The light the range's fumes are lit by, in the shader's terms, with
/// `CUSTOMS LIGHT` off (`ESTIMATE`: the range's lights are this work's, not
/// a level's): its sun a white light of 1, its ambient (`GlobalAmbientLight`
/// 400 to the sun's 12,000 lux) a thirtieth of that, which the shader takes
/// twice. With it on, the level's (`fume_light`).
const SUN: f32 = 1.0;

const AMBIENT: f32 = 1.0 / 30.0;

/// The fumes' light with the level's (`Particles/Smoke Distorted`; the
/// lighting spec's table): twice `glstate_lightmodel_ambient` (the level's
/// flat ambient), at least the material's `_Ambient`, and the sun's colour
/// as a vertex light times one less `_DirectionLightShadow`.
fn fume_light(level: &crate::rendering::lighting::LevelLighting, own_ambient: f32) -> Vec4 {
    let share = 1.0 - level.direction_light_shadow;
    let c = |i: usize| level.flat_ambient[i].max(own_ambient) + level.sun[i] * share;
    Vec4::new(c(0), c(1), c(2), 1.0)
}

/// What the fumes' shader is given (`Particles/Smoke Distorted`'s figures
/// and the camera's).
#[derive(Clone, Copy, Default, ShaderType)]
pub struct FumeUniform {
    pub view_from_world: Mat4,
    pub clip_from_view: Mat4,
    /// `_Time.x`, `_AnimationSpeed`, `_Distortion`, `_InvFade`.
    pub figures: Vec4,
    pub light: Vec4,
    /// `_TintColor`.
    pub tint: Vec4,
    /// The camera's near plane; the renderer's least and most size on the
    /// screen (`explosion_view::held_size`, kept finite); the projection's
    /// width scale.
    pub near: Vec4,
}

/// The fumes to draw this frame: the figures, the pictures and each
/// particle's four corners (place, corner, size, turn, colour: eleven
/// numbers each).
#[derive(Clone)]
pub struct FumeDraw {
    pub uniform: FumeUniform,
    pub picture: Handle<Image>,
    pub noise: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
}

/// On the camera: the fumes while any shows.
#[derive(Component, Clone, Default)]
pub struct FumeScreen(pub Option<FumeDraw>);

impl ExtractComponent for FumeScreen {
    type QueryData = &'static FumeScreen;
    type QueryFilter = ();
    type Out = FumeScreen;

    /// Only while a fume shows.
    fn extract_component(fume: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        fume.0.as_ref().is_some_and(|draw| !draw.corners.is_empty()).then(|| fume.clone())
    }
}

/// The shared system and what the weapons' fumes draw from: `FastRndom`,
/// the engine's own draws (`Random.Range`), each fume's place as last
/// noted, the shots seen, the material's pictures and figures.
#[derive(Resource)]
pub struct Fumes {
    pub(super) system: ParticleSystem,
    pub(super) fast: FastRandom,
    pub(super) rng: Rng,
    noted: HashMap<Entity, [f32; 3]>,
    pub(super) shots_seen: [u32; 2],
    pub(super) picture: Handle<Image>,
    pub(super) noise: Handle<Image>,
    pub(super) figures: [f32; 3],
    pub(super) tint: Vec4,
    /// The material's least ambient (`_Ambient`).
    ambient: f32,
}

/// A picture of the effects' as its own settings say it is sampled, read
/// raw (drawn on the gamma-encoded frame).
pub(crate) fn picture(texture: &trp_data::unity::extract::TextureData, images: &mut impl crate::assets::background::AddImage) -> Option<Handle<Image>> {
    let raw = trp_data::unity::extract::TextureData { srgb: false, ..texture.clone() };
    let mut image = crate::assets::render_data::build_image(&raw, crate::assets::render_data::Role::Color).map_err(|e| warn!("a picture of the fumes': {e:#}")).ok()?;
    let wrap = |clamped: bool| if clamped { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
    let filter = if texture.point { ImageFilterMode::Nearest } else { ImageFilterMode::Linear };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: wrap(texture.clamp[0]),
        address_mode_v: wrap(texture.clamp[1]),
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: filter,
        ..default()
    });
    Some(images.add_image(image))
}

/// A cube of the effects' (a material's `_Cube`), read raw as the pictures
/// are: its six faces one after another, each with all its levels (`+x`,
/// `−x`, `+y`, `−y`, `+z`, `−z`, as Unity and the graphics card both order
/// them), decoded to their colours. `PROVISIONAL`: each face's rows in their
/// stored order (bottom first, as every picture of the game's).
pub(crate) fn cube_picture(texture: &trp_data::unity::extract::TextureData, images: &mut impl crate::assets::background::AddImage) -> Option<Handle<Image>> {
    use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
    let face = trp_data::unity::extract::TextureData { faces: 1, data: Vec::new(), ..texture.clone() };
    let levels = trp_data::unity::texture::mips(&face).ok()?;
    let face_size = levels.last().map(|m| m.offset + m.size)?;
    if texture.faces != 6 || texture.data.len() < face_size * 6 {
        warn!("the cube {} is not six faces of {face_size} bytes: left white", texture.name);
        return None;
    }
    let mut data = Vec::new();
    for f in 0..6 {
        let one = trp_data::unity::extract::TextureData { faces: 1, data: texture.data[f * face_size..(f + 1) * face_size].to_vec(), ..texture.clone() };
        for level in &levels {
            data.extend(trp_data::unity::texture::decode_rgba(&one, *level).ok()?);
        }
    }
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d { width: texture.width, height: texture.height, depth_or_array_layers: 6 },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = levels.len() as u32;
    image.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { mag_filter: ImageFilterMode::Linear, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Linear, ..default() });
    Some(images.add_image(image))
}

/// A white cube, for the effects whose shader reads none.
pub(crate) fn white_cube(images: &mut impl crate::assets::background::AddImage) -> Handle<Image> {
    use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
        bevy::render::render_resource::TextureDimension::D2,
        vec![255; 24],
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    images.add_image(image)
}

/// The shared system read, at the start.
pub(crate) fn prepare_fumes(unity: &mut trp_data::unity::Assets, images: &mut impl crate::assets::background::AddImage) -> Option<Fumes> {
    let data = match trp_data::drawing::particles::muzzle_fume_system(unity) {
        Ok(data) => data,
        Err(e) => {
            warn!("the muzzles' fumes are not drawn: their system could not be read: {e:#}");
            return None;
        }
    };
    for module in &data.unbuilt {
        warn!("{}: its {module} is on and not built: left out", data.name);
    }
    let m = &data.renderer.material;
    let (Some(picture), Some(noise)) = (m.texture("_MainTex").and_then(|t| picture(t, images)), m.texture("_NoiseTex").and_then(|t| picture(t, images))) else {
        warn!("the muzzles' fumes are not drawn: their material {} has not its two pictures", m.name);
        return None;
    };
    let figures = [m.float("_AnimationSpeed").unwrap_or(0.0), m.float("_Distortion").unwrap_or(0.0), m.float("_InvFade").unwrap_or(1.0)];
    let tint = Vec4::from(m.color("_TintColor").unwrap_or([1.0; 4]));
    let ambient = m.float("_Ambient").unwrap_or(0.0);
    info!(
        "the muzzles' fumes: {} ({}, at most {}; on the screen {} to {} of the view's width), its material {} with {}",
        data.name, data.renderer.material.shader, data.max_particles, data.renderer.min_size, data.renderer.max_size, m.name, m.shader
    );
    Some(Fumes {
        system: ParticleSystem::new(Arc::new(data)),
        fast: FastRandom::default(),
        rng: Rng::new(0x0f0e_5eed),
        noted: HashMap::new(),
        shots_seen: [0; 2],
        picture,
        noise,
        figures,
        tint,
        ambient,
    })
}

/// A frame, once everything is placed: each shot's puffs from the weapon
/// in hand's fumes, the system stepped, each fume's place noted, and what
/// it draws handed to each camera that draws it (the main camera, and the
/// optic camera while on: the shared system is an object of the world's
/// layer).
pub fn fume_frame(
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    controller: Res<crate::player::controller::Controller>,
    time: Res<Time>,
    mut fumes: ResMut<Fumes>,
    places: Query<&GlobalTransform>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut FumeScreen)>,
    level: Option<Res<crate::rendering::lighting::LevelLighting>>,
    data: Option<Res<crate::GameData>>,
) {
    let fumes = &mut *fumes;
    let game = |v: Vec3| [v.x, v.y, -v.z];
    // Each fume's object: its place, its forward (minus its up), and how
    // far it has moved since it was last noted; of the weapon's muzzle and
    // its launcher's, each by its own shots.
    let mut now = HashMap::new();
    for (parts, shots) in crate::weapons::muzzle_view::fired_muzzles(rig.as_deref(), &controller, &mut fumes.shots_seen) {
        for (index, entity) in parts.fumes.iter().enumerate() {
            let Some((entity, place)) = entity.and_then(|entity| Some((entity, places.get(entity).ok()?))) else { continue };
            let at = game(place.translation());
            let forward = game(-place.up().as_vec3());
            let moved = fumes.noted.get(&entity).map_or([0.0; 3], |before| std::array::from_fn(|k| at[k] - before[k]));
            now.insert(entity, at);
            for _ in 0..shots.min(8) {
                let fume = &parts.data.fumes[index];
                for puff in trp_sim::weapons::muzzle::fume_puffs(fume, at, forward, moved, &mut fumes.fast, &mut fumes.rng) {
                    fumes.system.emit(puff);
                }
            }
        }
    }
    fumes.noted = now;
    // A long frame in equal steps (particles spec, "A long frame").
    let most = data.as_ref().map_or(f32::MAX, |g| g.engine.maximum_particle_timestep);
    fumes.system.step_frame(time.delta_secs(), most, None);

    // The particles' corners once, in the world: each camera's shader
    // turns them to face it.
    // Four corners a particle, each of ±0.5 across and up.
    const CORNERS: [[f32; 2]; 4] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
    let corners: Option<Arc<Vec<f32>>> = (!fumes.system.particles.is_empty()).then(|| {
        Arc::new(
            fumes
                .system
                .drawn()
                .flat_map(|p| {
                    let at = crate::platform::convert::position(p.position);
                    CORNERS.into_iter().flat_map(move |c| [at.x, at.y, at.z, c[0], c[1], p.size, p.rotation.to_radians(), p.color[0], p.color[1], p.color[2], p.color[3]])
                })
                .collect(),
        )
    });
    let light = match level.as_deref() {
        Some(level) => fume_light(level, fumes.ambient),
        None => Vec4::new(SUN + 2.0 * AMBIENT, SUN + 2.0 * AMBIENT, SUN + 2.0 * AMBIENT, 1.0),
    };
    // The renderer's least and most size, kept finite for the shader (the
    // engine's `MakeMinMaxPlaneValuesFinite`; one not read is none).
    let renderer = &fumes.system.data.renderer;
    let (least, most) = (renderer.min_size.clamp(-1e30, 1e30), renderer.max_size.clamp(-1e30, 1e30));
    for (camera, eye, projection, mut screen) in &mut cameras {
        let Some(corners) = corners.as_ref().filter(|_| camera.is_active) else {
            if screen.0.is_some() {
                screen.0 = None;
            }
            continue;
        };
        screen.0 = Some(FumeDraw {
            uniform: FumeUniform {
                view_from_world: eye.to_matrix().inverse(),
                clip_from_view: camera.clip_from_view(),
                figures: Vec4::new(time.elapsed_secs() / 20.0, fumes.figures[0], fumes.figures[1], fumes.figures[2]),
                light,
                tint: fumes.tint,
                near: Vec4::new(crate::weapons::muzzle_view::near_of(projection), least, most, camera.clip_from_view().x_axis.x),
            },
            picture: fumes.picture.clone(),
            noise: fumes.noise.clone(),
            corners: corners.clone(),
        });
    }
}
