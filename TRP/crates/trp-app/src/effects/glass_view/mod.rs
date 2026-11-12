//! The see-through glass with the options' `SEE-THROUGH GLASS`.
//! After the body-animator spec, "The shield's glass": every face shield's glass and the window
//! magazines' (`Transparent/DepthZwriteDithered`, known by its shader: `trp_data::glass`) drawn as
//! the game draws it, never blended.
//!
//! - **The glass** (`glass.wgsl`): its material is noted with its figures,
//!   its specular picture and its cube as materials are made (`note`,
//!   `render_data`), and each mesh drawn with it is drawn with a twin of
//!   its standard material: the shader's deferred pass in the game's
//!   numbers, its colour, specular (a rim of its own), smoothness and
//!   emission handed to the game's light (`game_shading.wgsl`'s
//!   `trp_game_light`) with `TARKOV SHADING`, to Bevy's without; where its
//!   alpha is under a half every other pixel of a fixed checkerboard thrown
//!   away and the colour doubled; each kept pixel marked by an alpha of
//!   nought in the frame (the game's mark in its normals' alpha). The glass
//!   casts no shadow (its renderers' shadows off): with the body's
//!   shadow-only wish on (its own player's head, in first person) it draws
//!   nothing at all.
//! - **The camera's undithering** (`undither.wgsl`): on a camera that
//!   draws a glass this frame (`UnditherScreen`), after the ambient
//!   occlusion and the sky's scattering and before the app's transparent
//!   things (the explosions', the muzzle's), on the frame in the game's
//!   numbers: each marked pixel and the one a row over taken as their mean.
//!   On a preview's camera (the edit build screen's, the OVERALL tab's
//!   figure's), whose frame stays linear for its photographic curve, before
//!   its outline and that curve, the figures made the game's numbers for the
//!   mean and back.
//!
//! So the glass is seen through at a fixed half wherever it is thin, opaque
//! where its alpha is a half or more, every edge softened over a row. The
//! drawing alone: nothing of the simulation changes. Off, nothing of it is
//! read: the glass is drawn as its material was, opaque.
//!
//! `PROVISIONAL`: the level's reflection probe (`_MyGlobalReflectionProbe`)
//! is not in the app, nought in the emission; the top ambient the level's
//! (nought without `CUSTOMS LIGHT`) on the gear page's figure too; with
//! Bevy's light (`TARKOV SHADING` off, the figures' views) the glass's
//! figures are made linear for it, its specular colour as Bevy's
//! reflectance; the row over taken as the one above as seen; with MSAA a
//! pixel marked where most of its samples are the glass's; Bevy's own
//! transparent things (a level's blended things) are drawn before the
//! undithering, where the game draws them after it; the Minotaur welding
//! mask's glass casts no shadow, where its renderer's does (`m_CastShadows`
//! is not read); on a preview's camera the frame's figures taken to the
//! game's numbers by the 2.2 power its photographic curve takes them by.
//! The item's picture's glass (its `_Icon` twin) and the inspect window's
//! (the world's deferred pass on the preview's own surface) are drawn by
//! `game_surface`, from the same notes (`glass_note`); every mesh drawn
//! dithered carries `Dithered`, which gives its cameras the undithering.
//! Not built: a tinted glass (none in the install), the thermal vision's
//! colour.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::camera::visibility::{RenderLayers, VisibilitySystems};
use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::texture_2d;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderContext;
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::ShaderRef;
use trp_data::drawing::glass::GlassFigures;

use crate::platform::park::EvenParked;
use crate::rendering::shadow_only::ShadowOnly;

mod glass_drawn;
mod undither_pass;

pub use glass_drawn::*;
pub(crate) use undither_pass::*;
use glass_drawn::{Twins, follow, mark_cameras, ready, ready_to_draw, swap};
use undither_pass::{UnditherNode, UnditherPipeline, init_pipeline, prepare};

const GLASS_SHADER: Handle<Shader> = uuid_handle!("8d41c7e2-5b93-4f60-a2e8-1c7b9d3f5a26");
const GLASS_PREPASS_SHADER: Handle<Shader> = uuid_handle!("c25e8a17-9f4d-4b3a-8e61-7d0f2b9c4e53");
const UNDITHER_SHADER: Handle<Shader> = uuid_handle!("4a9f3e6c-1d27-4c85-b0e3-6f8a2d5c7b19");

static ON: AtomicBool = AtomicBool::new(false);

/// The switch for the run, set before anything is read.
pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// Whether the see-through glass is drawn this run.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// A camera's depth's uses: read as well as drawn where the glass is drawn,
/// for its undithering (the gear page's figures' cameras, the edit build
/// screen's, the inspect window's preview's; the world's and the optic's
/// read theirs anyway).
pub fn depth_usages() -> bevy::camera::Camera3dDepthTextureUsage {
    if on() { TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING } else { TextureUsages::RENDER_ATTACHMENT }.into()
}

/// A reader of the install for models, the cubemaps' bundle open where the
/// glass is drawn, or the OVERALL figure's own surface (`figure_surface`):
/// its cube (`_Cube`) is there, and a reference into a bundle not open is
/// not followed (`read_material`).
pub fn open_cubes(unity: &mut trp_data::unity::Assets) {
    if on() || crate::screens::figure_surface::on() {
        if let Err(e) = unity.load("cubemaps") {
            warn!("the cubemaps could not be read ({e:#}): the glass and the OVERALL figure are drawn with the default cube");
        }
    }
}

/// What a material of the dithered glass is drawn with besides its standard
/// material's pictures: its figures, its specular picture (`_SpecTex`, read
/// as stored) and its cube.
#[derive(Clone, Debug)]
pub struct GlassNote {
    pub figures: GlassFigures,
    pub spec: Option<Handle<Image>>,
    pub cube: Option<Handle<Image>>,
}

/// The materials of the dithered glass, as they are made (`render_data`).
static NOTED: LazyLock<Mutex<HashMap<AssetId<StandardMaterial>, GlassNote>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// A material of the dithered glass, noted as made.
pub fn note(material: &Handle<StandardMaterial>, note: GlassNote) {
    if let Ok(mut map) = NOTED.lock() {
        map.insert(material.id(), note);
    }
}

fn note_of(material: &Handle<StandardMaterial>) -> Option<GlassNote> {
    NOTED.lock().ok().and_then(|map| map.get(&material.id()).cloned())
}

/// A material's glass note, where it is the dithered glass drawn so this
/// run: for the views that draw it on a surface of their own (an item's
/// picture, the inspect window's preview: `game_surface`).
pub fn glass_note(material: &Handle<StandardMaterial>) -> Option<GlassNote> {
    if on() { note_of(material) } else { None }
}

/// On every mesh drawn dithered this run (the twin here, the preview's own
/// surface's glass): a camera that shows one runs the undithering.
#[derive(Component, Clone, Copy, Default)]
pub struct Dithered;

/// Whether a material is the dithered glass's, drawn so this run.
pub fn is_glass(material: &Handle<StandardMaterial>) -> bool {
    on() && NOTED.lock().is_ok_and(|map| map.contains_key(&material.id()))
}

/// The standard material with the glass's pass over it.
pub type GlassMaterial = ExtendedMaterial<StandardMaterial, Glass>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Glass {
    #[uniform(100)]
    pub figures: GlassUniform,
    /// `_SpecTex`; none, the shader's default white.
    #[texture(101)]
    #[sampler(102)]
    pub spec: Option<Handle<Image>>,
    /// `_Cube`; none, the engine's default cube (a flat 0x80).
    #[texture(103, dimension = "cube")]
    #[sampler(104)]
    pub cube: Option<Handle<Image>>,
}

pub use uniform::GlassUniform;

// The derive that lays the figures out for the shader writes helpers this
// compiler counts as unused.
#[allow(dead_code)]
mod uniform {
    use super::*;

    /// The glass's figures as the shader reads them (`glass.wgsl`).
    #[derive(ShaderType, Reflect, Debug, Clone, Copy, Default)]
    pub struct GlassUniform {
        /// `_Color`, `_SpecColor`, `_ReflectColor`, as stored.
        pub color: Vec4,
        pub spec_color: Vec4,
        pub reflect_color: Vec4,
        /// `_OpacityScale`, `_SpecPower`, `_Glossness`,
        /// `_GlobalReflectionStrength`.
        pub figures: Vec4,
        /// The top ambient; w 1 where it has a cube of its own.
        pub ambient: Vec4,
    }
}

impl Glass {
    /// The twin of a glass's standard material: its pictures and normal map
    /// kept, its colour white (the shader takes `_Color` itself), its faces
    /// culled by `_Cull`.
    fn twin(standard: &StandardMaterial, note: &GlassNote, top_ambient: [f32; 3]) -> GlassMaterial {
        let f = &note.figures;
        let cull_mode = match f.cull {
            0 => None,
            1 => Some(Face::Front),
            _ => Some(Face::Back),
        };
        let base = StandardMaterial { base_color: Color::WHITE, alpha_mode: AlphaMode::Opaque, cull_mode, ..standard.clone() };
        let figures = GlassUniform {
            color: Vec4::from(f.color),
            spec_color: Vec4::from(f.spec_color),
            reflect_color: Vec4::from(f.reflect_color),
            figures: Vec4::new(f.opacity_scale, f.spec_power, f.glossness, f.global_reflection_strength),
            ambient: Vec3::from(top_ambient).extend(if note.cube.is_some() { 1.0 } else { 0.0 }),
        };
        GlassMaterial { base, extension: Self { figures, spec: note.spec.clone(), cube: note.cube.clone() } }
    }
}

impl MaterialExtension for Glass {
    fn fragment_shader() -> ShaderRef {
        GLASS_SHADER.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        GLASS_PREPASS_SHADER.into()
    }

    /// Its renderers cast no shadow.
    fn enable_shadows() -> bool {
        false
    }

    /// The game's light called from the shader where a view may ask for it
    /// (`shading::installed`: the world's, or the OVERALL figure's).
    fn specialize(_: &MaterialExtensionPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialExtensionKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        if crate::rendering::shading::installed() {
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("TRP_GAME_SHADING".into());
            }
        }
        Ok(())
    }
}

pub struct GlassPlugin;

impl Plugin for GlassPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, GLASS_SHADER, "../../shaders/glass.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, GLASS_PREPASS_SHADER, "../../shaders/glass_prepass.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, UNDITHER_SHADER, "../../shaders/undither.wgsl", Shader::from_wgsl);
        if !on() {
            return;
        }
        app.add_plugins((MaterialPlugin::<GlassMaterial>::default(), ExtractComponentPlugin::<UnditherScreen>::default()))
            .init_resource::<Twins>()
            .add_systems(
                PostUpdate,
                (
                    ready.before(crate::rendering::shadow_only::apply),
                    swap.after(crate::rendering::shadow_only::apply).run_if(ready_to_draw),
                    follow,
                    mark_cameras.after(VisibilitySystems::CheckVisibility),
                ),
            );
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<UnditherPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<UnditherNode>>(Core3d, UnditherLabel)
            .add_render_graph_edges(Core3d, (crate::rendering::scattering_view::ScatteringLabel, UnditherLabel, crate::effects::explosion_view::ExplosionLabel))
            // On a preview's camera, before its outline and its photographic
            // curve: the game's undithering is an opaque image effect, on
            // the frame before the tonemap (inspect-window spec, "The
            // model").
            .add_render_graph_edges(Core3d, (UnditherLabel, crate::screens::preview_outline::PreviewOutlineLabel));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cameras that undither the glass besides the world's (the gear
    /// page's figures', the edit build screen's) read their depth only
    /// with `SEE-THROUGH GLASS`: off, their depth is Bevy's default, drawn
    /// only, as before the glass.
    #[test]
    fn a_preview_camera_s_depth_is_read_only_with_the_glass() {
        let was = on();
        set_on(true);
        let read = TextureUsages::from(depth_usages());
        set_on(false);
        let unread = TextureUsages::from(depth_usages());
        set_on(was);
        assert!(read.contains(TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING));
        assert!(!unread.contains(TextureUsages::TEXTURE_BINDING));
        assert_eq!(unread, TextureUsages::from(Camera3d::default().depth_texture_usages));
    }
}
