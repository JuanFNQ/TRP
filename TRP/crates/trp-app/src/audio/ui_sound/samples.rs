//! Levels, samples, pitch, and the voices playing.

use super::*;

/// A clip read, and what it is (with its name, for the interface's dump).
pub(super) enum Read {
    Ui(i32, String),
    Item(String),
    Load(String),
    Unload(String),
    Notification,
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

    pub(super) fn gain(index: u8) -> f32 {
        10f32.powf(Self::DECIBELS[index.min(10) as usize] / 20.0)
    }

    /// The overall volume as a share of the clip's level (the master
    /// group's, which every sound passes through).
    pub fn overall_gain(&self) -> f32 {
        Self::gain(self.overall)
    }

    /// A sound forced on the interface's group (`GUISounds.ForcePlaySound`:
    /// a voice heard as it is chosen): the overall volume's gain and the
    /// interface's, the interface's held at -17 dB (the table's fifth) at
    /// the least while it plays. (The game holds the group's whole volume
    /// so, the other interface sounds of that time with it: `PROVISIONAL`,
    /// only this sound is.)
    pub fn forced_gain(&self) -> f32 {
        Self::gain(self.overall) * Self::gain(self.interface.max(5))
    }
}

/// The rate the source plays at: the clips' own (all but a dozen of the
/// install's are kept at 44.1 kHz); one at another is read at its own rate
/// as it is mixed.
pub(super) const RATE: u32 = 44_100;

/// A clip ready to mix: its samples as read (16 bits, its channels
/// interleaved), how many channels and at what rate. Kept so, not as
/// floats: the sounds of moving alone are 100 MB of samples
/// (`step_sound`).
#[derive(Clone)]
pub struct Samples {
    pub(super) data: Arc<Vec<i16>>,
    pub(super) channels: usize,
    pub(super) rate: u32,
}

impl Samples {
    #[cfg(test)]
    pub fn of(clip: &trp_data::audio::sound::Clip) -> Self {
        Self { data: Arc::new(clip.samples.clone()), channels: clip.channels.max(1) as usize, rate: clip.frequency.max(1) }
    }

    pub fn of_owned(clip: trp_data::audio::sound::Clip) -> Self {
        let channels = clip.channels.max(1) as usize;
        Self { data: Arc::new(clip.samples), channels, rate: clip.frequency.max(1) }
    }

    pub fn mono(&self) -> bool {
        self.channels == 1
    }

    pub(super) fn frames(&self) -> usize {
        self.data.len() / self.channels
    }

    /// How long it plays at its own rate, in seconds (`AudioClip.length`).
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate.max(1) as f64
    }
}

/// The equal power of a mono source in the middle: each channel 1/√2 (the
/// game's mono sounds came out 3.0 to 3.4 dB under their clips' level in
/// the recording, its stereo ones at their own).
pub const MIDDLE: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// What each channel takes of a source at a point, at equal power: by how
/// far to the listener's right the way to it lies (`to`, from the listener;
/// `right`, the listener's right, of length one), wholly the left's at one
/// side and wholly the right's at the other, each `MIDDLE` ahead, behind,
/// above, below, and where the two are one place.
pub fn pan(to: Vec3, right: Vec3) -> [f32; 2] {
    let distance = to.length();
    let side = if distance > 1e-4 { (to / distance).dot(right).clamp(-1.0, 1.0) } else { 0.0 };
    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    [angle.cos(), angle.sin()]
}

/// A source's pitch that the sounds playing from it follow (the game's
/// steps and kit each play from a source whose pitch is set for each sound,
/// `step_sound`), as an `f32`'s bits.
pub type Pitch = Arc<std::sync::atomic::AtomicU32>;

/// A level a sound follows as it plays, in decibels as an `f32`'s bits (0
/// dB as made): a group of the game's mixer as heard, which scripts move
/// (a helmet's cut and a contusion's tinnitus: `hearing`), or one
/// sound's own volume moved as it plays.
pub type Level = Arc<std::sync::atomic::AtomicU32>;

/// Sets a level, in decibels.
pub fn set_level(level: &Level, db: f32) {
    level.store(db.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// What each of the two channels takes of a sound whose source moves as it
/// plays, told anew from the frame's side (`set_gains`): the left's and the
/// right's `f32` bits in one number, so that the mix never reads one side
/// of one frame with the other of the next.
pub type Gains = Arc<std::sync::atomic::AtomicU64>;

/// Gains to tell a sound anew, as they begin.
pub fn gains_of(gains: [f32; 2]) -> Gains {
    Arc::new(std::sync::atomic::AtomicU64::new(packed(gains)))
}

/// Tells a sound its gains anew: the mix glides to them (`GLIDE`).
pub fn set_gains(told: &Gains, gains: [f32; 2]) {
    told.store(packed(gains), std::sync::atomic::Ordering::Relaxed);
}

/// The gains as last told.
pub fn gains_told(told: &Gains) -> [f32; 2] {
    let both = told.load(std::sync::atomic::Ordering::Relaxed);
    [f32::from_bits((both >> 32) as u32), f32::from_bits(both as u32)]
}

fn packed(gains: [f32; 2]) -> u64 {
    (gains[0].to_bits() as u64) << 32 | gains[1].to_bits() as u64
}

/// The share of what is left to its told gains that a sound goes in a block
/// of the mix (`Voice::following_gains`): an eighth, most of the way in a
/// sixtieth of a second (735 frames, 11 blocks: 0.77). `ESTIMATE`: the
/// app's own, for a gain that does not jump between two frames' figures;
/// how the engine glides a moving source's volume is not read.
pub(super) const GLIDE: f32 = 0.125;

/// A sound being played: its clip, how far it has got (in the clip's
/// frames), what each of the two channels takes of it (of a mono clip's
/// one channel; of a stereo clip's own, or of both together if `merged`),
/// and the pitch it follows, if any. A sound scheduled (`Voice::at_frame`)
/// begins at a frame of the mix, may loop, and may be cut at a frame of the
/// mix set after it was added (the game's queue of a weapon's shots,
/// `shot_sound`).
pub struct Voice {
    pub(super) clip: Samples,
    pub(super) at: f64,
    pub(super) gains: [f32; 2],
    /// The gains it is told anew as it plays, if its source moves: `gains`
    /// then is where its glide to them has got.
    pub(super) told: Option<Gains>,
    /// Levels it follows as it plays: its group's and its own.
    pub(super) levels: Vec<Level>,
    pub(super) merged: bool,
    pub(super) pitch: Option<Pitch>,
    /// The frame of the mix it begins at; from the next block if 0.
    pub(super) starts: u64,
    pub(super) started: bool,
    pub(super) looping: bool,
    /// The frame of the mix it ends at (`u64::MAX`: its clip's end).
    pub(super) ends: Option<Cut>,
    pub(super) done: bool,
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
        Self { clip: clip.clone(), at: 0.0, gains: [each; 2], told: None, levels: Vec::new(), merged: false, pitch: None, starts: 0, started: false, looping: false, ends: None, done: false }
    }

    /// A clip from a point: what each channel takes of it, a stereo clip's
    /// two channels taken together (a point has one place), and the pitch
    /// it follows.
    pub fn placed(clip: &Samples, gains: [f32; 2], pitch: Pitch) -> Self {
        Self { clip: clip.clone(), at: 0.0, gains, told: None, levels: Vec::new(), merged: !clip.mono(), pitch: Some(pitch), starts: 0, started: false, looping: false, ends: None, done: false }
    }

    /// Its gains told anew as it plays (`set_gains`), for a source that
    /// moves or a listener that does: it begins at them as they stand, and
    /// each block goes a share of the way to what they are then (`GLIDE`).
    pub fn following_gains(mut self, told: Gains) -> Self {
        self.gains = gains_told(&told);
        self.told = Some(told);
        self
    }

    /// A level it follows as it plays besides its gains (read at each
    /// block's start).
    pub fn following_level(mut self, level: Level) -> Self {
        self.levels.push(level);
        self
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
