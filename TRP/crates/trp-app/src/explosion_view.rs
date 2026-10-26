//! A grenade's explosion (grenades spec, "The blast's effect"; the
//! options' `PARTICLES`): the effect its template names
//! (`ExplosionEffectType`, `Effects.EmitGrenade`), read at the start from the
//! install (`trp_data::particles::named_effect`: a system and every one
//! below it), its mediator's next copy played where the grenade was with
//! its z axis up (`Quaternion.LookRotation(Vector3.up)`), each system run by
//! `trp_sim::particles` and drawn by a port of its material's shader
//! (`particle_effects.wgsl`), in the engine's order among transparent
//! things: by render queue, then far to near by each system's place (its
//! particles' middle) plus its sorting fudge. On the frame encoded for the
//! camera's effects, before the muzzle's (`muzzle_view`), as the game's
//! (`gamma_ui`). The particles of a system that meets the world meet the
//! range in the game's physics engine (a world of the range's own,
//! `loose::range_world`; `trp_sim::particles::ParticleSystem::step_in`), and
//! each meeting sets off the systems its sub-emitters name for a collision
//! (the concrete's dust); the lights module's lights are Bevy's point lights
//! (`ExplosionLight`). A mesh's particles (the shock wave, on Unity's own
//! sphere) are drawn as the mesh at each; a shader that grabs the frame
//! (the shock wave's and the heat's distortions) is given a copy of the
//! frame as it stands before it is drawn. The scorch is `decal_view`'s.
//! Beside the mediators' effects: an entry's older emitters onto the
//! effects' shared systems and sparks, and the pool's light (the
//! flashbang's: `trp_sim::emitters`); a launcher's blasts played at its
//! muzzle's pivots at each shot, with their trails and their driven light
//! (firing spec, "A launcher's blasts"); the engine's vertex lights for the
//! vertex-lit shaders. A bullet's hit plays its material's entry the same
//! way (`impacts`; ballistics spec, "What a hit shows and sounds"). The
//! older emitters draw from the game's own tables (`FastRndom`), a spark's
//! place from Unity's generator.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderContext;
use bevy::render::texture::GpuImage;
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use trp_data::particles::{ParticleEffect, ParticleSystemData};
use trp_sim::particles::{ParticleSystem, Placement};
use trp_sim::recoil::Rng;

const EFFECTS_SHADER: Handle<Shader> = uuid_handle!("8c3e5a17-4f62-49d0-b2a8-1d7e60f93c45");

/// The light the range's effects are lit by, in the shaders' terms, with
/// `CUSTOMS LIGHT` off (`ESTIMATE`, as the fumes': `particles_view`): its
/// sun a white light of 1, its ambient a thirtieth of that. With it on,
/// the level's (`lighting`; lighting spec, "What each effect's shader
/// takes").
const SUN: f32 = 1.0;
const AMBIENT: f32 = 1.0 / 30.0;

/// The light figures an effect's shader is given: the sun's colour and the
/// share of it the unshadowed shaders take (`1 - _DirectionLightShadow`),
/// `glstate_lightmodel_ambient`, the engine's harmonics' constant (linear),
/// and `_EFT_Ambient` with `_LocalMinimalAmbientLight`.
fn effect_light(level: Option<&crate::lighting::LevelLighting>) -> [Vec4; 4] {
    match level {
        Some(l) => [
            Vec4::new(l.sun[0], l.sun[1], l.sun[2], 1.0 - l.direction_light_shadow),
            Vec4::new(l.flat_ambient[0] * 0.5, l.flat_ambient[1] * 0.5, l.flat_ambient[2] * 0.5, 1.0),
            Vec4::new(l.probe_constant[0], l.probe_constant[1], l.probe_constant[2], 0.0),
            // `_LocalMinimalAmbientLight`: unset in Factory by day, not
            // recorded on Customs.
            Vec4::new(l.eft_ambient[0], l.eft_ambient[1], l.eft_ambient[2], 0.0),
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
    /// `EFT/BumpedGlass`, its forward base pass.
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
    fn of_material(m: &trp_data::particles::ParticleMaterial) -> Option<Self> {
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

    fn entry(self) -> &'static str {
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
    fn vertex_lit(self) -> f32 {
        match self {
            Self::SmokeGrenade => 1.0,
            Self::AdditiveVertLit => 2.0,
            _ => 0.0,
        }
    }

    /// Its shader reads the frame as it stood before it was drawn (an
    /// unnamed `GrabPass` first): the frame is copied for it.
    fn grabs(self) -> bool {
        matches!(self, Self::Wave | Self::Heat)
    }

    /// Its pass's blend and what it writes (each read from its shader's
    /// pass state).
    fn blend(self) -> (BlendState, ColorWrites) {
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

/// What a system's shader is given.
#[derive(Clone, Copy, Default, ShaderType)]
pub struct EffectUniform {
    pub clip_from_world: Mat4,
    pub view_from_world: Mat4,
    /// The near plane, `_Time.x`, `_Time.y`.
    pub camera: Vec4,
    pub light: Vec4,
    pub light_view: Vec4,
    pub ambient: Vec4,
    pub probe: Vec4,
    pub level: Vec4,
    pub tint: Vec4,
    pub st: Vec4,
    pub a: Vec4,
    pub b: Vec4,
    /// The time of day's harmonics, packed as the shaders take them (the
    /// three `a`, the three `b`, `c`), for the screen's ambient at a
    /// G-buffer surface's normal.
    pub sh: [Vec4; 7],
    /// More of the material's figures (the grenade smoke's
    /// `_IndoorTintColor` and `_TODAttenuation`).
    pub c: Vec4,
    pub d: Vec4,
    /// `x`: how the corners are lit by the vertex lights (`Look::vertex_lit`).
    pub lit: Vec4,
    /// The level's figures the grenade smoke takes: `_MinAmbientColor` and
    /// `_SunVisibility`; the engine's `unity_AmbientSky` and `_ClearSky`;
    /// `_TopHorizontSkyColor` and `_ForceIndoor`; `_ForceIndoorIntensity`.
    pub smoke_a: Vec4,
    pub smoke_b: Vec4,
    pub smoke_c: Vec4,
    pub smoke_d: Vec4,
    /// The engine's vertex lights (`unity_LightPosition`, `unity_LightColor`,
    /// `unity_LightAtten`, `unity_SpotDirection`), in the app's axes: a
    /// place with `w` 1, or the way to a directional one with `w` 0.
    pub vl_position: [Vec4; 8],
    pub vl_color: [Vec4; 8],
    pub vl_atten: [Vec4; 8],
    pub vl_spot: [Vec4; 8],
}

/// One system to draw this frame: its shader, its figures, its three
/// pictures, its corners (place, picture's place, colour, normal: twelve
/// numbers each, four a particle), and for a mesh's particles their
/// triangles (none: two to each four corners).
#[derive(Clone)]
pub struct SystemDraw {
    pub look: Look,
    pub uniform: EffectUniform,
    pub pictures: [Handle<Image>; 4],
    pub corners: Arc<Vec<f32>>,
    pub indices: Option<Arc<Vec<u32>>>,
}

/// On the camera: the explosions' systems this frame, in the order drawn.
#[derive(Component, Clone, Default)]
pub struct ExplosionScreen(pub Vec<SystemDraw>);

impl ExtractComponent for ExplosionScreen {
    type QueryData = &'static ExplosionScreen;
    type QueryFilter = ();
    type Out = ExplosionScreen;

    fn extract_component(screen: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (!screen.0.is_empty()).then(|| screen.clone())
    }
}

/// A system of an effect made ready: its figures, where it is from the
/// effect's root, its shader, its pictures and its material's figures.
struct ReadySystem {
    data: Arc<ParticleSystemData>,
    local: Placement,
    look: Option<Look>,
    pictures: [Handle<Image>; 4],
    figures: EffectUniform,
    queue: i64,
    fudge: f32,
    render_mode: i64,
    sort_mode: i64,
    /// Set off by another of the effect's systems: never played with the
    /// effect, only placed with it.
    sub_emitter: bool,
    /// The systems a collision of its particles sets off: each one's place
    /// among the effect's, whether it takes this system's duration, its
    /// chance.
    sets_off: Vec<(usize, bool, f32)>,
    /// Its trails' drawing, where it has trails and a material for them.
    trail: Option<ReadyTrail>,
}

/// A system's trails' material made ready: its shader, figures, pictures
/// and queue.
struct ReadyTrail {
    look: Look,
    figures: EffectUniform,
    pictures: [Handle<Image>; 4],
    queue: i64,
}

/// A muzzle's shared blast (firing spec, "A launcher's blasts"): its kind
/// (0 the front, 1 the back), its systems ready and running, its lights
/// driven by a system's playing, and where it was last played.
struct MuzzleBlast {
    kind: i64,
    effect: Arc<ReadyEffect>,
    systems: Vec<ParticleSystem>,
    lights: Vec<DrivenLight>,
    root: Placement,
}

/// A light of a blast driven by one of its systems (`MuzzleParticleLight`):
/// its colour and range, its place from the blast's root, the system's place
/// among the blast's, the intensities it goes between and its time, and its
/// intensity and the damping's velocity now.
struct DrivenLight {
    color: [f32; 3],
    range: f32,
    local: [f32; 3],
    system: Option<usize>,
    min_max: [f32; 2],
    time: f32,
    intensity: f32,
    velocity: f32,
}

/// A light of the frame as the engine hands its vertex lights to a shader,
/// in the app's axes, with how much it matters (`PROVISIONAL`: the sun
/// first, then by its intensity times its fall-off at the thing drawn).
#[derive(Clone, Copy)]
struct VertexLight {
    position: Vec4,
    color: Vec4,
    atten: Vec4,
    spot: Vec4,
}

impl VertexLight {
    fn point(at: Vec3, color: [f32; 3], intensity: f32, range: f32) -> Self {
        let r = range.max(1e-3);
        Self { position: at.extend(1.0), color: Vec4::new(color[0] * intensity, color[1] * intensity, color[2] * intensity, 1.0), atten: Vec4::new(-1.0, 1.0, 25.0 / (r * r), r * r), spot: Vec4::ZERO }
    }

    /// How much it lights a place (a directional light most).
    fn importance(&self, at: Vec3) -> f32 {
        if self.position.w == 0.0 {
            return f32::MAX;
        }
        let d2 = (self.position.truncate() - at).length_squared();
        (self.color.x + self.color.y + self.color.z) / (1.0 + d2 * self.atten.z)
    }
}

/// The eight vertex lights for a thing drawn whose middle is `at`, the
/// most important first, the rest none.
fn vertex_lights(all: &[VertexLight], at: Vec3) -> ([Vec4; 8], [Vec4; 8], [Vec4; 8], [Vec4; 8]) {
    let mut order: Vec<&VertexLight> = all.iter().collect();
    order.sort_by(|a, b| b.importance(at).total_cmp(&a.importance(at)));
    let mut out = ([Vec4::ZERO; 8], [Vec4::ZERO; 8], [Vec4::new(-1.0, 1.0, 0.0, 1.0); 8], [Vec4::ZERO; 8]);
    for (i, l) in order.into_iter().take(8).enumerate() {
        out.0[i] = l.position;
        out.1[i] = l.color;
        out.2[i] = l.atten;
        out.3[i] = l.spot;
    }
    out
}

/// An effect made ready.
struct ReadyEffect {
    systems: Vec<ReadySystem>,
    copies: usize,
}

/// An effect's copies, as its mediator keeps them: each one's systems once
/// it has been played, and the one played next.
struct Copies {
    effect: Arc<ReadyEffect>,
    shown: Vec<Option<Vec<ParticleSystem>>>,
    next: usize,
}

/// The effects read, and their copies; the range in the game's physics
/// engine their particles meet, made at the first blast; the entries'
/// older emitters, the systems they emit into (shared, as the effects'
/// objects are), each adapter's draws, and the pool's lights on.
#[derive(Resource)]
pub struct Explosions {
    effects: HashMap<String, Arc<ReadyEffect>>,
    copies: HashMap<String, Copies>,
    rng: Rng,
    engine: Option<trp_physics::World>,
    entries: HashMap<String, ReadyEntry>,
    shared: Vec<(ReadySystem, ParticleSystem)>,
    adapters: Vec<trp_sim::emitters::AdapterDraws>,
    pool: Vec<trp_sim::emitters::PoolLight>,
    blasts: Vec<MuzzleBlast>,
    shots_seen: u32,
    fast: trp_sim::unity_random::FastRandom,
    unity: trp_sim::unity_random::UnityRandom,
}

/// An entry's older emitters and its light (grenades spec, "The
/// flashbang's emitters and the pool's light").
struct ReadyEntry {
    emitters: Vec<ReadyEmitter>,
    shift: f32,
    light: Option<trp_data::particles::PoolLight>,
}

struct ReadyEmitter {
    distance: [f32; 2],
    way: i64,
    min_count: i64,
    count_range: i64,
    scale: Option<[f32; 3]>,
    kind: ReadyKind,
}

enum ReadyKind {
    /// Into a shared system (its place in `shared`), by an adapter (its
    /// place in `adapters`), with its turn and colour.
    System { shared: usize, adapter: usize, rotation_random: bool, rotation: f32, color: [u8; 4] },
    /// Into the shared sparks (`particles_view::Sparks`).
    Sparks(Arc<trp_data::particles::SparksAdapter>),
}

/// A light of this frame: a lights module's or the pool's.
struct FrameLight {
    position: [f32; 3],
    color: [f32; 3],
    range: f32,
    intensity: f32,
    shadows: bool,
}

/// The game's physics engine as the world the particles meet.
struct Engine<'a>(&'a trp_physics::World);

impl trp_sim::particles::World for Engine<'_> {
    fn sweep_sphere(&self, origin: [f32; 3], dir: [f32; 3], distance: f32, radius: f32) -> Option<(f32, [f32; 3])> {
        self.0.sweep_sphere(origin, dir, distance, radius)
    }
}

/// One of the explosions' lights (the lights module's), as a Bevy point
/// light: one of a pool kept for them, hidden when not in use.
#[derive(Component)]
pub struct ExplosionLight;

/// A material's figures as each shader takes them (`particle_effects.wgsl`).
fn figures_of(look: Look, m: &trp_data::particles::ParticleMaterial) -> EffectUniform {
    let f = |name: &str, or: f32| m.float(name).unwrap_or(or);
    let tint = |name: &str| Vec4::from(m.color(name).unwrap_or([1.0; 4]));
    let st = Vec4::from(m.scale("_MainTex").unwrap_or([1.0, 1.0, 0.0, 0.0]));
    let (tint, a, b) = match look {
        Look::SmokeDistorted => (tint("_TintColor"), Vec4::new(f("_AnimationSpeed", 0.0), f("_Distortion", 0.0), f("_InvFade", 1.0), f("_Ambient", 0.0)), Vec4::ZERO),
        Look::SmokeBumped => (tint("_Color"), Vec4::new(f("_AnimationSpeed", 0.0), f("_Distortion", 0.0), f("_NoiseScale", 1.0), f("_InvFade", 1.0)), Vec4::new(f("_NearCameraFade", 0.0), f("_GIIndirect", 1.0), f("_Scaterring", 0.0), 0.0)),
        Look::SuperAdditive => (Vec4::ONE, Vec4::new(f("_Energy", 1.0), 0.0, 0.0, 0.0), Vec4::ZERO),
        Look::CutoutBumped => (tint("_Color"), Vec4::new(f("_Cutoff", 0.5), 0.0, 0.0, 0.0), Vec4::from(m.scale("_BumpMap").unwrap_or([1.0, 1.0, 0.0, 0.0]))),
        // The shaders' own defaults where the material has none.
        Look::Wave => (Vec4::ONE, Vec4::new(f("_BumpAmt", 10.0), f("_FresnelPow", 5.0), f("_FresnelR0", 0.04), 0.0), Vec4::ZERO),
        Look::Heat => (Vec4::ONE, Vec4::new(f("_BumpAmt", 10.0), f("_InvFade", 0.5), 0.0, 0.0), Vec4::ZERO),
        Look::AdditiveSimple => (Vec4::ONE, Vec4::ZERO, Vec4::ZERO),
        Look::LegacyAdditive | Look::HdrFire | Look::HslAdditive | Look::FogDust => {
            let first = if look == Look::HdrFire { f("_HDRAmount", 1.0) } else { f("_InvFade", 1.0) };
            let b = if look == Look::HslAdditive { Vec4::from(m.color("_HueRGB").unwrap_or([0.0; 4])) } else { Vec4::ZERO };
            // The level's `_EFT_Ambient` and `_LocalMinimalAmbientLight`
            // come with the light (`effect_light`).
            (tint("_TintColor"), Vec4::new(first, 0.0, 0.0, 0.0), b)
        }
        Look::SmokeGrenade => (tint("_TintColor"), Vec4::new(f("_AnimationSpeed", 1.0), f("_Distortion", 1.0), f("_InvFade", 1.0), f("_Indoor", 0.0)), Vec4::new(f("_AddAmbient", 0.0), 0.0, 0.0, 0.0)),
        Look::AdditiveVertLit => (tint("_TintColor"), Vec4::new(f("_InvFade", 1.0), f("_MinAmbient", 0.2), 0.0, 0.0), Vec4::ZERO),
        Look::AlphaBlended => (tint("_TintColor"), Vec4::new(f("_InvFade", 1.0), 0.0, 0.0, 0.0), Vec4::ZERO),
        // Its tint blended from `_NightTintColor` toward `_TintColor` by the
        // mean of `TOD_SunCloudColor`, which nothing of the game's sets: the
        // night's.
        Look::WaterSplash => (tint("_NightTintColor"), Vec4::new(f("_InvFade", 1.0), 0.0, 0.0, 0.0), Vec4::ZERO),
        Look::AdditiveSoft | Look::Multiply | Look::MultiplyDouble => (Vec4::ONE, Vec4::new(f("_InvFade", 1.0), 0.0, 0.0, 0.0), Vec4::ZERO),
        Look::TransparentDiffuse => (tint("_Color"), Vec4::ZERO, Vec4::ZERO),
        Look::BumpedGlass => (
            tint("_ReflectColor"),
            Vec4::new(f("_FresPow", 1.0), f("_FresAdd", 0.0), f("_Specular", 1.0), f("_Gloss", 0.0)),
            Vec4::from(m.scale("_BumpMap").unwrap_or([1.0, 1.0, 0.0, 0.0])),
        ),
        Look::StandardFade | Look::StandardOpaque => {
            let emission = if m.keywords.iter().any(|k| k == "_EMISSION") { Vec4::from(m.color("_EmissionColor").unwrap_or([0.0; 4])) } else { Vec4::ZERO };
            (tint("_Color"), Vec4::new(f("_Metallic", 0.0), f("_Glossiness", 0.5), f("_Mode", 0.0), 0.0), emission)
        }
    };
    let (c, d) = match look {
        Look::SmokeGrenade => (Vec4::from(m.color("_IndoorTintColor").unwrap_or([0.5; 4])), Vec4::from(m.color("_TODAttenuation").unwrap_or([1.0; 4]))),
        Look::BumpedGlass => (Vec4::new(f("_Shininess", 0.078125), f("_GlobalReflectionStrength", 0.0), 0.0, 0.0), Vec4::from(m.color("_SpecColor").unwrap_or([0.5; 4]))),
        _ => (Vec4::ZERO, Vec4::ZERO),
    };
    EffectUniform { tint, st, a, b, c, d, lit: Vec4::new(look.vertex_lit(), 0.0, 0.0, 0.0), ..default() }
}

/// A system made ready to run and draw: its shader, pictures and figures,
/// where it is from its effect's root, and what its collisions set off.
#[allow(clippy::too_many_arguments)]
fn ready_system(
    name: &str,
    data: ParticleSystemData,
    local: Placement,
    sub_emitter: bool,
    sets_off: Vec<(usize, bool, f32)>,
    images: &mut Assets<Image>,
    pictures: &mut HashMap<(String, u32, u32), Handle<Image>>,
    white: &Handle<Image>,
) -> ReadySystem {
    let m = &data.renderer.material;
    let look = Look::of_material(m);
    if !data.renderer.enabled {
        info!("{name}: {}'s renderer is off: not drawn", data.name);
    } else if look.is_none() && data.renderer.render_mode != 5 {
        info!("{name}: {} is drawn by {}, not built: left out", data.name, m.shader);
    }
    for module in &data.unbuilt {
        info!("{name}: {}'s {module} is on and not built: left out", data.name);
    }
    if sub_emitter {
        info!("{name}: {} is another system's sub-emitter: not played with the effect", data.name);
    }
    // A cube where the material has one (`_Cube`), else a white one.
    let cube = match m.texture("_Cube").filter(|t| t.faces == 6) {
        Some(t) => match pictures.get(&(t.name.clone(), t.width, t.height)) {
            Some(made) => made.clone(),
            None => {
                let made = crate::particles_view::cube_picture(t, images).unwrap_or_else(|| crate::particles_view::white_cube(images));
                pictures.insert((t.name.clone(), t.width, t.height), made.clone());
                made
            }
        },
        None => match pictures.get(&("<white cube>".to_owned(), 1, 1)) {
            Some(made) => made.clone(),
            None => {
                let made = crate::particles_view::white_cube(images);
                pictures.insert(("<white cube>".to_owned(), 1, 1), made.clone());
                made
            }
        },
    };
    let mut picture_of = |field: &str| -> Handle<Image> {
        let Some(t) = m.texture(field) else { return white.clone() };
        pictures.entry((t.name.clone(), t.width, t.height)).or_insert_with(|| crate::particles_view::picture(t, images).unwrap_or_else(|| white.clone())).clone()
    };
    let drawn_pictures = [picture_of("_MainTex"), picture_of("_NoiseTex"), picture_of("_BumpMap"), cube.clone()];
    let figures = look.map(|look| figures_of(look, m)).unwrap_or_default();
    // Its trails, by the renderer's second material.
    let trail = match (&data.trails, &data.renderer.trail_material) {
        (Some(_), Some(tm)) => match Look::of_material(tm) {
            Some(trail_look) => {
                if (tm.float("_BlendSrc"), tm.float("_BlendDst")) != (Some(5.0), Some(10.0)) && trail_look == Look::AdditiveVertLit {
                    info!("{name}: {}'s trails' blend {:?} is not built: drawn as 5, 10", data.name, (tm.float("_BlendSrc"), tm.float("_BlendDst")));
                }
                let mut trail_picture = |field: &str| -> Handle<Image> {
                    let Some(t) = tm.texture(field) else { return white.clone() };
                    pictures.entry((t.name.clone(), t.width, t.height)).or_insert_with(|| crate::particles_view::picture(t, images).unwrap_or_else(|| white.clone())).clone()
                };
                let trail_pictures = [trail_picture("_MainTex"), trail_picture("_NoiseTex"), trail_picture("_BumpMap"), cube.clone()];
                Some(ReadyTrail { look: trail_look, figures: figures_of(trail_look, tm), pictures: trail_pictures, queue: tm.render_queue })
            }
            None => {
                info!("{name}: {}'s trails are drawn by {}, not built: left out", data.name, tm.shader);
                None
            }
        },
        (Some(_), None) => {
            info!("{name}: {}'s trails have no material: left out", data.name);
            None
        }
        _ => None,
    };
    ReadySystem {
        trail,
        local,
        look: look.filter(|_| data.renderer.enabled),
        pictures: drawn_pictures,
        figures,
        queue: m.render_queue,
        fudge: data.renderer.sorting_fudge,
        render_mode: data.renderer.render_mode,
        sort_mode: data.renderer.sort_mode,
        sub_emitter,
        sets_off,
        data: Arc::new(data),
    }
}

/// The effects the database's grenades name, read at the start.
pub fn read_explosions(mut commands: Commands, mut game: ResMut<crate::GameData>, mut images: ResMut<Assets<Image>>) {
    // The grenades', not their classes' (`ThrowWeap` names one no grenade
    // has).
    let classes: std::collections::HashSet<&str> = game.db.items.values().map(|item| item.parent.as_str()).collect();
    let mut names: Vec<String> = game
        .db
        .items
        .values()
        .filter(|item| !classes.contains(item.id.as_str()))
        .filter_map(|item| item.props.get("ExplosionEffectType")?.as_str().map(str::to_owned))
        .filter(|name| !name.is_empty())
        .collect();
    // And the bullets' hits' entries (the effects' list's entries that name
    // materials), whose older emitters are of the same kind.
    let impact_names: Vec<String> = trp_data::particles::material_entries(&mut game.unity).map(|all| all.into_iter().map(|e| e.name).collect()).unwrap_or_default();
    names.extend(impact_names.iter().cloned());
    names.sort();
    names.dedup();
    let mut fast = trp_sim::unity_random::FastRandom::new();
    let white = images.add(Image::new_fill(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    ));
    let mut pictures: HashMap<(String, u32, u32), Handle<Image>> = HashMap::new();
    let mut effects = HashMap::new();
    let mut entries = HashMap::new();
    let mut shared: Vec<(ReadySystem, ParticleSystem)> = Vec::new();
    let mut shared_of: HashMap<trp_data::unity::ObjRef, usize> = HashMap::new();
    let mut adapters = Vec::new();
    let mut adapter_of: HashMap<trp_data::unity::ObjRef, usize> = HashMap::new();
    let mut rng = Rng::new(0xb1a5);
    for name in names {
        // The entry's older emitters and its light (the flashbang's).
        match trp_data::particles::effect_entry(&mut game.unity, &name) {
            Ok(entry) if !entry.emitters.is_empty() || entry.light.is_some() => {
                let mut emitters = Vec::new();
                for e in entry.emitters {
                    let kind = match e.kind {
                        trp_data::particles::EmitterKind::System(a) => {
                            let shared_index = *shared_of.entry(a.system).or_insert_with(|| {
                                // The effects' object at the world's origin
                                // (`PROVISIONAL`, as the haze system's).
                                let ready = ready_system(&name, a.data.clone(), Placement::default(), false, Vec::new(), &mut images, &mut pictures, &white);
                                let system = ParticleSystem::seeded(ready.data.clone(), (rng.unit() * u32::MAX as f32) as u64);
                                shared.push((ready, system));
                                shared.len() - 1
                            });
                            let adapter = *adapter_of.entry(a.adapter).or_insert_with(|| {
                                adapters.push(trp_sim::emitters::AdapterDraws::new(a.lifetime, a.speed, a.size, &mut fast));
                                adapters.len() - 1
                            });
                            ReadyKind::System { shared: shared_index, adapter, rotation_random: a.rotation_random, rotation: a.rotation, color: a.color }
                        }
                        trp_data::particles::EmitterKind::Sparks(s) => ReadyKind::Sparks(Arc::new(s)),
                    };
                    emitters.push(ReadyEmitter { distance: e.distance, way: e.way, min_count: e.min_count, count_range: e.count_range, scale: e.random_scale, kind });
                }
                info!("the explosion {name}: {} older emitters, the pool's light {:?}", emitters.len(), entry.light);
                entries.insert(name.clone(), ReadyEntry { emitters, shift: entry.particles_shift, light: entry.light });
            }
            Ok(_) => {}
            Err(e) => info!("the explosion {name}'s entry is not read: {e:#}"),
        }
        if impact_names.contains(&name) {
            // A hit's: no mediator's effect.
            continue;
        }
        let effect: ParticleEffect = match trp_data::particles::named_effect(&mut game.unity, &name) {
            Ok(effect) => effect,
            Err(e) => {
                // The flash's (`Flashbang`) is its older emitters' alone; a
                // smoke grenade's is its own.
                info!("the explosion {name} has no mediator's effect: {e:#}");
                continue;
            }
        };
        let objects: Vec<trp_data::unity::ObjRef> = effect.systems.iter().map(|s| s.system).collect();
        let mut systems = Vec::new();
        for s in effect.systems {
            // Its sub-emitters set off by a collision; the others not built.
            let mut sets_off = Vec::new();
            for e in &s.data.sub_emitters {
                match (e.kind, e.system.and_then(|o| objects.iter().position(|&x| x == o))) {
                    (1, Some(target)) => sets_off.push((target, e.inherit & 16 != 0, e.probability)),
                    (kind, _) => info!("{name}: {}'s sub-emitter of kind {kind} is not built: left out", s.data.name),
                }
            }
            let local = Placement { position: s.position, rotation: s.rotation, scale: s.scale };
            systems.push(ready_system(&name, s.data, local, s.sub_emitter, sets_off, &mut images, &mut pictures, &white));
        }
        info!("the explosion {name}: {} systems", systems.len());
        effects.insert(name, Arc::new(ReadyEffect { systems, copies: effect.copies.max(1) }));
    }
    // The muzzles' shared blasts (a launcher's: firing spec, "A launcher's
    // blasts").
    let mut blasts = Vec::new();
    match trp_data::particles::muzzle_blasts(&mut game.unity) {
        Ok(read) => {
            for (kind, effect) in read {
                let name = effect.name.clone();
                let objects: Vec<trp_data::unity::ObjRef> = effect.systems.iter().map(|s| s.system).collect();
                let systems: Vec<ReadySystem> = effect
                    .systems
                    .into_iter()
                    .map(|s| {
                        let local = Placement { position: s.position, rotation: s.rotation, scale: s.scale };
                        ready_system(&name, s.data, local, s.sub_emitter, Vec::new(), &mut images, &mut pictures, &white)
                    })
                    .collect();
                let lights = effect
                    .lights
                    .iter()
                    .filter_map(|l| {
                        let driver = l.driver?;
                        Some(DrivenLight {
                            color: [l.color[0], l.color[1], l.color[2]],
                            range: l.range,
                            local: l.position,
                            system: objects.iter().position(|&o| o == driver.system),
                            min_max: driver.min_max,
                            time: driver.time,
                            intensity: l.intensity,
                            velocity: 0.0,
                        })
                    })
                    .collect::<Vec<_>>();
                info!("the muzzles' blast {name} ({}): {} systems, {} driven lights", if kind == 0 { "front" } else { "back" }, systems.len(), lights.len());
                let ready = Arc::new(ReadyEffect { systems, copies: 1 });
                let running = ready.systems.iter().map(|s| ParticleSystem::seeded(s.data.clone(), (rng.unit() * u32::MAX as f32) as u64)).collect();
                blasts.push(MuzzleBlast { kind, effect: ready, systems: running, lights, root: Placement::default() });
            }
        }
        Err(e) => info!("the muzzles' blasts are not drawn: {e:#}"),
    }
    let unity = trp_sim::unity_random::UnityRandom::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as i32));
    commands.insert_resource(Explosions { effects, copies: HashMap::new(), rng, engine: None, entries, shared, adapters, pool: Vec::new(), blasts, shots_seen: 0, fast, unity });
}

/// An entry's older emitters played (`Effect.Emit`'s particles): each
/// within its distances from the camera, its count and ways drawn from the
/// game's tables, from `at` (the place moved along the normal by the
/// entry's shift); the particles each gave.
#[allow(clippy::too_many_arguments)]
fn emit_entry(
    entry: &ReadyEntry,
    at: [f32; 3],
    normal: [f32; 3],
    distance: f32,
    fast: &mut trp_sim::unity_random::FastRandom,
    unity: &mut trp_sim::unity_random::UnityRandom,
    shared: &mut [(ReadySystem, ParticleSystem)],
    adapters: &mut [trp_sim::emitters::AdapterDraws],
    mut sparks: Option<&mut crate::particles_view::Sparks>,
    now: f32,
) -> usize {
    let at = [at[0] + normal[0] * entry.shift, at[1] + normal[1] * entry.shift, at[2] + normal[2] * entry.shift];
    let mut emitted = 0;
    for e in entry.emitters.iter().filter(|e| trp_sim::impacts::within(distance, e.distance)) {
        for _ in 0..trp_sim::emitters::count(e.min_count, e.count_range, fast) {
            let (way, lift) = trp_sim::emitters::way(e.way, normal, fast);
            let way = trp_sim::emitters::scaled(way, e.scale);
            let place = [at[0], at[1] + lift, at[2]];
            match &e.kind {
                ReadyKind::System { shared: index, adapter, rotation_random, rotation, color } => {
                    let params = adapters[*adapter].emit(place, way, *rotation_random, *rotation, *color);
                    emitted += usize::from(shared[*index].1.emit(params));
                }
                ReadyKind::Sparks(adapter) => {
                    if let Some(sparks) = sparks.as_mut() {
                        sparks.emit(trp_sim::emitters::spark(adapter, place, way, now, unity));
                        emitted += 1;
                    }
                }
            }
        }
    }
    emitted
}

/// The frame's bullets' hits played (`Effects.Update` playing `Effect.Emit`
/// for each queued, ten a frame: `bullets`): where seen, the material's
/// entry's older emitters along the normal and its light 5 cm out within
/// its distance. The decal is `decal_view`'s, the sound `impact_sound`'s.
pub fn impacts(
    bullets: Res<crate::bullets::Bullets>,
    time: Res<Time>,
    mut explosions: ResMut<Explosions>,
    eyes: Query<&GlobalTransform, With<crate::MainCamera>>,
    mut sparks: Option<ResMut<crate::particles_view::Sparks>>,
) {
    if bullets.effects.is_empty() {
        return;
    }
    let Explosions { entries, shared, adapters, pool, fast, unity, .. } = &mut *explosions;
    let eye = eyes.iter().next().map_or(Vec3::ZERO, |e| e.translation());
    let eye = [eye.x, eye.y, -eye.z];
    for (impact, entry) in &bullets.effects {
        if !impact.seen {
            continue;
        }
        let Some(name) = bullets.entries.get(*entry).map(|e| e.name.as_str()) else { continue };
        let Some(ready) = entries.get(name) else { continue };
        let p = impact.position;
        let distance = ((p[0] - eye[0]).powi(2) + (p[1] - eye[1]).powi(2) + (p[2] - eye[2]).powi(2)).sqrt();
        let emitted = emit_entry(ready, p, impact.normal, distance, fast, unity, shared, adapters, sparks.as_deref_mut(), time.elapsed_secs());
        if let Some(light) = ready.light.filter(|l| distance < l.max_distance) {
            let n = impact.normal;
            pool.push(trp_sim::emitters::PoolLight {
                position: [p[0] + n[0] * 0.05, p[1] + n[1] * 0.05, p[2] + n[2] * 0.05],
                color: light.color,
                range: light.range,
                intensity: light.intensity,
                time: light.time,
                shadows: light.shadows,
                age: 0.0,
            });
        }
        debug!("a hit on {name}: {emitted} particles {distance:.1} m off");
    }
}

/// Each grenade's blast plays its effect where the grenade was, its z axis
/// up (`EmitGrenade(..., position, Vector3.up)`, the mediator turning it to
/// `LookRotation(normal)`): the mediator's next copy moved there and played
/// with all its systems. A system of that copy still playing is only moved:
/// `Play` does nothing to a system already playing (the 2022.3 scripting
/// reference; the engine's `Play` resets a system only when stopped).
/// `PROVISIONAL`: one with particles alive after its emission ended taken as
/// still playing. A system another sets off (a sub-emitter) is not played
/// with the rest: the engine marks it so that it never emits on its own
/// (`ParticleSystem::MarkAllSubEmitters`, `IsEmitting`; grenades spec).
#[allow(clippy::too_many_arguments)]
pub fn explode(
    mut blasts: MessageReader<crate::loose::Blasted>,
    game: Res<crate::GameData>,
    frame: Res<bevy::diagnostic::FrameCount>,
    time: Res<Time>,
    mut explosions: ResMut<Explosions>,
    range: Res<crate::range::Range>,
    loose: Option<Res<crate::loose::Loose>>,
    eyes: Query<&GlobalTransform, With<crate::MainCamera>>,
    mut sparks: Option<ResMut<crate::particles_view::Sparks>>,
) {
    let Explosions { effects, copies, rng, engine, entries, shared, adapters, pool, fast, unity, .. } = &mut *explosions;
    for blasted in blasts.read() {
        // Its entry's older emitters, each within its distances from the
        // camera, and its light (`Effect.Emit`; the normal up).
        let Some(name) = game.db.items.get(&blasted.template).and_then(|item| item.props.get("ExplosionEffectType")?.as_str().map(str::to_owned)) else { continue };
        if let Some(entry) = entries.get(&name) {
            let eye = eyes.iter().next().map_or(Vec3::ZERO, |e| e.translation());
            let distance = (Vec3::new(eye.x, eye.y, -eye.z) - Vec3::from(blasted.at)).length();
            let up = [0.0, 1.0, 0.0];
            let emitted = emit_entry(entry, blasted.at, up, distance, fast, unity, shared, adapters, sparks.as_deref_mut(), time.elapsed_secs());
            if let Some(light) = entry.light.filter(|l| distance < l.max_distance) {
                pool.push(trp_sim::emitters::PoolLight {
                    position: [blasted.at[0], blasted.at[1] + 0.05, blasted.at[2]],
                    color: light.color,
                    range: light.range,
                    intensity: light.intensity,
                    time: light.time,
                    shadows: light.shadows,
                    age: 0.0,
                });
            }
            info!("the explosion {name}'s older emitters: {emitted} particles {distance:.1} m off, the pool's light {}", entry.light.is_some_and(|l| distance < l.max_distance));
        }
        // The range in the game's engine, once: its ground and boxes, as
        // the things thrown meet it (stepped once, for its queries).
        if engine.is_none() {
            if let Some(physics) = loose.as_ref().and_then(|l| l.physics()) {
                let mut world = crate::loose::range_world(physics, &range);
                world.step(1.0 / 60.0);
                *engine = Some(world);
            }
        }
        let Some(effect) = effects.get(&name) else { continue };
        let copies = copies.entry(name.clone()).or_insert_with(|| Copies { effect: effect.clone(), shown: (0..effect.copies).map(|_| None).collect(), next: 0 });
        let index = copies.next;
        copies.next = (index + 1) % copies.shown.len();
        // Up: a quarter turn about x, taking z to y.
        let root = Placement { position: blasted.at, rotation: [-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2], scale: [1.0; 3] };
        let systems = copies.shown[index].get_or_insert_with(|| {
            copies.effect.systems.iter().map(|s| ParticleSystem::seeded(s.data.clone(), (rng.unit() * u32::MAX as f32) as u64)).collect()
        });
        for (system, ready) in systems.iter_mut().zip(&copies.effect.systems) {
            let placement = compose(root, ready.local);
            if ready.sub_emitter {
                system.placement = placement;
            } else if system.alive() {
                system.placement = placement;
            } else {
                system.play(placement);
            }
        }
        info!("the explosion {name} (copy {}) at {:?} on frame {}", index + 1, blasted.at, frame.0);
    }
}

/// The time of day's harmonics, packed, with `CUSTOMS LIGHT`; none
/// without it (`ESTIMATE`: the flat ambient's constant alone).
fn effect_harmonics(level: Option<&crate::lighting::LevelLighting>) -> [Vec4; 7] {
    match level {
        Some(l) => {
            let p = l.harmonics.packed();
            [p.a[0], p.a[1], p.a[2], p.b[0], p.b[1], p.b[2], p.c].map(Vec4::from_array)
        }
        None => {
            let mut sh = [Vec4::ZERO; 7];
            for (i, row) in sh.iter_mut().take(3).enumerate() {
                row[3] = [AMBIENT; 3][i];
            }
            sh
        }
    }
}

/// The lights module's lights this frame as Bevy's point lights, from a
/// pool kept for them (more made as needed, the rest hidden). Each as the
/// hall's lamps are given (`lighting::hall_lamp`): its colour the linear of
/// the game's, and its power such that at a fifth of its range (where the
/// engine's fall-off has halved it) a surface square to it takes what the
/// game's would. `PROVISIONAL`: elsewhere Bevy's fall-off with the square of
/// the distance; the effects' own particles are not lit by them (the
/// game's G-buffer things are: the concrete). With the game's shading
/// (`shading`), each as the game gives it: its colour, its intensity
/// (times 4π, which Bevy divides by) and its range, the engine's fall-off.
fn place_lights(commands: &mut Commands, pool: &mut Query<(&mut PointLight, &mut Transform, &mut Visibility), With<ExplosionLight>>, lights: &[FrameLight]) {
    let lamp = |l: &FrameLight| {
        let reach = 0.2 * l.range;
        let at = l.intensity / (1.0 + 25.0 * 0.2 * 0.2);
        let pi = std::f32::consts::PI;
        let (color, intensity) = if crate::shading::on() {
            (Color::linear_rgb(l.color[0], l.color[1], l.color[2]), 4.0 * pi * l.intensity)
        } else {
            (
                Color::linear_rgb(crate::lighting::linear(l.color[0]), crate::lighting::linear(l.color[1]), crate::lighting::linear(l.color[2])),
                4.0 * pi * pi * crate::lighting::EXPOSURE_SCALE * reach * reach * crate::lighting::linear(at),
            )
        };
        PointLight {
            color,
            intensity,
            range: l.range,
            radius: 0.0,
            // The pool's light with shadows where its entry asks (hard, the
            // game's: `PROVISIONAL`, Bevy's own soft filtering).
            shadows_enabled: l.shadows,
            ..default()
        }
    };
    let mut wanted = lights.iter();
    for (mut light, mut transform, mut visibility) in pool.iter_mut() {
        match wanted.next() {
            Some(l) => {
                *light = lamp(l);
                transform.translation = crate::convert::position(l.position);
                *visibility = Visibility::Inherited;
            }
            None => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
    for l in wanted {
        commands.spawn((lamp(l), Transform::from_translation(crate::convert::position(l.position)), Visibility::Inherited, ExplosionLight));
    }
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A placement under another.
fn compose(parent: Placement, local: Placement) -> Placement {
    let q = Quat::from_array(parent.rotation);
    let at = q * (Vec3::from(local.position) * Vec3::from(parent.scale));
    Placement {
        position: (Vec3::from(parent.position) + at).to_array(),
        rotation: (q * Quat::from_array(local.rotation)).to_array(),
        scale: (Vec3::from(parent.scale) * Vec3::from(local.scale)).to_array(),
    }
}

/// A frame, once everything is placed: each copy's systems that still play
/// stepped once, and each one's corners handed to every camera that draws
/// them, made for that camera in the engine's order (`camera_draws`).
#[allow(clippy::too_many_arguments)]
pub fn explosion_frame(
    mut commands: Commands,
    time: Res<Time>,
    mut explosions: ResMut<Explosions>,
    suns: Query<&GlobalTransform, With<crate::lighting::RangeSun>>,
    level: Option<Res<crate::lighting::LevelLighting>>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut ExplosionScreen)>,
    mut pool: Query<(&mut PointLight, &mut Transform, &mut Visibility), With<ExplosionLight>>,
    rig: Option<Res<crate::weapon::WeaponRig>>,
    controller: Res<crate::controller::Controller>,
    places: Query<&GlobalTransform>,
    lamps: Query<&GlobalTransform, With<SpotLight>>,
) {
    let dt = time.delta_secs();
    let mut shown = false;
    let mut lights = Vec::new();
    let Explosions { copies, rng, engine, shared, pool: pool_lights, blasts, shots_seen, .. } = &mut *explosions;
    let world = engine.as_ref().map(Engine);
    // A shot of the weapon in hand plays, at each of its particle pivots,
    // the shared blast of that pivot's kind: moved there, its systems
    // stopped and played again (`IMuzzleParticlePivot.Play`).
    let shots = controller.shots_fired.wrapping_sub(*shots_seen);
    *shots_seen = controller.shots_fired;
    if shots > 0 {
        if let Some(parts) = rig.as_ref().and_then(|rig| rig.muzzle.as_ref()) {
            for &(entity, kind) in &parts.pivots {
                let Some(place) = entity.and_then(|e| places.get(e).ok()) else { continue };
                let Some(blast) = blasts.iter_mut().find(|b| b.kind == kind) else { continue };
                let (_, turn, at) = place.to_scale_rotation_translation();
                blast.root = Placement { position: [at.x, at.y, -at.z], rotation: crate::convert::rotation_back(turn), scale: [1.0; 3] };
                for (system, ready) in blast.systems.iter_mut().zip(&blast.effect.systems) {
                    system.play(compose(blast.root, ready.local));
                }
                info!("the muzzles' blast of kind {kind} played at {:?}", blast.root.position);
            }
        }
    }
    // Each blast's systems stepped; its driven lights eased toward their
    // system's playing (`MuzzleParticleLight`), lit while above nought.
    for blast in blasts.iter_mut() {
        for system in blast.systems.iter_mut().filter(|s| s.alive()) {
            system.step_in(dt, world.as_ref().map(|w| w as &dyn trp_sim::particles::World));
            shown |= !system.particles.is_empty();
        }
        for light in &mut blast.lights {
            let playing = light.system.and_then(|i| blast.systems.get(i)).is_some_and(ParticleSystem::alive);
            let target = if playing { light.min_max[1] } else { light.min_max[0] };
            light.intensity = trp_sim::particles::smooth_damp(light.intensity, target, &mut light.velocity, dt * light.time, dt);
            if (light.intensity - target).abs() < 0.01 {
                light.intensity = target;
            }
            if light.intensity > 0.0 {
                let at = compose(blast.root, Placement { position: light.local, rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] }).position;
                lights.push(FrameLight { position: at, color: light.color, range: light.range, intensity: light.intensity, shadows: false });
            }
        }
        for system in blast.systems.iter() {
            lights.extend(system.lights().into_iter().map(|l| FrameLight { position: l.position, color: l.color, range: l.range, intensity: l.intensity, shadows: false }));
        }
    }
    // The shared systems the older emitters emit into, each stepped in the
    // world while it has particles; the pool's lights, each falling.
    for (_, system) in shared.iter_mut().filter(|(_, s)| s.alive()) {
        system.step_in(dt, world.as_ref().map(|w| w as &dyn trp_sim::particles::World));
        shown |= !system.particles.is_empty();
    }
    for light in pool_lights.iter_mut() {
        light.step(dt);
    }
    pool_lights.retain(|l| l.now().is_some());
    for l in pool_lights.iter() {
        if let Some((range, intensity)) = l.now() {
            lights.push(FrameLight { position: l.position, color: [l.color[0], l.color[1], l.color[2]], range, intensity, shadows: l.shadows });
        }
    }
    for copy in copies.values_mut() {
        let effect = copy.effect.clone();
        for systems in copy.shown.iter_mut().flatten() {
            // Each system stepped in the world; what its particles' meetings
            // set off born after.
            let mut set_off: Vec<(usize, [f32; 3], f32)> = Vec::new();
            for (system, ready) in systems.iter_mut().zip(&effect.systems) {
                if !system.alive() {
                    continue;
                }
                let met = system.step_in(dt, world.as_ref().map(|w| w as &dyn trp_sim::particles::World));
                for p in &met {
                    for &(target, takes_duration, chance) in &ready.sets_off {
                        if rng.unit() <= chance {
                            set_off.push((target, system.world_place(p), if takes_duration { system.cycle() } else { 0.0 }));
                        }
                    }
                }
                shown |= !system.particles.is_empty();
            }
            for (target, at, cycle) in set_off {
                if let Some(system) = systems.get_mut(target) {
                    system.emit_at(at, cycle);
                }
            }
            for system in systems.iter() {
                lights.extend(system.lights().into_iter().map(|l| FrameLight { position: l.position, color: l.color, range: l.range, intensity: l.intensity, shadows: false }));
            }
        }
    }
    place_lights(&mut commands, &mut pool, &lights);
    if !shown {
        for (_, _, _, mut screen) in &mut cameras {
            if !screen.0.is_empty() {
                screen.0.clear();
            }
        }
        return;
    }
    // The way to the app's sun.
    let sun = suns.iter().next().map_or(Vec3::Y, |sun| sun.back().as_vec3());
    let light = effect_light(level.as_deref());
    let harmonics = effect_harmonics(level.as_deref());
    // The engine's vertex lights of the frame (for the vertex-lit shaders):
    // the sun, the effects' lights, the hall's lamps.
    let game = crate::convert::position;
    let mut frame_lights: Vec<VertexLight> = Vec::new();
    if let Some(sun_way) = suns.iter().next().map(|s| s.back().as_vec3()) {
        frame_lights.push(VertexLight { position: sun_way.extend(0.0), color: Vec4::new(light[0].x, light[0].y, light[0].z, 1.0), atten: Vec4::new(-1.0, 1.0, 0.0, 1.0), spot: Vec4::ZERO });
    }
    for l in &lights {
        frame_lights.push(VertexLight::point(game(l.position), l.color, l.intensity, l.range));
    }
    for lamp in lamps.iter() {
        let (color, intensity, range, cone) = crate::lighting::hall_lamp_figures();
        let (cos_half, cos_quarter) = ((cone * 0.5).to_radians().cos(), (cone * 0.25).to_radians().cos());
        let mut l = VertexLight::point(lamp.translation(), color, intensity, range);
        l.atten.x = cos_half;
        l.atten.y = 1.0 / (cos_quarter - cos_half).max(1e-3);
        l.spot = (-lamp.forward().as_vec3()).extend(0.0);
        frame_lights.push(l);
    }
    let frame = FrameFigures { light, harmonics, smoke_globals: crate::lighting::smoke_globals(level.as_deref()), frame_lights, sun, time: time.elapsed_secs() };
    for (camera, eye, projection, mut screen) in &mut cameras {
        if !camera.is_active {
            if !screen.0.is_empty() {
                screen.0.clear();
            }
            continue;
        }
        screen.0 = camera_draws(&explosions, &frame, camera, eye, crate::muzzle_view::near_of(projection));
    }
}

/// What a frame's draws share, whichever camera draws them: the light's
/// figures, the time of day's harmonics, the grenade smoke's figures, the
/// engine's vertex lights, the way to the sun, the time.
struct FrameFigures {
    light: [Vec4; 4],
    harmonics: [Vec4; 7],
    smoke_globals: [Vec4; 4],
    frame_lights: Vec<VertexLight>,
    sun: Vec3,
    time: f32,
}

/// The systems' draws for one camera, in the engine's order among
/// transparent things: each system's particles and trails made facing that
/// camera and sorted from it, as the engine makes a particle system's for
/// each camera that draws it (the main camera, and the optic camera while
/// on: the effects' objects are of the world's layer).
fn camera_draws(explosions: &Explosions, frame: &FrameFigures, camera: &Camera, eye: &GlobalTransform, near: f32) -> Vec<SystemDraw> {
    let FrameFigures { light, harmonics, smoke_globals, frame_lights, .. } = frame;
    let (light, harmonics, smoke_globals) = (*light, *harmonics, *smoke_globals);
    let view_from_world = eye.to_matrix().inverse();
    let clip_from_world = camera.clip_from_view() * view_from_world;
    let (right, up, forward) = (eye.right().as_vec3(), eye.up().as_vec3(), eye.forward().as_vec3());
    let facing = -forward;
    // The way to the app's sun, in the view's axes.
    let sun = view_from_world.transform_vector3(frame.sun).normalize_or_zero();
    let mut draws: Vec<(i64, f32, SystemDraw)> = Vec::new();
    let game = crate::convert::position;
    let mediators = explosions.copies.values().flat_map(|copies| copies.shown.iter().flatten().flat_map(|systems| copies.effect.systems.iter().zip(systems)));
    let older = explosions.shared.iter().map(|(ready, system)| (ready, system));
    let launchers = explosions.blasts.iter().flat_map(|b| b.effect.systems.iter().zip(&b.systems));
    {
        for (ready, system) in mediators.chain(older).chain(launchers) {
            // Its trails first, a draw of their own.
            if let Some(trail) = &ready.trail {
                let trails = system.drawn_trails();
                let mut corners: Vec<f32> = Vec::new();
                let mut indices: Vec<u32> = Vec::new();
                let mut middle = Vec3::ZERO;
                let mut count = 0.0f32;
                for points in trails.iter().filter(|t| t.len() >= 2) {
                    let first = (corners.len() / 12) as u32;
                    for (i, p) in points.iter().enumerate() {
                        let at = game(p.position);
                        let (prev, next) = (game(points[i.saturating_sub(1)].position), game(points[(i + 1).min(points.len() - 1)].position));
                        let along = (next - prev).normalize_or_zero();
                        let across = along.cross(eye.translation() - at).normalize_or_zero() * (p.width * 0.5);
                        middle += at;
                        count += 1.0;
                        for (side, v) in [(-1.0f32, 0.0f32), (1.0, 1.0)] {
                            let c = at + across * side;
                            corners.extend_from_slice(&[c.x, c.y, c.z, p.along, v, p.color[0], p.color[1], p.color[2], p.color[3], facing.x, facing.y, facing.z]);
                        }
                    }
                    for i in 0..points.len() as u32 - 1 {
                        let (a, b) = (first + 2 * i, first + 2 * i + 2);
                        indices.extend([a, a + 1, b + 1, a, b + 1, b]);
                    }
                }
                if !indices.is_empty() {
                    middle /= count.max(1.0);
                    let (vl_position, vl_color, vl_atten, vl_spot) = vertex_lights(frame_lights, middle);
                    let uniform = EffectUniform {
                        clip_from_world,
                        view_from_world,
                        camera: Vec4::new(near, frame.time / 20.0, frame.time, 0.0),
                        light: light[0],
                        light_view: sun.extend(0.0),
                        ambient: light[1],
                        probe: light[2],
                        level: light[3],
                        sh: harmonics,
                        smoke_a: smoke_globals[0],
                        smoke_b: smoke_globals[1],
                        smoke_c: smoke_globals[2],
                        smoke_d: smoke_globals[3],
                        vl_position,
                        vl_color,
                        vl_atten,
                        vl_spot,
                        ..trail.figures
                    };
                    let distance = (middle - eye.translation()).length() + ready.fudge;
                    draws.push((trail.queue, distance, SystemDraw { look: trail.look, uniform, pictures: trail.pictures.clone(), corners: Arc::new(corners), indices: Some(Arc::new(indices)) }));
                }
            }
            let Some(look) = ready.look else { continue };
            // A testing aid: the shaders named in `TRP_EFFECTS_LEFT_OUT`
            // (their entries, `distortion_wave,heat_distortion`) not drawn,
            // to set a capture beside one with them.
            if std::env::var("TRP_EFFECTS_LEFT_OUT").is_ok_and(|out| out.split(',').any(|name| name == look.entry())) {
                continue;
            }
            let mesh = ready.data.renderer.mesh.as_ref().filter(|_| ready.render_mode == 4);
            if system.particles.is_empty() || !(matches!(ready.render_mode, 0 | 1) || mesh.is_some()) {
                continue;
            }
            let mut corners = Vec::with_capacity(system.particles.len() * 48);
            let mut indices: Vec<u32> = Vec::new();
            let mut middle = Vec3::ZERO;
            let mut count = 0.0f32;
            // In the renderer's order (`m_SortMode`): as the system keeps
            // them (none: the engine's own order), far to near from the
            // camera, or by the life left (the engine's key for both
            // orders by age): the oldest in front, the youngest in front.
            let mut drawn: Vec<_> = system.drawn().collect();
            match ready.sort_mode {
                1 => drawn.sort_by(|a, b| (game(b.position) - eye.translation()).length_squared().total_cmp(&(game(a.position) - eye.translation()).length_squared())),
                2 => drawn.sort_by(|a, b| b.remaining.total_cmp(&a.remaining)),
                3 => drawn.sort_by(|a, b| a.remaining.total_cmp(&b.remaining)),
                _ => {}
            }
            for d in drawn {
                let at = game(d.position);
                middle += at;
                count += 1.0;
                let [u0, v0, u1, v1] = d.sheet;
                let corner = |p: Vec3, u: f32, v: f32, out: &mut Vec<f32>| {
                    out.extend_from_slice(&[p.x, p.y, p.z, u0 + (u1 - u0) * u, v0 + (v1 - v0) * v, d.color[0], d.color[1], d.color[2], d.color[3], facing.x, facing.y, facing.z]);
                };
                let velocity = game(d.velocity);
                if let Some(mesh) = mesh {
                    // The mesh at the particle, sized by it, turned as the
                    // system is (alignment 2, its own axes) and by the
                    // particle's turn about its z (`PROVISIONAL`: a turn of
                    // three axes not read); in the game's axes, then the
                    // app's.
                    let first = (corners.len() / 12) as u32;
                    let turn = Quat::from_array(system.placement.rotation) * Quat::from_rotation_z(d.rotation.to_radians());
                    for (k, p) in mesh.positions.iter().enumerate() {
                        let local = turn * (Vec3::from_array(*p) * d.size);
                        let place = game(add3(d.position, local.to_array()));
                        let n = mesh.normals.get(k).map_or(Vec3::Y, |n| game((turn * Vec3::from_array(*n)).to_array()));
                        let uv = mesh.uv0.get(k).copied().unwrap_or([0.0, 0.0]);
                        corners.extend_from_slice(&[place.x, place.y, place.z, uv[0], uv[1], d.color[0], d.color[1], d.color[2], d.color[3], n.x, n.y, n.z]);
                    }
                    for submesh in &mesh.submeshes {
                        indices.extend(submesh.iter().map(|i| first + i));
                    }
                } else if ready.render_mode == 1 && velocity.length_squared() > 1e-10 {
                    // Stretched along its way (`PROVISIONAL`: the particle
                    // at the head, its tail behind by its length; its
                    // picture's u along it), its width its size.
                    let r = &ready.data.renderer;
                    let along = velocity.normalize();
                    let length = d.size * r.length_scale + velocity.length() * r.velocity_scale;
                    let across = along.cross(at - eye.translation()).normalize_or_zero() * (d.size * 0.5);
                    let tail = at - along * length;
                    corner(tail - across, 0.0, 0.0, &mut corners);
                    corner(at - across, 1.0, 0.0, &mut corners);
                    corner(at + across, 1.0, 1.0, &mut corners);
                    corner(tail + across, 0.0, 1.0, &mut corners);
                } else {
                    // Facing the view, turned in its plane by its turn.
                    let (s, c) = d.rotation.to_radians().sin_cos();
                    for (x, y) in [(-0.5f32, -0.5f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                        let (rx, ry) = (x * c - y * s, x * s + y * c);
                        corner(at + (right * rx + up * ry) * d.size, x + 0.5, y + 0.5, &mut corners);
                    }
                }
            }
            middle /= count.max(1.0);
            let distance = (middle - eye.translation()).length() + ready.fudge;
            let (vl_position, vl_color, vl_atten, vl_spot) = if look.vertex_lit() > 0.0 { vertex_lights(frame_lights, middle) } else { ([Vec4::ZERO; 8], [Vec4::ZERO; 8], [Vec4::ZERO; 8], [Vec4::ZERO; 8]) };
            let uniform = EffectUniform {
                clip_from_world,
                view_from_world,
                camera: Vec4::new(near, frame.time / 20.0, frame.time, 0.0),
                light: light[0],
                light_view: sun.extend(0.0),
                ambient: light[1],
                probe: light[2],
                level: light[3],
                sh: harmonics,
                smoke_a: smoke_globals[0],
                smoke_b: smoke_globals[1],
                smoke_c: smoke_globals[2],
                smoke_d: smoke_globals[3],
                vl_position,
                vl_color,
                vl_atten,
                vl_spot,
                ..ready.figures
            };
            let indices = mesh.map(|_| Arc::new(indices));
            draws.push((ready.queue, distance, SystemDraw { look, uniform, pictures: ready.pictures.clone(), corners: Arc::new(corners), indices }));
        }
    }
    // By queue, then far to near.
    draws.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
    draws.into_iter().map(|(_, _, d)| d).collect()
}

pub struct ExplosionViewPlugin {
    /// The explosions read and drawn (`PARTICLES`); off, nothing of them is
    /// read, and the pass never draws.
    pub on: bool,
}

impl Plugin for ExplosionViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, EFFECTS_SHADER, "particle_effects.wgsl", Shader::from_wgsl);
        if self.on {
            app.add_plugins(ExtractComponentPlugin::<ExplosionScreen>::default())
                .add_systems(Startup, read_explosions)
                .add_systems(Update, explode.run_if(resource_exists::<Explosions>))
                .add_systems(PostUpdate, explosion_frame.run_if(resource_exists::<Explosions>).after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems).after(crate::optic_view::squeeze_drawn));
        }
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<EffectsPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<ExplosionNode>>(Core3d, ExplosionLabel);
    }
}

/// Its pass, in the camera's effects' order (`gamma_ui`).
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct ExplosionLabel;

#[derive(Resource)]
struct EffectsPipeline {
    layout: BindGroupLayoutDescriptor,
    /// The frame's copy read bilinear, clamped at its edges.
    grab_sampler: Sampler,
}

impl SpecializedRenderPipeline for EffectsPipeline {
    type Key = (Look, TextureFormat);

    fn specialize(&self, (look, format): Self::Key) -> RenderPipelineDescriptor {
        let at = |location: u32, offset: u64, format: VertexFormat| VertexAttribute { format, offset, shader_location: location };
        let (blend, write_mask) = look.blend();
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

fn init_pipeline(mut commands: Commands, device: Res<bevy::render::renderer::RenderDevice>) {
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

/// What a view with explosions needs this frame: each shader's pipeline,
/// and the frame's copy where a system's shader reads the frame.
#[derive(Component)]
struct EffectsParts(HashMap<Look, CachedRenderPipelineId>, Option<bevy::render::texture::CachedTexture>);

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, &ExplosionScreen)>,
    done: Query<Entity, (With<EffectsParts>, Without<ExplosionScreen>)>,
    cache: Res<PipelineCache>,
    pipeline: Res<EffectsPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<EffectsPipeline>>,
    device: Res<bevy::render::renderer::RenderDevice>,
    mut textures: ResMut<bevy::render::texture::TextureCache>,
) {
    for entity in &done {
        commands.entity(entity).remove::<EffectsParts>();
    }
    for (entity, target, screen) in &views {
        let format = target.main_texture_format();
        let ids = screen.0.iter().map(|d| (d.look, pipelines.specialize(&cache, &pipeline, (d.look, format)))).collect();
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
        commands.entity(entity).insert(EffectsParts(ids, grab));
    }
}

#[derive(Default)]
struct ExplosionNode;

impl ViewNode for ExplosionNode {
    type ViewQuery = (&'static ViewTarget, &'static ExplosionScreen, &'static EffectsParts, Option<&'static crate::muzzle_view::JetParts>);

    fn run(&self, _graph: &mut RenderGraphContext, context: &mut RenderContext, (target, screen, parts, muzzle): QueryItem<Self::ViewQuery>, world: &World) -> Result<(), NodeRunError> {
        // The scene's depth as copied this frame (`DepthCopyLabel`).
        let Some(muzzle) = muzzle else { return Ok(()) };
        let pipeline = world.resource::<EffectsPipeline>();
        let cache = world.resource::<PipelineCache>();
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let device = context.render_device().clone();
        for draw in &screen.0 {
            let quads = draw.corners.len() as u32 / 48;
            let Some(look_pipeline) = parts.0.get(&draw.look).and_then(|id| cache.get_render_pipeline(*id)) else { continue };
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
                    &muzzle.depth.default_view,
                    grab_view,
                    &pipeline.grab_sampler,
                    &cube.texture_view,
                    &cube.sampler,
                )),
            );
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("particle_effect"),
                color_attachments: &[Some(RenderPassColorAttachment { view: target.main_texture_view(), depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Load, store: StoreOp::Store } })],
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
        Ok(())
    }
}
