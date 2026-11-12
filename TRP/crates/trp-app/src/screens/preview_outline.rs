//! The edit build screen's outline of the part under the mouse.
//!
//! The edit build screen's outline of a part (workbench spec, "A part
//! under the mouse"): the game's `HighLightMesh` on the screen's camera.
//! The part's meshes are drawn white on black into a mask with no depth
//! (`edit_build`'s mask camera: what the rest of the weapon hides is in it
//! all the same), and the frame is then drawn again through the shader's
//! fifth pass (`preview_outline.wgsl`): a pixel whose mask differs from the
//! mask the line's width (2 pixels) to its right, or above it, has the
//! colour added once for each. The game adds it after the opaque image
//! effects and before its tonemap (`CameraEvent.AfterImageEffectsOpaque`);
//! here after Bevy's tonemapping (none on that camera) and before the
//! preview's photographic tonemap (`preview_tonemap`). `PROVISIONAL`:
//! "above" taken as up on the screen (the engine's blit's v running up),
//! and where the preview drew nothing the line is drawn whole over the
//! screen's ground (the game's camera draws over its own). The drawing
//! alone.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::GpuImage;
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;

const OUTLINE_SHADER: Handle<Shader> = uuid_handle!("b0c6e2d4-5a71-4f38-9e1d-2c84a7f3e915");

/// The line's width in pixels (`HighLightMesh.LineWidth` on the edit build
/// screen's `Weapon Camera`, read from the scene).
pub const LINE_WIDTH: f32 = 2.0;

/// On a camera: the line's width over its picture's width and height (x,
/// y), and its colour in the game's numbers (the alpha nought for none).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct PreviewOutline {
    pub offset: Vec4,
    pub color: Vec4,
}

impl PreviewOutline {
    /// The outline of that colour (none) on a picture of that size.
    pub fn of(color: Option<Color>, size: UVec2) -> Self {
        let offset = Vec4::new(LINE_WIDTH / size.x.max(1) as f32, LINE_WIDTH / size.y.max(1) as f32, 0.0, 0.0);
        let color = color.map_or(Vec4::ZERO, |color| {
            let c = color.to_srgba();
            Vec4::new(c.red, c.green, c.blue, 1.0)
        });
        Self { offset, color }
    }
}

impl ExtractComponent for PreviewOutline {
    type QueryData = &'static PreviewOutline;
    type QueryFilter = ();
    type Out = PreviewOutline;

    fn extract_component(outline: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*outline)
    }
}

/// On the same camera: the mask its outline is drawn by.
#[derive(Component, Clone)]
pub struct OutlineMask(pub Handle<Image>);

impl ExtractComponent for OutlineMask {
    type QueryData = &'static OutlineMask;
    type QueryFilter = ();
    type Out = OutlineMask;

    fn extract_component(mask: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(mask.clone())
    }
}

pub struct PreviewOutlinePlugin;

impl Plugin for PreviewOutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, OUTLINE_SHADER, "../shaders/preview_outline.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<PreviewOutline>::default(), UniformComponentPlugin::<PreviewOutline>::default(), ExtractComponentPlugin::<OutlineMask>::default()));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<OutlinePipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<OutlineNode>>(Core3d, PreviewOutlineLabel)
            .add_render_graph_edges(Core3d, (Node3d::Tonemapping, PreviewOutlineLabel, crate::screens::preview_tonemap::PreviewTonemapLabel));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct PreviewOutlineLabel;

#[derive(Resource)]
struct OutlinePipeline {
    layout: BindGroupLayoutDescriptor,
    frame_sampler: Sampler,
    mask_sampler: Sampler,
    fullscreen: FullscreenShader,
}

impl SpecializedRenderPipeline for OutlinePipeline {
    type Key = TextureFormat;

    fn specialize(&self, format: TextureFormat) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("preview outline".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: OUTLINE_SHADER,
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
        "preview_outline",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<PreviewOutline>(true),
            ),
        ),
    );
    let frame_sampler = device.create_sampler(&SamplerDescriptor { label: Some("preview_outline_frame"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    // (The mask's pixels compared as they are: the game's `ne`.)
    let mask_sampler = device.create_sampler(&SamplerDescriptor { label: Some("preview_outline_mask"), mag_filter: FilterMode::Nearest, min_filter: FilterMode::Nearest, ..default() });
    commands.insert_resource(OutlinePipeline { layout, frame_sampler, mask_sampler, fullscreen: fullscreen.clone() });
}

#[derive(Component)]
struct OutlineParts(CachedRenderPipelineId);

fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<PreviewOutline>>,
    cache: Res<PipelineCache>,
    pipeline: Res<OutlinePipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<OutlinePipeline>>,
) {
    for (entity, target) in &views {
        let id = pipelines.specialize(&cache, &pipeline, target.main_texture_format());
        commands.entity(entity).insert(OutlineParts(id));
    }
}

#[derive(Default)]
struct OutlineNode;

impl ViewNode for OutlineNode {
    type ViewQuery = (&'static ViewTarget, &'static OutlineParts, &'static PreviewOutline, &'static OutlineMask, &'static DynamicUniformIndex<PreviewOutline>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, outline, mask, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        // No part outlined: the frame left as it is.
        if outline.color.w <= 0.0 {
            return Ok(());
        }
        let pipeline = world.resource::<OutlinePipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(drawn) = cache.get_render_pipeline(parts.0) else { return Ok(()) };
        let Some(mask) = world.resource::<RenderAssets<GpuImage>>().get(&mask.0) else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<PreviewOutline>>().uniforms().binding() else { return Ok(()) };
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group(
            "preview_outline",
            &cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((write.source, &pipeline.frame_sampler, &mask.texture_view, &pipeline.mask_sampler, uniform)),
        );
        let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("preview outline"),
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
