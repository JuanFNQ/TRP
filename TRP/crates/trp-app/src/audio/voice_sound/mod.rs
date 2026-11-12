//! The character's voice and the health's own sounds.
//! After the health spec, "The voice"; under the options' `HEALTH SYSTEM`: the voice chosen on the
//! SOUNDS page's PLAYER VOICE, else the profile's (`Customization.Voice`, `Bear_2_Eng` here), read
//! as the run begins and again when another is chosen (`change_voice`), its hurt, death, agony and
//! breathing banks, and a bone's crack. Each frame the speaker (`trp_sim::speaker`) is told what
//! the health did, what hit the character, whether its breath is heard and whether a face shield
//! muffles it; what it says plays unplaced on the speech source (`Player.PlaySpeechFromTime` in
//! first person: forced stereo, blend 0), a phrase cutting the one before it: the volume the
//! speech's preset gives a source (`Speech`'s `OverallVolume`, 0.7) times the clip's, and the
//! speech's group (`ClientPlayerSpeech`, or `ClientPlayerSelfSpeechReverb` when muffled) with the
//! groups above it, which are under `Main`. A fracture's crack (`Sounds.FractureSound`) plays where
//! the feet are (`BetterAudio.PlayAtPoint`, the `Character` group's rolloff over 15 m).
//! `PROVISIONAL`: the speech group's two equalisers, the muffled group's reverb and the reverb's
//! copy of the source are not built.
//!
//! The phrases asked for by the gestures' menu, a key bound to one or the
//! quick prompt come as `PhraseAsked` (gestures spec, "A phrase said"):
//! every bank's tags and lengths are read as the run begins, the sounds of
//! the five above at once and of any other the first time it is said, the
//! phrase then playing as they come. Measured (`probe voicecost`, and the
//! frame's cost's runs of 0.202.0): all 94 banks' sounds would hold 85 MB;
//! the voice's bundles kept open for later reads held 188 MB; so the voice
//! is opened again for each bank first said (about 0.2 s on the reading
//! thread, the phrase that late its first time) and let go of after. A
//! shot of the character's own makes it aware for 15 s.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_data::audio::audio_preset::AudioGroupPreset;
use trp_sim::condition::health::{Event, Kind};
use trp_sim::sound::speaker::{Bank, Said, Situation, Speaker, Status};

use crate::player::controller::Controller;
use crate::audio::ui_sound::{no_cut, Cut, Group, Levels, Mixer, Samples, Voice};
use crate::{platform::convert, MainCamera};

mod choosing;
mod speaking;

pub use choosing::*;
pub use speaking::*;

/// The audio source groups' numbers (`AudioSourceGroupType`).
pub(crate) const CHARACTER: i32 = 3;
pub(crate) const SPEECH: i32 = 6;
/// How far a bone's crack is heard (`PlayAtPoint`'s rolloff, `Player.OnHealthEffectAdded`).
pub(crate) const CRACK_ROLLOFF: f32 = 15.0;

/// What the reading thread brings: the voice's banks with their sounds, the
/// crack, and the two groups' presets.
struct Read {
    voice: trp_data::audio::sound::VoiceBanks,
    crack: Option<trp_data::audio::sound::Clip>,
    speech: Option<AudioGroupPreset>,
    character: Option<AudioGroupPreset>,
}

/// A phrase asked for by the player (`Player.Say`, demanded, at once), and
/// whether aggressively (a middle click, a key pressed twice).
#[derive(Message, Clone, Copy, Debug)]
pub struct PhraseAsked {
    pub trigger: i32,
    pub aggressive: bool,
}

/// A phrase asked for that the voice began to say (`OnPhraseTold`), and its
/// clip's length.
#[derive(Message, Clone, Copy, Debug)]
pub struct PhraseTold {
    pub trigger: i32,
    pub length: f32,
}

/// The phrases the voice has a bank for (`Speaker.PhrasesBanks`): what the
/// gestures' menu offers. Empty until the voice is read, and without the
/// `HEALTH SYSTEM`.
#[derive(Resource, Default)]
pub struct Voiced(pub std::collections::HashSet<i32>);

/// The voice once read, its speaker, what plays, and what the speaker was
/// last told of the character.
#[derive(Resource)]
pub struct VoiceSounds {
    speaker: Speaker,
    /// Each bank's clips: the sound and its volume.
    clips: Vec<Vec<(Option<Samples>, f32)>>,
    crack: Option<Samples>,
    speech: Option<AudioGroupPreset>,
    character: Option<AudioGroupPreset>,
    coming: Option<Mutex<Receiver<Read>>>,
    /// What plays on the speech source, to be cut.
    playing: Option<Cut>,
    /// The parts' points as last seen: a change of a hundredth asks the
    /// breathing again (`OnHealthChanged`).
    parts: [f32; 7],
    audible: bool,
    shield: Option<(bool, bool)>,
    /// `StaminaExhaustionStartsBreathSound`: the legs' exhaustion heard.
    exhaustion_heard: bool,
    logged: u32,
    /// A bank's sounds asked of the reading thread, by its phrase, and what
    /// it brings back; which banks have been asked for.
    ask: Option<Sender<i32>>,
    decoded: Option<Mutex<Receiver<trp_data::audio::sound::PhraseBank>>>,
    asked: Vec<bool>,
    /// The phrase said whose sounds are still being read, and until when it
    /// would play: played as they come, if it is not over by then.
    waiting: Option<(trp_sim::sound::speaker::Phrase, f32)>,
    /// The character's shots as last counted.
    shots: u32,
    /// Where another voice is read from, when one is chosen.
    install: trp_data::Install,
}
