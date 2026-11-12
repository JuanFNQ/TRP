//! The undither pipeline, its parts and the view node.

use super::*;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct UnditherLabel;

#[derive(Resource)]
pub(super) struct UnditherPipeline {
    layout: BindGroupLayoutDescriptor,
    fullscreen: FullscreenShader,
}

impl SpecializedRenderPipeline for UnditherPipeline {
    /// The frame's format; whether its figures are linear by the 2.2 power
    /// (a preview's camera, its photographic curve after the undithering:
    /// `preview_tonemap`), where the world's are already the game's numbers
    /// (`gamma_ui`'s encode before it).
    type Key = (TextureFormat, bool);

    fn specialize(&self, (format, linear): (TextureFormat, bool)) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("undithering".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: UNDITHER_SHADER,
                shader_defs: if linear { vec!["FRAME_LINEAR".into()] } else { Vec::new() },
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

pub(super) fn init_pipeline(mut commands: Commands, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "undithering",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (texture_2d(TextureSampleType::Float { filterable: false }), texture_2d(TextureSampleType::Float { filterable: false }))),
    );
    commands.insert_resource(UnditherPipeline { layout, fullscreen: fullscreen.clone() });
}

#[derive(Component)]
pub(super) struct UnditherParts(CachedRenderPipelineId);

pub(super) fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, Has<crate::screens::preview_tonemap::PreviewTonemap>), With<UnditherScreen>>,
    done: Query<Entity, (With<UnditherParts>, Without<UnditherScreen>)>,
    cache: Res<PipelineCache>,
    pipeline: Res<UnditherPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<UnditherPipeline>>,
) {
    for entity in &done {
        commands.entity(entity).remove::<UnditherParts>();
    }
    for (entity, target, photographic) in &views {
        let id = pipelines.specialize(&cache, &pipeline, (target.main_texture_format(), photographic));
        commands.entity(entity).insert(UnditherParts(id));
    }
}

#[derive(Default)]
pub(super) struct UnditherNode;

impl ViewNode for UnditherNode {
    /// The frame, the scene's depth as copied this frame (`muzzle_view`'s
    /// `DepthCopyLabel`, for a view with this screen).
    type ViewQuery = (&'static ViewTarget, &'static UnditherParts, &'static crate::weapons::muzzle_view::JetParts, &'static UnditherScreen);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, parts, jets, _): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<UnditherPipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(drawn) = cache.get_render_pipeline(parts.0) else { return Ok(()) };
        let write = target.post_process_write();
        let group = context.render_device().create_bind_group("undithering", &cache.get_bind_group_layout(&pipeline.layout), &BindGroupEntries::sequential((write.source, &jets.depth.default_view)));
        let mut render = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("undithering"),
            color_attachments: &[Some(RenderPassColorAttachment { view: write.destination, depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Clear(Default::default()), store: StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render.set_render_pipeline(drawn);
        render.set_bind_group(0, &group, &[]);
        render.draw(0..3, 0..1);
        Ok(())
    }
}
