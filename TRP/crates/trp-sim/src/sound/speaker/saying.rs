//! Making a speaker, steering it, its awareness and saying and playing phrases.

use super::*;

impl Speaker {
    /// The player's own character's.
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
            aware_until: f32::NEG_INFINITY,
            ai: false,
            group: tag::SOLO,
            queued: None,
            tags: 0,
            steered: Steered::default(),
            said: Vec::new(),
            rng: Rng::new(seed),
        }
    }

    /// A bot's (`Player.Init` with `aiControlled`): its speaker queues its
    /// banks' chains, a fall is heard as any damage, and a hit has no grunt
    /// of its own.
    pub fn of_ai(seed: u64) -> Self {
        Self { ai: true, ..Self::new(seed) }
    }

    pub fn is_ai(&self) -> bool {
        self.ai
    }

    /// The tags the last phrase said was drawn with (a recording's clip
    /// checked against them).
    pub fn last_tags(&self) -> u32 {
        self.tags
    }

    /// A bank, by its place.
    pub fn bank(&self, at: usize) -> Option<&Bank> {
        self.banks.get(at)
    }

    /// For a recording set beside the rule (`voice_trace`): the next draw
    /// of that phrase's bank takes that clip where its tags let it (several
    /// may wait, a bank each), and the next draw of that phrase's bank's
    /// chain comes out as told, so that what follows (how long the speaker
    /// is busy, the chain) goes by the game's draws, which Unity's
    /// generator made and this cannot. `unsteer` lets go of what waits.
    pub fn steer(&mut self, clip: Option<(i32, usize)>, chain: Option<(i32, bool)>) {
        self.steered.clips.extend(clip);
        if chain.is_some() {
            self.steered.chain = chain;
        }
    }

    /// What was steered and not drawn, let go of.
    pub fn unsteer(&mut self) {
        self.steered = Steered::default();
    }

    /// Made aware for so many seconds from now, or longer if it already
    /// was (`Player.IncreaseAwareness`): 15 for a shot of its own
    /// (`OnMakingShot`), 20 for an enemy's hit (`ApplyHitDebuff`).
    pub fn aware(&mut self, seconds: f32, now: f32) {
        self.aware_until = self.aware_until.max(now + seconds);
    }

    /// Whether the character is aware now.
    pub fn is_aware(&self, now: f32) -> bool {
        now < self.aware_until
    }

    /// The awareness's tag a phrase is said with (`Player.Say`): combat
    /// where aggressive or aware, else unaware.
    pub(super) fn awareness(&self, aggressive: bool, now: f32) -> u32 {
        if aggressive || self.is_aware(now) {
            tag::COMBAT
        } else {
            tag::UNAWARE
        }
    }

    /// Whether the voice has a bank for that phrase (the menu offers only
    /// those: `Speaker.PhrasesBanks`).
    pub fn has(&self, trigger: i32) -> bool {
        trigger == trigger::MUMBLE || self.banks.iter().any(|bank| bank.trigger == trigger)
    }

    /// The voice's banks' phrases.
    pub fn triggers(&self) -> impl Iterator<Item = i32> + '_ {
        self.banks.iter().map(|bank| bank.trigger)
    }

    /// A phrase asked for (`Player.Say`, demanded, at once: the gestures'
    /// menu, a key bound to one, the quick prompt): the mumble said as the
    /// fight's bank where aggressive or aware, else the mutter's; with the
    /// health's tag and the awareness's. Whether it is said (refused while
    /// one at least as important plays).
    pub fn say(&mut self, trigger: i32, aggressive: bool, now: f32, s: &Situation) -> bool {
        let trigger = match trigger {
            trigger::MUMBLE if aggressive || self.is_aware(now) => trigger::FIGHT,
            trigger::MUMBLE => trigger::MUTTER,
            other => other,
        };
        let tags = s.status.tag() | self.awareness(aggressive, now);
        self.play(trigger, tags, None, now)
    }

    /// A phrase already said given to the host again, whose sound was not
    /// ready when it was said (a bank read as it is first wanted).
    pub fn replay(&mut self, phrase: Phrase) {
        self.said.push(Said::Phrase(phrase));
    }

    /// The length of the clip last said, if one plays (what the quick
    /// prompt blinks for).
    pub fn playing_for(&self, now: f32) -> Option<f32> {
        self.busy(now).then(|| self.release_time - BUFFER - now)
    }

    /// A new character's speaker with this one's voice (a run begun again).
    pub fn renewed(self) -> Self {
        let mut speaker = Speaker { rng: self.rng, ai: self.ai, group: self.group, ..Speaker::new(0) };
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
    /// the clip drawn by its tags, the speaker's group added; said over a
    /// phrase still playing, what was queued is dropped; then its bank's
    /// own chain drawn at its odds and, for a bot, queued to follow when
    /// the clip is over (`Queue(..., demand: false)`, which the player's
    /// speaker refuses).
    pub(super) fn play(&mut self, trigger: i32, tags: u32, importance: Option<i32>, now: f32) -> bool {
        if trigger == trigger::NONE {
            return false;
        }
        let Some(bank) = self.banks.iter().position(|b| b.trigger == trigger) else { return false };
        let importance = importance.unwrap_or(self.banks[bank].importance);
        if self.busy(now) && importance <= self.importance {
            return false;
        }
        let tags = tags | self.group;
        let Some(clip) = self.draw(bank, tags) else { return false };
        self.tags = tags;
        if self.busy(now) {
            self.queued = None;
        }
        let length = self.banks[bank].clips[clip].length;
        self.speaking = true;
        self.release_time = now + length + BUFFER;
        self.said.push(Said::Phrase(Phrase { bank, clip, muffled: self.muffled }));
        if let Some(chain) = self.banks[bank].chain {
            let drawn = self.rng.unit() * 100.0 < chain.probability as f32;
            let trigger = self.banks[bank].trigger;
            let steered = self.steered.chain.filter(|(of, _)| *of == trigger).map(|(_, chained)| chained);
            if steered.is_some() {
                self.steered.chain = None;
            }
            if steered.unwrap_or(drawn) && self.ai {
                // `PhraseNone` queued is nothing queued.
                self.queued = (chain.trigger != trigger::PHRASE_NONE).then_some(Queued { trigger: chain.trigger, tags, at: now + length });
            }
        }
        self.importance = importance;
        true
    }

    /// A clip of the bank by the tags, each once a round and never the
    /// same twice running (`TagBank.Match`). The game draws it with
    /// Unity's generator, this with its own.
    pub(super) fn draw(&mut self, bank: usize, tags: u32) -> Option<usize> {
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
        // A recording's clip, where its tags let it be drawn at all.
        if let Some(at) = self.steered.clips.iter().position(|(trigger, _)| *trigger == b.trigger) {
            let (_, clip) = self.steered.clips.remove(at);
            if eligible.contains(&clip) {
                excluded[clip] = true;
                return Some(clip);
            }
        }
        excluded[open[pick]] = true;
        Some(open[pick])
    }

    /// Stops being busy now (`Shut`): nothing queued, and what plays let go
    /// of at the next update.
    pub fn shut(&mut self, now: f32) {
        self.queued = None;
        self.release_time = now;
    }
}
