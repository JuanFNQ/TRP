//! Readers for a user's own Single Player Tushonka (SPT) install: the
//! game's files under it and SPT's database.
//!
//! Nothing here patches or writes to the install. Everything is read in place.
//!
//! One thing here is not the install's: a weapon of one's own (`custom`),
//! whose model (`fbx`) lies in the workspace's `local` folder.

pub mod arms;
pub mod body;
pub mod compress;
pub mod curve;
pub mod custom;
pub mod database;
pub mod dress;
pub mod engine;
pub mod fbx;
pub mod hard_settings;
pub mod icon;
pub mod install;
pub mod inventory;
pub mod outfit;
pub mod player;
pub mod rootmotion;
pub mod screen;
pub mod sketch;
pub mod states;
pub mod unity;
pub mod weapon;
pub mod xray;
pub mod xray_archive;

pub use database::Database;
pub use install::{Install, Manifest, PlayerSettings};
