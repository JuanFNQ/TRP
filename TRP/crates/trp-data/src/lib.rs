//! Readers for a user's own Single Player Tushonka (SPT) install: the
//! game's files under it and SPT's database.
//!
//! Nothing here patches or writes to the install. Everything is read in place.
//!
//! One thing here is not the install's: a weapon of one's own (`custom`),
//! whose model (`fbx`) lies in the workspace's `local` folder.

pub mod arms;
pub mod audio_preset;
pub mod body;
pub mod camera_effects;
pub mod clouds;
pub mod compress;
pub mod curve;
pub mod custom;
pub mod database;
pub mod dress;
pub mod edit_build;
pub mod engine;
pub mod eye_burn;
pub mod fbx;
pub mod grenade;
pub mod hard_settings;
pub mod heat;
pub mod icon;
pub mod install;
pub mod level_light;
pub mod muzzle;
pub mod inventory;
pub mod outfit;
pub mod particles;
pub mod player;
pub mod prism;
pub mod ragdoll;
pub mod range_targets;
pub mod rootmotion;
pub mod scene_props;
pub mod screen;
pub mod sight;
pub mod sketch;
pub mod sound;
pub mod tactical;
pub mod thing_sound;
pub mod tmp_text;
pub mod tod;
pub mod states;
pub mod suits;
pub mod unity;
pub mod visor;
pub mod weapon;
pub mod xray;
pub mod xray_archive;

pub use database::Database;
pub use install::{Install, Manifest, PlayerSettings};
