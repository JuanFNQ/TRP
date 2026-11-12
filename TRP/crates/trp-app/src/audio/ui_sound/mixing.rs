//! The mix, its gains, the tape, and the mixing source.

use super::*;

/// The one source the sounds are mixed into (a Bevy audio source of its
/// own): the sounds playing, which `play` adds to.
#[derive(Asset, TypePath, Clone, Default)]
pub struct Mixed {
    pub(super) voices: Arc<Mutex<Vec<Voice>>>,
    /// Everything mixed since the tape was started, if it was
    /// (`--sound-out`): two channels at `RATE`.
    tape: Arc<Mutex<Option<Vec<f32>>>>,
    /// How many frames have been mixed: the mix's own clock (the game's
    /// `AudioSettings.dspTime`).
    pub(super) clock: Arc<std::sync::atomic::AtomicU64>,
    /// What the game's own mixer does that the app's does, once read
    /// (`trp_data::sound::master_mixer`); set with its gains at 1 where it
    /// cannot be read.
    pub(super) game: Arc<std::sync::OnceLock<GameMix>>,
    /// The levels of the game's groups the app moves, by `Group`.
    pub(super) groups: [Level; Group::COUNT],
}

/// The game's mixer as the app's follows it: the master group's effect as
/// read (kind 16: -1, 10, 20, 0; taken for a compressor's threshold in dB,
/// attack and release in seconds, make-up in dB, and not applied: measured
/// not to act, `Mixing::mix`), and as gains the volumes of the gunshots' group and of the
/// player's own movement's (`ClientPlayer/ClientPlayerMovement`), each
/// with the groups above it.
#[derive(Debug, Clone, Copy)]
pub struct GameMix {
    pub compressor: Option<[f32; 4]>,
    pub gunshots: f32,
    pub movement: f32,
    /// The player's own speech's (`ClientPlayer/ClientPlayerSpeech`) and
    /// its muffled speech's (`ClientPlayerSelfSpeechReverb`).
    pub speech: f32,
    pub self_speech: f32,
    /// Another character's speech (`ObservedPlayer/ObservedPlayerSpeech`,
    /// -5 dB) and its muffled speech (`Occlusion/SimpleOccluded`, 0 dB).
    pub observed_speech: f32,
    pub occluded: f32,
    /// The start snapshot's values of what a headphones template sets
    /// (`trp_sim::hearing::Param::ALL`, by their names), where all are
    /// found: a template's first fade begins there (`hearing`).
    pub headphones_start: Option<[f32; trp_sim::sound::hearing::Param::ALL.len()]>,
}

impl GameMix {
    /// What the app's mix goes by where the game's mixer cannot be read:
    /// its groups add nothing.
    pub(super) fn unread() -> Self {
        Self { compressor: None, gunshots: 1.0, movement: 1.0, speech: 1.0, self_speech: 1.0, observed_speech: 1.0, occluded: 1.0, headphones_start: None }
    }
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

    /// The player's speech's group's gain, or its muffled speech's, with
    /// the groups above it (1 until the game's mixer is read).
    pub fn speech_gain(&self, muffled: bool) -> f32 {
        self.game.get().map_or(1.0, |game| if muffled { game.self_speech } else { game.speech })
    }

    /// Another character's speech's group's gain, or its muffled speech's,
    /// with the groups above it (1 until the game's mixer is read).
    pub fn observed_speech_gain(&self, muffled: bool) -> f32 {
        self.game.get().map_or(1.0, |game| if muffled { game.occluded } else { game.observed_speech })
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

    /// A sound of a group of the game's mixer, following its level.
    pub fn add_to(&self, group: Group, voice: Voice) {
        self.add(voice.following_level(self.level(group)));
    }

    /// A group's level, to move (`set_level`).
    pub fn level(&self, group: Group) -> Level {
        self.groups[group as usize].clone()
    }

    /// The game's mixer as read, once its reading is over (its gains at 1
    /// where it could not be read).
    pub fn game(&self) -> Option<&GameMix> {
        self.game.get()
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
        Some(trp_data::audio::sound::Clip { name: String::new(), channels: 2, frequency: RATE, samples }.wav())
    }
}

impl Decodable for Mixed {
    type DecoderItem = f32;
    type Decoder = Mixing;

    fn decoder(&self) -> Mixing {
        Mixing { voices: self.voices.clone(), tape: self.tape.clone(), clock: self.clock.clone(), block: Vec::new(), at: 0 }
    }
}

/// The source's samples: each block of 64 frames mixed from the sounds
/// playing as it comes, those done let go.
pub struct Mixing {
    pub(super) voices: Arc<Mutex<Vec<Voice>>>,
    tape: Arc<Mutex<Option<Vec<f32>>>>,
    pub(super) clock: Arc<std::sync::atomic::AtomicU64>,
    block: Vec<f32>,
    pub(super) at: usize,
}

pub(super) const BLOCK: usize = 64;

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
        // The master's effect (`GameMix::compressor`) is not applied: in
        // `courses2`'s `7-sound-shots` the game's mix runs over full scale
        // (to +1.5 dBFS) and a shot's first 45 ms are heard at one gain
        // whether the mix there is at +1.0 dBFS or at -4.6 (sound spec, "The
        // game's mixer").
        if let Ok(mut tape) = self.tape.lock() {
            if let Some(tape) = tape.as_mut() {
                tape.extend_from_slice(&self.block);
            }
        }
    }


    fn mix_voices(&mut self, voices: &mut Vec<Voice>, first: u64) {
        for voice in voices.iter_mut() {
            let pitch = voice.pitch.as_ref().map_or(1.0, |pitch| f32::from_bits(pitch.load(std::sync::atomic::Ordering::Relaxed)) as f64);
            let db: f32 = voice.levels.iter().map(|level| f32::from_bits(level.load(std::sync::atomic::Ordering::Relaxed))).sum();
            let level = if db == 0.0 { 1.0 } else { 10f32.powf(db / 20.0) };
            // A sound told its gains anew (`Voice::following_gains`) glides
            // to them: over this block by a straight line from where the
            // last block left them to a share nearer what they are told now
            // (`GLIDE`), so a gain never jumps. Any other keeps its own.
            let from = voice.gains;
            if let Some(told) = &voice.told {
                let to = gains_told(told);
                voice.gains = [0, 1].map(|side| {
                    let left = to[side] - from[side];
                    if left.abs() < 1e-6 { to[side] } else { from[side] + left * GLIDE }
                });
            }
            let by = [0, 1].map(|side| (voice.gains[side] - from[side]) * level / BLOCK as f32);
            let from = from.map(|gain| gain * level);
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
                let gains = [from[0] + by[0] * (i + 1) as f32, from[1] + by[1] * (i + 1) as f32];
                if channels == 1 || voice.merged {
                    let one = if channels == 1 { at(0) } else { (at(0) + at(1)) * 0.5 };
                    self.block[2 * i] += one * gains[0];
                    self.block[2 * i + 1] += one * gains[1];
                } else {
                    self.block[2 * i] += at(0) * gains[0];
                    self.block[2 * i + 1] += at(1) * gains[1];
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
