//! The visor and crack pipelines, their parts prepared, and the view node.

use super::*;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct VisorLabel;

#[derive(Resource)]
pub(super) struct VisorPipeline {
    layout: BindGroupLayoutDescriptor,
    clamped: Sampler,
    repeated: Sampler,
}

/// The passes, in the order `VisorParts` keeps their pipelines.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub(super) enum Pass {
    Scratches,
    Cracks,
    Distort,
    BlurDown,
    BlurVertical,
    BlurAcross,
    Framed,
}

const PASSES: [Pass; 7] = [Pass::Scratches, Pass::Cracks, Pass::Distort, Pass::BlurDown, Pass::BlurVertical, Pass::BlurAcross, Pass::Framed];

/// The cracks' picture's format (`GetOrCreateGlassRenderTexture`: `R8`).
const CRACK_FORMAT: TextureFormat = TextureFormat::R8Unorm;

/// The cracks' quads' pipeline (`Custom/Glass Scratch`): the crack picture,
/// its sampler (bilinear, clamped at its edges: the picture's own), the
/// quads; added into the one channel (`Blend One One`), no faces culled.
#[derive(Resource)]
pub(super) struct CrackPipeline {
    layout: BindGroupLayoutDescriptor,
    pub(super) sampler: Sampler,
}

impl SpecializedRenderPipeline for CrackPipeline {
    type Key = ();

    fn specialize(&self, _: ()) -> RenderPipelineDescriptor {
        let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
        RenderPipelineDescriptor {
            label: Some("visor cracks".into()),
            layout: vec![self.layout.clone()],
            vertex: VertexState { shader: CRACKS_SHADER, entry_point: Some("vertex".into()), ..default() },
            fragment: Some(FragmentState {
                shader: CRACKS_SHADER,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState { format: CRACK_FORMAT, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

/// The cracks' picture on the card: one channel, no levels, of the view's
/// size (`PROVISIONAL`: the game's is the display's, `Screen.
/// currentResolution`; only the cracks' size in pixels differs), made anew
/// when that changes; a black one of a pixel bound while there is none
/// (the shader's own black, nothing); the quads' pipeline; and the drawing
/// it holds (`CrackScreen::drawings`).
#[derive(Resource)]
pub(super) struct CrackTarget {
    black: TextureView,
    target: Option<(Texture, TextureView, UVec2)>,
    pipeline: Option<CachedRenderPipelineId>,
    pub(super) drawn: Mutex<Option<u64>>,
}

impl SpecializedRenderPipeline for VisorPipeline {
    type Key = (TextureFormat, Pass);

    fn specialize(&self, (format, pass): Self::Key) -> RenderPipelineDescriptor {
        let entry = match pass {
            Pass::Scratches => "scratches",
            Pass::Cracks => "cracks",
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

pub(super) fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let layout = BindGroupLayoutDescriptor::new(
        "visor",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (picture(), sampler(SamplerBindingType::Filtering), uniform_buffer::<VisorScreen>(true), picture(), picture(), picture(), picture(), picture(), sampler(SamplerBindingType::Filtering), picture()),
        ),
    );
    // The cracks': the picture, its sampler and the quads (two four-number
    // rows a crack, twenty cracks).
    let crack_layout = BindGroupLayoutDescriptor::new(
        "visor_cracks",
        &BindGroupLayoutEntries::sequential(ShaderStages::VERTEX_FRAGMENT, (picture(), sampler(SamplerBindingType::Filtering), uniform_buffer_sized(false, std::num::NonZeroU64::new(QUADS_BYTES)))),
    );
    // The crack picture's own sampling (`glassshield_damage2`: bilinear,
    // clamped), its level by the pixels' footprint as the engine picks it.
    let crack_sampler = device.create_sampler(&SamplerDescriptor { label: Some("visor_cracks"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(CrackPipeline { layout: crack_layout, sampler: crack_sampler });
    let black = device.create_texture_with_data(
        &queue,
        &TextureDescriptor {
            label: Some("visor_cracks_none"),
            size: Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: CRACK_FORMAT,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        TextureDataOrder::LayerMajor,
        &[0],
    );
    commands.insert_resource(CrackTarget { black: black.create_view(&TextureViewDescriptor::default()), target: None, pipeline: None, drawn: Mutex::new(None) });
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
pub(super) struct VisorParts {
    pipelines: [CachedRenderPipelineId; 7],
    half: [CachedTexture; 2],
}

/// The most cracks drawn (`trp_sim::visor::MOST_CRACKS`), and their quads'
/// bytes: two rows of four numbers a crack.
const QUADS: usize = 20;

const QUADS_BYTES: u64 = (QUADS * 8 * 4) as u64;

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, &VisorScreen)>,
    (mut cracks, crack_pipeline, mut crack_pipelines): (ResMut<CrackTarget>, Res<CrackPipeline>, ResMut<SpecializedRenderPipelines<CrackPipeline>>),
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
    for (entity, target, screen) in &views {
        let format = target.main_texture_format();
        let size = target.main_texture().size();
        // The cracks' picture of the view's size, made anew when that moves
        // (its drawing then made again).
        if screen.cracks > 0.0 {
            if cracks.pipeline.is_none() {
                cracks.pipeline = Some(crack_pipelines.specialize(&cache, &crack_pipeline, ()));
            }
            let wanted = UVec2::new(size.width.max(1), size.height.max(1));
            if cracks.target.as_ref().is_none_or(|(_, _, had)| *had != wanted) {
                let texture = device.create_texture(&TextureDescriptor {
                    label: Some("visor_cracks"),
                    size: Extent3d { width: wanted.x, height: wanted.y, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: CRACK_FORMAT,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&TextureViewDescriptor::default());
                cracks.target = Some((texture, view, wanted));
                *cracks.drawn.get_mut().unwrap_or_else(|e| e.into_inner()) = None;
            }
        }
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
pub(super) struct VisorNode;

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
        // The cracks' picture drawn anew where its drawing or its size
        // moved (`DrawScratches`): cleared, then each crack's quad added.
        let cracks = world.resource::<CrackTarget>();
        let cracked = screen.cracks > 0.0;
        if let (true, Some((_, crack_view, _)), Some(quads), Some(picture), Some(render)) = (
            cracked,
            cracks.target.as_ref(),
            world.get_resource::<CrackScreen>(),
            view_of(&images.crack),
            cracks.pipeline.and_then(|id| cache.get_render_pipeline(id)),
        ) {
            let mut drawn = cracks.drawn.lock().unwrap_or_else(|e| e.into_inner());
            if *drawn != Some(quads.drawings) {
                let mut rows = [0.0f32; QUADS * 8];
                for (k, quad) in quads.quads.iter().take(QUADS).enumerate() {
                    rows[k * 8..k * 8 + 5].copy_from_slice(&[quad.low[0], quad.low[1], quad.high[0], quad.high[1], quad.rotation]);
                }
                let bytes: Vec<u8> = rows.iter().flat_map(|row| row.to_le_bytes()).collect();
                let device = context.render_device();
                let buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("visor_cracks"), contents: &bytes, usage: BufferUsages::UNIFORM });
                let crack_pipeline = world.resource::<CrackPipeline>();
                let group = device.create_bind_group("visor_cracks", &cache.get_bind_group_layout(&crack_pipeline.layout), &BindGroupEntries::sequential((picture, &crack_pipeline.sampler, buffer.as_entire_binding())));
                let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                    label: Some("visor_cracks"),
                    color_attachments: &[Some(RenderPassColorAttachment { view: crack_view, depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Clear(LinearRgba::BLACK.into()), store: StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_render_pipeline(render);
                pass.set_bind_group(0, &group, &[]);
                let count = quads.quads.len().min(QUADS) as u32;
                if count > 0 {
                    pass.draw(0..6, 0..count);
                }
                *drawn = Some(quads.drawings);
            }
        }
        let crack_view = match (cracked, cracks.target.as_ref()) {
            (true, Some((_, view, _))) => view,
            _ => &cracks.black,
        };
        // A mask of none is not sampled (`framed` 0): any picture stands in.
        let mask = view_of(&images.mask).unwrap_or(scratches);
        let layout = cache.get_bind_group_layout(&pipeline.layout);
        let at = |pass: Pass| found[pass as usize];
        let draw = |context: &mut RenderContext, render: &RenderPipeline, from: &TextureView, soft: &TextureView, into: &TextureView, round: u32| {
            let group = context.render_device().create_bind_group(
                "visor",
                &layout,
                &BindGroupEntries::sequential((from, &pipeline.clamped, uniform.clone(), mask, scratches, blur_mask, distort, soft, &pipeline.repeated, crack_view)),
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
        // Pass 0, the scratches and the cracks' grey; pass 1, the frame
        // moved by the cracks (none to move without them).
        let write = target.post_process_write();
        draw(context, at(Pass::Scratches), write.source, a, write.destination, 0);
        if cracked {
            let write = target.post_process_write();
            draw(context, at(Pass::Cracks), write.source, a, write.destination, 0);
        }
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
