//! The photographic tonemap of the inspect window's preview camera.
//!
//! The inspect window's preview camera's photographic tonemap (inventory
//! spec, "The window's model"): Standard Assets' `Tonemapping` image
//! effect of the type 3 on the pool's camera, `1 - 2^(-exposure × c)` on
//! each channel of its picture in half floats, its exposure the template's
//! (2). The game's project is in gamma space, so the curve is put on the
//! frame's figures as the game's numbers (the 2.2 power) and back.
//! `PROVISIONAL`: the curve is Standard Assets' published `fragPhotographic`,
//! the build's own pass not disassembled.
//!
//! And the OVERALL tab's figure's camera's (`Camera_inventory`'s
//! `PrismEffects`, only its tonemap on, with `OVERALL FIGURE`; inventory
//! spec, "The character's light"): PRISM's filmic curve squared, as the
//! world's PRISM puts it (`prism.wgsl`), with no exposure, on a picture of
//! eight bits a channel (the camera draws without HDR into an `ARGB32`
//! texture), so its figures are held to nought and one first; in the
//! game's numbers and back, as the photographic.
//!
//! "Back" is the number the picture stores: the pass writes the light
//! whose sRGB encoding (the picture's, `Rgba8UnormSrgb`) is that number,
//! which the screens show as stored (before, it wrote the
//! number to the 2.2 power, and the encoding left the darks lower: a 0.06
//! stored as 0.027, a 0.13 as 0.11, a half unchanged).

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;

const TONEMAP_SHADER: Handle<Shader> = uuid_handle!("7d2e9a41-3c58-4f06-b1e7-5a9c0d36e8f2");

/// On a camera: the photographic tonemap's exposure (x), and which curve
/// (y: 0 the photographic, 1 PRISM's filmic); the filmic's tone and
/// secondary tone (`PrismSettings::tone`, `secondary_tone`).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct PreviewTonemap {
    pub figures: Vec4,
    pub tone: Vec4,
    pub secondary: Vec4,
}

impl PreviewTonemap {
    /// The inspect window's photographic curve at that exposure.
    pub fn of(exposure: f32) -> Self {
        Self { figures: Vec4::new(exposure, 0.0, 0.0, 0.0), ..default() }
    }

    /// PRISM's filmic curve by a camera's settings.
    pub fn filmic(prism: &trp_data::drawing::prism::PrismSettings) -> Self {
        let ([t0, t1, t2], [s0, s1, s2]) = (prism.tone, prism.secondary_tone);
        Self { figures: Vec4::new(0.0, 1.0, 0.0, 0.0), tone: Vec4::new(t0, t1, t2, t2), secondary: Vec4::new(s0, s1, s2, s2) }
    }
}

impl ExtractComponent for PreviewTonemap {
    type QueryData = &'static PreviewTonemap;
    type QueryFilter = ();
    type Out = PreviewTonemap;

    fn extract_component(tonemap: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*tonemap)
    }
}

pub struct PreviewTonemapPlugin;

impl Plugin for PreviewTonemapPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, TONEMAP_SHADER, "../shaders/preview_tonemap.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<PreviewTonemap>::default(), UniformComponentPlugin::<PreviewTonemap>::default()));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<TonemapPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<TonemapNode>>(Core3d, PreviewTonemapLabel)
            .add_render_graph_edges(Core3d, (Node3d::Tonemapping, PreviewTonemapLabel, crate::ui::screen_blur::HitBlurLabel));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct PreviewTonemapLabel;

#[derive(Resource)]
struct TonemapPipeline {
    layout: BindGroupLayoutDescriptor,
    frame_sampler: Sampler,
    fullscreen: FullscreenShader,
}

impl SpecializedRenderPipeline for TonemapPipeline {
    type Key = TextureFormat;

    fn specialize(&self, format: TextureFormat) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("preview photographic tonemap".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: TONEMAP_SHADER,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "preview_tonemap",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), uniform_buffer::<PreviewTonemap>(true)),
        ),
    );
    let frame_sampler = device.create_sampler(&SamplerDescriptor { label: Some("preview_tonemap_frame"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(TonemapPipeline { layout, frame_sampler, fullscreen: fullscreen.clone() });
}

#[derive(Component)]
struct TonemapParts(CachedRenderPipelineId);

fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<PreviewTonemap>>,
    cache: Res<PipelineCache>,
    pipeline: Res<TonemapPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<TonemapPipeline>>,
) {
    for (entity, target) in &views {
        let id = pipelines.specialize(&cache, &pipeline, target.main_texture_format());
        commands.entity(entity).insert(TonemapParts(id));
    }
}

#[derive(Default)]
struct TonemapNode;

impl ViewNode for TonemapNode {
    type ViewQuery = (&'static ViewTarget, &'static TonemapParts, &'static DynamicUniformIndex<PreviewTonemap>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<TonemapPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(drawn) = cache.get_render_pipeline(parts.0) else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<PreviewTonemap>>().uniforms().binding() else { return Ok(()) };
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group("preview_tonemap", &cache.get_bind_group_layout(&pipeline.layout), &BindGroupEntries::sequential((write.source, &pipeline.frame_sampler, uniform)));
        let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("preview photographic tonemap"),
            color_attachments: &[Some(RenderPassColorAttachment { view: write.destination, depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Clear(Default::default()), store: StoreOp::Store } })],
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
