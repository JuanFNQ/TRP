//! Choosing the parts to help and using medical items.

use super::*;

impl Health {
    /// The frame after the movement's (`Player`'s handlers of both): what
    /// the movement did comes in (falls, jumps, sprints, vaults, the
    /// stamina's fatigue), the effects are stepped, and what health allows
    /// goes out to the movement and its stamina.
    pub fn follow(&mut self, dt: f32, movement: &mut crate::Movement) {
        use crate::locomotion::movement::Moved;
        let before = self.conditions();
        let overweight = movement.stamina.settings.overweight.clamp(0.0, 1.0);
        let vaulting = movement.settings.vaulting;
        for moved in movement.take_moved() {
            match moved {
                Moved::Fell(height) => {
                    let weight = 1.0 + (self.settings.fall_overweight - 1.0) * overweight;
                    self.handle_fall(height, weight);
                }
                Moved::JumpStarted => self.wound_relapse(1.0, None),
                Moved::JumpEnded => self.jump_landed(before),
                Moved::SprintStarted => {
                    if before.leg_damaged() {
                        self.start_self_damage();
                    }
                }
                Moved::SprintEnded(distance) => self.wound_relapse(distance / 10.0, None),
                Moved::Vaulted(height) => self.vaulted(height, vaulting.vault.max_without_hand, before),
            }
        }
        for _ in 0..movement.stamina.take_fatigues() {
            self.add_fatigue();
        }
        self.sprint_frame(dt, movement.state == crate::State::Sprint, before);
        self.step(dt);
        let now = self.conditions();
        movement.set_conditions(now);
        // A berserk's start and residue, after the conditions
        // (`OnHealthEffectAdded`: `BerserkRestorationFactor` after
        // `UpdateSpeedLimitByHealth`).
        for on in std::mem::take(&mut self.berserk_edges) {
            movement.stamina.set_berserk(on);
        }
        if now.leg_damaged() && !now.on_painkillers && movement.state == crate::State::Sprint {
            self.start_self_damage();
        }
        let [common, most] = self.common();
        movement.stamina.set_health(self.energy_share(), self.hydration_share(), if most > 0.0 { common / most } else { 0.0 });
        // Stimulators' capacity and refill (health spec, "Buffs").
        let (capacity, refill) = self.stamina_buffs();
        movement.stamina.set_buffs(capacity, refill);
    }

    // ---- Medicine, food and drink -------------------------------------

    /// Whether the part can be helped by the item, with `resource` left in
    /// it (`BodyPartHealHelper.CanApply`).
    fn helps(&self, m: &Medicine, resource: f32, part: Part) -> bool {
        let state = self.part(part);
        let affordable = |cure: Cure| m.cure(cure).is_some_and(|spec| m.food() || spec.cost <= resource);
        if !state.destroyed {
            if state.current < state.maximum && !m.food() && m.hp_resource_rate > 0.0 && resource > 0.0 {
                return true;
            }
        } else if !part.vital() && affordable(Cure::DestroyedPart) {
            return true;
        }
        self.effects.iter().filter(|e| e.part == part && e.active()).filter_map(|e| cure_of(e.kind)).any(affordable)
    }

    /// The cures the item would do on a part, as far as `resource` pays,
    /// and whether it would heal hit points (`BodyPartHealHelper.UpdateCache`).
    fn cures_on(&self, m: &Medicine, resource: f32, part: Part) -> (bool, Vec<Cure>) {
        let state = self.part(part);
        let mut cures: Vec<Cure> = self.effects.iter().filter(|e| e.part == part && e.active()).filter_map(|e| cure_of(e.kind)).filter(|c| m.cure(*c).is_some()).collect();
        if state.destroyed && !part.vital() && m.cure(Cure::DestroyedPart).is_some() {
            cures.push(Cure::DestroyedPart);
        }
        cures.sort();
        if m.food() {
            return (false, cures);
        }
        let mut left = resource;
        let mut heals = false;
        if !state.destroyed && state.current < state.maximum && m.hp_resource_rate > 0.0 && left > 0.0 {
            heals = true;
            left -= (state.maximum - state.current).min(m.hp_resource_rate);
        }
        let mut kept = Vec::new();
        for cure in cures {
            let cost = m.cure(cure).map_or(0.0, |s| s.cost);
            if left >= cost {
                left -= cost;
                kept.push(cure);
            }
        }
        (heals, kept)
    }

    /// The parts the item can help, best first (`BodyPartsPriority`,
    /// `BodyPartHealHelper.CompareTo`, `PrioritizeBodyParts`).
    pub fn parts_to_help(&self, m: &Medicine, resource: f32, only: Option<Part>) -> Vec<Part> {
        struct Ranked {
            part: Part,
            can: bool,
            cures: Vec<Cure>,
            destroyed: bool,
            share: f32,
        }
        let mut ranked: Vec<Ranked> = Part::ALL
            .into_iter()
            .map(|part| {
                if only.is_some_and(|o| o != part) {
                    return Ranked { part, can: false, cures: Vec::new(), destroyed: false, share: 1.0 };
                }
                let (heals, cures) = self.cures_on(m, resource, part);
                let s = self.part(part);
                Ranked { part, can: heals || !cures.is_empty(), cures, destroyed: s.destroyed, share: if s.maximum > 0.0 { s.current / s.maximum } else { 0.0 } }
            })
            .collect();
        ranked.sort_by(|a, b| {
            use std::cmp::Ordering;
            if a.can != b.can {
                return b.can.cmp(&a.can);
            }
            let (ha, hb) = (!a.cures.is_empty(), !b.cures.is_empty());
            if ha != hb {
                return hb.cmp(&ha);
            }
            if ha {
                for (x, y) in a.cures.iter().zip(&b.cures) {
                    if x != y {
                        return x.cmp(y);
                    }
                }
                if a.cures.len() != b.cures.len() {
                    return b.cures.len().cmp(&a.cures.len());
                }
                if a.destroyed != b.destroyed {
                    return a.destroyed.cmp(&b.destroyed);
                }
            }
            if (a.share - b.share).abs() >= 0.05 {
                return a.share.partial_cmp(&b.share).unwrap_or(Ordering::Equal);
            }
            a.part.cmp(&b.part)
        });
        if !m.part_priority.is_empty() {
            let rank = |p: Part| m.part_priority.iter().position(|q| *q == p).unwrap_or(m.part_priority.len());
            ranked.sort_by_key(|r| rank(r.part));
        }
        ranked.into_iter().filter(|r| r.can).map(|r| r.part).collect()
    }

    /// Whether the item may be used now at all (`ReadyToApplyItem`, health's
    /// part; the hands being busy is the host's).
    pub fn ready(&self) -> Result<(), Refused> {
        if !self.alive {
            return Err(Refused::Dead);
        }
        if self.existing(Kind::MedEffect, None).is_some() {
            return Err(Refused::Incompatible);
        }
        Ok(())
    }

    /// Whether using the item would do anything (`HasPartsToApply`).
    pub fn has_parts_to_apply(&self, m: &Medicine, resource: f32) -> Result<(), Refused> {
        self.ready()?;
        if m.class == MedClass::Drugs || m.buffs.is_some() {
            return Ok(());
        }
        if m.food() {
            return if resource > 0.0 { Ok(()) } else { Err(Refused::Depleted) };
        }
        if Part::ALL.into_iter().any(|p| self.helps(m, resource, p)) { Ok(()) } else { Err(Refused::Incompatible) }
    }

    /// The part the inventory's "Use" names (`ItemUiContext.HealPriority`:
    /// `BodyPartsPriority` with the game's continuous healing off, as the
    /// install has it): the common body (`None`), unless the item has a
    /// priority of parts of its own (on its medkit's part, so with points
    /// to give) and is not of the medkits' class: then the first part it
    /// can help, if that is one of its own, and the common body if not.
    pub fn use_part(&self, m: &Medicine, resource: f32) -> Option<Part> {
        if m.part_priority.is_empty() || m.max_hp_resource <= 0.0 || m.class == MedClass::MedKit {
            return None;
        }
        let first = self.parts_to_help(m, resource, None).first().copied()?;
        m.part_priority.contains(&first).then_some(first)
    }

    /// The part the item goes to (`TryGetBodyPartToApply`): `asked` is
    /// the part it was dropped on, if any.
    pub fn part_to_apply(&self, m: &Medicine, resource: f32, asked: Option<Part>) -> Option<Part> {
        self.ready().ok()?;
        if m.food() && resource > 0.0 && asked.is_none_or(|p| p == Part::Head) {
            return Some(Part::Head);
        }
        if m.class == MedClass::Drugs && asked.is_some() {
            return asked;
        }
        let found = self.parts_to_help(m, resource, asked).first().copied();
        if m.class == MedClass::Drugs {
            return Some(found.unwrap_or(Part::Head));
        }
        if m.buffs.is_some() {
            return Some(Part::Head);
        }
        found
    }

    /// Starts the item at work on a part (`DoMedEffect`): returns the
    /// effect's number, or nothing if it can do nothing there.
    pub fn use_item(&mut self, m: Arc<Medicine>, item: u64, resource: f32, asked: Option<Part>, amount: Option<f32>) -> Option<u32> {
        let part = self.part_to_apply(&m, resource, asked)?;
        let delay = m.start_delay(&self.settings);
        if m.food() {
            let amount = amount.unwrap_or(if m.max_resource > 0.0 { resource / m.max_resource } else { 1.0 });
            let work = m.use_time * amount;
            return Some(self.add_med(m, item, resource, part, delay, work, amount));
        }
        let state = *self.part(part);
        let mut work = m.use_time_on(part);
        if state.maximum - state.current < 10.0 {
            work /= 1.0 + 0.2;
        }
        // The Surgery skill's speed, for a destroyed part the item mends.
        if state.destroyed && m.cure(Cure::DestroyedPart).is_some() {
            work /= 1.0 + self.skills.surgery_speed;
        }
        Some(self.add_med(m, item, resource, part, delay, work, 1.0))
    }
}
