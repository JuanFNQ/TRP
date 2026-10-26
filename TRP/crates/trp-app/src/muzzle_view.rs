//! A weapon's muzzle flash (firing spec, "The muzzle flash"; the options'
//! `MUZZLE FLASH`): the jets' mesh the rig was made with
//! (`weapon::MuzzleParts`, `trp_sim::muzzle::jet_mesh`), drawn by the jets'
//! shader (`muzzle_view.wgsl`, the game's `Custom/Additive`) at the
//! weapon's `weapon` object, each shot giving it new draws and its time.
//!
//! The game adds the flash's numbers to its frame in gamma space (its
//! project is: `gamma_ui`). Added to Bevy's linear frame in its own pass
//! the flash came out several times dimmer over the bright range. So it is
//! drawn after the tonemapping, onto the frame encoded for the camera's
//! effects, before the first of them (as the game draws it before its image
//! effects), its depth tested by hand against a single-sample copy of the
//! scene's (`scene_depth.wgsl`). `PROVISIONAL`: after Bevy's tonemapping,
//! where the game's flash goes through its own.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_3d, texture_depth_2d, texture_depth_2d_multisampled, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{CachedTexture, GpuImage, TextureCache};
use bevy::render::view::{Msaa, ViewDepthTexture, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, ShaderDefVal};
use trp_data::muzzle::MuzzleData;
use trp_sim::muzzle::{JetVertex, MuzzleFlash, Smoke};
use trp_sim::recoil::Rng;

use crate::convert;

const JET_SHADER: Handle<Shader> = uuid_handle!("9a3e6c27-4d81-4b5f-8e2a-0c7b5d1f3e96");
const DEPTH_SHADER: Handle<Shader> = uuid_handle!("2c84f1d3-6e59-4a07-b3d8-91e5c7a4f206");
const SMOKE_SHADER: Handle<Shader> = uuid_handle!("6f0b3d92-8a14-4c75-9e3b-d27a5c8e1f40");
const FUME_SHADER: Handle<Shader> = uuid_handle!("c41a7e5d-2b96-4f83-a0d7-5e8c3b19f6a2");
const SPARKS_SHADER: Handle<Shader> = uuid_handle!("7d2e9b40-5c13-4a86-b1f7-3e60a8d4c295");
/// The tactical devices' lasers (`tactical_view`), drawn in this pass.
const LASER_SHADER: Handle<Shader> = uuid_handle!("5b8e1d47-9a26-4c3f-8e71-2d4a6f0c9b13");

/// What the smoke's shader is given for one trail (`Custom/MuzzleSmoke`'s
/// figures and the camera's).
#[derive(Clone, Copy, Default, ShaderType)]
pub struct SmokeUniform {
    pub view_from_world: Mat4,
    pub clip_from_view: Mat4,
    /// `_Width`, `_UVYScale`, `_DiffusionStrength`, `_StartFade`.
    pub a: Vec4,
    /// `_EndFade`, `_End`, `_InvFade`, `_Alpha`.
    pub b: Vec4,
    /// The camera's near plane.
    pub near: Vec4,
}

/// One trail to draw: its figures, its picture and its corners (place,
/// side, number, diffusion, direction, alpha: ten numbers each).
#[derive(Clone)]
pub struct SmokeDraw {
    pub uniform: SmokeUniform,
    pub picture: Handle<Image>,
    pub corners: Arc<Vec<f32>>,
}

/// On the camera while any trail shows: the trails.
#[derive(Component, Clone, Default)]
pub struct SmokeScreen(pub Vec<SmokeDraw>);

impl ExtractComponent for SmokeScreen {
    type QueryData = &'static SmokeScreen;
    type QueryFilter = ();
    type Out = SmokeScreen;

    /// Only while a trail shows.
    fn extract_component(smoke: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (!smoke.0.is_empty()).then(|| smoke.clone())
    }
}

/// What the jets' shader is given this frame (its figures, `Custom/
/// Additive`'s, and where the mesh and the camera stand).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct JetUniform {
    pub clip_from_world: Mat4,
    pub world_from_local: Mat4,
    pub projection: Vec4,
    pub tint: Vec4,
    pub rot_mx: Vec4,
    pub random_vals: Vec4,
    /// `_CellShift`, `_InvWindowSize`, `_RandomizePhase`.
    pub cell: Vec4,
    /// `_ShotVals`.
    pub shot: Vec4,
    /// `_ZShift`, `_HDR`.
    pub z_hdr: Vec4,
}

/// Which rig's jets the camera draws this frame, by the entity they hang
/// from.
#[derive(Component, Clone, Copy)]
pub struct JetRig(pub Entity);

/// On the camera while a flash shows: its figures and its rig.
#[derive(Component, Clone, Copy, Default)]
pub struct JetScreen {
    pub uniform: JetUniform,
    pub rig: Option<Entity>,
}

impl ExtractComponent for JetScreen {
    type QueryData = &'static JetScreen;
    type QueryFilter = ();
    type Out = (JetUniform, JetRig);

    /// Only while a flash shows.
    fn extract_component(jets: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        jets.rig.map(|rig| (jets.uniform, JetRig(rig)))
    }
}

/// Each rig's jets: their corners and indices, and their picture.
#[derive(Clone)]
pub struct JetMesh {
    pub vertices: Arc<Vec<JetVertex>>,
    pub indices: Arc<Vec<u32>>,
    pub picture: Handle<Image>,
}

/// A picture of the muzzle's, read as stored (the game's gamma-space
/// project), its cells tiled.
fn tiled(texture: &trp_data::unity::extract::TextureData, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let raw = trp_data::unity::extract::TextureData { srgb: false, ..texture.clone() };
    let mut image = crate::render_data::build_image(&raw, crate::render_data::Role::Color).map_err(|e| warn!("a picture of the muzzle's: {e:#}")).ok()?;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    Some(images.add(image))
}

/// The rigs' jets, by the entity each hangs from.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct JetMeshes(pub HashMap<Entity, JetMesh>);

/// A rig's flash: its figures, its shot's time and its draws; its gun
/// smokes and their pictures.
struct Flash {
    data: Arc<MuzzleData>,
    flash: MuzzleFlash,
    figures: JetUniform,
    smokes: Vec<Smoke>,
    smoke_pictures: Vec<Option<Handle<Image>>>,
    /// The time the trails have still to age by, and where each trail's
    /// object was at the last frame (`smoke_frame`).
    smoke_clock: f32,
    smoke_last: Vec<Option<[f32; 3]>>,
}

/// Each rig's flash, by the entity its mesh hangs from; the draws; the
/// shots counted so far; the rig in hand when last seen.
#[derive(Resource)]
pub struct MuzzleViews {
    flashes: HashMap<Entity, Flash>,
    rng: Rng,
    shots_seen: u32,
    in_hand: Option<Entity>,
}

impl Default for MuzzleViews {
    fn default() -> Self {
        Self { flashes: HashMap::new(), rng: Rng::new(0x4d55_5a5a), shots_seen: 0, in_hand: None }
    }
}

/// The flash's frame: the rig in hand's jets taken in the first time it is
/// seen (their picture read as stored, as the game's gamma-space project
/// reads it, its cells tiled), and each shot since the last frame given its
/// draws and its time.
pub fn muzzle_flash(
    rig: Option<Res<crate::weapon::WeaponRig>>,
    controller: Res<crate::controller::Controller>,
    time: Res<Time>,
    mut views: ResMut<MuzzleViews>,
    mut meshes: ResMut<JetMeshes>,
    mut images: ResMut<Assets<Image>>,
) {
    let views = &mut *views;
    let shots = controller.shots_fired.wrapping_sub(views.shots_seen);
    views.shots_seen = controller.shots_fired;
    let parts = rig.as_ref().and_then(|rig| rig.muzzle.clone());
    // The weapon put away: its trails cleared (`MuzzleEffectsClear`, as its
    // manager is switched off).
    let in_hand = parts.as_ref().map(|parts| parts.parent);
    if views.in_hand != in_hand {
        if let Some(flash) = views.in_hand.and_then(|before| views.flashes.get_mut(&before)) {
            flash.smokes.iter_mut().for_each(Smoke::clear);
            flash.smoke_clock = 0.0;
            flash.smoke_last.iter_mut().for_each(|last| *last = None);
        }
        views.in_hand = in_hand;
    }
    let Some(parts) = parts else { return };
    if !views.flashes.contains_key(&parts.parent) {
        let m = &parts.data.material;
        // A muzzle with no jets (most suppressed weapons') has no mesh: its
        // gun smoke is all there is to it.
        if !parts.vertices.is_empty() && !parts.indices.is_empty() {
            let Some(picture) = tiled(&m.texture, &mut images) else { return };
            meshes.0.insert(parts.parent, JetMesh { vertices: parts.vertices.clone(), indices: parts.indices.clone(), picture });
        }
        let figures = JetUniform {
            tint: Vec4::from(m.tint),
            cell: Vec4::new(0.0, 0.0, m.inv_window, m.randomize_phase),
            shot: Vec4::new(1.0, 0.0, 0.0, 0.0),
            z_hdr: Vec4::new(m.z_shift, m.hdr, 0.0, 0.0),
            ..default()
        };
        info!("the muzzle flash: {} jets, {} quads, {} gun smokes", parts.data.jets.len(), parts.vertices.len() / 4, parts.data.smokes.len());
        let smoke_pictures = parts.data.smokes.iter().map(|smoke| tiled(&smoke.material.texture, &mut images)).collect();
        let count = parts.data.smokes.len();
        views.flashes.insert(parts.parent, Flash { data: parts.data.clone(), flash: MuzzleFlash::default(), figures, smokes: vec![Smoke::default(); count], smoke_pictures, smoke_clock: 0.0, smoke_last: vec![None; count] });
    }
    let Some(flash) = views.flashes.get_mut(&parts.parent) else { return };
    let now = time.elapsed_secs();
    for _ in 0..shots {
        let draws = flash.flash.shot(&flash.data, now, &mut views.rng);
        flash.figures.random_vals = Vec4::from(draws.random_vals);
        flash.figures.rot_mx = Vec4::from(draws.rot_mx);
        flash.figures.cell.x = draws.cell_shift[0];
        flash.figures.cell.y = draws.cell_shift[1];
        for (smoke, data) in flash.smokes.iter_mut().zip(&flash.data.smokes) {
            smoke.shot(data, &mut views.rng);
        }
    }
}

/// The near plane a camera's effects take.
pub(crate) fn near_of(projection: &Projection) -> f32 {
    match projection {
        Projection::Perspective(p) => p.near,
        _ => 0.03,
    }
}

/// The most steps a trail ages by in one drawn frame; what a longer frame
/// leaves over is let go, as the game's single long frame would take it.
const MOST_SMOKE_STEPS: u32 = 8;

/// The gun smokes' frame (`LateUpdateMuzzleEffectsValues`, given the main
/// camera), once everything is placed: each trail of the rig in hand worked
/// from its object's place and the main camera's forward, and the trails
/// handed to each camera that draws them. Only the main camera has their
/// screen: the game's trail draws itself for it alone
/// (`MuzzleSmoke.OnRenderObject`), not for the optic camera.
///
/// The game works a trail once a frame, and its drag and spread go by the
/// frame, not the time: its life grows with the frame's time cubed (the
/// AKM's at 60 frames a second 1.3 s, at 120 4.2 s, at 300 a minute). So it
/// ages here in steps at the game's own frame rate setting (the options'
/// `GameFramerate`, the install's 120), whatever the app draws at, its
/// object's place eased across the frame for the steps in it, and is drawn
/// every frame as it last stood (the user's choice, 2026-10-23: "Your game's
/// frame rate"; firing spec, "The gun smoke").
#[allow(clippy::too_many_arguments)]
pub fn smoke_frame(
    rig: Option<Res<crate::weapon::WeaponRig>>,
    time: Res<Time>,
    options: Res<crate::options_view::OptionsState>,
    mut views: ResMut<MuzzleViews>,
    places: Query<&GlobalTransform>,
    main: Query<&GlobalTransform, With<crate::MainCamera>>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut SmokeScreen)>,
) {
    let Ok(main) = main.single() else { return };
    let parts = rig.as_ref().and_then(|rig| rig.muzzle.clone());
    let mut trails: Vec<Trail> = Vec::new();
    if let Some((flash, parts)) = parts.as_ref().and_then(|parts| Some((views.flashes.get_mut(&parts.parent)?, parts))) {
        let game = |v: Vec3| [v.x, v.y, -v.z];
        let forward = game(main.forward().as_vec3());
        let [low, high] = crate::options_view::GAME_FPS;
        let step = 1.0 / options.graphics.game_fps.clamp(low, high) as f32;
        flash.smoke_clock += time.delta_secs();
        let steps = ((flash.smoke_clock / step) as u32).min(MOST_SMOKE_STEPS);
        flash.smoke_clock = if steps == MOST_SMOKE_STEPS { 0.0 } else { flash.smoke_clock - steps as f32 * step };
        for (i, (smoke, data)) in flash.smokes.iter_mut().zip(&flash.data.smokes).enumerate() {
            let Some(place) = parts.smokes.get(i).and_then(|entity| places.get(*entity).ok()) else { continue };
            let now = game(place.translation());
            let last = flash.smoke_last.get(i).copied().flatten().unwrap_or(now);
            let was_on = smoke.on();
            for k in 1..=steps {
                let share = k as f32 / steps as f32;
                let at: [f32; 3] = std::array::from_fn(|c| last[c] + (now[c] - last[c]) * share);
                smoke.frame(data, step, at, forward);
            }
            if was_on && !smoke.on() {
                debug!("a gun smoke's trail ended at {:.2} s, aged in steps of 1/{:.0} s", time.elapsed_secs(), 1.0 / step);
            }
            if let Some(slot) = flash.smoke_last.get_mut(i) {
                *slot = Some(now);
            }
            let corners = smoke.corners();
            let Some(picture) = flash.smoke_pictures.get(i).cloned().flatten() else { continue };
            if corners.is_empty() {
                continue;
            }
            let numbers: Vec<f32> = corners
                .iter()
                .flat_map(|c| {
                    let p = crate::convert::position(c.position);
                    let d = crate::convert::position(c.direction);
                    [p.x, p.y, p.z, c.side, c.number, c.diffusion, d.x, d.y, d.z, c.alpha]
                })
                .collect();
            let m = &data.material;
            trails.push(Trail { corners: Arc::new(numbers), picture, a: Vec4::new(m.width, m.uv_y_scale, m.diffusion_strength, m.start_fade), b: Vec4::new(m.end_fade, m.end, m.inv_fade, m.alpha) });
        }
    }
    for (camera, eye, projection, mut screen) in &mut cameras {
        if trails.is_empty() || !camera.is_active {
            if !screen.0.is_empty() {
                screen.0.clear();
            }
            continue;
        }
        let (view_from_world, clip_from_view, near) = (eye.to_matrix().inverse(), camera.clip_from_view(), near_of(projection));
        screen.0 = trails
            .iter()
            .map(|t| SmokeDraw { uniform: SmokeUniform { view_from_world, clip_from_view, a: t.a, b: t.b, near: Vec4::new(near, 0.0, 0.0, 0.0) }, picture: t.picture.clone(), corners: t.corners.clone() })
            .collect();
    }
}

/// A trail worked out this frame, whichever camera draws it: its corners,
/// its picture, its material's figures (`SmokeUniform`'s `a` and `b`).
struct Trail {
    corners: Arc<Vec<f32>>,
    picture: Handle<Image>,
    a: Vec4,
    b: Vec4,
}

/// Each camera's jets this frame, once everything is placed: the shot's
/// figures now, the mesh's and the camera's places; none while nothing
/// shows (`_ShotVals.x` 1: every pixel cut) or the camera is off. The jets'
/// mesh is an object of the world's layer (`MuzzleJet`'s, made on layer
/// 0), so the optic camera draws them too.
pub fn jet_screen(
    rig: Option<Res<crate::weapon::WeaponRig>>,
    time: Res<Time>,
    views: Res<MuzzleViews>,
    places: Query<&GlobalTransform>,
    mut cameras: Query<(&Camera, &GlobalTransform, &mut JetScreen)>,
) {
    let shown = rig.as_ref().and_then(|rig| rig.muzzle.as_ref()).filter(|parts| !parts.vertices.is_empty() && !parts.indices.is_empty()).and_then(|parts| {
        let flash = views.flashes.get(&parts.parent)?;
        let shot = flash.flash.shot_vals(&flash.data, time.elapsed_secs());
        (shot[0] < 1.0).then_some((parts.parent, flash, shot))
    });
    for (camera, place, mut screen) in &mut cameras {
        let Some((parent, flash, shot)) = shown.filter(|_| camera.is_active) else {
            if screen.rig.is_some() {
                screen.rig = None;
            }
            continue;
        };
        let Ok(mesh_place) = places.get(parent) else { continue };
        let clip_from_view = camera.clip_from_view();
        *screen = JetScreen {
            uniform: JetUniform {
                clip_from_world: clip_from_view * place.to_matrix().inverse(),
                world_from_local: mesh_place.to_matrix(),
                projection: Vec4::new(clip_from_view.x_axis.x, clip_from_view.y_axis.y, 0.0, 0.0),
                shot: Vec4::new(shot[0], shot[1], 0.0, 0.0),
                ..flash.figures
            },
            rig: Some(parent),
        };
    }
}

pub struct MuzzleViewPlugin {
    /// The flash read and drawn (`MUZZLE FLASH`); off, only its pass is in
    /// the graph, and never runs.
    pub on: bool,
}

impl Plugin for MuzzleViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, JET_SHADER, "muzzle_view.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, DEPTH_SHADER, "scene_depth.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, SMOKE_SHADER, "muzzle_smoke.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, FUME_SHADER, "smoke_distorted.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, SPARKS_SHADER, "simple_sparks.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, LASER_SHADER, "laser.wgsl", Shader::from_wgsl);
        if self.on {
            app.init_resource::<MuzzleViews>().init_resource::<JetMeshes>().add_plugins((
                ExtractComponentPlugin::<JetScreen>::default(),
                UniformComponentPlugin::<JetUniform>::default(),
                ExtractResourcePlugin::<JetMeshes>::default(),
                ExtractComponentPlugin::<SmokeScreen>::default(),
            ));
            // After the cameras' projections are made from this frame's
            // fields of view (the optic camera's moves as a scope zooms).
            app.add_systems(PostUpdate, (jet_screen, smoke_frame).after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems).after(crate::optic_view::squeeze_drawn));
        }
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<JetPipeline>>()
            .init_resource::<JetBuffers>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<DepthCopyNode>>(Core3d, DepthCopyLabel)
            .add_render_graph_node::<ViewNodeRunner<JetNode>>(Core3d, JetLabel);
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct JetLabel;

#[derive(Resource)]
struct JetPipeline {
    /// The figures, the picture and its sampler, the scene's depth copied.
    layout: BindGroupLayoutDescriptor,
    /// The same for a trail (its figures not dynamic).
    smoke_layout: BindGroupLayoutDescriptor,
    /// The fumes' figures, their picture and noise with their samplers,
    /// the scene's depth copied (`particles_view`).
    fume_layout: BindGroupLayoutDescriptor,
    /// The sparks' figures, their picture and sampler, the scene's depth.
    sparks_layout: BindGroupLayoutDescriptor,
    /// A laser's figures, its beam's picture and noise, its dot's picture
    /// and noise, with their samplers, the scene's depth (`tactical_view`).
    laser_layout: BindGroupLayoutDescriptor,
    /// The scene's depth, as it is sampled or as it is not.
    depth_layouts: [BindGroupLayoutDescriptor; 2],
    fullscreen: FullscreenShader,
}

/// The jets' pass for a picture's format, the smoke's, or the depth's copy
/// (for a multisampled depth or not).
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum JetKey {
    Jets(TextureFormat),
    Smoke(TextureFormat),
    Fume(TextureFormat),
    Sparks(TextureFormat),
    /// A laser's beam, and its dot.
    LaserBeam(TextureFormat),
    LaserDot(TextureFormat),
    Depth(bool),
}

/// The corners' layout: place, axis, cell, corner, tangent, colour.
const STRIDE: u64 = 18 * 4;

impl SpecializedRenderPipeline for JetPipeline {
    type Key = JetKey;

    fn specialize(&self, key: JetKey) -> RenderPipelineDescriptor {
        match key {
            JetKey::Jets(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("muzzle jets".into()),
                    layout: vec![self.layout.clone()],
                    vertex: VertexState {
                        shader: JET_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: STRIDE,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![
                                at(0, 0, VertexFormat::Float32x3),
                                at(1, 12, VertexFormat::Float32x3),
                                at(2, 24, VertexFormat::Float32x2),
                                at(3, 32, VertexFormat::Float32x2),
                                at(4, 40, VertexFormat::Float32x4),
                                at(5, 56, VertexFormat::Float32x4),
                            ],
                        }],
                        ..default()
                    },
                    fragment: Some(FragmentState {
                        shader: JET_SHADER,
                        entry_point: Some("fragment".into()),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::ALL })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Smoke(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                let over = BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("gun smoke".into()),
                    layout: vec![self.smoke_layout.clone()],
                    vertex: VertexState {
                        shader: SMOKE_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: 10 * 4,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![at(0, 0, VertexFormat::Float32x3), at(1, 12, VertexFormat::Float32x3), at(2, 24, VertexFormat::Float32x3), at(3, 36, VertexFormat::Float32)],
                        }],
                        ..default()
                    },
                    fragment: Some(FragmentState {
                        shader: SMOKE_SHADER,
                        entry_point: Some("fragment".into()),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: over, alpha: over }), write_mask: ColorWrites::ALL })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Fume(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                let over = BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("muzzle fumes".into()),
                    layout: vec![self.fume_layout.clone()],
                    vertex: VertexState {
                        shader: FUME_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: 11 * 4,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![
                                at(0, 0, VertexFormat::Float32x3),
                                at(1, 12, VertexFormat::Float32x2),
                                at(2, 20, VertexFormat::Float32),
                                at(3, 24, VertexFormat::Float32),
                                at(4, 28, VertexFormat::Float32x4),
                            ],
                        }],
                        ..default()
                    },
                    fragment: Some(FragmentState {
                        shader: FUME_SHADER,
                        entry_point: Some("fragment".into()),
                        // Its colour written, not its alpha (`ColorMask RGB`).
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: over, alpha: over }), write_mask: ColorWrites::COLOR })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Sparks(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("muzzle sparks".into()),
                    layout: vec![self.sparks_layout.clone()],
                    vertex: VertexState {
                        shader: SPARKS_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: 20 * 4,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![
                                at(0, 0, VertexFormat::Float32x3),
                                at(1, 12, VertexFormat::Float32x2),
                                at(2, 20, VertexFormat::Float32x2),
                                at(3, 28, VertexFormat::Float32x2),
                                at(4, 36, VertexFormat::Float32x4),
                                at(5, 52, VertexFormat::Float32x3),
                                at(6, 64, VertexFormat::Float32x4),
                            ],
                        }],
                        ..default()
                    },
                    // Both faces (`Cull Off`).
                    primitive: PrimitiveState { cull_mode: None, ..default() },
                    fragment: Some(FragmentState {
                        shader: SPARKS_SHADER,
                        entry_point: Some("fragment".into()),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::COLOR })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::LaserBeam(format) | JetKey::LaserDot(format) => {
                let beam = matches!(key, JetKey::LaserBeam(_));
                let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some(if beam { "laser beam".into() } else { "laser dot".into() }),
                    layout: vec![self.laser_layout.clone()],
                    vertex: VertexState {
                        shader: LASER_SHADER,
                        entry_point: Some(if beam { "beam_vertex".into() } else { "dot_vertex".into() }),
                        buffers: vec![VertexBufferLayout {
                            array_stride: 2 * 4,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![VertexAttribute { format: VertexFormat::Float32x2, offset: 0, shader_location: 0 }],
                        }],
                        ..default()
                    },
                    // The beam both faces (`Cull Off`); the dot its front only
                    // (`Cull Back`): it faces away from the surface it lies on.
                    primitive: PrimitiveState { cull_mode: None, ..default() },
                    fragment: Some(FragmentState {
                        shader: LASER_SHADER,
                        entry_point: Some(if beam { "beam_fragment".into() } else { "dot_fragment".into() }),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::COLOR })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Depth(multisampled) => RenderPipelineDescriptor {
                label: Some("scene depth copy".into()),
                layout: vec![self.depth_layouts[usize::from(multisampled)].clone()],
                vertex: self.fullscreen.to_vertex_state(),
                fragment: Some(FragmentState {
                    shader: DEPTH_SHADER,
                    shader_defs: if multisampled { vec![ShaderDefVal::Bool("MULTISAMPLED".into(), true)] } else { Vec::new() },
                    entry_point: Some("copy_depth".into()),
                    targets: vec![Some(ColorTargetState { format: TextureFormat::R32Float, blend: None, write_mask: ColorWrites::ALL })],
                }),
                ..default()
            },
        }
    }
}

fn init_pipeline(mut commands: Commands, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "muzzle_jets",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (uniform_buffer::<JetUniform>(true), texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false })),
        ),
    );
    let smoke_layout = BindGroupLayoutDescriptor::new(
        "gun_smoke",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (uniform_buffer::<SmokeUniform>(false), texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false })),
        ),
    );
    let fume_layout = BindGroupLayoutDescriptor::new(
        "muzzle_fumes",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<crate::particles_view::FumeUniform>(false),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let sparks_layout = BindGroupLayoutDescriptor::new(
        "muzzle_sparks",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (uniform_buffer::<crate::particles_view::SparkUniform>(false), texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false })),
        ),
    );
    let laser_layout = BindGroupLayoutDescriptor::new(
        "lasers",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<crate::tactical_view::LaserUniform>(false),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_3d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let depth_layouts = [
        BindGroupLayoutDescriptor::new("scene_depth", &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, texture_depth_2d())),
        BindGroupLayoutDescriptor::new("scene_depth_multisampled", &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, texture_depth_2d_multisampled())),
    ];
    commands.insert_resource(JetPipeline { layout, smoke_layout, fume_layout, sparks_layout, laser_layout, depth_layouts, fullscreen: fullscreen.clone() });
}

/// Each rig's jets on the graphics card: their corners, their indices and
/// how many.
#[derive(Resource, Default)]
struct JetBuffers(HashMap<Entity, (Buffer, Buffer, u32)>);

/// What a view with a flash needs this frame: the depth's copy, and the
/// pipelines.
#[derive(Component)]
pub(crate) struct JetParts {
    /// The scene's depth, its first sample, copied this frame (the
    /// distortion's pass reads it too: `distort_view`).
    pub(crate) depth: CachedTexture,
    jets: CachedRenderPipelineId,
    smoke: CachedRenderPipelineId,
    fume: CachedRenderPipelineId,
    sparks: CachedRenderPipelineId,
    laser_beam: CachedRenderPipelineId,
    laser_dot: CachedRenderPipelineId,
    copy: CachedRenderPipelineId,
    multisampled: bool,
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<
        (Entity, &ViewTarget, &Msaa),
        Or<(
            With<JetUniform>,
            With<SmokeScreen>,
            With<crate::particles_view::FumeScreen>,
            With<crate::particles_view::SparkScreen>,
            With<crate::particles_view::HazeScreen>,
            With<crate::explosion_view::ExplosionScreen>,
            With<crate::scattering_view::ScatteringScreen>,
            With<crate::hbao_view::HbaoScreen>,
            With<crate::tactical_view::LaserScreen>,
        )>,
    >,
    done: Query<
        Entity,
        (
            With<JetParts>,
            Without<JetUniform>,
            Without<SmokeScreen>,
            Without<crate::particles_view::FumeScreen>,
            Without<crate::particles_view::SparkScreen>,
            Without<crate::particles_view::HazeScreen>,
            Without<crate::explosion_view::ExplosionScreen>,
            Without<crate::scattering_view::ScatteringScreen>,
            Without<crate::hbao_view::HbaoScreen>,
            Without<crate::tactical_view::LaserScreen>,
        ),
    >,
    meshes: Option<Res<JetMeshes>>,
    mut buffers: ResMut<JetBuffers>,
    device: Res<RenderDevice>,
    mut textures: ResMut<TextureCache>,
    cache: Res<PipelineCache>,
    pipeline: Res<JetPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<JetPipeline>>,
) {
    for entity in &done {
        commands.entity(entity).remove::<(JetParts, DynamicUniformIndex<JetUniform>)>();
    }
    if let Some(meshes) = meshes {
        for (rig, mesh) in &meshes.0 {
            // An empty buffer cannot be drawn from (wgpu's slices may not be
            // empty): none is made.
            if buffers.0.contains_key(rig) || mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }
            let words: Vec<f32> = mesh
                .vertices
                .iter()
                .flat_map(|v| {
                    let p = convert::position(v.position);
                    let n = convert::position(v.normal);
                    [p.x, p.y, p.z, n.x, n.y, n.z, v.uv0[0], v.uv0[1], v.uv1[0], v.uv1[1], v.tangent[0], v.tangent[1], v.tangent[2], v.tangent[3], v.color[0], v.color[1], v.color[2], v.color[3]]
                })
                .collect();
            let bytes = |numbers: &mut dyn Iterator<Item = [u8; 4]>| numbers.flatten().collect::<Vec<u8>>();
            let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_jets"), contents: &bytes(&mut words.iter().map(|w| w.to_le_bytes())), usage: BufferUsages::VERTEX });
            let indices = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_jets_indices"), contents: &bytes(&mut mesh.indices.iter().map(|i| i.to_le_bytes())), usage: BufferUsages::INDEX });
            buffers.0.insert(*rig, (corners, indices, mesh.indices.len() as u32));
        }
    }
    for (entity, target, msaa) in &views {
        let size = target.main_texture().size();
        let depth = textures.get(
            &device,
            TextureDescriptor {
                label: Some("scene_depth_copy"),
                size: Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R32Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let multisampled = msaa.samples() > 1;
        commands.entity(entity).insert(JetParts {
            depth,
            jets: pipelines.specialize(&cache, &pipeline, JetKey::Jets(target.main_texture_format())),
            smoke: pipelines.specialize(&cache, &pipeline, JetKey::Smoke(target.main_texture_format())),
            fume: pipelines.specialize(&cache, &pipeline, JetKey::Fume(target.main_texture_format())),
            sparks: pipelines.specialize(&cache, &pipeline, JetKey::Sparks(target.main_texture_format())),
            laser_beam: pipelines.specialize(&cache, &pipeline, JetKey::LaserBeam(target.main_texture_format())),
            laser_dot: pipelines.specialize(&cache, &pipeline, JetKey::LaserDot(target.main_texture_format())),
            copy: pipelines.specialize(&cache, &pipeline, JetKey::Depth(multisampled)),
            multisampled,
        });
    }
}

#[derive(Default)]
struct JetNode;

/// The scene's depth, its first sample, copied for the effects that read
/// it (the muzzle's, the explosions', the distortion's, the sky's scattering:
/// `scattering_view`; the ambient occlusion: `hbao_view`), before any of them
/// draws.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct DepthCopyLabel;

#[derive(Default)]
struct DepthCopyNode;

impl ViewNode for DepthCopyNode {
    type ViewQuery = (&'static ViewDepthTexture, &'static JetParts);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (depth, parts): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<JetPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(copy) = cache.get_render_pipeline(parts.copy) else { return Ok(()) };
        let device = context.render_device().clone();
        let group = device.create_bind_group("scene_depth", &cache.get_bind_group_layout(&pipeline.depth_layouts[usize::from(parts.multisampled)]), &BindGroupEntries::single(depth.view()));
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("scene_depth_copy"),
            color_attachments: &[Some(RenderPassColorAttachment { view: &parts.depth.default_view, depth_slice: None, resolve_target: None, ops: Operations::default() })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(copy);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

impl ViewNode for JetNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static JetParts,
        Option<(&'static JetRig, &'static DynamicUniformIndex<JetUniform>)>,
        Option<&'static SmokeScreen>,
        Option<&'static crate::particles_view::FumeScreen>,
        Option<&'static crate::particles_view::SparkScreen>,
        Option<&'static crate::tactical_view::LaserScreen>,
    );

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, jets_shown, smoke, fume, sparks, lasers): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<JetPipeline>();
        let cache = world.resource::<PipelineCache>();
        let (Some(jets), Some(smoke_pipeline)) = (cache.get_render_pipeline(parts.jets), cache.get_render_pipeline(parts.smoke)) else { return Ok(()) };
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let device = context.render_device().clone();
        // The fumes, behind the jets (`PROVISIONAL`: the engine sorts its
        // transparent renderers far to near by their bounds' middles).
        if let (Some(draw), Some(fume_pipeline)) = (fume.and_then(|f| f.0.as_ref()), cache.get_render_pipeline(parts.fume)) {
            draw_fumes(context, world, &device, target, parts, fume_pipeline, draw);
        }
        if let Some((rig, index)) = jets_shown {
            draw_jets(context, world, &device, target, parts, jets, rig, index);
        }
        // The sparks after both (their material's queue, 3003, after the
        // transparent queue's 3000), before the trails.
        if let (Some(draw), Some(sparks_pipeline)) = (sparks.and_then(|s| s.0.as_ref()), cache.get_render_pipeline(parts.sparks)) {
            draw_sparks(context, world, &device, target, parts, sparks_pipeline, draw);
        }
        // The lasers' dots (their material's queue, 3002 or 5000 for the
        // red, after the transparent's), then their beams (4011).
        if let (Some(lasers), Some(beam), Some(dot)) = (lasers, cache.get_render_pipeline(parts.laser_beam), cache.get_render_pipeline(parts.laser_dot)) {
            for draw in &lasers.0 {
                draw_laser(context, world, &device, target, parts, dot, draw, false);
            }
            for draw in &lasers.0 {
                draw_laser(context, world, &device, target, parts, beam, draw, true);
            }
        }
        // The trails, drawn after the scene's own (`OnRenderObject`).
        for trail in smoke.map(|s| s.0.as_slice()).unwrap_or(&[]) {
            let quads = trail.corners.len() as u32 / 40;
            let Some(picture) = gpu.get(&trail.picture).filter(|_| quads > 0) else { continue };
            let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
            if figures.write(&trail.uniform).is_err() {
                continue;
            }
            let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("gun_smoke"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
            let bytes: Vec<u8> = trail.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
            let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("gun_smoke_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
            let indices: Vec<u8> = (0..quads).flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 3, 4 * q]).flat_map(u32::to_le_bytes).collect();
            let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("gun_smoke_indices"), contents: &indices, usage: BufferUsages::INDEX });
            let group = device.create_bind_group(
                "gun_smoke",
                &cache.get_bind_group_layout(&pipeline.smoke_layout),
                &BindGroupEntries::sequential((uniform.as_entire_binding(), &picture.texture_view, &picture.sampler, &parts.depth.default_view)),
            );
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("gun_smoke"),
                color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_render_pipeline(smoke_pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.set_vertex_buffer(0, corners.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
            pass.draw_indexed(0..quads * 6, 0, 0..1);
        }
        Ok(())
    }
}

/// A laser's beam (a strip of two quads' corners: each side at the emitter
/// and at the end) or its dot (a square), added to the frame as it stands.
#[allow(clippy::too_many_arguments)]
fn draw_laser(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, pipeline_drawn: &RenderPipeline, draw: &crate::tactical_view::LaserDraw, beam: bool) {
    if !beam && draw.uniform.dot.x <= 0.0 {
        return;
    }
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let gpu = world.resource::<RenderAssets<GpuImage>>();
    let (Some(main), Some(noise), Some(dot), Some(dot_noise)) = (gpu.get(&draw.main), gpu.get(&draw.noise), gpu.get(&draw.dot_main), gpu.get(&draw.dot_noise)) else { return };
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    if figures.write(&draw.uniform).is_err() {
        return;
    }
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("laser"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let corners: [f32; 8] = if beam { [-1.0, 0.0, 1.0, 0.0, 1.0, 1.0, -1.0, 1.0] } else { [-1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0] };
    let bytes: Vec<u8> = corners.iter().flat_map(|n| n.to_le_bytes()).collect();
    let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("laser_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
    let indices: Vec<u8> = [0u32, 1, 2, 2, 3, 0].iter().flat_map(|i| i.to_le_bytes()).collect();
    let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("laser_indices"), contents: &indices, usage: BufferUsages::INDEX });
    let group = device.create_bind_group(
        "laser",
        &cache.get_bind_group_layout(&pipeline.laser_layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), &main.texture_view, &main.sampler, &noise.texture_view, &noise.sampler, &dot.texture_view, &dot_noise.texture_view, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("laser"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(pipeline_drawn);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..6, 0, 0..1);
}

/// The sparks' streaks, added to the frame as it stands.
fn draw_sparks(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, sparks: &RenderPipeline, draw: &crate::particles_view::SparkDraw) {
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let gpu = world.resource::<RenderAssets<GpuImage>>();
    let Some(picture) = gpu.get(&draw.picture) else { return };
    // None drawn without a corner and a triangle.
    if draw.corners.is_empty() || draw.indices.is_empty() {
        return;
    }
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    if figures.write(&draw.uniform).is_err() {
        return;
    }
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_sparks"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let bytes: Vec<u8> = draw.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
    let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_sparks_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
    let indices: Vec<u8> = draw.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_sparks_indices"), contents: &indices, usage: BufferUsages::INDEX });
    let group = device.create_bind_group(
        "muzzle_sparks",
        &cache.get_bind_group_layout(&pipeline.sparks_layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), &picture.texture_view, &picture.sampler, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("muzzle_sparks"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(sparks);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..draw.indices.len() as u32, 0, 0..1);
}

/// The fumes' particles, blended over the frame as it stands.
fn draw_fumes(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, fumes: &RenderPipeline, draw: &crate::particles_view::FumeDraw) {
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let gpu = world.resource::<RenderAssets<GpuImage>>();
    // Four corners and six indices a particle; none drawn without one.
    let quads = draw.corners.len() as u32 / 44;
    let (Some(picture), Some(noise)) = (gpu.get(&draw.picture), gpu.get(&draw.noise)) else { return };
    if quads == 0 {
        return;
    }
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    if figures.write(&draw.uniform).is_err() {
        return;
    }
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_fumes"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let bytes: Vec<u8> = draw.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
    let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_fumes_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
    let indices: Vec<u8> = (0..quads).flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 3, 4 * q]).flat_map(u32::to_le_bytes).collect();
    let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_fumes_indices"), contents: &indices, usage: BufferUsages::INDEX });
    let group = device.create_bind_group(
        "muzzle_fumes",
        &cache.get_bind_group_layout(&pipeline.fume_layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), &picture.texture_view, &picture.sampler, &noise.texture_view, &noise.sampler, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("muzzle_fumes"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(fumes);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..quads * 6, 0, 0..1);
}

/// The jets, added onto the frame as it stands.
#[allow(clippy::too_many_arguments)]
fn draw_jets(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, jets: &RenderPipeline, rig: &JetRig, index: &DynamicUniformIndex<JetUniform>) {
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let Some(uniform) = world.resource::<ComponentUniforms<JetUniform>>().uniforms().binding() else { return };
    let Some((corners, indices, count)) = world.resource::<JetBuffers>().0.get(&rig.0).filter(|(_, _, count)| *count > 0) else { return };
    let Some(mesh) = world.get_resource::<JetMeshes>().and_then(|meshes| meshes.0.get(&rig.0)) else { return };
    let Some(picture) = world.resource::<RenderAssets<GpuImage>>().get(&mesh.picture) else { return };
    let group = device.create_bind_group(
        "muzzle_jets",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((uniform, &picture.texture_view, &picture.sampler, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("muzzle_jets"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(jets);
    pass.set_bind_group(0, &group, &[index.index()]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(indices.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..*count, 0, 0..1);
}
