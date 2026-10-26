//! The game's ambient occlusion with the options' `AMBIENT OCCLUSION` on
//! (`docs/spec/ambient-occlusion.md`): the camera's `HBAO` at the install's
//! own choice of the `SSAO` setting (`Graphics.ini`, or `--ssao`), drawn as
//! the game's in four passes over the frame: the occlusion, the blur across
//! and down, and the composite. It runs right after the scene's depth is
//! copied, on the frame in the game's numbers (`gamma_ui`), before the
//! sky's scattering and the world's effects, as the game's opaque image
//! effect runs after its opaque scene, before its `TOD_Scattering` and
//! before anything transparent: everything Bevy draws is opaque.
//!
//! The normals are Bevy's normal prepass (the camera's `NormalPrepass`,
//! world normals with their normal maps in ten bits, as the game's G-buffer
//! keeps them); the depth is the scene's copied (`muzzle_view`). Its
//! textures are the game's kind: eight bits a channel, the depth in two
//! bytes, read bilinearly. The drawing alone: nothing of the simulation
//! changes.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_2d_multisampled, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::texture::{CachedTexture, TextureCache};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, ShaderDefVal};
use trp_sim::hbao::{self, HbaoSettings};

const HBAO_SHADER: Handle<Shader> = uuid_handle!("8d3f6a27-41c9-4e5b-b7a0-6c92e1f4d358");

/// The textures' kind: the engine's default render texture, four bytes.
const STORED: TextureFormat = TextureFormat::Rgba8Unorm;

/// On the camera: the preset it draws with.
#[derive(Component, Clone, Copy)]
pub struct Hbao(pub HbaoSettings);

/// On the camera: the passes' figures this frame (`Hbao` in `hbao.wgsl`).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct HbaoScreen {
    pub view_from_world: Mat4,
    pub view_from_clip: Mat4,
    pub size: Vec4,
    pub across_size: Vec4,
    pub uv_to_view: Vec4,
    pub reach: Vec4,
    pub distance: Vec4,
    pub march: Vec4,
    pub blur: Vec4,
    pub composite: Vec4,
    pub turns: [Vec4; 8],
}

impl ExtractComponent for HbaoScreen {
    type QueryData = &'static HbaoScreen;
    type QueryFilter = ();
    type Out = HbaoScreen;

    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*screen)
    }
}

/// What the render world makes once: the noise of the preset's quality,
/// texels from the bottom row, and whether the blur's first pass is at half
/// size and how far it reaches.
#[derive(Component, Clone, Copy)]
pub struct HbaoMaking {
    noise: [[u8; 4]; 16],
    half: bool,
    blur: bool,
    /// The occlusion shown alone, as the game's display mode `AOOnly`
    /// shows it (`--ssao-display ao`).
    ao_only: bool,
}

impl ExtractComponent for HbaoMaking {
    type QueryData = &'static HbaoMaking;
    type QueryFilter = ();
    type Out = HbaoMaking;

    fn extract_component(making: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(*making)
    }
}

/// The camera's parts for a preset: the preset, its figures (filled each
/// frame) and what the render world makes of it. The camera needs the
/// normal prepass besides (`main.rs`).
pub fn camera_parts(settings: HbaoSettings, ao_only: bool) -> (Hbao, HbaoScreen, HbaoMaking) {
    let making = HbaoMaking { noise: hbao::noise(settings.quality), half: settings.blur_downsample, blur: settings.blur.reach() > 0, ao_only };
    (Hbao(settings), HbaoScreen::default(), making)
}

/// The view's size, its projection and its place this frame.
fn follow(mut cameras: Query<(&Camera, &GlobalTransform, &Hbao, &mut HbaoScreen)>) {
    for (camera, eye, Hbao(settings), mut screen) in &mut cameras {
        let Some(size) = camera.physical_viewport_size() else { continue };
        let clip_from_view = camera.clip_from_view();
        let tan_half_fov = 1.0 / clip_from_view.y_axis.y;
        let aspect = clip_from_view.y_axis.y / clip_from_view.x_axis.x;
        let figures = hbao::Figures::of(settings, size.y, tan_half_fov);
        let (width, height) = (size.x as f32, size.y as f32);
        let across = if settings.blur_downsample { Vec2::new((size.x / 2) as f32, (size.y / 2) as f32) } else { Vec2::new(width, height) };
        let (directions, steps) = (settings.quality.directions(), settings.quality.steps());
        let mut turns = [Vec4::ZERO; 8];
        for (turn, [cos, sin]) in turns.iter_mut().zip(hbao::turns(settings.quality)) {
            *turn = Vec4::new(cos, sin, 0.0, 0.0);
        }
        *screen = HbaoScreen {
            view_from_world: eye.to_matrix().inverse(),
            view_from_clip: clip_from_view.inverse(),
            size: Vec4::new(width, height, 1.0 / width, 1.0 / height),
            across_size: Vec4::new(across.x, across.y, 1.0 / across.x, 1.0 / across.y),
            uv_to_view: Vec4::from_array(hbao::uv_to_view(tan_half_fov, aspect)),
            reach: Vec4::new(figures.radius, figures.max_radius_pixels, figures.neg_inv_radius2, figures.angle_bias),
            distance: Vec4::new(figures.ao_multiplier, figures.max_distance, figures.distance_falloff, hbao::FAR_PLANE),
            march: Vec4::new(directions as f32, steps as f32, 1.0 / (steps + 1) as f32, 1.0 / (directions * steps) as f32),
            blur: Vec4::new(settings.blur.reach() as f32, settings.blur.falloff(), settings.sharpness, 0.0),
            composite: Vec4::new(settings.intensity, settings.base_color[0], settings.base_color[1], settings.base_color[2]),
            turns,
        };
    }
}

pub struct HbaoPlugin;

impl Plugin for HbaoPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, HBAO_SHADER, "hbao.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<HbaoScreen>::default(), ExtractComponentPlugin::<HbaoMaking>::default(), UniformComponentPlugin::<HbaoScreen>::default()))
            .add_systems(PostUpdate, follow.after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<HbaoPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<HbaoNode>>(Core3d, HbaoLabel)
            .add_render_graph_edges(Core3d, (crate::muzzle_view::DepthCopyLabel, HbaoLabel, crate::scattering_view::ScatteringLabel));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct HbaoLabel;

#[derive(Resource)]
struct HbaoPipeline {
    /// The figures (group 0).
    figures: BindGroupLayoutDescriptor,
    /// The occlusion's: the depth, the normals (as one sample or many),
    /// the noise.
    occlusion: [BindGroupLayoutDescriptor; 2],
    /// The blur's: the texture read and its sampler.
    blur: BindGroupLayoutDescriptor,
    /// The composite's: the frame, the occlusion, its sampler.
    composite: BindGroupLayoutDescriptor,
    /// Bilinear and clamped, as the game's render textures are.
    bilinear: Sampler,
    fullscreen: FullscreenShader,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum HbaoKey {
    Occlusion { multisampled: bool },
    BlurAcross,
    BlurDown,
    Composite { format: TextureFormat, ao_only: bool },
}

impl SpecializedRenderPipeline for HbaoPipeline {
    type Key = HbaoKey;

    fn specialize(&self, key: HbaoKey) -> RenderPipelineDescriptor {
        let (label, entry, part, layout, format, mut defs): (&str, &str, &str, &BindGroupLayoutDescriptor, TextureFormat, Vec<ShaderDefVal>) = match key {
            HbaoKey::Occlusion { multisampled } => ("hbao occlusion", "occlusion", "OCCLUSION", &self.occlusion[usize::from(multisampled)], STORED, if multisampled { vec!["MULTISAMPLED".into()] } else { vec![] }),
            HbaoKey::BlurAcross => ("hbao blur across", "blur_across", "BLUR", &self.blur, STORED, vec![]),
            HbaoKey::BlurDown => ("hbao blur down", "blur_down", "BLUR", &self.blur, STORED, vec![]),
            HbaoKey::Composite { format, ao_only } => ("hbao composite", "composite", "COMPOSITE", &self.composite, format, if ao_only { vec!["AO_ONLY".into()] } else { vec![] }),
        };
        defs.push(part.into());
        RenderPipelineDescriptor {
            label: Some(label.into()),
            layout: vec![self.figures.clone(), layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: HBAO_SHADER,
                shader_defs: defs,
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let unfiltered = || texture_2d(TextureSampleType::Float { filterable: false });
    let filtered = || texture_2d(TextureSampleType::Float { filterable: true });
    let figures = BindGroupLayoutDescriptor::new("hbao_figures", &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, uniform_buffer::<HbaoScreen>(true)));
    let occlusion = [
        BindGroupLayoutDescriptor::new("hbao_occlusion", &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (unfiltered(), unfiltered(), unfiltered()))),
        BindGroupLayoutDescriptor::new(
            "hbao_occlusion_multisampled",
            &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (unfiltered(), texture_2d_multisampled(TextureSampleType::Float { filterable: false }), unfiltered())),
        ),
    ];
    let blur = BindGroupLayoutDescriptor::new("hbao_blur", &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (filtered(), sampler(SamplerBindingType::Filtering))));
    let composite = BindGroupLayoutDescriptor::new("hbao_composite", &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (unfiltered(), filtered(), sampler(SamplerBindingType::Filtering))));
    let bilinear = device.create_sampler(&SamplerDescriptor { label: Some("hbao_bilinear"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(HbaoPipeline { figures, occlusion, blur, composite, bilinear, fullscreen: fullscreen.clone() });
}

/// The noise on the card, made once.
#[derive(Resource)]
struct NoiseOnCard(TextureView);

/// A view's textures and pipelines this frame.
#[derive(Component)]
struct HbaoParts {
    /// The occlusion, at the frame's size.
    occlusion: CachedTexture,
    /// The blur's first pass, at half the size or the same.
    across: CachedTexture,
    blur: bool,
    /// The occlusion's pass for normals of one sample and of many: chosen
    /// as it draws by the normals then in use, which a change of the
    /// camera's multisampling makes anew within the frame.
    occlude: [CachedRenderPipelineId; 2],
    blur_across: CachedRenderPipelineId,
    blur_down: CachedRenderPipelineId,
    composite: CachedRenderPipelineId,
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, &HbaoMaking, &ViewPrepassTextures), With<HbaoScreen>>,
    noise: Option<Res<NoiseOnCard>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut textures: ResMut<TextureCache>,
    cache: Res<PipelineCache>,
    pipeline: Res<HbaoPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<HbaoPipeline>>,
) {
    for (entity, target, making, prepass) in &views {
        let Some(normals) = &prepass.normal else { continue };
        if noise.is_none() {
            let texture = device.create_texture_with_data(
                &queue,
                &TextureDescriptor {
                    label: Some("hbao_noise"),
                    size: Extent3d { width: 4, height: 4, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: STORED,
                    usage: TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                TextureDataOrder::LayerMajor,
                making.noise.as_flattened(),
            );
            commands.insert_resource(NoiseOnCard(texture.create_view(&TextureViewDescriptor::default())));
        }
        let size = target.main_texture().size();
        let stored = |label: &'static str, width: u32, height: u32, textures: &mut TextureCache| {
            textures.get(
                &device,
                TextureDescriptor {
                    label: Some(label),
                    size: Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: STORED,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        };
        let occlusion = stored("hbao_occlusion", size.width, size.height, &mut textures);
        let across = if making.half { stored("hbao_across", size.width / 2, size.height / 2, &mut textures) } else { stored("hbao_across", size.width, size.height, &mut textures) };
        let _ = normals;
        commands.entity(entity).insert(HbaoParts {
            occlusion,
            across,
            blur: making.blur,
            occlude: [false, true].map(|multisampled| pipelines.specialize(&cache, &pipeline, HbaoKey::Occlusion { multisampled })),
            blur_across: pipelines.specialize(&cache, &pipeline, HbaoKey::BlurAcross),
            blur_down: pipelines.specialize(&cache, &pipeline, HbaoKey::BlurDown),
            composite: pipelines.specialize(&cache, &pipeline, HbaoKey::Composite { format: target.main_texture_format(), ao_only: making.ao_only }),
        });
    }
}

#[derive(Default)]
struct HbaoNode;

impl ViewNode for HbaoNode {
    type ViewQuery = (&'static ViewTarget, &'static HbaoParts, &'static crate::muzzle_view::JetParts, &'static ViewPrepassTextures, &'static DynamicUniformIndex<HbaoScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, jets, prepass, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<HbaoPipeline>();
        let cache = world.resource::<PipelineCache>();
        let multisampled = prepass.normal.as_ref().is_some_and(|n| n.texture.texture.sample_count() > 1);
        let (Some(occlude), Some(blur_across), Some(blur_down), Some(composite)) = (
            cache.get_render_pipeline(parts.occlude[usize::from(multisampled)]),
            cache.get_render_pipeline(parts.blur_across),
            cache.get_render_pipeline(parts.blur_down),
            cache.get_render_pipeline(parts.composite),
        ) else {
            return Ok(());
        };
        let (Some(uniform), Some(noise), Some(normals)) = (world.resource::<ComponentUniforms<HbaoScreen>>().uniforms().binding(), world.get_resource::<NoiseOnCard>(), prepass.normal_view()) else {
            return Ok(());
        };
        let device = context.render_device().clone();
        let figures = device.create_bind_group("hbao_figures", &cache.get_bind_group_layout(&pipeline.figures), &BindGroupEntries::single(uniform));
        let pass = |context: &mut RenderContext, label: &'static str, into: &TextureView, drawn: &RenderPipeline, group: &BindGroup| {
            let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(RenderPassColorAttachment { view: into, depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Clear(wgpu_types::Color::WHITE), store: StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render.set_render_pipeline(drawn);
            render.set_bind_group(0, &figures, &[index.index()]);
            render.set_bind_group(1, group, &[]);
            render.draw(0..3, 0..1);
        };

        // 1. The occlusion, into a texture cleared to white.
        let group = device.create_bind_group(
            "hbao_occlusion",
            &cache.get_bind_group_layout(&pipeline.occlusion[usize::from(multisampled)]),
            &BindGroupEntries::sequential((&jets.depth.default_view, normals, &noise.0)),
        );
        pass(context, "hbao occlusion", &parts.occlusion.default_view, occlude, &group);
        // 2 and 3. The blur across, then down back into the first.
        if parts.blur {
            let layout = cache.get_bind_group_layout(&pipeline.blur);
            let group = device.create_bind_group("hbao_blur_across", &layout, &BindGroupEntries::sequential((&parts.occlusion.default_view, &pipeline.bilinear)));
            pass(context, "hbao blur across", &parts.across.default_view, blur_across, &group);
            let group = device.create_bind_group("hbao_blur_down", &layout, &BindGroupEntries::sequential((&parts.across.default_view, &pipeline.bilinear)));
            pass(context, "hbao blur down", &parts.occlusion.default_view, blur_down, &group);
        }
        // 4. The frame multiplied by it.
        let write = target.post_process_write();
        let group = device.create_bind_group(
            "hbao_composite",
            &cache.get_bind_group_layout(&pipeline.composite),
            &BindGroupEntries::sequential((write.source, &parts.occlusion.default_view, &pipeline.bilinear)),
        );
        pass(context, "hbao composite", write.destination, composite, &group);
        Ok(())
    }
}
