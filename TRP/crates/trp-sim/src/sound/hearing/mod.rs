//! A helmet's cut of what is heard.
//! After the sound spec, "Hearing under headgear" (`Player.UpdatePhonesReally`, `BetterAudio.ApplyHeadphonesTemplate`,
//! `ActiveHeadphonesController`, `ActiveHeadphonesDefaultConfigurator`,
//! `AudioMixerFader`): what is worn on the head chooses one of the game's
//! three templates of its own (`HeadphonesLocalTemplatesContainer`), and
//! applying one fades the mixer's values to it over the install's time.
//! Each group of the world's sounds is then heard at its dry level, which a
//! helmet takes 50 dB down, plus what it sends, before its own fader,
//! through the headphones' bus, which a helmet opens: a few decibels down in
//! all. No engine types: for a host as much as for the app.
//!
//! `PROVISIONAL`, the levels alone: the bus's filters (a high pass at 60 Hz,
//! a low pass at 21 or 18 kHz), its three equalisers (flat in all three
//! templates) and the two compressors with their distortions (neutral in
//! all three) are not applied, and the dry and the sent ways are summed as
//! amplitudes in phase. A headset's own template is not built: a worn
//! headset gives the default template, today's sound.

use trp_data::audio::sound::{FadeType, HeadphonesSettings};

use crate::items::inventory::{with_parts, Equipment, Item};

mod templates;
mod params;
mod headphones;

pub use templates::*;
pub use params::*;
pub use headphones::*;

#[cfg(test)]
mod tests;
