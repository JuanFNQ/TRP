//! A level's time-of-day sky about the camera (`tod_sky.wgsl`; lighting
//! spec, "Customs by its time of day"): the level's own atmosphere mesh (an
//! icosphere of radius one) drawn as the game's `Time of Day/Atmosphere`
//! draws it, turned by the view alone and put at the far plane, so that the
//! scene covers it; the sun's disc within the same draw, added under it as
//! the game adds the atmosphere over the sun's quad. The figures are those
//! of the moment the range is lit at (`lighting`): they do not change while
//! the app runs. The drawing alone: nothing of the simulation changes.
//!
//! The moon (`Time of Day/Moon`) in the same draw: its sphere where the
//! way meets it, its picture lit by the sun, put under the atmosphere as
//! the game draws it before (opaque, over the sun's disc). The clouds are
//! `clouds_view`'s, over the sky after the scene; the stars are nought by
//! day.

use bevy::asset::{load_internal_asset, uuid_handle, RenderAssetUsages};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;
use trp_data::screen::BuildMesh;
use trp_sim::tod::SkyGlobals;

const TOD_SKY_SHADER: Handle<Shader> = uuid_handle!("8e3c5a21-4d7f-4b19-a6e2-1f9c0d7b5e33");

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct TodSkyMaterial {
    #[uniform(0)]
    pub sky: TodSky,
    /// The moon's picture (`_MainTex`), its values as stored.
    #[texture(1)]
    #[sampler(2)]
    pub moon: Option<Handle<Image>>,
}

pub use uniform::TodSky;

#[allow(dead_code)]
mod uniform {
    use bevy::prelude::*;
    use bevy::render::render_resource::ShaderType;

    /// The shader's figures (`tod_sky.wgsl`, `TodSky`).
    #[derive(ShaderType, Debug, Clone, Copy, Default)]
    pub struct TodSky {
        pub sun_sky: Vec4,
        pub moon_sky: Vec4,
        pub ground: Vec4,
        pub moon_halo: Vec4,
        pub sun: Vec4,
        pub moon: Vec4,
        pub figures: Vec4,
        pub k_beta_mie: Vec4,
        pub k_sun: Vec4,
        pub k_4pi: Vec4,
        pub k_radius: Vec4,
        pub k_scale: Vec4,
        pub sun_mesh: Vec4,
        pub sun_mesh_figures: Vec4,
        pub moon_right: Vec4,
        pub moon_up: Vec4,
        pub moon_forward: Vec4,
        pub moon_figures: Vec4,
        pub moon_tiling: Vec4,
        pub sun_world: Vec4,
    }
}

impl TodSky {
    /// The sky's figures, the moon's picture's tiling where it has one.
    pub fn of(g: &SkyGlobals, moon_tiling: Option<[f32; 4]>) -> Self {
        let three = |c: [f32; 3], w: f32| Vec4::new(c[0], c[1], c[2], w);
        let s = &g.scattering;
        Self {
            sun_sky: three(g.sun_sky_color, 0.0),
            moon_sky: three(g.moon_sky_color, 0.0),
            ground: three(g.ground_color, 0.0),
            moon_halo: three(g.moon_halo_color, 0.0),
            sun: three(g.local_sun_direction, 0.0),
            moon: three(g.local_moon_direction, 0.0),
            figures: Vec4::new(g.contrast, g.scattering_brightness, g.fogginess, g.moon_halo_power),
            k_beta_mie: three(s.beta_mie, 0.0),
            k_sun: Vec4::from_array(s.sun),
            k_4pi: Vec4::from_array(s.four_pi),
            k_radius: Vec4::from_array(s.radius),
            k_scale: Vec4::from_array(s.scale),
            sun_mesh: three(g.sun_mesh_color, g.sun_mesh_half_width),
            sun_mesh_figures: Vec4::new(g.sun_mesh_contrast, g.sun_mesh_brightness, if g.sun_drawn { 1.0 } else { 0.0 }, 0.0),
            moon_right: three(g.moon_mesh.right, g.moon_mesh.radius),
            moon_up: three(g.moon_mesh.up, if g.moon_mesh.drawn && moon_tiling.is_some() { 1.0 } else { 0.0 }),
            moon_forward: three(g.moon_mesh.forward, g.moon_mesh.contrast),
            moon_figures: Vec4::new(g.moon_mesh.brightness, 0.0, 0.0, 0.0),
            moon_tiling: Vec4::from_array(moon_tiling.unwrap_or([1.0, 1.0, 0.0, 0.0])),
            sun_world: three(g.sun_direction, 0.0),
        }
    }
}

/// The moon's picture as the shader samples it: its values as stored (the
/// game's project samples them as they are, not by the sRGB curve), its
/// rows as stored (the game's v is the stored row's), its edges as its own
/// sampler takes them.
fn moon_image(picture: &trp_data::unity::extract::TextureData) -> Option<Image> {
    let mut raw = picture.clone();
    raw.srgb = false;
    let mut image = crate::render_data::build_image(&raw, crate::render_data::Role::Color).ok()?;
    let mode = |clamped: bool| if clamped { bevy::image::ImageAddressMode::ClampToEdge } else { bevy::image::ImageAddressMode::Repeat };
    image.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: mode(picture.clamp[0]),
        address_mode_v: mode(picture.clamp[1]),
        mag_filter: bevy::image::ImageFilterMode::Linear,
        min_filter: bevy::image::ImageFilterMode::Linear,
        mipmap_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    });
    Some(image)
}

impl Material for TodSkyMaterial {
    fn vertex_shader() -> ShaderRef {
        TOD_SKY_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        TOD_SKY_SHADER.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// The dome's inside, as the game's culls its front; no depth written,
    /// as the game's writes none.
    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = false;
        }
        Ok(())
    }
}

/// The dome's mesh as Bevy's: its vertices' places as the game has them
/// (the shader turns them into the app's axes), its triangles each turned
/// round (z turned round turns a triangle's sense; with no culling, only
/// for order's sake).
pub fn dome_mesh(mesh: &BuildMesh) -> Mesh {
    let mut triangles = mesh.triangles.clone();
    for t in triangles.chunks_exact_mut(3) {
        t.swap(1, 2);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, mesh.positions.clone())
        .with_inserted_indices(Indices::U32(triangles))
}

/// The sky's dome in the world, with the level's light (`lighting`): no
/// shadow, never culled (its vertices are put about the camera whatever its
/// place).
fn spawn(mut commands: Commands, level: Option<Res<crate::lighting::LevelLighting>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<TodSkyMaterial>>, mut images: ResMut<Assets<Image>>) {
    let Some(level) = level else { return };
    let moon = level.sky.moon.as_ref().and_then(|moon| Some((images.add(moon_image(&moon.picture)?), moon.tiling)));
    let sky = TodSky::of(&level.sky.globals, moon.as_ref().map(|(_, tiling)| *tiling));
    commands.spawn((
        Mesh3d(meshes.add(dome_mesh(&level.sky.mesh))),
        MeshMaterial3d(materials.add(TodSkyMaterial { sky, moon: moon.map(|(image, _)| image) })),
        Transform::default(),
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

pub struct TodSkyPlugin;

impl Plugin for TodSkyPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, TOD_SKY_SHADER, "tod_sky.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<TodSkyMaterial>::default()).add_systems(Startup, spawn);
    }
}
