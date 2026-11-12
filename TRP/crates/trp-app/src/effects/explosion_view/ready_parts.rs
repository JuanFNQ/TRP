//! The uniform, the readied systems, trails and lights, and the explosions' state.

use super::*;

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
pub(super) struct ReadySystem {
    pub(super) data: Arc<ParticleSystemData>,
    pub(super) local: Placement,
    pub(super) look: Option<Look>,
    pub(super) pictures: [Handle<Image>; 4],
    pub(super) figures: EffectUniform,
    pub(super) queue: i64,
    pub(super) fudge: f32,
    pub(super) render_mode: i64,
    pub(super) sort_mode: i64,
    /// Set off by another of the effect's systems: never played with the
    /// effect, only placed with it.
    pub(super) sub_emitter: bool,
    /// The systems a collision of its particles sets off: each one's place
    /// among the effect's, whether it takes this system's duration, its
    /// chance.
    pub(super) sets_off: Vec<(usize, bool, f32)>,
    /// Its trails' drawing, where it has trails and a material for them.
    pub(super) trail: Option<ReadyTrail>,
}

/// A system's trails' material made ready: its shader, figures, pictures
/// and queue.
pub(super) struct ReadyTrail {
    pub(super) look: Look,
    pub(super) figures: EffectUniform,
    pub(super) pictures: [Handle<Image>; 4],
    pub(super) queue: i64,
}

/// A muzzle's shared blast (firing spec, "A launcher's blasts"): its kind
/// (0 the front, 1 the back), its systems ready and running, its lights
/// driven by a system's playing, and where it was last played.
pub(super) struct MuzzleBlast {
    pub(super) kind: i64,
    pub(super) effect: Arc<ReadyEffect>,
    pub(super) systems: Vec<ParticleSystem>,
    pub(super) lights: Vec<DrivenLight>,
    pub(super) root: Placement,
}

/// A light of a blast driven by one of its systems (`MuzzleParticleLight`):
/// its colour and range, its place from the blast's root, the system's place
/// among the blast's, the intensities it goes between and its time, and its
/// intensity and the damping's velocity now.
pub(super) struct DrivenLight {
    pub(super) color: [f32; 3],
    pub(super) range: f32,
    pub(super) local: [f32; 3],
    pub(super) system: Option<usize>,
    pub(super) min_max: [f32; 2],
    pub(super) time: f32,
    pub(super) intensity: f32,
    pub(super) velocity: f32,
}

/// A light of the frame as the engine hands its vertex lights to a shader,
/// in the app's axes, with how much it matters (`PROVISIONAL`: the sun
/// first, then by its intensity times its fall-off at the thing drawn).
#[derive(Clone, Copy)]
pub(super) struct VertexLight {
    pub(super) position: Vec4,
    pub(super) color: Vec4,
    pub(super) atten: Vec4,
    pub(super) spot: Vec4,
}

impl VertexLight {
    pub(super) fn point(at: Vec3, color: [f32; 3], intensity: f32, range: f32) -> Self {
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
pub(super) fn vertex_lights(all: &[VertexLight], at: Vec3) -> ([Vec4; 8], [Vec4; 8], [Vec4; 8], [Vec4; 8]) {
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
pub(super) struct ReadyEffect {
    pub(super) systems: Vec<ReadySystem>,
    pub(super) copies: usize,
}

/// An effect's copies, as its mediator keeps them: each one's systems once
/// it has been played, and the one played next.
pub(super) struct Copies {
    pub(super) effect: Arc<ReadyEffect>,
    pub(super) shown: Vec<Option<Vec<ParticleSystem>>>,
    pub(super) next: usize,
}

/// The effects read, and their copies; the range in the game's physics
/// engine their particles meet, made at the first blast; the entries'
/// older emitters, the systems they emit into (shared, as the effects'
/// objects are), each adapter's draws, and the pool's lights on.
#[derive(Resource)]
pub struct Explosions {
    pub(super) effects: HashMap<String, Arc<ReadyEffect>>,
    pub(super) copies: HashMap<String, Copies>,
    pub(super) rng: Rng,
    pub(super) engine: Option<trp_physics::World>,
    pub(super) entries: HashMap<String, ReadyEntry>,
    pub(super) shared: Vec<(ReadySystem, ParticleSystem)>,
    pub(super) adapters: Vec<trp_sim::effects::emitters::AdapterDraws>,
    pub(super) pool: Vec<trp_sim::effects::emitters::PoolLight>,
    pub(super) blasts: Vec<MuzzleBlast>,
    pub(super) shots_seen: [u32; 2],
    pub(super) fast: trp_sim::base::unity_random::FastRandom,
    pub(super) unity: trp_sim::base::unity_random::UnityRandom,
}

/// An entry's older emitters and its light (grenades spec, "The
/// flashbang's emitters and the pool's light").
pub(super) struct ReadyEntry {
    pub(super) emitters: Vec<ReadyEmitter>,
    pub(super) shift: f32,
    pub(super) light: Option<trp_data::drawing::particles::PoolLight>,
}

pub(super) struct ReadyEmitter {
    pub(super) distance: [f32; 2],
    pub(super) way: i64,
    pub(super) min_count: i64,
    pub(super) count_range: i64,
    pub(super) scale: Option<[f32; 3]>,
    pub(super) kind: ReadyKind,
}

pub(super) enum ReadyKind {
    /// Into a shared system (its place in `shared`), by an adapter (its
    /// place in `adapters`), with its turn and colour.
    System { shared: usize, adapter: usize, rotation_random: bool, rotation: f32, color: [u8; 4] },
    /// Into the shared sparks (`particles_view::Sparks`).
    Sparks(Arc<trp_data::drawing::particles::SparksAdapter>),
}

/// A light of this frame: a lights module's or the pool's.
pub(super) struct FrameLight {
    pub(super) position: [f32; 3],
    pub(super) color: [f32; 3],
    pub(super) range: f32,
    pub(super) intensity: f32,
    pub(super) shadows: bool,
}

/// The game's physics engine as the world the particles meet.
pub(super) struct Engine<'a>(pub(super) &'a trp_physics::World);

impl trp_sim::effects::particles::World for Engine<'_> {
    fn sweep_sphere(&self, origin: [f32; 3], dir: [f32; 3], distance: f32, radius: f32) -> Option<(f32, [f32; 3])> {
        self.0.sweep_sphere(origin, dir, distance, radius)
    }

    fn sweep_sphere_still(&self, origin: [f32; 3], dir: [f32; 3], distance: f32, radius: f32) -> Option<(f32, [f32; 3])> {
        self.0.sweep_sphere_still(origin, dir, distance, radius).map(|(d, n, _)| (d, n))
    }
}

/// The rays a frame shares among the particle systems meeting the world at
/// medium and low quality (`QualitySettings.particleRaycastBudget`: the
/// game sets 256 at every one of its quality levels, `QualityLevelPreset`;
/// particles spec, "Meeting the world at medium and low quality").
pub(super) const PARTICLE_RAYS: u32 = 256;

/// One of the explosions' lights (the lights module's), as a Bevy point
/// light: one of a pool kept for them, hidden when not in use.
#[derive(Component)]
pub struct ExplosionLight;

/// A material's figures as each shader takes them (`particle_effects.wgsl`).
pub(super) fn figures_of(look: Look, m: &trp_data::drawing::particles::ParticleMaterial) -> EffectUniform {
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
            // The level's top ambient and `_LocalMinimalAmbientLight`
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
