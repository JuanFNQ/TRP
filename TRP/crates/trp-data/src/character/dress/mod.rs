//! What a character wears over its clothing: armour, vests, backpacks,
//! helmets, headsets. An item's bundle is one of two kinds
//! (`docs/spec/body-animator.md`, "Worn items"): a skin for the player's
//! skeleton, built like a piece of clothing, or a plain model that is hung
//! on the head.

use anyhow::{anyhow, Result};

use crate::character::arms::{self, Arms};
use crate::formats::curve::Curve;
use crate::setup::database::Database;
use crate::unity::extract::{exported_game_objects, read_hierarchy, script_class, Hierarchy};
use crate::unity::{class, Assets, ObjRef, Value};

mod hoods;
mod head_parts;
mod fittings;
mod loading;

pub use hoods::*;
pub use head_parts::*;
pub use fittings::*;
pub use loading::*;
use hoods::{arm_band};
use fittings::{game_object_of};

/// What else is worn, which some skins choose their mesh by: a backpack
/// sits further out over a vest, a vest over armour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Under {
    pub armour: bool,
    pub vest: bool,
}

pub enum Worn {
    /// Skinned to the bones it names by path, like clothing.
    Skinned(Arms),
    /// A model of its own, for the head bone.
    Rigid(Hierarchy),
    /// An armband: skins whose meshes go by the upper body worn.
    Band(ArmBand),
}
