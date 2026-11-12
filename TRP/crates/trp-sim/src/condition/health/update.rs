//! The step: regular updates, health, energy, hydration and death.

use super::*;

impl Health {
    /// Steps every effect by the frame's time (`ManualUpdate`).
    pub fn step(&mut self, dt: f32) {
        self.clock += dt;
        let ids: Vec<u32> = self.effects.iter().map(|e| e.id).collect();
        for id in ids {
            let Some(i) = self.index_of(id) else { continue };
            if matches!(self.effects[i].stage, Stage::None | Stage::Removed) {
                continue;
            }
            self.update_effect(id, dt);
        }
    }

    fn update_effect(&mut self, id: u32, dt: f32) {
        let Some(i) = self.index_of(id) else { return };
        let e = &mut self.effects[i];
        let mut dt = dt;
        if e.state_current + dt >= e.state_time {
            dt = e.state_time - e.state_current;
            e.state_current = e.state_time;
        } else {
            e.state_current += dt;
        }
        e.whole += dt;
        match e.stage {
            Stage::Added => {
                if e.state_current >= e.state_time {
                    self.next_state(id);
                }
            }
            Stage::Started => {
                if dt > 0.0 {
                    self.regular_update(id, dt);
                }
                if self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Started && self.effects[i].state_current >= self.effects[i].state_time) {
                    self.next_state(id);
                }
            }
            Stage::Residual => {
                if self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Residual && self.effects[i].state_current >= self.effects[i].state_time) {
                    self.next_state(id);
                }
            }
            Stage::None | Stage::Removed => {}
        }
        if let Some(i) = self.index_of(id) {
            self.effects[i].calculate_strength(dt);
        }
    }

    fn regular_update(&mut self, id: u32, dt: f32) {
        let Some(i) = self.index_of(id) else { return };
        let (kind, part) = (self.effects[i].kind, self.effects[i].part);
        match kind {
            Kind::LightBleeding | Kind::HeavyBleeding => {
                let s = if kind == Kind::HeavyBleeding { self.settings.heavy } else { self.settings.light };
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                let (damage, every) = (e.figures[0], e.figures[1]);
                let bleed = e.loops[0] >= every;
                if bleed {
                    e.loops[0] -= every;
                }
                e.loops[1] += dt;
                let drain = e.loops[1] >= s.energy_loop;
                if drain {
                    e.loops[1] -= s.energy_loop;
                }
                // `BleedHealth`: nothing at all while a berserk is at work,
                // the loop going on.
                if bleed && self.active(Kind::Berserk, None).is_none() {
                    let as_kind = if kind == Kind::HeavyBleeding { DamageKind::HeavyBleeding } else { DamageKind::LightBleeding };
                    let mut taken = 0.0;
                    for p in Part::ALL {
                        taken += self.apply_damage(p, damage, as_kind);
                    }
                    // `BleedHealth`: its rate becomes what the loop took
                    // from the whole, where that differs.
                    let rate = -taken / every;
                    let multiplier = self.damage_multiplier;
                    if let Some(i) = self.index_of(id) {
                        if (self.effects[i].figures[2] - rate).abs() >= f32::EPSILON {
                            self.effects[i].figures[2] = rate / multiplier;
                        }
                    }
                }
                if drain {
                    self.change_energy(-s.energy_damage);
                    if self.energy[0] <= self.energy[1] {
                        self.add(Kind::Exhaustion, Part::Head, None, None, None, None);
                    }
                }
            }
            Kind::Exhaustion => {
                let [damage, every] = self.starving(self.settings.exhaustion_damage);
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                if e.loops[0] >= every {
                    e.loops[0] -= every;
                    for p in Part::ALL {
                        self.apply_damage(p, damage, DamageKind::Exhaustion);
                    }
                }
                if self.energy[0] > self.energy[1] {
                    self.force_residue(id);
                }
            }
            Kind::Dehydration => {
                let [damage, every] = self.starving(self.settings.dehydration_damage);
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                if e.loops[0] < every {
                    return;
                }
                if self.hydration[0] > 0.0 {
                    self.force_residue(id);
                    return;
                }
                if let Some(i) = self.index_of(id) {
                    self.effects[i].loops[0] -= every;
                }
                for p in Part::ALL {
                    self.apply_damage(p, damage, DamageKind::Dehydration);
                }
            }
            Kind::Existence => {
                let s = self.settings.existence;
                let stomach_gone = self.part(Part::Stomach).destroyed;
                let (energy_loop, hydration_loop) = if stomach_gone { (s[1] / s[4], s[3] / s[5]) } else { (s[1], s[3]) };
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                let eat = e.loops[0] >= energy_loop && !self.metabolism_disabled;
                if eat {
                    e.loops[0] -= energy_loop;
                }
                e.loops[1] += dt;
                let drink = e.loops[1] >= hydration_loop && !self.metabolism_disabled;
                if drink {
                    e.loops[1] -= hydration_loop;
                }
                // Cut by the skills' buffs (`HealthEnergy`, `HealthHydration`).
                let (energy_cut, hydration_cut) = (1.0 - self.skills.health_energy, 1.0 - self.skills.health_hydration);
                if eat {
                    self.change_energy(-s[0] * energy_cut);
                }
                if drink {
                    self.change_hydration(-s[2] * hydration_cut);
                }
            }
            Kind::Pain => {
                let delay = self.settings.pain_tremor_delay;
                let e = &mut self.effects[i];
                if let Some(t) = e.tremor_after.as_mut() {
                    *t += dt;
                    if *t >= delay {
                        e.tremor_after = None;
                        self.add_side(id, Kind::Tremor, Part::Head, None, None);
                    }
                }
            }
            Kind::LowHealth => {
                if self.common()[0] >= self.settings.low_health_common {
                    self.force_residue(id);
                }
            }
            Kind::ChronicFatigue => {
                let tick = self.settings.fatigue[2];
                if self.clock > self.effects[i].figures[1] {
                    let rate = self.fatigue_rate(id);
                    self.change_energy(-rate * tick);
                    if let Some(i) = self.index_of(id) {
                        self.effects[i].figures[1] = self.clock + tick;
                    }
                }
            }
            // `Intoxication.RegularUpdate`, `BleedHealth`: each loop its
            // damage to every part as poison (a berserk does not hold it
            // off); its rate then what the loop took from the whole, where
            // that differs.
            Kind::Intoxication => {
                let e = &mut self.effects[i];
                e.loops[0] += dt;
                let (damage, every) = (e.figures[0], e.figures[1]);
                if e.loops[0] >= every {
                    e.loops[0] -= every;
                    let mut taken = 0.0;
                    for p in Part::ALL {
                        taken += self.apply_damage(p, damage, DamageKind::Poison);
                    }
                    let rate = -taken / every;
                    let multiplier = self.damage_multiplier;
                    if let Some(i) = self.index_of(id) {
                        if (self.effects[i].figures[2] - rate).abs() >= f32::EPSILON {
                            self.effects[i].figures[2] = rate / multiplier;
                        }
                    }
                }
            }
            Kind::MedEffect => self.med_update(id, dt),
            Kind::Stimulator => self.stim_update(id, dt),
            Kind::Wound | Kind::Fracture | Kind::Tremor | Kind::PainKiller | Kind::Encumbered | Kind::OverEncumbered | Kind::Contusion | Kind::Stun | Kind::Flash | Kind::Endurance | Kind::DamageModifier | Kind::TunnelVision | Kind::Berserk | Kind::ImmunityPrevented => {}
        }
        let _ = part;
    }

    pub(super) fn change_health(&mut self, part: Part, value: f32) -> f32 {
        if !self.alive {
            return 0.0;
        }
        let p = &mut self.parts[part.index()];
        if p.destroyed {
            return 0.0;
        }
        let before = p.current;
        p.current = (p.current + value).clamp(0.0, p.maximum);
        let change = p.current - before;
        if change > 0.0 {
            self.events.push(Event::Healed(part, change));
        }
        change
    }

    /// `ChangeEnergy`.
    pub fn change_energy(&mut self, value: f32) {
        if !self.alive {
            return;
        }
        let before = self.energy[0];
        self.energy[0] = (self.energy[0] + value).clamp(self.energy[1], self.energy[2]);
        if self.energy[0] > before {
            self.events.push(Event::EnergyGained(self.energy[0] - before));
        }
        if self.energy[0] != before && self.energy[0] <= self.energy[1] && !self.metabolism_disabled {
            self.add(Kind::Exhaustion, Part::Head, None, None, None, None);
        }
    }

    /// `ChangeHydration`.
    pub fn change_hydration(&mut self, value: f32) {
        if !self.alive {
            return;
        }
        let before = self.hydration[0];
        self.hydration[0] = (self.hydration[0] + value).clamp(self.hydration[1], self.hydration[2]);
        if self.hydration[0] > before {
            self.events.push(Event::HydrationGained(self.hydration[0] - before));
        }
        if self.hydration[0] != before && self.hydration[0] <= self.hydration[1] && !self.metabolism_disabled {
            self.add(Kind::Dehydration, Part::Stomach, None, None, None, None);
        }
    }

    pub(super) fn kill(&mut self, kind: DamageKind) {
        if self.alive {
            self.alive = false;
            self.cause = Some(kind);
            self.self_damage = None;
            self.events.push(Event::Died(kind));
        }
    }

    /// `DestroyBodyPart`: the part marked destroyed, its points as they are.
    pub fn destroy(&mut self, part: Part, kind: DamageKind) {
        if !self.parts[part.index()].destroyed {
            self.parts[part.index()].destroyed = true;
            self.events.push(Event::Destroyed(part, kind));
        }
        self.kill_after_destroyed(part, kind);
    }

    pub(super) fn kill_after_destroyed(&mut self, part: Part, kind: DamageKind) {
        if !kind.bleeding() && part.vital() {
            self.kill(kind);
        }
    }
}
