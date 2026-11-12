//! The effects' smoke and dust drawn off the frame at a smaller size, the options' DUST RESOLUTION.
//! These are the shaders that blend over the frame by their alpha,
//! `explosion_view::Look::off_frame`, drawn off the frame at a half or a
//! quarter of its size each way and laid back over it, after GPU Gems 3's
//! "High-Speed, Off-Screen Particles". Each run of such draws, one after
//! another in the engine's order, goes into a picture of its own: its colour
//! the dust adds, its alpha how much of the frame shows through, cleared to
//! none and all; the dust tested against the scene's depth made as small
//! (the nearest of each block); then laid over the frame as one sum (the
//! frame times what shows through, plus the colour), the same sum as the
//! draws blended one by one. Read back by NVIDIA's nearest-depth
//! upsampling: between the small pixels where they lie at the pixel's
//! depth, the nearest in depth at the scene's edges. FULL draws the dust on
//! the frame as the game does, and none of this runs.
//!
//! This is the app's own drawing (rule 8's liberty: the simulation is
//! untouched), not the game's. `ESTIMATE`: the share of the depth past which
//! the nearest sample is taken (0.1), chosen with the captures of session
//! 208.

use std::sync::atomic::{AtomicU8, Ordering};

use bevy::asset::uuid_handle;
use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{CachedTexture, TextureCache};
use bevy::shader::Shader;

pub(crate) const SHADER: Handle<Shader> = uuid_handle!("5d0e7b21-93c4-4af8-8e16-c2a9f04b7d35");

/// The off-frame picture's format: as the frame's, half floats.
pub(crate) const COLOUR: TextureFormat = TextureFormat::Rgba16Float;

/// The depth's share past which a small pixel at an edge is read alone
/// (`ESTIMATE`).
const EDGE_SHARE: f32 = 0.1;

/// The options' DUST RESOLUTION.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DustResolution {
    /// Drawn on the frame, as the game does.
    Full,
    /// Off the frame at half its size each way (a quarter of its pixels).
    Half,
    /// At a quarter of its size each way (a sixteenth of its pixels).
    Quarter,
}

impl DustResolution {
    pub const ALL: [Self; 3] = [Self::Full, Self::Half, Self::Quarter];
    /// The default.
    pub const DEFAULT: Self = Self::Half;

    /// Its name in the options' file and on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Half => "half",
            Self::Quarter => "quarter",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.key() == key)
    }

    /// Its word on the options' page.
    pub fn word(self) -> &'static str {
        match self {
            Self::Full => "FULL",
            Self::Half => "HALF",
            Self::Quarter => "QUARTER",
        }
    }

    /// The frame's pixels across one of the off-frame picture's, where the
    /// dust is drawn off the frame.
    pub fn reduction(self) -> Option<u32> {
        match self {
            Self::Full => None,
            Self::Half => Some(2),
            Self::Quarter => Some(4),
        }
    }
}

/// The setting in force, set by the main world as the options change and
/// read by the renderer each frame.
static IN_FORCE: AtomicU8 = AtomicU8::new(1);

/// The setting the renderer is to draw by from the next frame.
pub fn set(resolution: DustResolution) {
    IN_FORCE.store(DustResolution::ALL.iter().position(|r| *r == resolution).unwrap_or(0) as u8, Ordering::Relaxed);
}

/// The setting in force.
pub fn in_force() -> DustResolution {
    DustResolution::ALL.get(IN_FORCE.load(Ordering::Relaxed) as usize).copied().unwrap_or(DustResolution::DEFAULT)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DustKey {
    Downsample,
    Upsample(TextureFormat),
}

#[derive(Resource)]
pub(crate) struct DustPipeline {
    layout: BindGroupLayoutDescriptor,
    bilinear: Sampler,
    fullscreen: FullscreenShader,
}

#[derive(Clone, Copy, Default, ShaderType)]
struct DustUniform {
    figures: Vec4,
}

impl SpecializedRenderPipeline for DustPipeline {
    type Key = DustKey;

    fn specialize(&self, key: DustKey) -> RenderPipelineDescriptor {
        let (label, entry, target) = match key {
            DustKey::Downsample => ("dust depth", "downsample", ColorTargetState { format: TextureFormat::R32Float, blend: None, write_mask: ColorWrites::ALL }),
            DustKey::Upsample(format) => {
                // The frame times what shows through, plus the dust's colour;
                // the frame's alpha kept.
                let colour = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::SrcAlpha, operation: BlendOperation::Add };
                let alpha = BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                ("dust over the frame", "upsample", ColorTargetState { format, blend: Some(BlendState { color: colour, alpha }), write_mask: ColorWrites::COLOR })
            }
        };
        RenderPipelineDescriptor {
            label: Some(label.into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState { shader: SHADER, entry_point: Some(entry.into()), targets: vec![Some(target)], ..default() }),
            ..default()
        }
    }
}

pub(crate) fn init_pipeline(mut commands: Commands, device: Res<RenderDevice>, fullscreen: Res<FullscreenShader>) {
    let depth = || texture_2d(TextureSampleType::Float { filterable: false });
    let layout = BindGroupLayoutDescriptor::new(
        "offscreen_dust",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (uniform_buffer::<DustUniform>(false), depth(), depth(), texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering)),
        ),
    );
    let bilinear = device.create_sampler(&SamplerDescriptor { label: Some("offscreen_dust"), mag_filter: FilterMode::Linear, min_filter: FilterMode::Linear, ..default() });
    commands.insert_resource(DustPipeline { layout, bilinear, fullscreen: fullscreen.clone() });
}

/// A view's off-frame pictures and passes this frame.
pub(crate) struct DustTargets {
    pub colour: CachedTexture,
    pub depth: CachedTexture,
    downsample: CachedRenderPipelineId,
    upsample: CachedRenderPipelineId,
    reduction: u32,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn targets(
    reduction: u32,
    size: Extent3d,
    format: TextureFormat,
    device: &RenderDevice,
    textures: &mut TextureCache,
    cache: &PipelineCache,
    pipeline: &DustPipeline,
    pipelines: &mut SpecializedRenderPipelines<DustPipeline>,
) -> DustTargets {
    let small = Extent3d { width: size.width.div_ceil(reduction), height: size.height.div_ceil(reduction), depth_or_array_layers: 1 };
    let picture = |label: &'static str, format: TextureFormat, textures: &mut TextureCache| {
        textures.get(
            device,
            TextureDescriptor {
                label: Some(label),
                size: small,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        )
    };
    DustTargets {
        colour: picture("offscreen_dust_colour", COLOUR, textures),
        depth: picture("offscreen_dust_depth", TextureFormat::R32Float, textures),
        downsample: pipelines.specialize(cache, pipeline, DustKey::Downsample),
        upsample: pipelines.specialize(cache, pipeline, DustKey::Upsample(format)),
        reduction,
    }
}

/// The passes' bindings; the downsample's with the full depth in the small
/// depth's place (the small one is what it draws, which may not be read in
/// the same pass, and its shader does not read it).
fn group(context: &RenderContext, world: &World, targets: &DustTargets, full_depth: &TextureView, near: f32, downsampling: bool) -> Option<BindGroup> {
    let pipeline = world.resource::<DustPipeline>();
    let cache = world.resource::<PipelineCache>();
    let device = context.render_device();
    let mut figures = encase::UniformBuffer::new(Vec::<u8>::new());
    figures.write(&DustUniform { figures: Vec4::new(near, targets.reduction as f32, EDGE_SHARE, 0.0) }).ok()?;
    let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("offscreen_dust"), contents: figures.as_ref(), usage: BufferUsages::UNIFORM });
    let small_depth = if downsampling { full_depth } else { &targets.depth.default_view };
    Some(device.create_bind_group(
        "offscreen_dust",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((uniform.as_entire_binding(), full_depth, small_depth, &targets.colour.default_view, &pipeline.bilinear)),
    ))
}

/// The scene's depth made small, once a frame before the first run.
pub(crate) fn downsample(context: &mut RenderContext, world: &World, targets: &DustTargets, full_depth: &TextureView, near: f32) {
    let cache = world.resource::<PipelineCache>();
    let (Some(pipeline), Some(group)) = (cache.get_render_pipeline(targets.downsample), group(context, world, targets, full_depth, near, true)) else { return };
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("offscreen_dust_depth"),
        color_attachments: &[Some(RenderPassColorAttachment { view: &targets.depth.default_view, depth_slice: None, resolve_target: None, ops: Operations::default() })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &group, &[]);
    pass.draw(0..3, 0..1);
}

/// A run's picture laid over the frame.
pub(crate) fn upsample(context: &mut RenderContext, world: &World, targets: &DustTargets, frame: &TextureView, full_depth: &TextureView, near: f32) {
    let cache = world.resource::<PipelineCache>();
    let (Some(pipeline), Some(group)) = (cache.get_render_pipeline(targets.upsample), group(context, world, targets, full_depth, near, false)) else { return };
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("offscreen_dust_over"),
        color_attachments: &[Some(RenderPassColorAttachment { view: frame, depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_round_trips_and_half_is_the_default() {
        for r in DustResolution::ALL {
            assert_eq!(DustResolution::from_key(r.key()), Some(r));
        }
        assert_eq!(DustResolution::DEFAULT, DustResolution::Half);
        assert_eq!(DustResolution::Full.reduction(), None, "FULL draws on the frame, as the game does");
        assert_eq!(DustResolution::Half.reduction(), Some(2));
        assert_eq!(DustResolution::Quarter.reduction(), Some(4));
    }
}
