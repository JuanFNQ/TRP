//! Making the health, taking up a saved state and reading it.

use super::*;

impl Health {
    /// Full health, as a run begins (full every run),
    /// with the existence effect on the head as the game's player has it.
    pub fn new(settings: Arc<HealthSettings>, seed: u64) -> Self {
        let parts = std::array::from_fn(|i| PartState { current: settings.maximum[i], maximum: settings.maximum[i], destroyed: false });
        let (e, h) = (settings.energy, settings.hydration);
        let mut health = Self {
            parts,
            energy: [e[2], e[0], e[1]],
            hydration: [h[2], h[0], h[1]],
            settings,
            alive: true,
            cause: None,
            effects: Vec::new(),
            next_id: 0,
            rng: Rng::new(seed),
            outcomes: Vec::new(),
            events: Vec::new(),
            damage_multiplier: 1.0,
            skills: HealthSkills::default(),
            metabolism_disabled: false,
            clock: 0.0,
            self_damage: None,
            encumbered: None,
            over_encumbered: None,
            berserk_edges: Vec::new(),
        };
        health.add(Kind::Existence, Part::Head, None, None, None, None);
        health
    }

    /// An effect taken up as at work already, with its kind's own figures
    /// and no delay: for a run that takes up a character mid-raid (the
    /// health replay), where an effect began before it did.
    pub fn take_up(&mut self, kind: Kind, part: Part) -> u32 {
        self.add(kind, part, Some(0.0), None, None, None)
    }

    /// An effect taken up as a recording has it at work (the health
    /// replay, whose recordings begin mid-raid): of `kind` on `part`, added
    /// or started, with `left` of its state's time (`None`: for as long as
    /// it lasts, the kind's own). One that an effect taken up before it made
    /// of itself as it began (dehydration's pain, a fracture's) is set so
    /// and stays that one's; a tremor taken up while a pain is at work is
    /// that pain's, as the pain makes its tremor (`Pain`'s `TremorDelay`).
    /// Take up the causes first.
    pub fn take_up_as(&mut self, kind: Kind, part: Part, stage: Stage, left: Option<f32>) -> u32 {
        let id = match self.existing_id(kind, Some(part)) {
            Some(id) => {
                if stage == Stage::Started && self.index_of(id).is_some_and(|i| self.effects[i].stage == Stage::Added) {
                    self.next_state(id);
                }
                id
            }
            None => {
                let delay = if stage == Stage::Added { left } else { Some(0.0) };
                let work = if stage == Stage::Started { left } else { None };
                self.add(kind, part, delay, work, None, None)
            }
        };
        if let (Some(i), Some(left)) = (self.index_of(id), left) {
            let e = &mut self.effects[i];
            match e.stage {
                Stage::Added => {
                    e.delay = left;
                    e.state_time = left;
                }
                Stage::Started => {
                    e.delay = 0.0;
                    e.work = left;
                    e.state_time = left;
                }
                _ => {}
            }
            e.state_current = 0.0;
            e.whole = 0.0;
        }
        if kind == Kind::Tremor {
            if let Some(i) = self.effects.iter().position(|e| e.kind == Kind::Pain && e.stage == Stage::Started && e.side.is_none() && e.tremor_after.is_some()) {
                self.effects[i].tremor_after = None;
                self.effects[i].side = Some(id);
            }
        }
        id
    }

    /// Sets how far into its own loop an effect is (`loops[0]`): for a
    /// starving loop taken up mid-raid, set to tick where the recording's
    /// does.
    pub fn set_loop_clock(&mut self, id: u32, into_loop: f32) {
        if let Some(i) = self.index_of(id) {
            self.effects[i].loops[0] = into_loop;
        }
    }

    /// Sets how far into its loops existence's hunger and thirst are: for a
    /// run that takes up a character mid-raid (the health replay, whose
    /// recordings begin with the raid's clock already going).
    pub fn set_existence_clock(&mut self, into_loop: f32) {
        for e in self.effects.iter_mut().filter(|e| e.kind == Kind::Existence) {
            e.loops = [into_loop, into_loop];
        }
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// What the effects at work take from or give to the whole's health,
    /// energy and hydration, a second, as the inventory's figures show it
    /// (`HealthRate`, `EnergyRate`, `HydrationRate`, which the screen shows
    /// a minute): each effect's own rate (`SetHealthRatesPerSecond`), set
    /// as it starts and changed as it works, counted while it works; energy
    /// and hydration none while metabolism is stopped. Only the effects
    /// this simulation has: a bleeding's damage, and its energy; an
    /// intoxication's damage; strong
    /// dehydration's and exhaustion's damage; existence's energy and
    /// hydration; chronic fatigue's energy; medicine's, food's and drink's
    /// points over their work.
    pub fn rates(&self) -> [f32; 3] {
        let parts = Part::ALL.len() as f32;
        let mut sum = [0.0f32; 3];
        for e in self.effects.iter().filter(|e| e.active()) {
            let rate = match e.kind {
                Kind::LightBleeding | Kind::HeavyBleeding => {
                    let s = if e.kind == Kind::HeavyBleeding { self.settings.heavy } else { self.settings.light };
                    [e.figures[2], -s.energy_damage / s.energy_loop, 0.0]
                }
                Kind::Intoxication => [e.figures[2], 0.0, 0.0],
                Kind::Dehydration => {
                    let [damage, every] = self.starving(self.settings.dehydration_damage);
                    [-damage * parts / every, 0.0, 0.0]
                }
                Kind::Exhaustion => {
                    let [damage, every] = self.starving(self.settings.exhaustion_damage);
                    [-damage * parts / every, 0.0, 0.0]
                }
                Kind::Existence => {
                    let s = self.settings.existence;
                    let stomach_gone = self.part(Part::Stomach).destroyed;
                    let (energy_loop, hydration_loop) = if stomach_gone { (s[1] / s[4], s[3] / s[5]) } else { (s[1], s[3]) };
                    [0.0, -s[0] * (1.0 - self.skills.health_energy) / energy_loop, -s[2] * (1.0 - self.skills.health_hydration) / hydration_loop]
                }
                Kind::ChronicFatigue => [0.0, -self.fatigue_rate(e.id), 0.0],
                Kind::MedEffect => match e.med.as_deref() {
                    Some(m) if e.work > 0.0 => [m.health_points / e.work, m.energy_points / e.work, m.hydration_points / e.work],
                    _ => [0.0; 3],
                },
                _ => [0.0; 3],
            };
            for (total, own) in sum.iter_mut().zip(rate) {
                *total += own;
            }
        }
        if self.metabolism_disabled {
            sum[1] = 0.0;
            sum[2] = 0.0;
        }
        sum
    }

    /// The seven parts' sum (`GetBodyPartHealth(Common)`): current and
    /// maximum.
    pub fn common(&self) -> [f32; 2] {
        self.parts.iter().fold([0.0, 0.0], |[c, m], p| [c + p.current, m + p.maximum])
    }

    pub fn part(&self, part: Part) -> &PartState {
        &self.parts[part.index()]
    }

    /// Energy and hydration as shares of their most.
    pub fn energy_share(&self) -> f32 {
        share(self.energy)
    }

    pub fn hydration_share(&self) -> f32 {
        share(self.hydration)
    }

    pub(super) fn index_of(&self, id: u32) -> Option<usize> {
        self.effects.iter().position(|e| e.id == id)
    }

    /// The first effect of a kind on a part (`None` for any part) that is
    /// added or started.
    pub fn existing(&self, kind: Kind, part: Option<Part>) -> Option<&Effect> {
        self.effects.iter().find(|e| e.kind == kind && part.is_none_or(|p| e.part == p) && e.existing())
    }

    /// The first effect of a kind on a part that has started.
    pub fn active(&self, kind: Kind, part: Option<Part>) -> Option<&Effect> {
        self.effects.iter().find(|e| e.kind == kind && part.is_none_or(|p| e.part == p) && e.active())
    }

    pub(super) fn active_id(&self, kind: Kind, part: Option<Part>) -> Option<u32> {
        self.active(kind, part).map(|e| e.id)
    }

    pub(super) fn existing_id(&self, kind: Kind, part: Option<Part>) -> Option<u32> {
        self.existing(kind, part).map(|e| e.id)
    }

    /// Whether a limb is broken (`IsBodyPartBroken`: a fracture started).
    pub fn broken(&self, part: Part) -> bool {
        self.active(Kind::Fracture, Some(part)).is_some()
    }

    /// The conditions as the game's player sets them.
    pub fn conditions(&self) -> Conditions {
        let painkiller = self.active(Kind::PainKiller, None).is_some();
        let berserk = self.active(Kind::Berserk, None).is_some();
        let damaged = |part: Part| self.broken(part) || self.part(part).destroyed;
        let meds: Vec<&Effect> = self.effects.iter().filter(|e| e.kind == Kind::MedEffect && e.active()).collect();
        Conditions {
            on_painkillers: painkiller || berserk,
            left_leg: damaged(Part::LeftLeg),
            right_leg: damaged(Part::RightLeg),
            left_arm: damaged(Part::LeftArm) && !painkiller,
            right_arm: damaged(Part::RightArm) && !painkiller,
            tremor: self.active(Kind::Tremor, None).is_some(),
            using_meds: !meds.is_empty(),
            healing_legs: meds.iter().any(|e| e.med.as_ref().is_some_and(|m| m.no_move)),
        }
    }
}
