//! The range lit as Customs in the recorded morning.
//! After `docs/spec/lighting.md`, "Customs by its time of day": with the options' `CUSTOMS LIGHT`
//! on: the sun, the ambient harmonics as an environment map, the sky (`tod_sky_view`), all worked
//! out by the game's time-of-day rules from the install (`trp_sim::tod`); the camera in HDR with no
//! tonemapper of Bevy's (PRISM's in its place, `prism_view`), and the figures the effects' shaders
//! are lit by.
//!
//! The game shades in its gamma space and the app in Bevy's linear light;
//! the frame is turned into the game's numbers after the scene (`gamma_ui`,
//! by a plain 2.2 power with this light: the sRGB curve's straight part near
//! black would make a product of two dim figures, an albedo and an ambient,
//! three times darker than the game's). So each of the game's figures is
//! given to Bevy made linear by the same power, and Bevy's exposure is put
//! where a light of intensity one is one (EV100 0, as the icon studio's:
//! `icons`). `PROVISIONAL` (lighting spec, "On the range"): a product of a
//! picture's colour and one light is the game's (the picture's darkest
//! colours a little lighter: Bevy reads a picture by the sRGB curve); sums
//! and fall-off with angle are linear light's.

use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
use trp_data::drawing::prism::{CameraScattering, ColorFilter, PrismSettings};
use trp_data::interface::screen::BuildMesh;
use trp_data::level::tod::TimeOfDayLevel;
use trp_sim::lighting::ambient::Harmonics;
use trp_sim::lighting::tod::{Moment, SkyGlobals, CUSTOMS_RECORDED_MORNING};

mod level_lighting;
mod hall_light;

pub use level_lighting::*;
pub use hall_light::*;
use level_lighting::{cube};
#[cfg(test)]
use level_lighting::{face_way, half};
use hall_light::{LAMP_COLOR, LAMP_CONE, LAMP_INTENSITY, LAMP_RANGE};

#[cfg(test)]
mod tests;
