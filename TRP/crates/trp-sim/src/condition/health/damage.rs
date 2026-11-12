//! Damage and its effects, falls, exertion and the barbed wire tally.

use super::*;

impl Health {
    /// Damage to a part (`ApplyDamage`); returns how much common health
    /// was lost. `bias` is what a round adds to the odds of a bleeding
    /// (`HeavyBleedingDelta`, `LightBleedingDelta`); 0 for anything else.
    pub fn apply_damage(&mut self, part: Part, damage: f32, kind: DamageKind) -> f32 {
        self.apply_damage_biased(part, damage, kind, [0.0, 0.0])
    }

    pub fn apply_damage_biased(&mut self, part: Part, damage: f32, kind: DamageKind, bias: [f32; 2]) -> f32 {
        self.apply_damage_info(part, damage, kind, bias, false)
    }

    /// A character taken up as it stands (a replay's victim): each part's
    /// health now and at most, a part at nought destroyed.
    pub fn take_up_parts(&mut self, current: [f32; 7], maximum: [f32; 7]) {
        for k in 0..7 {
            self.parts[k] = PartState { current: current[k], maximum: maximum[k], destroyed: current[k] <= 0.0 };
        }
    }

    /// `ApplyDamage` with the hit's figures (`DamageInfo`): the round's
    /// bleedings' deltas, and whether light armour blocks a bleeding
    /// (`BleedBlock`, body hits spec).
    pub fn apply_damage_info(&mut self, part: Part, damage: f32, kind: DamageKind, bias: [f32; 2], bleed_block: bool) -> f32 {
        if !self.alive {
            return 0.0;
        }
        let mut damage = damage;
        if self.damage_multiplier > 1.0 || part != Part::Head || !kind.enemy() {
            damage *= self.damage_multiplier;
        }
        if kind.environmental() {
            damage *= self.settings.environment[part.index()];
        }
        let before = self.parts[part.index()].current;
        let common_before = self.common()[0];
        self.change_health(part, -damage);
        self.events.push(Event::Damaged(part, damage, kind));
        let state = self.parts[part.index()];
        if !state.destroyed && state.current <= 0.0 {
            self.destroy(part, kind);
        }
        if self.parts[part.index()].destroyed {
            self.kill_after_destroyed(part, kind);
        }
        if !kind.self_inflicted() {
            let over = (damage - before).max(0.0);
            if over > 0.0 {
                let others: f32 = Part::ALL.iter().filter(|p| **p != part && !self.part(**p).destroyed).map(|p| self.part(*p).maximum).sum();
                let spread = over * self.settings.over_damage_factor[part.index()];
                for other in Part::ALL {
                    let o = self.parts[other.index()];
                    if other != part && !o.destroyed && others > 0.0 {
                        let share = (-spread * o.maximum / others * self.settings.over_damage_received[other.index()]).min(0.0);
                        self.change_health(other, share);
                        if self.parts[other.index()].current <= 0.0 {
                            self.destroy(other, kind);
                        }
                    }
                }
            }
            let maximum = self.parts[part.index()].maximum;
            if damage >= 1.0 && kind != DamageKind::Barbed {
                self.do_pain(part, 0.0, 0.0, (15.0 * damage / maximum).clamp(1.0, 10.0), (damage / maximum * 2.0).clamp(0.33, 2.0));
            }
            let x = damage / maximum;
            // A stimulator at work may stop blood loss; light armour may
            // block it. The odds' top raised by the Vitality skill's buff.
            let top = 1.0 + self.skills.bleed_chance;
            if bleed_block || self.blood_loss_blocked() {
            } else if (kind.weapon() || kind == DamageKind::GrenadeFragment) && self.chance(Chance::HeavyBleeding, part, self.settings.heavy.odds, x, top, bias[0]) {
                self.do_bleed(true, part);
            } else if self.existing(Kind::HeavyBleeding, Some(part)).is_none() && self.chance(Chance::LightBleeding, part, self.settings.light.odds, x, top, bias[1]) {
                self.do_bleed(false, part);
            }
            let odds = if kind == DamageKind::Fall { self.settings.fracture_falling } else { self.settings.fracture_hit };
            if self.chance(Chance::Fracture, part, odds, x, 1.0 + self.skills.break_chance, 0.0) {
                self.do_fracture(part);
            }
        }
        let common = self.common()[0];
        if common <= 0.0 {
            self.kill(kind);
        } else if common <= self.settings.low_health_common {
            self.add(Kind::LowHealth, Part::Head, None, None, None, None);
        }
        common_before - self.common()[0]
    }

    fn do_pain(&mut self, part: Part, delay: f32, work: f32, residue: f32, strength: f32) {
        if self.alive {
            self.add(Kind::Pain, part, Some(delay), Some(work), Some(residue), Some(strength));
        }
    }

    /// `DoBleed`: one bleeding a part, the heavier replacing the lighter.
    pub fn do_bleed(&mut self, heavy: bool, part: Part) {
        let kind = if heavy { Kind::HeavyBleeding } else { Kind::LightBleeding };
        if !self.alive || self.existing(kind, Some(part)).is_some() {
            return;
        }
        for other in [Kind::LightBleeding, Kind::HeavyBleeding, Kind::Wound] {
            if let Some(id) = self.existing_id(other, Some(part)) {
                self.force_remove(id);
            }
        }
        self.add(kind, part, None, None, None, None);
    }

    /// `DoContusion`: a contusion on the head for so many seconds, from
    /// 0.1 s on, fading for half of them (health spec, "A contusion").
    pub fn do_contusion(&mut self, time: f32, strength: f32) {
        if self.alive {
            self.add(Kind::Contusion, Part::Head, Some(0.1), Some(time), Some(time / 2.0), Some(strength));
        }
    }

    /// `DoIntoxication` (health spec, "An intoxication"): alive, none while
    /// an antidote's buff is at work, none when Immunity's elite keeps it
    /// off (`ImmunityAvoidPoisonChance`, 0.9); else an intoxication on the
    /// stomach, or the one there kept. What the game's poisoned blade and a
    /// level's trigger call; a host's too.
    pub fn do_intoxication(&mut self) -> Option<u32> {
        if !self.alive || self.antidote_at_work() {
            return None;
        }
        if self.immunity_roll(self.skills.poison_avoid) {
            return None;
        }
        Some(self.add(Kind::Intoxication, Part::Stomach, None, None, None, None))
    }

    /// `RemoveIntoxication`: an intoxication at work on the stomach to its
    /// residue (one still waiting its delay is left).
    pub fn remove_intoxication(&mut self) {
        if let Some(id) = self.active_id(Kind::Intoxication, Some(Part::Stomach)) {
            self.force_residue(id);
        }
    }

    /// One of Immunity's elite chances rolled (`ProbabilityBuff.Roll`):
    /// nothing below elite; at elite, whether the ill effect is kept off,
    /// and the notice of it put on the chest whichever way the roll went
    /// (`Player.OnImmunityPreventedEvent` reads no result).
    pub(crate) fn immunity_roll(&mut self, chance: f32) -> bool {
        if chance <= 0.0 {
            return false;
        }
        let kept_off = self.rng.unit() <= chance;
        self.add(Kind::ImmunityPrevented, Part::Chest, None, None, None, None);
        kept_off
    }

    /// `BluntContusion` (after a hit's damage, `Player.ApplyDamageInfo`):
    /// a hit to the head whose armour took 10 or more off it concusses,
    /// alive: a contusion on the head after 0.1 s, at half strength,
    /// working a quarter of what the armour took and fading over as long
    /// (health spec, "A blunt hit's concussion").
    pub fn blunt_contusion(&mut self, part: Part, absorbed: f32) {
        if self.alive && part == Part::Head && absorbed >= 10.0 {
            let quarter = absorbed / 4.0;
            self.add(Kind::Contusion, Part::Head, Some(0.1), Some(quarter), Some(quarter), Some(0.5));
        }
    }

    /// `DoStun`: a stun on the head at once (grenade spec, "A flash"),
    /// working its build-up and the flash's time by how hard it struck and
    /// the eyes' cover (`BlindnessDuration`), at that strength.
    pub fn do_stun(&mut self, max_time: f32, strength: f32, blindness: f32) {
        if self.alive {
            self.add(Kind::Stun, Part::Head, Some(0.0), Some(0.2 + max_time * strength * blindness), None, Some(strength));
        }
    }

    /// `DoBurnEyes`: the eyes burnt, a `Flash` on the head at once working
    /// its build-up and the flash's time by the distance's share and the
    /// eyes' cover twice, at the strength the flash struck with. Whether it
    /// was added (the camera then told, `BurnEyesEvent`), and the distance's
    /// share as the camera is told it.
    pub fn do_burn_eyes(&mut self, distance_strength: f32, strength: f32, max_time: f32, blindness: f32) -> Option<f32> {
        if !self.alive {
            return None;
        }
        let distance_strength = distance_strength * blindness;
        self.add(Kind::Flash, Part::Head, Some(0.0), Some(0.2 + max_time * distance_strength * blindness), None, Some(strength));
        Some(distance_strength)
    }

    /// `DoFracture`: an arm or a leg, not destroyed, breaks at once.
    pub fn do_fracture(&mut self, part: Part) {
        if self.alive && part.limb() && !self.part(part).destroyed {
            self.add(Kind::Fracture, part, Some(0.0), None, None, None);
        }
    }

    /// A chance drawn (`ProbabilitySetting.Try`): nothing under its
    /// threshold; else an outcome laid down for it (`outcomes`) if there is
    /// one, else the draw.
    fn chance(&mut self, chance: Chance, part: Part, odds: Odds, x: f32, top: f32, bias: f32) -> bool {
        if x < odds.threshold {
            return false;
        }
        if let Some(at) = self.outcomes.iter().position(|(c, p, _)| *c == chance && *p == part) {
            return self.outcomes.remove(at).2;
        }
        odds.try_at(x, top, bias, &mut self.rng)
    }

    /// A fall (`HandleFall`): the height fallen since last on the ground,
    /// and the weight's multiplier (`Physical.FallDamageMultiplier`).
    /// Returns the damage done to each leg.
    pub fn handle_fall(&mut self, height: f32, weight_multiplier: f32) -> f32 {
        if !self.alive {
            return 0.0;
        }
        let over = height - self.settings.fall_safe_height;
        if over <= 0.0 {
            return 0.0;
        }
        let damage = over * over.sqrt() * self.settings.fall_damage_per_meter * weight_multiplier;
        self.apply_damage(Part::LeftLeg, damage, DamageKind::Fall);
        self.apply_damage(Part::RightLeg, damage, DamageKind::Fall);
        damage
    }

    /// A wound's tally pushed (`DoWoundRelapse`): on one part's wounds, or
    /// on every wound.
    pub fn wound_relapse(&mut self, value: f32, part: Option<Part>) {
        if !self.alive {
            return;
        }
        let wounds: Vec<u32> = self.effects.iter().filter(|e| e.kind == Kind::Wound && e.active() && part.is_none_or(|p| e.part == p)).map(|e| e.id).collect();
        for id in wounds {
            let Some(i) = self.index_of(id) else { continue };
            let wound_part = self.effects[i].part;
            self.effects[i].figures[1] += value;
            let [threshold, tally, ..] = self.effects[i].figures;
            if tally > threshold && !self.blood_loss_blocked() && self.existing(Kind::LightBleeding, Some(wound_part)).is_none() && self.existing(Kind::HeavyBleeding, Some(wound_part)).is_none() {
                self.do_bleed(false, wound_part);
            }
        }
    }

    /// The stamina's fatigue (`AddFatigue`): one more stack.
    pub fn add_fatigue(&mut self) {
        let strength = self.active(Kind::ChronicFatigue, Some(Part::Chest)).map_or(1.0, |e| e.strength + 1.0);
        let work = self.settings.fatigue[3];
        self.add(Kind::ChronicFatigue, Part::Chest, None, Some(work), None, Some(strength));
    }

    /// The weight's two notices (`SetEncumbered`, `SetOverEncumbered`).
    pub fn set_encumbered(&mut self, encumbered: bool, over: bool) {
        for (on, slot, kind) in [(encumbered, 0, Kind::Encumbered), (over, 1, Kind::OverEncumbered)] {
            let held = if slot == 0 { self.encumbered } else { self.over_encumbered };
            let now = match (on, held) {
                (true, None) => Some(self.add(kind, Part::Chest, None, None, None, None)),
                (false, Some(id)) => {
                    self.force_remove(id);
                    None
                }
                (_, held) => held,
            };
            if slot == 0 { self.encumbered = now } else { self.over_encumbered = now }
        }
    }

    /// The sprint's self-damage on a damaged leg (`InflictSelfDamage`),
    /// once it has been started: each frame its count down runs, and in a
    /// sprint at 0 it hurts each damaged leg by 2 and counts again from 1 to
    /// 1.5 s. `sprinting` is whether the movement's state is the sprint.
    pub fn sprint_frame(&mut self, dt: f32, sprinting: bool, conditions: Conditions) {
        let Some(countdown) = self.self_damage.as_mut() else { return };
        *countdown -= dt;
        if *countdown <= 0.0 && sprinting {
            *countdown = self.rng.range(1.0, 1.5);
            if conditions.left_leg {
                self.apply_damage(Part::LeftLeg, 2.0, DamageKind::Fall);
            }
            if conditions.right_leg {
                self.apply_damage(Part::RightLeg, 2.0, DamageKind::Fall);
            }
        }
    }

    /// Starts the sprint's self-damage if it is not running
    /// (`StartInflictSelfDamageCoroutine`).
    pub fn start_self_damage(&mut self) {
        if self.alive && self.self_damage.is_none() {
            self.self_damage = Some(0.0);
        }
    }

    /// A jump landed (`MovementContextOnStateChanged`, leaving the jump):
    /// each damaged leg hurt by 3.
    pub fn jump_landed(&mut self, conditions: Conditions) {
        if conditions.left_leg {
            self.apply_damage(Part::LeftLeg, 3.0, DamageKind::Fall);
        }
        if conditions.right_leg {
            self.apply_damage(Part::RightLeg, 3.0, DamageKind::Fall);
        }
    }

    /// A vault begun (`ApplyDamageByVaulting`): the left leg, damaged, is
    /// hurt by 3 over a height above `without_hand`.
    pub fn vaulted(&mut self, height: f32, without_hand: f32, conditions: Conditions) {
        if conditions.left_leg && height >= without_hand {
            self.apply_damage(Part::LeftLeg, 3.0, DamageKind::Fall);
        }
    }
}

/// A body-part collider's tally in barbed wire (`BodyPartCollider.
/// ProceedBarb`): 2 a metre the collider moves, done in whole points.
#[derive(Debug, Clone, Copy, Default)]
pub struct BarbTally {
    pub(super) last: Option<[f32; 3]>,
    tally: f32,
}

impl BarbTally {
    /// The collider is in the wire at `at` this step: the damage due, if a
    /// whole point is.
    pub fn cut(&mut self, at: [f32; 3]) -> Option<f32> {
        let moved = self.last.map_or(f32::INFINITY, |l| ((at[0] - l[0]).powi(2) + (at[1] - l[1]).powi(2) + (at[2] - l[2]).powi(2)).sqrt());
        self.last = Some(at);
        if moved < 0.5 {
            self.tally += moved * 2.0;
            if self.tally >= 1.0 {
                let damage = self.tally;
                self.tally %= 1.0;
                return Some(damage);
            }
        } else {
            self.tally = 0.0;
        }
        None
    }

    /// The collider out of the wire: it keeps where it was, as the game's
    /// does, and its tally.
    pub fn moved_to(&mut self, at: [f32; 3]) {
        self.last = Some(at);
    }
}
