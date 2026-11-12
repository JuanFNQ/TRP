//! Adding effects and running them through their stages.

use super::*;

impl Health {
    /// Adds an effect as `AddEffect` does, or lengthens the one there.
    pub(super) fn add(&mut self, kind: Kind, part: Part, delay: Option<f32>, work: Option<f32>, residue: Option<f32>, strength: Option<f32>) -> u32 {
        self.add_with(kind, part, delay, work, residue, strength, |_| {})
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_with(&mut self, kind: Kind, part: Part, delay: Option<f32>, work: Option<f32>, residue: Option<f32>, strength: Option<f32>, init: impl FnOnce(&mut Effect)) -> u32 {
        if !kind.stacks() && !kind.stacks_by_item() {
            if let Some(id) = self.existing_id(kind, Some(part)) {
                self.add_work_time(id, work, false);
                if let Some(s) = strength.filter(|s| *s > self.effects[self.index_of(id).unwrap()].strength) {
                    self.set_strength(id, s);
                }
                return id;
            }
        }
        let residual: Vec<u32> = self.effects.iter().filter(|e| e.kind == kind && e.part == part && e.stage == Stage::Residual).map(|e| e.id).collect();
        for id in residual {
            self.force_remove(id);
        }
        let [default_delay, default_work, default_residue, default_build] = self.defaults(kind);
        let (mut work, mut residue, mut strength) = (work, residue, strength.unwrap_or(1.0));
        if kind == Kind::Pain {
            // `Pain.Init`: cut by the Stress Resistance skill's pain buff.
            let cut = (1.0 - self.skills.stress_pain).max(0.01);
            work = work.map(|w| if w.is_finite() { w * cut } else { w });
            residue = residue.map(|r| r * cut);
            strength *= cut;
        }
        self.next_id += 1;
        let delay = delay.unwrap_or(default_delay);
        let work = work.unwrap_or(default_work);
        let mut effect = Effect {
            id: self.next_id,
            kind,
            part,
            stage: Stage::None,
            strength,
            current_strength: 0.0,
            delay,
            build_up: default_build.min(work),
            work,
            residue: residue.unwrap_or(default_residue),
            state_time: delay,
            state_current: 0.0,
            whole: 0.0,
            forced: false,
            side: None,
            loops: [0.0; 2],
            figures: [0.0; 4],
            item: None,
            tremor_after: None,
            med: None,
            stim: None,
        };
        init(&mut effect);
        let id = effect.id;
        self.effects.push(effect);
        self.next_state(id);
        id
    }

    /// A painkiller or the like from an item: a second from the same item
    /// lengthens the first (`AddStackableEffect`, `UpdateWithSameOne`).
    pub(super) fn add_by_item(&mut self, kind: Kind, part: Part, item: &str, strength: f32, delay: f32, work: f32, residue: f32, full_duration: f32) -> u32 {
        if let Some(i) = self.effects.iter().position(|e| e.kind == kind && e.part == part && e.existing() && e.item.as_deref() == Some(item)) {
            let id = self.effects[i].id;
            self.effects[i].figures[0] = full_duration;
            let left = self.effects[i].time_left();
            let longer = (left + full_duration * strength).clamp(0.0, full_duration);
            self.add_work_time(id, Some(longer), true);
            return id;
        }
        let item = item.to_owned();
        self.add_with(kind, part, Some(delay), Some(work), Some(residue), Some(strength), move |e| {
            e.item = Some(item);
            e.figures[0] = full_duration;
        })
    }

    /// `AddSideEffect`: made for another, and ended with it, unless the
    /// part had one already.
    pub(super) fn add_side(&mut self, of: u32, kind: Kind, part: Part, delay: Option<f32>, strength: Option<f32>) {
        let before = self.existing_id(kind, Some(part));
        let side = self.add(kind, part, delay, None, None, strength);
        if let Some(i) = self.index_of(of) {
            self.effects[i].side = (Some(side) != before).then_some(side);
        }
    }

    fn remove_side(&mut self, side: Option<u32>) {
        let Some(id) = side else { return };
        let Some(i) = self.index_of(id) else { return };
        match self.effects[i].stage {
            Stage::Started => self.add_work_time(id, Some(0.0), true),
            Stage::Added => self.force_remove(id),
            _ => {}
        }
    }

    fn add_work_time(&mut self, id: u32, time: Option<f32>, reset: bool) {
        let Some(i) = self.index_of(id) else { return };
        let default_work = self.defaults(self.effects[i].kind)[1];
        let default_build = self.defaults(self.effects[i].kind)[3];
        let e = &mut self.effects[i];
        if e.stage > Stage::Started {
            return;
        }
        let time = time.unwrap_or(default_work);
        if e.stage >= Stage::Started && time.is_finite() {
            e.work = if reset { e.state_current } else { e.work } + time;
            e.build_up = (e.state_current + default_build).min(e.work);
        } else {
            e.work = time.max(e.work);
        }
        e.calculate_strength(0.0);
        if e.stage == Stage::Started && (e.state_time - e.work).abs() > f32::EPSILON {
            e.state_time = e.work;
        }
    }

    fn set_residue_time(&mut self, id: u32, time: f32) {
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        if e.stage < Stage::Residual {
            e.residue = time.min(e.residue);
        } else if e.state_current + time < e.residue {
            e.residue = e.state_current + time;
            e.state_time = e.residue;
            e.calculate_strength(0.0);
        }
    }

    fn set_strength(&mut self, id: u32, strength: f32) {
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        if (strength - e.strength).abs() > f32::EPSILON {
            e.strength = strength;
            e.calculate_strength(0.0);
        }
    }

    /// Moves an effect on to residual now (`ForceResidue`).
    pub fn force_residue(&mut self, id: u32) {
        let Some(i) = self.index_of(id) else { return };
        match self.effects[i].kind {
            // A forced residue lasts two seconds (`DEFAULT_FORCE_RESIDUE_TIME`).
            Kind::Pain | Kind::Contusion => self.set_residue_time(id, 2.0),
            Kind::MedEffect => {
                if let Some(med) = self.effects[i].med.as_mut() {
                    med.interrupted = true;
                }
            }
            _ => {}
        }
        let Some(i) = self.index_of(id) else { return };
        self.effects[i].forced = true;
        if self.effects[i].stage == Stage::Added {
            self.next_state(id);
        }
        if self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Started) {
            self.next_state(id);
        }
    }

    /// Takes an effect off now (`ForceRemove`).
    pub fn force_remove(&mut self, id: u32) {
        self.force_residue(id);
        if self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Residual) {
            self.next_state(id);
        }
    }


    pub(super) fn next_state(&mut self, id: u32) {
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        e.state_current = 0.0;
        match e.stage {
            Stage::None => {
                e.stage = Stage::Added;
                let (kind, part, delay) = (e.kind, e.part, e.delay);
                self.on_added(id, kind, part);
                if delay <= 0.0 {
                    self.next_state(id);
                }
            }
            Stage::Added => {
                e.stage = Stage::Started;
                e.state_time = e.work;
                let (kind, part, work) = (e.kind, e.part, e.work);
                self.on_started(id, kind, part);
                self.events.push(Event::Started(kind, part));
                if work.abs() < f32::EPSILON && self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Started) {
                    self.next_state(id);
                }
            }
            Stage::Started => {
                e.stage = Stage::Residual;
                e.state_time = e.residue;
                let (kind, part, residue) = (e.kind, e.part, e.residue);
                self.on_residue(id, kind, part);
                self.events.push(Event::Residual(kind, part));
                if residue.abs() < f32::EPSILON && self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Residual) {
                    self.next_state(id);
                }
            }
            Stage::Residual => {
                e.stage = Stage::Removed;
                e.state_time = 0.0;
                let removed = self.effects.remove(i);
                self.on_removed(&removed);
                self.events.push(Event::Removed(removed.kind, removed.part, removed.id));
                self.remove_side(removed.side);
            }
            Stage::Removed => {}
        }
    }

    fn on_added(&mut self, id: u32, kind: Kind, part: Part) {
        match kind {
            Kind::Wound => {
                let [least, most, _] = self.settings.wound;
                let threshold = self.rng.range(least, most);
                if let Some(i) = self.index_of(id) {
                    self.effects[i].figures = [threshold, 0.0, 0.0, 0.0];
                }
            }
            Kind::Dehydration => self.add_side(id, Kind::Pain, Part::Head, Some(10.0), None),
            _ => {}
        }
        let _ = part;
    }

    fn on_started(&mut self, id: u32, kind: Kind, part: Part) {
        match kind {
            Kind::LightBleeding | Kind::HeavyBleeding => {
                let dehydrated = self.active(Kind::Dehydration, None).is_some();
                self.reset_bleeding(id, dehydrated);
            }
            Kind::Dehydration => {
                let bleedings: Vec<u32> = self.effects.iter().filter(|e| e.kind.bleeding() && e.active()).map(|e| e.id).collect();
                for b in bleedings {
                    self.reset_bleeding(b, true);
                }
                let damage = self.starving(self.settings.dehydration_damage)[0];
                if let Some(i) = self.index_of(id) {
                    self.effects[i].figures[0] = damage;
                }
            }
            Kind::Fracture => self.add_side(id, Kind::Pain, part, Some(1.0), None),
            // With Stress Resistance's elite, a berserk on the head, made
            // for low health and cut short with it (`LowEdgeHealth.Started`).
            Kind::LowHealth if self.skills.stress_berserk => self.add_side(id, Kind::Berserk, Part::Head, None, Some(2.0)),
            Kind::Berserk => self.berserk_edges.push(true),
            // `Intoxication.Started`: its damage to each part a loop, cut by
            // Immunity's poison buff; its rate, that damage to the seven
            // parts each loop; a pain on its part after a second.
            Kind::Intoxication => {
                let [_, _, damage, every] = self.settings.intoxication;
                let damage = damage * (1.0 - self.skills.poison_cut);
                if let Some(i) = self.index_of(id) {
                    let e = &mut self.effects[i];
                    e.figures[0] = damage;
                    e.figures[1] = every;
                    e.figures[2] = -damage * Part::ALL.len() as f32 / every;
                }
                self.add_side(id, Kind::Pain, part, Some(1.0), None);
            }
            Kind::Pain => {
                if let Some(i) = self.index_of(id) {
                    self.effects[i].tremor_after = Some(0.0);
                }
            }
            Kind::ChronicFatigue => {
                // `ChronicStaminaFatigue.Started` sets its strength to 1
                // and its first tick a tick away.
                let tick = self.settings.fatigue[2];
                let clock = self.clock;
                if let Some(i) = self.index_of(id) {
                    self.effects[i].strength = 1.0;
                    self.effects[i].calculate_strength(0.0);
                    self.effects[i].figures[1] = clock + tick;
                }
            }
            Kind::MedEffect => self.med_started(id),
            Kind::Stimulator => self.stim_started(id),
            // The damage taken changed by its strength while it works,
            // kept to be undone (`DamageModifier.Started`).
            Kind::DamageModifier => {
                if let Some(i) = self.index_of(id) {
                    let s = self.effects[i].strength;
                    self.effects[i].figures[0] = s;
                    self.damage_multiplier *= 1.0 + s;
                }
            }
            _ => {}
        }
    }

    fn on_residue(&mut self, id: u32, kind: Kind, part: Part) {
        match kind {
            Kind::Dehydration => {
                let bleedings: Vec<u32> = self.effects.iter().filter(|e| e.kind.bleeding() && e.active()).map(|e| e.id).collect();
                for b in bleedings {
                    self.reset_bleeding(b, false);
                }
            }
            Kind::MedEffect => self.med_residue(id),
            Kind::Stimulator => self.stim_residue(id),
            Kind::DamageModifier => {
                if let Some(i) = self.index_of(id) {
                    self.damage_multiplier /= 1.0 + self.effects[i].figures[0];
                }
            }
            Kind::Berserk => self.berserk_edges.push(false),
            _ => {}
        }
        let _ = part;
    }

    fn on_removed(&mut self, effect: &Effect) {
        if effect.kind == Kind::HeavyBleeding {
            self.add(Kind::Wound, effect.part, None, None, None, None);
        }
    }

    /// A kind's default figures (`defaults`), a bleeding's working time
    /// the elite Vitality buff's where it has one (`Bleeding.
    /// DefaultWorkTime`: `EliteVitalityDuration`, if above 0).
    pub(super) fn defaults(&self, kind: Kind) -> [f32; 4] {
        let mut figures = defaults(kind, &self.settings);
        if kind.bleeding() && self.skills.bleed_stop {
            let s = if kind == Kind::HeavyBleeding { self.settings.heavy } else { self.settings.light };
            if s.elite_duration > 0.0 {
                figures[1] = s.elite_duration;
            }
        }
        figures
    }

    /// Strong dehydration's or exhaustion's damage and how often: no
    /// damage with the elite Metabolism buff (`Dehydration.Damage`,
    /// `Exhaustion.Damage`).
    pub(super) fn starving(&self, [damage, every]: [f32; 2]) -> [f32; 2] {
        [if self.skills.no_starvation_damage { 0.0 } else { damage }, every]
    }

    /// `Bleeding.ResetParameters`: the figures with or without
    /// dehydration.
    fn reset_bleeding(&mut self, id: u32, dehydrated: bool) {
        let Some(i) = self.index_of(id) else { return };
        let s = if self.effects[i].kind == Kind::HeavyBleeding { self.settings.heavy } else { self.settings.light };
        let default_work = self.defaults(self.effects[i].kind)[1];
        let life = if dehydrated && s.life_dehydrated > 0.0 { s.life_dehydrated.min(default_work) } else { default_work };
        self.add_work_time(id, Some(life), true);
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        e.figures[0] = if dehydrated { s.damage_dehydrated } else { s.damage };
        e.figures[1] = if dehydrated { s.loop_dehydrated } else { s.loop_time };
        // Its rate of health, a second (`SetHealthRatesPerSecond`): its
        // damage to each of the seven parts each loop.
        e.figures[2] = -e.figures[0] * Part::ALL.len() as f32 / e.figures[1];
        if e.loops[0] > e.figures[1] {
            e.loops[0] = e.figures[1];
        }
        if e.loops[1] > s.energy_loop {
            e.loops[1] = s.energy_loop;
        }
    }

    pub(super) fn fatigue_rate(&self, id: u32) -> f32 {
        let strength = self.index_of(id).map_or(1.0, |i| self.effects[i].strength);
        self.settings.fatigue[0] + strength * self.settings.fatigue[1]
    }
}
