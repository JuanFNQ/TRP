//! Tactical devices: the lights and lasers on a weapon, and the head light.
//! After the tactical-devices spec: what each key asks of a
//! weapon's devices in either activation mode (`FirearmHandsInputTranslator`,
//! `GamePlayerOwner.ReleaseTactical`), the weapon's hip penalty they change
//! (`FirearmController.UpdateHipInaccuracy`), and the head light's switch
//! with its gesture, its two waits and its lock (`Player.SwitchHeadLights`
//! and its coroutine). Which device a weapon's part is, its state and its
//! modes are the inventory's (`inventory::Light`, `Kind::light_modes`).

use crate::items::inventory::{is, Id, Item, Light};

mod weapon_devices;
mod headwear;
mod switching;
mod laser;

pub use weapon_devices::*;
pub use headwear::*;
pub use switching::*;
pub use laser::*;

#[cfg(test)]
mod tests;
