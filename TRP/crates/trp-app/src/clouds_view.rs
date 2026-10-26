//! Customs' clouds with the options' `CUSTOMS CLOUDS` on, under `CUSTOMS
//! LIGHT` (`docs/spec/lighting.md`, "Customs' clouds"; `clouds.wgsl`): the
//! weather's cloud layer as the game's `CloudController` runs it, at the
//! GRAPHICS page's CLOUDS QUALITY as the run began.
//!
//! - The cloud texture (the upper sky in lat-long: the light the layer
//!   scatters toward the eye, its opacity) is baked once from the level's
//!   cloud map, as the game bakes it again only when the sun turns by
//!   three degrees or more (the range's sun stays where the morning puts
//!   it).
//! - The sun's cookie (`Cloud Shadows`) is baked on every frame from it,
//!   before the scene is drawn, as the game bakes it before its G-buffer;
//!   it lies on the sun as Bevy's light texture, tiled, its texel across
//!   written the other way as Bevy reads a light texture across the other
//!   way (each texel what the game's is), its middle at the sun's light's
//!   place.
//! - The layer is drawn over the sky alone on the frame in the game's
//!   numbers, after the scene's depth is copied and before the ambient
//!   occlusion, as the game draws it after its opaque scene
//!   (`AfterForwardOpaque`) and before its opaque image effects.
//!
//! The wind scrolls the layer by the frame's time (`ScrollFactor`, from
//! nought at the start, as the game's from the level's load). The drawing
//! alone: nothing of the simulation changes. `PROVISIONAL`: the range's
//! places taken as Customs' own (the cookie's middle at the sun's recorded
//! place from the range's origin).

use bevy::asset::{load_internal_asset, uuid_handle, RenderAssetUsages};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{self, NodeRunError, RenderGraph, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_storage_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use trp_data::clouds::LevelClouds;
use trp_sim::clouds::CloudFigures;

const CLOUDS_SHADER: Handle<Shader> = uuid_handle!("b7d41e93-2c6a-4f58-9e0b-5a3c8d1f7e24");

/// The level's clouds as read at the start (`lighting::read`).
#[derive(Clone)]
pub struct SceneClouds {
    pub clouds: LevelClouds,
    pub figures: CloudFigures,
    /// The sun's light's place, in the game's axes.
    pub sun_position: [f32; 3],
}

/// What the passes are given that is the run's (the main world's, handed
/// to the renderer with each frame): the figures, the images, the scroll.
#[derive(Resource, Clone, ExtractResource)]
pub struct CloudsFrame {
    pub figures: CloudFigures,
    pub map: Handle<Image>,
    pub texture: Handle<Image>,
    pub cookie: Option<Handle<Image>>,
    /// `ScrollFactor`, in metres.
    pub scroll: f32,
}

/// On the camera: the draw's figures (`Draw` in `clouds.wgsl`).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct CloudsScreen {
    pub flow: Vec4,
    pub sun_color: Vec4,
    pub ambient: Vec4,
    pub planet: Vec4,
    pub world_from_clip: Mat4,
    pub camera: Vec4,
}

impl ExtractComponent for CloudsScreen {
    type QueryData = &'static CloudsScreen;
    type QueryFilter = ();
    type Out = CloudsScreen;

    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*screen)
    }
}

/// The cloud map as an image: its channels as stored (the game's project
/// samples them as they are), its rows as stored (the game's v is the
/// stored row's), its edges as its own sampler takes them (across
/// repeated, up clamped), blended between its pixels, one level.
fn map_image(clouds: &LevelClouds) -> Image {
    let map = &clouds.map;
    let mut image = Image::new(
        Extent3d { width: map.width, height: map.height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        map.data[..(map.width * map.height * 4) as usize].to_vec(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mode = |clamped: bool| if clamped { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode(map.clamp[0]),
        address_mode_v: mode(map.clamp[1]),
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// An image the passes write (half floats, four channels, as the game's
/// `DefaultHDR` cookie; the cloud texture's two in four), repeated at its
/// edges and blended, as the game's are.
fn written_image(width: u32, height: u32) -> Image {
    let mut image = Image::new_fill(Extent3d { width, height, depth_or_array_layers: 1 }, TextureDimension::D2, &[0; 8], TextureFormat::Rgba16Float, RenderAssetUsages::RENDER_WORLD);
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// Sets the clouds up for the camera: the images, the run's figures, the
/// camera's draw. The sun's cookie, where the clouds cast shadows, with
/// the sun light's transform's place and scale for it (Bevy lays a light's
/// texture two of its units across, so the scale is half the cookie's
/// width).
pub fn begin(commands: &mut Commands, images: &mut Assets<Image>, camera: Entity, clouds: &SceneClouds) -> Option<(Handle<Image>, Vec3, f32)> {
    let f = &clouds.figures;
    let map = images.add(map_image(&clouds.clouds));
    let texture = images.add(written_image(f.resolution, f.resolution / 2));
    let cookie = f.shadows.map(|s| images.add(written_image(s.resolution, s.resolution)));
    commands.insert_resource(CloudsFrame { figures: *f, map, texture, cookie: cookie.clone(), scroll: 0.0 });
    let [sx, sz] = f.scroll_direction;
    commands.entity(camera).insert(CloudsScreen {
        flow: Vec4::new(sx, sz, 0.0, f.opacity),
        sun_color: Vec3::from_array(f.sun_color).extend(f.altitude),
        ambient: Vec3::from_array(f.ambient).extend(1.0),
        planet: Vec4::new(f.planet_radius, 0.0, 0.0, 0.0),
        world_from_clip: Mat4::IDENTITY,
        camera: Vec4::ZERO,
    });
    let place = crate::convert::position(clouds.sun_position);
    Some((cookie?, place, f.shadows?.size / 2.0))
}

/// The wind's scroll by the frame's time (`ScrollFactor += ScrollSpeed ×
/// dt × 0.277778`), and this frame's camera.
fn follow(time: Res<Time>, frame: Option<ResMut<CloudsFrame>>, mut cameras: Query<(&Camera, &GlobalTransform, &mut CloudsScreen)>) {
    let Some(mut frame) = frame else { return };
    frame.scroll += trp_sim::clouds::scroll_step(frame.figures.scroll_speed, time.delta_secs());
    for (camera, eye, mut screen) in &mut cameras {
        screen.flow.z = frame.scroll;
        screen.camera = eye.translation().extend(1.0);
        screen.world_from_clip = eye.to_matrix() * camera.clip_from_view().inverse();
    }
}

pub struct CloudsPlugin;

impl Plugin for CloudsPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, CLOUDS_SHADER, "clouds.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<CloudsScreen>::default(), UniformComponentPlugin::<CloudsScreen>::default(), ExtractResourcePlugin::<CloudsFrame>::default()))
            .add_systems(PostUpdate, follow.after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<DrawPipeline>>()
            .init_resource::<BakeBuffers>()
            .add_systems(RenderStartup, init_pipelines)
            .add_systems(Render, (prepare_buffers.in_set(RenderSystems::PrepareResources), prepare_draw.in_set(RenderSystems::PrepareResources)))
            .add_render_graph_node::<ViewNodeRunner<DrawNode>>(Core3d, CloudsLabel)
            .add_render_graph_edges(Core3d, (crate::muzzle_view::DepthCopyLabel, CloudsLabel, crate::hbao_view::HbaoLabel));
        // The bakes before any camera draws.
        let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();
        graph.add_node(BakeLabel, BakeNode::default());
        graph.add_node_edge(BakeLabel, bevy::render::graph::CameraDriverLabel);
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct CloudsLabel;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct BakeLabel;

/// `Bake` in `clouds.wgsl`.
#[derive(Clone, Copy, Default, ShaderType)]
struct BakeUniform {
    sun: Vec4,
    opacities: Vec4,
    layer: Vec4,
    resolution: Vec4,
}

/// `Shadows` in `clouds.wgsl`.
#[derive(Clone, Copy, Default, ShaderType)]
struct ShadowsUniform {
    flow: Vec4,
    sun: Vec4,
    right: Vec4,
    up: Vec4,
    shadow: Vec4,
}

#[derive(Resource, Default)]
struct BakeBuffers {
    bake: UniformBuffer<BakeUniform>,
    shadows: UniformBuffer<ShadowsUniform>,
}

#[derive(Resource)]
struct Pipelines {
    bake_layout: BindGroupLayoutDescriptor,
    bake: CachedComputePipelineId,
    shadows_layout: BindGroupLayoutDescriptor,
    shadows: CachedComputePipelineId,
    /// The cloud texture's sampler (repeated, blended).
    repeat: Sampler,
}

#[derive(Resource)]
struct DrawPipeline {
    layout: BindGroupLayoutDescriptor,
    fullscreen: FullscreenShader,
}

impl SpecializedRenderPipeline for DrawPipeline {
    type Key = TextureFormat;

    fn specialize(&self, format: TextureFormat) -> RenderPipelineDescriptor {
        // `Blend One OneMinusSrcAlpha`: premultiplied over the frame.
        let over = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
        RenderPipelineDescriptor {
            label: Some("clouds".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: CLOUDS_SHADER,
                shader_defs: vec!["DRAW".into()],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: over, alpha: over }), write_mask: ColorWrites::ALL })],
            }),
            ..default()
        }
    }
}

fn init_pipelines(mut commands: Commands, cache: Res<PipelineCache>, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let float = TextureSampleType::Float { filterable: true };
    let bake_layout = BindGroupLayoutDescriptor::new(
        "clouds_bake",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (texture_2d(float), sampler(SamplerBindingType::Filtering), texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly), uniform_buffer::<BakeUniform>(false)),
        ),
    );
    let shadows_layout = BindGroupLayoutDescriptor::new(
        "clouds_shadows",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (texture_2d(float), sampler(SamplerBindingType::Filtering), texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly), uniform_buffer::<ShadowsUniform>(false)),
        ),
    );
    let compute = |label: &'static str, layout: &BindGroupLayoutDescriptor, def: &'static str, entry: &'static str| ComputePipelineDescriptor {
        label: Some(label.into()),
        layout: vec![layout.clone()],
        shader: CLOUDS_SHADER,
        shader_defs: vec![def.into()],
        entry_point: Some(entry.into()),
        ..default()
    };
    let bake = cache.queue_compute_pipeline(compute("clouds_bake", &bake_layout, "BAKE_TEXTURE", "bake_texture"));
    let shadows = cache.queue_compute_pipeline(compute("clouds_shadows", &shadows_layout, "BAKE_SHADOWS", "bake_shadows"));
    let repeat = device.create_sampler(&SamplerDescriptor {
        label: Some("clouds_repeat"),
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    commands.insert_resource(Pipelines { bake_layout, bake, shadows_layout, shadows, repeat });
    let layout = BindGroupLayoutDescriptor::new(
        "clouds_draw",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(float), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false }), uniform_buffer::<CloudsScreen>(true)),
        ),
    );
    commands.insert_resource(DrawPipeline { layout, fullscreen: fullscreen.clone() });
}

/// The bakes' figures for this frame.
fn prepare_buffers(frame: Option<Res<CloudsFrame>>, mut buffers: ResMut<BakeBuffers>, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
    let Some(frame) = frame else { return };
    let f = &frame.figures;
    let sun = Vec3::from_array(f.sun_direction);
    buffers.bake.set(BakeUniform {
        sun: sun.extend(f.planet_radius),
        opacities: Vec4::from_array(f.opacities),
        layer: Vec4::new(f.rotation, f.steps as f32, f.sigma_t, f.altitude),
        resolution: Vec4::new(1.0 / f.resolution as f32, 0.0, 0.0, 0.0),
    });
    buffers.bake.write_buffer(&device, &queue);
    if let Some(s) = f.shadows {
        let [sx, sz] = f.scroll_direction;
        buffers.shadows.set(ShadowsUniform {
            flow: Vec4::new(sx, sz, frame.scroll, f.opacity),
            sun: sun.extend(f.planet_radius),
            right: Vec3::from_array(s.sun_right).extend(f.altitude),
            up: Vec3::from_array(s.sun_up).extend(0.0),
            shadow: Vec4::new(s.tint_luminance, s.opacity, 0.0, 0.0),
        });
        buffers.shadows.write_buffer(&device, &queue);
    }
}

/// The cloud texture once, then the sun's cookie on every frame.
#[derive(Default)]
struct BakeNode {
    baked: Arc<AtomicBool>,
}

impl render_graph::Node for BakeNode {
    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, world: &World) -> Result<(), NodeRunError> {
        let Some(frame) = world.get_resource::<CloudsFrame>() else { return Ok(()) };
        let pipelines = world.resource::<Pipelines>();
        let cache = world.resource::<PipelineCache>();
        let images = world.resource::<RenderAssets<GpuImage>>();
        let buffers = world.resource::<BakeBuffers>();
        let (Some(map), Some(texture)) = (images.get(&frame.map), images.get(&frame.texture)) else { return Ok(()) };
        let groups = |x: u32, y: u32| (x.div_ceil(8), y.div_ceil(8));
        if !self.baked.load(Ordering::Relaxed) {
            let (Some(pipeline), Some(uniform)) = (cache.get_compute_pipeline(pipelines.bake), buffers.bake.binding()) else { return Ok(()) };
            let sampler = &map.sampler;
            let group = context.render_device().create_bind_group("clouds_bake", &cache.get_bind_group_layout(&pipelines.bake_layout), &BindGroupEntries::sequential((&map.texture_view, sampler, &texture.texture_view, uniform)));
            let mut pass = context.command_encoder().begin_compute_pass(&ComputePassDescriptor { label: Some("clouds_bake"), timestamp_writes: None });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            let (x, y) = groups(texture.size.width, texture.size.height);
            pass.dispatch_workgroups(x, y, 1);
            self.baked.store(true, Ordering::Relaxed);
        }
        let Some(cookie) = frame.cookie.as_ref().and_then(|cookie| images.get(cookie)) else { return Ok(()) };
        let (Some(pipeline), Some(uniform)) = (cache.get_compute_pipeline(pipelines.shadows), buffers.shadows.binding()) else { return Ok(()) };
        let group = context.render_device().create_bind_group("clouds_shadows", &cache.get_bind_group_layout(&pipelines.shadows_layout), &BindGroupEntries::sequential((&texture.texture_view, &pipelines.repeat, &cookie.texture_view, uniform)));
        let mut pass = context.command_encoder().begin_compute_pass(&ComputePassDescriptor { label: Some("clouds_shadows"), timestamp_writes: None });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        let (x, y) = groups(cookie.size.width, cookie.size.height);
        pass.dispatch_workgroups(x, y, 1);
        Ok(())
    }
}

#[derive(Component)]
struct DrawParts(CachedRenderPipelineId);

fn prepare_draw(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<CloudsScreen>>,
    cache: Res<PipelineCache>,
    pipeline: Res<DrawPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<DrawPipeline>>,
) {
    for (entity, target) in &views {
        let id = pipelines.specialize(&cache, &pipeline, target.main_texture_format());
        commands.entity(entity).insert(DrawParts(id));
    }
}

#[derive(Default)]
struct DrawNode;

impl ViewNode for DrawNode {
    type ViewQuery = (&'static ViewTarget, &'static DrawParts, &'static crate::muzzle_view::JetParts, &'static DynamicUniformIndex<CloudsScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, jets, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let Some(frame) = world.get_resource::<CloudsFrame>() else { return Ok(()) };
        let pipeline = world.resource::<DrawPipeline>();
        let pipelines = world.resource::<Pipelines>();
        let cache = world.resource::<PipelineCache>();
        let Some(drawn) = cache.get_render_pipeline(parts.0) else { return Ok(()) };
        let Some(texture) = world.resource::<RenderAssets<GpuImage>>().get(&frame.texture) else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<CloudsScreen>>().uniforms().binding() else { return Ok(()) };
        let group = context.render_device().create_bind_group("clouds_draw", &cache.get_bind_group_layout(&pipeline.layout), &BindGroupEntries::sequential((&texture.texture_view, &pipelines.repeat, &jets.depth.default_view, uniform)));
        let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("clouds"),
            color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render.set_render_pipeline(drawn);
        render.set_bind_group(0, &group, &[index.index()]);
        render.draw(0..3, 0..1);
        Ok(())
    }
}
