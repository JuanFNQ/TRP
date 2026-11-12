//! The range as a file to edit in Blender and read back.
//! The file is glTF 2.0, which Blender imports and exports as it
//! ships and Bevy reads; `--export-level` writes the range as it stands,
//! and `--level` reads one back at the start, to try an edit for a run
//! (`docs/manual.md`, "The range in Blender"); the range itself is the
//! code's (`range`), an edit taken into it, with no switch between levels.
//! The app's own; nothing of the game's is in it.
//!
//! What the file holds, each by its name (Blender keeps an object's name,
//! adding `.001` to a copy's):
//!
//! - **boxes**: every wall, floor, step and block, each an upright box
//!   (a mesh's bounds, by its place, turn about the upright and size),
//!   its colour its material's and what it sounds of underfoot the first
//!   word of the material's name (one of the game's twenty surfaces,
//!   `trp_data::sound::SURFACES`; concrete if none). Moved, sized, turned,
//!   copied or deleted at will. A box named `roof` (or `roof ...`) is a
//!   roof: drawn and met by sight and blasts, but no part of the
//!   character's own collision, whose ground is the highest top over its
//!   feet (`range::Solid::overhead`).
//! - **`prop <name>`**: each thing the hands work with what it stands on
//!   (the doors, the crates, the tables with their cases and boxes, the
//!   storage, the car, the barbed wire): moved and turned as one, its
//!   parts drawn under it for the eye only. But the sampler of surfaces'
//!   parts, `sampler part <n>`: each the square of the game's `n`th
//!   surface, where it is and as large as it is drawn.
//! - **`start <name>`**: where `--start` puts the character and which way
//!   it looks, by its arrow (`start <name> arrow`, under it, from the feet
//!   ahead; moved or turned alone, it still says), or the empty's own
//!   place and turn where it has no arrow; a prop's own under it, to go
//!   with it.
//! - **`lamp <n>`**: the hall's ceiling lamps (`HALL LAMPS`).
//! - **`indoors`**: the box inside which the camera takes the level's
//!   indoor exposure and the sky's ambient is taken away (`lighting`,
//!   "The range hall"); none, and the range is all outdoors.
//! - **`ground`**: the ground, drawn for the eye; not read.

use bevy::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

mod level_read;
mod gltf_write;

pub use level_read::*;
pub use gltf_write::*;
#[cfg(test)]
use level_read::{PLAIN, clean, surface_named};
#[cfg(test)]
use gltf_write::{base64, unit_cube};

#[cfg(test)]
mod tests;
