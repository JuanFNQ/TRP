//! Blood on the screen (health spec, "Blood on the screen";
//! `trp_sim::blood` works out the drops and when it shows): a hit's and a
//! bleeding's drops drawn into a drops' picture, blurred without glasses,
//! laid into a distortion map that fades a step a frame, and the frame seen
//! through it toward the blood's colour, with a heavy bleeding's red pulse;
//! each pass by the game's compiled shader's sums (`blood.wgsl`), right
//! after a hit's blur as on the game's camera. The drops' picture and the
//! map are kept from frame to frame per view, the map in two pictures in
//! turn (the game copies it back through a scratch picture each time: the
//! same result, a copy fewer). Runs with `HEALTH SYSTEM` on.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{CachedTexture, GpuImage, TextureCache};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use trp_sim::blood::{Blood, DROP_BLUR_PIXELS};
use trp_sim::health::Part;

const BLOOD_SHADER: Handle<Shader> = uuid_handle!("5d1f8a63-2b47-4c9e-8e05-c3a7b91d6f24");

/// The most drops drawn in a frame (a bleeding's first: one to seven).
const DRAWN: usize = 8;
/// The squares the frame is blurred in (`BloodOnScreen`'s `_tmpHalfRT`,
/// `_tmpQuarterRT`).
const HALF: u32 = 512;
const QUARTER: u32 = 256;
/// The maps' format: the game's render textures' `ARGB32`, 8-bit
/// (`PROVISIONAL`: linear, its colour space not read).
const MAP_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// What the blood draws this frame (`on` 1 while it shows).
#[derive(Component, Clone, Copy, ShaderType)]
pub struct BloodScreen {
    pub drops: [Vec4; DRAWN],
    pub count: f32,
    pub aspect: f32,
    pub fade: f32,
    pub step: f32,
    pub refraction: f32,
    pub blurred: f32,
    pub heavy: f32,
    pub on: f32,
    pub side: f32,
    /// The frame's blur's passes down and back (`DownsamplingCount`).
    pub passes: f32,
    pub input_min: Vec4,
    pub input_max: Vec4,
    pub input_gamma: Vec4,
    pub output_min: Vec4,
    pub output_max: Vec4,
    pub color: Vec4,
    pub vignette: Vec4,
}

impl Default for BloodScreen {
    fn default() -> Self {
        Self {
            drops: [Vec4::ZERO; DRAWN],
            count: 0.0,
            aspect: 16.0 / 9.0,
            fade: 0.0,
            step: 0.0,
            refraction: 0.0,
            blurred: 0.0,
            heavy: 0.0,
            on: 0.0,
            side: 0.0,
            passes: 0.0,
            input_min: Vec4::ZERO,
            input_max: Vec4::ONE,
            input_gamma: Vec4::ONE,
            output_min: Vec4::ZERO,
            output_max: Vec4::ONE,
            color: Vec4::ZERO,
            vignette: Vec4::ZERO,
        }
    }
}

impl ExtractComponent for BloodScreen {
    type QueryData = &'static BloodScreen;
    type QueryFilter = ();
    type Out = BloodScreen;

    /// Only while it shows: with none the picture goes through untouched,
    /// and the maps wait as they are.
    fn extract_component(blood: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (blood.on > 0.0).then_some(*blood)
    }
}

/// The blood once its figures are read, the bleeding centres, and what the
/// head's slots held when last looked at.
#[derive(Resource, Default)]
pub struct BloodState {
    pub blood: Option<Blood>,
    pub centres: Vec<[f32; 3]>,
    head: Option<Vec<trp_sim::inventory::Id>>,
}

impl BloodState {
    /// A part's bleeding centre (`EFTHardSettings.BleedingCenters`).
    pub fn centre(&self, part: Part) -> [f32; 3] {
        self.centres.get(part as usize).copied().unwrap_or([0.0, 0.0, -1.0])
    }
}

/// The blood's frame: the glasses when the head's slots change, each
/// bleeding's blood (`OnHealthApplyDamage`), then what it draws.
pub fn blood_frame(
    time: Res<Time>,
    mut controller: ResMut<crate::controller::Controller>,
    carried: Res<crate::inventory_view::Carried>,
    mut state: ResMut<BloodState>,
    mut cameras: Query<(&Camera, &mut BloodScreen)>,
    frames: Res<bevy::diagnostic::FrameCount>,
) {
    let bled = std::mem::take(&mut controller.bled);
    let now = time.elapsed_secs();
    let state = &mut *state;
    // `OnItemAddedOrRemoved` for the head's slots: the glasses told again;
    // nothing at the start.
    let head = trp_sim::inventory::head_things(&carried.equipment);
    let changed = state.head.as_ref().is_some_and(|before| *before != head);
    state.head = Some(head);
    let centres = state.centres.clone();
    let Some(blood) = state.blood.as_mut() else { return };
    if changed {
        blood.set_glasses(trp_sim::inventory::has_glasses(&carried.equipment));
    }
    for (part, heavy) in bled {
        let centre = centres.get(part as usize).copied().unwrap_or([0.0, 0.0, -1.0]);
        let direction = blood.about(centre, 1.0 / 3.0);
        let drawn = blood.bleed(direction, heavy, now);
        info!("blood on the screen: the {} bleeding{}: {} (frame {})", part.name(), if heavy { " (heavy)" } else { "" }, if drawn { "drops placed" } else { "locked, none" }, frames.0);
    }
    let frame = blood.frame(time.delta_secs(), now);
    let f = blood.figures().data.clone();
    for (camera, mut screen) in &mut cameras {
        let mut shown = BloodScreen { on: if frame.on { 1.0 } else { 0.0 }, ..default() };
        if frame.on {
            for (n, drop) in frame.drops.iter().take(DRAWN).enumerate() {
                shown.drops[n] = Vec4::new(drop.place[0], drop.place[1], drop.scale, drop.life);
            }
            let side = (1u32 << f.dimension.min(13)) as f32;
            shown.count = frame.drops.len().min(DRAWN) as f32;
            shown.aspect = camera.logical_viewport_size().map_or(16.0 / 9.0, |size| size.x / size.y.max(1.0));
            shown.fade = frame.fade;
            shown.step = DROP_BLUR_PIXELS / side;
            shown.refraction = f.refraction;
            shown.blurred = if frame.blurred { 1.0 } else { 0.0 };
            shown.heavy = if frame.heavy.is_some() { 1.0 } else { 0.0 };
            shown.side = side;
            shown.passes = f.downsampling as f32;
            [shown.input_min, shown.input_max, shown.input_gamma, shown.output_min, shown.output_max] = f.levels.map(Vec4::from);
            shown.color = Vec4::from(f.color);
            shown.vignette = Vec4::new(f.center[0], f.center[1], f.sharpness * 0.01, frame.heavy.unwrap_or(0.0) * 0.02);
        }
        *screen = shown;
    }
}

pub struct BloodViewPlugin;

impl Plugin for BloodViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, BLOOD_SHADER, "blood.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<BloodScreen>::default(), UniformComponentPlugin::<BloodScreen>::default()));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<BloodPipeline>>()
            .init_resource::<BloodMaps>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<BloodNode>>(Core3d, BloodLabel)
            .add_render_graph_edges(Core3d, (crate::screen_blur::HitBlurLabel, BloodLabel, crate::contusion_view::ContusionLabel));
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct BloodLabel;

#[derive(Resource)]
struct BloodPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    fullscreen: FullscreenShader,
}

/// The passes, in the order `BloodParts` keeps their pipelines (`as
/// usize`).
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum Pass {
    Drop,
    BlurAcross,
    BlurDown,
    Blit,
    Countdown,
    Copy,
    Blood,
    Pulse,
}

const PASSES: [Pass; 8] = [Pass::Drop, Pass::BlurAcross, Pass::BlurDown, Pass::Blit, Pass::Countdown, Pass::Copy, Pass::Blood, Pass::Pulse];

impl SpecializedRenderPipeline for BloodPipeline {
    type Key = (TextureFormat, Pass);

    fn specialize(&self, (format, pass): Self::Key) -> RenderPipelineDescriptor {
        let (entry, on_map) = match pass {
            Pass::Drop => ("drop_fragment", true),
            Pass::BlurAcross => ("blur_across", true),
            Pass::BlurDown => ("blur_down", true),
            Pass::Blit => ("blit", true),
            Pass::Countdown => ("countdown", true),
            Pass::Copy => ("copy", false),
            Pass::Blood => ("blood_on_screen", false),
            Pass::Pulse => ("heavy_pulse", false),
        };
        // The drops added (`Blend One One`); the rest replace.
        let blend = (pass == Pass::Drop).then_some(BlendState {
            color: BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
            alpha: BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
        });
        let vertex = if pass == Pass::Drop {
            VertexState { shader: BLOOD_SHADER, entry_point: Some("drop_vertex".into()), ..default() }
        } else {
            self.fullscreen.to_vertex_state()
        };
        RenderPipelineDescriptor {
            label: Some(format!("blood {entry}").into()),
            layout: vec![self.layout.clone()],
            vertex,
            fragment: Some(FragmentState {
                shader: BLOOD_SHADER,
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState { format: if on_map { MAP_FORMAT } else { format }, blend, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let layout = BindGroupLayoutDescriptor::new(
        "blood",
        &BindGroupLayoutEntries::sequential(ShaderStages::VERTEX_FRAGMENT, (picture(), sampler(SamplerBindingType::Filtering), uniform_buffer::<BloodScreen>(true), picture(), picture())),
    );
    // Bilinear, clamped at the edges.
    let sampler = device.create_sampler(&SamplerDescriptor { label: Some("blood"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(BloodPipeline { layout, sampler, fullscreen: fullscreen.clone() });
}

/// A picture kept from frame to frame.
struct Kept {
    _texture: Texture,
    view: TextureView,
}

/// Each view's drops' picture, its blur's scratch and its two distortion
/// maps with the one current (`_dropsTex`, `_onBloodImageRenderedTmpRT`,
/// `_duDvMap`; the node turns which is current as it draws), and their
/// side.
struct Maps {
    drops: Kept,
    scratch: Kept,
    maps: [Kept; 2],
    current: std::sync::atomic::AtomicUsize,
    side: u32,
}

#[derive(Resource, Default)]
struct BloodMaps(HashMap<Entity, Maps>);

/// What a view with blood needs this frame: its squares of the frame and
/// the pipelines.
#[derive(Component)]
struct BloodParts {
    half: CachedTexture,
    quarter: CachedTexture,
    out: CachedTexture,
    pipelines: [CachedRenderPipelineId; 8],
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, &BloodScreen)>,
    done: Query<Entity, (With<BloodParts>, Without<BloodScreen>)>,
    device: Res<RenderDevice>,
    mut textures: ResMut<TextureCache>,
    cache: Res<PipelineCache>,
    pipeline: Res<BloodPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<BloodPipeline>>,
    mut maps: ResMut<BloodMaps>,
) {
    for entity in &done {
        commands.entity(entity).remove::<(BloodParts, DynamicUniformIndex<BloodScreen>)>();
    }
    let make = |side: u32, format: TextureFormat, label: &'static str| TextureDescriptor {
        label: Some(label),
        size: Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };
    for (entity, target, blood) in &views {
        let side = (blood.side as u32).max(1);
        if maps.0.get(&entity).is_none_or(|kept| kept.side != side) {
            let kept = |label: &'static str| {
                let texture = device.create_texture(&make(side, MAP_FORMAT, label));
                let view = texture.create_view(&TextureViewDescriptor::default());
                Kept { _texture: texture, view }
            };
            maps.0.insert(entity, Maps { drops: kept("blood_drops"), scratch: kept("blood_scratch"), maps: [kept("blood_map_a"), kept("blood_map_b")], current: 0.into(), side });
        }
        let format = target.main_texture_format();
        commands.entity(entity).insert(BloodParts {
            half: textures.get(&device, make(HALF, format, "blood_half")),
            quarter: textures.get(&device, make(QUARTER, format, "blood_quarter")),
            out: textures.get(&device, make(side, format, "blood_out")),
            pipelines: PASSES.map(|pass| pipelines.specialize(&cache, &pipeline, (format, pass))),
        });
    }
}

#[derive(Default)]
struct BloodNode;

impl ViewNode for BloodNode {
    type ViewQuery = (Entity, &'static ViewTarget, &'static BloodScreen, &'static BloodParts, &'static DynamicUniformIndex<BloodScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (entity, target, blood, parts, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<BloodPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(uniform) = world.resource::<ComponentUniforms<BloodScreen>>().uniforms().binding() else { return Ok(()) };
        let found: Option<Vec<&RenderPipeline>> = parts.pipelines.iter().map(|id| cache.get_render_pipeline(*id)).collect();
        let Some(found) = found else { return Ok(()) };
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let images = world.resource::<crate::contusion_view::ScreenImages>();
        let Some(drop) = images.blood_drop.as_ref().and_then(|handle| gpu.get(handle)) else { return Ok(()) };
        let maps = world.resource::<BloodMaps>();
        let Some(kept) = maps.0.get(&entity) else { return Ok(()) };
        let layout = cache.get_bind_group_layout(&pipeline.layout);
        let pass = |context: &mut RenderContext, render: &RenderPipeline, textures: [&TextureView; 3], into: &TextureView, clear: bool, vertices: u32, instances: u32| {
            let group = context.render_device().create_bind_group("blood", &layout, &BindGroupEntries::sequential((textures[0], &pipeline.sampler, uniform.clone(), textures[1], textures[2])));
            let ops = if clear { Operations { load: LoadOp::Clear(LinearRgba::BLACK.into()), store: StoreOp::Store } } else { Operations::default() };
            let mut render_pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("blood"),
                color_attachments: &[Some(RenderPassColorAttachment { view: into, depth_slice: None, resolve_target: None, ops })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_render_pipeline(render);
            render_pass.set_bind_group(0, &group, &[index.index()]);
            render_pass.draw(0..vertices, 0..instances);
        };
        let filler = &drop.texture_view;
        let at = |pass: Pass| found[pass as usize];
        let mut current = kept.current.load(std::sync::atomic::Ordering::Relaxed);
        // `BloodDropsController.Update`: the drops drawn on black, blurred
        // in place without glasses, then laid into the map.
        if blood.count > 0.0 {
            pass(context, at(Pass::Drop), [filler, filler, filler], &kept.drops.view, true, 6, blood.count as u32);
            if blood.blurred > 0.0 {
                pass(context, at(Pass::BlurAcross), [&kept.drops.view, filler, filler], &kept.scratch.view, false, 3, 1);
                pass(context, at(Pass::BlurDown), [&kept.scratch.view, filler, filler], &kept.drops.view, false, 3, 1);
            }
            pass(context, at(Pass::Blit), [&kept.drops.view, &kept.maps[current].view, filler], &kept.maps[1 - current].view, false, 3, 1);
            current = 1 - current;
        }
        // `FadeDrops`.
        pass(context, at(Pass::Countdown), [&kept.maps[current].view, filler, filler], &kept.maps[1 - current].view, false, 3, 1);
        current = 1 - current;
        kept.current.store(current, std::sync::atomic::Ordering::Relaxed);
        // The frame blurred in its squares, seen through the map into the
        // large square, then onto the frame with the pulse or without.
        let main = target.main_texture_view();
        pass(context, at(Pass::Copy), [main, filler, filler], &parts.half.default_view, false, 3, 1);
        for _ in 0..blood.passes as u32 {
            pass(context, at(Pass::Copy), [&parts.half.default_view, filler, filler], &parts.quarter.default_view, false, 3, 1);
            pass(context, at(Pass::Copy), [&parts.quarter.default_view, filler, filler], &parts.half.default_view, false, 3, 1);
        }
        pass(context, at(Pass::Blood), [main, &kept.maps[current].view, &parts.half.default_view], &parts.out.default_view, false, 3, 1);
        let write = target.post_process_write();
        let last = if blood.heavy > 0.0 { at(Pass::Pulse) } else { at(Pass::Copy) };
        pass(context, last, [&parts.out.default_view, filler, filler], write.destination, false, 3, 1);
        Ok(())
    }
}
