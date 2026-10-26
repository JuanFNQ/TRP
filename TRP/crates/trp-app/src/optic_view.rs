//! A scope's picture-in-picture, as the game draws it (optics spec, "The
//! optic camera", "The lens"; the options' `PIP`): one optic camera, on
//! while aiming through an optic, draws the world from the current mode's
//! `optic_camera` object at the scope's field of view into a square picture
//! of 1024 pixels; the scope's lens shows it through its own shader
//! (`optic_lens.wgsl`: the picture on a plane ahead of the lens, the shadow
//! on a funnel toward the eye, the reticle on the picture's plane), every
//! lens not in use dimmed. A red dot's or a holographic sight's lens adds
//! its dot (`collimator.wgsl`) whatever the switch; the scopes' front glass
//! and a red dot's window (`EFT/CollimatorGlass`, the PK-06's) are not drawn
//! (`PROVISIONAL`: their added reflections are not built; drawn as the
//! part's other materials are, the window was an opaque disc over the dot).

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::{load_internal_asset, uuid_handle, RenderAssetUsages};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Exposure, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::Hdr;
use bevy::shader::ShaderRef;
use trp_data::sight::{LensKind, LensNode};
use trp_data::unity::extract::TextureData;

use crate::weapon::{HandsRig, WeaponRig};
use crate::{MainCamera, View};

/// The layer the scopes' lenses are drawn on: the main camera's, not the
/// optic camera's (its picture is what a lens reads, and the game's optic
/// camera sees no lens either: the camera stands ahead of it).
pub const LENS_LAYER: usize = 20;
/// The optic camera's picture: one square of 1024 pixels whatever the
/// settings (`OpticCameraManager`).
const PICTURE_SIZE: u32 = 1024;
/// A lens not in use (`OpticSight.LensFade`): `_SwitchToSight`.
const DIMMED: f32 = 0.97;
/// The optic camera's near plane where a scope's data gives none.
const NEAR: f32 = 0.05;

/// The layer a variable scope's reticle mesh is drawn on: the optic
/// camera's alone.
pub const RETICLE_LAYER: usize = 21;

const LENS_SHADER: Handle<Shader> = uuid_handle!("5a0f7d93-2c4e-4b61-9f38-7e2d1c6b4a85");
const COLLIMATOR_SHADER: Handle<Shader> = uuid_handle!("c3e18b47-96d2-4f05-8a7c-0b5e9d2f6a13");
const RETICLE_SHADER: Handle<Shader> = uuid_handle!("9b47e2c1-58a3-4d6f-b0e9-3c2a7f1d8e64");

pub struct OpticViewPlugin {
    /// The options' `PIP`.
    pub pip: bool,
}

impl Plugin for OpticViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, LENS_SHADER, "optic_lens.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, COLLIMATOR_SHADER, "collimator.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, RETICLE_SHADER, "reticle.wgsl", Shader::from_wgsl);
        app.add_plugins((MaterialPlugin::<LensMaterial>::default(), MaterialPlugin::<CollimatorMaterial>::default(), MaterialPlugin::<ReticleMaterial>::default()))
            .insert_resource(Pip(self.pip))
            .init_resource::<LensPictures>()
            .init_resource::<WorldZoom>()
            .init_resource::<ClearLens>()
            .add_systems(Update, (dress_lenses, turn_sights, see_through, place_camera, show_reticle).chain().after(crate::hands_view::apply_hands))
            // Before what culls by the drawn places; the muzzle's effects,
            // which read them, are ordered after it (`muzzle_view`,
            // `particles_view`, `explosion_view`).
            .add_systems(PostUpdate, squeeze_drawn.after(bevy::transform::TransformSystems::Propagate).before(bevy::camera::visibility::VisibilitySystems::CheckVisibility))
            // Before the render world takes the new meshes' corners.
            .add_systems(PostUpdate, keep_sources);
        if self.pip {
            app.add_systems(PostStartup, spawn_camera);
        } else {
            app.add_systems(PostStartup, spawn_reticle_camera);
        }
    }
}

/// The options' `PIP` as the run took it.
#[derive(Resource)]
pub struct Pip(pub bool);

/// With `PIP` off, how many times the world is narrowed while aiming
/// through an optic (optics spec, "With `PIP` off"): the tangent of the
/// simulation's half field of view over the drawn one's; 1 otherwise. The
/// first-person weapon and arms are drawn squeezed across the view by as
/// much (`squeeze_drawn`), so they keep their size.
#[derive(Resource)]
pub struct WorldZoom(pub f32);

impl Default for WorldZoom {
    fn default() -> Self {
        Self(1.0)
    }
}

/// With `PIP` off, the world's narrowing for the frame's field of view
/// (the simulation's, `fov`, degrees, with the setting's): the sight's
/// magnification, put on as the simulation's view goes from the setting to
/// the aimed 35 degrees, so that it comes and goes with the aim's ease; 1
/// off an optic, outside first person, or with `PIP` on.
pub fn world_zoom(pip: bool, first_person: bool, hands: &trp_sim::Hands, fov: f32) -> f32 {
    if pip || !first_person {
        return 1.0;
    }
    let Some(m) = hands.sights().and_then(trp_sim::sights::Sights::magnification).filter(|m| *m > 1.0) else { return 1.0 };
    let setting = hands.settings.field_of_view;
    let aimed = trp_sim::sights::OPTIC_FIELD_OF_VIEW;
    let share = if setting > aimed { ((setting - fov) / (setting - aimed)).clamp(0.0, 1.0) } else { 1.0 };
    1.0 + (m - 1.0) * share
}

/// With `PIP` off and the world narrowed, the first-person weapon, its arms
/// and all drawn with them squeezed across the view about the camera by
/// the narrowing (`WorldZoom`), once the frame's transforms are worked out:
/// what is drawn alone (the bullets and the simulation read the rig's own
/// transforms, earlier in the frame), so that they keep the size the aimed
/// field of view gives them over the narrowed world, as a camera of the
/// weapon's own would draw them (`PROVISIONAL`: the squeeze, not a second
/// camera; their shadows squeezed with them).
pub fn squeeze_drawn(
    zoom: Res<WorldZoom>,
    rig: Option<Res<WeaponRig>>,
    view: Res<View>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    children: Query<&Children>,
    mut globals: Query<&mut GlobalTransform, Without<MainCamera>>,
) {
    let (Some(rig), true) = (rig, zoom.0 > 1.0 + 1e-5 && *view == View::FirstPerson) else { return };
    let Ok(eye) = camera.single() else { return };
    let eye = eye.affine();
    let across = 1.0 / zoom.0;
    let squeeze = eye * bevy::math::Affine3A::from_scale(Vec3::new(across, across, 1.0)) * eye.inverse();
    for entity in std::iter::once(rig.root).chain(children.iter_descendants(rig.root)) {
        if let Ok(mut global) = globals.get_mut(entity) {
            *global = GlobalTransform::from(squeeze * global.affine());
        }
    }
}

/// The optic camera.
#[derive(Component)]
pub struct OpticCamera;

/// Its picture (`_CamTex`).
#[derive(Resource)]
pub struct OpticPicture(pub Handle<Image>);

/// The lenses' pictures, made once each from the build's reading.
#[derive(Resource, Default)]
struct LensPictures(HashMap<usize, Handle<Image>>);

/// A lens piece that has been given its own drawing: which, and the
/// dimming it was last given.
#[derive(Component)]
struct Dressed {
    node: usize,
    dimmed: f32,
    /// The narrowing its material was last given (`LensUniform::narrow`),
    /// and the zeroing's shift of its reticle.
    narrow: f32,
    shift: [f32; 2],
    /// With `PIP` off, whether it was last drawn as clear glass (its
    /// see-through cut in force) or black.
    clear: bool,
}

/// With `PIP` off, the lens whose see-through cut is in force
/// (`see_through`): the only one drawn as clear glass.
#[derive(Resource, Default)]
struct ClearLens(Option<Entity>);

#[allow(dead_code)]
mod uniforms {
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
        /// with `CUSTOMS LIGHT`, else 1), whether the lens has a reticle.
        pub space: Vec4,
        /// With `PIP` off, the world's narrowing the drawn lens is squeezed
        /// across the view by (`WorldZoom`), undone where the lens works out
        /// its own frame, 1 otherwise; where the zeroing puts the reticle
        /// on the clear glass, right and up from the picture's middle in
        /// its widths (`Sights::zeroing_offset`); and whether it is clear
        /// glass now (1) or black (0: `see_through`'s cut not in force).
        pub narrow: Vec4,
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
}
pub use uniforms::{CollimatorUniform, LensUniform, ReticleUniform};

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
    /// game's writes none). With `PIP` off, clear glass laid over the world
    /// behind it, which it darkens (premultiplied: the reticle laid on, the
    /// world kept by what its alpha leaves), or black.
    fn alpha_mode(&self) -> AlphaMode {
        if self.lens.space.w > 0.5 {
            AlphaMode::Premultiplied
        } else {
            AlphaMode::Opaque
        }
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

/// The optic camera, made once with the main camera and left off: its
/// picture HDR (half floats) where the main camera's is, drawn before the
/// main camera's, of the world (not the lenses), as the main camera draws
/// it (the same sampling, tonemapping, exposure and environment).
///
/// It draws the world's effects as the game's optic camera does: the
/// effects' objects are all of the world's layer (the effects' bundle's 1,006
/// objects on layer 0 but four smoke volumes' colliders, `probe
/// bundle-layers`; the jets' mesh made on layer 0), which its mask takes.
/// So it has the screens of the flash's jets, the fumes, the sparks and the
/// explosions' and hits' systems, and the scene's depth readable for their
/// copy (`muzzle_view`). Not the gun smoke's trail, which the game draws for
/// the main camera alone (`MuzzleSmoke.OnRenderObject`), nor the heat haze,
/// drawn by the main camera's distortion alone (`DistortCameraFX`, which
/// this camera has not), nor the screen's blood. Each screen is filled only
/// while the camera is on.
/// With `PIP` off, a variable scope's reticle is a mesh drawn in the optic
/// camera's own projection (`show_reticle`): a camera of its own draws it
/// alone (the reticle's layer, nothing of the world) into a transparent
/// square, which the clear lens takes as its mark, where the lens shader
/// reads the picture and the mark alike, so that it grows with the zoom as
/// with `PIP` on. On only while such a scope is aimed through
/// (`place_camera`). `PROVISIONAL`: the reticle's colour as the camera keeps
/// it, in an 8-bit sRGB square (black lines: their cover is what shows).
fn spawn_reticle_camera(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    const SIZE: u32 = 512;
    let mut picture = Image::new_uninit(Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    picture.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    picture.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::ClampToEdge, address_mode_v: ImageAddressMode::ClampToEdge, ..ImageSamplerDescriptor::linear() });
    let picture = images.add(picture);
    commands.spawn((
        Name::new("Reticle camera"),
        OpticCamera,
        Camera3d::default(),
        Camera { is_active: false, order: -1, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
        RenderTarget::Image(picture.clone().into()),
        Projection::Perspective(PerspectiveProjection { fov: 20f32.to_radians(), aspect_ratio: 1.0, near: NEAR, near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -NEAR), ..default() }),
        Transform::default(),
        RenderLayers::layer(RETICLE_LAYER),
        Tonemapping::None,
        Msaa::Off,
    ));
    commands.insert_resource(OpticPicture(picture));
}

#[allow(clippy::type_complexity)]
fn spawn_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    light: Option<Res<crate::lighting::LevelLighting>>,
    main: Query<(Entity, &Camera, Option<&Hdr>, Option<&Tonemapping>, Option<&Exposure>, Option<&bevy::light::EnvironmentMapLight>, Option<&AmbientLight>, Option<&Msaa>), With<MainCamera>>,
) {
    let Ok((main, camera, hdr, tonemapping, exposure, environment, ambient, msaa)) = main.single() else {
        warn!("no main camera: no optic camera");
        return;
    };
    let format = if hdr.is_some() { TextureFormat::Rgba16Float } else { TextureFormat::Rgba8UnormSrgb };
    let mut picture = Image::new_uninit(Extent3d { width: PICTURE_SIZE, height: PICTURE_SIZE, depth_or_array_layers: 1 }, TextureDimension::D2, format, RenderAssetUsages::default());
    picture.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    picture.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::ClampToEdge, address_mode_v: ImageAddressMode::ClampToEdge, ..ImageSamplerDescriptor::linear() });
    let picture = images.add(picture);
    let mut optic = commands.spawn((
        Name::new("Optic camera"),
        OpticCamera,
        // Its depth readable after the main pass, for the effects' copy.
        Camera3d { depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING).into(), ..default() },
        Camera { is_active: false, order: -1, clear_color: camera.clear_color, ..default() },
        RenderTarget::Image(picture.clone().into()),
        Projection::Perspective(PerspectiveProjection { fov: 20f32.to_radians(), aspect_ratio: 1.0, near: NEAR, near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -NEAR), ..default() }),
        Transform::default(),
        RenderLayers::from_layers(&[0, RETICLE_LAYER, crate::weapon::HANDS_LAYER]),
        (
            crate::muzzle_view::JetScreen::default(),
            crate::particles_view::FumeScreen::default(),
            crate::particles_view::SparkScreen::default(),
            crate::explosion_view::ExplosionScreen::default(),
        ),
    ));
    if hdr.is_some() {
        optic.insert(Hdr);
    }
    // With a level's light its picture stays in the scene's figures around
    // its effects (`gamma_ui`): the main camera's PRISM takes it on.
    if light.is_some() {
        optic.insert(crate::gamma_ui::KeepFigures);
    }
    if let Some(tonemapping) = tonemapping {
        optic.insert(*tonemapping);
    }
    if let Some(exposure) = exposure {
        optic.insert(*exposure);
    }
    if let Some(environment) = environment {
        optic.insert(environment.clone());
    }
    if let Some(ambient) = ambient {
        optic.insert(ambient.clone());
    }
    if let Some(msaa) = msaa {
        optic.insert(*msaa);
    }
    commands.insert_resource(OpticPicture(picture));
    // The main camera sees the lenses besides the world.
    commands.entity(main).insert(RenderLayers::from_layers(&[0, LENS_LAYER, crate::weapon::HANDS_LAYER]));
}

/// The entity a lens piece is drawn by: its object's own where its mesh
/// has one part, else that part's among the object's drawn children
/// (`weapon::spawn_nodes`: one child a submesh, in order, with no name).
fn piece_entity(object: Entity, slot: usize, meshes: &Query<(), With<Mesh3d>>, named: &Query<(), With<Name>>, children: &Query<&Children>) -> Option<Entity> {
    if meshes.contains(object) {
        return (slot == 0).then_some(object);
    }
    let kids = children.get(object).ok()?;
    kids.iter().filter(|&kid| meshes.contains(kid) && !named.contains(kid)).nth(slot)
}

/// A lens picture as an image, its sampler clamping where the game's does.
fn lens_picture(pictures: &mut LensPictures, images: &mut Assets<Image>, texture: &Arc<TextureData>) -> Option<Handle<Image>> {
    let key = Arc::as_ptr(texture) as usize;
    if let Some(handle) = pictures.0.get(&key) {
        return Some(handle.clone());
    }
    let mut image = match crate::render_data::build_image(texture, crate::render_data::Role::Color) {
        Ok(image) => image,
        Err(e) => {
            warn!("the lens's picture {}: {e:#}", texture.name);
            return None;
        }
    };
    if let ImageSampler::Descriptor(sampler) = &mut image.sampler {
        let mode = |clamp: bool| if clamp { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
        (sampler.address_mode_u, sampler.address_mode_v) = (mode(texture.clamp[0]), mode(texture.clamp[1]));
    }
    let handle = images.add(image);
    pictures.0.insert(key, handle.clone());
    Some(handle)
}

/// Gives the lens pieces of the weapon in hand their own drawing, once each:
/// a scope's lens the lens's (with `PIP`), a collimator's its dot, the
/// front glass nothing.
#[allow(clippy::too_many_arguments)]
fn dress_lenses(
    mut commands: Commands,
    pip: Res<Pip>,
    rig: Option<Res<WeaponRig>>,
    hands: Option<Res<HandsRig>>,
    picture: Option<Res<OpticPicture>>,
    light: Option<Res<crate::lighting::LevelLighting>>,
    mut pictures: ResMut<LensPictures>,
    mut images: ResMut<Assets<Image>>,
    mut lenses: ResMut<Assets<LensMaterial>>,
    mut collimators: ResMut<Assets<CollimatorMaterial>>,
    meshes: Query<(), With<Mesh3d>>,
    named: Query<(), With<Name>>,
    children: Query<&Children>,
    dressed: Query<(), With<Dressed>>,
) {
    let (Some(rig), Some(hands)) = (rig, hands) else { return };
    let Some(sights) = hands.hands.sights() else { return };
    let power = if light.is_some() { 2.2 } else { 1.0 };
    let pieces: Vec<&LensNode> = sights.parts().iter().flat_map(|part| &part.lenses).collect();
    for piece in pieces {
        let Some(&object) = rig.bones.get(piece.node) else { continue };
        let Some(entity) = piece_entity(object, piece.slot, &meshes, &named, &children) else { continue };
        if dressed.contains(entity) {
            continue;
        }
        let picture_of = |name: &str| piece.pictures.iter().find(|(n, _)| n == name).map(|(_, texture)| texture.clone());
        let mut e = commands.entity(entity);
        match &piece.kind {
            LensKind::Optic(lens) => {
                // With `PIP` off, clear glass with its reticle (optics spec,
                // "With `PIP` off"), on the world's layer: no optic camera,
                // no lens layer.
                let glass = !pip.0;
                // A variable scope's reticle mesh, drawn by the reticle
                // camera into its picture, is the clear glass's picture
                // (read as the optic camera's would be), under its mark.
                let meshed = glass && sights.parts().iter().flat_map(|part| &part.modes).filter_map(|mode| mode.optic.as_ref()).any(|optic| optic.lens == Some(piece.node) && optic.reticle.is_some());
                let picture = match (glass, picture.as_ref()) {
                    (true, Some(picture)) if meshed => Some(picture.0.clone()),
                    (true, _) => None,
                    (false, Some(picture)) => Some(picture.0.clone()),
                    (false, None) => {
                        e.insert(Dressed { node: piece.node, dimmed: DIMMED, narrow: 1.0, shift: [0.0; 2], clear: false });
                        continue;
                    }
                };
                let mark = picture_of("_MarkTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t));
                // Its picture's aberration: its mode's.
                let aberration = sights.parts().iter().flat_map(|part| &part.modes).filter_map(|mode| mode.optic.as_ref()).find(|optic| optic.lens == Some(piece.node)).map_or(0.0, |optic| optic.aberration);
                let material = LensMaterial {
                    lens: LensUniform {
                        scales_shifts: Vec4::new(lens.scales[0], lens.scales[1], lens.shifts[0], lens.shifts[1]),
                        direction: Vec4::new(lens.shift_direction[0], lens.shift_direction[1], lens.shift_direction[2], lens.normal_hideness),
                        figures: Vec4::new(lens.inner_radius, lens.outer_radius, lens.mark_lightness, 1.0 - DIMMED),
                        space: Vec4::new(power, if mark.is_some() { 1.0 } else { 0.0 }, aberration, if meshed { 2.0 } else if glass { 1.0 } else { 0.0 }),
                        narrow: Vec4::new(1.0, 0.0, 0.0, 0.0),
                    },
                    picture,
                    mark,
                    mask: picture_of("_MaskTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t)),
                    mask2: picture_of("_MaskTex2").and_then(|t| lens_picture(&mut pictures, &mut images, &t)),
                };
                e.remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(lenses.add(material)), NotShadowCaster, Dressed { node: piece.node, dimmed: DIMMED, narrow: 1.0, shift: [0.0; 2], clear: false }));
                if !glass {
                    e.insert(RenderLayers::layer(LENS_LAYER));
                }
            }
            LensKind::Collimator(dot) => {
                let noise = picture_of("_NoiseTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t));
                let fade = picture_of("_FadeTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t));
                let material = CollimatorMaterial {
                    collimator: CollimatorUniform {
                        color: Vec4::new(dot.color[0], dot.color[1], dot.color[2], dot.hdr),
                        mark: Vec4::new(dot.mark_shift[0], dot.mark_shift[1], dot.mark_shift[2], dot.mark_scale),
                        space: Vec4::new(power, if noise.is_some() { 1.0 } else { 0.0 }, if fade.is_some() { 1.0 } else { 0.0 }, 0.0),
                    },
                    mark: picture_of("_MarkTex").and_then(|t| lens_picture(&mut pictures, &mut images, &t)),
                    noise,
                    fade,
                };
                e.remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(collimators.add(material)), NotShadowCaster, Dressed { node: piece.node, dimmed: 0.0, narrow: 1.0, shift: [0.0; 2], clear: false }));
            }
            LensKind::Glass => {
                e.insert((Visibility::Hidden, Dressed { node: piece.node, dimmed: 0.0, narrow: 1.0, shift: [0.0; 2], clear: false }));
            }
            // With `PIP` off the scope is seen through: the objective's back
            // face, opaque as the part's other materials are drawn, not drawn.
            LensKind::Back if !pip.0 => {
                e.insert((Visibility::Hidden, Dressed { node: piece.node, dimmed: 0.0, narrow: 1.0, shift: [0.0; 2], clear: false }));
            }
            LensKind::Back => {
                e.insert(Dressed { node: piece.node, dimmed: 0.0, narrow: 1.0, shift: [0.0; 2], clear: false });
            }
        }
    }
}

/// With `PIP` off a scope is seen through: the weapon's triangles that the
/// eye sees through the live lens (the scope's inside: the tube, the rings,
/// a mode's mechanism; the barrel and the front ahead: what the game's own
/// picture covers) are left out while it is live (optics spec, "With `PIP`
/// off"), as a 3D scope's modeller cuts a tunnel through the scope from the
/// aiming point (Arma 3's 3D scopes). The entity's whole mesh, its mesh
/// with the cut in force, and a copy of its corners kept in the main world
/// (the meshes keep theirs in the render world alone once drawn).
#[derive(Component)]
pub struct SeenThrough {
    full: Handle<Mesh>,
    cut: Option<Handle<Mesh>>,
    source: Arc<Mesh>,
}

/// With `PIP` off, each rigid mesh of the rig kept as `SeenThrough`'s
/// source, on the frame it appears (before the render world takes its
/// corners): not the arms' skinned meshes, nor the lens pieces.
#[allow(clippy::type_complexity)]
fn keep_sources(
    mut commands: Commands,
    pip: Res<Pip>,
    rig: Option<Res<WeaponRig>>,
    children: Query<&Children>,
    fresh: Query<&Mesh3d, (Without<SeenThrough>, Without<Dressed>)>,
    meshes: Res<Assets<Mesh>>,
) {
    let Some(rig) = rig.filter(|_| !pip.0) else { return };
    for entity in children.iter_descendants(rig.root) {
        let Ok(mesh3d) = fresh.get(entity) else { continue };
        let Some(mesh) = meshes.get(&mesh3d.0) else { continue };
        let rigid = matches!(mesh.try_attribute_option(Mesh::ATTRIBUTE_JOINT_INDEX), Ok(None));
        if rigid && matches!(mesh.try_attribute_option(Mesh::ATTRIBUTE_POSITION), Ok(Some(_))) {
            commands.entity(entity).insert(SeenThrough { full: mesh3d.0.clone(), cut: None, source: Arc::new(mesh.clone()) });
        }
    }
}

/// The cut in force: the lens it is for and the eye it was made from (in
/// the rig root's frame); the eye at the last frame.
#[derive(Default)]
struct CutState {
    made: Option<(Entity, Vec3)>,
    last_eye: Option<Vec3>,
}

/// How far the eye may have moved from the one the cut was made from before
/// it is made again, and how little it must move in a frame for that (the
/// aim's ease over), metres.
const RECUT: f32 = 0.002;
const SETTLED: f32 = 0.0002;

/// With `PIP` off, the live lens's view cut out of the rig's rigid meshes
/// (not the arms' skinned ones, whose corners are kept in their bind pose):
/// a triangle goes when a corner of it lies more than a centimetre ahead of
/// the eye on a line from the eye that passes through the glass's disc, to
/// its rim (beyond the lens, or between it and the eye). Made once
/// the aim has settled, from the eye where the camera stands, and again
/// should the eye move by `RECUT` (the old cut kept meanwhile); until the
/// first, the lens is black (`ClearLens`, `place_camera`: the user's, so
/// that the inside does not show through it during the aim's ease, where a
/// cut every frame would cost a millisecond a frame and the meshes'
/// uploads); worked in the rig root's frame from the
/// rig's own transforms and the camera's beside it: the field of view's
/// squeeze (an affine map) and the narrowing's (about the eye) keep what
/// the eye sees through the lens.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn see_through(
    pip: Res<Pip>,
    view: Res<View>,
    rig: Option<Res<WeaponRig>>,
    hands: Option<Res<HandsRig>>,
    mut state: Local<CutState>,
    transforms: Query<&Transform>,
    camera: Query<&Transform, With<MainCamera>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    with_mesh: Query<(), With<Mesh3d>>,
    named: Query<(), With<Name>>,
    dressed: Query<(), With<Dressed>>,
    mut drawn: Query<(Entity, &mut Mesh3d, Option<&mut SeenThrough>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut clear: ResMut<ClearLens>,
) {
    if pip.0 {
        return;
    }
    // The live lens, while aiming through it in first person.
    let live = rig.as_ref().zip(hands.as_ref()).filter(|(_, hands)| hands.optic_seen() && *view == View::FirstPerson).and_then(|(rig, hands)| {
        let sights = hands.hands.sights()?;
        let (part, mode) = sights.live_optic()?;
        let node = sights.parts().get(part)?.modes.get(mode)?.optic.as_ref()?.lens?;
        let piece = sights.parts().iter().flat_map(|part| &part.lenses).find(|piece| piece.node == node)?;
        let LensKind::Optic(lens) = &piece.kind else { return None };
        let entity = piece_entity(*rig.bones.get(node)?, piece.slot, &with_mesh, &named, &children)?;
        Some((entity, lens.clone()))
    });
    let (Some(rig), Some((lens_entity, lens))) = (rig.as_ref(), live) else {
        // None live: every mesh whole.
        state.made = None;
        state.last_eye = None;
        clear.0 = None;
        for (_, mut mesh, seen) in &mut drawn {
            if let Some(seen) = seen.filter(|seen| mesh.0 != seen.full) {
                mesh.0 = seen.full.clone();
            }
        }
        return;
    };
    let local = |e: Entity| transforms.get(e).ok().copied();
    let in_root = |e: Entity| crate::hands_view::in_root(e, rig.root, local, &parents).map(|t| t.compute_affine());
    // The eye in the root's frame: the camera and the root hang from the
    // same pivot.
    let (Ok(eye), Ok(root)) = (camera.single(), transforms.get(rig.root)) else { return };
    let eye = root.compute_affine().inverse().transform_point3(eye.translation);
    let settled = state.last_eye.is_some_and(|last| last.distance(eye) < SETTLED);
    state.last_eye = Some(eye);
    let current = state.made.is_some_and(|(entity, made)| entity == lens_entity && made.distance(eye) < RECUT);
    if !current && settled {
        state.made = Some((lens_entity, eye));
        make_cut(rig, lens_entity, &lens, eye, &in_root, &children, &dressed, &mut drawn, &mut meshes);
    }
    // The cut drawn while it is the live lens's, whole meshes otherwise; the
    // lens clear glass only then (`place_camera`).
    let cut_live = state.made.is_some_and(|(entity, _)| entity == lens_entity);
    clear.0 = cut_live.then_some(lens_entity);
    for (_, mut mesh, seen) in &mut drawn {
        let Some(seen) = seen else { continue };
        let wanted = seen.cut.as_ref().filter(|_| cut_live).unwrap_or(&seen.full);
        if mesh.0 != *wanted {
            mesh.0 = wanted.clone();
        }
    }
}

/// The cut of `see_through` made for a lens and an eye: each rigid mesh of
/// the rig given the mesh without the triangles seen through the lens (or
/// none where it has none), the cut it had before let go.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn make_cut(
    rig: &WeaponRig,
    lens_entity: Entity,
    lens: &trp_data::sight::OpticLens,
    eye: Vec3,
    in_root: &dyn Fn(Entity) -> Option<bevy::math::Affine3A>,
    children: &Query<&Children>,
    dressed: &Query<(), With<Dressed>>,
    drawn: &mut Query<(Entity, &mut Mesh3d, Option<&mut SeenThrough>)>,
    meshes: &mut Assets<Mesh>,
) {
    let Some(at) = in_root(lens_entity) else { return };
    // The lens's axis toward the eye (its `_ShiftDirection`, the game's
    // axes turned into the app's), its middle, its inner ring's radius.
    let way = Vec3::new(lens.shift_direction[0], lens.shift_direction[1], -lens.shift_direction[2]);
    let toward_eye = at.transform_vector3(way).normalize_or_zero();
    let middle = Vec3::from(at.translation);
    // The glass's whole disc, to its rim (the picture's ring is smaller): on
    // the Specter 6.8 cm from the eye by the screen, as by the rig's places.
    let radius = lens.outer_radius * at.matrix3.x_axis.length();
    let from_eye = (eye - middle).dot(toward_eye);
    if toward_eye == Vec3::ZERO || radius <= 0.0 || from_eye <= 0.0 {
        return;
    }
    // On a line from the eye through the glass's disc, more than a
    // centimetre ahead of the eye: beyond the lens (its inside, the barrel)
    // or between it and the eye (an eyepiece's parts, where the lens is at
    // the objective).
    let seen = |p: Vec3| {
        let ahead = (eye - p).dot(toward_eye);
        if ahead <= 0.01 {
            return false;
        }
        let t = from_eye / ahead;
        (eye + (p - eye) * t - middle).length() < radius
    };
    let started = std::time::Instant::now();
    let (mut cut_count, mut tested) = (0, 0);
    let entities: Vec<Entity> = children.iter_descendants(rig.root).filter(|e| *e != lens_entity && !dressed.contains(*e)).collect();
    for entity in entities {
        let (Ok((_, _, Some(mut seen_through))), Some(place)) = (drawn.get_mut(entity), in_root(entity)) else { continue };
        let old = seen_through.cut.take();
        let mesh = seen_through.source.clone();
        let Ok(Some(bevy::mesh::VertexAttributeValues::Float32x3(positions))) = mesh.try_attribute_option(Mesh::ATTRIBUTE_POSITION) else { continue };
        tested += positions.len();
        let corners: Vec<bool> = positions.iter().map(|p| seen(place.transform_point3(Vec3::from_array(*p)))).collect();
        let indices: Vec<u32> = match mesh.try_indices_option() {
            Ok(Some(bevy::mesh::Indices::U16(list))) => list.iter().map(|&i| i as u32).collect(),
            Ok(Some(bevy::mesh::Indices::U32(list))) => list.clone(),
            _ => (0..positions.len() as u32).collect(),
        };
        // A triangle with any corner seen through the lens goes (those
        // crossing its edge too: the inside's large walls).
        let kept: Vec<u32> = indices.chunks_exact(3).filter(|t| !t.iter().any(|&i| corners.get(i as usize).copied().unwrap_or(false))).flatten().copied().collect();
        let cut = (kept.len() != indices.len()).then(|| {
            let mut cut = (*mesh).clone();
            cut.insert_indices(bevy::mesh::Indices::U32(kept));
            cut.asset_usage = bevy::asset::RenderAssetUsages::RENDER_WORLD;
            meshes.add(cut)
        });
        if let Some(old) = old {
            meshes.remove(old.id());
        }
        cut_count += usize::from(cut.is_some());
        seen_through.cut = cut;
    }
    debug!("the see-through cut: {cut_count} meshes cut, {tested} corners tested in {:.2} ms, the eye {from_eye:.3} m behind the lens of radius {radius:.4} m", started.elapsed().as_secs_f64() * 1e3);
}

/// The zeroing's turns (`trp_sim::sights::Sights::turns`; optics spec,
/// "Zeroing"): each optic's camera object, each collimator and a part a
/// prefab turns by distance, as the simulation last left them, and a
/// variable scope's ring as its zoom turns it (`Sights::ring_turns`; optics
/// spec, "The ring"), put on the rig's bones after the animator's pose,
/// which writes every bone each frame (`animation::pose_rig`).
fn turn_sights(rig: Option<Res<WeaponRig>>, hands: Option<Res<HandsRig>>, mut transforms: Query<&mut Transform, Without<OpticCamera>>) {
    let (Some(rig), Some(hands)) = (rig, hands) else { return };
    let Some(sights) = hands.hands.sights() else { return };
    for (node, turn) in sights.turns().chain(sights.ring_turns()) {
        let Some(mut transform) = rig.bones.get(node).and_then(|&entity| transforms.get_mut(entity).ok()) else { continue };
        let rotation = crate::convert::rotation(turn);
        if transform.rotation != rotation {
            transform.rotation = rotation;
        }
    }
}

/// Each frame: the optic camera on while aiming through an optic in first
/// person, at the current mode's `optic_camera` object (turned to the
/// zeroing point: `turn_sights`), at the scope's field of view; the lenses
/// dimmed but the live one's (with `PIP` off, the live one's reticle where
/// the zeroing puts it); a scope's modes' objects shown as its prefab's
/// mode is.
#[allow(clippy::too_many_arguments)]
fn place_camera(
    mut commands: Commands,
    pip: Res<Pip>,
    view: Res<View>,
    rig: Option<Res<WeaponRig>>,
    hands: Option<Res<HandsRig>>,
    mut camera: Query<(Entity, &mut Camera, &mut Transform, &mut Projection, Option<&ChildOf>), With<OpticCamera>>,
    transforms: Query<&Transform, Without<OpticCamera>>,
    parents: Query<&ChildOf>,
    mut lenses: Query<(Entity, &mut Dressed, &MeshMaterial3d<LensMaterial>)>,
    mut materials: ResMut<Assets<LensMaterial>>,
    mut visibility: Query<&mut Visibility>,
    mut shown_modes: Local<HashMap<Entity, usize>>,
    zoom: Res<WorldZoom>,
    clear_lens: Res<ClearLens>,
) {
    let (Some(rig), Some(hands)) = (rig, hands) else { return };
    let Some(sights) = hands.hands.sights() else { return };
    let first_person = *view == View::FirstPerson;
    // The optic whose camera is on, and its lens.
    // (Its lens is the live one with `PIP` off too: clear glass, its
    // reticle undimmed.)
    let live = sights.live_optic().filter(|_| first_person && hands.optic_seen());
    let optic = live.and_then(|(part, mode)| sights.parts().get(part)?.modes.get(mode)?.optic.as_ref());
    // A scope's modes' objects as its prefab's mode (`ScopePrefabCache.
    // SetMode`): the first object kept as the prefab has it until the mode
    // changes.
    for (part, piece) in sights.parts().iter().enumerate() {
        if piece.modes.len() < 2 {
            continue;
        }
        let Some(&root) = rig.bones.get(piece.root) else { continue };
        let mode = sights.state(part).prefab_mode;
        let was = *shown_modes.get(&root).unwrap_or(&0);
        if was == mode {
            continue;
        }
        for (index, info) in piece.modes.iter().enumerate() {
            if let Some(mut seen) = info.node.and_then(|node| rig.bones.get(node)).and_then(|&e| visibility.get_mut(e).ok()) {
                *seen = if index == mode { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
        shown_modes.insert(root, mode);
    }
    // The lenses' dimming: none on the live one.
    let live_lens = optic.and_then(|optic| optic.lens);
    // With `PIP` off, where the zeroing puts the live lens's reticle: the
    // optic camera's turn as it would show in the picture (optics spec,
    // "With `PIP` off").
    let offset = live.filter(|_| !pip.0).and_then(|_| sights.zeroing_offset(sights.optic_field_of_view()?)).unwrap_or([0.0; 2]);
    for (entity, mut dressed, material) in &mut lenses {
        let live = Some(dressed.node) == live_lens;
        let dimmed = if live { 0.0 } else { DIMMED };
        // With `PIP` off, the squeeze the lens is drawn with, for it to undo.
        let narrow = if pip.0 { 1.0 } else { zoom.0 };
        let shift = if live { offset } else { [0.0; 2] };
        // With `PIP` off, clear glass only while its see-through cut is in
        // force, black otherwise: until the aim settles, and off it.
        let clear = !pip.0 && live && clear_lens.0 == Some(entity);
        if dressed.dimmed != dimmed || dressed.narrow != narrow || dressed.shift != shift || dressed.clear != clear {
            (dressed.dimmed, dressed.narrow, dressed.shift, dressed.clear) = (dimmed, narrow, shift, clear);
            if let Some(material) = materials.get_mut(&material.0) {
                material.lens.figures.w = 1.0 - dimmed;
                material.lens.narrow.x = narrow;
                (material.lens.narrow.y, material.lens.narrow.z) = (shift[0], shift[1]);
                material.lens.narrow.w = if clear { 1.0 } else { 0.0 };
            }
        }
    }
    let Ok((entity, mut camera, mut transform, mut projection, parent)) = camera.single_mut() else { return };
    // With `PIP` off, the reticle camera: on while a scope with a reticle
    // mesh is aimed through, at its zoom's field of view (its place matters
    // not: the reticle hangs from it).
    if !pip.0 {
        let fov = optic.filter(|optic| optic.reticle.is_some()).and_then(|_| sights.optic_field_of_view());
        if let (Some(fov), Projection::Perspective(perspective)) = (fov, &mut *projection) {
            perspective.fov = fov.to_radians();
        }
        if camera.is_active != fov.is_some() {
            camera.is_active = fov.is_some();
        }
        return;
    }
    // Its place in the pivot's frame: the rig's root as it stands, without
    // the squeeze of a wide field of view (the game's camera is placed
    // before the squeeze is put on), then down to the object (whose own
    // scale is nought on the Tango: only its place and turn are taken).
    let place = optic.filter(|_| pip.0).and_then(|optic| {
        let object = *rig.bones.get(optic.camera?)?;
        let root = transforms.get(rig.root).ok()?;
        let within = crate::hands_view::in_root(object, rig.root, |e| transforms.get(e).ok().copied(), &parents)?;
        let mut place = Transform { scale: Vec3::ONE, ..*root } * within;
        place.scale = Vec3::ONE;
        Some((place, sights.optic_field_of_view()?, optic.near.unwrap_or(NEAR)))
    });
    let Some((place, fov, near)) = place else {
        if camera.is_active {
            camera.is_active = false;
        }
        return;
    };
    if parent.map(ChildOf::parent) != Some(rig.pivot) {
        commands.entity(rig.pivot).add_child(entity);
    }
    *transform = place;
    if let Projection::Perspective(perspective) = &mut *projection {
        perspective.fov = fov.to_radians();
        perspective.near = near;
        perspective.near_clip_plane = Vec4::new(0.0, 0.0, -1.0, -near);
    }
    if !camera.is_active {
        camera.is_active = true;
    }
}

/// The reticle mesh shown: whose (`ScopeReticle`, by its place in memory),
/// its entity, its mesh and the blend weight it was last made at.
struct Shown {
    key: usize,
    entity: Entity,
    mesh: Handle<Mesh>,
    weight: f32,
}

/// A reticle's mesh at a weight of its blend shape, in the app's axes.
fn reticle_mesh(reticle: &trp_data::sight::ScopeReticle, weight: f32) -> Mesh {
    let moved: Vec<[f32; 3]> = reticle
        .positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let d = reticle.shape.as_ref().and_then(|s| s.get(i)).copied().unwrap_or([0.0; 3]);
            crate::convert::position([p[0] + weight * d[0], p[1] + weight * d[1], p[2] + weight * d[2]]).to_array()
        })
        .collect();
    // Turned round with z, each triangle keeps its order (they are drawn
    // one over another in it) and its corners are taken the other way.
    let mut triangles = reticle.triangles.clone();
    for t in triangles.chunks_exact_mut(3) {
        t.swap(1, 2);
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, moved)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, reticle.colors.clone())
        .with_inserted_indices(bevy::mesh::Indices::U32(triangles))
}

/// The live optic's reticle mesh, where its scope has one (a variable
/// scope's, `OpticRetrice`): a child of the optic camera at its place and
/// turn, on the camera's own layer, its blend shape weighted by the zoom.
#[allow(clippy::too_many_arguments)]
fn show_reticle(
    mut commands: Commands,
    hands: Option<Res<HandsRig>>,
    light: Option<Res<crate::lighting::LevelLighting>>,
    camera: Query<Entity, With<OpticCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ReticleMaterial>>,
    mut shown: Local<Option<Shown>>,
) {
    let Ok(camera) = camera.single() else { return };
    let wanted = hands.as_ref().and_then(|hands| {
        let sights = hands.hands.sights()?;
        let (part, mode) = sights.live_optic()?;
        let reticle = sights.parts().get(part)?.modes.get(mode)?.optic.as_ref()?.reticle.clone()?;
        Some((reticle, sights.reticle_weight()))
    });
    let Some((reticle, weight)) = wanted else {
        if let Some(old) = shown.take() {
            commands.entity(old.entity).despawn();
        }
        return;
    };
    let key = Arc::as_ptr(&reticle) as usize;
    if let Some(now) = shown.as_mut().filter(|now| now.key == key) {
        if (now.weight - weight).abs() > 1e-4 {
            now.weight = weight;
            if let Some(mesh) = meshes.get_mut(&now.mesh) {
                *mesh = reticle_mesh(&reticle, weight);
            }
        }
        return;
    }
    if let Some(old) = shown.take() {
        commands.entity(old.entity).despawn();
    }
    let mesh = meshes.add(reticle_mesh(&reticle, weight));
    let power = if light.is_some() { 2.2 } else { 1.0 };
    let material = materials.add(ReticleMaterial { reticle: ReticleUniform { color: Vec4::from_array(reticle.color), figures: Vec4::new(reticle.scale * 0.1, power, 0.0, 0.0) } });
    let turn = trp_sim::math::quat_euler(reticle.rotation);
    let entity = commands
        .spawn((
            Name::new("Scope reticle"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform { translation: crate::convert::position(reticle.position), rotation: crate::convert::rotation(turn), scale: Vec3::ONE },
            RenderLayers::layer(RETICLE_LAYER),
            NotShadowCaster,
            bevy::camera::visibility::NoFrustumCulling,
        ))
        .id();
    commands.entity(camera).add_child(entity);
    *shown = Some(Shown { key, entity, mesh, weight });
}
