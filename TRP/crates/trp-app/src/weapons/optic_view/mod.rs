//! A scope's picture-in-picture, as the game draws it.
//! After the optics spec, "The optic camera", "The lens"; with the options' `ATTACHMENTS`, off
//! which no optic camera is made and a scope's lens keeps the part's own material, a blank grey
//! disc: one optic camera, on while aiming through an optic, draws the world from the current
//! mode's `optic_camera` object at the scope's field of view into a square picture of 1024 pixels;
//! the scope's lens shows it through its own shader (`optic_lens.wgsl`: the picture on a plane
//! ahead of the lens, the shadow on a funnel toward the eye, the reticle on the picture's plane),
//! every lens not in use dimmed. A red dot's or a holographic sight's lens adds its dot
//! (`collimator.wgsl`) whatever the switch; the scopes' front glass and a red dot's window
//! (`CollimatorGlass`, the PK-06's) are not drawn (`PROVISIONAL`: their added reflections are
//! not built; drawn as the part's other materials are, the window was an opaque disc over the dot).

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
use trp_data::weapons::sight::{LensKind, LensNode};
use trp_data::unity::extract::TextureData;

use crate::weapons::weapon::{HandsRig, WeaponRig};
use crate::{MainCamera, View};

mod culling;
mod materials;
mod optic_camera;
mod reticle;

pub use materials::*;
use optic_camera::{dress_lenses, place_camera, spawn_camera, turn_sights};
use reticle::{show_reticle};

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
    /// Whether the scopes draw their picture (the options' `ATTACHMENTS`).
    pub pictures: bool,
}

impl Plugin for OpticViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, LENS_SHADER, "../../shaders/optic_lens.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, COLLIMATOR_SHADER, "../../shaders/collimator.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, RETICLE_SHADER, "../../shaders/reticle.wgsl", Shader::from_wgsl);
        app.add_plugins((MaterialPlugin::<LensMaterial>::default(), MaterialPlugin::<CollimatorMaterial>::default(), MaterialPlugin::<ReticleMaterial>::default()))
            .init_resource::<LensPictures>()
            .add_systems(Update, (dress_lenses, turn_sights, place_camera, show_reticle).chain().after(crate::player::hands_view::apply_hands));
        if self.pictures {
            app.add_systems(PostStartup, spawn_camera);
            use bevy::camera::visibility::VisibilitySystems;
            app.add_systems(PostUpdate, (culling::cull_unseen, culling::count_drawn).chain().after(VisibilitySystems::CheckVisibility).before(VisibilitySystems::MarkNewlyHiddenEntitiesInvisible));
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
}

#[allow(dead_code)]
mod uniforms;
pub use uniforms::{CollimatorUniform, LensUniform, ReticleUniform};
