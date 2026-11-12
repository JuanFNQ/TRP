//! What the character carries and where each thing sits, by the game's rules.
//! After `docs/spec/inventory.md`: items with slots and grids, what a
//! slot and a grid accept, where a grid finds room, an item's size in
//! cells and its weight, and the carried weight the controller goes by.
//! No engine types; a host fills it and reads it.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use trp_data::character::inventory::{Holder, Loadout};
use trp_data::Database;

mod kinds;
mod item;
mod equipment;
mod chambers;
mod rules;
mod moves;
mod transfers;
mod wearing;
mod rounds;
mod switches;
mod loading;
mod magazines;
mod menus;

pub use kinds::*;
pub use item::*;
pub use equipment::*;
pub use chambers::*;
pub use rules::*;
pub use moves::*;
pub use transfers::*;
pub use wearing::*;
pub use rounds::*;
pub use switches::*;
pub use loading::*;
pub use magazines::*;
pub use menus::*;
use kinds::{own, unknown_to_own};
use chambers::{CAMORA, CHAMBER, camoras, into_cylinder, is_chamber, merges, own_chambers, takes_rounds};
use rules::{BACKPACK, within_limits};
use moves::{malfunction_refuses, root_of};
use transfers::{stacked_within_limits};
use rounds::{WITHIN_REACH};
use magazines::{unloaded_place_of};
use menus::{chamber_closed, chamber_held};

/// The character's equipment slots, in the game's order (`EquipmentSlot`).
pub const SLOTS: [&str; 15] = [
    "FirstPrimaryWeapon",
    "SecondPrimaryWeapon",
    "Holster",
    "Scabbard",
    "Backpack",
    "SecuredContainer",
    "TacticalVest",
    "ArmorVest",
    "Pockets",
    "Eyewear",
    "FaceCover",
    "Headwear",
    "Earpiece",
    "Dogtag",
    "ArmBand",
];

/// The slots whose item is drawn on the body (`AllVisualSlotNames`).
pub const WORN: [&str; 8] = ["Backpack", "TacticalVest", "ArmorVest", "Eyewear", "FaceCover", "Headwear", "Earpiece", "ArmBand"];

/// The slots that still weigh something with Strength at its elite level.
const ELITE_WEIGHT: [&str; 9] = ["TacticalVest", "Pockets", "Backpack", "SecuredContainer", "Earpiece", "Headwear", "Eyewear", "FaceCover", "ArmorVest"];

/// The template of the equipment item itself ("Default Inventory").
pub const EQUIPMENT_TEMPLATE: &str = "55d7217a4bdc2d86028b456d";

/// What stands for an item wherever it lies: a number of its own.
pub type Id = u64;

fn next_id() -> Id {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests;
