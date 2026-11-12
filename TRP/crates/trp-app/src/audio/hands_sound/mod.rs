//! The sounds of what is in the hands.
//! After `docs/spec/sound.md`, "The hands' sounds": a weapon's, an item's or the empty hands' own
//! sound player (`BaseSoundPlayer`, read with the rig: `trp_data::sound::hands_sounds`) plays the
//! clip its animator's `Sound` marks name (`OnSound`, `SoundEventHandler`): the element of its
//! `AdditionalSounds` named by the mark, or by the mark with `Snd` before it, one of its clips at
//! random, at its volume.
//!
//! - **For the character's own eyes** the clip plays in stereo, whole
//!   (`PlayClip`: `forceStereo` in first person), a mono clip at equal
//!   power in the middle;
//! - **seen from outside** it plays from where the hands are, heard to the
//!   element's rolloff and not beyond (`PlayClip` returns past it): its
//!   level falls as one over the distance past 1 m and it is panned by
//!   how far to the side of the camera it lies (`PROVISIONAL`: the source's
//!   own curve and the engine's pan law are not read; the place is the
//!   character's, 1.4 m up, not the weapon's bone);
//! - at the overall volume, through the mixer's `Guns/Instrumental` group,
//!   read at 0.52.0: 0 dB, with the groups above it (`Group::Guns`).
//!
//! With the options' `HANDS SOUNDS` off the players are not read and
//! nothing plays.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_sim::weapons::recoil::Rng;

use crate::player::controller::Controller;
use crate::audio::ui_sound::{Levels, Mixer, Pitch, Samples, Voice};
use crate::weapons::weapon::HandsRig;
use crate::MainCamera;

mod reading;
mod playing;

pub use reading::*;
use reading::{BIPOD_SURFACE_RAY, Draws, FULL_WITHIN, HANDS_HIGH, aiming_volume, next_clip, take_aiming};
use playing::{play};

/// A rig's sounds ready to play: each element of its `AdditionalSounds`
/// by its event's name, with its rolloff, volume and clips; and its shots
/// (`shot_sound`), where it has banks of them.
pub struct RigSounds {
    elements: Vec<Element>,
    pub shots: Option<Arc<crate::audio::shot_sound::Shots>>,
    /// The weapon's kind of sound and its ergonomics, which its aiming's
    /// sound goes by; none for a rig that is no weapon.
    aiming: Option<(&'static str, f32)>,
    /// Its bipod's own sounds and the bipod's node in the rig, where the
    /// build has a bipod with a sound player (`BipodAudioController`).
    bipod: Option<BipodSet>,
    /// Its underbarrel launcher's own sound player's, where the build has one
    /// (`UnderbarrelContainer.WeaponSoundPlayer`): the marks of the launcher's
    /// animator and the launcher's shots play on it.
    pub launcher: Option<Arc<RigSounds>>,
}

/// A bipod's sounds ready to play (`trp_data::sound::BipodSounds`): its
/// opening, its folding, and on the ground by the surface's number
/// (`ESurfaceSound`).
struct BipodSet {
    node: usize,
    open: ClipSet,
    close: ClipSet,
    surfaces: Vec<(u8, ClipSet)>,
}

struct ClipSet {
    clips: Vec<(String, Samples)>,
    volume: f32,
    max_distance: f32,
}

impl ClipSet {
    fn of(set: trp_data::audio::sound::ClipSet) -> Self {
        Self { clips: set.clips.into_iter().map(|clip| (clip.name.clone(), Samples::of_owned(clip))).collect(), volume: set.volume, max_distance: set.max_distance }
    }
}

/// Which of a rig's sounds are read: its marks' (`HANDS SOUNDS`), its
/// shots' (`SHOT SOUNDS`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Wanted {
    pub hands: bool,
    pub shots: bool,
}

impl Wanted {
    pub fn of(features: &crate::screens::options_view::Features) -> Self {
        Self { hands: features.hands_sounds, shots: features.shot_sounds }
    }
}

struct Element {
    event: String,
    rolloff: f32,
    volume: f32,
    clips: Vec<Samples>,
    names: Vec<String>,
}

impl RigSounds {
    pub fn of(sounds: trp_data::audio::sound::HandsSounds, wanted: Wanted, silenced: bool) -> Self {
        let elements = match wanted.hands {
            true => sounds
                .additional
                .into_iter()
                .map(|element| {
                    let names = element.clips.iter().map(|clip| clip.name.clone()).collect();
                    Element { event: element.event, rolloff: element.rolloff as f32, volume: element.volume, clips: element.clips.into_iter().map(Samples::of_owned).collect(), names }
                })
                .collect(),
            false => Vec::new(),
        };
        let shots = sounds.shots.filter(|_| wanted.shots).map(|shots| Arc::new(crate::audio::shot_sound::Shots::of(shots, silenced)));
        Self { elements, shots, aiming: None, bipod: None, launcher: None }
    }

    /// The element a mark's string names (`SoundEventHandler`).
    fn for_mark(&self, name: &str) -> Option<&Element> {
        self.elements.iter().find(|element| element.event == name || element.event.strip_prefix("Snd") == Some(name))
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }
}

#[cfg(test)]
mod tests;

pub struct HandsSoundPlugin;

impl Plugin for HandsSoundPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Draws(Rng::new(0x4841_4e44))).add_systems(Update, (take_aiming, play.after(crate::player::controller::step)));
    }
}
