//! The reticle, lens and collimator materials.

use super::*;

/// A variable scope's reticle mesh (`Custom/Crossfair`).
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct ReticleMaterial {
    #[uniform(0)]
    pub reticle: ReticleUniform,
}

impl Material for ReticleMaterial {
    fn vertex_shader() -> ShaderRef {
        RETICLE_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        RETICLE_SHADER.into()
    }

    /// After the world (the game's: at the optic camera's
    /// `BeforeImageEffects`).
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// Over what is drawn whatever its depth, written as it is (blend one,
    /// zero), both faces, no depth written.
    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = false;
            depth.depth_compare = bevy::render::render_resource::CompareFunction::Always;
        }
        if let Some(target) = descriptor.fragment.as_mut().and_then(|fragment| fragment.targets.first_mut()).and_then(Option::as_mut) {
            target.blend = None;
        }
        Ok(())
    }
}

/// A scope's lens (`CW FX/OpticSight`).
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct LensMaterial {
    #[uniform(0)]
    pub lens: LensUniform,
    /// The optic camera's picture (`_CamTex`).
    #[texture(1)]
    #[sampler(2)]
    pub picture: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    pub mark: Option<Handle<Image>>,
    #[texture(5)]
    #[sampler(6)]
    pub mask: Option<Handle<Image>>,
    #[texture(7)]
    #[sampler(8)]
    pub mask2: Option<Handle<Image>>,
}

impl Material for LensMaterial {
    fn vertex_shader() -> ShaderRef {
        LENS_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        LENS_SHADER.into()
    }

    /// Drawn as an opaque surface, so that the passes after the scene that
    /// go by its depth (the ambient occlusion, the scattering, the clouds)
    /// find the lens and not what lies behind it: the game draws the lens
    /// after its own (`PROVISIONAL`: the lens's depth is written, where the
    /// game's writes none).
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// A red dot's or a holographic sight's lens (`CW FX/Collimator`).
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct CollimatorMaterial {
    #[uniform(0)]
    pub collimator: CollimatorUniform,
    #[texture(1)]
    #[sampler(2)]
    pub mark: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    pub noise: Option<Handle<Image>>,
    #[texture(5)]
    #[sampler(6)]
    pub fade: Option<Handle<Image>>,
}

impl Material for CollimatorMaterial {
    fn vertex_shader() -> ShaderRef {
        COLLIMATOR_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        COLLIMATOR_SHADER.into()
    }

    /// Added over the frame (the game's `One OneMinusSrcAlpha` with an
    /// alpha of nought).
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// Both faces, the depth tested and none written.
    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = false;
        }
        Ok(())
    }
}
