//! A contusion's screen effects.
//! After the health spec, "A contusion's screen effects"; `trp_sim::screen_effects` works out what
//! shows: on the camera's picture after its tonemapping and before the inventory's blur, in the
//! game camera's order, the double vision, the wiggle and the motion blur, each by the game's
//! compiled shader's own sums (`contusion_view.wgsl`). The motion blur keeps a picture of its own
//! per view, made from the frame as it comes on, which takes each frame by `1 − amount` (blended
//! over, its colour only) and is what is shown. Drawn in the picture's own (linear) colour
//! (`PROVISIONAL`: the game's camera's colour space not read). Runs with `HEALTH SYSTEM` on. Also a
//! flash's white and the eyes' burn (grenade spec, "A flash"): the burn's spots
//! (`trp_sim::screen_effects::EyeBurn`) worked out per pixel from their list (`Custom/Eye Burn`'s
//! sums, combined by their largest as its blend is), laid over the frame by a screen blend
//! (`Hidden/EyeBurnScreenEffect`). And health's others (health spec, "A hit and health's other
//! screen effects"): pain's radial blur after the double vision, exhaustion's vignette after the
//! burn, and near death's blend last of all, after the inventory's blur, as the game adds it to the
//! camera; a hit's blur is `screen_blur`'s, its value worked out here, and a hit's push on the view
//! the hands' (`Hands::add_force`).

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::texture::GpuImage;
use trp_sim::condition::health::{DamageKind, Kind, Part, Stage};
use trp_sim::effects::screen_effects::{hit_reaction, EyeBurn, EyeBurnFigures, Listed, ScreenEffects, Source, DOUBLE_VISION_DISPLACE};

mod contusion_state;
mod passes;

pub use contusion_state::*;
pub(crate) use passes::*;
use contusion_state::{CONTUSION_SHADER, EYE_BURN_SHADER};
use passes::{Accumulations, ContusionLateNode, ContusionNode, ContusionPipeline, NearDeathNode, init_pipeline, prepare};

pub struct ContusionViewPlugin;

impl Plugin for ContusionViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, CONTUSION_SHADER, "../../shaders/contusion_view.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, EYE_BURN_SHADER, "../../shaders/eye_burn.wgsl", Shader::from_wgsl);
        app.add_plugins((ExtractComponentPlugin::<ContusionScreen>::default(), UniformComponentPlugin::<ContusionScreen>::default()));
        app.add_plugins((ExtractComponentPlugin::<EyeBurnScreen>::default(), UniformComponentPlugin::<EyeBurnScreen>::default(), ExtractResourcePlugin::<ScreenImages>::default()));
        app.add_message::<BurnedEyes>();
        app.add_message::<Received>();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<ContusionPipeline>>()
            .init_resource::<Accumulations>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<ContusionNode>>(Core3d, ContusionLabel)
            .add_render_graph_node::<ViewNodeRunner<ContusionLateNode>>(Core3d, ContusionLateLabel)
            .add_render_graph_node::<ViewNodeRunner<NearDeathNode>>(Core3d, NearDeathLabel)
            .add_render_graph_edges(Core3d, (Node3d::Tonemapping, crate::ui::screen_blur::HitBlurLabel, ContusionLabel, ContusionLateLabel, crate::ui::screen_blur::BlurLabel, NearDeathLabel, Node3d::EndMainPassPostProcessing));
    }
}
