//! The face shield's mask over the first person's view.
//! After the inventory spec, "The face shield's mask"; `trp_sim::visor` works out which shield, its
//! fade and its swing: the camera's `VisorEffect` drawn on the frame by the game's compiled
//! shaders' sums (`visor.wgsl`), in the game camera's order (after pain's blur, before the flash:
//! `contusion_view`), on the frame's gamma-encoded numbers as the game's gamma-space camera has
//! them (`gamma_ui`). Its figures and pictures are read from the install's camera
//! (`trp_data::visor`) away from the frame. Drawn in first person only, as the game shows it (the
//! hideout's `VisorVisibility` is its first person's). With `FACE SHIELD MASK` off (or `VISUAL
//! EFFECTS`) nothing of it is read or drawn. With `FACE SHIELD CRACKS` on, the shield's hits are
//! drawn as the game draws them (`DrawScratches`) into a one-channel picture of the view's size
//! (`visor_cracks.wgsl`), at every drawing the camera makes (`trp_sim::visor::Visor::cracks`) and
//! at a change of the view's size, which passes 0 and 1 read.

use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer, uniform_buffer_sized};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::texture::{CachedTexture, GpuImage, TextureCache};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use trp_data::character::visor::{MaskKind, VisorData};
use trp_sim::effects::visor::{crack_quads, shield_in, CrackQuad, Visor, VisorFigures};

mod visor_state;
mod visor_pass;

pub use visor_state::*;
pub(crate) use visor_pass::*;
use visor_pass::{CrackPipeline, VisorNode, VisorPipeline, init_pipeline, prepare};

const VISOR_SHADER: Handle<Shader> = uuid_handle!("6c1e9a47-2d83-4f5b-9e06-b48a3d7c1f92");
const CRACKS_SHADER: Handle<Shader> = uuid_handle!("2b7f40c1-93de-4a16-8e5a-c0d71f3a9b48");

pub struct VisorViewPlugin;

impl Plugin for VisorViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, VISOR_SHADER, "../../shaders/visor.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, CRACKS_SHADER, "../../shaders/visor_cracks.wgsl", Shader::from_wgsl);
        app.add_plugins((
            ExtractComponentPlugin::<VisorScreen>::default(),
            UniformComponentPlugin::<VisorScreen>::default(),
            ExtractResourcePlugin::<VisorImages>::default(),
            ExtractResourcePlugin::<CrackScreen>::default(),
        ));
        app.init_resource::<VisorImages>().init_resource::<Blindness>().init_resource::<CrackScreen>();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<VisorPipeline>>()
            .init_resource::<SpecializedRenderPipelines<CrackPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<VisorNode>>(Core3d, VisorLabel)
            .add_render_graph_edges(Core3d, (crate::effects::contusion_view::ContusionLabel, crate::rendering::sharpen_view::SharpenLabel, VisorLabel, crate::effects::contusion_view::ContusionLateLabel));
    }
}
