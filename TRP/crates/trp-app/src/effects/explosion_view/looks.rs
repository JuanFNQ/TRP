//! The effects' light and each material's look.

use super::*;

pub(super) const EFFECTS_SHADER: Handle<Shader> = uuid_handle!("8c3e5a17-4f62-49d0-b2a8-1d7e60f93c45");

/// The light the range's effects are lit by, in the shaders' terms, with
/// `CUSTOMS LIGHT` off (`ESTIMATE`, as the fumes': `particles_view`): its
/// sun a white light of 1, its ambient a thirtieth of that. With it on,
/// the level's (`lighting`; lighting spec, "What each effect's shader
/// takes").
const SUN: f32 = 1.0;

pub(super) const AMBIENT: f32 = 1.0 / 30.0;

/// The light figures an effect's shader is given: the sun's colour and the
/// share of it the unshadowed shaders take (`1 - _DirectionLightShadow`),
/// `glstate_lightmodel_ambient`, the engine's harmonics' constant (linear),
/// and the top ambient with `_LocalMinimalAmbientLight`.
pub(super) fn effect_light(level: Option<&crate::rendering::lighting::LevelLighting>) -> [Vec4; 4] {
    match level {
        Some(l) => [
            Vec4::new(l.sun[0], l.sun[1], l.sun[2], 1.0 - l.direction_light_shadow),
            Vec4::new(l.flat_ambient[0] * 0.5, l.flat_ambient[1] * 0.5, l.flat_ambient[2] * 0.5, 1.0),
            Vec4::new(l.probe_constant[0], l.probe_constant[1], l.probe_constant[2], 0.0),
            // `_LocalMinimalAmbientLight`: unset in Factory by day, not
            // recorded on Customs.
            Vec4::new(l.top_ambient[0], l.top_ambient[1], l.top_ambient[2], 0.0),
        ],
        None => [Vec4::new(SUN, SUN, SUN, 1.0), Vec4::new(AMBIENT, AMBIENT, AMBIENT, 1.0), Vec4::new(AMBIENT, AMBIENT, AMBIENT, 0.0), Vec4::ZERO],
    }
}

/// Which of the game's shaders a system is drawn by (the ones ported).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Look {
    SmokeDistorted,
    SmokeBumped,
    SuperAdditive,
    LegacyAdditive,
    HdrFire,
    FogDust,
    HslAdditive,
    CutoutBumped,
    Wave,
    Heat,
    AdditiveSimple,
    SmokeGrenade,
    AdditiveVertLit,
    /// `Legacy Shaders/Particles/Alpha Blended`: the legacy additive's
    /// fragment, blended over.
    AlphaBlended,
    /// `Legacy Shaders/Particles/Additive (Soft)`.
    AdditiveSoft,
    /// `Particles/Water Splash`: the legacy additive's fragment, blended
    /// over, its tint the night's.
    WaterSplash,
    /// `Legacy Shaders/Transparent/Diffuse`, its forward base pass.
    TransparentDiffuse,
    /// Unity's `Standard`, faded (`_ALPHABLEND_ON`) or opaque (no blend
    /// keyword): its forward base pass.
    StandardFade,
    StandardOpaque,
    /// `BumpedGlass`, its forward base pass.
    BumpedGlass,
    /// `Legacy Shaders/Particles/Multiply` and `Multiply (Double)`: the
    /// frame times the colour, and twice it (a body's blood).
    Multiply,
    MultiplyDouble,
}

impl Look {
    fn of(shader: &str) -> Option<Self> {
        Some(match shader {
            "Legacy Shaders/Transparent/Cutout/Bumped Diffuse" => Self::CutoutBumped,
            "Particles/Smoke Distorted" => Self::SmokeDistorted,
            "Particles/Smoke Bumped" => Self::SmokeBumped,
            "Particles/SuperAdditive" => Self::SuperAdditive,
            "Legacy Shaders/Particles/Additive" => Self::LegacyAdditive,
            "CustomFX/HDRFireParticle" => Self::HdrFire,
            "Global Fog/Alpha Blended Lighted" => Self::FogDust,
            "Particles/HSL Additive" => Self::HslAdditive,
            "Effects/Explosions/DistortionFresnel" => Self::Wave,
            "Effects/Explosions/Distortion/ParticlesCutOut" => Self::Heat,
            "Particles/AdditiveSimple" => Self::AdditiveSimple,
            "Particles/Smoke Distorted Grenade" => Self::SmokeGrenade,
            "Custom/Particles-AdditiveVertLit" => Self::AdditiveVertLit,
            "Legacy Shaders/Particles/Alpha Blended" => Self::AlphaBlended,
            "Legacy Shaders/Particles/Additive (Soft)" => Self::AdditiveSoft,
            "Particles/Water Splash" => Self::WaterSplash,
            "Legacy Shaders/Transparent/Diffuse" => Self::TransparentDiffuse,
            "EFT/BumpedGlass" => Self::BumpedGlass,
            "Legacy Shaders/Particles/Multiply" => Self::Multiply,
            "Legacy Shaders/Particles/Multiply (Double)" => Self::MultiplyDouble,
            _ => return None,
        })
    }

    /// By the material: `Standard` by its blend keyword (the faded, or the
    /// opaque where it has none; cut out or premultiplied not built), the
    /// rest by their shader alone.
    pub(super) fn of_material(m: &trp_data::drawing::particles::ParticleMaterial) -> Option<Self> {
        if m.shader == "Standard" {
            let on = |k: &str| m.keywords.iter().any(|x| x == k);
            return if on("_ALPHABLEND_ON") {
                Some(Self::StandardFade)
            } else if !on("_ALPHATEST_ON") && !on("_ALPHAPREMULTIPLY_ON") {
                Some(Self::StandardOpaque)
            } else {
                None
            };
        }
        Self::of(&m.shader)
    }

    pub(super) fn entry(self) -> &'static str {
        match self {
            Self::SmokeDistorted => "smoke_distorted",
            Self::SmokeBumped => "smoke_bumped",
            Self::SuperAdditive => "super_additive",
            Self::LegacyAdditive => "legacy_additive",
            Self::HdrFire => "hdr_fire",
            Self::FogDust => "fog_dust",
            Self::HslAdditive => "hsl_additive",
            Self::CutoutBumped => "cutout_bumped",
            Self::Wave => "distortion_wave",
            Self::Heat => "heat_distortion",
            Self::AdditiveSimple => "additive_simple",
            Self::SmokeGrenade => "smoke_grenade",
            Self::AdditiveVertLit => "additive_vert_lit",
            Self::AlphaBlended | Self::WaterSplash => "legacy_additive",
            Self::AdditiveSoft => "additive_soft",
            Self::TransparentDiffuse => "transparent_diffuse",
            Self::StandardFade => "standard_fade",
            Self::StandardOpaque => "standard_opaque",
            Self::BumpedGlass => "bumped_glass",
            Self::Multiply => "multiply",
            Self::MultiplyDouble => "multiply_double",
        }
    }

    /// How its corners are lit by the engine's vertex lights (the uniform's
    /// `lit.x`): none, the grenade smoke's, the vertex-lit trails'.
    pub(super) fn vertex_lit(self) -> f32 {
        match self {
            Self::SmokeGrenade => 1.0,
            Self::AdditiveVertLit => 2.0,
            _ => 0.0,
        }
    }

    /// Its shader reads the frame as it stood before it was drawn (an
    /// unnamed `GrabPass` first): the frame is copied for it.
    pub(super) fn grabs(self) -> bool {
        matches!(self, Self::Wave | Self::Heat)
    }

    /// Blended over the frame by its alpha, its colour alone written: it is
    /// drawn off the frame and laid over it at the options' DUST RESOLUTION
    /// of HALF or QUARTER (`offscreen_dust`).
    pub(super) fn off_frame(self) -> bool {
        matches!(self, Self::SmokeDistorted | Self::SmokeBumped | Self::FogDust)
    }

    /// Its pass's blend and what it writes (each read from its shader's
    /// pass state).
    pub(super) fn blend(self) -> (BlendState, ColorWrites) {
        let component = |src_factor, dst_factor| BlendComponent { src_factor, dst_factor, operation: BlendOperation::Add };
        if matches!(self, Self::CutoutBumped | Self::StandardOpaque) {
            // Into the G-buffer, opaque.
            return (BlendState::REPLACE, ColorWrites::COLOR);
        }
        let (src, dst, writes) = match self {
            Self::SmokeDistorted | Self::SmokeBumped | Self::FogDust => (BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, ColorWrites::COLOR),
            // The grenade smoke's pass; the trails' by their material's
            // `_BlendSrc` and `_BlendDst` (5, 10: the one material).
            Self::SmokeGrenade | Self::AdditiveVertLit => (BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, ColorWrites::ALL),
            Self::Wave | Self::Heat => (BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, ColorWrites::ALL),
            Self::AlphaBlended | Self::WaterSplash | Self::TransparentDiffuse => (BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, ColorWrites::COLOR),
            // The fade's `_SrcBlend`, `_DstBlend` (5, 10), every channel.
            Self::StandardFade => (BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha, ColorWrites::ALL),
            Self::AdditiveSoft => (BlendFactor::One, BlendFactor::OneMinusSrc, ColorWrites::COLOR),
            Self::SuperAdditive | Self::LegacyAdditive | Self::HslAdditive | Self::AdditiveSimple => (BlendFactor::SrcAlpha, BlendFactor::One, ColorWrites::COLOR),
            Self::HdrFire | Self::BumpedGlass => (BlendFactor::One, BlendFactor::One, ColorWrites::ALL),
            // `Blend Zero SrcColor`, `Blend DstColor SrcColor` (2, 3 and 0, 3
            // in their passes' states).
            Self::Multiply => (BlendFactor::Zero, BlendFactor::Src, ColorWrites::COLOR),
            Self::MultiplyDouble => (BlendFactor::Dst, BlendFactor::Src, ColorWrites::COLOR),
            Self::CutoutBumped | Self::StandardOpaque => unreachable!("opaque"),
        };
        (BlendState { color: component(src, dst), alpha: component(src, dst) }, writes)
    }
}
