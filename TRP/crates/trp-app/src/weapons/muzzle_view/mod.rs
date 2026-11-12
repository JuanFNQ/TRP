//! A weapon's muzzle flash.
//! After the firing spec, "The muzzle flash"; the options' `MUZZLE FLASH`: the jets' mesh the rig
//! was made with (`weapon::MuzzleParts`, `trp_sim::muzzle::jet_mesh`), drawn by the jets' shader
//! (`muzzle_view.wgsl`, the game's `Custom/Additive`) at the weapon's `weapon` object, each shot
//! giving it new draws and its time.
//!
//! The game adds the flash's numbers to its frame in gamma space (its
//! project is: `gamma_ui`). Added to Bevy's linear frame in its own pass
//! the flash came out several times dimmer over the bright range. So it is
//! drawn after the tonemapping, onto the frame encoded for the camera's
//! effects, before the first of them (as the game draws it before its image
//! effects), its depth tested by hand against a single-sample copy of the
//! scene's (`scene_depth.wgsl`). `PROVISIONAL`: after Bevy's tonemapping,
//! where the game's flash goes through its own.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_3d, texture_depth_2d, texture_depth_2d_multisampled, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{CachedTexture, GpuImage, TextureCache};
use bevy::render::view::{Msaa, ViewDepthTexture, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, ShaderDefVal};
use trp_data::weapons::muzzle::MuzzleData;
use trp_sim::weapons::muzzle::{JetVertex, MuzzleFlash, MuzzleLight, Smoke};
use trp_sim::weapons::recoil::Rng;

use crate::platform::convert;

mod jet_data;
mod flash;
mod pipeline;
mod nodes;

pub use jet_data::*;
pub use flash::*;
pub(crate) use pipeline::*;
pub(crate) use nodes::*;
use jet_data::{Flash, tiled};
use pipeline::{JetKey, JetPipeline, init_pipeline};
use nodes::{DepthCopyNode, JetBuffers, JetNode, prepare};

const JET_SHADER: Handle<Shader> = uuid_handle!("9a3e6c27-4d81-4b5f-8e2a-0c7b5d1f3e96");
const DEPTH_SHADER: Handle<Shader> = uuid_handle!("2c84f1d3-6e59-4a07-b3d8-91e5c7a4f206");
const SMOKE_SHADER: Handle<Shader> = uuid_handle!("6f0b3d92-8a14-4c75-9e3b-d27a5c8e1f40");
const FUME_SHADER: Handle<Shader> = uuid_handle!("c41a7e5d-2b96-4f83-a0d7-5e8c3b19f6a2");
const SPARKS_SHADER: Handle<Shader> = uuid_handle!("7d2e9b40-5c13-4a86-b1f7-3e60a8d4c295");
/// The tactical devices' lasers (`tactical_view`), drawn in this pass.
const LASER_SHADER: Handle<Shader> = uuid_handle!("5b8e1d47-9a26-4c3f-8e71-2d4a6f0c9b13");
const TRACER_SHADER: Handle<Shader> = uuid_handle!("e3a91c58-7d24-4b6f-9a05-c18d2f7b4e63");
/// The lights' flares (`flare_view`), drawn in this pass.
const FLARE_SHADER: Handle<Shader> = uuid_handle!("4f7c2a91-3e58-4d06-b8a1-6c9e0d2f5b37");

pub struct MuzzleViewPlugin {
    /// The flash read and drawn (`MUZZLE FLASH`), its light with it; off,
    /// only its pass is in the graph, and never runs.
    pub on: bool,
    /// The range lit by its own light (`CUSTOMS LIGHT` off): the camera at
    /// Bevy's own exposure (`tactical_view::exposure_gain`).
    pub own_light: bool,
}

impl Plugin for MuzzleViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, JET_SHADER, "../../shaders/muzzle_view.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, DEPTH_SHADER, "../../shaders/scene_depth.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, SMOKE_SHADER, "../../shaders/muzzle_smoke.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, FUME_SHADER, "../../shaders/smoke_distorted.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, SPARKS_SHADER, "../../shaders/simple_sparks.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, LASER_SHADER, "../../shaders/laser.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, TRACER_SHADER, "../../shaders/tracer.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, FLARE_SHADER, "../../shaders/flare.wgsl", Shader::from_wgsl);
        if self.on {
            app.insert_resource(MuzzleViews { exposure_gain: crate::weapons::tactical_view::exposure_gain(self.own_light), ..default() }).init_resource::<JetMeshes>().add_plugins((
                ExtractComponentPlugin::<JetScreen>::default(),
                UniformComponentPlugin::<JetUniform>::default(),
                ExtractResourcePlugin::<JetMeshes>::default(),
                ExtractComponentPlugin::<SmokeScreen>::default(),
            ));
            // After the cameras' projections are made from this frame's
            // fields of view (the optic camera's moves as a scope zooms).
            app.add_systems(PostUpdate, (jet_screen, smoke_frame).after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems));
        }
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<JetPipeline>>()
            .init_resource::<JetBuffers>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<DepthCopyNode>>(Core3d, DepthCopyLabel)
            .add_render_graph_node::<ViewNodeRunner<JetNode>>(Core3d, JetLabel);
    }
}
