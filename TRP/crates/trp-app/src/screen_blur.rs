//! The world blurred behind the inventory, as the game's camera blurs it
//! (`docs/spec/inventory.md`, "What lies behind the screen").
//!
//! The game's camera carries `InventoryBlur`, which the inventory screen
//! turns on as it opens in a raid and off as it shuts
//! (`CameraManager.Blur`): its value goes from 0 to 1 over half a second
//! of frames, and back as long. While it is above nothing, each frame the
//! camera's picture is copied into a square of 256 by 256, which is passed
//! down to 128 by 128 and back up eight times, each copy one sample by the
//! sampler's linear filter (Unity's `Graphics.Blit` into its temporary
//! textures, whose filter is bilinear); then the picture is the mix of the
//! sharp one and that square by the value (its shader, `FastBlur`: the two
//! samples and `value * (blurred - sharp) + sharp`, nothing else). The
//! sizes and the count are the camera prefab's (`Cam2` in the build's
//! resources). The screen's own drawing comes after and stays sharp.

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
use bevy::render::texture::{CachedTexture, TextureCache};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;

const SCREEN_BLUR_SHADER: Handle<Shader> = uuid_handle!("7c41e0a2-5d93-4b8e-a1f6-0e2c9b7d4a35");

/// The blurred square's sides, and how often it is passed down and back
/// (`InventoryBlur`: `_upsampleTexDimension`, `_downsampleTexDimension`,
/// `_blurCount` on the camera prefab).
const LARGE: u32 = 256;
const SMALL: u32 = 128;
const PASSES: usize = 8;

/// The time the value takes from nothing to whole and back
/// (`CameraManager.Blur`'s default, which the inventory screen uses).
const FADE_TIME: f32 = 0.5;

/// How far the world behind the screen is blurred, on the camera that
/// draws it: 0 sharp, 1 the blurred square alone.
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct ScreenBlur {
    pub value: f32,
}

impl ExtractComponent for ScreenBlur {
    type QueryData = &'static ScreenBlur;
    type QueryFilter = ();
    type Out = ScreenBlur;

    /// Only while it does something: with the value at nothing the game's
    /// camera copies its picture through unchanged, and here nothing runs.
    fn extract_component(blur: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (blur.value > f32::EPSILON).then_some(*blur)
    }
}

pub struct ScreenBlurPlugin;

impl Plugin for ScreenBlurPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SCREEN_BLUR_SHADER, "screen_blur.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<ScreenBlur>::default(), UniformComponentPlugin::<ScreenBlur>::default()));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<BlurPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<BlurNode>>(Core3d, BlurLabel)
            .add_render_graph_edges(Core3d, (Node3d::Tonemapping, BlurLabel, Node3d::EndMainPassPostProcessing));
    }
}

/// The value follows the inventory screen: up while it is open, down while
/// it is shut, a frame's time over the fade's each frame
/// (`InventoryBlur.Update`).
pub fn fade(time: Res<Time>, controller: Res<crate::controller::Controller>, mut cameras: Query<&mut ScreenBlur>) {
    let step = time.delta_secs() / FADE_TIME;
    for mut blur in &mut cameras {
        let value = if controller.inventory_open { blur.value + step } else { blur.value - step };
        let value = value.clamp(0.0, 1.0);
        if blur.value != value {
            blur.value = value;
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct BlurLabel;

#[derive(Resource)]
struct BlurPipeline {
    /// A picture and its sampler (the copies).
    copy: BindGroupLayoutDescriptor,
    /// The sharp picture, the sampler, the blurred square and the value.
    mix: BindGroupLayoutDescriptor,
    sampler: Sampler,
    fullscreen: FullscreenShader,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
struct BlurKey {
    format: TextureFormat,
    mix: bool,
}

impl SpecializedRenderPipeline for BlurPipeline {
    type Key = BlurKey;

    fn specialize(&self, key: BlurKey) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some(if key.mix { "screen blur mix" } else { "screen blur copy" }.into()),
            layout: vec![if key.mix { self.mix.clone() } else { self.copy.clone() }],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: SCREEN_BLUR_SHADER,
                entry_point: Some(if key.mix { "mix_blurred" } else { "copy" }.into()),
                targets: vec![Some(ColorTargetState { format: key.format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let copy = BindGroupLayoutDescriptor::new("screen_blur_copy", &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (picture(), sampler(SamplerBindingType::Filtering))));
    let mix = BindGroupLayoutDescriptor::new(
        "screen_blur_mix",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (picture(), sampler(SamplerBindingType::Filtering), picture(), uniform_buffer::<ScreenBlur>(true))),
    );
    // Unity's temporary textures: bilinear, clamped at their edges.
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("screen_blur"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    commands.insert_resource(BlurPipeline { copy, mix, sampler, fullscreen: fullscreen.clone() });
}

/// What a view being blurred needs this frame: its two squares and the
/// pipelines for its picture's format.
#[derive(Component)]
struct BlurViewParts {
    large: CachedTexture,
    small: CachedTexture,
    copy: CachedRenderPipelineId,
    mix: CachedRenderPipelineId,
}

fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<ScreenBlur>>,
    done: Query<Entity, (With<BlurViewParts>, Without<ScreenBlur>)>,
    device: Res<RenderDevice>,
    mut textures: ResMut<TextureCache>,
    cache: Res<PipelineCache>,
    pipeline: Res<BlurPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<BlurPipeline>>,
) {
    // A view no longer blurred lets its squares go: the value is no longer
    // extracted (`ExtractComponent` takes it off), but what was put on the
    // view beside it stays unless taken off here.
    for entity in &done {
        commands.entity(entity).remove::<(BlurViewParts, DynamicUniformIndex<ScreenBlur>)>();
    }
    for (entity, target) in &views {
        let format = target.main_texture_format();
        let square = |side: u32, label: &'static str| TextureDescriptor {
            label: Some(label),
            size: Extent3d { width: side, height: side, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        commands.entity(entity).insert(BlurViewParts {
            large: textures.get(&device, square(LARGE, "screen_blur_large")),
            small: textures.get(&device, square(SMALL, "screen_blur_small")),
            copy: pipelines.specialize(&cache, &pipeline, BlurKey { format, mix: false }),
            mix: pipelines.specialize(&cache, &pipeline, BlurKey { format, mix: true }),
        });
    }
}

#[derive(Default)]
struct BlurNode;

impl ViewNode for BlurNode {
    // Only while the view's value is extracted this frame: its parts and
    // its uniform's index outlive it by a frame (`prepare`).
    type ViewQuery = (&'static ViewTarget, &'static ScreenBlur, &'static BlurViewParts, &'static DynamicUniformIndex<ScreenBlur>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, _, parts, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<BlurPipeline>();
        let cache = world.resource::<PipelineCache>();
        let (Some(copy), Some(mix)) = (cache.get_render_pipeline(parts.copy), cache.get_render_pipeline(parts.mix)) else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<ScreenBlur>>().uniforms().binding() else { return Ok(()) };
        let copy_layout = cache.get_bind_group_layout(&pipeline.copy);
        let pass = |context: &mut RenderContext, from: &TextureView, into: &TextureView| {
            let group = context.render_device().create_bind_group("screen_blur_copy", &copy_layout, &BindGroupEntries::sequential((from, &pipeline.sampler)));
            let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("screen_blur_copy"),
                color_attachments: &[Some(RenderPassColorAttachment { view: into, depth_slice: None, resolve_target: None, ops: Operations::default() })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render.set_render_pipeline(copy);
            render.set_bind_group(0, &group, &[]);
            render.draw(0..3, 0..1);
        };
        pass(context, target.main_texture_view(), &parts.large.default_view);
        for _ in 0..PASSES {
            pass(context, &parts.large.default_view, &parts.small.default_view);
            pass(context, &parts.small.default_view, &parts.large.default_view);
        }
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group(
            "screen_blur_mix",
            &cache.get_bind_group_layout(&pipeline.mix),
            &BindGroupEntries::sequential((write.source, &pipeline.sampler, &parts.large.default_view, uniform)),
        );
        let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("screen_blur_mix"),
            color_attachments: &[Some(RenderPassColorAttachment { view: write.destination, depth_slice: None, resolve_target: None, ops: Operations::default() })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render.set_render_pipeline(mix);
        render.set_bind_group(0, &group, &[index.index()]);
        render.draw(0..3, 0..1);
        Ok(())
    }
}
