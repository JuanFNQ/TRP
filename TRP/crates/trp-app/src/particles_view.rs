//! The particle system's effects (particles spec; the options'
//! `PARTICLES`): the muzzles' fumes (firing spec, "The muzzle's fumes"),
//! sparks ("The muzzle's sparks") and heat haze ("The muzzle's heat haze",
//! drawn by the camera's distortion: `distort_view`).
//! The shared system all the weapons' fumes are emitted into is read once
//! from the install (`trp_data::particles::muzzle_fume_system`); each shot
//! of the weapon in hand emits its fumes' puffs into it by the game's rule
//! (`trp_sim::muzzle::fume_puffs`), and it is stepped each frame
//! (`trp_sim::particles`). What it draws is handed to the camera
//! (`FumeScreen`) and drawn by the muzzle's pass (`muzzle_view`, before the
//! flash's jets), with the game's shader (`smoke_distorted.wgsl`).

use std::collections::HashMap;
use std::sync::Arc;

use bevy::ecs::query::QueryItem;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::ShaderType;
use trp_sim::muzzle::FastRandom;
use trp_sim::particles::ParticleSystem;
use trp_sim::recoil::Rng;
use trp_sim::sparks::SparksRing;

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
fn fume_light(level: &crate::lighting::LevelLighting, own_ambient: f32) -> Vec4 {
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
    /// The camera's near plane.
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
    system: ParticleSystem,
    fast: FastRandom,
    rng: Rng,
    noted: HashMap<Entity, [f32; 3]>,
    shots_seen: u32,
    picture: Handle<Image>,
    noise: Handle<Image>,
    figures: [f32; 3],
    tint: Vec4,
    /// The material's least ambient (`_Ambient`).
    ambient: f32,
}

/// A picture of the effects' as its own settings say it is sampled, read
/// raw (drawn on the gamma-encoded frame).
pub(crate) fn picture(texture: &trp_data::unity::extract::TextureData, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let raw = trp_data::unity::extract::TextureData { srgb: false, ..texture.clone() };
    let mut image = crate::render_data::build_image(&raw, crate::render_data::Role::Color).map_err(|e| warn!("a picture of the fumes': {e:#}")).ok()?;
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
    Some(images.add(image))
}

/// A cube of the effects' (a material's `_Cube`), read raw as the pictures
/// are: its six faces one after another, each with all its levels (`+x`,
/// `−x`, `+y`, `−y`, `+z`, `−z`, as Unity and the graphics card both order
/// them), decoded to their colours. `PROVISIONAL`: each face's rows in their
/// stored order (bottom first, as every picture of the game's).
pub(crate) fn cube_picture(texture: &trp_data::unity::extract::TextureData, images: &mut Assets<Image>) -> Option<Handle<Image>> {
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
    Some(images.add(image))
}

/// A white cube, for the effects whose shader reads none.
pub(crate) fn white_cube(images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
        bevy::render::render_resource::TextureDimension::D2,
        vec![255; 24],
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    images.add(image)
}

/// The shared system read, at the start.
pub fn read_fumes(mut commands: Commands, mut game: ResMut<crate::GameData>, mut images: ResMut<Assets<Image>>) {
    let data = match trp_data::particles::muzzle_fume_system(&mut game.unity) {
        Ok(data) => data,
        Err(e) => {
            warn!("the muzzles' fumes are not drawn: their system could not be read: {e:#}");
            return;
        }
    };
    for module in &data.unbuilt {
        warn!("{}: its {module} is on and not built: left out", data.name);
    }
    let m = &data.renderer.material;
    let (Some(picture), Some(noise)) = (m.texture("_MainTex").and_then(|t| picture(t, &mut images)), m.texture("_NoiseTex").and_then(|t| picture(t, &mut images))) else {
        warn!("the muzzles' fumes are not drawn: their material {} has not its two pictures", m.name);
        return;
    };
    let figures = [m.float("_AnimationSpeed").unwrap_or(0.0), m.float("_Distortion").unwrap_or(0.0), m.float("_InvFade").unwrap_or(1.0)];
    let tint = Vec4::from(m.color("_TintColor").unwrap_or([1.0; 4]));
    let ambient = m.float("_Ambient").unwrap_or(0.0);
    info!("the muzzles' fumes: {} ({}, at most {}), its material {} with {}", data.name, data.renderer.material.shader, data.max_particles, m.name, m.shader);
    commands.insert_resource(Fumes {
        system: ParticleSystem::new(Arc::new(data)),
        fast: FastRandom::default(),
        rng: Rng::new(0x0f0e_5eed),
        noted: HashMap::new(),
        shots_seen: 0,
        picture,
        noise,
        figures,
        tint,
        ambient,
    });
}

/// A frame, once everything is placed: each shot's puffs from the weapon
/// in hand's fumes, the system stepped, each fume's place noted, and what
/// it draws handed to each camera that draws it (the main camera, and the
/// optic camera while on: the shared system is an object of the world's
/// layer).
pub fn fume_frame(
    rig: Option<Res<crate::weapon::WeaponRig>>,
    controller: Res<crate::controller::Controller>,
    time: Res<Time>,
    mut fumes: ResMut<Fumes>,
    places: Query<&GlobalTransform>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut FumeScreen)>,
    level: Option<Res<crate::lighting::LevelLighting>>,
) {
    let fumes = &mut *fumes;
    let shots = controller.shots_fired.wrapping_sub(fumes.shots_seen);
    fumes.shots_seen = controller.shots_fired;
    let game = |v: Vec3| [v.x, v.y, -v.z];
    let parts = rig.as_ref().and_then(|rig| rig.muzzle.clone());
    // Each fume's object: its place, its forward (minus its up), and how
    // far it has moved since it was last noted.
    let mut now = HashMap::new();
    if let Some(parts) = &parts {
        for (index, entity) in parts.fumes.iter().enumerate() {
            let Some((entity, place)) = entity.and_then(|entity| Some((entity, places.get(entity).ok()?))) else { continue };
            let at = game(place.translation());
            let forward = game(-place.up().as_vec3());
            let moved = fumes.noted.get(&entity).map_or([0.0; 3], |before| std::array::from_fn(|k| at[k] - before[k]));
            now.insert(entity, at);
            for _ in 0..shots.min(8) {
                let fume = &parts.data.fumes[index];
                for puff in trp_sim::muzzle::fume_puffs(fume, at, forward, moved, &mut fumes.fast, &mut fumes.rng) {
                    fumes.system.emit(puff);
                }
            }
        }
    }
    fumes.noted = now;
    fumes.system.step(time.delta_secs());

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
                    let at = crate::convert::position(p.position);
                    CORNERS.into_iter().flat_map(move |c| [at.x, at.y, at.z, c[0], c[1], p.size, p.rotation.to_radians(), p.color[0], p.color[1], p.color[2], p.color[3]])
                })
                .collect(),
        )
    });
    let light = match level.as_deref() {
        Some(level) => fume_light(level, fumes.ambient),
        None => Vec4::new(SUN + 2.0 * AMBIENT, SUN + 2.0 * AMBIENT, SUN + 2.0 * AMBIENT, 1.0),
    };
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
                near: Vec4::new(crate::muzzle_view::near_of(projection), 0.0, 0.0, 0.0),
            },
            picture: fumes.picture.clone(),
            noise: fumes.noise.clone(),
            corners: corners.clone(),
        });
    }
}

/// What the sparks' shader is given (`Custom/SimpleParticleMoveAdd`'s
/// figures and the camera's).
#[derive(Clone, Copy, Default, ShaderType)]
pub struct SparkUniform {
    pub clip_from_world: Mat4,
    /// `_MainTime`, `_LastMainTime`, `_Size`, `_FadeOut`.
    pub times: Vec4,
    /// The turbulence's two frequencies and amplitudes.
    pub turbulence: Vec4,
    /// `_TintColor`.
    pub tint: Vec4,
}

/// The sparks to draw this frame: the figures, the picture, each live
/// spark's streak's corners (place, picture, offset, share and draw, born,
/// velocity, bytes: twenty numbers each) and their triangles.
#[derive(Clone)]
pub struct SparkDraw {
    pub uniform: SparkUniform,
    pub picture: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
    pub indices: Arc<Vec<u32>>,
}

/// On the camera: the sparks while any may show.
#[derive(Component, Clone, Default)]
pub struct SparkScreen(pub Option<SparkDraw>);

impl ExtractComponent for SparkScreen {
    type QueryData = &'static SparkScreen;
    type QueryFilter = ();
    type Out = SparkScreen;

    fn extract_component(sparks: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        sparks.0.as_ref().is_some_and(|draw| !draw.indices.is_empty()).then(|| sparks.clone())
    }
}

/// The shared sparks' ring and what the weapons' sparks draw from:
/// `FastRndom`, the engine's own draws, the shots seen, the last frame's
/// time, the renderer's picture and figures, and one streak's corners and
/// triangles.
#[derive(Resource)]
pub struct Sparks {
    ring: SparksRing,
    fast: FastRandom,
    rng: Rng,
    shots_seen: u32,
    last_time: f32,
    picture: Handle<Image>,
    uniform: SparkUniform,
    corners: Vec<trp_sim::sparks::Corner>,
    indices: Vec<u32>,
}

impl Sparks {
    /// A spark into the shared ring (an older emitter's: `explosion_view`).
    pub fn emit(&mut self, spark: trp_sim::sparks::SparkEmit) {
        self.ring.emit(spark);
    }
}

/// The shared sparks' renderer read, at the start.
pub fn read_sparks(mut commands: Commands, mut game: ResMut<crate::GameData>, mut images: ResMut<Assets<Image>>) {
    let data = match trp_data::particles::muzzle_sparks_renderer(&mut game.unity) {
        Ok(data) => data,
        Err(e) => {
            warn!("the muzzles' sparks are not drawn: their renderer could not be read: {e:#}");
            return;
        }
    };
    let Some(picture) = picture(&data.texture, &mut images) else { return };
    info!("the muzzles' sparks: {} of {} segments ({})", data.count, data.segments, data.shader);
    let mut rng = Rng::new(0x5_9a2c);
    commands.insert_resource(Sparks {
        ring: SparksRing::new(data.count, data.segments, &mut rng),
        fast: FastRandom::default(),
        rng,
        shots_seen: 0,
        last_time: 0.0,
        picture,
        uniform: SparkUniform { times: Vec4::new(0.0, 0.0, data.size, data.fade_out), turbulence: Vec4::from(data.turbulence.unwrap_or([0.0; 4])), tint: Vec4::from(data.tint), ..default() },
        corners: trp_sim::sparks::corners(data.segments),
        indices: trp_sim::sparks::indices(data.segments),
    });
}

/// A frame, once everything is placed: each shot's sparks from the weapon
/// in hand's, and every spark that may show handed to each camera that
/// draws them (the main camera, and the optic camera while on: the sparks'
/// renderer is an object of the world's layer).
pub fn spark_frame(
    rig: Option<Res<crate::weapon::WeaponRig>>,
    controller: Res<crate::controller::Controller>,
    time: Res<Time>,
    mut sparks: ResMut<Sparks>,
    places: Query<&GlobalTransform>,
    mut cameras: Query<(&Camera, &GlobalTransform, &mut SparkScreen)>,
) {
    let sparks = &mut *sparks;
    let now = time.elapsed_secs();
    let shots = controller.shots_fired.wrapping_sub(sparks.shots_seen);
    sparks.shots_seen = controller.shots_fired;
    let game = |v: Vec3| [v.x, v.y, -v.z];
    if let Some(parts) = rig.as_ref().and_then(|rig| rig.muzzle.as_ref()) {
        for (index, entity) in parts.sparks.iter().enumerate() {
            let Some(place) = entity.and_then(|entity| places.get(entity).ok()) else { continue };
            for _ in 0..shots.min(8) {
                for spark in trp_sim::muzzle::spark_emits(&parts.data.sparks[index], game(place.translation()), game(-place.up().as_vec3()), now, &mut sparks.fast, &mut sparks.rng) {
                    sparks.ring.emit(spark);
                }
            }
        }
    }
    let last = std::mem::replace(&mut sparks.last_time, now);
    // The streaks' corners once, in the world (each camera's shader makes
    // the streak on its own screen).
    let per = sparks.corners.len() as u32;
    let mut corners = Vec::new();
    let mut indices = Vec::new();
    for (k, (spark, draw)) in sparks.ring.live(now, last).enumerate() {
        let born = [spark.time, spark.lifetime, spark.gravity, spark.drag];
        let bytes = [spark.emission, spark.size, spark.turbulence, spark.frequency].map(|b| b as f32 / 255.0);
        for c in &sparks.corners {
            corners.extend_from_slice(&spark.position);
            corners.extend_from_slice(&c.uv);
            corners.extend_from_slice(&c.offset);
            corners.extend_from_slice(&[c.share, draw]);
            corners.extend_from_slice(&born);
            corners.extend_from_slice(&spark.velocity);
            corners.extend_from_slice(&bytes);
        }
        indices.extend(sparks.indices.iter().map(|i| i + k as u32 * per));
    }
    let shown = (!indices.is_empty()).then(|| (Arc::new(corners), Arc::new(indices)));
    for (camera, eye, mut screen) in &mut cameras {
        let Some((corners, indices)) = shown.as_ref().filter(|_| camera.is_active) else {
            if screen.0.is_some() {
                screen.0 = None;
            }
            continue;
        };
        screen.0 = Some(SparkDraw {
            uniform: SparkUniform { clip_from_world: camera.clip_from_view() * eye.to_matrix().inverse(), times: Vec4::new(now, last, sparks.uniform.times.z, sparks.uniform.times.w), ..sparks.uniform },
            picture: sparks.picture.clone(),
            corners: corners.clone(),
            indices: indices.clone(),
        });
    }
}

/// What the haze's shader is given (`Particles/DistortAndBlur`'s figures
/// and the camera's).
#[derive(Clone, Copy, Default, ShaderType)]
pub struct HazeUniform {
    pub view_from_world: Mat4,
    pub clip_from_view: Mat4,
    /// `_Time.x`, `_Distort`, `_Blur`, `_InvFade` × 10.
    pub figures: Vec4,
    /// `_TextureMovement`.
    pub movement: Vec4,
    /// `_DistortTex_ST`.
    pub scale: Vec4,
    /// The camera's near plane, the most size on the screen, the
    /// projection's height scale.
    pub near: Vec4,
}

/// The haze to draw this frame: the figures, its pictures, each puff's
/// corners (as the fumes': eleven numbers each), and the camera's
/// distortion (its pixels, its blur's iterations).
#[derive(Clone)]
pub struct HazeDraw {
    pub uniform: HazeUniform,
    pub noise: Handle<Image>,
    pub alpha: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
    pub distortion: Vec2,
}

/// On the camera: the haze while any shows.
#[derive(Component, Clone, Default)]
pub struct HazeScreen(pub Option<HazeDraw>);

impl ExtractComponent for HazeScreen {
    type QueryData = &'static HazeScreen;
    type QueryFilter = ();
    type Out = HazeScreen;

    fn extract_component(haze: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        haze.0.as_ref().is_some_and(|draw| !draw.corners.is_empty()).then(|| haze.clone())
    }
}

/// The shared haze system, the engine's draws, the shots seen, the puffs
/// due (`WaitForSeconds(DelayInSec)`: when, from which object, by which
/// of the muzzle's), the material's pictures and figures, the camera's
/// distortion.
#[derive(Resource)]
pub struct Hazes {
    system: ParticleSystem,
    rng: Rng,
    shots_seen: u32,
    due: Vec<(f32, Entity, Arc<trp_data::muzzle::MuzzleData>, usize)>,
    noise: Handle<Image>,
    alpha: Handle<Image>,
    uniform: HazeUniform,
    distortion: Vec2,
}

/// The shared haze system and the camera's distortion read, at the start;
/// none where the hard settings have no heat emitters
/// (`HEAT_EMITTER_ENABLED`).
pub fn read_hazes(mut commands: Commands, mut game: ResMut<crate::GameData>, mut images: ResMut<Assets<Image>>) {
    if game.hard.float("HEAT_EMITTER_ENABLED").is_ok_and(|on| on == 0.0) {
        info!("the muzzles' heat haze: off in the hard settings");
        return;
    }
    let data = match trp_data::particles::muzzle_heat_haze_system(&mut game.unity) {
        Ok(data) => data,
        Err(e) => {
            warn!("the muzzles' heat haze is not drawn: its system could not be read: {e:#}");
            return;
        }
    };
    let distortion = match trp_data::camera_effects::read_distortion(&game.unity.install) {
        Ok(d) => d,
        Err(e) => {
            warn!("the muzzles' heat haze is not drawn: the camera's distortion could not be read: {e:#}");
            return;
        }
    };
    for module in &data.unbuilt {
        warn!("{}: its {module} is on and not built: left out", data.name);
    }
    let m = &data.renderer.material;
    let (Some(noise), Some(alpha)) = (m.texture("_DistortTex").and_then(|t| picture(t, &mut images)), m.texture("_AlphaTex").and_then(|t| picture(t, &mut images))) else {
        warn!("the muzzles' heat haze is not drawn: its material {} has not its two pictures", m.name);
        return;
    };
    let uniform = HazeUniform {
        figures: Vec4::new(0.0, m.float("_Distort").unwrap_or(0.0), m.float("_Blur").unwrap_or(0.0), m.float("_InvFade").unwrap_or(1.0) * 10.0),
        movement: Vec4::from(m.color("_TextureMovement").unwrap_or([0.0; 4])),
        scale: Vec4::from(m.scale("_DistortTex").unwrap_or([1.0, 1.0, 0.0, 0.0])),
        near: Vec4::new(0.0, data.renderer.max_size, 1.0, 0.0),
        ..default()
    };
    info!("the muzzles' heat haze: {} ({}, at most {}), the camera's distortion {} pixels, {} blur", data.name, m.shader, data.max_particles, distortion.pixels, distortion.blur_iterations);
    commands.insert_resource(Hazes {
        system: ParticleSystem::new(Arc::new(data)),
        rng: Rng::new(0x4a2e),
        shots_seen: 0,
        due: Vec::new(),
        noise,
        alpha,
        uniform,
        distortion: Vec2::new(distortion.pixels, distortion.blur_iterations as f32),
    });
}

/// A frame, once everything is placed: each shot's haze that its chance
/// gives queued for its delay; the puffs due emitted from where their
/// object is now; the system stepped; what it draws handed to each camera
/// that draws it. Only the main camera has its screen: the game's haze is
/// drawn by that camera's distortion alone (`DistortCameraFX`, which the
/// optic camera has not; its renderer's own material draws nothing).
#[allow(clippy::too_many_arguments)]
pub fn haze_frame(
    rig: Option<Res<crate::weapon::WeaponRig>>,
    controller: Res<crate::controller::Controller>,
    time: Res<Time>,
    mut hazes: ResMut<Hazes>,
    places: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut HazeScreen)>,
) {
    let hazes = &mut *hazes;
    let now = time.elapsed_secs();
    let shots = controller.shots_fired.wrapping_sub(hazes.shots_seen);
    hazes.shots_seen = controller.shots_fired;
    if let Some(parts) = rig.as_ref().and_then(|rig| rig.muzzle.as_ref()) {
        for _ in 0..shots.min(8) {
            for (index, entity) in parts.hazes.iter().enumerate() {
                let Some(entity) = *entity else { continue };
                let haze = &parts.data.hazes[index];
                if trp_sim::muzzle::haze_shot(haze, &mut hazes.rng) {
                    hazes.due.push((now + haze.delay, entity, parts.data.clone(), index));
                }
            }
        }
    }
    let game = |v: Vec3| [v.x, v.y, -v.z];
    let mut waiting = Vec::new();
    for (when, entity, data, index) in std::mem::take(&mut hazes.due) {
        if when > now {
            waiting.push((when, entity, data, index));
            continue;
        }
        // Its object gone (the weapon put away), nothing.
        let Ok(place) = places.get(entity) else { continue };
        let parent = parents.get(entity).ok().and_then(|child| places.get(child.parent()).ok());
        let from_parent = parent.map_or([0.0; 3], |p| {
            let (a, b) = (game(place.translation()), game(p.translation()));
            std::array::from_fn(|k| a[k] - b[k])
        });
        for puff in trp_sim::muzzle::haze_puffs(&data.hazes[index], game(place.translation()), from_parent, &mut hazes.rng) {
            hazes.system.emit(puff);
        }
    }
    hazes.due = waiting;
    hazes.system.step(time.delta_secs());

    const CORNERS: [[f32; 2]; 4] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
    let corners: Option<Arc<Vec<f32>>> = (!hazes.system.particles.is_empty()).then(|| {
        Arc::new(
            hazes
                .system
                .drawn()
                .flat_map(|p| {
                    let at = crate::convert::position(p.position);
                    CORNERS.into_iter().flat_map(move |c| [at.x, at.y, at.z, c[0], c[1], p.size, p.rotation.to_radians(), p.color[0], p.color[1], p.color[2], p.color[3]])
                })
                .collect(),
        )
    });
    for (camera, eye, projection, mut screen) in &mut cameras {
        let Some(corners) = corners.as_ref().filter(|_| camera.is_active) else {
            if screen.0.is_some() {
                screen.0 = None;
            }
            continue;
        };
        let clip_from_view = camera.clip_from_view();
        let mut uniform = hazes.uniform;
        uniform.view_from_world = eye.to_matrix().inverse();
        uniform.clip_from_view = clip_from_view;
        uniform.figures.x = now / 20.0;
        uniform.near = Vec4::new(crate::muzzle_view::near_of(projection), uniform.near.y, clip_from_view.y_axis.y, 0.0);
        screen.0 = Some(HazeDraw { uniform, noise: hazes.noise.clone(), alpha: hazes.alpha.clone(), corners: corners.clone(), distortion: hazes.distortion });
    }
}

pub struct ParticlesViewPlugin {
    /// The fumes read and drawn (`PARTICLES`, where the hard settings have
    /// shot effects); off, nothing of them is read, and the muzzle's pass
    /// never draws them.
    pub on: bool,
}

impl Plugin for ParticlesViewPlugin {
    fn build(&self, app: &mut App) {
        if self.on {
            app.add_plugins((ExtractComponentPlugin::<FumeScreen>::default(), ExtractComponentPlugin::<SparkScreen>::default(), ExtractComponentPlugin::<HazeScreen>::default()))
                .add_systems(Startup, (read_fumes, read_sparks, read_hazes))
                .add_systems(
                    PostUpdate,
                    (fume_frame.run_if(resource_exists::<Fumes>), spark_frame.run_if(resource_exists::<Sparks>), haze_frame.run_if(resource_exists::<Hazes>))
                        .after(bevy::transform::TransformSystems::Propagate)
                        .after(bevy::camera::CameraUpdateSystems)
                        .after(crate::optic_view::squeeze_drawn),
                );
        }
    }
}
