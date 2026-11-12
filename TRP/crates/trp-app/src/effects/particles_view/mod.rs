//! The particle system's effects.
//! After the particles spec; the options' `PARTICLES`: the muzzles' fumes (firing spec, "The
//! muzzle's fumes"), sparks ("The muzzle's sparks") and heat haze ("The muzzle's heat haze", drawn
//! by the camera's distortion: `distort_view`), and a hot weapon's parts' shimmer (malfunctions
//! spec, "The shimmer over a hot barrel"), drawn so too. The shared system all the weapons' fumes
//! are emitted into is read once from the install (`trp_data::particles::muzzle_fume_system`); each
//! shot of the weapon in hand emits its fumes' puffs into it by the game's rule
//! (`trp_sim::muzzle::fume_puffs`), and it is stepped each frame (`trp_sim::particles`). What it
//! draws is handed to the camera (`FumeScreen`) and drawn by the muzzle's pass (`muzzle_view`,
//! before the flash's jets), with the game's shader (`smoke_distorted.wgsl`).

use std::collections::HashMap;
use std::sync::Arc;

use bevy::ecs::query::QueryItem;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::ShaderType;
use trp_sim::weapons::muzzle::FastRandom;
use trp_sim::effects::particles::ParticleSystem;
use trp_sim::weapons::recoil::Rng;
use trp_sim::effects::sparks::SparksRing;

mod fume_draw;
mod spark_draw;
mod haze_draw;

pub use fume_draw::*;
pub use spark_draw::*;
pub use haze_draw::*;

pub struct ParticlesViewPlugin {
    /// The fumes read and drawn (`PARTICLES`, where the hard settings have
    /// shot effects); off, nothing of them is read, and the muzzle's pass
    /// never draws them.
    pub on: bool,
}

impl Plugin for ParticlesViewPlugin {
    fn build(&self, app: &mut App) {
        if self.on {
            app.add_plugins((ExtractComponentPlugin::<FumeScreen>::default(), ExtractComponentPlugin::<SparkScreen>::default(), ExtractComponentPlugin::<HazeScreen>::default()))
                .add_systems(
                    PostUpdate,
                    (fume_frame.run_if(resource_exists::<Fumes>), spark_frame.run_if(resource_exists::<Sparks>), haze_frame.run_if(resource_exists::<Hazes>))
                        .after(bevy::transform::TransformSystems::Propagate)
                        .after(bevy::camera::CameraUpdateSystems),
                );
        }
    }
}
