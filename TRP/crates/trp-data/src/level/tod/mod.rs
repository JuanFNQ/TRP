//! A level's time of day as the game sets it up.
//! After `docs/spec/lighting.md`, "Customs by its time of day": the Time of Day sky (`TOD_Sky`: the
//! world's place on the globe, the day's and night's gradients, the sun's,
//! the moon's and the stars' figures, the cycle and the atmosphere), the
//! game's time-of-day controller inside its weather controller
//! (`WeatherController.TimeOfDayController`: the atmosphere's curves by the
//! sun's height, the sun's colour, the ambient harmonics' curves and
//! gradients), the sky dome's place, the sun, the level settings and the
//! environment manager's open air. Read from the scene of the level's
//! scripts; Customs' is `custom_Scripts.unity`.
//!
//! The scripts' fields are read by their classes' order of declaration,
//! written here as lists (`Kind`), nested classes inline.

use anyhow::{bail, Context, Result};

use crate::drawing::eye_burn::pointer;
use crate::level::level_light::{color, gradient, light, settings, Outdoor, Settings, Sun};
use crate::drawing::particles::Gradient;
use crate::interface::screen::{build_mesh, scene_paths, BuildMesh, Scene, MESH_FILTER};
use crate::unity::reader::Reader;
use crate::Install;

mod fields;
mod level_settings;

pub use fields::*;
pub use level_settings::*;

/// The scene of Customs' scripts, with its weather and its sky.
pub const CUSTOMS: &str = "custom_Scripts.unity";

/// What a script's field is, as the engine keeps it.
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// A float.
    F,
    /// A whole number (an enum, a layer mask).
    I,
    /// A flag: one byte, padded to four.
    Flag,
    Color,
    Vec2,
    Vec3,
    Gradient,
    Curve,
    /// A pointer to an object (12 bytes).
    Pointer,
    /// A serializable class or struct, its fields inline.
    Class(&'static [(&'static str, Kind)]),
}

use Kind::*;

