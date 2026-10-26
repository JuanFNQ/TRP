//! The camera's PRISM effect with the options' `CUSTOMS LIGHT` on
//! (`docs/spec/lighting.md`, "PRISM: the exposure, the vignette and the
//! tonemap"): after the world's effects and before the camera's screen
//! effects, on the frame in the game's numbers (`gamma_ui`), as the game's
//! camera runs it after the scene and its particles.
//!
//! Each frame: the frame's brightness read into a square of half its width
//! on a side, halved down to one texel, blended into the brightness kept
//! from the frames before by a share of the speed (the first frame copies
//! it); then the frame times the exposure (the middle grey over the kept
//! brightness, held to the limits), its vignette, and its filmic curve
//! squared. The middle grey and the speed are the environment manager's,
//! following the open air's targets (`trp_sim::prism::EnvironmentExposure`):
//! the range has no indoor trigger.

use std::collections::HashMap;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{CachedTexture, TextureCache};
use bevy::render::view::{ExtractedView, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use trp_sim::prism::{adaptation_share, EnvironmentExposure};

const PRISM_SHADER: Handle<Shader> = uuid_handle!("4b9d2e71-8a3c-4f05-b6e2-91c7d3a85f10");

/// The textures the brightness goes through (the game's
/// `defaultHDRRenderTextureFormat`, a half float's four channels).
const BRIGHTNESS_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

/// The kept brightness (`_adaptationTexture`, `RenderTextureFormat.ARGB32`):
/// eight bits a channel, so it is held between nought and one and kept in
/// 255ths, and a blend that would move it less than half a step leaves it
/// where it is, as the game's does.
const KEPT_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// On the camera: this frame's figures (`Prism` in `prism.wgsl`).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct PrismScreen {
    /// The middle grey, the lower and upper limits, the adaptation's share
    /// this frame.
    pub exposure: Vec4,
    pub tone: Vec4,
    pub secondary: Vec4,
    /// Start, end, strength, on.
    pub vignette: Vec4,
    pub vignette_color: Vec4,
}

impl ExtractComponent for PrismScreen {
    type QueryData = &'static PrismScreen;
    type QueryFilter = ();
    type Out = PrismScreen;

    fn extract_component(prism: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*prism)
    }
}

/// The environment manager's figures as they follow their targets, and the
/// level's indoor trigger's figures for the range hall (`range::in_hall`),
/// where the level has them.
#[derive(Resource)]
pub struct PrismEnvironment {
    pub exposure: EnvironmentExposure,
    pub indoor: Option<trp_sim::prism::IndoorFigures>,
}

/// The camera's PRISM as the level's camera has it, made the figures the
/// main pass takes (all but the exposure's middle grey and share, which
/// change each frame).
pub fn screen_of(prism: &trp_data::prism::PrismSettings) -> PrismScreen {
    let [t0, t1, t2] = prism.tone;
    let [s0, s1, s2] = prism.secondary_tone;
    PrismScreen {
        exposure: Vec4::new(0.0, prism.lower_limit, prism.upper_limit, 1.0),
        tone: Vec4::new(t0, t1, t2, t2),
        secondary: Vec4::new(s0, s1, s2, s2),
        vignette: Vec4::new(prism.vignette_start, prism.vignette_end, prism.vignette_strength, if prism.use_vignette { 1.0 } else { 0.0 }),
        vignette_color: Vec4::from(prism.vignette_color),
    }
}

pub struct PrismPlugin;

impl Plugin for PrismPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, PRISM_SHADER, "prism.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<PrismScreen>::default(), UniformComponentPlugin::<PrismScreen>::default()))
            .add_systems(PostUpdate, follow);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<PrismPipeline>>()
            .init_resource::<KeptBrightness>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<PrismNode>>(Core3d, PrismLabel)
            .add_render_graph_edges(Core3d, (crate::muzzle_view::JetLabel, PrismLabel, crate::screen_blur::HitBlurLabel));
    }
}

/// The environment manager's step, and this frame's middle grey and share
/// on the camera (`EnvironmentManager.Update` before the camera draws).
/// Under the warm-up's curtain the share is whole: the kept brightness is
/// the frame's, copied, as the game's camera copies it on its first frame,
/// so that play begins from the whole scene's brightness and not from
/// whatever the first frame drawn while the range was still being made
/// held (an eight-bit kept brightness stops up to 0.08 short of where it
/// is going: lighting spec, "PRISM"). How results are drawn across frames
/// alone.
fn follow(
    time: Res<Time>,
    environment: Option<ResMut<PrismEnvironment>>,
    warmup: Option<Res<crate::Warmup>>,
    controller: Option<Res<crate::controller::Controller>>,
    mut cameras: Query<&mut PrismScreen>,
) {
    let Some(mut environment) = environment else { return };
    // The player's trigger by where the feet are (the game's triggers are
    // checked against the player's place, `EnvironmentManager.
    // UpdateEnvironmentForPlayer`): inside the hall its figures, else none.
    if let Some(controller) = controller {
        let inside = crate::range::in_hall(controller.shown().position + Vec3::Y * 0.1);
        let indoor = environment.indoor.filter(|_| inside);
        environment.exposure.set_trigger(indoor);
    }
    environment.exposure.step(time.delta_secs());
    let warming = warmup.is_some_and(|w| w.0);
    for mut screen in &mut cameras {
        screen.exposure.x = environment.exposure.middle_grey;
        screen.exposure.w = if warming { 1.0 } else { adaptation_share(environment.exposure.speed) };
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct PrismLabel;

#[derive(Resource)]
struct PrismPipeline {
    /// A picture, its sampler and the figures.
    pass: BindGroupLayoutDescriptor,
    /// The kept brightness, for the main pass.
    kept: BindGroupLayoutDescriptor,
    sampler: Sampler,
    fullscreen: FullscreenShader,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum Pass {
    First,
    Halve,
    /// The first frame's copy into what is kept.
    Copy,
    Adapt,
    Main(TextureFormat),
}

impl SpecializedRenderPipeline for PrismPipeline {
    type Key = Pass;

    fn specialize(&self, key: Pass) -> RenderPipelineDescriptor {
        let (label, entry, format, blend) = match key {
            Pass::First => ("prism brightness", "brightness_first", BRIGHTNESS_FORMAT, None),
            Pass::Halve => ("prism halve", "halve", BRIGHTNESS_FORMAT, None),
            Pass::Copy => ("prism keep", "halve", KEPT_FORMAT, None),
            Pass::Adapt => {
                let over = BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
                ("prism adapt", "adapt", KEPT_FORMAT, Some(BlendState { color: over, alpha: over }))
            }
            Pass::Main(format) => ("prism main", "main_pass", format, None),
        };
        let mut layout = vec![self.pass.clone()];
        if matches!(key, Pass::Main(_)) {
            layout.push(self.kept.clone());
        }
        RenderPipelineDescriptor {
            label: Some(label.into()),
            layout,
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: PRISM_SHADER,
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState { format, blend, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let pass = BindGroupLayoutDescriptor::new(
        "prism_pass",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (picture(), sampler(SamplerBindingType::Filtering), uniform_buffer::<PrismScreen>(true))),
    );
    let kept = BindGroupLayoutDescriptor::new("prism_kept", &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (picture(),)));
    // PROVISIONAL: Unity's temporary textures taken as bilinear and clamped at
    // their edges (their wrap mode not read from the game); the camera's other
    // effects are Cam2's while PRISM is the level's camera's (`lighting.rs`,
    // `trp_data::camera_effects`).
    // Unity's temporary textures: bilinear, clamped at their edges.
    let sampler = device.create_sampler(&SamplerDescriptor { label: Some("prism"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(PrismPipeline { pass, kept, sampler, fullscreen: fullscreen.clone() });
}

/// The brightness kept from frame to frame for each view
/// (`_adaptationTexture`), one texel, and whether its first copy has been
/// drawn (the pipelines ready).
#[derive(Resource, Default)]
struct KeptBrightness(HashMap<Entity, (Texture, TextureView, bool)>);

/// What a view needs this frame: the textures the brightness is halved
/// through, the kept one, whether this is its first frame, the pipelines.
#[derive(Component)]
struct PrismParts {
    chain: Vec<CachedTexture>,
    kept: TextureView,
    first: bool,
    pipelines: [CachedRenderPipelineId; 5],
}

fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, &ExtractedView), With<PrismScreen>>,
    device: Res<RenderDevice>,
    mut textures: ResMut<TextureCache>,
    mut kept: ResMut<KeptBrightness>,
    cache: Res<PipelineCache>,
    pipeline: Res<PrismPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<PrismPipeline>>,
) {
    for (entity, target, view) in &views {
        let texture = |size: (u32, u32), label: &'static str| TextureDescriptor {
            label: Some(label),
            size: Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: BRIGHTNESS_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        // A square of half the frame's width on a side, then halved (each
        // side at least one) until a texel (`histWidth` 1).
        let width = view.viewport.z.max(2);
        let mut size = ((width / 2).max(1), (width / 2).max(1));
        let mut chain = vec![textures.get(&device, texture(size, "prism_brightness"))];
        while size.0 > 1 || size.1 > 1 {
            size = ((size.0 / 2).max(1), (size.1 / 2).max(1));
            chain.push(textures.get(&device, texture(size, "prism_brightness")));
        }
        let format = target.main_texture_format();
        let ids = [Pass::First, Pass::Halve, Pass::Copy, Pass::Adapt, Pass::Main(format)].map(|pass| pipelines.specialize(&cache, &pipeline, pass));
        let (_, kept_view, copied) = kept.0.entry(entity).or_insert_with(|| {
            let made = device.create_texture(&TextureDescriptor { format: KEPT_FORMAT, ..texture((1, 1), "prism_kept") });
            let view = made.create_view(&TextureViewDescriptor::default());
            (made, view, false)
        });
        // The first frame drawn copies; until the pipelines are ready the
        // node draws nothing, and the copy waits for it.
        let first = !*copied;
        *copied |= ids.iter().all(|id| cache.get_render_pipeline(*id).is_some());
        commands.entity(entity).insert(PrismParts { chain, kept: kept_view.clone(), first, pipelines: ids });
    }
}

#[derive(Default)]
struct PrismNode;

impl ViewNode for PrismNode {
    type ViewQuery = (&'static ViewTarget, &'static PrismParts, &'static DynamicUniformIndex<PrismScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<PrismPipeline>();
        let cache = world.resource::<PipelineCache>();
        let ids = parts.pipelines.map(|id| cache.get_render_pipeline(id));
        let [Some(first), Some(halve), Some(copy), Some(adapt), Some(main)] = ids else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<PrismScreen>>().uniforms().binding() else { return Ok(()) };
        let layout = cache.get_bind_group_layout(&pipeline.pass);
        let offset = index.index();
        let pass = |context: &mut RenderContext, drawn: &RenderPipeline, from: &TextureView, into: &TextureView, load: bool, kept: Option<&BindGroup>| {
            let group = context.render_device().create_bind_group("prism_pass", &layout, &BindGroupEntries::sequential((from, &pipeline.sampler, uniform.clone())));
            let ops = Operations { load: if load { LoadOp::Load } else { LoadOp::Clear(Default::default()) }, store: StoreOp::Store };
            let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("prism"),
                color_attachments: &[Some(RenderPassColorAttachment { view: into, depth_slice: None, resolve_target: None, ops })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render.set_render_pipeline(drawn);
            render.set_bind_group(0, &group, &[offset]);
            if let Some(kept) = kept {
                render.set_bind_group(1, kept, &[]);
            }
            render.draw(0..3, 0..1);
        };
        let write = target.post_process_write();
        pass(context, first, write.source, &parts.chain[0].default_view, false, None);
        for pair in parts.chain.windows(2) {
            pass(context, halve, &pair[0].default_view, &pair[1].default_view, false, None);
        }
        // The first frame copies the brightness (`Graphics.Blit`): drawn by
        // the halving sums, whose four samples of a one-texel picture are
        // that texel; every later frame blends it in by the share.
        let last = &parts.chain[parts.chain.len() - 1].default_view;
        if parts.first {
            pass(context, copy, last, &parts.kept, false, None);
        } else {
            pass(context, adapt, last, &parts.kept, true, None);
        }
        let kept_group = context.render_device().create_bind_group("prism_kept", &cache.get_bind_group_layout(&pipeline.kept), &BindGroupEntries::sequential((&parts.kept,)));
        pass(context, main, write.source, write.destination, false, Some(&kept_group));
        Ok(())
    }
}

