//! The game's own surface shader, for the two views that draw an item alone under lights of their own.
//! The shader is `p0/Reflective/Bumped Specular SMap`; the views are its
//! picture (icons spec, "The game's surface in a picture") and the inspect
//! window's preview (inspect-window spec, "The model"). Every item's
//! material is the shader's, and these views drew it with Bevy's standard
//! surface, whose specular adds grey to every texel and whose light is
//! linear: a picture paler and greyer than the game's, in the icons and on
//! the inspect screen alike (the Salewa very dull where the game's is very bright).
//!
//! The standard material is kept for its pictures and its normal map, and
//! its fragment (`game_surface.wgsl`) does the game's sums in the game's
//! numbers (its project is in gamma):
//!
//! - **a picture** (`Way::Picture`): the shader's `_Icon` twin, which the
//!   icon camera draws every material with (`ShaderReplacer`), forward: per
//!   light the engine's old Blinn-Phong, a rim on the colour and on the
//!   specular, the colour picture's alpha masking the specular and the
//!   material's cubemap reflection, the eye 1 m before the model's middle;
//! - **the preview** (`Way::Preview`): the shader's deferred pass, its
//!   G-buffer's colour, specular, smoothness and emission (the cube's
//!   reflection), lit by the deferred light the world's shading ports
//!   (`game_shading.wgsl`), handed on in the frame's figures for the
//!   preview's tonemap (`preview_tonemap`).
//!
//! The lights are the view's own, handed in the material's figures as the
//! game's numbers (colour times intensity, the way to each): Bevy's lights
//! there reach the shader already turned linear and scaled.
//!
//! The OVERALL tab's own figure (`figure_surface`, with `OVERALL FIGURE`)
//! is drawn by the preview's way too (`Way::Preview`, its `view_lights`):
//! the same G-buffer, lit not by lights in its figures but by the view's
//! own (Bevy's, handed in the game's units: its eight, six of them points)
//! through the game's light rule (`game_shading.wgsl`'s `trp_game_light`),
//! which the patched PBR functions carry (`shading`).
//!
//! The dithered glass (`Transparent/DepthZwriteDithered`, with the options'
//! `SEE-THROUGH GLASS`: `glass_view`) has its own two drawings (`glass`):
//!
//! - **in a picture**, its own twin, `Transparent/DepthZwriteDithered_Icon`:
//!   pass 1 (the brightest light and the reflection: the cube 2.5 times,
//!   plus the specular picture's red times a fixed blue) and pass 2 (each
//!   other light), blended over what the opaque things drew by the colour's
//!   alpha (the colour picture's times `_Color`'s; no `_OpacityScale`), the
//!   colour alone written; and pass 0, a second draw of its mesh (`cover`),
//!   writing its vertex colour's alpha alone wherever it is in front;
//! - **in the preview**, the world's deferred pass as `glass.wgsl` does it
//!   (dithered where its alpha is under a half, marked in the frame's alpha
//!   for the preview camera's undithering), lit by the preview's lights.
//!
//! `PROVISIONAL`: a picture's three lights are all lit per pixel, as the
//! game's shadows quality of medium and above allows (at low, two pixel
//! lights, the fill would go into the harmonics: the engine's rule not
//! read). Every material is drawn by the `p0` shader's sums, the twin that
//! stands for any shader without one of its own; the other six twins are
//! not read. In a picture's glass: a mesh with no vertex colours (the
//! window magazine's) writes an alpha of one, the engine's default for a
//! missing colour taken as white; each pass's sum held to one, as the
//! eight-bit target holds each pass's output, where the blend itself is in
//! half floats; two of its faces in front of one another both blended
//! (Bevy's blended pass writes no depth; the game's pass 0 leaves the front
//! one alone); the base pass's light the brightest. In the preview's glass,
//! the frame's alpha where the glass meets the empty panel the undithering's
//! mean, 0.5 (the game's light buffer's alpha, 1 and one a light, goes
//! through the photographic curve: 0.875). `ESTIMATE`: the
//! preview's deferred reflections pass is left out (about 0.01 facing, by
//! a capture of the game), and so is its ambient occlusion.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, ColorWrites, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;

use crate::assets::render_data::GameFigures;

const SHADER: Handle<Shader> = uuid_handle!("3b7d2f56-9a41-4c8e-b1d6-27e05f4a9c13");

/// The standard material with the game's own surface over it.
pub type GameSurfaceMaterial = ExtendedMaterial<StandardMaterial, GameSurface>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(SurfaceKey)]
pub struct GameSurface {
    #[uniform(100)]
    pub figures: SurfaceFigures,
    /// The material's cubemap (`_Cube`); none, and the engine's default
    /// cube is used, a flat grey of 0x80 (`UnityDefaultCube`).
    #[texture(101, dimension = "cube")]
    #[sampler(102)]
    pub cube: Option<Handle<Image>>,
    /// The glass's specular picture (`_SpecTex`, read as stored); none, the
    /// shader's default white.
    #[texture(103)]
    #[sampler(104)]
    pub spec: Option<Handle<Image>>,
    /// Which of its target's channels it writes, and how.
    pub draw: Draw,
    /// Lit by the view's own lights through the game's light rule (the
    /// OVERALL figure's: `lit_by_view`), whose shader then calls it.
    pub view_lights: bool,
}

/// What a material's pipeline writes: everything as drawn; a picture's
/// glass's colour alone, blended by its alpha (`_Icon`'s passes 1 and 2:
/// source alpha and one less it, the colour's mask); its alpha alone, as
/// drawn (pass 0: one and nought, the alpha's mask).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
pub enum Draw {
    #[default]
    Whole,
    Colour,
    Alpha,
}

/// What a material's pipeline is built by: what it writes, and whether its
/// shader calls the game's light (`view_lights`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SurfaceKey {
    draw: Draw,
    view_lights: bool,
}

impl From<&GameSurface> for SurfaceKey {
    fn from(surface: &GameSurface) -> Self {
        Self { draw: surface.draw, view_lights: surface.view_lights }
    }
}

pub use uniform::SurfaceFigures;

// The derive that lays the figures out for the shader writes helpers this
// compiler counts as unused.
#[allow(dead_code)]
mod uniform {
    use super::*;

    /// The material's figures and the view's lights, as the shader reads
    /// them (`game_surface.wgsl`).
    #[derive(ShaderType, Reflect, Debug, Clone, Copy, Default)]
    pub struct SurfaceFigures {
        /// `_Color`, `_SpecColor`, `_ReflectColor`, as stored.
        pub color: Vec4,
        pub spec_color: Vec4,
        pub reflect_color: Vec4,
        /// `_DefVals`' two, then `_SpecVals`' two; the glass's
        /// `_OpacityScale`, `_SpecPower`, `_Glossness`,
        /// `_GlobalReflectionStrength`.
        pub vals: Vec4,
        /// `_Specularness`, `_Glossness`, whether it has a gloss picture
        /// and whether a cube of its own (1 or 0); the glass's
        /// top ambient and whether a cube of its own.
        pub shine: Vec4,
        /// The eye where w is 1 (a picture's); w 0, the view's own place.
        pub eye: Vec4,
        /// x: the way (0 a picture, 1 the preview, 2 the standard surface's
        /// light in a picture's figures, 3 a picture's glass, 4 the
        /// preview's glass, 5 a picture's glass's alpha, 6 the preview's
        /// lit by the view's own lights); y: how many lights; z: 1 where
        /// the colour picture is read as stored, not as sRGB; w: a
        /// picture's glass's base pass's light.
        pub way: Vec4,
        /// Each light's way to it (in the engine's world) and its colour
        /// times its intensity (the game's numbers).
        pub light_to: [Vec4; 4],
        pub light_color: [Vec4; 4],
    }
}

/// Which of the shader's two drawings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// The `_Icon` twin, forward, as the icon camera draws.
    Picture,
    /// The deferred pass and light, as the preview's camera draws.
    Preview,
}

/// A view's lights and eye, for its materials.
#[derive(Clone, Debug)]
pub struct SurfaceView {
    pub way: Way,
    /// The eye's place, where it is not the view's own (a picture's: 1 m
    /// before the model's middle, where the icon camera stands).
    pub eye: Option<Vec3>,
    /// Each light: the way to it in the engine's world and its colour times
    /// its intensity, at most four.
    pub lights: Vec<(Vec3, Vec3)>,
    /// The top ambient as last set, which the glass's emission takes (the
    /// preview: the level's, as the world's; a picture's twin has none).
    pub ambient: [f32; 3],
}

impl SurfaceView {
    /// Of its first four lights, the brightest (its colour's luminance):
    /// the one the engine gives the base pass of a forward shader.
    pub fn brightest(&self) -> usize {
        let luminance = |c: Vec3| c.dot(Vec3::new(0.299, 0.587, 0.114));
        self.lights.iter().take(4).enumerate().fold((0, f32::MIN), |best, (k, (_, c))| if luminance(*c) > best.1 { (k, luminance(*c)) } else { best }).0
    }

    /// A directional light as the engine places it (`convert::rotation` of
    /// its turn, in the world): the way to it is its own back (+z), the
    /// light going along its forward.
    pub fn light(turn: Quat, color: [f32; 3], intensity: f32) -> (Vec3, Vec3) {
        ((turn * Vec3::Z).normalize_or_zero(), Vec3::from(color) * intensity)
    }
}

impl GameSurface {
    /// A material's figures and its cube under a view's lights.
    pub fn new(game: &GameFigures, cube: Option<Handle<Image>>, view: &SurfaceView) -> Self {
        let mut light_to = [Vec4::ZERO; 4];
        let mut light_color = [Vec4::ZERO; 4];
        for (k, (to, color)) in view.lights.iter().take(4).enumerate() {
            light_to[k] = to.extend(0.0);
            light_color[k] = color.extend(0.0);
        }
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let figures = SurfaceFigures {
            color: Vec4::from(game.color),
            spec_color: Vec4::from(game.spec_color),
            reflect_color: Vec4::from(game.reflect_color),
            vals: Vec4::new(game.def_vals[0], game.def_vals[1], game.spec_vals[0], game.spec_vals[1]),
            shine: Vec4::new(game.specularness, game.glossness, flag(game.has_gloss), flag(cube.is_some())),
            eye: view.eye.map_or(Vec4::ZERO, |eye| eye.extend(1.0)),
            way: Vec4::new(if view.way == Way::Preview { 1.0 } else { 0.0 }, view.lights.len().min(4) as f32, flag(game.raw_colour), 0.0),
            light_to,
            light_color,
        };
        Self { figures, cube, spec: None, draw: Draw::Whole, view_lights: false }
    }

    /// A material of the app's own (a weapon of one's own's, the other
    /// game's suits: no figures of the game's) in a picture: the standard
    /// surface's own light, handed on as the stored figures a picture keeps
    /// (the way 2), as it was drawn before the studio took the game's
    /// numbers.
    pub fn standard(standard: &StandardMaterial) -> GameSurfaceMaterial {
        let figures = SurfaceFigures { way: Vec4::new(2.0, 0.0, 0.0, 0.0), ..default() };
        GameSurfaceMaterial { base: standard.clone(), extension: Self { figures, cube: None, spec: None, draw: Draw::Whole, view_lights: false } }
    }

    /// A dithered glass's material for this surface (its figures, specular
    /// picture and cube as noted: `glass_view`). In a picture, its `_Icon`
    /// twin's passes 1 and 2, its colour alone blended over what is behind
    /// (its alpha the second draw's: `cover`), back faces culled as the
    /// twin's passes fix it; in the preview, the world's deferred pass,
    /// culled by `_Cull`, dithered and marked for the undithering.
    pub fn glass(standard: &StandardMaterial, game: &GameFigures, note: &crate::effects::glass_view::GlassNote, view: &SurfaceView) -> GameSurfaceMaterial {
        let f = &note.figures;
        let picture = view.way == Way::Picture;
        let cull_mode = match f.cull {
            _ if picture => Some(Face::Back),
            0 => None,
            1 => Some(Face::Front),
            _ => Some(Face::Back),
        };
        let alpha_mode = if picture { AlphaMode::Blend } else { AlphaMode::Opaque };
        let base = StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 1.0, metallic: 0.0, alpha_mode, cull_mode, ..standard.clone() };
        let mut extension = Self::new(game, note.cube.clone(), view);
        let figures = &mut extension.figures;
        figures.color = Vec4::from(f.color);
        figures.spec_color = Vec4::from(f.spec_color);
        figures.reflect_color = Vec4::from(f.reflect_color);
        figures.vals = Vec4::new(f.opacity_scale, f.spec_power, f.glossness, f.global_reflection_strength);
        figures.shine = Vec3::from(view.ambient).extend(if note.cube.is_some() { 1.0 } else { 0.0 });
        figures.way.x = if picture { 3.0 } else { 4.0 };
        figures.way.w = view.brightest() as f32;
        extension.spec = note.spec.clone();
        extension.draw = if picture { Draw::Colour } else { Draw::Whole };
        GameSurfaceMaterial { base, extension }
    }

    /// A picture's glass's second draw, its `_Icon` twin's pass 0: after
    /// what is opaque, its back faces culled, its alpha alone written (its
    /// vertex colour's alpha, one where it has none), wherever it is in
    /// front.
    pub fn cover() -> GameSurfaceMaterial {
        let base = StandardMaterial { base_color: Color::WHITE, alpha_mode: AlphaMode::Blend, cull_mode: Some(Face::Back), unlit: true, ..default() };
        let figures = SurfaceFigures { way: Vec4::new(5.0, 0.0, 0.0, 0.0), ..default() };
        GameSurfaceMaterial { base, extension: Self { figures, cube: None, spec: None, draw: Draw::Alpha, view_lights: false } }
    }

    /// The standard material a prepared one was made as, for this surface:
    /// its colour picture read as it is (the shader takes `_Color` itself)
    /// and its gloss picture's roughness at full scale.
    pub fn material(standard: &StandardMaterial, game: &GameFigures, cube: Option<Handle<Image>>, view: &SurfaceView) -> GameSurfaceMaterial {
        let base = StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 1.0, metallic: 0.0, ..standard.clone() };
        GameSurfaceMaterial { base, extension: Self::new(game, cube, view) }
    }

    /// The same, the preview's deferred pass lit by the view's own lights
    /// through the game's light rule (the way 6: the OVERALL figure's,
    /// `figure_surface`), where Bevy's PBR functions carry it (`shading::
    /// patched`): no lights in its figures.
    pub fn lit_by_view(standard: &StandardMaterial, game: &GameFigures, cube: Option<Handle<Image>>) -> GameSurfaceMaterial {
        let view = SurfaceView { way: Way::Preview, eye: None, lights: Vec::new(), ambient: [0.0; 3] };
        let mut made = Self::material(standard, game, cube, &view);
        made.extension.figures.way.x = 6.0;
        made.extension.view_lights = true;
        made
    }
}

impl MaterialExtension for GameSurface {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    /// A picture's glass's two draws: its colour blended by its alpha and
    /// written alone, then its alpha written alone, as drawn (`Draw`); the
    /// game's light called from the shader where it is lit by the view's
    /// own lights (`view_lights`).
    fn specialize(_: &MaterialExtensionPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, key: MaterialExtensionKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.view_lights {
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("TRP_GAME_SHADING".into());
            }
        }
        let (blend, mask) = match key.bind_group_data.draw {
            Draw::Whole => return Ok(()),
            Draw::Colour => (
                Some(BlendState { color: BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::OneMinusSrcAlpha, operation: BlendOperation::Add }, alpha: BlendComponent::REPLACE }),
                ColorWrites::COLOR,
            ),
            Draw::Alpha => (None, ColorWrites::ALPHA),
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = blend;
                target.write_mask = mask;
            }
        }
        Ok(())
    }
}

pub struct GameSurfacePlugin;

impl Plugin for GameSurfacePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SHADER, "../shaders/game_surface.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<GameSurfaceMaterial>::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The icon camera's three lights as read (`Studio::as_read`), turned
    /// into the engine's world, point where the game's do (the shader's
    /// way to each light, read in its own world: z the other way here).
    #[test]
    fn the_studio_s_lights_point_as_the_game_s() {
        let studio = trp_data::interface::icon::Studio::as_read();
        let game = [[-0.1785, 0.7694, -0.6133], [0.6047, 0.6286, -0.4892], [-0.1421, -0.9387, -0.3143]];
        for (light, want) in studio.lights.iter().zip(game) {
            let turn = crate::platform::convert::rotation(studio.rotation) * crate::platform::convert::rotation(light.rotation);
            let (to, _) = SurfaceView::light(turn, light.color, light.intensity);
            let want = Vec3::new(want[0], want[1], -want[2]);
            assert!((to - want).length() < 0.002, "{to} against {want}");
        }
    }

    /// A dithered glass in each view (`glass`): in a picture its `_Icon`
    /// twin, blended and back faces culled whatever its `_Cull`, its colour
    /// alone written, its figures where the shader reads them and its base
    /// pass the brightest light; its alpha the second draw's (`cover`); in
    /// the preview the world's deferred pass, opaque, culled by `_Cull`,
    /// with the level's top ambient.
    #[test]
    fn a_glass_is_drawn_as_each_view_s_twin() {
        // The PMAG GEN M3 Window's glass (`the_window_magazine_s_glass_is_read`),
        // faces of both sides drawn here to tell the culls apart.
        let figures = trp_data::drawing::glass::GlassFigures {
            color: [0.8962, 0.8962, 0.8962, 1.0],
            opacity_scale: 1.0,
            spec_color: [0.9412, 0.9412, 0.9412, 1.0],
            reflect_color: [1.0, 1.0, 1.0, 0.5],
            spec_power: 0.17,
            glossness: 1.5,
            global_reflection_strength: 1.0,
            cull: 0,
        };
        let note = crate::effects::glass_view::GlassNote { figures, spec: None, cube: None };
        let game = GameFigures {
            color: [1.0; 4],
            spec_color: [0.5, 0.5, 0.5, 1.0],
            reflect_color: [1.0; 4],
            def_vals: [1.0, 0.0],
            spec_vals: [1.0, 0.0],
            specularness: 1.0,
            glossness: 1.0,
            bump_tiling: 1.0,
            has_gloss: false,
            raw_colour: true,
        };
        let lights = vec![(Vec3::Y, Vec3::splat(0.4)), (Vec3::X, Vec3::splat(0.9)), (Vec3::Z, Vec3::splat(0.2))];
        let picture = SurfaceView { way: Way::Picture, eye: Some(Vec3::Z), lights: lights.clone(), ambient: [0.0; 3] };
        let standard = StandardMaterial::default();
        let twin = GameSurface::glass(&standard, &game, &note, &picture);
        assert_eq!(twin.base.alpha_mode, AlphaMode::Blend);
        assert_eq!(twin.base.cull_mode, Some(Face::Back));
        assert_eq!(twin.extension.draw, Draw::Colour);
        assert_eq!(twin.extension.figures.way, Vec4::new(3.0, 3.0, 1.0, 1.0));
        assert_eq!(twin.extension.figures.vals, Vec4::new(1.0, 0.17, 1.5, 1.0));
        assert_eq!(twin.extension.figures.color, Vec4::new(0.8962, 0.8962, 0.8962, 1.0));
        let cover = GameSurface::cover();
        assert_eq!((cover.base.alpha_mode, cover.base.cull_mode, cover.extension.draw), (AlphaMode::Blend, Some(Face::Back), Draw::Alpha));
        assert_eq!(cover.extension.figures.way.x, 5.0);
        let preview = SurfaceView { way: Way::Preview, eye: None, lights, ambient: [0.1, 0.2, 0.3] };
        let world = GameSurface::glass(&standard, &game, &note, &preview);
        assert_eq!((world.base.alpha_mode, world.base.cull_mode, world.extension.draw), (AlphaMode::Opaque, None, Draw::Whole));
        assert_eq!(world.extension.figures.way.x, 4.0);
        assert_eq!(world.extension.figures.shine, Vec4::new(0.1, 0.2, 0.3, 0.0));
    }
}
