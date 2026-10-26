//! The camera's own colour filter with the options' `CAMERA COLOR FILTER`
//! on, under `CUSTOMS LIGHT` (`docs/spec/lighting.md`, "The camera's colour
//! filter"): `CC_Vintage` as every level's camera ships it (Customs': the
//! table "Jason" at 0.498), after PRISM and before the camera's screen
//! effects, on the frame in the game's numbers (`gamma_ui`), as the game's
//! camera runs it. Not the player's optional colour filter of Tarkov's
//! Post-FX settings (`GradingPostFX`, off unless the player turns it on),
//! which is not built.
//!
//! The table is read at the start (`trp_data::prism::ColorFilter`) and put
//! on the graphics card once, its rows as stored and repeated as its
//! texture is: the shader reads it as the game's does.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;

const COLOR_FILTER_SHADER: Handle<Shader> = uuid_handle!("c4a1e8f3-5b27-4d96-8e0a-2f7b9c13d654");

/// On the camera: the filter's amount (`ColorFilter` in `color_filter.wgsl`).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct ColorFilterScreen {
    pub figures: Vec4,
}

impl ColorFilterScreen {
    pub fn of(amount: f32) -> Self {
        Self { figures: Vec4::new(amount, 0.0, 0.0, 0.0) }
    }
}

impl ExtractComponent for ColorFilterScreen {
    type QueryData = &'static ColorFilterScreen;
    type QueryFilter = ();
    type Out = ColorFilterScreen;

    fn extract_component(filter: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*filter)
    }
}

/// The filter's table as read, in the render world until it is on the card.
#[derive(Resource, Clone)]
struct FilterTable(trp_data::prism::FilterTable);

/// The plugin, with the table where the filter is drawn (none: nothing of
/// it is put on the card).
pub struct ColorFilterPlugin {
    pub table: Option<trp_data::prism::FilterTable>,
}

impl Plugin for ColorFilterPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, COLOR_FILTER_SHADER, "color_filter.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<ColorFilterScreen>::default(), UniformComponentPlugin::<ColorFilterScreen>::default()));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        if let Some(table) = &self.table {
            render_app.insert_resource(FilterTable(table.clone()));
        }
        render_app
            .init_resource::<SpecializedRenderPipelines<FilterPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<FilterNode>>(Core3d, ColorFilterLabel)
            .add_render_graph_edges(Core3d, (crate::prism_view::PrismLabel, ColorFilterLabel, crate::screen_blur::HitBlurLabel));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct ColorFilterLabel;

#[derive(Resource)]
struct FilterPipeline {
    layout: BindGroupLayoutDescriptor,
    /// The frame's sampler (bilinear, clamped: Unity's temporary textures).
    frame_sampler: Sampler,
    fullscreen: FullscreenShader,
}

impl SpecializedRenderPipeline for FilterPipeline {
    type Key = TextureFormat;

    fn specialize(&self, format: TextureFormat) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("camera colour filter".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: COLOR_FILTER_SHADER,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let layout = BindGroupLayoutDescriptor::new(
        "color_filter",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (picture(), sampler(SamplerBindingType::Filtering), picture(), sampler(SamplerBindingType::Filtering), uniform_buffer::<ColorFilterScreen>(true)),
        ),
    );
    let frame_sampler = device.create_sampler(&SamplerDescriptor { label: Some("color_filter_frame"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(FilterPipeline { layout, frame_sampler, fullscreen: fullscreen.clone() });
}

/// The table on the card, made once, and its sampler as its texture's
/// settings have it (Jason's: repeated, bilinear).
#[derive(Resource)]
struct TableOnCard(TextureView, Sampler);

/// A view's pipeline this frame.
#[derive(Component)]
struct FilterParts(CachedRenderPipelineId);

fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<ColorFilterScreen>>,
    table: Option<Res<FilterTable>>,
    on_card: Option<Res<TableOnCard>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    cache: Res<PipelineCache>,
    pipeline: Res<FilterPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<FilterPipeline>>,
) {
    let Some(table) = table else { return };
    let table = &table.0;
    if on_card.is_none() {
        let texture = device.create_texture_with_data(
            &queue,
            &TextureDescriptor {
                label: Some("color_filter_table"),
                size: Extent3d { width: table.width, height: table.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            TextureDataOrder::LayerMajor,
            &table.rgba,
        );
        let address = |clamp: bool| if clamp { AddressMode::ClampToEdge } else { AddressMode::Repeat };
        let filter = if table.point { FilterMode::Nearest } else { FilterMode::Linear };
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("color_filter_table"),
            address_mode_u: address(table.clamp[0]),
            address_mode_v: address(table.clamp[1]),
            mag_filter: filter,
            min_filter: filter,
            ..default()
        });
        commands.insert_resource(TableOnCard(texture.create_view(&TextureViewDescriptor::default()), sampler));
    }
    for (entity, target) in &views {
        let id = pipelines.specialize(&cache, &pipeline, target.main_texture_format());
        commands.entity(entity).insert(FilterParts(id));
    }
}

#[derive(Default)]
struct FilterNode;

impl ViewNode for FilterNode {
    type ViewQuery = (&'static ViewTarget, &'static FilterParts, &'static DynamicUniformIndex<ColorFilterScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<FilterPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(drawn) = cache.get_render_pipeline(parts.0) else { return Ok(()) };
        let Some(table) = world.get_resource::<TableOnCard>() else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<ColorFilterScreen>>().uniforms().binding() else { return Ok(()) };
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group(
            "color_filter",
            &cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((write.source, &pipeline.frame_sampler, &table.0, &table.1, uniform)),
        );
        let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("camera colour filter"),
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
