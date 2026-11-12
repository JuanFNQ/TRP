//! Buffers prepared, and the nodes that draw jets, lasers, flares, tracers, sparks and fumes.

use super::*;

/// Each rig's jets on the graphics card: their corners, their indices and
/// how many.
#[derive(Resource, Default)]
pub(super) struct JetBuffers(pub(super) HashMap<Entity, (Buffer, Buffer, u32)>);

/// What a view with a flash needs this frame: the depth's copy, and the
/// pipelines.
#[derive(Component)]
pub(crate) struct JetParts {
    /// The scene's depth, its first sample, copied this frame (the
    /// distortion's pass reads it too: `distort_view`).
    pub(crate) depth: CachedTexture,
    pub(super) jets: CachedRenderPipelineId,
    pub(super) smoke: CachedRenderPipelineId,
    fume: CachedRenderPipelineId,
    sparks: CachedRenderPipelineId,
    laser_beam: CachedRenderPipelineId,
    laser_dot: CachedRenderPipelineId,
    tracers: CachedRenderPipelineId,
    flares: CachedRenderPipelineId,
    copy: CachedRenderPipelineId,
    pub(super) multisampled: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    mut commands: Commands,
    views: Query<
        (Entity, &ViewTarget, &Msaa),
        Or<(
            With<JetUniform>,
            With<SmokeScreen>,
            With<crate::effects::particles_view::FumeScreen>,
            With<crate::effects::particles_view::SparkScreen>,
            With<crate::effects::particles_view::HazeScreen>,
            With<crate::effects::explosion_view::ExplosionScreen>,
            With<crate::rendering::scattering_view::ScatteringScreen>,
            With<crate::rendering::hbao_view::HbaoScreen>,
            With<crate::weapons::tactical_view::LaserScreen>,
            With<crate::weapons::tracer_view::TracerScreen>,
            With<crate::weapons::flare_view::FlareScreen>,
            With<crate::effects::glass_view::UnditherScreen>,
        )>,
    >,
    done: Query<
        Entity,
        (
            With<JetParts>,
            Without<JetUniform>,
            Without<SmokeScreen>,
            Without<crate::effects::particles_view::FumeScreen>,
            Without<crate::effects::particles_view::SparkScreen>,
            Without<crate::effects::particles_view::HazeScreen>,
            Without<crate::effects::explosion_view::ExplosionScreen>,
            Without<crate::rendering::scattering_view::ScatteringScreen>,
            Without<crate::rendering::hbao_view::HbaoScreen>,
            Without<crate::weapons::tactical_view::LaserScreen>,
            Without<crate::weapons::tracer_view::TracerScreen>,
            Without<crate::weapons::flare_view::FlareScreen>,
            Without<crate::effects::glass_view::UnditherScreen>,
        ),
    >,
    meshes: Option<Res<JetMeshes>>,
    mut buffers: ResMut<JetBuffers>,
    device: Res<RenderDevice>,
    mut textures: ResMut<TextureCache>,
    cache: Res<PipelineCache>,
    pipeline: Res<JetPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<JetPipeline>>,
) {
    for entity in &done {
        commands.entity(entity).remove::<(JetParts, DynamicUniformIndex<JetUniform>)>();
    }
    if let Some(meshes) = meshes {
        for (rig, mesh) in &meshes.0 {
            // An empty buffer cannot be drawn from (wgpu's slices may not be
            // empty): none is made.
            if buffers.0.contains_key(rig) || mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }
            let words: Vec<f32> = mesh
                .vertices
                .iter()
                .flat_map(|v| {
                    let p = convert::position(v.position);
                    let n = convert::position(v.normal);
                    [p.x, p.y, p.z, n.x, n.y, n.z, v.uv0[0], v.uv0[1], v.uv1[0], v.uv1[1], v.tangent[0], v.tangent[1], v.tangent[2], v.tangent[3], v.color[0], v.color[1], v.color[2], v.color[3]]
                })
                .collect();
            let bytes = |numbers: &mut dyn Iterator<Item = [u8; 4]>| numbers.flatten().collect::<Vec<u8>>();
            let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_jets"), contents: &bytes(&mut words.iter().map(|w| w.to_le_bytes())), usage: BufferUsages::VERTEX });
            let indices = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_jets_indices"), contents: &bytes(&mut mesh.indices.iter().map(|i| i.to_le_bytes())), usage: BufferUsages::INDEX });
            buffers.0.insert(*rig, (corners, indices, mesh.indices.len() as u32));
        }
    }
    for (entity, target, msaa) in &views {
        let size = target.main_texture().size();
        let depth = textures.get(
            &device,
            TextureDescriptor {
                label: Some("scene_depth_copy"),
                size: Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R32Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let multisampled = msaa.samples() > 1;
        commands.entity(entity).insert(JetParts {
            depth,
            jets: pipelines.specialize(&cache, &pipeline, JetKey::Jets(target.main_texture_format())),
            smoke: pipelines.specialize(&cache, &pipeline, JetKey::Smoke(target.main_texture_format())),
            fume: pipelines.specialize(&cache, &pipeline, JetKey::Fume(target.main_texture_format())),
            sparks: pipelines.specialize(&cache, &pipeline, JetKey::Sparks(target.main_texture_format())),
            laser_beam: pipelines.specialize(&cache, &pipeline, JetKey::LaserBeam(target.main_texture_format())),
            laser_dot: pipelines.specialize(&cache, &pipeline, JetKey::LaserDot(target.main_texture_format())),
            tracers: pipelines.specialize(&cache, &pipeline, JetKey::Tracers(target.main_texture_format())),
            flares: pipelines.specialize(&cache, &pipeline, JetKey::Flares(target.main_texture_format())),
            copy: pipelines.specialize(&cache, &pipeline, JetKey::Depth(multisampled)),
            multisampled,
        });
    }
}

#[derive(Default)]
pub(super) struct JetNode;

/// The scene's depth, its first sample, copied for the effects that read
/// it (the muzzle's, the explosions', the distortion's, the sky's scattering:
/// `scattering_view`; the ambient occlusion: `hbao_view`; the glass's
/// undithering: `glass_view`), before any of them draws.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct DepthCopyLabel;

#[derive(Default)]
pub(super) struct DepthCopyNode;

impl ViewNode for DepthCopyNode {
    type ViewQuery = (&'static ViewDepthTexture, &'static JetParts);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (depth, parts): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<JetPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(copy) = cache.get_render_pipeline(parts.copy) else { return Ok(()) };
        let device = context.render_device().clone();
        let group = device.create_bind_group("scene_depth", &cache.get_bind_group_layout(&pipeline.depth_layouts[usize::from(parts.multisampled)]), &BindGroupEntries::single(depth.view()));
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("scene_depth_copy"),
            color_attachments: &[Some(RenderPassColorAttachment { view: &parts.depth.default_view, depth_slice: None, resolve_target: None, ops: Operations::default() })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(copy);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

impl ViewNode for JetNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static JetParts,
        Option<(&'static JetRig, &'static DynamicUniformIndex<JetUniform>)>,
        Option<&'static SmokeScreen>,
        Option<&'static crate::effects::particles_view::FumeScreen>,
        Option<&'static crate::effects::particles_view::SparkScreen>,
        Option<&'static crate::weapons::tactical_view::LaserScreen>,
        Option<&'static crate::weapons::tracer_view::TracerScreen>,
        Option<&'static crate::weapons::flare_view::FlareScreen>,
    );

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, jets_shown, smoke, fume, sparks, lasers, tracers, flares): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<JetPipeline>();
        let cache = world.resource::<PipelineCache>();
        let (Some(jets), Some(smoke_pipeline)) = (cache.get_render_pipeline(parts.jets), cache.get_render_pipeline(parts.smoke)) else { return Ok(()) };
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let device = context.render_device().clone();
        // The fumes, behind the jets (`PROVISIONAL`: the engine sorts its
        // transparent renderers far to near by their bounds' middles).
        if let (Some(draw), Some(fume_pipeline)) = (fume.and_then(|f| f.0.as_ref()), cache.get_render_pipeline(parts.fume)) {
            draw_fumes(context, world, &device, target, parts, fume_pipeline, draw);
        }
        if let Some((rig, index)) = jets_shown {
            draw_jets(context, world, &device, target, parts, jets, rig, index);
        }
        // The tracers, of the transparent queue as the jets and fumes are
        // (`PROVISIONAL`: after them, where the engine sorts its transparent
        // renderers far to near by their bounds' middles).
        if let (Some(draw), Some(tracer_pipeline)) = (tracers.and_then(|t| t.0.as_ref()), cache.get_render_pipeline(parts.tracers)) {
            draw_tracers(context, world, &device, target, parts, tracer_pipeline, draw);
        }
        // The sparks after both (their material's queue, 3003, after the
        // transparent queue's 3000), before the trails.
        if let (Some(draw), Some(sparks_pipeline)) = (sparks.and_then(|s| s.0.as_ref()), cache.get_render_pipeline(parts.sparks)) {
            draw_sparks(context, world, &device, target, parts, sparks_pipeline, draw);
        }
        // The lasers' dots (their material's queue, 3002 or 5000 for the
        // red, after the transparent's), then their beams (4011).
        if let (Some(lasers), Some(beam), Some(dot)) = (lasers, cache.get_render_pipeline(parts.laser_beam), cache.get_render_pipeline(parts.laser_dot)) {
            for draw in &lasers.0 {
                draw_laser(context, world, &device, target, parts, dot, draw, false);
            }
            for draw in &lasers.0 {
                draw_laser(context, world, &device, target, parts, beam, draw, true);
            }
        }
        // The lights' flares, after the transparent things (the game's
        // command buffer at `AfterForwardAlpha`; `PROVISIONAL`: before the
        // trails, whose `OnRenderObject` order against it is not read).
        if let (Some(draw), Some(flare_pipeline)) = (flares.and_then(|f| f.0.as_ref()), cache.get_render_pipeline(parts.flares)) {
            draw_flares(context, world, &device, target, parts, flare_pipeline, draw);
        }
        // The trails, drawn after the scene's own (`OnRenderObject`).
        for trail in smoke.map(|s| s.0.as_slice()).unwrap_or(&[]) {
            let quads = trail.corners.len() as u32 / 40;
            let Some(picture) = gpu.get(&trail.picture).filter(|_| quads > 0) else { continue };
            let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
            if figures.write(&trail.uniform).is_err() {
                continue;
            }
            let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("gun_smoke"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
            let bytes: Vec<u8> = trail.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
            let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("gun_smoke_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
            let indices: Vec<u8> = (0..quads).flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 3, 4 * q]).flat_map(u32::to_le_bytes).collect();
            let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("gun_smoke_indices"), contents: &indices, usage: BufferUsages::INDEX });
            let group = device.create_bind_group(
                "gun_smoke",
                &cache.get_bind_group_layout(&pipeline.smoke_layout),
                &BindGroupEntries::sequential((uniform.as_entire_binding(), &picture.texture_view, &picture.sampler, &parts.depth.default_view)),
            );
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("gun_smoke"),
                color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_render_pipeline(smoke_pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.set_vertex_buffer(0, corners.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
            pass.draw_indexed(0..quads * 6, 0, 0..1);
        }
        Ok(())
    }
}

/// A laser's beam (a strip of two quads' corners: each side at the emitter
/// and at the end) or its dot (a square), added to the frame as it stands.
#[allow(clippy::too_many_arguments)]
fn draw_laser(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, pipeline_drawn: &RenderPipeline, draw: &crate::weapons::tactical_view::LaserDraw, beam: bool) {
    if !beam && draw.uniform.dot.x <= 0.0 {
        return;
    }
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let gpu = world.resource::<RenderAssets<GpuImage>>();
    let (Some(main), Some(noise), Some(dot), Some(dot_noise)) = (gpu.get(&draw.main), gpu.get(&draw.noise), gpu.get(&draw.dot_main), gpu.get(&draw.dot_noise)) else { return };
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    if figures.write(&draw.uniform).is_err() {
        return;
    }
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("laser"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let corners: [f32; 8] = if beam { [-1.0, 0.0, 1.0, 0.0, 1.0, 1.0, -1.0, 1.0] } else { [-1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0] };
    let bytes: Vec<u8> = corners.iter().flat_map(|n| n.to_le_bytes()).collect();
    let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("laser_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
    let indices: Vec<u8> = [0u32, 1, 2, 2, 3, 0].iter().flat_map(|i| i.to_le_bytes()).collect();
    let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("laser_indices"), contents: &indices, usage: BufferUsages::INDEX });
    let group = device.create_bind_group(
        "laser",
        &cache.get_bind_group_layout(&pipeline.laser_layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), &main.texture_view, &main.sampler, &noise.texture_view, &noise.sampler, &dot.texture_view, &dot_noise.texture_view, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("laser"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(pipeline_drawn);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..6, 0, 0..1);
}

/// The tracers' streaks, added to the frame as it stands.
fn draw_flares(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, flares: &RenderPipeline, draw: &crate::weapons::flare_view::FlareDraw) {
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let gpu = world.resource::<RenderAssets<GpuImage>>();
    let (Some(atlas), Some(grain)) = (gpu.get(&draw.atlas), gpu.get(&draw.noise)) else { return };
    // Six corners a flare; none drawn without one.
    let count = (draw.corners.len() / (6 * crate::weapons::flare_view::CORNER_NUMBERS)) as u32;
    if count == 0 {
        return;
    }
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    if figures.write(&draw.uniform).is_err() {
        return;
    }
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("flares"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let bytes: Vec<u8> = draw.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
    let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("flares_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
    let group = device.create_bind_group(
        "flares",
        &cache.get_bind_group_layout(&pipeline.flare_layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), &atlas.texture_view, &atlas.sampler, &grain.texture_view, &grain.sampler, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("flares"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(flares);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.draw(0..count * 6, 0..1);
}

fn draw_tracers(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, tracers: &RenderPipeline, draw: &crate::weapons::tracer_view::TracerDraw) {
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let gpu = world.resource::<RenderAssets<GpuImage>>();
    let Some(picture) = gpu.get(&draw.picture) else { return };
    // Twelve corners and eighteen indices a tracer; none drawn without one.
    let count = (draw.corners.len() / (12 * crate::weapons::tracer_view::CORNER_NUMBERS)) as u32;
    if count == 0 {
        return;
    }
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    if figures.write(&draw.uniform).is_err() {
        return;
    }
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("tracers"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let bytes: Vec<u8> = draw.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
    let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("tracers_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
    let indices: Vec<u8> = (0..count).flat_map(|t| trp_sim::combat::tracers::INDICES.map(|i| i + 12 * t)).flat_map(u32::to_le_bytes).collect();
    let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("tracers_indices"), contents: &indices, usage: BufferUsages::INDEX });
    let group = device.create_bind_group(
        "tracers",
        &cache.get_bind_group_layout(&pipeline.tracer_layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), &picture.texture_view, &picture.sampler, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("tracers"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(tracers);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..count * 18, 0, 0..1);
}

/// The sparks' streaks, added to the frame as it stands.
fn draw_sparks(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, sparks: &RenderPipeline, draw: &crate::effects::particles_view::SparkDraw) {
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let gpu = world.resource::<RenderAssets<GpuImage>>();
    let Some(picture) = gpu.get(&draw.picture) else { return };
    // None drawn without a corner and a triangle.
    if draw.corners.is_empty() || draw.indices.is_empty() {
        return;
    }
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    if figures.write(&draw.uniform).is_err() {
        return;
    }
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_sparks"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let bytes: Vec<u8> = draw.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
    let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_sparks_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
    let indices: Vec<u8> = draw.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_sparks_indices"), contents: &indices, usage: BufferUsages::INDEX });
    let group = device.create_bind_group(
        "muzzle_sparks",
        &cache.get_bind_group_layout(&pipeline.sparks_layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), &picture.texture_view, &picture.sampler, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("muzzle_sparks"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(sparks);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..draw.indices.len() as u32, 0, 0..1);
}

/// The fumes' particles, blended over the frame as it stands.
fn draw_fumes(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, fumes: &RenderPipeline, draw: &crate::effects::particles_view::FumeDraw) {
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let gpu = world.resource::<RenderAssets<GpuImage>>();
    // Four corners and six indices a particle; none drawn without one.
    let quads = draw.corners.len() as u32 / 44;
    let (Some(picture), Some(noise)) = (gpu.get(&draw.picture), gpu.get(&draw.noise)) else { return };
    if quads == 0 {
        return;
    }
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    if figures.write(&draw.uniform).is_err() {
        return;
    }
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_fumes"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let bytes: Vec<u8> = draw.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
    let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_fumes_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
    let indices: Vec<u8> = (0..quads).flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 3, 4 * q]).flat_map(u32::to_le_bytes).collect();
    let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("muzzle_fumes_indices"), contents: &indices, usage: BufferUsages::INDEX });
    let group = device.create_bind_group(
        "muzzle_fumes",
        &cache.get_bind_group_layout(&pipeline.fume_layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), &picture.texture_view, &picture.sampler, &noise.texture_view, &noise.sampler, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("muzzle_fumes"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(fumes);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..quads * 6, 0, 0..1);
}

/// The jets, added onto the frame as it stands.
#[allow(clippy::too_many_arguments)]
fn draw_jets(context: &mut RenderContext, world: &World, device: &RenderDevice, target: &ViewTarget, parts: &JetParts, jets: &RenderPipeline, rig: &JetRig, index: &DynamicUniformIndex<JetUniform>) {
    let pipeline = world.resource::<JetPipeline>();
    let cache = world.resource::<PipelineCache>();
    let Some(uniform) = world.resource::<ComponentUniforms<JetUniform>>().uniforms().binding() else { return };
    let Some((corners, indices, count)) = world.resource::<JetBuffers>().0.get(&rig.0).filter(|(_, _, count)| *count > 0) else { return };
    let Some(mesh) = world.get_resource::<JetMeshes>().and_then(|meshes| meshes.0.get(&rig.0)) else { return };
    let Some(picture) = world.resource::<RenderAssets<GpuImage>>().get(&mesh.picture) else { return };
    let group = device.create_bind_group(
        "muzzle_jets",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((uniform, &picture.texture_view, &picture.sampler, &parts.depth.default_view)),
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("muzzle_jets"),
        color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(jets);
    pass.set_bind_group(0, &group, &[index.index()]);
    pass.set_vertex_buffer(0, corners.slice(..));
    pass.set_index_buffer(indices.slice(..), IndexFormat::Uint32);
    pass.draw_indexed(0..*count, 0, 0..1);
}
