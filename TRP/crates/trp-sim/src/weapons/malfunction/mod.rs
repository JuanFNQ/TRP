//! A weapon's wear, its malfunctions and its heat, as pure rules.
//! After `docs/spec/malfunctions.md`: the game's `Weapon.MalfunctionState`, `Weapon.OnShot`, `GetCurrentOverheat`
//! and the firearm controller's roll (`GetTotalMalfunctionChance`,
//! `GetMalfunctionSources`, `GetSpecificMalfunctionVariants`), as pure rules.
//! The weapon's logic (`crate::Firearm`) asks them at each shot and keeps
//! what they leave; the host keeps the weapon's wear and its state with the
//! weapon's item between draws.

use anyhow::{Context, Result};
use serde_json::Value as Json;

use crate::base::math::lerp;
use crate::base::unity_random::UnityRandom;

mod settings;
mod rolling;

pub use settings::*;
pub use rolling::*;

/// A malfunction (`Weapon.EMalfunctionState`; the animator's whole numbers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Malf {
    #[default]
    None,
    Misfire,
    Jam,
    HardSlide,
    SoftSlide,
    Feed,
}

impl Malf {
    /// The game's number for it, which the animator's `Malfunction` and
    /// `MalfType` take.
    pub fn number(self) -> i32 {
        match self {
            Malf::None => 0,
            Malf::Misfire => 1,
            Malf::Jam => 2,
            Malf::HardSlide => 3,
            Malf::SoftSlide => 4,
            Malf::Feed => 5,
        }
    }

    /// The key of its name in the game's texts (`EMalfunctionState/...`:
    /// "Misfire", "Failure to eject", "Bolt jammed" for either slide,
    /// "Failure to feed").
    pub fn text_key(self) -> &'static str {
        match self {
            Malf::None => "EMalfunctionState/None",
            Malf::Misfire => "EMalfunctionState/Misfire",
            Malf::Jam => "EMalfunctionState/Jam",
            Malf::HardSlide => "EMalfunctionState/HardSlide",
            Malf::SoftSlide => "EMalfunctionState/SoftSlide",
            Malf::Feed => "EMalfunctionState/Feed",
        }
    }
}

/// Where a malfunction came from (`Weapon.EMalfunctionSource`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    #[default]
    Durability,
    Ammo,
    Magazine,
    Overheat,
}

#[cfg(test)]
mod tests;
