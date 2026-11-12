//! Things on the ground, moved as the game's physics moves them.
//! After `docs/spec/physics.md`, "Loose things"; `trp_physics::loose`: a thing put down from the
//! inventory or a backpack dropped is thrown as the game's `ThrowItem` throws it, the magazine a
//! quick reload lets go falls from where the weapon had it (`DropMod`), a live round the weapon
//! throws out leaves its shell port (`ThrowPatronAsLoot`), and each, with its own model and its
//! prefab's colliders, falls in the game's engine against the range until the game would take its
//! body away.
//!
//! A thing's model is read on a thread of its own, as its picture's is
//! (`icons::Worker::model`): until it has come the thing is not drawn, and
//! its body, once made, is stepped through every step it missed, so that
//! it falls as it would have (it meets nothing that moves: the game's
//! `Loot` layer meets neither itself nor a corpse nor the character). Each
//! thing falls in a world of its own for that reason. While the inventory
//! screen is open the reader is kept, and a weapon the screen may put down
//! next has its model asked for ahead (`foreseen`): the game makes a thrown
//! thing's object in the throw itself.
//!
//! A thing on the ground that was not thrown (the range's own, put out
//! when the run begins, or a round the weapon put down) is set on the
//! ground and let settle (this work's layout, not the game's: its things
//! are where the map puts them).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task};
use bevy::transform::helper::TransformHelper;
use trp_data::setup::engine::PhysicsSettings;
use trp_physics::loose::{self, Dice, Start, Thrower, Watch};
use trp_physics::{Body, Collider, Material, Shape, World, V3};
use trp_sim::items::inventory::Id;

use crate::player::controller::Controller;
use crate::platform::convert;
use crate::screens::icons::{Model, Worker};
use crate::screens::inventory_view::Carried;
use crate::world::range::Range;
use crate::weapons::weapon::WeaponRig;

mod loose_state;
mod models_read;
mod set_off;
mod flying;
mod uses;
mod foreseen;
#[cfg(test)]
mod tests;

pub use loose_state::*;
pub use models_read::*;
pub use set_off::*;
pub use flying::*;
use loose_state::{Asked, Flight, GrenadeBody, Moving, Prepared, Read};
use models_read::{GRENADE_MODEL, drawn};
use uses::Uses;
use foreseen::{Foreseen, Signs};
