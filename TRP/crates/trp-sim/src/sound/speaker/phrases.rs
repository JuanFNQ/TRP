//! Banks of phrases, statuses, situations and what was said.

use super::*;

/// A clip of a bank: its tags and its length as the bank keeps it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BankClip {
    pub mask: u32,
    pub length: f32,
}

/// A voice's bank (`TagBank`).
#[derive(Debug, Clone, PartialEq)]
pub struct Bank {
    pub trigger: i32,
    pub importance: i32,
    pub ignore_tags: bool,
    pub clips: Vec<BankClip>,
    /// The phrase its own speaker may say after it (`ChainEvent` whose
    /// `Who` is the speaker itself); a chain to its group or another's is
    /// none (nothing says it).
    pub chain: Option<Chain>,
}

/// A bank's chain (`Chain`): the phrase, and its odds in a hundred
/// (`Random.Range(0, 100) < Probability`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chain {
    pub trigger: i32,
    pub probability: i32,
}

impl Chain {
    /// Of a bank's chain as the voice keeps it (`who` 0 the speaker
    /// itself, 1 its group, 2 another): its own only.
    pub fn own(who: i32, trigger: i32, probability: i32) -> Option<Self> {
        (who == 0).then_some(Self { trigger, probability })
    }
}

impl Bank {
    /// A bank as the voice keeps it (`trp_data::sound::voice_banks`): its
    /// phrase, importance, whether its tags count, its clips' tags and
    /// lengths, and its own chain.
    pub fn of(b: &trp_data::audio::sound::PhraseBank) -> Self {
        Self {
            trigger: b.trigger,
            importance: b.importance,
            ignore_tags: b.ignore_tags,
            clips: b.clips.iter().map(|c| BankClip { mask: c.mask, length: c.length }).collect(),
            chain: Chain::own(b.chain.who, b.chain.event, b.chain.probability),
        }
    }
}

/// How hurt the character is, for the voice (`Player.HealthStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Healthy,
    Injured,
    BadlyInjured,
    Dying,
}

impl Status {
    /// Of the health: with the share of all its points left, and its parts
    /// under 15 % of their most (`GetBodyPartsInCriticalCondition(0.15)`),
    /// a vital one (head, chest, stomach) or any: dying with a vital one or
    /// under a fifth left; badly injured with more than two, or under two
    /// fifths; injured under nine tenths.
    pub fn of(health: &Health) -> Status {
        Self::of_parts(Part::ALL.map(|p| [health.part(p).current, health.part(p).maximum]))
    }

    /// Of each part's points now and at most, in `Part::ALL`'s order (a
    /// recorded character's).
    pub fn of_parts(parts: [[f32; 2]; 7]) -> Status {
        let (current, most) = parts.iter().fold((0.0, 0.0), |(c, m), [pc, pm]| (c + pc, m + pm));
        let share = if most > 0.0 { current / most } else { 0.0 };
        let critical = |p: &Part| {
            let [current, maximum] = parts[p.index()];
            maximum > 0.0 && current / maximum < 0.15
        };
        let all = Part::ALL.iter().filter(|p| critical(p)).count();
        let vital = [Part::Head, Part::Chest, Part::Stomach].iter().filter(|p| critical(p)).count();
        if vital > 0 || share < 0.2 {
            Status::Dying
        } else if all > 2 || share < 0.4 {
            Status::BadlyInjured
        } else if share < 0.9 {
            Status::Injured
        } else {
            Status::Healthy
        }
    }

    pub fn tag(self) -> u32 {
        match self {
            Status::Healthy => tag::HEALTHY,
            Status::Injured => tag::INJURED,
            Status::BadlyInjured => tag::BADLY_INJURED,
            Status::Dying => tag::DYING,
        }
    }
}

/// What the character is as the voice asks it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Situation {
    pub alive: bool,
    pub status: Status,
    /// The movement's painkillers' condition (`OnPainkillers`).
    pub on_painkillers: bool,
    /// A painkiller at work (`FindActiveEffect<IPainKiller>`).
    pub painkiller: bool,
    /// The breath heard (`PhysicalBase.BreathIsAudible`): not held, and the
    /// legs' stamina or the breath's below its exhaustion.
    pub audible: bool,
}

/// A phrase to play: its bank and clip, by their places, and whether the
/// voice is muffled (a face shield down over armour on the jaw: the
/// speech's reverb group, `ClientPlayerSelfSpeechReverb`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phrase {
    pub bank: usize,
    pub clip: usize,
    pub muffled: bool,
}

/// What the voice says to its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    /// Play this, cutting what plays (the speech source's `Play`).
    Phrase(Phrase),
    /// What plays is let go of and stops (`OnSpeakerRelease`'s release).
    Release,
}

/// The least fall damage heard even on painkillers (`MIN_FALL_DAMAGE`).
pub(super) const MIN_FALL_DAMAGE: f32 = 1.0;

/// The damage above which a hurt is heard on painkillers.
pub(super) const PAINKILLER_DAMAGE: f32 = 4.0;

/// The ticks of a self-inflicted damage between two grunts, for each such
/// effect at work (`DEFAULT_TICK_COUNT`).
pub(super) const TICKS: u32 = 16;

/// The speaker stays busy this long past its clip (`BUFFER_TIME`).
pub(super) const BUFFER: f32 = 0.1;

/// The kinds of effect whose damage is the character's own, counted for
/// its grunt (`FillSelfInflictedHealthEffects`: of these the health has
/// the stimulator, an intoxication, the bleedings, dehydration,
/// exhaustion, existence).
pub(super) fn self_inflicting(kind: Kind) -> bool {
    matches!(kind, Kind::Stimulator | Kind::Intoxication | Kind::LightBleeding | Kind::HeavyBleeding | Kind::Dehydration | Kind::Exhaustion | Kind::Existence)
}
