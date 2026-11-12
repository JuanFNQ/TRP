//! Medicine at work: starting, updating, residue and restoring.

use super::*;

impl Health {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_med(&mut self, m: Arc<Medicine>, item: u64, resource: f32, part: Part, delay: f32, work: f32, amount: f32) -> u32 {
        let no_move = {
            let legs = matches!(part, Part::LeftLeg | Part::RightLeg);
            (legs && m.cure(Cure::Fracture).is_some() && self.broken(part)) || m.cure(Cure::DestroyedPart).is_some()
        };
        let med = MedWork {
            medicine: m,
            item,
            amount,
            resource,
            health_points: 0.0,
            energy_points: 0.0,
            hydration_points: 0.0,
            energy_gained: 0.0,
            hydration_gained: 0.0,
            loop_time: 0.0,
            interrupted: false,
            no_move,
        };
        self.add_with(Kind::MedEffect, part, Some(delay), Some(work), None, None, move |e| e.med = Some(Box::new(med)))
    }

    /// Stops what is at work (`RemoveMedEffect`, `CancelApplyingItem`).
    pub fn cancel_items(&mut self) {
        let ids: Vec<u32> = self.effects.iter().filter(|e| e.kind == Kind::MedEffect && e.existing()).map(|e| e.id).collect();
        for id in ids {
            self.force_residue(id);
        }
    }

    pub(super) fn med_started(&mut self, id: u32) {
        let Some(i) = self.index_of(id) else { return };
        let Some(med) = self.effects[i].med.as_mut() else { return };
        let m = med.medicine.clone();
        if !m.food() {
            med.health_points = m.hp_resource_rate * med.amount;
        }
        // `CutPiece`: the item's value times the share used; food's and
        // drink's gains raised by the Metabolism skill, their losses cut by
        // the Immunity skill's.
        med.energy_points = m.energy * med.amount;
        med.hydration_points = m.hydration * med.amount;
        if m.food() {
            let (gain, loss) = (1.0 + self.skills.metabolism_ratio_plus, 1.0 - self.skills.immunity_misc);
            for points in [&mut med.energy_points, &mut med.hydration_points] {
                *points *= if *points > 0.0 { gain } else if *points < 0.0 { loss } else { 1.0 };
            }
        }
    }

    pub(super) fn med_update(&mut self, id: u32, dt: f32) {
        let loop_time = self.settings.med_loop;
        let Some(i) = self.index_of(id) else { return };
        let (part, work, left) = (self.effects[i].part, self.effects[i].work, self.effects[i].state_left());
        let Some(med) = self.effects[i].med.as_mut() else { return };
        med.loop_time += dt;
        if med.loop_time < loop_time && left > 0.0 {
            return;
        }
        let step = if med.loop_time >= loop_time { loop_time } else { med.loop_time };
        med.loop_time -= step;
        let m = med.medicine.clone();
        let mut events = Vec::new();
        if m.food() {
            let piece = m.max_resource * med.amount * step / work;
            if med.resource > 0.0 {
                med.resource = (med.resource - piece).max(0.0);
                events.push(Event::Resource(med.item, med.resource));
            }
        }
        let (health_points, energy_points, hydration_points) = (med.health_points, med.energy_points, med.hydration_points);
        let mut resource = med.resource;
        let state = *self.part(part);
        if health_points != 0.0 {
            let mut give = (health_points * step / work).min(state.maximum - state.current);
            if !m.food() {
                give = give.min(resource);
            }
            if give != 0.0 && !state.destroyed {
                self.change_health(part, give);
                if !m.food() {
                    resource -= give;
                    events.push(Event::Resource(0, resource));
                }
            }
        }
        if energy_points != 0.0 {
            let gain = (energy_points * step / work).min(self.energy[2] - self.energy[0]);
            if gain != 0.0 {
                self.change_energy(gain);
                if let Some(med) = self.index_of(id).and_then(|i| self.effects[i].med.as_mut()) {
                    med.energy_gained += gain;
                }
            }
        }
        if hydration_points != 0.0 {
            let gain = (hydration_points * step / work).min(self.hydration[2] - self.hydration[0]);
            if gain != 0.0 {
                self.change_hydration(gain);
                if let Some(med) = self.index_of(id).and_then(|i| self.effects[i].med.as_mut()) {
                    med.hydration_gained += gain;
                }
            }
        }
        if let Some(med) = self.index_of(id).and_then(|i| self.effects[i].med.as_mut()) {
            if !m.food() {
                med.resource = resource;
            }
            let item = med.item;
            for event in events {
                self.events.push(match event {
                    Event::Resource(_, value) => Event::Resource(item, value),
                    other => other,
                });
            }
        }
    }

    pub(super) fn med_residue(&mut self, id: u32) {
        let interruption = self.settings.interruption_time;
        let Some(i) = self.index_of(id) else { return };
        let (part, whole, delay) = (self.effects[i].part, self.effects[i].whole, self.effects[i].delay);
        let Some(med) = self.effects[i].med.clone() else { return };
        let m = med.medicine.clone();
        let kit = !m.food();
        let mut resource = med.resource;
        if !med.interrupted {
            for cure in Cure::ALL {
                let Some(spec) = m.cure(cure).copied() else { continue };
                if kit && resource < spec.cost {
                    continue;
                }
                let mut done = false;
                match cure {
                    Cure::Pain => {
                        // A painkiller on the head: its delay, its duration
                        // times the share used and raised by the Immunity
                        // skill's buff, its fade.
                        let template = m.template.clone();
                        let duration = spec.duration * med.amount * (1.0 + self.skills.immunity_painkiller);
                        self.add_by_item(Kind::PainKiller, Part::Head, &template, med.amount, spec.delay, duration, spec.fade_out, spec.duration);
                        done = true;
                    }
                    Cure::DestroyedPart => {
                        let kept = whole_range(&mut self.rng, spec.kept[0], spec.kept[1]) as f32 / 100.0;
                        done = self.restore(part, kept);
                    }
                    other => {
                        let on = match other {
                            Cure::Contusion => Some(Part::Head),
                            Cure::Intoxication | Cure::LethalIntoxication | Cure::RadExposure => Some(Part::Stomach),
                            _ => Some(part),
                        };
                        if let (Some(kind), Some(on)) = (other.effect(), on) {
                            if let Some(found) = self.active_id(kind, Some(on)) {
                                self.force_residue(found);
                                self.events.push(Event::Cured(kind, on));
                                done = true;
                            }
                        }
                    }
                }
                if done && kit {
                    resource -= spec.cost;
                }
            }
            // The item's set of buffs, at the share used (`MedEffect.Residue`:
            // only a use not cut short; health spec, "Buffs").
            if let Some(set) = m.buffs.as_deref() {
                self.add_stimulator(&m.template, set, med.amount);
            }
            if kit && m.hp_resource_rate == 0.0 {
                resource = (resource - 1.0).round();
            }
        } else {
            let worked = whole - delay;
            if kit && worked > interruption && m.hp_resource_rate == 0.0 {
                resource = (resource - 1.0).round();
            }
            if worked < interruption {
                self.change_energy(-med.energy_gained);
                self.change_hydration(-med.hydration_gained);
            }
        }
        // What the use gained, kept: told once for the whole use.
        let kept = !med.interrupted || whole - delay >= interruption;
        if kept && (med.energy_gained > 0.0 || med.hydration_gained > 0.0) {
            self.events.push(Event::Fed { energy: med.energy_gained.max(0.0), hydration: med.hydration_gained.max(0.0) });
        }
        resource = resource.floor();
        self.events.push(Event::Resource(med.item, resource));
        if let Some(i) = self.index_of(id) {
            if let Some(w) = self.effects[i].med.as_mut() {
                w.resource = resource;
            }
        }
        if resource <= 0.0 {
            self.events.push(Event::UsedUp(med.item));
        }
    }

    /// Mends a destroyed part (`RestoreBodyPart`): back to 1 point, its
    /// maximum that share of the old, drawn toward all by the Surgery
    /// skill's buff.
    fn restore(&mut self, part: Part, kept: f32) -> bool {
        if part.vital() || !self.parts[part.index()].destroyed {
            return false;
        }
        let kept = kept + (1.0 - kept) * self.skills.surgery_penalty;
        let p = &mut self.parts[part.index()];
        p.destroyed = false;
        p.maximum = (p.maximum * kept).ceil().max(1.0);
        p.current = 1.0f32.min(p.maximum);
        self.events.push(Event::Restored(part));
        true
    }

    /// What an item at work holds now, if it is at work.
    pub fn item_at_work(&self) -> Option<&MedWork> {
        self.effects.iter().filter(|e| e.kind == Kind::MedEffect && e.existing()).find_map(|e| e.med.as_deref())
    }
}

pub(super) fn share(value: [f32; 3]) -> f32 {
    let [current, least, most] = value;
    if most > least { (current - least) / (most - least) } else { 1.0 }
}

/// The cure an effect is cured by (`HealthHelper.TryGetDamageEffectType`).
pub(super) fn cure_of(kind: Kind) -> Option<Cure> {
    Some(match kind {
        Kind::HeavyBleeding => Cure::HeavyBleeding,
        Kind::LightBleeding => Cure::LightBleeding,
        Kind::Fracture => Cure::Fracture,
        Kind::Pain => Cure::Pain,
        Kind::Contusion => Cure::Contusion,
        Kind::Intoxication => Cure::Intoxication,
        _ => return None,
    })
}
