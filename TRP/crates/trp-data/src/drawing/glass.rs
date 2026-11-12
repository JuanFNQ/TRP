//! The see-through glass of the face shields and the window magazines.
//! The shader `Transparent/DepthZwriteDithered`, after the body-animator spec, "The shield's
//! glass": which materials the game draws with it, and the figures its
//! deferred pass reads. Drawn by the app's `glass_view`.

use crate::unity::extract::MaterialData;

/// The shader's object in the install's `shaders` bundle, by its path id,
/// as a material names it (`m_Shader`): every face shield's glass and the
/// window magazines', the PMAG GEN M3 Window's among them
/// (`local\visor_glass_s126.md`). Known by its shader, not by its
/// properties: a material keeps the properties of a shader it once had.
pub const DITHERED_GLASS_SHADER: i64 = -9198708976800390661;

/// What the shader's deferred pass reads of a material, each as the
/// material saves it or, where it saves none, the shader's own default (its
/// parsed form): figures as stored, a gamma project's, none made linear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassFigures {
    /// `_Color` (default white): the picture's colour and alpha times it.
    pub color: [f32; 4],
    /// `_OpacityScale` (default 1): the alpha's scale, the glass opaque
    /// where it comes to a half or more.
    pub opacity_scale: f32,
    /// `_SpecColor` (default grey 0.5).
    pub spec_color: [f32; 4],
    /// `_ReflectColor` (default white, alpha 0.5): the cube's reflection
    /// times it.
    pub reflect_color: [f32; 4],
    /// `_SpecPower` (default 1): the specular picture's red's scale to the
    /// smoothness.
    pub spec_power: f32,
    /// `_Glossness` (default 1): its alpha's scale to the specular colour.
    pub glossness: f32,
    /// `_GlobalReflectionStrength` (default 1): the level's reflection
    /// probe's share of the emission.
    pub global_reflection_strength: f32,
    /// `_Cull` (default 2): the faces culled, Unity's (0 none, 1 front, 2
    /// back).
    pub cull: u8,
}

impl GlassFigures {
    /// A material's figures, where its shader is the dithered glass. A
    /// tinted one (`_Tinted` 1) is not taken: its mark and the camera's
    /// branch for it are not built (no material of the install's is
    /// tinted), and it stays drawn as any other material.
    pub fn of(data: &MaterialData) -> Option<Self> {
        if data.shader != DITHERED_GLASS_SHADER {
            return None;
        }
        let color = |name: &str, default: [f32; 4]| data.colors.get(name).copied().unwrap_or(default);
        let float = |name: &str, default: f32| data.floats.get(name).copied().unwrap_or(default);
        if float("_Tinted", 0.0) > 0.5 {
            return None;
        }
        Some(Self {
            color: color("_Color", [1.0; 4]),
            opacity_scale: float("_OpacityScale", 1.0),
            spec_color: color("_SpecColor", [0.5, 0.5, 0.5, 1.0]),
            reflect_color: color("_ReflectColor", [1.0, 1.0, 1.0, 0.5]),
            spec_power: float("_SpecPower", 1.0),
            glossness: float("_Glossness", 1.0),
            global_reflection_strength: float("_GlobalReflectionStrength", 1.0),
            cull: float("_Cull", 2.0).clamp(0.0, 2.0) as u8,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known by its shader alone: the same properties under another shader
    /// are not the glass, and a material of the glass's that saves none
    /// takes the shader's defaults.
    #[test]
    fn the_glass_is_known_by_its_shader() {
        let mut data = MaterialData { shader: DITHERED_GLASS_SHADER, ..Default::default() };
        let figures = GlassFigures::of(&data).expect("the glass");
        assert_eq!((figures.color, figures.opacity_scale, figures.spec_color, figures.reflect_color), ([1.0; 4], 1.0, [0.5, 0.5, 0.5, 1.0], [1.0, 1.0, 1.0, 0.5]));
        assert_eq!((figures.spec_power, figures.glossness, figures.global_reflection_strength, figures.cull), (1.0, 1.0, 1.0, 2));
        data.floats.insert("_Tinted".into(), 1.0);
        assert!(GlassFigures::of(&data).is_none(), "a tinted glass is not taken");
        let other = MaterialData { shader: 1, floats: [("_OpacityScale".to_owned(), 1.0), ("_Tinted".to_owned(), 0.0)].into_iter().collect(), ..Default::default() };
        assert!(GlassFigures::of(&other).is_none());
    }
}
