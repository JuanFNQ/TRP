//! Adding a stimulator and running its buffs through their stages.

use super::*;

impl Health {
    /// A buff's own copy (`GetPersonalBuffSettings`): a side effect's
    /// duration cut by Metabolism and its value by Immunity, then every
    /// duration times the share of the item used. The delay stays.
    pub(super) fn personal(&self, def: &BuffDef, strength: f32) -> BuffDef {
        let mut p = def.clone();
        if !def.kind.is_buff(def.value) {
            p.duration *= 1.0 - self.skills.metabolism_debuff;
            p.value *= 1.0 - self.skills.immunity_misc;
        }
        p.duration *= strength;
        p
    }

    /// The item's set put on the head as a stimulator (`MedEffect.Residue`,
    /// `AddStackableEffect<Stimulator>`), at the share of the item used.
    /// The same item's again while its stimulator is still there renews
    /// its buffs instead (`UpdateWithSameOne`, `Buff.IncreaseBySameBuff`):
    /// one at work gets the new share's duration and what it had left, up
    /// to its whole; one done waits its delay again from now and rolls
    /// again; one waiting is left.
    pub fn add_stimulator(&mut self, template: &str, set: &str, strength: f32) {
        let Some(defs) = self.settings.stimulator.sets.get(set).cloned() else { return };
        if !self.alive {
            return;
        }
        let same = self.effects.iter().position(|e| e.kind == Kind::Stimulator && e.existing() && e.stim.as_ref().is_some_and(|s| s.template == template));
        if let Some(i) = same {
            let debuff_cut = 1.0 - self.skills.metabolism_debuff;
            let Some(stim) = self.effects[i].stim.as_mut() else { return };
            let now = stim.clock;
            for b in &mut stim.buffs {
                let whole = if b.original.kind.is_buff(b.original.value) { b.original.duration } else { b.original.duration * debuff_cut };
                let fresh = whole * strength;
                if b.at_work {
                    let left = (b.personal.delay + b.personal.duration - now).clamp(0.0, b.personal.duration);
                    b.personal.delay = now;
                    b.personal.duration = (fresh + left).min(whole);
                } else if b.done {
                    b.personal.duration = fresh.min(whole);
                    b.personal.delay = now + b.original.delay;
                    b.done = false;
                }
            }
            return;
        }
        let buffs = defs.iter().map(|d| BuffRun { original: d.clone(), personal: self.personal(d, strength), at_work: false, done: false, value: 0.0, made: None }).collect();
        let work = StimWork { set: set.to_owned(), template: template.to_owned(), buffs, clock: 0.0, loop_timer: 0.0 };
        self.add_with(Kind::Stimulator, Part::Head, Some(0.0), None, None, Some(strength), move |e| e.stim = Some(Box::new(work)));
    }

    /// Started (at once): every buff of no delay goes to work.
    pub(crate) fn stim_started(&mut self, id: u32) {
        let count = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).map_or(0, |s| s.buffs.len());
        for b in 0..count {
            let pending = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| !s.buffs[b].at_work && !s.buffs[b].done && s.buffs[b].personal.delay <= 0.0);
            if pending {
                self.activate(id, b, true);
            }
        }
    }

    /// Each frame while started (`Stimulator.RegularUpdate`): the clock
    /// runs; buffs whose delay is up go to work, those whose time is up are
    /// undone; every loop of the set's time the rates act once; with every
    /// buff done the stimulator ends.
    pub(crate) fn stim_update(&mut self, id: u32, dt: f32) {
        let loop_time = self.settings.stimulator.loop_time;
        let Some(i) = self.index_of(id) else { return };
        let Some(stim) = self.effects[i].stim.as_mut() else { return };
        stim.clock += dt;
        stim.loop_timer += dt;
        let now = stim.clock;
        let count = stim.buffs.len();
        for b in 0..count {
            let due = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| !s.buffs[b].at_work && !s.buffs[b].done && now >= s.buffs[b].personal.delay);
            if due {
                self.activate(id, b, true);
            }
        }
        for b in 0..count {
            let over = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| s.buffs[b].at_work && now >= s.buffs[b].personal.delay + s.buffs[b].personal.duration);
            if over {
                self.activate(id, b, false);
            }
        }
        let looped = self.index_of(id).and_then(|i| self.effects[i].stim.as_mut()).is_some_and(|s| {
            let due = s.loop_timer >= loop_time;
            if due {
                s.loop_timer -= loop_time;
            }
            due
        });
        if looped {
            for b in 0..count {
                let rate = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).filter(|s| s.buffs[b].at_work).map(|s| (s.buffs[b].personal.kind, s.buffs[b].value));
                if let Some((kind, value)) = rate {
                    self.process(kind, value * loop_time);
                }
            }
        }
        let all_done = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| s.buffs.iter().all(|b| b.done));
        if all_done {
            self.force_residue(id);
        }
    }

    /// Residual (its end): every buff still at work undone.
    pub(crate) fn stim_residue(&mut self, id: u32) {
        let count = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).map_or(0, |s| s.buffs.len());
        for b in 0..count {
            let working = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).is_some_and(|s| s.buffs[b].at_work);
            if working {
                self.activate(id, b, false);
            }
        }
    }

    /// A buff to work, or undone (`ActivateBuff`): to work, its chance rolled
    /// (`Random.value >= Chance` and it is done, nothing happening), a side
    /// effect dodged by Immunity's elite (0.9; its notice put on with the
    /// roll, `immunity_roll`), then its action; undone, its action taken
    /// back.
    fn activate(&mut self, id: u32, b: usize, on: bool) {
        let Some(buff) = self.index_of(id).and_then(|i| self.effects[i].stim.as_ref()).map(|s| s.buffs[b].clone()) else { return };
        let p = &buff.personal;
        if on {
            let failed = self.rng.unit() >= p.chance;
            let dodged = !failed && !p.kind.is_buff(p.value) && self.immunity_roll(self.skills.immunity_avoid);
            if failed || dodged {
                self.set_buff(id, b, |r| r.done = true);
                return;
            }
            let value = self.evaluate(p);
            let made = self.act(p, value);
            self.set_buff(id, b, |r| {
                r.at_work = true;
                r.value = value;
                r.made = made;
            });
            if p.kind.once() {
                self.set_buff(id, b, |r| {
                    r.at_work = false;
                    r.done = true;
                });
            }
        } else {
            self.set_buff(id, b, |r| {
                r.at_work = false;
                r.done = true;
            });
            if let Some(made) = buff.made {
                self.force_residue(made);
            }
        }
    }

    fn set_buff(&mut self, id: u32, b: usize, change: impl FnOnce(&mut BuffRun)) {
        if let Some(r) = self.index_of(id).and_then(|i| self.effects[i].stim.as_mut()).and_then(|s| s.buffs.get_mut(b)) {
            change(r);
        }
    }

    /// The value a buff acts by (`Stimulator.EvaluateValue`): its own where
    /// absolute, else one more than it times what it is reckoned against
    /// (the head's most, energy's, hydration's; the stamina's base
    /// capacity and refill, 115 and 4.5). Only the rates, the skills and
    /// the stamina are reckoned so; the rest take the value as it is.
    /// (None of the install's items has a relative one of these.)
    fn evaluate(&self, p: &BuffDef) -> f32 {
        let reference = match p.kind {
            BuffKind::HealthRate => self.part(Part::Head).maximum,
            BuffKind::EnergyRate => self.energy[2],
            BuffKind::HydrationRate => self.hydration[2],
            BuffKind::MaxStamina => STAMINA_CAPACITY,
            BuffKind::StaminaRate => STAMINA_REFILL,
            // A skill's is reckoned against its level by the host (`skill_boosts`).
            _ => return p.value,
        };
        if p.absolute { p.value } else { (p.value + 1.0) * reference }
    }

    /// What a buff does as it goes to work (the cases of `ActivateBuff`), and
    /// the effect it made, which its end takes away.
    fn act(&mut self, p: &BuffDef, value: f32) -> Option<u32> {
        let allowed = |part: Part| p.applies_to.is_empty() || p.applies_to.contains(&part);
        match p.kind {
            BuffKind::WeightLimit => Some(self.add(Kind::Endurance, Part::Head, Some(0.0), None, None, Some(p.value))),
            BuffKind::DamageModifier => Some(self.add(Kind::DamageModifier, Part::Head, Some(0.0), None, None, Some(p.value.max(-0.99)))),
            BuffKind::Pain => Some(self.add(Kind::Pain, Part::Head, Some(0.0), None, None, Some(1.0))),
            BuffKind::HandsTremor if p.value >= 0.0 => Some(self.add(Kind::Tremor, Part::Head, None, None, None, None)),
            BuffKind::HandsTremor => {
                if let Some(t) = self.active_id(Kind::Tremor, Some(Part::Head)) {
                    self.force_residue(t);
                }
                None
            }
            BuffKind::QuantumTunnelling if p.value >= 0.0 => Some(self.add(Kind::TunnelVision, Part::Head, None, None, None, None)),
            BuffKind::QuantumTunnelling => {
                if let Some(t) = self.existing_id(Kind::TunnelVision, Some(Part::Head)) {
                    self.force_residue(t);
                }
                None
            }
            BuffKind::RemoveAllBloodLosses => {
                let bleedings: Vec<u32> = self.effects.iter().filter(|e| e.kind.bleeding() && e.existing()).map(|e| e.id).collect();
                for bleeding in bleedings {
                    self.force_residue(bleeding);
                }
                None
            }
            BuffKind::Contusion if allowed(Part::Head) => {
                self.do_contusion(p.duration, p.value);
                None
            }
            BuffKind::LightBleeding | BuffKind::HeavyBleeding if allowed(Part::Head) => {
                self.do_bleed(p.kind == BuffKind::HeavyBleeding, Part::Head);
                None
            }
            // An item's stimulator is on the head, which does not break.
            BuffKind::Fracture => None,
            // A poisoning, once (health spec, "An intoxication"); what it
            // makes outlives the stimulator.
            BuffKind::UnknownToxin => {
                self.do_intoxication();
                None
            }
            // An intoxication at work taken off; while the buff works, none
            // can begin (`antidote_at_work`).
            BuffKind::Antidote => {
                self.remove_intoxication();
                None
            }
            // The rates act each loop; the skills and the stamina are read
            // off the buffs at work (`skill_boosts`, `stamina_buffs`).
            // The lethal toxin (no set in the install names one), the
            // body's temperature, the stomach's bleeding, the contusion's
            // own screens and the cleansing ones are not built (nothing on
            // the range causes or reads them).
            _ => {
                let _ = value;
                None
            }
        }
    }

    /// A rate's loop (`ProcessBuff`): health to one part a loop (the parts in
    /// a random order, the first neither full nor destroyed healed, or the
    /// first not destroyed hurt, straight, and destroyed at nought), energy,
    /// hydration.
    fn process(&mut self, kind: BuffKind, amount: f32) {
        match kind {
            BuffKind::HealthRate => {
                let mut order = Part::ALL;
                for i in (1..order.len()).rev() {
                    let j = (self.rng.unit() * (i + 1) as f32) as usize;
                    order.swap(i, j.min(i));
                }
                if amount > 0.0 {
                    if let Some(part) = order.into_iter().find(|p| {
                        let s = self.part(*p);
                        !s.destroyed && s.current < s.maximum
                    }) {
                        self.change_health(part, amount);
                    }
                } else if amount < 0.0 {
                    if let Some(part) = order.into_iter().find(|p| !self.part(*p).destroyed) {
                        if self.part(part).current > 0.0 {
                            self.change_health(part, amount);
                        }
                        if self.part(part).current <= 0.0 {
                            self.destroy(part, DamageKind::Stimulator);
                        }
                    }
                }
            }
            BuffKind::EnergyRate => self.change_energy(amount),
            BuffKind::HydrationRate => self.change_hydration(amount),
            _ => {}
        }
    }

    /// The buffs at work, of every stimulator.
    pub(super) fn at_work(&self) -> impl Iterator<Item = &BuffRun> {
        self.effects.iter().filter(|e| e.kind == Kind::Stimulator).filter_map(|e| e.stim.as_ref()).flat_map(|s| s.buffs.iter()).filter(|b| b.at_work)
    }
}
