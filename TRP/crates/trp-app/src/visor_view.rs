//! The face shield's mask over the first person's view (inventory spec, "The
//! face shield's mask"; `trp_sim::visor` works out which shield, its fade
//! and its swing): the camera's `VisorEffect` drawn on the frame by the
//! game's compiled shaders' sums (`visor.wgsl`), in the game camera's order
//! (after pain's blur, before the flash: `contusion_view`), on the frame's
//! gamma-encoded numbers as the game's gamma-space camera has them
//! (`gamma_ui`). Its figures and pictures are read from the install's
//! camera (`trp_data::visor`) away from the frame. Drawn in first person
//! only, as the game shows it (the hideout's `VisorVisibility` is its first
//! person's). With `FACE SHIELD MASK` off (or `VISUAL EFFECTS`) nothing of
//! it is read or drawn.

use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{CachedTexture, GpuImage, TextureCache};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use trp_data::visor::{MaskKind, VisorData};
use trp_sim::visor::{shield_in, Visor, VisorFigures};

const VISOR_SHADER: Handle<Shader> = uuid_handle!("6c1e9a47-2d83-4f5b-9e06-b48a3d7c1f92");

/// What the mask draws this frame (`on` 1 while it shows).
#[derive(Component, Clone, Copy, Default, ShaderType)]
pub struct VisorScreen {
    pub swing: Vec2,
    pub intensity: f32,
    pub edge_fade: f32,
    pub rotation: f32,
    pub hit_zoom: f32,
    pub scratches_intensity: f32,
    pub light: f32,
    pub distort_intensity: f32,
    pub blindness: f32,
    pub textured: f32,
    pub framed: f32,
    pub blur_steps: Vec4,
    pub rounds: f32,
    pub on: f32,
}

impl ExtractComponent for VisorScreen {
    type QueryData = &'static VisorScreen;
    type QueryFilter = ();
    type Out = VisorScreen;

    /// Only while it shows: below an intensity of 0.01 the game's effect
    /// copies the frame as it is.
    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (screen.on > 0.0).then_some(*screen)
    }
}

/// The flash's strength as it last ran (`_BlindnessCoef`, set by
/// `GrenadeFlashScreenEffect.Update` while it runs, and kept after: its
/// `OnDisable` does not put it back), which takes the scratches away.
#[derive(Resource, Default)]
pub struct Blindness(pub f32);

/// The mask's pictures on the card: the mask the shield's kind is drawn
/// with (none: no frame), the scratches, the blur's mask and the
/// distortion's.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct VisorImages {
    pub mask: Option<Handle<Image>>,
    pub scratches: Option<Handle<Image>>,
    pub blur_mask: Option<Handle<Image>>,
    pub distort_mask: Option<Handle<Image>>,
}

/// The mask once its figures have come: the camera's state, the figures,
/// each kind's mask picture, and what brings them.
#[derive(Resource, Default)]
pub struct VisorState {
    visor: Option<Visor>,
    data: Option<VisorData>,
    masks: Vec<(MaskKind, Option<Handle<Image>>)>,
    mask: Option<MaskKind>,
    coming: Option<Mutex<Receiver<VisorData>>>,
}

/// Starts reading the mask's figures and pictures away from the frame.
pub fn start_reading(app: &mut App, install: trp_data::Install) {
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::visor::read(&install) {
        Ok(read) => {
            let _ = send.send(read);
        }
        Err(e) => eprintln!("the face shield's mask could not be read; a face shield down shows nothing over the view: {e:#}"),
    });
    app.insert_resource(VisorState { coming: Some(Mutex::new(receive)), ..default() });
}

/// The mask's frame: its figures and pictures taken in as they come; the
/// shields of the head's and the face's slots heard (`trp_sim::visor`);
/// the filter and the swing moved on (the walk's step frequency, the
/// body's speed across the ground); and what the main camera draws, in
/// first person only.
#[allow(clippy::too_many_arguments)]
pub fn visor_frame(
    mut commands: Commands,
    mut state: ResMut<VisorState>,
    mut images: ResMut<VisorImages>,
    mut assets: ResMut<Assets<Image>>,
    carried: Res<crate::inventory_view::Carried>,
    controller: Res<crate::controller::Controller>,
    hands: Option<Res<crate::weapon::HandsRig>>,
    blindness: Res<Blindness>,
    view: Res<crate::View>,
    time: Res<Time>,
    cameras: Query<(Entity, Option<&VisorScreen>), With<crate::MainCamera>>,
) {
    let state = &mut *state;
    if let Some(coming) = state.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(data)) => {
                // Read as stored, as the game's gamma-space project reads
                // them (`gamma_ui`): no sRGB curve undone.
                let mut raw = |picture: &trp_data::unity::extract::TextureData, name: &str| {
                    crate::render_data::build_image(&trp_data::unity::extract::TextureData { srgb: false, ..picture.clone() }, crate::render_data::Role::Color)
                        .map_err(|e| warn!("the face shield's {name}: {e:#}"))
                        .ok()
                        .map(|image| assets.add(image))
                };
                images.scratches = raw(&data.scratches, "scratches");
                images.blur_mask = raw(&data.blur_mask, "blur mask");
                images.distort_mask = raw(&data.distort_mask, "distortion mask");
                state.masks = [MaskKind::None, MaskKind::Narrow, MaskKind::Wide].into_iter().map(|kind| (kind, data.mask_of(kind).and_then(|picture| raw(picture, "mask")))).collect();
                info!(
                    "the face shield's mask: scratches at {}, distortion {}, a blur of {} rounds; masks {:?}",
                    data.scratches_intensity,
                    data.distort_intensity,
                    data.blur_iterations,
                    state.masks.iter().map(|(kind, picture)| (*kind, picture.is_some())).collect::<Vec<_>>()
                );
                state.visor = Some(Visor::new(VisorFigures {
                    forward: data.forward.clone(),
                    backward: data.backward.clone(),
                    edge_fade: data.edge_fade.clone(),
                    dx: data.dx.clone(),
                    dy: data.dy.clone(),
                    walk_rotation: data.walk_rotation.clone(),
                }));
                state.data = Some(data);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => state.coming = Some(coming),
            _ => {}
        }
    }
    let (Some(visor), Some(data)) = (state.visor.as_mut(), state.data.as_ref()) else { return };
    let equipment = &carried.equipment;
    visor.observe(shield_in(equipment, "Headwear"), shield_in(equipment, "FaceCover"));
    let step_frequency = hands.as_ref().map_or(0.0, |rig| rig.hands.step_frequency);
    let [vx, _, vz] = controller.movement.velocity;
    let frame = visor.frame(time.delta_secs(), step_frequency, (vx * vx + vz * vz).sqrt());
    if state.mask != Some(frame.mask) {
        state.mask = Some(frame.mask);
        images.mask = state.masks.iter().find(|(kind, _)| *kind == frame.mask).and_then(|(_, picture)| picture.clone());
    }
    let mut shown = VisorScreen::default();
    if Visor::drawn(&frame) && *view == crate::View::FirstPerson {
        let n = 1.0 / (1u32 << data.downsample) as f32;
        let mut steps = [0.0; 4];
        for (round, step) in steps.iter_mut().enumerate().take(data.blur_iterations as usize) {
            *step = data.blur_size * n + round as f32;
        }
        shown = VisorScreen {
            swing: Vec2::from(frame.swing),
            intensity: frame.intensity,
            edge_fade: frame.edge_fade,
            rotation: frame.rotation,
            hit_zoom: 0.0,
            scratches_intensity: data.scratches_intensity,
            light: if data.affected_by_light { 1.0 } else { data.min_light },
            distort_intensity: data.distort_intensity,
            blindness: blindness.0,
            textured: if frame.mask == MaskKind::Narrow { 0.0 } else { 1.0 },
            framed: if images.mask.is_some() { 1.0 } else { 0.0 },
            blur_steps: Vec4::from(steps),
            rounds: data.blur_iterations.min(4) as f32,
            on: 1.0,
        };
    }
    for (camera, had) in &cameras {
        if had.is_none_or(|had| had.on != shown.on || shown.on > 0.0) {
            commands.entity(camera).insert(shown);
        }
    }
}

pub struct VisorViewPlugin;

impl Plugin for VisorViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, VISOR_SHADER, "visor.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<VisorScreen>::default(), UniformComponentPlugin::<VisorScreen>::default(), ExtractResourcePlugin::<VisorImages>::default()));
        app.init_resource::<VisorImages>().init_resource::<Blindness>();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<VisorPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<VisorNode>>(Core3d, VisorLabel)
            .add_render_graph_edges(Core3d, (crate::contusion_view::ContusionLabel, crate::sharpen_view::SharpenLabel, VisorLabel, crate::contusion_view::ContusionLateLabel));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct VisorLabel;

#[derive(Resource)]
struct VisorPipeline {
    layout: BindGroupLayoutDescriptor,
    clamped: Sampler,
    repeated: Sampler,
}

/// The passes, in the order `VisorParts` keeps their pipelines.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum Pass {
    Scratches,
    Distort,
    BlurDown,
    BlurVertical,
    BlurAcross,
    Framed,
}

const PASSES: [Pass; 6] = [Pass::Scratches, Pass::Distort, Pass::BlurDown, Pass::BlurVertical, Pass::BlurAcross, Pass::Framed];

impl SpecializedRenderPipeline for VisorPipeline {
    type Key = (TextureFormat, Pass);

    fn specialize(&self, (format, pass): Self::Key) -> RenderPipelineDescriptor {
        let entry = match pass {
            Pass::Scratches => "scratches",
            Pass::Distort => "distort",
            Pass::BlurDown => "blur_down",
            Pass::BlurVertical => "blur_vertical",
            Pass::BlurAcross => "blur_across",
            Pass::Framed => "framed",
        };
        RenderPipelineDescriptor {
            label: Some(format!("visor {entry}").into()),
            layout: vec![self.layout.clone()],
            vertex: VertexState { shader: VISOR_SHADER, entry_point: Some("vertex".into()), ..default() },
            fragment: Some(FragmentState {
                shader: VISOR_SHADER,
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let layout = BindGroupLayoutDescriptor::new(
        "visor",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (picture(), sampler(SamplerBindingType::Filtering), uniform_buffer::<VisorScreen>(true), picture(), picture(), picture(), picture(), picture(), sampler(SamplerBindingType::Filtering)),
        ),
    );
    // The frame's, the blur's and the distortion mask's: bilinear, clamped
    // at their edges (the distortion mask's own wrap); the mask's, the
    // scratches' and the blur mask's: bilinear, repeating (their wrap).
    let clamped = device.create_sampler(&SamplerDescriptor { label: Some("visor_clamped"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    let repeated = device.create_sampler(&SamplerDescriptor {
        label: Some("visor_repeated"),
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    commands.insert_resource(VisorPipeline { layout, clamped, repeated });
}

/// What a view with the mask needs this frame: its pipelines and the
/// blur's two half-size pictures (`source >> downsample`).
#[derive(Component)]
struct VisorParts {
    pipelines: [CachedRenderPipelineId; 6],
    half: [CachedTexture; 2],
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget), With<VisorScreen>>,
    done: Query<Entity, (With<VisorParts>, Without<VisorScreen>)>,
    device: Res<RenderDevice>,
    mut textures: ResMut<TextureCache>,
    cache: Res<PipelineCache>,
    pipeline: Res<VisorPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<VisorPipeline>>,
) {
    for entity in &done {
        commands.entity(entity).remove::<(VisorParts, DynamicUniformIndex<VisorScreen>)>();
    }
    for (entity, target) in &views {
        let format = target.main_texture_format();
        let size = target.main_texture().size();
        // `downsample` 1 as the install has it (`trp_data::visor`).
        let half = |label: &'static str| TextureDescriptor {
            label: Some(label),
            size: Extent3d { width: (size.width >> 1).max(1), height: (size.height >> 1).max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        commands.entity(entity).insert(VisorParts {
            pipelines: PASSES.map(|pass| pipelines.specialize(&cache, &pipeline, (format, pass))),
            half: [textures.get(&device, half("visor_half_a")), textures.get(&device, half("visor_half_b"))],
        });
    }
}

#[derive(Default)]
struct VisorNode;

impl ViewNode for VisorNode {
    type ViewQuery = (&'static ViewTarget, &'static VisorScreen, &'static VisorParts, &'static DynamicUniformIndex<VisorScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, screen, parts, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<VisorPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(uniform) = world.resource::<ComponentUniforms<VisorScreen>>().uniforms().binding() else { return Ok(()) };
        let found: Option<Vec<&RenderPipeline>> = parts.pipelines.iter().map(|id| cache.get_render_pipeline(*id)).collect();
        let Some(found) = found else { return Ok(()) };
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let images = world.resource::<VisorImages>();
        let view_of = |handle: &Option<Handle<Image>>| handle.as_ref().and_then(|handle| gpu.get(handle)).map(|image| &image.texture_view);
        let (Some(scratches), Some(blur_mask), Some(distort)) = (view_of(&images.scratches), view_of(&images.blur_mask), view_of(&images.distort_mask)) else { return Ok(()) };
        // A mask of none is not sampled (`framed` 0): any picture stands in.
        let mask = view_of(&images.mask).unwrap_or(scratches);
        let layout = cache.get_bind_group_layout(&pipeline.layout);
        let at = |pass: Pass| found[pass as usize];
        let draw = |context: &mut RenderContext, render: &RenderPipeline, from: &TextureView, soft: &TextureView, into: &TextureView, round: u32| {
            let group = context.render_device().create_bind_group(
                "visor",
                &layout,
                &BindGroupEntries::sequential((from, &pipeline.clamped, uniform.clone(), mask, scratches, blur_mask, distort, soft, &pipeline.repeated)),
            );
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("visor"),
                color_attachments: &[Some(RenderPassColorAttachment { view: into, depth_slice: None, resolve_target: None, ops: Operations::default() })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_render_pipeline(render);
            pass.set_bind_group(0, &group, &[index.index()]);
            pass.draw(0..3, round..round + 1);
        };
        let [a, b] = [&parts.half[0].default_view, &parts.half[1].default_view];
        // Pass 0, the scratches; pass 1 (the cracks') has nothing to move.
        let write = target.post_process_write();
        draw(context, at(Pass::Scratches), write.source, a, write.destination, 0);
        // Pass 2, the distortion.
        let write = target.post_process_write();
        draw(context, at(Pass::Distort), write.source, a, write.destination, 0);
        // The fast blur of that (`Modify`): down to a half, then each round
        // up and down and across, by its step.
        // (The blurred picture's place is not read by these: it takes one
        // that is not the pass's target.)
        draw(context, at(Pass::BlurDown), target.main_texture_view(), b, a, 0);
        for round in 0..screen.rounds as u32 {
            draw(context, at(Pass::BlurVertical), a, a, b, round);
            draw(context, at(Pass::BlurAcross), b, b, a, round);
        }
        // Pass 3, the blur and the frame's mask.
        let write = target.post_process_write();
        draw(context, at(Pass::Framed), write.source, a, write.destination, 0);
        Ok(())
    }
}
