//! Making the stamina and feeding it buffs, health, weight and stance.

use super::*;

impl Stamina {
    pub fn new(settings: StaminaSettings) -> Self {
        let (legs, arms, oxygen) = (settings.legs_capacity(), settings.arms_capacity(), settings.oxygen_capacity());
        let mut stamina = Self {
            settings,
            legs: Pool::new(legs),
            arms: Pool::new(arms),
            oxygen: Pool::new(oxygen),
            aim_mass: 0.0,
            pose: 2,
            pose_rise: 0.0,
            spam: 0.0,
            spam_until: f32::NEG_INFINITY,
            aim_cost: None,
            aim_taken: (false, 0.0),
            aim_drain_by: 1.0,
            rangefinder: false,
            drains: true,
            holds_back: true,
            health_lets_sprint: true,
            fatigue: 0.0,
            fatigues: 0,
            switch_lack: 0.0,
        };
        // A fully overweight character pays for standing about.
        if stamina.settings.overweight >= 1.0 {
            stamina.legs.run(Drain::OverweightIdle);
        }
        stamina
    }

    /// Takes changed settings (skills, how well fed): the capacities
    /// follow, and a pool that was full stays full.
    pub fn resettle(&mut self) {
        let s = &self.settings;
        let capacities = [s.legs_capacity(), s.arms_capacity(), s.oxygen_capacity()];
        for (pool, capacity) in [&mut self.legs, &mut self.arms, &mut self.oxygen].into_iter().zip(capacities) {
            let full = pool.current >= pool.capacity;
            pool.capacity = capacity;
            pool.current = if full { capacity } else { pool.current.min(capacity) };
        }
        // A skill's new level has the cost of aiming worked out again.
        self.aim_cost = None;
    }

    /// A berserk started or went residual (`Physical.BerserkRestorationFactor`,
    /// on the effect's start and its residue): the legs and the arms pay
    /// nothing while it works, and each refills at once either way; the
    /// oxygen pays as ever (health spec, "A berserk").
    pub fn set_berserk(&mut self, on: bool) {
        self.legs.set_force(on);
        self.arms.set_force(on);
    }

    /// What stimulators' buffs at work add to the legs' and the arms'
    /// capacity and to the refills (`StaminaSettings::buff_capacity`,
    /// `buff_restoration`), told every frame; the capacities worked out
    /// again where they change, as each buff's start and end does in the
    /// game (`Skills.Endurance.OnLevelUp`).
    pub fn set_buffs(&mut self, capacity: f32, restoration: f32) {
        self.settings.buff_restoration = restoration;
        if capacity != self.settings.buff_capacity {
            self.settings.buff_capacity = capacity;
            // The pools keep what they hold, down to a smaller capacity
            // (`Stamina.Update`): a larger one is filled by refilling.
            let capacities = [self.settings.legs_capacity(), self.settings.arms_capacity()];
            for (pool, capacity) in [&mut self.legs, &mut self.arms].into_iter().zip(capacities) {
                pool.capacity = capacity;
                pool.current = pool.current.min(capacity);
            }
        }
    }

    /// Energy, hydration and health as health has them, each a share of its
    /// most (`OnEnergyChanged`, `OnHydrationChanged`). The refills and the
    /// sprint's and the jump's costs go by them at once; aiming's kept cost
    /// does not take a change of hydration (stamina spec, "Figures the
    /// game keeps"); the oxygen's capacity is worked out again only when
    /// energy changes, with the health of that moment, and its pool keeps
    /// what it holds up to that.
    pub fn set_health(&mut self, energy: f32, hydration: f32, health: f32) {
        self.settings.hydration = hydration;
        if energy != self.settings.energy {
            self.settings.energy = energy;
            self.settings.health = health;
            self.oxygen.capacity = self.settings.oxygen_capacity();
            self.oxygen.current = self.oxygen.current.min(self.oxygen.capacity);
        }
    }

    /// Damaged arms make aiming dearer (`Physical`'s aim drain, marked stale
    /// when an arm's condition changes).
    pub fn set_aim_factor(&mut self, factor: f32) {
        if factor != self.settings.arms_aim {
            self.settings.arms_aim = factor;
            self.aim_cost = None;
        }
    }

    /// How many fatigues spent legs have given since this was last asked
    /// (`PhysicalBase`: 15 s with the legs under the threshold gives the
    /// health system one, `AddFatigue`).
    pub fn take_fatigues(&mut self) -> u32 {
        std::mem::take(&mut self.fatigues)
    }

    /// The carried weight has changed: how overweight the character is,
    /// for everything and for an upright walk. A fully overweight
    /// character pays for standing about from then on, and one no longer
    /// so stops paying.
    pub fn reweigh(&mut self, overweight: f32, walk_overweight: f32) {
        (self.settings.overweight, self.settings.walk_overweight) = (overweight, walk_overweight);
        match overweight >= 1.0 {
            true if !self.legs.runs(Drain::OverweightIdle) => self.legs.run(Drain::OverweightIdle),
            false => self.legs.stop(Drain::OverweightIdle),
            true => {}
        }
    }

    /// The legs or the oxygen under the threshold (`Physical.Exhausted`).
    pub fn exhausted(&self) -> bool {
        self.holds_back && (self.legs.exhausted() || self.oxygen.exhausted())
    }

    /// The legs under the threshold, where that holds the character back.
    pub fn legs_spent(&self) -> bool {
        self.holds_back && self.legs.exhausted()
    }

    /// Whether a sprint is being paid for: the sprint's own flag.
    pub fn sprinting(&self) -> bool {
        self.legs.runs(Drain::Sprint)
    }

    /// Whether a jump may start: more than the threshold in the legs.
    pub fn can_jump(&self) -> bool {
        !self.holds_back || self.legs.can_add(EXHAUSTED)
    }

    /// Whether a vault or a climb may start (`Physical.CanVault`,
    /// `CanClimb`): more than the threshold in the legs, and in the arms
    /// too where the obstacle is high enough to take a hand.
    pub fn can_vault(&self, hand: bool) -> bool {
        !self.holds_back || (self.legs.can_add(EXHAUSTED) && (!hand || self.arms.can_add(EXHAUSTED)))
    }
}
