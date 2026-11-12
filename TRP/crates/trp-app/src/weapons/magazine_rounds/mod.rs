//! A magazine's rounds drawn in its model.
//!
//! A magazine's rounds drawn in its model (inventory spec, "A magazine's
//! rounds drawn"; the options' `MAGAZINE ROUNDS`): its `patron_` points
//! (`trp_sim::magazine_view`) filled as the game fills them when its
//! object is made (`SpringMagazineVisual.FillAmmoObjectsOnStart`), each
//! with its round's own model, and on every other frame after, a point
//! numbered past the count switched off (`UpdateCartridgesActiveState`).
//! For the weapon in hand's magazine (its rig's, filled again when the
//! magazine in the weapon changes, its count the logic's: the game's item
//! loses a round at the shot, the app's at rest) and for a magazine on
//! the ground, or a weapon's there (`loose`). A magazine's picture has its
//! rounds too (`icons::picture_key`). The weapon in hand's chambered round
//! is drawn at its chamber's point, and a cylinder's or barrels' rounds at
//! theirs (more-weapons spec, "The rounds and cases seen in chambers of
//! their own"), of the kinds kept beside the logic (`ChamberKinds`).

use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use trp_sim::items::inventory::{Id, Item};
use trp_sim::items::magazine_view::{round_at, stays};

use crate::player::controller::Controller;
use crate::platform::convert;
use crate::screens::inventory_view::{Carried, HandsHeld};
use crate::world::loose::{Loose, Ready, ROUND_MODEL};
use crate::weapons::weapon::{HandsRig, WeaponRig};

mod magazine_shown;
mod chamber;

pub use magazine_shown::*;
pub use chamber::*;
