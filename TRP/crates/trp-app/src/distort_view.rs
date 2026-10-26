//! The camera's distortion (`DistortCameraFX` on `Cam2`; firing spec, "The
//! muzzle's heat haze"): the muzzles' heat haze drawn not on the frame but
//! into a picture of distortion values (`heat_haze.wgsl`), then, in its
//! place among the camera's effects (after the blood on the screen, before
//! the contusion's), the frame copied at half size, blurred, and moved by
//! those values toward the blurred copy (`distort_camera.wgsl`). On the
//! frame encoded for the camera's effects, as the game's (`gamma_ui`).
//! What it draws comes from `particles_view` (`HazeScreen`); with none, the
//! pass does nothing.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{CachedTexture, GpuImage, TextureCache};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;

use crate::particles_view::{HazeScreen, HazeUniform};

const HAZE_SHADER: Handle<Shader> = uuid_handle!("3b8f1c62-9e47-4d05-a6c1-8f2e7d40b915");
const DISTORT_SHADER: Handle<Shader> = uuid_handle!("e05a7d93-1f28-4c6b-9d34-6a1b8e5f2c07");

/// The distortion values' picture (`_distortTexture`, `ARGB32`).
const VALUES_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// What the camera's passes are given: `_Distortion` (its pixels over the
/// frame's width and height) and the blur's step.
#[derive(Clone, Copy, Default, ShaderType)]
struct DistortUniform {
    distortion: Vec4,
    step: Vec4,
}

pub struct DistortViewPlugin;

impl Plugin for DistortViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, HAZE_SHADER, "heat_haze.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, DISTORT_SHADER, "distort_camera.wgsl", Shader::from_wgsl);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<DistortPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<DistortNode>>(Core3d, DistortLabel)
            .add_render_graph_edges(Core3d, (crate::blood_view::BloodLabel, DistortLabel, crate::contusion_view::ContusionLabel));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct DistortLabel;

#[derive(Resource)]
struct DistortPipeline {
    /// The haze's figures, its noise and alpha pictures with their
    /// samplers, the scene's depth.
    haze_layout: BindGroupLayoutDescriptor,
    /// The passes' figures, the picture read, a sampler, the values, the
    /// blurred copy.
    camera_layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    fullscreen: FullscreenShader,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum Key {
    Haze,
    Half(TextureFormat),
    BlurAcross(TextureFormat),
    BlurDown(TextureFormat),
    Composite(TextureFormat),
}

impl SpecializedRenderPipeline for DistortPipeline {
    type Key = Key;

    fn specialize(&self, key: Key) -> RenderPipelineDescriptor {
        if key == Key::Haze {
            let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
            let over = BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
            return RenderPipelineDescriptor {
                label: Some("heat haze values".into()),
                layout: vec![self.haze_layout.clone()],
                vertex: VertexState {
                    shader: HAZE_SHADER,
                    entry_point: Some("vertex".into()),
                    buffers: vec![VertexBufferLayout {
                        array_stride: 11 * 4,
                        step_mode: VertexStepMode::Vertex,
                        attributes: vec![at(0, 0, VertexFormat::Float32x3), at(1, 12, VertexFormat::Float32x2), at(2, 20, VertexFormat::Float32), at(3, 24, VertexFormat::Float32), at(4, 28, VertexFormat::Float32x4)],
                    }],
                    ..default()
                },
                fragment: Some(FragmentState {
                    shader: HAZE_SHADER,
                    entry_point: Some("fragment".into()),
                    // Its colour written, not its alpha (`ColorMask RGB`).
                    targets: vec![Some(ColorTargetState { format: VALUES_FORMAT, blend: Some(BlendState { color: over, alpha: over }), write_mask: ColorWrites::COLOR })],
                    ..default()
                }),
                ..default()
            };
        }
        let (entry, format) = match key {
            Key::Half(f) => ("half", f),
            Key::BlurAcross(f) => ("blur_across", f),
            Key::BlurDown(f) => ("blur_down", f),
            Key::Composite(f) => ("composite", f),
            Key::Haze => unreachable!(),
        };
        RenderPipelineDescriptor {
            label: Some(format!("distortion {entry}").into()),
            layout: vec![self.camera_layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState { shader: DISTORT_SHADER, entry_point: Some(entry.into()), targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })], ..default() }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let haze_layout = BindGroupLayoutDescriptor::new(
        "heat_haze",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (uniform_buffer::<HazeUniform>(false), picture(), sampler(SamplerBindingType::Filtering), picture(), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false })),
        ),
    );
    let camera_layout = BindGroupLayoutDescriptor::new(
        "distort_camera",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (uniform_buffer::<DistortUniform>(false), picture(), sampler(SamplerBindingType::Filtering), picture(), picture())),
    );
    // Bilinear, clamped at the edges.
    let sampler = device.create_sampler(&SamplerDescriptor { label: Some("distort_camera"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(DistortPipeline { haze_layout, camera_layout, sampler, fullscreen: fullscreen.clone() });
}

/// What a view with heat haze needs this frame: the values' picture, the
/// half-size copy and its scratch, and the pipelines.
#[derive(Component)]
struct DistortParts {
    values: CachedTexture,
    half: CachedTexture,
    scratch: CachedTexture,
    haze: CachedRenderPipelineId,
    half_pass: CachedRenderPipelineId,
    across: CachedRenderPipelineId,
    down: CachedRenderPipelineId,
    composite: CachedRenderPipelineId,
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<HazeScreen>>,
    done: Query<Entity, (With<DistortParts>, Without<HazeScreen>)>,
    device: Res<RenderDevice>,
    mut textures: ResMut<TextureCache>,
    cache: Res<PipelineCache>,
    pipeline: Res<DistortPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<DistortPipeline>>,
) {
    for entity in &done {
        commands.entity(entity).remove::<DistortParts>();
    }
    for (entity, target) in &views {
        let size = target.main_texture().size();
        let format = target.main_texture_format();
        let make = |width: u32, height: u32, format: TextureFormat, label: &'static str| TextureDescriptor {
            label: Some(label),
            size: Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let (hw, hh) = (size.width >> 1, size.height >> 1);
        commands.entity(entity).insert(DistortParts {
            values: textures.get(&device, make(size.width, size.height, VALUES_FORMAT, "distort_values")),
            half: textures.get(&device, make(hw, hh, format, "distort_blur")),
            scratch: textures.get(&device, make(hw, hh, format, "distort_blur_scratch")),
            haze: pipelines.specialize(&cache, &pipeline, Key::Haze),
            half_pass: pipelines.specialize(&cache, &pipeline, Key::Half(format)),
            across: pipelines.specialize(&cache, &pipeline, Key::BlurAcross(format)),
            down: pipelines.specialize(&cache, &pipeline, Key::BlurDown(format)),
            composite: pipelines.specialize(&cache, &pipeline, Key::Composite(format)),
        });
    }
}

#[derive(Default)]
struct DistortNode;

impl ViewNode for DistortNode {
    type ViewQuery = (&'static ViewTarget, &'static DistortParts, &'static HazeScreen, Option<&'static crate::muzzle_view::JetParts>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, screen, muzzle): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let Some(draw) = screen.0.as_ref() else { return Ok(()) };
        // The scene's depth as the muzzle's pass copied it this frame.
        let Some(muzzle) = muzzle else { return Ok(()) };
        let pipeline = world.resource::<DistortPipeline>();
        let cache = world.resource::<PipelineCache>();
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let ids = [parts.haze, parts.half_pass, parts.across, parts.down, parts.composite];
        let Some([haze, half, across, down, composite]) = ids.map(|id| cache.get_render_pipeline(id)).into_iter().collect::<Option<Vec<_>>>().and_then(|all| <[&RenderPipeline; 5]>::try_from(all).ok()) else { return Ok(()) };
        let (Some(noise), Some(alpha)) = (gpu.get(&draw.noise), gpu.get(&draw.alpha)) else { return Ok(()) };
        let quads = draw.corners.len() as u32 / 44;
        if quads == 0 {
            return Ok(());
        }
        let device = context.render_device().clone();
        // The values: cleared to no move and no blur, then each puff.
        {
            let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
            if figures.write(&draw.uniform).is_err() {
                return Ok(());
            }
            let buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("heat_haze"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
            let bytes: Vec<u8> = draw.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
            let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("heat_haze_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
            let indices: Vec<u8> = (0..quads).flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 3, 4 * q]).flat_map(u32::to_le_bytes).collect();
            let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("heat_haze_indices"), contents: &indices, usage: BufferUsages::INDEX });
            let group = device.create_bind_group(
                "heat_haze",
                &cache.get_bind_group_layout(&pipeline.haze_layout),
                &BindGroupEntries::sequential((buffer.as_entire_binding(), &noise.texture_view, &noise.sampler, &alpha.texture_view, &alpha.sampler, &muzzle.depth.default_view)),
            );
            let clear = Operations { load: LoadOp::Clear(LinearRgba::new(0.5, 0.5, 0.0, 0.0).into()), store: StoreOp::Store };
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("heat_haze_values"),
                color_attachments: &[Some(RenderPassColorAttachment { view: &parts.values.default_view, depth_slice: None, resolve_target: None, ops: clear })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_render_pipeline(haze);
            pass.set_bind_group(0, &group, &[]);
            pass.set_vertex_buffer(0, corners.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
            pass.draw_indexed(0..quads * 6, 0, 0..1);
        }
        let size = target.main_texture().size();
        let (hw, hh) = ((size.width >> 1).max(1) as f32, (size.height >> 1).max(1) as f32);
        let distortion = Vec4::new(draw.distortion.x / size.width.max(1) as f32, draw.distortion.x / size.height.max(1) as f32, 0.0, 0.0);
        let post = target.post_process_write();
        // One fullscreen pass: its figures, what it reads and where it draws.
        let run = |context: &mut RenderContext, label: &'static str, pass_pipeline: &RenderPipeline, step: Vec4, source: &TextureView, blurred: &TextureView, out: &TextureView| {
            let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
            if figures.write(&DistortUniform { distortion, step }).is_err() {
                return;
            }
            let buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some(label), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
            let group = device.create_bind_group(label, &cache.get_bind_group_layout(&pipeline.camera_layout), &BindGroupEntries::sequential((buffer.as_entire_binding(), source, &pipeline.sampler, &parts.values.default_view, blurred)));
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(RenderPassColorAttachment { view: out, depth_slice: None, resolve_target: None, ops: Operations::default() })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_render_pipeline(pass_pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        };
        // `MakeBlurTexture`: the frame at half size, then across and down
        // for each iteration, its step twice the last's.
        run(context, "distort_half", half, Vec4::ZERO, post.source, &parts.values.default_view, &parts.half.default_view);
        for i in 0..(draw.distortion.y as u32).max(1) {
            let step = (1u32 << i) as f32;
            run(context, "distort_blur_across", across, Vec4::new(step / hw, 0.0, 0.0, 0.0), &parts.half.default_view, &parts.values.default_view, &parts.scratch.default_view);
            run(context, "distort_blur_down", down, Vec4::new(0.0, step / hh, 0.0, 0.0), &parts.scratch.default_view, &parts.values.default_view, &parts.half.default_view);
        }
        run(context, "distort_composite", composite, Vec4::ZERO, post.source, &parts.half.default_view, post.destination);
        Ok(())
    }
}
