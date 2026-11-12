//! A weapon of one's own: a model not the install's, hung on the rig of one of the install's weapons.
//! It is worked by that weapon's animator (`docs/spec/custom-weapons.md`).
//!
//! Such a weapon is described by a file in the workspace's `custom`
//! folder. It becomes an item of the database like any other (its
//! template the donor's with what the file changes, a magazine of its
//! own, a stock preset), and it is assembled as the game assembles a
//! weapon, with two differences: what is drawn is the model's own parts,
//! each hung on one of the donor's bones, and the pose the donor's
//! animator gives is edited, by rules the file states, so that the hands
//! and the moving parts fit the model.
//!
//! Nothing here is read from the install but through the donor; the model
//! and its pictures stay where the file says they are.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value as Json;

use crate::setup::database::{Database, ItemTemplate, Preset, PresetItem};
use crate::formats::fbx;
use crate::unity::extract::{quat_mul, Hierarchy, MeshData, Node};
use crate::unity::ObjRef;

mod weapon_def;
mod part_def;
mod registry;
mod pose_edits;
mod assembly;
mod part_models;
mod picture_cache;

pub use weapon_def::*;
pub use part_def::*;
pub use registry::*;
pub use pose_edits::*;
pub use assembly::*;
pub use part_models::*;
pub use picture_cache::*;
use weapon_def::{PartsFile};
use part_def::{picked, scene_of};
use pose_edits::{Hold, Placed, add, back, of_centimetres, rotate, sub};
use assembly::{mesh_of};
use part_models::material_of;

/// The bone of a weapon's rig the weapon itself hangs from, by its path.
pub const WEAPON_BONE: &str = "Weapon_root/Weapon_root_anim/weapon";
/// The markers the left hand goes by: where it is to be, and the fixed
/// reference for "on the weapon".
const LEFT_MARKER: &str = "weapon_L_IK_marker";
const LEFT_REFERENCE: &str = "weapon_L_hand_marker";
/// How near a palm counts as on a hold, in metres: what keeps the weight
/// of a hold the palm is exactly on from being without end.
const ON_A_HOLD: f32 = 0.005;

#[cfg(test)]
mod tests;
