//! Sound groups, decibel sums and the headphones' fading level.

use super::*;

/// The groups of the game's mixer under `World` the cut reaches, each by
/// its send into the bus (`Ambient` and `Returns` hold nothing of TRP's).
/// The rest (the inventory's, the interface's, the music, the chat) are
/// not under `World`, and are not cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// `Guns`: the shots and the blasts (`Gunshots`), the hands' sounds
    /// (`Instrumental`).
    Guns,
    /// `ClientPlayer`: the character's own movement, kit and switches, its
    /// voice (`ClientPlayerMovement`, `ClientPlayerSpeech`,
    /// `ClientPlayerSelfSpeechReverb`).
    ClientPlayer,
    /// `ObservedPlayer`: another character's movement and voice.
    ObservedPlayer,
    /// `NPC`.
    Npc,
    /// `CommonSounds`: whatever BetterAudio's source groups play (the hits,
    /// the cases, things handled, a bone's crack).
    CommonSounds,
    NatureSounds,
    TechnicalSounds,
    /// `Ambient`: the raid's ambience.
    Ambient,
    /// `Returns`: the reverb and delay returns.
    Returns,
    /// `Occlusion/SimpleOccluded`: another character's voice muffled by its
    /// face shield (`Player.UpdateMuffledState` seen from outside). Not
    /// under `Main`: its dry fader is `OcclusionVolume` alone, and its send
    /// to the bus's compressor is fixed (`SIMPLE_OCCLUDED_SEND_DB`).
    SimpleOccluded,
}

impl Group {
    pub const ALL: [Self; 10] = [Self::Guns, Self::ClientPlayer, Self::ObservedPlayer, Self::Npc, Self::CommonSounds, Self::NatureSounds, Self::TechnicalSounds, Self::Ambient, Self::Returns, Self::SimpleOccluded];
    pub const COUNT: usize = Self::ALL.len();

    /// Its send into the bus, where a template sets it.
    pub fn send(self) -> Option<Param> {
        Some(match self {
            Self::Guns => Param::GunsSend,
            Self::ClientPlayer => Param::ClientPlayerSend,
            Self::ObservedPlayer => Param::ObservedPlayerSend,
            Self::Npc => Param::NpcSend,
            Self::CommonSounds => Param::EnvCommonSend,
            Self::NatureSounds => Param::EnvNatureSend,
            Self::TechnicalSounds => Param::EnvTechnicalSend,
            Self::Ambient => Param::AmbientSend,
            Self::Returns => Param::EffectsReturnsSend,
            Self::SimpleOccluded => return None,
        })
    }
}

/// The two compressors' groups' volumes under the bus, `GunCompressor`
/// (the guns' send) and `Compressor` (every other's): 0 dB in the start
/// snapshot, and set by nothing (the install; `trp-data`'s
/// `the_mixer_s_sends_and_the_headphones_names_are_read`).
const GUN_COMPRESSOR_DB: f32 = 0.0;

const COMPRESSOR_DB: f32 = 0.0;

/// `SimpleOccluded`'s send to `Compressor`, which no parameter exposes:
/// 0 dB (`probe mixer`: `-3>Compressor@0`).
const SIMPLE_OCCLUDED_SEND_DB: f32 = 0.0;

/// Two levels heard together, the same sound in phase: their amplitudes
/// added, in dB.
pub fn sum_db(a: f32, b: f32) -> f32 {
    let amplitude = |db: f32| 10f64.powf(db as f64 / 20.0);
    (20.0 * (amplitude(a) + amplitude(b)).log10()) as f32
}

/// A fade of one value under way (`AudioMixerFader.FadeValue`): from where
/// the value stood as it began, to its target, the time gone, by which
/// easing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Fade {
    from: f32,
    to: f32,
    elapsed: f32,
    easing: FadeType,
}

/// The headphones as the game keeps them (`ActiveHeadphonesController` and
/// its fader): the template last applied, the mixer's values it sets as
/// they stand, each one's fade under way, the value last asked of each
/// (`_prevValues`), and the main group's level, which a tinnitus moves.
#[derive(Debug, Clone)]
pub struct Headphones {
    pub(super) settings: HeadphonesSettings,
    current: &'static Template,
    values: [f32; Param::ALL.len()],
    fades: [Option<Fade>; Param::ALL.len()],
    asked: [Option<f32>; Param::ALL.len()],
    main: f32,
}

impl Headphones {
    /// Before any template is applied: the mixer's values as its start
    /// snapshot has them (`start`, in `Param::ALL`'s order), the default
    /// template current, the main group at 0 dB.
    pub fn new(settings: HeadphonesSettings, start: [f32; Param::ALL.len()]) -> Self {
        Self { settings, current: &DEFAULT, values: start, fades: [None; Param::ALL.len()], asked: [None; Param::ALL.len()], main: 0.0 }
    }

    /// The default template's values as the start (where the mixer's own
    /// cannot be read).
    pub fn default_start() -> [f32; Param::ALL.len()] {
        Param::ALL.map(|param| DEFAULT.value(param))
    }

    /// The template last applied (`CurrentTemplate`).
    pub fn current(&self) -> &'static Template {
        self.current
    }

    /// A value of the mixer as it stands.
    pub fn value(&self, param: Param) -> f32 {
        self.values[param as usize]
    }

    /// Whether any value is still fading.
    pub fn fading(&self) -> bool {
        self.fades.iter().any(Option::is_some)
    }

    /// A template applied (`ApplyTemplate`): every fade stopped where it
    /// stands (`StopFade`), then each value faded from there to the
    /// template's over the settings' time, by their fade in where the new
    /// value is above the one last asked of it, else (and the first time)
    /// by their fade out; set at once with no time. The same template again
    /// restarts its fades (by the fade out, the values being equal).
    pub fn apply(&mut self, template: &'static Template) {
        self.current = template;
        for (at, param) in Param::ALL.into_iter().enumerate() {
            let to = template.value(param);
            let easing = match self.asked[at] {
                Some(last) if to > last => self.settings.fade_in,
                _ => self.settings.fade_out,
            };
            self.asked[at] = Some(to);
            if self.settings.fade_duration > 0.0 {
                self.fades[at] = Some(Fade { from: self.values[at], to, elapsed: 0.0, easing });
            } else {
                self.values[at] = to;
                self.fades[at] = None;
            }
        }
    }

    /// A frame of `dt` seconds (`FadeValue`, a coroutine's step a frame,
    /// the first on the frame of the application itself): while the time
    /// gone is short of the fade's, the frame's time added and the value
    /// set at the easing of the share gone (held to 1) between where it
    /// began and its target; on the frame after, the target exactly.
    pub fn step(&mut self, dt: f32) {
        let duration = self.settings.fade_duration;
        for (value, fade) in self.values.iter_mut().zip(self.fades.iter_mut()) {
            let Some(going) = fade.as_mut() else { continue };
            if going.elapsed >= duration {
                *value = going.to;
                *fade = None;
                continue;
            }
            going.elapsed += dt;
            let eased = going.easing.ease((going.elapsed / duration).clamp(0.0, 1.0));
            *value = going.from + (going.to - going.from) * eased.clamp(0.0, 1.0);
        }
    }

    /// A frame of a contusion's tinnitus (`BetterAudio`'s coroutine): the
    /// guns' group at the current template's dry level plus the dip
    /// (`MainChannelLevel`), the main group at its level as the ringing
    /// began plus the same dip (`PROVISIONAL`: that level taken as 0 dB, the
    /// game's in a raid not read). After the frame's fades (`PROVISIONAL`:
    /// the order of the two coroutines is not read; they meet only while a
    /// template fades during a ringing). Both are dry faders: what a group
    /// sends through the bus is not dipped.
    pub fn ring(&mut self, dip: f32) {
        self.values[Param::GunsVolume as usize] = self.current.dry_volume + dip;
        self.main = dip;
    }

    /// The ringing's end: the guns' group back at the current template's
    /// dry level, the main group at 0 dB.
    pub fn quiet(&mut self) {
        self.values[Param::GunsVolume as usize] = self.current.dry_volume;
        self.main = 0.0;
    }

    /// A group's level as heard, in dB: its dry way (the faders above it
    /// that the template and the tinnitus set: the guns' group's alone;
    /// for the others, theirs under the main group, the environment's, the
    /// ambience's or the returns') and its send through the bus's
    /// compressor and the bus, summed (`sum_db`). Every group sends before
    /// its fader, but the returns after theirs (the mixer's effects in
    /// order, a group's fader listed only where something follows it).
    pub fn level(&self, group: Group) -> f32 {
        let value = |param: Param| self.values[param as usize];
        let bus = value(Param::HeadphonesVolume);
        let (dry, sent) = match group {
            Group::Guns => (value(Param::GunsVolume), value(Param::GunsSend) + GUN_COMPRESSOR_DB + bus),
            Group::Ambient => (value(Param::AmbientVolume) + self.main, value(Param::AmbientSend) + COMPRESSOR_DB + bus),
            Group::Returns => {
                let fader = value(Param::EffectsReturnsGroupVolume);
                (fader + self.main, fader + value(Param::EffectsReturnsSend) + COMPRESSOR_DB + bus)
            }
            Group::SimpleOccluded => (value(Param::OcclusionVolume), SIMPLE_OCCLUDED_SEND_DB + COMPRESSOR_DB + bus),
            other => (value(Param::EnvironmentVolume) + self.main, other.send().map_or(-80.0, value) + COMPRESSOR_DB + bus),
        };
        sum_db(dry, sent)
    }
}
