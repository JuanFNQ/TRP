//! The game's sounds as the install keeps them.
//! After `docs/spec/sound.md`: an audio clip (`AudioClip`) is a sound bank of FMOD's (FSB5) holding one
//! sample, kept beside its file (`m_Resource`), its samples 16-bit PCM or
//! IMA ADPCM (`m_CompressionFormat` 0 or 2; Vorbis, 1, is not read). Read
//! here: the interface's sounds (`Resources/Audio/UISoundsWrapper`, by the
//! game's `EUISoundType`) and the things' sounds (the bundle
//! `assets/content/audio/itemsounds/itemsounds.bundle`, `ItemSounds`: a
//! clip a kind of thing's sound group (`ItemSound`) and a way of handling
//! it, named `<group>_<way>`).

use std::collections::HashMap;

use anyhow::{bail, ensure, Context, Result};

use crate::setup::install::Install;
use crate::unity::Value;

mod clips;
mod ui_clips;
mod hands;
mod mixer;
mod movement;
mod effect_clips;
mod voices;
mod drops;

pub use clips::*;
pub use ui_clips::*;
pub use hands::*;
pub use mixer::*;
pub use movement::*;
pub use effect_clips::*;
pub use voices::*;
pub use drops::*;
use clips::{AUDIO_CLIP};
use movement::{BankReader};

#[cfg(test)]
mod tests;
