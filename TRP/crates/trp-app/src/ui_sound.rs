//! The interface's and the things' sounds (`docs/spec/sound.md`; the user,
//! 2026-10-16: "Can we also start adding interface sounds"), as the game's
//! `GUISounds` plays them: an interface sound (`PlayUISound`, by
//! `EUISoundType`) whole on the mixer's `UI` group, a thing's sound
//! (`PlayItemSound`: its kind's group, `ItemSound`, and the way it is
//! handled) at 0.15 in a raid on its `InGame/Inventory` group
//! (`GetCommonSoundsVolume`), each by the game's own settings of the
//! volumes (`Sound.ini`: `OverallVolume`, `InterfaceVolume`, indices into
//! `SoundSettingsGroup._realSoundValues`, in decibels), or as the options'
//! SOUND page keeps them. The clips are read from the install away from
//! the frame as the run begins (`trp_data::sound`), and a sound asked for
//! before they have come is not played.
//!
//! **One source plays them all** (`Mixed`): a sound of its own for the whole
//! run, at the clips' rate, two channels, into which each sound asked for
//! is mixed from the next block of 64 frames. A sound of its own for each
//! (Bevy's sink for each `AudioPlayer`) had the first 6 to 12 ms of every
//! sound read in the empty sink's format (rodio 0.20.1's queue: its first
//! stretch taken as one channel at 48 kHz): a stereo clip's start garbled,
//! a mono clip's start played fast (measured in the user's recording of
//! 2026-10-16 and made again away from the app: `sound.md`, "Played").

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::audio::{AddAudioSource, AudioPlayer, Decodable, Source};
use bevy::prelude::*;

pub struct UiSoundPlugin;

impl Plugin for UiSoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Mixed>()
            .add_message::<Sound>()
            .init_resource::<Sounds>()
            .add_systems(Startup, start_mixing)
            .add_systems(Update, (take_clips, play).chain());
    }
}

/// The interface's sounds the app plays, by the game's `EUISoundType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ui {
    BackpackClose = 0,
    BackpackOpen = 1,
    ButtonClick = 3,
    ButtonOver = 4,
    MenuEscape = 13,
    InspectorClose = 14,
    InspectorOpen = 15,
    InstallMag = 16,
    InstallModFunc = 17,
    InstallModGear = 18,
    InstallModVital = 19,
    OpenContainer = 23,
}

impl Ui {
    pub const ALL: [Ui; 12] = [
        Ui::BackpackClose,
        Ui::BackpackOpen,
        Ui::ButtonClick,
        Ui::ButtonOver,
        Ui::MenuEscape,
        Ui::InspectorClose,
        Ui::InspectorOpen,
        Ui::InstallMag,
        Ui::InstallModFunc,
        Ui::InstallModGear,
        Ui::InstallModVital,
        Ui::OpenContainer,
    ];
}

/// A way a thing is handled, which names its clip after its group
/// (`EInventorySoundType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Drop,
    Pickup,
    Use,
    /// A container opened in a window: its group's own clip if it has
    /// one, at the interface's volume (`GUISounds.PlaySound`), else the
    /// interface's `MenuOpenContainer` (`ItemUiContext.OpenItem`).
    Open,
}

impl Way {
    fn name(self) -> &'static str {
        match self {
            Way::Drop => "drop",
            Way::Pickup => "pickup",
            Way::Use => "use",
            Way::Open => "open",
        }
    }
}

/// A sound asked for: the interface's, or a thing's by its kind's group.
#[derive(Message, Clone, Debug)]
pub enum Sound {
    Ui(Ui),
    Item(String, Way),
}

/// The volumes as the game's settings have them: indices 0 to 10 into its
/// table of decibels.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub overall: u8,
    pub interface: u8,
}

impl Levels {
    /// The game's table (`SoundSettingsGroup._realSoundValues`).
    pub const DECIBELS: [f32; 11] = [-80.0, -40.0, -30.0, -25.0, -20.0, -17.0, -11.0, -8.0, -5.0, -2.0, 0.0];

    /// As the install has them (`Sound.ini`; the game's default, 10, where
    /// it says nothing).
    pub fn read(install: &trp_data::Install) -> Self {
        let level = |name: &str| install.sound_setting(name).map_or(10, |index| index.min(10.0).max(0.0) as u8);
        Self { overall: level("OverallVolume"), interface: level("InterfaceVolume") }
    }

    fn gain(index: u8) -> f32 {
        10f32.powf(Self::DECIBELS[index.min(10) as usize] / 20.0)
    }

    /// The overall volume as a share of the clip's level (the master
    /// group's, which every sound passes through).
    pub fn overall_gain(&self) -> f32 {
        Self::gain(self.overall)
    }
}

/// The rate the source plays at: the clips' own (all but a dozen of the
/// install's are kept at 44.1 kHz); one at another is read at its own rate
/// as it is mixed.
const RATE: u32 = 44_100;

/// A clip ready to mix: its samples as read (16 bits, its channels
/// interleaved), how many channels and at what rate. Kept so, not as
/// floats: the sounds of moving alone are 100 MB of samples
/// (`step_sound.rs`).
#[derive(Clone)]
pub struct Samples {
    data: Arc<Vec<i16>>,
    channels: usize,
    rate: u32,
}

impl Samples {
    #[cfg(test)]
    pub fn of(clip: &trp_data::sound::Clip) -> Self {
        Self { data: Arc::new(clip.samples.clone()), channels: clip.channels.max(1) as usize, rate: clip.frequency.max(1) }
    }

    pub fn of_owned(clip: trp_data::sound::Clip) -> Self {
        let channels = clip.channels.max(1) as usize;
        Self { data: Arc::new(clip.samples), channels, rate: clip.frequency.max(1) }
    }

    pub fn mono(&self) -> bool {
        self.channels == 1
    }

    fn frames(&self) -> usize {
        self.data.len() / self.channels
    }

    /// How long it plays at its own rate, in seconds (`AudioClip.length`).
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate.max(1) as f64
    }
}

/// The equal power of a mono source in the middle: each channel 1/√2 (the
/// game's mono sounds came out 3.0 to 3.4 dB under their clips' level in
/// the user's recording, its stereo ones at their own).
pub const MIDDLE: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// A source's pitch that the sounds playing from it follow (the game's
/// steps and kit each play from a source whose pitch is set for each sound,
/// `step_sound.rs`), as an `f32`'s bits.
pub type Pitch = Arc<std::sync::atomic::AtomicU32>;

/// A sound being played: its clip, how far it has got (in the clip's
/// frames), what each of the two channels takes of it (of a mono clip's
/// one channel; of a stereo clip's own, or of both together if `merged`),
/// and the pitch it follows, if any. A sound scheduled (`Voice::at_frame`)
/// begins at a frame of the mix, may loop, and may be cut at a frame of the
/// mix set after it was added (the game's queue of a weapon's shots,
/// `shot_sound.rs`).
pub struct Voice {
    clip: Samples,
    at: f64,
    gains: [f32; 2],
    merged: bool,
    pitch: Option<Pitch>,
    /// The frame of the mix it begins at; from the next block if 0.
    starts: u64,
    started: bool,
    looping: bool,
    /// The frame of the mix it ends at (`u64::MAX`: its clip's end).
    ends: Option<Cut>,
    done: bool,
}

/// A frame of the mix at which a scheduled sound ends, set from the frame's
/// side as the game's queue sets a source's scheduled end
/// (`SetScheduledEndTime`).
pub type Cut = Arc<std::sync::atomic::AtomicU64>;

/// A cut not yet set.
pub fn no_cut() -> Cut {
    Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX))
}

impl Voice {
    /// A clip whole: a mono clip at `MIDDLE` on each channel, a stereo one
    /// as it is, both times `gain`.
    pub fn whole(clip: &Samples, gain: f32) -> Self {
        let each = if clip.mono() { gain * MIDDLE } else { gain };
        Self { clip: clip.clone(), at: 0.0, gains: [each; 2], merged: false, pitch: None, starts: 0, started: false, looping: false, ends: None, done: false }
    }

    /// A clip from a point: what each channel takes of it, a stereo clip's
    /// two channels taken together (a point has one place), and the pitch
    /// it follows.
    pub fn placed(clip: &Samples, gains: [f32; 2], pitch: Pitch) -> Self {
        Self { clip: clip.clone(), at: 0.0, gains, merged: !clip.mono(), pitch: Some(pitch), starts: 0, started: false, looping: false, ends: None, done: false }
    }

    /// The pitch it follows (a whole clip from a source whose pitch moves:
    /// the shots' queue).
    pub fn following(mut self, pitch: Pitch) -> Self {
        self.pitch = Some(pitch);
        self
    }

    /// The sound scheduled: it begins at that frame of the mix (its place
    /// in the clip counted from there, should the mix have passed it), in a
    /// loop if `looping`, and ends at its cut's frame if that comes first.
    pub fn at_frame(mut self, starts: u64, looping: bool, ends: Cut) -> Self {
        (self.starts, self.looping, self.ends) = (starts, looping, Some(ends));
        self
    }
}

/// The one source the sounds are mixed into (a Bevy audio source of its
/// own): the sounds playing, which `play` adds to.
#[derive(Asset, TypePath, Clone, Default)]
pub struct Mixed {
    voices: Arc<Mutex<Vec<Voice>>>,
    /// Everything mixed since the tape was started, if it was
    /// (`--sound-out`): two channels at `RATE`.
    tape: Arc<Mutex<Option<Vec<f32>>>>,
    /// How many frames have been mixed: the mix's own clock (the game's
    /// `AudioSettings.dspTime`).
    clock: Arc<std::sync::atomic::AtomicU64>,
    /// What the game's own mixer does that the app's does, once read
    /// (`trp_data::sound::master_mixer`).
    game: Arc<std::sync::OnceLock<GameMix>>,
}

/// The game's mixer as the app's follows it: the master group's
/// compressor (threshold in dB, attack and release in seconds, make-up in
/// dB), and as gains the volumes of the gunshots' group and of the
/// player's own movement's (`ClientPlayer/ClientPlayerMovement`), each
/// with the groups above it.
#[derive(Debug, Clone, Copy)]
pub struct GameMix {
    pub compressor: Option<[f32; 4]>,
    pub gunshots: f32,
    pub movement: f32,
}

/// The frames the mix plays in a second.
pub const MIX_RATE: u32 = RATE;

impl Mixed {
    /// The gunshots' group's gain, with the groups above it (1 until the
    /// game's mixer is read).
    pub fn gunshots_gain(&self) -> f32 {
        self.game.get().map_or(1.0, |game| game.gunshots)
    }

    /// The player's own movement's group's gain, with the groups above it
    /// (1 until the game's mixer is read).
    pub fn movement_gain(&self) -> f32 {
        self.game.get().map_or(1.0, |game| game.movement)
    }

    /// The frames mixed so far: a sound scheduled at this frame or later
    /// begins where it is meant to.
    pub fn clock(&self) -> u64 {
        self.clock.load(std::sync::atomic::Ordering::Acquire)
    }

    /// A sound to play from the next block on.
    pub fn add(&self, voice: Voice) {
        if let Ok(mut voices) = self.voices.lock() {
            voices.push(voice);
        }
    }

    /// Keeps every block mixed from now on.
    pub fn start_tape(&self) {
        if let Ok(mut tape) = self.tape.lock() {
            *tape = Some(Vec::new());
        }
    }

    /// What the tape holds, as a WAVE file's bytes (16 bits, two
    /// channels, at `RATE`), if it was started.
    pub fn tape_wav(&self) -> Option<Vec<u8>> {
        let tape = self.tape.lock().ok()?;
        let samples: Vec<i16> = tape.as_ref()?.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
        Some(trp_data::sound::Clip { name: String::new(), channels: 2, frequency: RATE, samples }.wav())
    }
}

impl Decodable for Mixed {
    type DecoderItem = f32;
    type Decoder = Mixing;

    fn decoder(&self) -> Mixing {
        Mixing { voices: self.voices.clone(), tape: self.tape.clone(), clock: self.clock.clone(), game: self.game.clone(), envelope: 0.0, block: Vec::new(), at: 0 }
    }
}

/// The source's samples: each block of 64 frames mixed from the sounds
/// playing as it comes, those done let go.
pub struct Mixing {
    voices: Arc<Mutex<Vec<Voice>>>,
    tape: Arc<Mutex<Option<Vec<f32>>>>,
    clock: Arc<std::sync::atomic::AtomicU64>,
    game: Arc<std::sync::OnceLock<GameMix>>,
    /// The master compressor's level followed (`compress`).
    envelope: f32,
    block: Vec<f32>,
    at: usize,
}

const BLOCK: usize = 64;

impl Mixing {
    /// Each sound's next stretch: read at its clip's rate times the pitch
    /// it follows (as it stands at the block's start), by straight lines
    /// between the clip's frames where that falls between them
    /// (`PROVISIONAL`: the engine's own resampling is not read); exactly
    /// its frames at the source's rate and no pitch.
    fn mix(&mut self) {
        self.block.clear();
        self.block.resize(BLOCK * 2, 0.0);
        self.at = 0;
        let shared = self.voices.clone();
        let Ok(mut voices) = shared.lock() else { return };
        let first = self.clock.load(std::sync::atomic::Ordering::Acquire);
        self.mix_voices(&mut voices, first);
        drop(voices);
        self.clock.store(first + BLOCK as u64, std::sync::atomic::Ordering::Release);
        if let Some(compressor) = self.game.get().and_then(|game| game.compressor) {
            self.compress(compressor);
        }
        if let Ok(mut tape) = self.tape.lock() {
            if let Some(tape) = tape.as_mut() {
                tape.extend_from_slice(&self.block);
            }
        }
    }

    /// The game's master compressor (`MasterMixer`'s `Master` group: its
    /// threshold, attack, release and make-up as read): the louder of the
    /// two channels followed, rising at the attack's pace and falling at
    /// the release's, and the mix brought down to the threshold where that
    /// is over it (`PROVISIONAL`: FMOD's own compressor is not published;
    /// with no ratio among its parameters it is taken as a limiter).
    fn compress(&mut self, [threshold_db, attack, release, makeup_db]: [f32; 4]) {
        let threshold = 10f32.powf(threshold_db / 20.0);
        let makeup = 10f32.powf(makeup_db / 20.0);
        let rise = (-1.0 / (attack.max(1e-4) * RATE as f32)).exp();
        let fall = (-1.0 / (release.max(1e-4) * RATE as f32)).exp();
        for frame in self.block.as_chunks_mut::<2>().0 {
            let peak = frame[0].abs().max(frame[1].abs());
            let pace = if peak > self.envelope { rise } else { fall };
            self.envelope = pace * self.envelope + (1.0 - pace) * peak;
            let gain = if self.envelope > threshold { threshold / self.envelope } else { 1.0 } * makeup;
            frame[0] *= gain;
            frame[1] *= gain;
        }
    }

    fn mix_voices(&mut self, voices: &mut Vec<Voice>, first: u64) {
        for voice in voices.iter_mut() {
            let pitch = voice.pitch.as_ref().map_or(1.0, |pitch| f32::from_bits(pitch.load(std::sync::atomic::Ordering::Relaxed)) as f64);
            let step = voice.clip.rate as f64 / RATE as f64 * pitch.max(0.01);
            let (data, channels, frames) = (&voice.clip.data, voice.clip.channels, voice.clip.frames());
            let read = |frame: usize, channel: usize| data.get(frame * channels + channel).map_or(0.0, |&s| s as f32 / 32768.0);
            let ends = voice.ends.as_ref().map_or(u64::MAX, |cut| cut.load(std::sync::atomic::Ordering::Acquire));
            for i in 0..BLOCK {
                let frame = first + i as u64;
                if frame >= ends {
                    voice.done = true;
                    break;
                }
                if frame < voice.starts {
                    continue;
                }
                // A scheduled sound the mix has passed the start of
                // begins where it would have got to.
                if !voice.started {
                    voice.started = true;
                    if voice.ends.is_some() {
                        voice.at = frame.saturating_sub(voice.starts) as f64 * step;
                    }
                }
                if voice.looping && frames > 0 {
                    voice.at %= frames as f64;
                }
                let k = voice.at as usize;
                if k >= frames {
                    voice.done = true;
                    break;
                }
                let f = (voice.at - k as f64) as f32;
                let at = |channel: usize| {
                    let a = read(k, channel);
                    if f == 0.0 { a } else { a + (read(k + 1, channel) - a) * f }
                };
                if channels == 1 || voice.merged {
                    let one = if channels == 1 { at(0) } else { (at(0) + at(1)) * 0.5 };
                    self.block[2 * i] += one * voice.gains[0];
                    self.block[2 * i + 1] += one * voice.gains[1];
                } else {
                    self.block[2 * i] += at(0) * voice.gains[0];
                    self.block[2 * i + 1] += at(1) * voice.gains[1];
                }
                voice.at += step;
            }
        }
        voices.retain(|voice| !voice.done);
    }
}

impl Iterator for Mixing {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.at >= self.block.len() {
            self.mix();
        }
        let sample = self.block[self.at];
        self.at += 1;
        Some(sample)
    }

    /// Endless: as one stretch of one format (the queue it plays from takes
    /// a source's least length as its stretch, and reads its format again
    /// only after it).
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

impl Source for Mixing {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        RATE
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// The clips once read, ready to mix; the source they are mixed into; and
/// what brings the clips.
#[derive(Resource, Default)]
struct Sounds {
    ui: HashMap<i32, Samples>,
    items: HashMap<String, Samples>,
    mixed: Mixed,
    coming: Option<Mutex<Receiver<(Option<i32>, String, Samples)>>>,
}

/// The one source the app's sounds are mixed into, for the other kinds of
/// sound (`step_sound.rs`).
#[derive(Resource, Clone)]
pub struct Mixer(pub Mixed);

/// Starts reading the clips from the install away from the frame.
pub fn start_reading(app: &mut App, install: trp_data::Install) {
    let (send, receive) = channel();
    let sounds = Sounds { coming: Some(Mutex::new(receive)), ..default() };
    let game = sounds.mixed.game.clone();
    std::thread::spawn(move || {
        // The game's own mixer first: what the mix does with all of it.
        match trp_data::sound::master_mixer(&install) {
            Ok(mixer) => {
                let gunshots = mixer.group("Gunshots").map_or(0.0, |group| mixer.path_db(group));
                let movement = mixer.group("ClientPlayerMovement").map_or(0.0, |group| mixer.path_db(group));
                let read = GameMix { compressor: mixer.master_compressor(), gunshots: 10f32.powf(gunshots / 20.0), movement: 10f32.powf(movement / 20.0) };
                info!(
                    "the game's mixer ({} groups, from its {} snapshot): the master's compressor {:?}, the gunshots' group {gunshots:+.1} dB, the player's movement's {movement:+.1} dB",
                    mixer.groups.len(),
                    mixer.start_snapshot,
                    read.compressor
                );
                let _ = game.set(read);
            }
            Err(e) => warn!("the game's mixer could not be read; the mix is not compressed: {e:#}"),
        }
        let kinds: Vec<i32> = Ui::ALL.iter().map(|ui| *ui as i32).collect();
        match trp_data::sound::ui_sounds(&install, &kinds) {
            Ok(clips) => {
                for (kind, clip) in clips {
                    let _ = send.send((Some(kind), clip.name.clone(), Samples::of_owned(clip)));
                }
            }
            Err(e) => eprintln!("the interface's sounds could not be read; it is silent: {e:#}"),
        }
        match trp_data::sound::item_sounds(&install) {
            Ok(clips) => {
                for (name, clip) in clips {
                    let _ = send.send((None, name, Samples::of_owned(clip)));
                }
            }
            Err(e) => eprintln!("the things' sounds could not be read; they are silent: {e:#}"),
        }
    });
    app.insert_resource(Mixer(sounds.mixed.clone()));
    app.insert_resource(sounds);
}

/// The one source, playing from the start for the whole run.
fn start_mixing(mut commands: Commands, sounds: Res<Sounds>, mut sources: ResMut<Assets<Mixed>>) {
    commands.spawn(AudioPlayer(sources.add(sounds.mixed.clone())));
}

fn take_clips(mut sounds: ResMut<Sounds>) {
    let Some(coming) = sounds.coming.take() else { return };
    let mut done = false;
    if let Ok(receive) = coming.lock() {
        loop {
            match receive.try_recv() {
                Ok((kind, name, frames)) => {
                    match kind {
                        Some(kind) => {
                            sounds.ui.insert(kind, frames);
                        }
                        None => {
                            sounds.items.insert(name, frames);
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }
    }
    if !done {
        sounds.coming = Some(coming);
    } else {
        info!("sounds: {} of the interface's and {} of the things' read", sounds.ui.len(), sounds.items.len());
    }
}

/// The game's volume of a thing's sound in a raid (`GetCommonSoundsVolume`).
const IN_RAID: f32 = 0.15;

/// Plays the sounds asked for, each once, at its volume: the overall
/// volume's and the interface's for the interface's; the overall's and the
/// raid's 0.15 for a thing's (its mixer group's own attenuation not read,
/// `PROVISIONAL`). A thing's group with no clip for the way is silent, as
/// the game's (`GetClip` finds none).
fn play(mut asked: MessageReader<Sound>, sounds: Res<Sounds>, levels: Option<Res<Levels>>) {
    let levels = levels.map_or(Levels { overall: 10, interface: 10 }, |levels| *levels);
    let interface = Levels::gain(levels.overall) * Levels::gain(levels.interface);
    for sound in asked.read() {
        let (frames, gain) = match sound {
            Sound::Ui(ui) => (sounds.ui.get(&(*ui as i32)), interface),
            Sound::Item(group, Way::Open) => (sounds.items.get(&format!("{group}_open")).or_else(|| sounds.ui.get(&(Ui::OpenContainer as i32))), interface),
            Sound::Item(group, way) => (sounds.items.get(&format!("{group}_{}", way.name())), Levels::gain(levels.overall) * IN_RAID),
        };
        if let Some(clip) = frames {
            sounds.mixed.add(Voice::whole(clip, gain));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stereo clip of two decaying tones, one on each channel, at `RATE`.
    fn clip() -> Vec<[f32; 2]> {
        (0..4410)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let tone = |hz: f32, fall: f32| (t * hz * std::f32::consts::TAU).sin() * (-t * fall).exp() * 0.5;
                [tone(3000.0, 30.0), tone(1700.0, 20.0)]
            })
            .collect()
    }

    /// The clip at the output's rate, by straight lines between frames.
    fn at_48k(frames: &[[f32; 2]]) -> Vec<[f32; 2]> {
        let count = frames.len() * 48_000 / RATE as usize;
        (0..count)
            .map(|i| {
                let t = i as f64 * RATE as f64 / 48_000.0;
                let (k, f) = (t as usize, (t - (t as usize) as f64) as f32);
                let a = frames[k.min(frames.len() - 1)];
                let b = frames[(k + 1).min(frames.len() - 1)];
                [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
            })
            .collect()
    }

    /// How alike two channels' runs are (1 the same shape).
    fn alike(a: impl Iterator<Item = f32> + Clone, b: impl Iterator<Item = f32> + Clone) -> f32 {
        let ab: f32 = a.clone().zip(b.clone()).map(|(x, y)| x * y).sum();
        let aa: f32 = a.map(|x| x * x).sum();
        let bb: f32 = b.map(|x| x * x).sum();
        ab / (aa * bb).sqrt().max(1e-12)
    }

    /// The source through what Bevy plays a source with (rodio 0.20.1: a
    /// sink made and the source appended to it at once, the sink's queue in
    /// an output's mixer at 48 kHz, two channels), a sound mixed in a tenth
    /// of a second later comes out whole from its first frame, each channel
    /// its own. A sound appended to a sink of its own came out with its
    /// first stretch read as one channel at 48 kHz (`sound.md`, "Played").
    #[test]
    fn a_sound_comes_out_whole_from_its_start() {
        let mixed = Mixed::default();
        let (controller, mut mixer) = rodio::dynamic_mixer::mixer::<f32>(2, 48_000);
        let (sink, queue) = rodio::Sink::new_idle();
        controller.add(queue);
        sink.append(mixed.decoder());
        let mut out: Vec<f32> = (0..9_600).map(|_| mixer.next().unwrap_or(0.0)).collect();
        assert!(out.iter().all(|s| s.abs() < 1e-6), "quiet before the sound");
        mixed.add(Voice::whole(&samples(&clip()), 1.0));
        out.extend((0..19_200).map(|_| mixer.next().unwrap_or(0.0)));
        let frames: Vec<[f32; 2]> = out.chunks_exact(2).map(|f| [f[0], f[1]]).collect();
        let start = frames.iter().position(|f| f[0].abs() > 1e-4 || f[1].abs() > 1e-4).expect("the sound comes out");
        let expected = at_48k(&clip());
        // Its first 10 ms (480 frames) and its next 40, each channel against
        // the clip's own, at the best shift of a frame or two.
        for (from, to) in [(0, 480), (480, 2400)] {
            for channel in 0..2 {
                let best = (0..4)
                    .map(|shift| {
                        let got = frames[start + from..start + to].iter().map(move |f| f[channel]);
                        let want = expected[(from + shift).min(expected.len())..(to + shift).min(expected.len())].iter().map(move |f| f[channel]);
                        alike(got, want)
                    })
                    .fold(-1.0f32, f32::max);
                assert!(best > 0.99, "channel {channel}, frames {from} to {to}: {best}");
            }
        }
        // The two channels are not one another.
        let left = frames[start..start + 480].iter().map(|f| f[0]);
        let right = frames[start..start + 480].iter().map(|f| f[1]);
        assert!(alike(left, right).abs() < 0.5);
    }

    /// A stereo clip of float frames, as the install keeps clips: 16 bits.
    fn samples(frames: &[[f32; 2]]) -> Samples {
        let data = frames.iter().flat_map(|f| f.map(|s| (s * 32767.0) as i16)).collect();
        Samples { data: Arc::new(data), channels: 2, rate: RATE }
    }

    /// The first block the source gives with these sounds in it.
    fn first_block(voices: Vec<Voice>) -> Vec<f32> {
        let mixed = Mixed::default();
        for voice in voices {
            mixed.add(voice);
        }
        let mut mixing = mixed.decoder();
        (0..BLOCK * 2).map(|_| mixing.next().unwrap()).collect()
    }

    /// A mono clip plays on both channels at 1/√2 of its level, a stereo one
    /// as it is.
    #[test]
    fn a_mono_clip_is_played_at_the_middle_s_equal_power() {
        let mono = Samples::of(&trp_data::sound::Clip { name: "m".into(), channels: 1, frequency: RATE, samples: vec![16384, -16384] });
        let out = first_block(vec![Voice::whole(&mono, 1.0)]);
        assert!((out[0] - 0.5 * MIDDLE).abs() < 1e-6 && out[0] == out[1]);
        assert!((out[2] + 0.5 * MIDDLE).abs() < 1e-6 && out[4] == 0.0);
        let stereo = Samples::of(&trp_data::sound::Clip { name: "s".into(), channels: 2, frequency: RATE, samples: vec![16384, -8192] });
        assert_eq!(&first_block(vec![Voice::whole(&stereo, 1.0)])[..4], &[0.5, -0.25, 0.0, 0.0]);
    }

    /// A clip at another rate is read at its own; a pitch reads it faster;
    /// a point's stereo clip is taken as one channel, each side at its gain.
    #[test]
    fn rates_pitches_and_a_point_s_gains() {
        let ramp: Vec<i16> = (0..400).map(|i| (i * 64) as i16).collect();
        let fast = Samples::of(&trp_data::sound::Clip { name: "f".into(), channels: 1, frequency: 2 * RATE, samples: ramp.clone() });
        let out = first_block(vec![Voice::whole(&fast, 1.0 / MIDDLE)]);
        assert!((out[2] - 128.0 / 32768.0).abs() < 1e-6, "{}", out[2]);
        let pitch: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.5f32.to_bits()));
        let plain = Samples::of(&trp_data::sound::Clip { name: "p".into(), channels: 1, frequency: RATE, samples: ramp });
        let out = first_block(vec![Voice::placed(&plain, [1.0, 0.5], pitch)]);
        // The second frame is read half way between the clip's first two
        // past it: 1.5 frames in.
        assert!((out[2] - 96.0 / 32768.0).abs() < 1e-6 && (out[3] - 48.0 / 32768.0).abs() < 1e-6, "{} {}", out[2], out[3]);
        let pair = Samples::of(&trp_data::sound::Clip { name: "s".into(), channels: 2, frequency: RATE, samples: vec![16384, 0] });
        let one: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
        let out = first_block(vec![Voice::placed(&pair, [1.0, 0.0], one)]);
        assert_eq!(&out[..2], &[0.25, 0.0]);
    }

    /// A loop scheduled at a frame of the mix plays from there round and
    /// round until its cut, and the sound scheduled at the cut's frame
    /// takes over on that very frame (a weapon's loop and its tail).
    #[test]
    fn a_loop_is_cut_on_the_frame_its_tail_begins() {
        // A loop of four frames counting up, and a tail of one level.
        let loop_clip = Samples::of(&trp_data::sound::Clip { name: "loop".into(), channels: 2, frequency: RATE, samples: vec![1000, 1000, 2000, 2000, 3000, 3000, 4000, 4000] });
        let tail_clip = Samples::of(&trp_data::sound::Clip { name: "tail".into(), channels: 2, frequency: RATE, samples: vec![-8000; 2 * 200] });
        let mixed = Mixed::default();
        let (cut, never) = (no_cut(), no_cut());
        cut.store(70, std::sync::atomic::Ordering::Release);
        mixed.add(Voice::whole(&loop_clip, 1.0).at_frame(10, true, cut));
        mixed.add(Voice::whole(&tail_clip, 1.0).at_frame(70, false, never));
        let mut mixing = mixed.decoder();
        let left: Vec<f32> = (0..BLOCK * 2 * 2).map(|_| mixing.next().unwrap()).step_by(2).collect();
        let level = |frame: usize| (left[frame] * 32768.0).round() as i32;
        assert_eq!(level(9), 0, "nothing before the loop's frame");
        assert_eq!((level(10), level(11), level(13), level(14)), (1000, 2000, 4000, 1000), "the loop from its first frame, round again after four");
        assert_eq!((level(69), level(70)), (4000, -8000), "the loop's last frame, then the tail's first");
        assert_eq!(mixed.clock(), 2 * BLOCK as u64);
    }
}
