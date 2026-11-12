//! The render pipeline and the view node that draws the effects.

use super::*;

/// Its pass, in the camera's effects' order (`gamma_ui`).
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct ExplosionLabel;

#[derive(Resource)]
pub(super) struct EffectsPipeline {
    layout: BindGroupLayoutDescriptor,
    /// The frame's copy read bilinear, clamped at its edges.
    grab_sampler: Sampler,
}

impl SpecializedRenderPipeline for EffectsPipeline {
    /// Its look, the frame's format, and whether it is drawn off the frame
    /// (`offscreen_dust`: into its picture, its alpha what shows through).
    type Key = (Look, TextureFormat, bool);

    fn specialize(&self, (look, format, off_frame): Self::Key) -> RenderPipelineDescriptor {
        let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
        let (blend, write_mask) = look.blend();
        let (format, blend, write_mask) = if off_frame {
            let through = BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
            (crate::effects::offscreen_dust::COLOUR, BlendState { color: blend.color, alpha: through }, ColorWrites::ALL)
        } else {
            (format, blend, write_mask)
        };
        RenderPipelineDescriptor {
            label: Some(format!("effect {}", look.entry()).into()),
            layout: vec![self.layout.clone()],
            vertex: VertexState {
                shader: EFFECTS_SHADER,
                entry_point: Some("vertex".into()),
                buffers: vec![VertexBufferLayout {
                    array_stride: 12 * 4,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![at(0, 0, VertexFormat::Float32x3), at(1, 12, VertexFormat::Float32x2), at(2, 20, VertexFormat::Float32x4), at(3, 36, VertexFormat::Float32x3)],
                }],
                ..default()
            },
            fragment: Some(FragmentState { shader: EFFECTS_SHADER, entry_point: Some(look.entry().into()), targets: vec![Some(ColorTargetState { format, blend: Some(blend), write_mask })], ..default() }),
            ..default()
        }
    }
}

pub(super) fn init_pipeline(mut commands: Commands, device: Res<bevy::render::renderer::RenderDevice>) {
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let layout = BindGroupLayoutDescriptor::new(
        "particle_effects",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<EffectUniform>(false),
                picture(),
                sampler(SamplerBindingType::Filtering),
                picture(),
                sampler(SamplerBindingType::Filtering),
                picture(),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: false }),
                picture(),
                sampler(SamplerBindingType::Filtering),
                bevy::render::render_resource::binding_types::texture_cube(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let grab_sampler = device.create_sampler(&SamplerDescriptor { label: Some("particle_effects_grab"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(EffectsPipeline { layout, grab_sampler });
}

/// What a view with explosions needs this frame: each shader's pipeline
/// (by whether it is drawn off the frame), the frame's copy where a system's
/// shader reads the frame, and the off-frame pictures where the dust is
/// drawn there (`offscreen_dust`).
#[derive(Component)]
pub(super) struct EffectsParts(HashMap<(Look, bool), CachedRenderPipelineId>, Option<bevy::render::texture::CachedTexture>, Option<crate::effects::offscreen_dust::DustTargets>);

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, &ExplosionScreen)>,
    done: Query<Entity, (With<EffectsParts>, Without<ExplosionScreen>)>,
    cache: Res<PipelineCache>,
    pipeline: Res<EffectsPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<EffectsPipeline>>,
    device: Res<bevy::render::renderer::RenderDevice>,
    mut textures: ResMut<bevy::render::texture::TextureCache>,
    (dust_pipeline, mut dust_pipelines): (Res<crate::effects::offscreen_dust::DustPipeline>, ResMut<SpecializedRenderPipelines<crate::effects::offscreen_dust::DustPipeline>>),
) {
    for entity in &done {
        commands.entity(entity).remove::<EffectsParts>();
    }
    let reduction = crate::effects::offscreen_dust::in_force().reduction();
    for (entity, target, screen) in &views {
        let format = target.main_texture_format();
        let dust = reduction
            .filter(|_| screen.0.iter().any(|d| d.look.off_frame()))
            .map(|reduction| crate::effects::offscreen_dust::targets(reduction, target.main_texture().size(), format, &device, &mut textures, &cache, &dust_pipeline, &mut dust_pipelines));
        let ids = screen
            .0
            .iter()
            .map(|d| {
                let off = dust.is_some() && d.look.off_frame();
                ((d.look, off), pipelines.specialize(&cache, &pipeline, (d.look, format, off)))
            })
            .collect();
        let grab = screen.0.iter().any(|d| d.look.grabs()).then(|| {
            textures.get(
                &device,
                TextureDescriptor {
                    label: Some("particle_effects_grab"),
                    size: target.main_texture().size(),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format,
                    usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        });
        commands.entity(entity).insert(EffectsParts(ids, grab, dust));
    }
}

#[derive(Default)]
pub(super) struct ExplosionNode;

impl ViewNode for ExplosionNode {
    type ViewQuery = (&'static ViewTarget, &'static ExplosionScreen, &'static EffectsParts, Option<&'static crate::weapons::muzzle_view::JetParts>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, screen, parts, muzzle): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        // The scene's depth as copied this frame (`DepthCopyLabel`).
        let Some(muzzle) = muzzle else { return Ok(()) };
        let pipeline = world.resource::<EffectsPipeline>();
        let cache = world.resource::<PipelineCache>();
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let device = context.render_device().clone();
        // The dust off the frame at the options' DUST RESOLUTION
        // (`offscreen_dust`): each run of its draws, one after another in
        // the engine's order, into its
        // picture (cleared at the run's first), laid over the frame before
        // the next draw of another kind and after the last; the scene's
        // depth made small before the first.
        let dust = parts.2.as_ref();
        let mut depth_made = false;
        let mut run_open = false;
        let mut near = 0.0;
        for draw in &screen.0 {
            let off = dust.is_some() && draw.look.off_frame();
            if let Some(d) = dust.filter(|_| run_open && !off) {
                crate::effects::offscreen_dust::upsample(context, world, d, target.main_texture_view(), &muzzle.depth.default_view, near);
                run_open = false;
            }
            let quads = draw.corners.len() as u32 / 48;
            let Some(look_pipeline) = parts.0.get(&(draw.look, off)).and_then(|id| cache.get_render_pipeline(*id)) else { continue };
            let [Some(main), Some(noise), Some(bump), Some(cube)] = draw.pictures.clone().map(|p| gpu.get(&p)) else { continue };
            // None drawn without a particle.
            if quads == 0 && draw.indices.as_ref().is_none_or(|i| i.is_empty()) {
                continue;
            }
            // The frame as it stands, copied for a shader that reads it.
            let grab = parts.1.as_ref();
            if draw.look.grabs() {
                let Some(grab) = grab else { continue };
                context.command_encoder().copy_texture_to_texture(target.main_texture().as_image_copy(), grab.texture.as_image_copy(), target.main_texture().size());
            }
            let grab_view = grab.map_or(&main.texture_view, |g| &g.default_view);
            let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
            if figures.write(&draw.uniform).is_err() {
                continue;
            }
            let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("particle_effect"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
            let bytes: Vec<u8> = draw.corners.iter().flat_map(|n| n.to_le_bytes()).collect();
            let corners = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("particle_effect_corners"), contents: &bytes, usage: BufferUsages::VERTEX });
            let (indices, count): (Vec<u8>, u32) = match &draw.indices {
                Some(own) => (own.iter().flat_map(|i| i.to_le_bytes()).collect(), own.len() as u32),
                None => ((0..quads).flat_map(|q| [4 * q, 4 * q + 1, 4 * q + 2, 4 * q + 2, 4 * q + 3, 4 * q]).flat_map(u32::to_le_bytes).collect(), quads * 6),
            };
            let index_buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("particle_effect_indices"), contents: &indices, usage: BufferUsages::INDEX });
            // Where it is drawn, the depth it is tested against, and whether
            // its picture is begun.
            let (into, depth, load) = match dust.filter(|_| off) {
                Some(d) => {
                    near = draw.uniform.camera.x;
                    if !depth_made {
                        crate::effects::offscreen_dust::downsample(context, world, d, &muzzle.depth.default_view, near);
                        depth_made = true;
                    }
                    let load = if run_open { LoadOp::Load } else { LoadOp::Clear(wgpu_types::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }) };
                    run_open = true;
                    (&d.colour.default_view, &d.depth.default_view, load)
                }
                None => (target.main_texture_view(), &muzzle.depth.default_view, LoadOp::Load),
            };
            let group = device.create_bind_group(
                "particle_effect",
                &cache.get_bind_group_layout(&pipeline.layout),
                &BindGroupEntries::sequential((
                    uniform.as_entire_binding(),
                    &main.texture_view,
                    &main.sampler,
                    &noise.texture_view,
                    &noise.sampler,
                    &bump.texture_view,
                    &bump.sampler,
                    depth,
                    grab_view,
                    &pipeline.grab_sampler,
                    &cube.texture_view,
                    &cube.sampler,
                )),
            );
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("particle_effect"),
                color_attachments: &[Some(RenderPassColorAttachment { view: into, depth_slice: None, resolve_target: None, ops: Operations { load, store: StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_render_pipeline(look_pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.set_vertex_buffer(0, corners.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
            pass.draw_indexed(0..count, 0, 0..1);
        }
        if let Some(d) = dust.filter(|_| run_open) {
            crate::effects::offscreen_dust::upsample(context, world, d, target.main_texture_view(), &muzzle.depth.default_view, near);
        }
        Ok(())
    }
}
