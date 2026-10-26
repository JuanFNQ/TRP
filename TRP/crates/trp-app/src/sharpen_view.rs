//! The camera's sharpening and desaturation (health spec, "Desaturation";
//! the options' `CAMERA SHARPENING`, with `CUSTOMS LIGHT`, on as the game
//! has it; the user, 2026-10-26: "Whole pass, own switch"): the level
//! camera's `CC_Sharpen` with its `DesaturateEffect` (read with the level's
//! light, `lighting::LevelLighting::sharpen`), drawn by the game's compiled
//! shader's sums (`sharpen_view.wgsl`) in the camera's order, after pain's
//! blur and before the face shield's mask. Its strength is the player's
//! graphics setting (`Sharpen`), the weather's share the weather's
//! (`WeatherController.ControlDesaturateEffect`), and the health's and the
//! painkiller's ring the health's accumulators' (`trp_sim::screen_effects`,
//! with `HEALTH SYSTEM`; none without). On while the strength is 0.1 or
//! more, or nought with the desaturation on (`CC_SharpenAccumulator`).
//! `PROVISIONAL`: a pixel and the aspect taken from the picture drawn (the
//! game's `_PX`, `_PY` and `_ScreenParams` are the screen's), the ramp
//! sampled bilinear and clamped (its own sampler not read), night vision
//! none (its ratio 1).

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::GpuImage;
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;

const SHARPEN_SHADER: Handle<Shader> = uuid_handle!("3f8a2c61-9d47-4e15-b0c3-7a5e6d9f1b28");

/// What the pass draws this frame (`on` 1 while it runs).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct SharpenScreen {
    /// Strength, clamp, the ring's falloff, night vision's ratio.
    pub figures: Vec4,
    /// The weather's, the health's and the mask's desaturation, the radius.
    pub desaturate: Vec4,
    pub ramp_offset: Vec4,
    pub on: f32,
}

impl ExtractComponent for SharpenScreen {
    type QueryData = &'static SharpenScreen;
    type QueryFilter = ();
    type Out = SharpenScreen;

    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (screen.on > 0.0).then_some(*screen)
    }
}

/// The ramp the brightness looks up (`DesaturateEffect.textureRamp`), once
/// made.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct SharpenImages {
    pub ramp: Option<Handle<Image>>,
}

/// The frame's figures on the main camera: the level camera's, the
/// player's strength and the weather's share, and the health's
/// accumulators' where health runs; the ramp made once.
pub fn update(
    mut commands: Commands,
    level: Option<Res<crate::lighting::LevelLighting>>,
    health: Option<ResMut<crate::contusion_view::ContusionState>>,
    mut images: ResMut<SharpenImages>,
    mut assets: ResMut<Assets<Image>>,
    cameras: Query<Entity, With<crate::MainCamera>>,
    mut radii_set: Local<bool>,
) {
    let Some(sharpening) = level.as_ref().and_then(|level| level.sharpen.as_ref()) else { return };
    let mut health = health;
    if !*radii_set {
        *radii_set = true;
        if let Some(state) = health.as_mut() {
            state.set_radii(sharpening.data.min_max_radius);
        }
    }
    if images.ramp.is_none() {
        // Read as stored, as the game's gamma-space project reads it.
        let picture = trp_data::unity::extract::TextureData { srgb: false, ..sharpening.data.ramp.clone() };
        match crate::render_data::build_image(&picture, crate::render_data::Role::Color) {
            Ok(image) => images.ramp = Some(assets.add(image)),
            Err(e) => {
                warn!("the desaturation's ramp: {e:#}; the camera's sharpening is not drawn");
                return;
            }
        }
    }
    let data = &sharpening.data;
    let [health, mask, radius] = health.map_or([0.0, 0.0, data.min_max_radius[1]], |state| state.desaturation());
    let strength = sharpening.strength;
    let on = strength >= 0.1 || (strength == 0.0 && data.desaturate);
    let shown = SharpenScreen {
        figures: Vec4::new(strength, data.clamp, data.radius_falloff, 1.0),
        desaturate: Vec4::new(sharpening.weather.clamp(0.0, 1.0), health.clamp(0.0, 1.0), mask.clamp(0.0, 1.0), radius),
        ramp_offset: Vec4::new(data.ramp_offset[0], data.ramp_offset[1], data.ramp_offset[2], 0.0),
        on: if on { 1.0 } else { 0.0 },
    };
    for camera in &cameras {
        commands.entity(camera).insert(shown);
    }
}

pub struct SharpenViewPlugin;

impl Plugin for SharpenViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SHARPEN_SHADER, "sharpen_view.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<SharpenScreen>::default(), UniformComponentPlugin::<SharpenScreen>::default(), ExtractResourcePlugin::<SharpenImages>::default()));
        app.init_resource::<SharpenImages>();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<SharpenPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<SharpenNode>>(Core3d, SharpenLabel)
            .add_render_graph_edges(Core3d, (crate::contusion_view::ContusionLabel, SharpenLabel, crate::contusion_view::ContusionLateLabel));
    }
}

/// After pain's blur (`CC_RadialBlur`, the camera's 29th) and before the
/// face shield's mask (`VisorEffect`, its 40th): the sharpening is its 30th.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct SharpenLabel;

#[derive(Resource)]
struct SharpenPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    fullscreen: FullscreenShader,
}

impl SpecializedRenderPipeline for SharpenPipeline {
    type Key = TextureFormat;

    fn specialize(&self, format: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("sharpen".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: SHARPEN_SHADER,
                entry_point: Some("sharpen".into()),
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let layout = BindGroupLayoutDescriptor::new("sharpen", &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (picture(), sampler(SamplerBindingType::Filtering), uniform_buffer::<SharpenScreen>(true), picture())));
    // The frame's and the ramp's: bilinear, clamped at their edges.
    let sampler = device.create_sampler(&SamplerDescriptor { label: Some("sharpen"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(SharpenPipeline { layout, sampler, fullscreen: fullscreen.clone() });
}

#[derive(Component)]
struct SharpenParts(CachedRenderPipelineId);

fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<SharpenScreen>>,
    done: Query<Entity, (With<SharpenParts>, Without<SharpenScreen>)>,
    cache: Res<PipelineCache>,
    pipeline: Res<SharpenPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<SharpenPipeline>>,
) {
    for entity in &done {
        commands.entity(entity).remove::<(SharpenParts, DynamicUniformIndex<SharpenScreen>)>();
    }
    for (entity, target) in &views {
        let id = pipelines.specialize(&cache, &pipeline, target.main_texture_format());
        commands.entity(entity).insert(SharpenParts(id));
    }
}

#[derive(Default)]
struct SharpenNode;

impl ViewNode for SharpenNode {
    type ViewQuery = (&'static ViewTarget, &'static SharpenParts, &'static DynamicUniformIndex<SharpenScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<SharpenPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(render) = cache.get_render_pipeline(parts.0) else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<SharpenScreen>>().uniforms().binding() else { return Ok(()) };
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let Some(ramp) = world.resource::<SharpenImages>().ramp.as_ref().and_then(|handle| gpu.get(handle)) else { return Ok(()) };
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group("sharpen", &cache.get_bind_group_layout(&pipeline.layout), &BindGroupEntries::sequential((write.source, &pipeline.sampler, uniform, &ramp.texture_view)));
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("sharpen"),
            color_attachments: &[Some(RenderPassColorAttachment { view: write.destination, depth_slice: None, resolve_target: None, ops: Operations::default() })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(render);
        pass.set_bind_group(0, &group, &[index.index()]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
