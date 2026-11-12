//! The game's key bindings as the install has them.
//! After `Control.ini`, read by `trp_data::Install::key_bindings` and `axis_bindings`: heard as the
//! game's input manager hears them (`trp_sim::bindings`; `docs/spec/settings.md`): every key the
//! app takes in play is one of these, by the game's own name for what it does (`EGameKey`), pressed
//! the way the install has it pressed.
//!
//! The options tab changes them while the app runs (`options_view`);
//! what it changes is kept in the app's own file, never in the install.

use std::collections::HashMap;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use trp_sim::base::bindings::{code, AxisGroup, Code, Controls, Heard, KeyGroup, MouseHeard, PressType, Preset, Source};

mod key_tables;
mod controls;
mod bound_keys;

pub use key_tables::*;
pub use controls::*;
pub use bound_keys::*;
use key_tables::{AS_READ, AXES_AS_READ, as_read, engine_keys};
#[cfg(test)]
use key_tables::{NAMED};
use controls::{key_alias};

#[cfg(test)]
mod tests;
