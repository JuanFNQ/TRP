//! Searching a container, and what the character has not yet searched or found.
//! After `docs/spec/inventory.md`, "Searching a container": which containers the character has not yet searched and
//! which things in them it has not yet found (`ActiveSearchController`),
//! and the search that opens one and finds what is in it one thing after
//! another (`ActiveSearchContentOperation`): two seconds to open a
//! container never searched, then each thing a second or two over
//! 1 + Attention's `AttentionLootSpeed` (and Search's `SearchBuffSpeed` for
//! a container on a body); at Attention's elite a first search over at once
//! one time in two; at Search's elite two searches at once.
//!
//! What the character carries, and anything it puts into a container
//! itself, is known: only what was in a container hidden here (`hide`) is
//! to be found, and a thing not found cannot be moved, so nothing found
//! needs forgetting. The range's crates, and the loot they are filled with
//! as a run begins (`fill`: the app's own, by SPT's tables), are what is
//! hidden.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use trp_data::setup::database::Database;
use trp_data::level::loot::LootTable;

use crate::items::inventory::{is, Id, Item, Kind};

mod searcher;
mod filling;

pub use searcher::*;
pub use filling::*;

/// The wait before a container never searched shows its grids, in seconds
/// (`ActiveSearchContentOperation.ACCESS_SECONDS`, waited as
/// `Task.Delay(2000)` unless the search is over at once).
pub const OPEN_TIME: f64 = 2.0;

/// What the skills bring to a search (`IInventoryProfileSkillInfo`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Skills {
    /// Attention's `AttentionLootSpeed` (0.02 a level).
    pub loot_speed: f32,
    /// Search's `SearchBuffSpeed` (0.01 a level): a container on a body.
    pub body_speed: f32,
    /// Attention's elite `AttentionEliteLuckySearch` (0.5; nought below).
    pub lucky: f32,
    /// Search's elite `SearchDouble`: two searches at once.
    pub double: bool,
}

impl Skills {
    /// A search's pace (`SearchContent`): 1 + the loot speed, and the body
    /// speed besides where the container lies in someone's equipment
    /// (`Item.Parent.GetOwner().RootItem is InventoryEquipment`).
    pub fn speed(&self, on_body: bool) -> f32 {
        1.0 + self.loot_speed + if on_body { self.body_speed } else { 0.0 }
    }
}

/// Whether a thing hides what lies in its grids from one who has not
/// searched it (`SearchableItem`: a backpack, a vest, the pockets, a loot
/// container, a body's container; by the database's class of that name,
/// which theirs all descend from).
pub fn searchable(item: &Item) -> bool {
    is(item, "SearchableItem")
}

/// Whether a container lies on a body, so that Search's speed counts
/// (`Item.Parent.GetOwner().RootItem is InventoryEquipment`): what holds it
/// at the top is someone's equipment (a bag found elsewhere and put on
/// counts as well).
pub fn on_body(root: &Item) -> bool {
    root.kind.template == crate::items::inventory::EQUIPMENT_TEMPLATE
}

/// The slots of a body's equipment that others see, in the order the
/// containers' column shows them (`ContainersPanel._slotNames` less what
/// `IsAllowedToSeeSlot` hides from others: the secured container).
pub const BODY_CONTAINERS: [&str; 4] = ["TacticalVest", "Pockets", "Backpack", "Dogtag"];

/// Whether a looter sees a slot of a body's equipment
/// (`IsAllowedToSeeSlot`, `IsAllowedToSeeEquipmentSlot`: the secured
/// container is hidden from all but its owner, and a deleted slot from
/// all; a Scav's armband slot is deleted as its inventory is made,
/// `InventoryController.ReplaceInventory` for the Savage side). A Scav's
/// dog tag slot stays, and shows empty.
pub fn seen_on_body(slot: &str, scav: bool) -> bool {
    slot != "SecuredContainer" && !(scav && slot == "ArmBand")
}

/// A Scav's body dressed as a copy of the player's gear (the app's
/// dummies, which are Scavs): every thing with a number of its own, less the dog tag (a Scav
/// carries none) and the armband (its slot deleted, `seen_on_body`).
pub fn scav_body(equipment: &Item) -> Item {
    let mut body = equipment.renumbered();
    for slot in ["Dogtag", "ArmBand"] {
        if let Some(index) = body.slot_index(slot) {
            body.slots[index] = None;
        }
    }
    body
}

/// The searcher's dice (the engine's `Random`, not the game's own stream:
/// what is rolled, and its odds, are the game's).
#[derive(Debug, Clone, PartialEq)]
pub struct Dice(u64);

impl Dice {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    /// From nought up to one, one left out.
    pub fn unit(&mut self) -> f64 {
        // xorshift64*.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A whole number from `least` to `most`, both in.
    pub fn whole(&mut self, least: u32, most: u32) -> u32 {
        if most <= least {
            return least;
        }
        least + ((self.unit() * (most - least + 1) as f64) as u32).min(most - least)
    }
}

/// How long the next thing takes to find, in seconds (`SearchContent`:
/// `Random.Range(1, 3)`, one or two, over the pace, waited in whole
/// milliseconds, `(int)(… * 1000)`; nothing for a search over at once).
pub fn find_time(dice: &mut Dice, speed: f32, instant: bool) -> f64 {
    if instant {
        return 0.0;
    }
    let seconds = dice.whole(1, 2) as f32 / speed;
    (seconds * 1000.0).floor() as f64 / 1000.0
}
