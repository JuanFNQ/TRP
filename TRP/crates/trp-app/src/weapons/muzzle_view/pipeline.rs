//! The jet render pipeline and its initialisation.

use super::*;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct JetLabel;

#[derive(Resource)]
pub(super) struct JetPipeline {
    /// The figures, the picture and its sampler, the scene's depth copied.
    pub(super) layout: BindGroupLayoutDescriptor,
    /// The same for a trail (its figures not dynamic).
    pub(super) smoke_layout: BindGroupLayoutDescriptor,
    /// The fumes' figures, their picture and noise with their samplers,
    /// the scene's depth copied (`particles_view`).
    pub(super) fume_layout: BindGroupLayoutDescriptor,
    /// The sparks' figures, their picture and sampler, the scene's depth.
    pub(super) sparks_layout: BindGroupLayoutDescriptor,
    /// A laser's figures, its beam's picture and noise, its dot's picture
    /// and noise, with their samplers, the scene's depth (`tactical_view`).
    pub(super) laser_layout: BindGroupLayoutDescriptor,
    /// The tracers' figures, their picture and sampler, the scene's depth
    /// (`tracer_view`).
    pub(super) tracer_layout: BindGroupLayoutDescriptor,
    /// The flares' figures, their atlas and grain with their samplers, the
    /// scene's depth (`flare_view`).
    pub(super) flare_layout: BindGroupLayoutDescriptor,
    /// The scene's depth, as it is sampled or as it is not.
    pub(super) depth_layouts: [BindGroupLayoutDescriptor; 2],
    fullscreen: FullscreenShader,
}

/// The jets' pass for a picture's format, the smoke's, or the depth's copy
/// (for a multisampled depth or not).
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub(super) enum JetKey {
    Jets(TextureFormat),
    Smoke(TextureFormat),
    Fume(TextureFormat),
    Sparks(TextureFormat),
    /// A laser's beam, and its dot.
    LaserBeam(TextureFormat),
    LaserDot(TextureFormat),
    Tracers(TextureFormat),
    Flares(TextureFormat),
    Depth(bool),
}

/// The corners' layout: place, axis, cell, corner, tangent, colour.
const STRIDE: u64 = 18 * 4;

impl SpecializedRenderPipeline for JetPipeline {
    type Key = JetKey;

    fn specialize(&self, key: JetKey) -> RenderPipelineDescriptor {
        match key {
            JetKey::Jets(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("muzzle jets".into()),
                    layout: vec![self.layout.clone()],
                    vertex: VertexState {
                        shader: JET_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: STRIDE,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![
                                at(0, 0, VertexFormat::Float32x3),
                                at(1, 12, VertexFormat::Float32x3),
                                at(2, 24, VertexFormat::Float32x2),
                                at(3, 32, VertexFormat::Float32x2),
                                at(4, 40, VertexFormat::Float32x4),
                                at(5, 56, VertexFormat::Float32x4),
                            ],
                        }],
                        ..default()
                    },
                    fragment: Some(FragmentState {
                        shader: JET_SHADER,
                        entry_point: Some("fragment".into()),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::ALL })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Smoke(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                let over = BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("gun smoke".into()),
                    layout: vec![self.smoke_layout.clone()],
                    vertex: VertexState {
                        shader: SMOKE_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: 10 * 4,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![at(0, 0, VertexFormat::Float32x3), at(1, 12, VertexFormat::Float32x3), at(2, 24, VertexFormat::Float32x3), at(3, 36, VertexFormat::Float32)],
                        }],
                        ..default()
                    },
                    fragment: Some(FragmentState {
                        shader: SMOKE_SHADER,
                        entry_point: Some("fragment".into()),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: over, alpha: over }), write_mask: ColorWrites::ALL })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Fume(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                let over = BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("muzzle fumes".into()),
                    layout: vec![self.fume_layout.clone()],
                    vertex: VertexState {
                        shader: FUME_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: 11 * 4,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![
                                at(0, 0, VertexFormat::Float32x3),
                                at(1, 12, VertexFormat::Float32x2),
                                at(2, 20, VertexFormat::Float32),
                                at(3, 24, VertexFormat::Float32),
                                at(4, 28, VertexFormat::Float32x4),
                            ],
                        }],
                        ..default()
                    },
                    fragment: Some(FragmentState {
                        shader: FUME_SHADER,
                        entry_point: Some("fragment".into()),
                        // Its colour written, not its alpha (`ColorMask RGB`).
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: over, alpha: over }), write_mask: ColorWrites::COLOR })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Sparks(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("muzzle sparks".into()),
                    layout: vec![self.sparks_layout.clone()],
                    vertex: VertexState {
                        shader: SPARKS_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: 20 * 4,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![
                                at(0, 0, VertexFormat::Float32x3),
                                at(1, 12, VertexFormat::Float32x2),
                                at(2, 20, VertexFormat::Float32x2),
                                at(3, 28, VertexFormat::Float32x2),
                                at(4, 36, VertexFormat::Float32x4),
                                at(5, 52, VertexFormat::Float32x3),
                                at(6, 64, VertexFormat::Float32x4),
                            ],
                        }],
                        ..default()
                    },
                    // Both faces (`Cull Off`).
                    primitive: PrimitiveState { cull_mode: None, ..default() },
                    fragment: Some(FragmentState {
                        shader: SPARKS_SHADER,
                        entry_point: Some("fragment".into()),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::COLOR })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::LaserBeam(format) | JetKey::LaserDot(format) => {
                let beam = matches!(key, JetKey::LaserBeam(_));
                let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some(if beam { "laser beam".into() } else { "laser dot".into() }),
                    layout: vec![self.laser_layout.clone()],
                    vertex: VertexState {
                        shader: LASER_SHADER,
                        entry_point: Some(if beam { "beam_vertex".into() } else { "dot_vertex".into() }),
                        buffers: vec![VertexBufferLayout {
                            array_stride: 2 * 4,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![VertexAttribute { format: VertexFormat::Float32x2, offset: 0, shader_location: 0 }],
                        }],
                        ..default()
                    },
                    // The beam both faces (`Cull Off`); the dot its front only
                    // (`Cull Back`): it faces away from the surface it lies on.
                    primitive: PrimitiveState { cull_mode: None, ..default() },
                    fragment: Some(FragmentState {
                        shader: LASER_SHADER,
                        entry_point: Some(if beam { "beam_fragment".into() } else { "dot_fragment".into() }),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::COLOR })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Tracers(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                // `Blend SrcAlpha One`.
                let add = BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("tracers".into()),
                    layout: vec![self.tracer_layout.clone()],
                    vertex: VertexState {
                        shader: TRACER_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: (crate::weapons::tracer_view::CORNER_NUMBERS * 4) as u64,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![at(0, 0, VertexFormat::Float32x3), at(1, 12, VertexFormat::Float32x2), at(2, 20, VertexFormat::Float32x4)],
                        }],
                        ..default()
                    },
                    // Both faces (`Cull Off`).
                    primitive: PrimitiveState { cull_mode: None, ..default() },
                    fragment: Some(FragmentState {
                        shader: TRACER_SHADER,
                        entry_point: Some("fragment".into()),
                        // Its colour written, not its alpha (`ColorMask RGB`).
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::COLOR })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Flares(format) => {
                let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
                // `Blend One One`, both faces, no depth test (`ZTest Always`).
                let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
                RenderPipelineDescriptor {
                    label: Some("lights' flares".into()),
                    layout: vec![self.flare_layout.clone()],
                    vertex: VertexState {
                        shader: FLARE_SHADER,
                        entry_point: Some("vertex".into()),
                        buffers: vec![VertexBufferLayout {
                            array_stride: (crate::weapons::flare_view::CORNER_NUMBERS * 4) as u64,
                            step_mode: VertexStepMode::Vertex,
                            attributes: vec![
                                at(0, 0, VertexFormat::Float32x3),
                                at(1, 12, VertexFormat::Float32x2),
                                at(2, 20, VertexFormat::Float32x4),
                                at(3, 36, VertexFormat::Float32x2),
                                at(4, 44, VertexFormat::Float32x4),
                                at(5, 60, VertexFormat::Float32x4),
                                at(6, 76, VertexFormat::Float32),
                            ],
                        }],
                        ..default()
                    },
                    primitive: PrimitiveState { cull_mode: None, ..default() },
                    fragment: Some(FragmentState {
                        shader: FLARE_SHADER,
                        entry_point: Some("fragment".into()),
                        targets: vec![Some(ColorTargetState { format, blend: Some(BlendState { color: add, alpha: add }), write_mask: ColorWrites::ALL })],
                        ..default()
                    }),
                    ..default()
                }
            }
            JetKey::Depth(multisampled) => RenderPipelineDescriptor {
                label: Some("scene depth copy".into()),
                layout: vec![self.depth_layouts[usize::from(multisampled)].clone()],
                vertex: self.fullscreen.to_vertex_state(),
                fragment: Some(FragmentState {
                    shader: DEPTH_SHADER,
                    shader_defs: if multisampled { vec![ShaderDefVal::Bool("MULTISAMPLED".into(), true)] } else { Vec::new() },
                    entry_point: Some("copy_depth".into()),
                    targets: vec![Some(ColorTargetState { format: TextureFormat::R32Float, blend: None, write_mask: ColorWrites::ALL })],
                }),
                ..default()
            },
        }
    }
}

pub(super) fn init_pipeline(mut commands: Commands, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "muzzle_jets",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (uniform_buffer::<JetUniform>(true), texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false })),
        ),
    );
    let smoke_layout = BindGroupLayoutDescriptor::new(
        "gun_smoke",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (uniform_buffer::<SmokeUniform>(false), texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false })),
        ),
    );
    let fume_layout = BindGroupLayoutDescriptor::new(
        "muzzle_fumes",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<crate::effects::particles_view::FumeUniform>(false),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let sparks_layout = BindGroupLayoutDescriptor::new(
        "muzzle_sparks",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (uniform_buffer::<crate::effects::particles_view::SparkUniform>(false), texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false })),
        ),
    );
    let laser_layout = BindGroupLayoutDescriptor::new(
        "lasers",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<crate::weapons::tactical_view::LaserUniform>(false),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_3d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let tracer_layout = BindGroupLayoutDescriptor::new(
        "tracers",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (uniform_buffer::<crate::weapons::tracer_view::TracerUniform>(false), texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), texture_2d(TextureSampleType::Float { filterable: false })),
        ),
    );
    let flare_layout = BindGroupLayoutDescriptor::new(
        "flares",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<crate::weapons::flare_view::FlareUniform>(false),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let depth_layouts = [
        BindGroupLayoutDescriptor::new("scene_depth", &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, texture_depth_2d())),
        BindGroupLayoutDescriptor::new("scene_depth_multisampled", &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, texture_depth_2d_multisampled())),
    ];
    commands.insert_resource(JetPipeline { layout, smoke_layout, fume_layout, sparks_layout, laser_layout, tracer_layout, flare_layout, depth_layouts, fullscreen: fullscreen.clone() });
}
