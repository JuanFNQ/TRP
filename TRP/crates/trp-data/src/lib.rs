//! Readers for your own Single Player Tushonka install: the game's files under it and SPT's database.
//! SPT is Single Player Tushonka's short name.
//!
//! Nothing here patches or writes to the install. Everything is read in place.
//!
//! One thing here is not the install's: a weapon of one's own (`custom`),
//! whose model (`fbx`) lies in the workspace's `local` folder.

// First, so that its `println!` and `eprintln!` are the ones every module below prints with.
#[macro_use]
pub mod console;
pub mod character;
pub mod audio;
pub mod drawing;
pub mod level;
pub mod formats;
pub mod weapons;
pub mod setup;
pub mod interface;
pub mod unity;

pub use setup::database::Database;
pub use setup::install::{Install, Manifest, PlayerSettings};
