//! The character's own voice (health spec, "The voice"): the hurt grunts,
//! the death's and the agony's cries and the heavy breathing, as the game's
//! speaker says them for the player (`BaseSpeaker`, `TagBank.Match`;
//! `Player.OnAudioHealthApplyDamage`, `ApplyHitDebuff`, `OnDead`,
//! `UpdateBreathStatus`, `OnSpeakerRelease`,
//! `SetupSoundsFromFaceShieldComponent`;
//! `PlayerSelfInflictedDamageThresholdAudioChecker`). The host gives it the
//! voice's banks, tells it what the health and the body did, and plays
//! what it says (`take_said`): a phrase, which cuts the one before it, or
//! the end of what plays.

use crate::health::{DamageKind, Health, Kind, Part};
use crate::recoil::Rng;

/// What a bank is said for (`EPhraseTrigger`).
pub mod trigger {
    pub const AGONY: i32 = 9;
    pub const BEING_HURT: i32 = 15;
    pub const DEATH: i32 = 26;
    pub const BREATH: i32 = 29;
    /// The banks the voice reads.
    pub const READ: [i32; 4] = [AGONY, BEING_HURT, DEATH, BREATH];
}

/// The tags a clip answers to and a phrase is said with (`ETagStatus`).
pub mod tag {
    pub const UNAWARE: u32 = 0x1;
    pub const AWARE: u32 = 0x2;
    pub const SOLO: u32 = 0x8;
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
        let [current, most] = health.common();
        let share = if most > 0.0 { current / most } else { 0.0 };
        let critical = |p: &Part| {
            let s = health.part(*p);
            s.maximum > 0.0 && s.current / s.maximum < 0.15
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
const MIN_FALL_DAMAGE: f32 = 1.0;
/// The damage above which a hurt is heard on painkillers.
const PAINKILLER_DAMAGE: f32 = 4.0;
/// The ticks of a self-inflicted damage between two grunts, for each such
/// effect at work (`DEFAULT_TICK_COUNT`).
const TICKS: u32 = 16;
/// The speaker stays busy this long past its clip (`BUFFER_TIME`).
const BUFFER: f32 = 0.1;

/// The kinds of effect whose damage is the character's own, counted for
/// its grunt (`FillSelfInflictedHealthEffects`: of these the health has
/// the stimulator, the bleedings, dehydration, exhaustion, existence).
fn self_inflicting(kind: Kind) -> bool {
    matches!(kind, Kind::Stimulator | Kind::LightBleeding | Kind::HeavyBleeding | Kind::Dehydration | Kind::Exhaustion | Kind::Existence)
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
    said: Vec<Said>,
    rng: Rng,
}

impl Speaker {
    pub fn new(seed: u64) -> Self {
        Self {
            banks: Vec::new(),
            excluded: Vec::new(),
            release_time: f32::NEG_INFINITY,
            importance: 0,
            speaking: false,
            heavy_breath: false,
            muffled: false,
            inflicting: Vec::new(),
            count: 0,
            target: TICKS,
            last_hit: Part::Head,
            said: Vec::new(),
            rng: Rng::new(seed),
        }
    }

    /// A new character's speaker with this one's voice (a run begun again).
    pub fn renewed(self) -> Self {
        let mut speaker = Speaker { rng: self.rng, ..Speaker::new(0) };
        speaker.set_banks(self.banks);
        speaker
    }

    /// The voice's banks, once read.
    pub fn set_banks(&mut self, banks: Vec<Bank>) {
        self.excluded = banks.iter().map(|b| vec![false; b.clips.len()]).collect();
        self.banks = banks;
    }

    pub fn has_banks(&self) -> bool {
        !self.banks.is_empty()
    }

    /// What it said since last asked.
    pub fn take_said(&mut self) -> Vec<Said> {
        std::mem::take(&mut self.said)
    }

    /// Busy until its clip and a tenth of a second are over.
    pub fn busy(&self, now: f32) -> bool {
        now < self.release_time
    }

    pub fn importance(&self) -> i32 {
        self.importance
    }

    /// Says a bank's phrase (`BaseSpeaker.Play`): the bank's importance
    /// unless told; nothing while busy with one at least as important;
    /// the clip drawn by its tags, the speaker's group (alone) added.
    fn play(&mut self, trigger: i32, tags: u32, importance: Option<i32>, now: f32) -> bool {
        let Some(bank) = self.banks.iter().position(|b| b.trigger == trigger) else { return false };
        let importance = importance.unwrap_or(self.banks[bank].importance);
        if self.busy(now) && importance <= self.importance {
            return false;
        }
        let Some(clip) = self.draw(bank, tags | tag::SOLO) else { return false };
        self.speaking = true;
        self.release_time = now + self.banks[bank].clips[clip].length + BUFFER;
        self.said.push(Said::Phrase(Phrase { bank, clip, muffled: self.muffled }));
        self.importance = importance;
        true
    }

    /// A clip of the bank by the tags, each once a round and never the
    /// same twice running (`TagBank.Match`). The game draws it with
    /// Unity's generator, this with its own.
    fn draw(&mut self, bank: usize, tags: u32) -> Option<usize> {
        let b = &self.banks[bank];
        let eligible: Vec<usize> = (0..b.clips.len()).filter(|&i| b.ignore_tags || compare(b.clips[i].mask, tags)).collect();
        let excluded = &mut self.excluded[bank];
        let mut open: Vec<usize> = eligible.iter().copied().filter(|&i| !excluded[i]).collect();
        let closed: Vec<usize> = eligible.iter().copied().filter(|&i| excluded[i]).collect();
        if open.len() == 1 {
            closed.iter().for_each(|&i| excluded[i] = false);
        } else if open.is_empty() {
            closed.iter().for_each(|&i| excluded[i] = false);
            open = closed;
        }
        if open.is_empty() {
            return None;
        }
        let pick = ((self.rng.unit() * open.len() as f32) as usize).min(open.len() - 1);
        excluded[open[pick]] = true;
        Some(open[pick])
    }

    /// Stops being busy now (`Shut`): what plays is let go of at the next
    /// update.
    pub fn shut(&mut self, now: f32) {
        self.release_time = now;
    }

    /// Each frame (`ManualUpdate`): no longer busy, the phrase is over and
    /// let go of (`OnSpeakerRelease`): the breathing asked again, and what
    /// plays stopped unless the breathing goes on.
    pub fn update(&mut self, now: f32, s: &Situation) {
        if self.busy(now) || !self.speaking {
            return;
        }
        self.speaking = false;
        self.heavy_breath = false;
        if s.alive {
            self.breath(now, s);
        }
        if !self.heavy_breath {
            self.said.push(Said::Release);
        }
    }

    /// The health took damage (`OnAudioHealthApplyDamage`, on every
    /// `ApplyDamage` with the damage as multiplied): a fall's heard even on
    /// painkillers; on painkillers nothing of 4 or less; the character's own
    /// (a bleeding, a stimulator, thirst) only every sixteenth tick for each
    /// such effect at work, and not badly hurt unless on a painkiller;
    /// anything else, a grunt.
    pub fn damaged(&mut self, kind: DamageKind, damage: f32, now: f32, s: &Situation) {
        if kind == DamageKind::Fall && damage > MIN_FALL_DAMAGE {
            self.hurt(now, s);
            return;
        }
        if s.on_painkillers && damage <= PAINKILLER_DAMAGE {
            return;
        }
        if kind.self_inflicted() {
            let calm = !matches!(s.status, Status::BadlyInjured | Status::Dying) || s.painkiller;
            if calm && self.tick() {
                self.hurt(now, s);
            }
        } else {
            self.hurt(now, s);
        }
    }

    /// A hit reached a part (`Player.ApplyDamageInfo`: `LastDamagedBodyPart`,
    /// `ApplyHitDebuff`), with its damage before the health's multiplier:
    /// alive and not on painkillers, or more than 4, a grunt (none while one
    /// is said).
    pub fn hit(&mut self, part: Part, damage: f32, now: f32, s: &Situation) {
        self.last_hit = part;
        if s.alive && (!s.on_painkillers || damage > PAINKILLER_DAMAGE) {
            self.play(trigger::BEING_HURT, s.status.tag(), None, now);
        }
    }

    /// `Player.Say(OnBeingHurt)`: the health's tag and the awareness's
    /// (unaware: nothing here makes the player aware).
    fn hurt(&mut self, now: f32, s: &Situation) {
        self.play(trigger::BEING_HURT, s.status.tag() | tag::UNAWARE, None, now);
    }

    /// The grunt's counter (`ThresholdCounter.TryReachThreshold`).
    fn tick(&mut self) -> bool {
        self.count += 1;
        if self.count >= self.target {
            self.count = 0;
            return true;
        }
        false
    }

    /// An effect started (`EffectStartedEvent`): one of the character's own
    /// damage counts for its grunt, the count begun again from none.
    pub fn effect_started(&mut self, kind: Kind, part: Part) {
        if !self_inflicting(kind) {
            return;
        }
        if self.inflicting.is_empty() {
            self.count = 0;
        }
        self.inflicting.push((kind, part));
        self.target = TICKS * self.inflicting.len() as u32;
    }

    /// An effect taken off (`EffectRemovedEvent`).
    pub fn effect_removed(&mut self, kind: Kind, part: Part) {
        if !self_inflicting(kind) {
            return;
        }
        if let Some(i) = self.inflicting.iter().position(|e| *e == (kind, part)) {
            self.inflicting.remove(i);
        }
        if self.inflicting.is_empty() {
            self.count = 0;
        } else {
            self.target = TICKS * self.inflicting.len() as u32;
        }
    }

    /// The character died (`Player.OnDead`): unless the last hit reached
    /// the head, its death's cry if a weapon killed it, its agony's
    /// otherwise, over anything said; a head's, silence.
    pub fn died(&mut self, kind: DamageKind, now: f32, s: &Situation) {
        if self.last_hit != Part::Head {
            let trigger = if kind.weapon() { trigger::DEATH } else { trigger::AGONY };
            self.play(trigger, s.status.tag(), None, now);
        } else {
            self.shut(now);
        }
    }

    /// The breathing asked (`Player.UpdateBreathStatus`): needed badly hurt
    /// off painkillers, with the breath heard, or muffled; started where it
    /// was not, with the health's tag where hurt and the breath's (aware
    /// where heard), at an importance of -1 when only muffled.
    pub fn breath(&mut self, now: f32, s: &Situation) {
        let hurt = matches!(s.status, Status::BadlyInjured | Status::Dying) && !s.painkiller;
        let need = hurt || s.audible || self.muffled;
        if need && !self.heavy_breath {
            let tags = (if hurt { s.status.tag() } else { tag::HEALTHY }) | if s.audible { tag::AWARE } else { tag::UNAWARE };
            let importance = (tags == tag::HEALTHY | tag::UNAWARE).then_some(-1);
            self.play(trigger::BREATH, tags, importance, now);
        }
        self.heavy_breath = need;
    }

    /// Whether the breath is heard changed (`PhysicalBase`'s audible
    /// effects): the breathing asked again from none.
    pub fn audible_changed(&mut self, now: f32, s: &Situation) {
        self.heavy_breath = false;
        self.breath(now, s);
    }

    /// The face shield changed (`SetupSoundsFromFaceShieldComponent`):
    /// muffled with one down over armour on the jaw; the breathing asked
    /// again, and with none down, silence where nothing else is breathed
    /// and what was said mattered no more than a breath.
    pub fn face_shield(&mut self, down: bool, jaw: bool, now: f32, s: &Situation) {
        self.muffled = down && jaw;
        self.breath(now, s);
        if !down && !self.heavy_breath && self.importance == 0 {
            self.shut(now);
        }
    }
}

/// Whether what is worn in a slot muffles the voice
/// (`SetupSoundsFromFaceShieldComponent`): its face shield down (or one
/// with no switch), and armour on the jaw in it or anything fitted to it
/// (`GetItemComponentsInChildren<CompositeArmorComponent>`).
pub fn muffles(db: &trp_data::Database, equipment: &crate::inventory::Equipment, slot: &str) -> bool {
    let Some(shield) = crate::visor::shield_in(equipment, slot) else { return false };
    let Some(worn) = equipment.worn(slot) else { return false };
    let jaw = crate::armor::collider_type("Jaw").map_or(0, |k| 1u32 << k);
    shield.down && crate::inventory::with_parts(worn).any(|part| crate::armor::Piece::of(db, &part.kind.template, 0, None).is_some_and(|piece| piece.colliders & jaw != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speaker() -> Speaker {
        let mut s = Speaker::new(1);
        let clips = |n: usize, mask: u32| (0..n).map(|i| BankClip { mask, length: 0.5 + i as f32 * 0.1 }).collect::<Vec<_>>();
        let mut breath = clips(1, 0x2000);
        breath.extend([BankClip { mask: 0x1800, length: 10.8 }, BankClip { mask: 0x0401, length: 14.6 }, BankClip { mask: 0x0402, length: 7.9 }]);
        s.set_banks(vec![
            Bank { trigger: trigger::BEING_HURT, importance: 2, ignore_tags: false, clips: clips(13, 0) },
            Bank { trigger: trigger::BREATH, importance: 0, ignore_tags: false, clips: breath },
            Bank { trigger: trigger::DEATH, importance: 3, ignore_tags: true, clips: clips(7, 0) },
            Bank { trigger: trigger::AGONY, importance: 3, ignore_tags: true, clips: clips(4, 0) },
        ]);
        s
    }

    fn calm() -> Situation {
        Situation { alive: true, status: Status::Healthy, on_painkillers: false, painkiller: false, audible: false }
    }

    #[test]
    fn tags_compare_field_by_field() {
        assert!(compare(0, tag::DYING | tag::SOLO));
        assert!(compare(0x0401, tag::HEALTHY | tag::UNAWARE | tag::SOLO));
        assert!(!compare(0x0402, tag::HEALTHY | tag::UNAWARE | tag::SOLO), "aware against unaware");
        assert!(!compare(0x1800, tag::HEALTHY | tag::AWARE), "hurt against healthy");
        assert!(compare(0x1800, tag::BADLY_INJURED));
    }

    #[test]
    fn every_clip_once_a_round_and_never_twice_running() {
        // The first round has all 13; each after it 12, the clip said
        // last held out of it (`Match` clears the others' marks as it
        // takes the round's last).
        let mut s = speaker();
        let drawn: Vec<usize> = (0..13 + 12 * 5).map(|_| s.draw(0, tag::HEALTHY).unwrap()).collect();
        let distinct = |w: &[usize]| w.iter().enumerate().all(|(i, c)| !w[..i].contains(c));
        assert!(distinct(&drawn[..13]));
        for round in drawn[13..].chunks(12) {
            assert!(distinct(round), "{round:?}");
        }
        assert!(drawn.windows(2).all(|w| w[0] != w[1]), "never twice running");
    }

    #[test]
    fn a_hit_grunts_once_and_painkillers_hush_small_ones() {
        let mut s = speaker();
        let now = 1.0;
        s.damaged(DamageKind::Bullet, 15.0, now, &calm());
        s.hit(Part::LeftArm, 15.0, now, &calm());
        assert_eq!(s.take_said().len(), 1, "one grunt a hit");
        let numb = Situation { on_painkillers: true, painkiller: true, ..calm() };
        s.damaged(DamageKind::Bullet, 3.0, 10.0, &numb);
        s.hit(Part::LeftArm, 3.0, 10.0, &numb);
        assert!(s.take_said().is_empty(), "4 or less on painkillers");
        s.damaged(DamageKind::Fall, 2.0, 20.0, &numb);
        assert_eq!(s.take_said().len(), 1, "a fall even so");
    }

    #[test]
    fn a_bleeding_grunts_every_sixteenth_tick() {
        let mut s = speaker();
        s.effect_started(Kind::LightBleeding, Part::LeftLeg);
        let mut grunts = 0;
        for t in 0..48 {
            s.damaged(DamageKind::LightBleeding, 0.5, t as f32 * 5.0, &calm());
            grunts += s.take_said().len();
        }
        assert_eq!(grunts, 3);
        s.effect_started(Kind::HeavyBleeding, Part::Chest);
        let mut grunts = 0;
        for t in 0..64 {
            s.damaged(DamageKind::HeavyBleeding, 0.5, 1000.0 + t as f32 * 5.0, &calm());
            grunts += s.take_said().len();
        }
        assert_eq!(grunts, 2, "two at work: every 32nd");
    }

    #[test]
    fn a_death_cries_unless_the_head_was_hit() {
        let mut s = speaker();
        s.hit(Part::Chest, 80.0, 1.0, &calm());
        s.take_said();
        let dead = Situation { alive: false, status: Status::Dying, ..calm() };
        s.died(DamageKind::Bullet, 1.0, &dead);
        assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 2, .. })]), "over the grunt: the death's");
        let mut s = speaker();
        s.hit(Part::Head, 80.0, 1.0, &calm());
        s.take_said();
        s.died(DamageKind::Bullet, 1.0, &dead);
        assert!(s.take_said().is_empty());
        s.update(1.05, &dead);
        assert_eq!(s.take_said(), vec![Said::Release], "the grunt let go of");
    }

    #[test]
    fn breathing_runs_on_while_needed_and_plays_out_after() {
        let mut s = speaker();
        let tired = Situation { audible: true, ..calm() };
        s.audible_changed(0.0, &tired);
        assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 3, .. })]), "the sprint's breath");
        s.update(8.0, &tired);
        assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 3, .. })]), "again at its end");
        s.audible_changed(9.0, &calm());
        assert!(s.take_said().is_empty() && !s.heavy_breath, "no longer needed: plays on");
        s.update(16.1, &calm());
        assert_eq!(s.take_said(), vec![Said::Release]);
    }

    #[test]
    fn a_face_shield_down_breathes_softly_and_up_lets_it_play_out() {
        let mut s = speaker();
        s.face_shield(true, true, 0.0, &calm());
        assert!(matches!(s.take_said()[..], [Said::Phrase(Phrase { bank: 1, clip: 2, muffled: true })]));
        assert_eq!(s.importance(), -1);
        s.face_shield(false, true, 5.0, &calm());
        assert!(s.busy(5.0), "not shut: it plays out");
        s.face_shield(true, false, 30.0, &calm());
        assert!(!s.muffled, "no armour on the jaw");
    }
}
