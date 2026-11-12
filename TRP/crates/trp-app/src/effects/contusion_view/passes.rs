//! The contusion pipeline, its passes and the view nodes that run them.

use super::*;

/// The camera's effects up to pain's blur (the double vision, the radial
/// blur), and after them (`ContusionLateLabel`) from the flash on: the face
/// shield's mask (`visor_view`) comes between, as on the game's camera.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct ContusionLabel;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct ContusionLateLabel;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct NearDeathLabel;

#[derive(Resource)]
pub(super) struct ContusionPipeline {
    layout: BindGroupLayoutDescriptor,
    /// The burn's: the frame, its sampler, the spots, and four pictures
    /// with their sampler (mipmapped).
    burn_layout: BindGroupLayoutDescriptor,
    /// Near death's: the frame, its sampler, the effects, and its picture.
    near_death_layout: BindGroupLayoutDescriptor,
    pub(super) sampler: Sampler,
    pictures: Sampler,
    fullscreen: FullscreenShader,
}

/// The passes: the double vision, the wiggle, the blur's accumulation (its
/// colour blended over by the frame's alpha), and a copy.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub(super) enum Pass {
    Double,
    Wiggle,
    Accumulate,
    Copy,
    Flash,
    Burn,
    Radial,
    Vignette,
    NearDeath,
}

/// The passes in the order `ContusionParts` keeps their pipelines.
const PASSES: [Pass; 9] = [Pass::Double, Pass::Wiggle, Pass::Accumulate, Pass::Copy, Pass::Flash, Pass::Burn, Pass::Radial, Pass::Vignette, Pass::NearDeath];

impl SpecializedRenderPipeline for ContusionPipeline {
    type Key = (TextureFormat, Pass);

    fn specialize(&self, (format, pass): Self::Key) -> RenderPipelineDescriptor {
        let (entry, blend, write) = match pass {
            Pass::Double => ("double_vision", None, ColorWrites::ALL),
            Pass::Wiggle => ("wiggle", None, ColorWrites::ALL),
            // `Hidden/MotionBlur`'s first pass: `SrcAlpha OneMinusSrcAlpha`,
            // colour only.
            Pass::Accumulate => (
                "accumulate",
                Some(BlendState {
                    color: BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add },
                    alpha: BlendComponent::REPLACE,
                }),
                ColorWrites::COLOR,
            ),
            Pass::Copy => ("copy", None, ColorWrites::ALL),
            Pass::Flash => ("white_flash", None, ColorWrites::ALL),
            Pass::Burn => ("eye_burn", None, ColorWrites::ALL),
            Pass::Radial => ("radial_blur", None, ColorWrites::ALL),
            Pass::Vignette => ("vignette", None, ColorWrites::ALL),
            Pass::NearDeath => ("near_death", None, ColorWrites::ALL),
        };
        let layout = match pass {
            Pass::Burn => self.burn_layout.clone(),
            Pass::NearDeath => self.near_death_layout.clone(),
            _ => self.layout.clone(),
        };
        RenderPipelineDescriptor {
            label: Some(format!("contusion {entry}").into()),
            layout: vec![layout],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: if pass == Pass::Burn { EYE_BURN_SHADER } else { CONTUSION_SHADER },
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState { format, blend, write_mask: write })],
                ..default()
            }),
            ..default()
        }
    }
}

pub(super) fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "contusion",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), uniform_buffer::<ContusionScreen>(true))),
    );
    let picture = || texture_2d(TextureSampleType::Float { filterable: true });
    let burn_layout = BindGroupLayoutDescriptor::new(
        "eye_burn",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (picture(), sampler(SamplerBindingType::Filtering), uniform_buffer::<EyeBurnScreen>(true), picture(), picture(), picture(), picture(), sampler(SamplerBindingType::Filtering)),
        ),
    );
    let near_death_layout = BindGroupLayoutDescriptor::new(
        "near_death",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (picture(), sampler(SamplerBindingType::Filtering), uniform_buffer::<ContusionScreen>(true), picture())),
    );
    // The camera's pictures: bilinear, clamped at their edges.
    let sampler = device.create_sampler(&SamplerDescriptor { label: Some("contusion"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    // The spots' pictures: trilinear, clamped.
    let pictures = device.create_sampler(&SamplerDescriptor { label: Some("eye_burn"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, mipmap_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(ContusionPipeline { layout, burn_layout, near_death_layout, sampler, pictures, fullscreen: fullscreen.clone() });
}

/// Each view's motion blur picture (`MotionBlur.accumTexture`), kept from
/// frame to frame, with its size and format.
#[derive(Resource, Default)]
pub(super) struct Accumulations(pub(super) HashMap<Entity, (Texture, TextureView, Extent3d, TextureFormat)>);

/// What a view with the effects needs this frame.
#[derive(Component)]
pub(super) struct ContusionParts {
    pipelines: [CachedRenderPipelineId; 9],
    /// Its blur's picture made this frame (a new size or format too).
    pub(super) made: bool,
}

pub(super) fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, &ContusionScreen)>,
    done: Query<Entity, (With<ContusionParts>, Without<ContusionScreen>)>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
    pipeline: Res<ContusionPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<ContusionPipeline>>,
    mut accumulations: ResMut<Accumulations>,
) {
    for entity in &done {
        commands.entity(entity).remove::<(ContusionParts, DynamicUniformIndex<ContusionScreen>)>();
    }
    // A view whose blur is off lets its picture go (`MotionBlur.OnDisable`).
    accumulations.0.retain(|entity, _| views.get(*entity).is_ok_and(|(_, _, screen)| screen.blur_on > 0.0));
    for (entity, target, screen) in &views {
        let format = target.main_texture_format();
        let size = target.main_texture().size();
        let made = screen.blur_on > 0.0 && accumulations.0.get(&entity).is_none_or(|(_, _, had, had_format)| *had != size || *had_format != format);
        if made {
            let texture = device.create_texture(&TextureDescriptor {
                label: Some("contusion_motion_blur"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&TextureViewDescriptor::default());
            accumulations.0.insert(entity, (texture, view, size, format));
        }
        let ids = PASSES.map(|pass| pipelines.specialize(&cache, &pipeline, (format, pass)));
        commands.entity(entity).insert(ContusionParts { pipelines: ids, made });
    }
}

#[derive(Default)]
pub(super) struct ContusionNode;

#[derive(Default)]
pub(super) struct ContusionLateNode;

type ContusionQuery = (
    Entity,
    &'static ViewTarget,
    &'static ContusionScreen,
    &'static ContusionParts,
    &'static DynamicUniformIndex<ContusionScreen>,
    Option<&'static DynamicUniformIndex<EyeBurnScreen>>,
);

impl ViewNode for ContusionNode {
    type ViewQuery = ContusionQuery;

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, item: QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        run_effects(false, context, item, world)
    }
}

impl ViewNode for ContusionLateNode {
    type ViewQuery = ContusionQuery;

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, item: QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        run_effects(true, context, item, world)
    }
}

/// The effects in the camera's order: up to pain's blur, or (`late`) from
/// the flash on.
fn run_effects(late: bool, context: &mut RenderContext, (entity, target, screen, parts, index, burn_index): QueryItem<ContusionQuery>, world: &World) -> Result<(), NodeRunError> {
    {
        let pipeline = world.resource::<ContusionPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(uniform) = world.resource::<ComponentUniforms<ContusionScreen>>().uniforms().binding() else { return Ok(()) };
        let found: Option<Vec<&RenderPipeline>> = parts.pipelines.iter().map(|id| cache.get_render_pipeline(*id)).collect();
        let Some(found) = found else { return Ok(()) };
        let layout = cache.get_bind_group_layout(&pipeline.layout);
        // One full-screen pass from a picture into another, loading what
        // is there where `keep` (the blur's blend over its own picture).
        let draw = |context: &mut RenderContext, render: &RenderPipeline, from: &TextureView, into: &TextureView, keep: bool| {
            let group = context.render_device().create_bind_group("contusion", &layout, &BindGroupEntries::sequential((from, &pipeline.sampler, uniform.clone())));
            let ops = if keep { Operations { load: LoadOp::Load, store: StoreOp::Store } } else { Operations::default() };
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("contusion"),
                color_attachments: &[Some(RenderPassColorAttachment { view: into, depth_slice: None, resolve_target: None, ops })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_render_pipeline(render);
            pass.set_bind_group(0, &group, &[index.index()]);
            pass.draw(0..3, 0..1);
        };
        if !late {
            if screen.double_on > 0.0 {
                let write = target.post_process_write();
                draw(context, found[0], write.source, write.destination, false);
            }
            // Pain's blur (`CC_RadialBlur`), next but for the hue focus,
            // which nothing here drives.
            if screen.radial_on > 0.0 {
                let write = target.post_process_write();
                draw(context, found[6], write.source, write.destination, false);
            }
            return Ok(());
        }
        // The flash (`GrenadeFlashScreenEffect`), next in the camera's order
        // after the face shield's mask (`VisorEffect`).
        if screen.flash_on > 0.0 {
            let write = target.post_process_write();
            draw(context, found[4], write.source, write.destination, false);
        }
        // The eyes' burn (`EyeBurn`), next.
        if screen.burn_on > 0.0 {
            let images = world.resource::<ScreenImages>();
            let gpu = world.resource::<RenderAssets<GpuImage>>();
            let views: Vec<&TextureView> = images.burn.iter().filter_map(|handle| gpu.get(handle).map(|image| &image.texture_view)).collect();
            let burn_uniform = world.resource::<ComponentUniforms<EyeBurnScreen>>().uniforms().binding();
            if let (Some(burn_index), Some(burn_uniform), Some(first)) = (burn_index, burn_uniform, views.first()) {
                let at = |n: usize| *views.get(n).unwrap_or(first);
                let write = target.post_process_write();
                let group = context.render_device().create_bind_group(
                    "eye_burn",
                    &cache.get_bind_group_layout(&pipeline.burn_layout),
                    &BindGroupEntries::sequential((write.source, &pipeline.sampler, burn_uniform, at(0), at(1), at(2), at(3), &pipeline.pictures)),
                );
                let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                    label: Some("eye_burn"),
                    color_attachments: &[Some(RenderPassColorAttachment { view: write.destination, depth_slice: None, resolve_target: None, ops: Operations::default() })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_render_pipeline(found[5]);
                pass.set_bind_group(0, &group, &[burn_index.index()]);
                pass.draw(0..3, 0..1);
            }
        }
        // Exhaustion's vignette (`CC_FastVignette`), next.
        if screen.vignette_on > 0.0 {
            let write = target.post_process_write();
            draw(context, found[7], write.source, write.destination, false);
        }
        if screen.wiggle_on > 0.0 {
            let write = target.post_process_write();
            draw(context, found[1], write.source, write.destination, false);
        }
        if screen.blur_on > 0.0 {
            let accumulations = world.resource::<Accumulations>();
            if let Some((_, accumulated, _, _)) = accumulations.0.get(&entity) {
                // Made as it comes on: a copy of the frame.
                if screen.fresh > 0.0 || parts.made {
                    draw(context, found[3], target.main_texture_view(), accumulated, false);
                }
                draw(context, found[2], target.main_texture_view(), accumulated, true);
                let write = target.post_process_write();
                draw(context, found[3], accumulated, write.destination, false);
            }
        }
        Ok(())
    }
}

/// Near death's blend (`CC_Blend`, mode 1), which the game adds to the
/// camera as it starts, after all it has: after the inventory's blur.
#[derive(Default)]
pub(super) struct NearDeathNode;

impl ViewNode for NearDeathNode {
    type ViewQuery = (&'static ViewTarget, &'static ContusionScreen, &'static ContusionParts, &'static DynamicUniformIndex<ContusionScreen>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, screen, parts, index): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        if screen.near_death_on <= 0.0 {
            return Ok(());
        }
        let pipeline = world.resource::<ContusionPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(render) = cache.get_render_pipeline(parts.pipelines[8]) else { return Ok(()) };
        let Some(uniform) = world.resource::<ComponentUniforms<ContusionScreen>>().uniforms().binding() else { return Ok(()) };
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let Some(overlay) = world.resource::<ScreenImages>().near_death.as_ref().and_then(|handle| gpu.get(handle)) else { return Ok(()) };
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group(
            "near_death",
            &cache.get_bind_group_layout(&pipeline.near_death_layout),
            &BindGroupEntries::sequential((write.source, &pipeline.sampler, uniform, &overlay.texture_view)),
        );
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("near_death"),
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
