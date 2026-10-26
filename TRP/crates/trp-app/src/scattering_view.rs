//! The sky's scattering over the scene with the options' `SKY SCATTERING`
//! on, under `CUSTOMS LIGHT` (`docs/spec/lighting.md`, "The sky's
//! scattering over the scene"): the camera's `TOD_Scattering` as Customs'
//! ran it (its density 0.0041 and glow 0.9755, the level settings' height
//! falloff and zero level), on the frame in the game's numbers right after
//! the scene's depth is copied and before the world's effects, as the
//! game's opaque image effect runs after the opaque scene and before the
//! transparent. The sky's figures are the moment's (`lighting`); the
//! camera's place and matrices this frame's. The drawing alone: nothing of
//! the simulation changes.
//!
//! `PROVISIONAL`: the range's heights taken as Customs' own (its ground at
//! nought, the zero level 1.6 m under it), and the game's `_StencilShadow`
//! (its indoor volumes, none on the range) as nought.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderContext;
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use trp_sim::tod::SkyGlobals;

use crate::tod_sky_view::TodSky;

const SCATTERING_SHADER: Handle<Shader> = uuid_handle!("5e2b9d71-3c84-4a1f-b06d-8f4c2a7e9b13");

/// On the camera: the pass's figures (`Scattering` in `scattering.wgsl`).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct ScatteringScreen {
    pub sun_sky: Vec4,
    pub moon_sky: Vec4,
    pub ground: Vec4,
    pub moon_halo: Vec4,
    pub sun: Vec4,
    pub moon: Vec4,
    pub figures: Vec4,
    pub k_beta_mie: Vec4,
    pub k_sun: Vec4,
    pub k_4pi: Vec4,
    pub k_radius: Vec4,
    pub k_scale: Vec4,
    /// The height falloff, the camera's height over the zero level (this
    /// frame's), the density, the sunrise glow.
    pub density: Vec4,
    pub world_from_clip: Mat4,
    pub camera: Vec4,
    pub dithering: [Vec4; 16],
}

/// The scattering's own figures and the zero level its height is taken
/// from (`ScatteringScreen::follow` adds the camera's height each frame).
#[derive(Component, Clone, Copy)]
pub struct ScatteringZero(pub f32);

impl ScatteringScreen {
    /// From the sky's figures, the camera's scattering and its dithering's
    /// 64 alphas (rows as stored).
    pub fn of(sky: &SkyGlobals, height_falloff: f32, density: f32, sunrise_glow: f32, dithering: &[u8]) -> Self {
        let t = TodSky::of(sky, None);
        let alpha = |i: usize| dithering.get(i).map_or(0.0, |&a| a as f32 / 255.0);
        Self {
            sun_sky: t.sun_sky,
            moon_sky: t.moon_sky,
            ground: t.ground,
            moon_halo: t.moon_halo,
            sun: t.sun,
            moon: t.moon,
            figures: t.figures,
            k_beta_mie: t.k_beta_mie,
            k_sun: t.k_sun,
            k_4pi: t.k_4pi,
            k_radius: t.k_radius,
            k_scale: t.k_scale,
            density: Vec4::new(height_falloff, 0.0, density, sunrise_glow),
            world_from_clip: Mat4::IDENTITY,
            camera: Vec4::ZERO,
            dithering: std::array::from_fn(|k| Vec4::new(alpha(4 * k), alpha(4 * k + 1), alpha(4 * k + 2), alpha(4 * k + 3))),
        }
    }
}

impl ExtractComponent for ScatteringScreen {
    type QueryData = &'static ScatteringScreen;
    type QueryFilter = ();
    type Out = ScatteringScreen;

    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*screen)
    }
}

/// This frame's camera: its place, its height over the zero level
/// (`_Density.y`, `position.y - ZeroLevel`), the world from its clip.
fn follow(mut cameras: Query<(&Camera, &GlobalTransform, &ScatteringZero, &mut ScatteringScreen)>) {
    for (camera, eye, zero, mut screen) in &mut cameras {
        let place = eye.translation();
        screen.camera = place.extend(1.0);
        screen.density.y = place.y - zero.0;
        screen.world_from_clip = eye.to_matrix() * camera.clip_from_view().inverse();
    }
}

pub struct ScatteringPlugin;

impl Plugin for ScatteringPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SCATTERING_SHADER, "scattering.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<ScatteringScreen>::default(), UniformComponentPlugin::<ScatteringScreen>::default()))
            .add_systems(PostUpdate, follow.after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<ScatteringPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<ScatteringNode>>(Core3d, ScatteringLabel)
            .add_render_graph_edges(Core3d, (crate::muzzle_view::DepthCopyLabel, ScatteringLabel, crate::explosion_view::ExplosionLabel));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct ScatteringLabel;

#[derive(Resource)]
struct ScatteringPipeline {
    layout: BindGroupLayoutDescriptor,
    fullscreen: FullscreenShader,
}

impl SpecializedRenderPipeline for ScatteringPipeline {
    type Key = TextureFormat;

    fn specialize(&self, format: TextureFormat) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("sky scattering".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: SCATTERING_SHADER,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "sky_scattering",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(TextureSampleType::Float { filterable: false }), texture_2d(TextureSampleType::Float { filterable: false }), uniform_buffer::<ScatteringScreen>(true)),
        ),
    );
    commands.insert_resource(ScatteringPipeline { layout, fullscreen: fullscreen.clone() });
}

#[derive(Component)]
struct ScatteringParts(CachedRenderPipelineId);

fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<ScatteringScreen>>,
    cache: Res<PipelineCache>,
    pipeline: Res<ScatteringPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<ScatteringPipeline>>,
) {
    for (entity, target) in &views {
        let id = pipelines.specialize(&cache, &pipeline, target.main_texture_format());
        commands.entity(entity).insert(ScatteringParts(id));
    }
}

#[derive(Default)]
struct ScatteringNode;

impl ViewNode for ScatteringNode {
    type ViewQuery = (&'static ViewTarget, &'static ScatteringParts, &'static crate::muzzle_view::JetParts, &'static DynamicUniformIndex<ScatteringScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, jets, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<ScatteringPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(drawn) = cache.get_render_pipeline(parts.0) else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<ScatteringScreen>>().uniforms().binding() else { return Ok(()) };
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group("sky_scattering", &cache.get_bind_group_layout(&pipeline.layout), &BindGroupEntries::sequential((write.source, &jets.depth.default_view, uniform)));
        let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("sky scattering"),
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
