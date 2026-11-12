//! The interface's and the things' sounds, as the game's `GUISounds` plays them.
//! After `docs/spec/sound.md`: an interface sound (`PlayUISound`, by
//! `EUISoundType`) whole on the mixer's `UI` group, a thing's sound
//! (`PlayItemSound`: its kind's group, `ItemSound`, and the way it is
//! handled) at 0.15 in a raid on its `InGame/Inventory` group
//! (`GetCommonSoundsVolume`), each by the game's own settings of the
//! volumes (`Sound.ini`: `OverallVolume`, `InterfaceVolume`, indices into
//! `SoundSettingsGroup._realSoundValues`, in decibels), or as the options'
//! SOUNDS page keeps them. The clips are read from the install away from
//! the frame as the run begins (`trp_data::sound`), and a sound asked for
//! before they have come is not played.
//!
//! **One source plays them all** (`Mixed`): a sound of its own for the whole
//! run, at the clips' rate, two channels, into which each sound asked for
//! is mixed from the next block of 64 frames. A sound of its own for each
//! (Bevy's sink for each `AudioPlayer`) had the first 6 to 12 ms of every
//! sound read in the empty sink's format (rodio 0.20.1's queue: its first
//! stretch taken as one channel at 48 kHz): a stereo clip's start garbled,
//! a mono clip's start played fast (measured in a recording
//! and made again away from the app: `sound.md`, "Played").

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::audio::{AddAudioSource, AudioPlayer, Decodable, Source};
use bevy::prelude::*;

mod kinds;
mod samples;
mod mixing;
mod playing;

pub use kinds::*;
pub use samples::*;
// Named outright: the engine's prelude has a `Pitch` of its own.
pub use samples::Pitch;
pub use mixing::*;
pub use playing::*;
use samples::{GLIDE, RATE, Read};
#[cfg(test)]
use mixing::{BLOCK};
use playing::{start_mixing, take_clips};

pub struct UiSoundPlugin;

impl Plugin for UiSoundPlugin {
    fn build(&self, app: &mut App) {
        // Played after the frame's Update, where every sound is asked for,
        // so that each plays on the frame it was asked for, as the game's.
        app.add_audio_source::<Mixed>()
            .add_message::<Sound>()
            .init_resource::<Sounds>()
            .add_systems(Startup, start_mixing)
            .add_systems(PostUpdate, (take_clips, play).chain());
    }
}

/// The groups of the game's mixer under `World` whose levels the app moves
/// (`Mixed`'s, set by `hearing`), each sound of the world's in the one
/// the game plays it in: the shots, the blasts and the hands' sounds in
/// `Guns`; the character's own steps, kit, switches and voice in
/// `ClientPlayer`; a dummy's steps in `ObservedPlayer`; the hits, the spent
/// cases, the things handled and taken up, a search, a bone's crack and the
/// poppers in `CommonSounds`. The inventory's and the interface's sounds
/// are in none.
pub use trp_sim::sound::hearing::Group;

#[cfg(test)]
mod tests;
