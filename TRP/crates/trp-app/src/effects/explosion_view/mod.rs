//! A grenade's explosion.
//! After the grenades spec, "The blast's effect"; the options' `PARTICLES`: the effect its template
//! names (`ExplosionEffectType`, `Effects.EmitGrenade`), read at the start from the install
//! (`trp_data::particles::named_effect`: a system and every one below it), its mediator's next copy
//! played where the grenade was with its z axis up (`Quaternion.LookRotation(Vector3.up)`), each
//! system run by `trp_sim::particles` and drawn by a port of its material's shader
//! (`particle_effects.wgsl`), in the engine's order among transparent things: by render queue, then
//! far to near by each system's place (its particles' middle) plus its sorting fudge. On the frame
//! encoded for the camera's effects, before the muzzle's (`muzzle_view`), as the game's
//! (`gamma_ui`). The particles of a system that meets the world meet the range in the game's
//! physics engine (a world of the range's own, `loose::range_world`;
//! `trp_sim::particles::ParticleSystem::step_in`), and each meeting sets off the systems its
//! sub-emitters name for a collision (the concrete's dust); the lights module's lights are Bevy's
//! point lights (`ExplosionLight`). A mesh's particles (the shock wave, on Unity's own sphere) are
//! drawn as the mesh at each; a shader that grabs the frame (the shock wave's and the heat's
//! distortions) is given a copy of the frame as it stands before it is drawn. The scorch is
//! `decal_view`'s. Beside the mediators' effects: an entry's older emitters onto the effects'
//! shared systems and sparks, and the pool's light (the flashbang's: `trp_sim::emitters`); a
//! launcher's blasts played at its muzzle's pivots at each shot, with their trails and their driven
//! light (firing spec, "A launcher's blasts"); the engine's vertex lights for the vertex-lit
//! shaders. A bullet's hit plays its material's entry the same way (`impacts`; ballistics spec,
//! "What a hit shows and sounds"). The older emitters draw from the game's own tables
//! (`FastRndom`), a spark's place from Unity's generator.

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
use trp_data::drawing::particles::{ParticleEffect, ParticleSystemData};
use trp_sim::effects::particles::{ParticleSystem, Placement};
use trp_sim::weapons::recoil::Rng;

mod looks;
mod ready_parts;
mod preparing;
mod exploding;
mod camera_view;
mod effects_pass;

pub use looks::*;
pub use ready_parts::*;
pub use preparing::*;
pub use exploding::*;
pub(crate) use effects_pass::*;
use looks::{AMBIENT, EFFECTS_SHADER, effect_light};
use ready_parts::{Copies, DrivenLight, Engine, FrameLight, MuzzleBlast, PARTICLE_RAYS, ReadyEffect, ReadyEmitter, ReadyEntry, ReadyKind, ReadySystem, ReadyTrail, VertexLight, figures_of, vertex_lights};
use preparing::{emit_entry};
use exploding::{add3};
use camera_view::{FrameFigures, camera_draws};
#[cfg(test)]
use camera_view::{held_size};
use effects_pass::{EffectsPipeline, ExplosionNode, init_pipeline, prepare};

pub struct ExplosionViewPlugin {
    /// The explosions read and drawn (`PARTICLES`); off, nothing of them is
    /// read, and the pass never draws.
    pub on: bool,
}

impl Plugin for ExplosionViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, EFFECTS_SHADER, "../../shaders/particle_effects.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, crate::effects::offscreen_dust::SHADER, "../../shaders/offscreen_dust.wgsl", Shader::from_wgsl);
        if self.on {
            app.add_plugins(ExtractComponentPlugin::<ExplosionScreen>::default())
                // After the bullets' frame, whose rounds' hits blast in it.
                .add_systems(Update, explode.after(crate::weapons::bullets::fly_bullets).run_if(resource_exists::<Explosions>))
                .add_systems(PostUpdate, explosion_frame.run_if(resource_exists::<Explosions>).after(bevy::transform::TransformSystems::Propagate).after(bevy::camera::CameraUpdateSystems));
        }
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<EffectsPipeline>>()
            .init_resource::<SpecializedRenderPipelines<crate::effects::offscreen_dust::DustPipeline>>()
            .add_systems(RenderStartup, (init_pipeline, crate::effects::offscreen_dust::init_pipeline))
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_render_graph_node::<ViewNodeRunner<ExplosionNode>>(Core3d, ExplosionLabel);
    }
}

#[cfg(test)]
mod tests {
    use super::held_size;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    // A view 2 m wide where the particle is (a perspective camera's width
    // at its depth): its least and most are shares of that.
    #[test]
    fn a_billboard_is_held_within_its_shares_of_the_view() {
        assert!(near(held_size(0.5, 0.0, 0.5, 2.0), 0.5), "within: as it is");
        assert!(near(held_size(3.0, 0.0, 0.5, 2.0), 1.0), "held to half the view");
        assert!(near(held_size(0.1, 0.25, 10.0, 2.0), 0.5), "raised to a quarter of it");
        assert!(near(held_size(3.0, 0.0, f32::MAX, 2.0), 3.0), "a most not read holds nothing");
    }

    // Behind the camera the width is below nought: a least above nought
    // makes the particle nought, a least of nought leaves it and the most
    // holds nothing (`GenerateParticleGeometry`'s signs).
    #[test]
    fn behind_the_camera() {
        assert!(near(held_size(1.0, 0.1, 0.5, -2.0), 0.0));
        assert!(near(held_size(1.0, 0.0, 0.5, -2.0), 1.0));
    }
}
