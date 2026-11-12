//! A weapon's muzzle flash.
//! After the firing spec, "The muzzle flash" (`MuzzleJet.
//! UpdateOrCreateMesh`, `MuzzleJet.RandomizeMaterial`, `MuzzleManager.Shot`,
//! `ManualUpdate`): the jets' mesh as the game builds it when the weapon is
//! put together, each shot's draws for its material, and the shot's
//! figures over its time. The host draws them with the jets' shader.

use std::collections::VecDeque;

use trp_data::weapons::muzzle::{FumeData, HazeData, MuzzleData, SmokeData, SparksData};
use trp_data::unity::extract::Hierarchy;

use crate::weapons::hands::slerp3;
use crate::base::math::{quat_mul, quat_rotate};
use crate::weapons::recoil::Rng;

/// `FastRndom`'s running draws (firing spec, "The muzzle's fumes"): the game's own
/// tables, made by Unity's generator at the game's seeds (`unity_random`).
pub use crate::base::unity_random::FastRandom;

mod flash;
mod haze;
mod smoke;

pub use flash::*;
pub use haze::*;
pub use smoke::*;
use haze::{add, scale, square, sub};

#[cfg(test)]
mod tests;
