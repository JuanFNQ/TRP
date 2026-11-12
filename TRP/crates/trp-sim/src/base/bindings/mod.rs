//! The game's keys as its input manager hears them, and its rules for changing them.
//! After `docs/spec/settings.md`, "How a key becomes a command" and
//! "Changing a key".
//!
//! A binding is the game's name for what a key does (`EGameKey`), up to two
//! variants of keys held together (a variant may end in a notch of the
//! mouse wheel), and a way of being pressed. The input manager makes one
//! combination of each variant and runs them all every frame, the one with
//! the most keys first; a combination that takes its keys keeps them from
//! the shorter ones, which is how `T` with `Left Alt` is not also `T`. What
//! comes out is which bindings were given this frame, and for those that
//! hold (a lean, a sprint bound to be held) which were let go.
//!
//! No engine types: a key is the engine's own number for it
//! (`UnityEngine.KeyCode`, whose names the install's `Control.ini` is
//! written in), and a host says which are down. `install_controls` reads
//! the install's own bindings into these.

mod keys;
mod groups;
mod controls;
mod preset;
mod commands;

pub use keys::*;
pub use groups::*;
pub use controls::*;
pub use preset::*;
use keys::{FUNCTION_KEYS};
use groups::{FIXED_PRESS, GENERATED, MERGE, NEVER_YIELDS, PRIORITIES, UNRANKED, UPDATE};
use preset::{KeyId, Kind, State};

#[cfg(test)]
mod tests;
