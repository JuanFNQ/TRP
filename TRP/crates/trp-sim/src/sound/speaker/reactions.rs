//! The update and what a speaker reacts to: damage, effects, death, breath and shields.

use super::*;

impl Speaker {
    /// Each frame (`ManualUpdate`): no longer busy, a phrase queued is said
    /// once its time has come (and nothing it chains to), and until then
    /// nothing is let go of; with none queued, the phrase is over and let
    /// go of (`OnSpeakerRelease`): the breathing asked again, and what
    /// plays stopped unless the breathing goes on.
    pub fn update(&mut self, now: f32, s: &Situation) {
        if self.busy(now) {
            return;
        }
        if let Some(queued) = self.queued {
            if queued.at < now {
                self.play(queued.trigger, queued.tags, None, now);
                self.queued = None;
            }
            return;
        }
        if !self.speaking {
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
    /// anything else, a grunt. A bot's fall is any damage (the fall's own
    /// branch is the player's: `!IsAI`).
    pub fn damaged(&mut self, kind: DamageKind, damage: f32, now: f32, s: &Situation) {
        if !self.ai && kind == DamageKind::Fall && damage > MIN_FALL_DAMAGE {
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
    /// is said); a bot's none.
    pub fn hit(&mut self, part: Part, damage: f32, kind: DamageKind, now: f32, s: &Situation) {
        // An enemy's (a weapon's, an explosion's, a fragment's) makes the
        // character aware for 20 s.
        if kind.enemy() {
            self.aware(20.0, now);
        }
        self.last_hit = part;
        if !self.ai && s.alive && (!s.on_painkillers || damage > PAINKILLER_DAMAGE) {
            self.play(trigger::BEING_HURT, s.status.tag(), None, now);
        }
    }

    /// A hit reached a part, before its damage is taken (`ApplyDamageInfo`
    /// sets `LastDamagedBodyPart` first): what a death on that damage goes
    /// by.
    pub fn struck(&mut self, part: Part) {
        self.last_hit = part;
    }

    /// `Player.Say(OnBeingHurt)`: the health's tag and the awareness's.
    fn hurt(&mut self, now: f32, s: &Situation) {
        self.play(trigger::BEING_HURT, s.status.tag() | self.awareness(false, now), None, now);
    }

    /// A berserk started (`Player.OnHealthEffectAdded`: `Say(OnFight,
    /// demand: true)`): the fight's phrase at its bank's importance, with
    /// the health's tag and the awareness's. Its bank's chain is never
    /// queued (the player's speaker queues only what is asked for).
    pub fn berserk(&mut self, now: f32, s: &Situation) {
        self.play(trigger::FIGHT, s.status.tag() | self.awareness(false, now), None, now);
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

    /// Another character's face shield changed, seen from outside: muffled
    /// or not, and nothing else (the breathing asked and the silence are
    /// the first person's, `FirstPersonPointOfView`). Muffled, the breath
    /// is needed the next time it is asked.
    pub fn face_shield_seen(&mut self, down: bool, jaw: bool) {
        self.muffled = down && jaw;
    }
}

/// Whether what is worn in a slot muffles the voice
/// (`SetupSoundsFromFaceShieldComponent`): its face shield down (or one
/// with no switch), and armour on the jaw in it or anything fitted to it
/// (`GetItemComponentsInChildren<CompositeArmorComponent>`).
pub fn muffles(db: &trp_data::Database, equipment: &crate::items::inventory::Equipment, slot: &str) -> bool {
    equipment.worn(slot).is_some_and(|worn| shield_of(db, worn).1)
}

/// What is worn in the headwear's or the face cover's slot as the voice
/// takes it (`SetupSoundsFromFaceShieldComponent`): whether it has a face
/// shield down (or one with no switch), and whether it muffles (down, over
/// armour on the jaw in it or anything fitted to it). For any character's
/// own things (a dummy's body has no `Equipment` of its own).
pub fn shield_of(db: &trp_data::Database, worn: &crate::items::inventory::Item) -> (bool, bool) {
    let Some(shield) = crate::items::inventory::face_shield(worn) else { return (false, false) };
    let down = !crate::items::inventory::has_switch(shield) || shield.on;
    let jaw = crate::condition::armor::collider_type("Jaw").map_or(0, |k| 1u32 << k);
    (down, down && crate::items::inventory::with_parts(worn).any(|part| crate::condition::armor::Piece::of(db, &part.kind.template, 0, None).is_some_and(|piece| piece.colliders & jaw != 0)))
}
