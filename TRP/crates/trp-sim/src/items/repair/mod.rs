//! Repairing a thing with a repair kit: what the repair window shows and what a repair does.
//! After `docs/spec/workbench.md`, "The repair window": what the game's
//! repair window shows of it, by the client's own reckoning
//! (`RepairController`, `DefaultRepairStrategy`, `ArmorRepairStrategy`,
//! `RepairKitsCollection`, `RepairerParametersPanel`), and what a repair
//! does to the thing and the kits, by the server your install runs
//! (SPT's `RepairService.RepairItemByKit`,
//! `RepairHelper.UpdateItemDurability`: the client computes no result of
//! its own, only the window's preview).
//!
//! A weapon is one part, itself; an armour item (one that holds plates or
//! inserts, or is armour itself) is its armour parts: what wears in it
//! (`RepairableComponent`s), each by its own material and class.

use std::collections::HashMap;

use serde_json::Value as Json;

use crate::items::inventory::{is, Id, Item};

mod settings;
mod planning;
mod kit_buffs;

pub use settings::*;
pub use planning::*;
pub use kit_buffs::*;
use settings::{BUFFED_ARMOUR, ELITE};

#[cfg(test)]
mod tests;
