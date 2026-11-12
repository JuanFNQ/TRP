//! The figures handed to the shaders of a scope's lens, its reticle and a collimator.
use bevy::prelude::*;
use bevy::render::render_resource::ShaderType;

/// `Lens` in `optic_lens.wgsl`.
#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub struct LensUniform {
    /// `_Scales.xy`, `_Shifts.xy`.
    pub scales_shifts: Vec4,
    /// `_ShiftDirection` (the game's axes), `_NormalHideness`.
    pub direction: Vec4,
    /// The mesh's inner ring's radius and its rim's, `_MarkLightness`,
    /// and 1 − `_SwitchToSight`.
    pub figures: Vec4,
    /// The power the frame's figures are the game's numbers to (2.2
    /// with `CUSTOMS LIGHT`, else 1), whether the lens has a reticle,
    /// the picture's chromatic aberration; w unused.
    pub space: Vec4,
}

/// `Reticle` in `reticle.wgsl`.
#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub struct ReticleUniform {
    /// `_Color`.
    pub color: Vec4,
    /// `_Scale` (the stored scale times 0.1), the power of the game's
    /// numbers.
    pub figures: Vec4,
}

/// `Collimator` in `collimator.wgsl`.
#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub struct CollimatorUniform {
    /// `_Color.rgb`, `_HDR`.
    pub color: Vec4,
    /// `_MarkShift.xyz` (the game's axes), `_MarkScale`.
    pub mark: Vec4,
    /// The power of the game's numbers, whether it has a noise and a
    /// fade picture.
    pub space: Vec4,
}
