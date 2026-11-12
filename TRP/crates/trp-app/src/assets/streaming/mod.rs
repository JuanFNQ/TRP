//! The models' pictures by their levels of detail, as the game's Unity keeps them.
//! After `docs/spec/streaming.md`: with the game's texture quality and, where asked for, its mip
//! streaming.
//!
//! - **Mip streaming off** (`GraphicsSettingsController.ChangeTexturesQuality`):
//!   every picture loses its largest `2 - quality` levels for good (Unity's
//!   `globalTextureMipmapLimit`).
//! - **Mip streaming on** (`RefreshStreamingMemoryBudget`): a picture is kept
//!   on the graphics card from the level what draws with it needs, as near
//!   and as large on the screen as it is (the texel density of its mesh,
//!   its distance from each camera, the camera's field of view and height
//!   in pixels), with the game's bias of `2 - quality` levels; never more
//!   than `4 - quality` levels down; within the game's budget of 1,024 MB,
//!   the farthest given up first; and what nothing draws with goes down to
//!   the most it may (`streamingTextureDiscardUnusedMips`). A picture's
//!   levels are read again from its bundle when they are wanted, as the
//!   game reads them (`ReadAgain`), away from the frame.
//!
//! A picture on the card is never changed in place: a picture is cut to its
//! first level before it is added (`add`), and a change of level is a new
//! picture that the materials drawing with it are pointed at, the one before
//! let go. Only the models' pictures are taken (the engine's own, its
//! lighting's tables among them, are left alone: changing every picture as
//! it came drew the whole range black). The inventory's pictures are drawn
//! with every picture whole, as the game draws them (`ItemIconCreator`:
//! `requestedMipmapLevel` 0): their models' pictures do not come here. The
//! drawing alone: nothing of the simulation changes.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureDimension, TextureFormat};
use bevy::tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task};

mod textures;
mod mip_state;
mod stream_frame;

pub use textures::*;
pub use mip_state::*;
pub use stream_frame::*;
use textures::{ADDED, ADDED_WHOLE, DECIDE_EVERY, LOWER_AFTER, ON_THEIR_WAY, Streamed, UPLOAD_A_FRAME, Whole};
#[cfg(test)]
use textures::{cut, levels_of};
use mip_state::{ON_CARD};

#[cfg(test)]
mod tests;
