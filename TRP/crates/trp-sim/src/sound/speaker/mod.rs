//! The character's own voice.
//! After the health spec, "The voice": the hurt grunts,
//! the death's and the agony's cries and the heavy breathing, as the game's
//! speaker says them for the player (`BaseSpeaker`, `TagBank.Match`;
//! `Player.OnAudioHealthApplyDamage`, `ApplyHitDebuff`, `OnDead`,
//! `UpdateBreathStatus`, `OnSpeakerRelease`,
//! `SetupSoundsFromFaceShieldComponent`;
//! `PlayerSelfInflictedDamageThresholdAudioChecker`). The host gives it the
//! voice's banks, tells it what the health and the body did, and plays
//! what it says (`take_said`): a phrase, which cuts the one before it, or
//! the end of what plays.
//!
//! The same speaker is any character's (`Player.Init` makes one for each):
//! a bot's (`of_ai`: `IsAI`, its speaker `OnDemandOnly` off) differs in
//! three things only. A fall's damage is not heard on painkillers as the
//! player's is, a hit's own grunt (`ApplyHitDebuff`) is the player's alone,
//! and a bank's chain is queued after its phrase (`BaseSpeaker.Play`,
//! `Queue`, `ManualUpdate`): a hurt's grunt followed by the disappointment's
//! phrase at its bank's odds (`courses3\3-bot-shots`, Scav 6's
//! `scav6_hurt_light_03_l` after `scav6_hit_04`). What the bot's mind says
//! (contact, mutter, enemy down) is its AI's, not this.

use crate::condition::health::{DamageKind, Health, Kind, Part};
use crate::weapons::recoil::Rng;

mod phrases;
mod saying;
mod reactions;

pub use phrases::*;
pub use reactions::*;
use phrases::{BUFFER, MIN_FALL_DAMAGE, PAINKILLER_DAMAGE, TICKS, self_inflicting};

/// What a bank is said for (`EPhraseTrigger`).
pub mod trigger {
    /// Nothing (`None`): a bank chained to it queues a phrase that is
    /// never said.
    pub const NONE: i32 = 0;
    /// No phrase (`PhraseNone`): what is queued when nothing is.
    pub const PHRASE_NONE: i32 = 8;
    pub const AGONY: i32 = 9;
    pub const BEING_HURT: i32 = 15;
    /// The hurt's disappointment (`OnBeingHurtDissapoinment`): what a
    /// PMC's hurt bank and Scav 6's chain to.
    pub const BEING_HURT_DISAPPOINTMENT: i32 = 16;
    pub const DEATH: i32 = 26;
    pub const FIGHT: i32 = 27;
    pub const MUTTER: i32 = 28;
    pub const BREATH: i32 = 29;
    /// The menu's mumble, said as the fight's or the mutter's bank
    /// (`MumblePhrase`; gestures spec, "A phrase said").
    pub const MUMBLE: i32 = 104;
    /// The banks whose sounds the voice reads at once; the rest's as each
    /// is first said (the app's `voice_sound`).
    pub const READ: [i32; 5] = [AGONY, BEING_HURT, DEATH, FIGHT, BREATH];
}

/// The tags a clip answers to and a phrase is said with (`ETagStatus`).
pub mod tag {
    pub const UNAWARE: u32 = 0x1;
    pub const AWARE: u32 = 0x2;
    pub const COMBAT: u32 = 0x4;
    pub const SOLO: u32 = 0x8;
    /// In a group with another of it alive within 50 m
    /// (`VirtualGroup.GetGroupStatus`): the Scavs are one group; a PMC
    /// bot's group has no name and the player none, so both are alone.
    pub const COOP: u32 = 0x10;
    pub const HEALTHY: u32 = 0x400;
    pub const INJURED: u32 = 0x800;
    pub const BADLY_INJURED: u32 = 0x1000;
    pub const DYING: u32 = 0x2000;
}

/// The tags' six fields' widths, in the order of their bits
/// (`TagBank.Sizes`).
const SIZES: [u32; 6] = [3, 2, 3, 2, 4, 3];

/// Whether a clip's tags let it be said with these (`TagBank.Compare`):
/// in each field where both have bits, they share one.
pub fn compare(mask: u32, tags: u32) -> bool {
    let mut offset = 0;
    SIZES.iter().all(|&width| {
        let field = ((1u32 << width) - 1) << offset;
        offset += width;
        let (a, b) = (mask & field, tags & field);
        a == 0 || b == 0 || a & b != 0
    })
}

/// The character's speaker.
#[derive(Debug, Clone)]
pub struct Speaker {
    banks: Vec<Bank>,
    /// Each clip said in this round of its bank (`TaggedClip.Exclude`).
    excluded: Vec<Vec<bool>>,
    release_time: f32,
    importance: i32,
    speaking: bool,
    /// The breathing said (`Player.HeavyBreath`).
    pub heavy_breath: bool,
    /// The speech muffled by a face shield (`Player.Muffled`).
    pub muffled: bool,
    /// The self-inflicting effects started and not removed, and the grunt's
    /// counter (`ThresholdCounter`: its count and its target).
    inflicting: Vec<(Kind, Part)>,
    count: u32,
    target: u32,
    /// The part a hit last reached (`LastDamagedBodyPart`; the head until
    /// one has).
    last_hit: Part,
    /// Until when the character is aware (`Player.Awareness`): a shot of
    /// its own, an enemy's hit.
    aware_until: f32,
    /// A bot's (`IsAI`; its speaker's `OnDemandOnly` off): see the module.
    ai: bool,
    /// The group's tag every phrase is said with
    /// (`SpeakerManager.GetGroupStatus`): `tag::SOLO` unless set.
    pub group: u32,
    /// The phrase queued to follow (`QueuedEvent`, `QueuedTags`,
    /// `QueuedDelay`): a bank's chain, said once the speaker is free and
    /// its time has come.
    queued: Option<Queued>,
    /// The tags the last phrase was drawn with.
    tags: u32,
    /// A recording's draws laid over the speaker's own (`steer`).
    steered: Steered,
    said: Vec<Said>,
    rng: Rng,
}

/// What a recording says the next draws came out as: a bank's clip (by
/// its phrase), and whether the next chain was drawn.
#[derive(Debug, Clone, Default, PartialEq)]
struct Steered {
    clips: Vec<(i32, usize)>,
    chain: Option<(i32, bool)>,
}

/// A phrase queued: what, with which tags, from when.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Queued {
    trigger: i32,
    tags: u32,
    at: f32,
}

impl Speaker {
}

#[cfg(test)]
mod tests;
