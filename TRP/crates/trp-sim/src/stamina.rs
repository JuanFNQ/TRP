//! What the character pays for moving and for holding a weapon up: the
//! game's three pools (legs, arms, oxygen), the drains on them and the
//! refill with its pause. Specified in `docs/spec/stamina.md`.

use anyhow::{anyhow, Context, Result};
use serde_json::Value as Json;

use crate::math::{inverse_lerp, lerp};

/// Below this a pool is exhausted, and at or below it nothing that needs
/// a start threshold starts (`Stamina.EXHAUSTED_THRESHOLD`; the sprint's
/// and the jump's `StartThreshold` are the same figure).
pub const EXHAUSTED: f32 = 15.0;

/// A factor by how much the character has eaten or drunk
/// (`StaminaPolygonalGraph`): 1 on a plateau, sloping away either side.
#[derive(Clone, Copy, Debug)]
pub struct Graph {
    pub lower_left: f32,
    pub lower_right: f32,
    pub left_plateau: f32,
    pub right_plateau: f32,
    pub right_limit: f32,
    pub zero: f32,
}

impl Graph {
    fn from(config: &Json, name: &str) -> Result<Self> {
        let section = config.get(name).ok_or_else(|| anyhow!("globals have no {name}"))?;
        let number = |key: &str| -> Result<f32> {
            section.get(key).and_then(Json::as_f64).map(|f| f as f32).ok_or_else(|| anyhow!("{name} has no {key}"))
        };
        Ok(Self {
            lower_left: number("LowerLeftPoint")?,
            lower_right: number("LowerRightPoint")?,
            left_plateau: number("LeftPlatoPoint")?,
            right_plateau: number("RightPlatoPoint")?,
            right_limit: number("RightLimit")?,
            zero: number("ZeroValue")?,
        })
    }

    /// The factor at `level`, where 1 is full.
    pub fn at(&self, level: f32) -> f32 {
        let level = level.clamp(0.0, self.right_limit);
        if level > self.right_plateau {
            (1.0 - self.lower_right) * (level - self.right_plateau) / (self.right_plateau - self.right_limit) + 1.0
        } else if level > self.left_plateau {
            1.0
        } else if level > 0.0 {
            self.lower_left + (1.0 - self.lower_left) * level / self.left_plateau
        } else {
            self.zero
        }
    }
}

/// The figures stamina runs on: `globals.json`'s `Stamina` section, the
/// carried weight's two overweights, and what the character's skills and
/// condition make of them.
#[derive(Clone, Debug)]
pub struct StaminaSettings {
    pub capacity: f32,
    pub restoration: f32,
    pub hands_capacity: f32,
    pub hands_restoration: f32,
    pub oxygen_capacity: f32,
    pub oxygen_restoration: f32,
    pub sprint_drain: f32,
    pub jump_cost: f32,
    pub aim_drain: f32,
    /// By stance class: lying, fully crouched, anything higher.
    pub aim_by_pose: [f32; 3],
    pub restoration_by_pose: [f32; 3],
    pub overweight_by_pose: [f32; 3],
    /// Each from an unburdened character to a fully overweight one.
    pub standup: [f32; 2],
    pub pose_notch: [f32; 2],
    pub walk: [f32; 2],
    pub crouch: [f32; 2],
    pub prone_cost: f32,
    /// A vault's cost to the legs and, over something high enough to take
    /// a hand, to the arms; a climb's to the legs, and to the arms with
    /// one hand taken and with both (vaulting spec, "Stamina").
    pub vault_legs: [f32; 2],
    pub vault_hand: [f32; 2],
    pub climb_legs: [f32; 2],
    pub climb_hand: [f32; 2],
    pub climb_hands: [f32; 2],
    /// What holding breath costs the oxygen a second, and what that is
    /// multiplied by from empty legs to full ones.
    pub hold_breath: f32,
    pub hold_breath_by_legs: [f32; 2],
    /// Whether the legs' level also counts toward how shaky the hands are
    /// (`StaminaExhaustionCausesJiggle`) and toward how hard the breath
    /// goes (`StaminaExhaustionRocksCamera`): weapon-handling spec,
    /// "Breathing".
    pub legs_shake_hands: bool,
    pub legs_deepen_breath: bool,
    /// How far a notch of the stance wheel raises the stance.
    pub delta_level: f32,

    /// 0 to 1 by carried weight, for everything and for an upright walk.
    pub overweight: f32,
    pub walk_overweight: f32,

    /// What the Endurance skill adds: to the legs' capacity, to every
    /// refill, off a jump's cost, to the oxygen's capacity. And what
    /// Strength takes off the cost of aiming. All 0 untrained.
    pub endurance_capacity: f32,
    pub endurance_restoration: f32,
    pub endurance_jump: f32,
    pub endurance_breath: f32,
    pub strength_aim: f32,
    /// The skills' buffs at level 50, in that order, for `set_skills`.
    skill_buffs: [f32; 5],

    /// How well fed and watered the character is, 1 for full, and the
    /// two curves that turn those into factors.
    pub energy: f32,
    pub hydration: f32,
    pub energy_graph: Graph,
    pub hydration_graph: Graph,
    /// The common health's share of its most, as the oxygen's capacity has
    /// it: worked in only when energy changes (`Stamina::set_health`).
    pub health: f32,
    /// What aiming's cost is multiplied by for damaged arms (1, 1.5 or 2:
    /// `Conditions::aim_cost`).
    pub arms_aim: f32,
    /// How long spent legs go before health is given a fatigue
    /// (`FatigueAmountToCreateEffect`), and how fast the tally falls
    /// otherwise (`FatigueRestorationRate`).
    pub fatigue: [f32; 2],
}

impl StaminaSettings {
    /// `config` is the `config` section of `globals.json`.
    pub fn new(config: &Json, weight: f32, delta_level: f32) -> Result<Self> {
        let s = config.get("Stamina").ok_or_else(|| anyhow!("globals have no Stamina"))?;
        let number = |name: &str| -> Result<f32> {
            s.get(name).and_then(Json::as_f64).map(|f| f as f32).ok_or_else(|| anyhow!("Stamina has no {name}"))
        };
        let parts = |name: &str| -> Result<[f32; 3]> {
            let v = s.get(name).ok_or_else(|| anyhow!("Stamina has no {name}"))?;
            let part = |k: &str| v.get(k).and_then(Json::as_f64).map(|f| f as f32).context(name.to_owned());
            Ok([part("x")?, part("y")?, part("z")?])
        };
        let range = |name: &str| -> Result<[f32; 2]> { parts(name).map(|v| [v[0], v[1]]) };
        let within = |name: &str| -> Result<f32> { range(name).map(|r| inverse_lerp(r[0], r[1], weight)) };
        Ok(Self {
            capacity: number("Capacity")?,
            restoration: number("BaseRestorationRate")?,
            hands_capacity: number("HandsCapacity")?,
            hands_restoration: number("HandsRestoration")?,
            oxygen_capacity: number("OxygenCapacity")?,
            oxygen_restoration: number("OxygenRestoration")?,
            sprint_drain: number("SprintDrainRate")?,
            jump_cost: number("JumpConsumption")?,
            aim_drain: number("AimDrainRate")?,
            aim_by_pose: parts("AimConsumptionByPose")?,
            restoration_by_pose: parts("RestorationMultiplierByPose")?,
            overweight_by_pose: parts("OverweightConsumptionByPose")?,
            standup: range("StandupConsumption")?,
            pose_notch: range("PoseLevelConsumptionPerNotch")?,
            walk: range("WalkConsumption")?,
            crouch: range("CrouchConsumption")?,
            prone_cost: number("ProneConsumption")?,
            vault_legs: range("VaultLegsConsumption")?,
            vault_hand: range("VaultOneHandConsumption")?,
            climb_legs: range("ClimbLegsConsumption")?,
            climb_hand: range("ClimbOneHandConsumption")?,
            climb_hands: range("ClimbTwoHandsConsumption")?,
            hold_breath: number("BaseHoldBreathConsumption")?,
            hold_breath_by_legs: range("HoldBreathStaminaMultiplier")?,
            legs_shake_hands: s.get("StaminaExhaustionCausesJiggle").and_then(Json::as_bool).unwrap_or(false),
            legs_deepen_breath: s.get("StaminaExhaustionRocksCamera").and_then(Json::as_bool).unwrap_or(false),
            delta_level,
            overweight: within("BaseOverweightLimits")?,
            walk_overweight: within("WalkOverweightLimits")?,
            endurance_capacity: 0.0,
            endurance_restoration: 0.0,
            endurance_jump: 0.0,
            endurance_breath: 0.0,
            strength_aim: 0.0,
            // The buffs' full values are written in the game's code
            // (`SkillManager`), not in the install's data: stamina spec,
            // "Capacity and refill".
            skill_buffs: [0.5, 0.5, 0.3, 1.0, 0.2],
            energy: 1.0,
            hydration: 1.0,
            energy_graph: Graph::from(config, "StaminaRestoration")?,
            hydration_graph: Graph::from(config, "StaminaDrain")?,
            health: 1.0,
            arms_aim: 1.0,
            fatigue: [number("FatigueAmountToCreateEffect")?, number("FatigueRestorationRate")?],
        })
    }

    /// Sets what the Endurance and Strength skills do to stamina, by their
    /// levels (0 to 50; each buff grows by the level).
    pub fn set_skills(&mut self, endurance: f32, strength: f32) {
        let share = |level: f32| (level.clamp(0.0, 50.0).floor()) / 50.0;
        let [capacity, restoration, jump, breath, aim] = self.skill_buffs;
        self.endurance_capacity = capacity * share(endurance);
        self.endurance_restoration = restoration * share(endurance);
        self.endurance_jump = jump * share(endurance);
        self.endurance_breath = breath * share(endurance);
        self.strength_aim = aim * share(strength);
    }

    /// The Endurance level at which the legs hold `capacity`.
    pub fn endurance_for(&self, capacity: f32) -> f32 {
        ((capacity / self.capacity - 1.0) / self.skill_buffs[0] * 50.0).round().clamp(0.0, 50.0)
    }

    /// What the legs and the arms refill by a second in a stance class,
    /// before the character's food has its say: for reading that say off
    /// a recording.
    pub fn refill_before_energy(&self, pose: usize) -> [f32; 2] {
        let rest = self.restoration_by_pose[pose] * (1.0 + self.endurance_restoration) / self.drain_factor();
        [self.restoration * rest, self.hands_restoration * rest]
    }

    /// Sets how well fed the character is from the factor that has on
    /// the refill: 1 or more is anywhere on the curve's plateau, less is
    /// taken as overfed, down the slope to the right of it.
    pub fn set_energy_for(&mut self, factor: f32) {
        let g = self.energy_graph;
        self.energy = if factor >= 1.0 || g.lower_right >= 1.0 {
            g.right_plateau
        } else {
            g.right_plateau + (1.0 - factor) * (g.right_limit - g.right_plateau) / (1.0 - g.lower_right)
        };
    }

    /// Sets how much the character has drunk from the factor that has on
    /// every drain: 1 is anywhere on the curve's plateau, more is taken
    /// as thirst, down the slope to the left of it.
    pub fn set_hydration_for(&mut self, factor: f32) {
        let g = self.hydration_graph;
        self.hydration = if factor <= 1.0 || g.lower_left <= 1.0 {
            1.0
        } else {
            (g.left_plateau * (g.lower_left - factor) / (g.lower_left - 1.0)).clamp(1e-3, g.left_plateau)
        };
    }

    /// What a third of a notch of stance raised costs.
    pub fn stance_third(&self) -> f32 {
        lerp(self.pose_notch[0], self.pose_notch[1], self.overweight) / 3.0
    }

    fn drain_factor(&self) -> f32 {
        self.hydration_graph.at(self.hydration)
    }

    fn refill_factor(&self, pose: usize) -> f32 {
        self.restoration_by_pose[pose] * self.energy_graph.at(self.energy) * (1.0 + self.endurance_restoration)
            / self.drain_factor()
    }

    pub fn legs_capacity(&self) -> f32 {
        (self.capacity * (1.0 + self.endurance_capacity)).max(1.0)
    }

    pub fn arms_capacity(&self) -> f32 {
        self.hands_capacity.max(1.0)
    }

    pub fn oxygen_capacity(&self) -> f32 {
        self.oxygen_capacity * (0.5 + 0.5 * self.energy.clamp(0.0, 1.0) * (2.0 * self.endurance_breath + self.health))
    }
}

/// Something that costs stamina by the second while it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drain {
    Sprint,
    Walk,
    Aim,
    OverweightIdle,
    Crawl,
    HoldBreath,
}

impl Drain {
    /// How long after it the pool does not refill, or `None` where it
    /// lets the pool refill regardless.
    fn downtime(self) -> Option<f32> {
        match self {
            Drain::Sprint => Some(1.0),
            Drain::Walk => Some(0.1),
            Drain::Aim => Some(0.0),
            Drain::OverweightIdle => None,
            Drain::Crawl => Some(0.5),
            Drain::HoldBreath => Some(0.0),
        }
    }
}

/// One of the three pools.
#[derive(Clone, Debug)]
pub struct Pool {
    pub current: f32,
    pub capacity: f32,
    /// The engine's clock has to pass this before the pool refills.
    no_refill_until: f32,
    drains: Vec<Drain>,
    /// How many times the game's pool would have said it changed
    /// (`Stamina.OnChanged`: a drain started or stopped, a cost taken off
    /// at once), which raises the game's stance panel.
    changes: u32,
}

impl Pool {
    fn new(capacity: f32) -> Self {
        Self { current: capacity, capacity, no_refill_until: 0.0, drains: Vec::new(), changes: 0 }
    }

    /// The count of its changes so far (`changes`): one that differs from
    /// the last seen means it changed since.
    pub fn changes(&self) -> u32 {
        self.changes
    }

    /// Holds the refill off until the engine's clock passes `time`: for a
    /// replay, which starts a pool wherever a recording's last drain
    /// left it.
    pub fn hold_refill_until(&mut self, time: f32) {
        self.no_refill_until = time;
    }

    /// How full, 0 to 1: what the game's bars show.
    pub fn level(&self) -> f32 {
        self.current / self.capacity
    }

    pub fn exhausted(&self) -> bool {
        self.current < EXHAUSTED
    }

    fn runs(&self, drain: Drain) -> bool {
        self.drains.contains(&drain)
    }

    fn run(&mut self, drain: Drain) {
        if !self.runs(drain) {
            self.drains.push(drain);
            self.changes = self.changes.wrapping_add(1);
        }
    }

    fn stop(&mut self, drain: Drain) {
        if self.runs(drain) {
            self.drains.retain(|d| *d != drain);
            self.changes = self.changes.wrapping_add(1);
        }
    }

    /// Takes a cost off at once. Gives back what the pool could not pay,
    /// and whether this emptied it.
    fn charge(&mut self, cost: f32, downtime: Option<f32>, now: f32) -> (f32, bool) {
        self.changes = self.changes.wrapping_add(1);
        let unpaid = (cost - self.current).max(0.0);
        self.current = (self.current - cost).max(0.0);
        if cost <= 0.0 {
            return (unpaid, false);
        }
        if let Some(downtime) = downtime {
            self.no_refill_until = now + downtime + (1.0 - self.level());
        }
        (unpaid, self.current <= 0.0)
    }

    /// One frame: the running drains come off, then the refill goes on if
    /// its pause is over. Says whether the pool ran dry.
    fn work(&mut self, costs: &[(Drain, f32)], refill: f32, dt: f32, now: f32) -> bool {
        let mut total = 0.0;
        let mut pause: Option<f32> = None;
        for (drain, cost) in costs.iter().filter(|(drain, _)| self.runs(*drain)) {
            total += cost;
            if let Some(downtime) = drain.downtime().filter(|_| *cost > 0.0) {
                pause = Some(pause.map_or(downtime, |longest| longest.max(downtime)));
            }
        }
        if let Some(pause) = pause {
            self.no_refill_until = now + pause + (1.0 - self.level());
        }
        let mut dry = false;
        if total > 0.0 {
            self.current = (self.current - total * dt).max(0.0);
            dry = self.current.abs() < 1e-6;
        }
        if self.current < self.capacity && now > self.no_refill_until {
            self.current = (self.current + refill * dt).min(self.capacity);
        }
        self.current = self.current.clamp(0.0, self.capacity);
        dry
    }
}

/// The character's stamina (the game's `Physical`).
#[derive(Clone, Debug)]
pub struct Stamina {
    pub settings: StaminaSettings,
    pub legs: Pool,
    pub arms: Pool,
    pub oxygen: Pool,
    /// The weapon's ergonomic weight, which aiming costs by: a host sets
    /// it for the weapon in hand (weapon-handling spec, "When the
    /// weapon's weight is taken").
    pub aim_mass: f32,
    /// The stance class the costs go by: 0 lying, 1 fully crouched, 2 any
    /// higher.
    pose: usize,
    /// How much the stance has risen since the last charge for it.
    pub pose_rise: f32,
    /// The cost of starting a sprint again too soon, and until when
    /// (engine's clock) a new start counts as too soon.
    spam: f32,
    spam_until: f32,
    /// What the sights up cost a second as last worked out, with the
    /// sights' state and the weight it was worked out for. The game keeps
    /// this figure until the sights, the stance class, the weapon's
    /// weight or the skills change; how well watered the character is is
    /// in it and is not among those (stamina spec, "Figures the game
    /// keeps").
    aim_cost: Option<f32>,
    aim_taken: (bool, f32),
    /// The two systems of the features' list this is (`crate::Systems`):
    /// with `drains` off nothing is paid and the pools stay full; with
    /// `holds_back` off (exhaustion) a spent pool holds nothing back.
    pub drains: bool,
    pub holds_back: bool,
    /// Whether the character's health lets a sprint be paid for (`Physical`'s
    /// sprint drain: no medicine at work, no damaged leg without a
    /// painkiller). Health's; true unless a host's health system says.
    pub health_lets_sprint: bool,
    /// The tally of spent legs, and how many fatigues it has given that
    /// health has not taken (`PhysicalBase`'s `Fatigue`).
    fatigue: f32,
    fatigues: u32,
}

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
            drains: true,
            holds_back: true,
            health_lets_sprint: true,
            fatigue: 0.0,
            fatigues: 0,
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
        !self.holds_back || self.legs.current > EXHAUSTED
    }

    /// Whether a vault or a climb may start (`Physical.CanVault`,
    /// `CanClimb`): more than the threshold in the legs, and in the arms
    /// too where the obstacle is high enough to take a hand.
    pub fn can_vault(&self, hand: bool) -> bool {
        !self.holds_back || (self.legs.current > EXHAUSTED && (!hand || self.arms.current > EXHAUSTED))
    }

    /// A cost that goes by how overweight the character is: standing by
    /// the walk's overweight, fully crouched by the general one, and
    /// nothing lying down.
    fn by_stance(&self, range: [f32; 2]) -> f32 {
        let s = &self.settings;
        match self.pose {
            2 => lerp(range[0], range[1], s.walk_overweight),
            1 => lerp(range[0], range[1], s.overweight),
            _ => 0.0,
        }
    }

    /// A vault begun (`Physical.OnVault`), over something that takes a
    /// hand or not. Standing, the legs pay only over the walk's weight.
    pub fn vault(&mut self, hand: bool, now: f32) {
        let free = self.pose == 2 && self.settings.walk_overweight <= 0.0;
        let legs = if free { 0.0 } else { self.by_stance(self.settings.vault_legs) };
        let arms = if hand { self.by_stance(self.settings.vault_hand) } else { 0.0 };
        self.charge_move(legs, arms, now);
    }

    /// A climb begun (`Physical.OnClimb`), with a hand taken, or both.
    pub fn climb(&mut self, hand: bool, both: bool, now: f32) {
        let s = &self.settings;
        let legs = self.by_stance(s.climb_legs);
        let arms = match (both, hand) {
            (true, _) => self.by_stance(s.climb_hands),
            (false, true) => self.by_stance(s.climb_hand),
            (false, false) => 0.0,
        };
        self.charge_move(legs, arms, now);
    }

    /// A move's two charges: the legs', then the arms', and whatever the
    /// arms could not pay of theirs the legs pay again whole.
    fn charge_move(&mut self, legs: f32, arms: f32, now: f32) {
        /// How long after such a charge a pool does not refill.
        const DOWNTIME: f32 = 0.1;
        let mut dry = self.legs.charge(legs, Some(DOWNTIME), now).1;
        let (unpaid, spent) = self.arms.charge(arms, Some(DOWNTIME), now);
        if unpaid > 0.0 {
            dry |= self.legs.charge(arms, Some(DOWNTIME), now).1;
        }
        if spent {
            self.arms_ran_dry();
        }
        if dry {
            self.legs_ran_dry();
        }
    }

    /// The stance class, as the stance's target and the prone flag give
    /// it whenever either is set.
    pub fn set_pose(&mut self, prone: bool, pose_level: f32) {
        let pose = match (prone, pose_level < 0.11) {
            (true, _) => 0,
            (false, true) => 1,
            (false, false) => 2,
        };
        if pose != self.pose {
            self.pose = pose;
            self.aim_cost = None;
        }
    }

    /// What a running drain costs a second.
    fn cost(&self, drain: Drain) -> f32 {
        let s = &self.settings;
        match drain {
            Drain::Sprint => (s.overweight + s.drain_factor()) * s.sprint_drain,
            Drain::Walk => match self.pose {
                2 if s.walk_overweight > 0.0 => lerp(s.walk[0], s.walk[1], s.walk_overweight),
                1 => lerp(s.crouch[0], s.crouch[1], s.overweight),
                _ => 0.0,
            },
            // The figure kept, or a fresh one where none is.
            Drain::Aim => self.aim_cost.unwrap_or_else(|| {
                self.aim_mass.max(0.0).sqrt() * s.aim_drain * s.arms_aim * s.aim_by_pose[self.pose] * (1.0 - s.strength_aim) * s.drain_factor()
            }),
            Drain::OverweightIdle => s.restoration * s.refill_factor(self.pose) + s.overweight_by_pose[self.pose],
            Drain::Crawl => s.prone_cost,
            // Dearer the emptier the legs.
            Drain::HoldBreath => s.hold_breath * lerp(s.hold_breath_by_legs[0], s.hold_breath_by_legs[1], self.legs.level()),
        }
    }

    /// Asks for the sprint to be paid for, or stops paying
    /// (`Physical.Sprint`). A start is refused on legs at or under the
    /// threshold, lying down and fully overweight; one that is taken is
    /// charged for coming too soon after the last. Gives back whether a
    /// sprint is now being paid for.
    pub fn sprint(&mut self, on: bool, prone: bool, now: f32) -> bool {
        if on == self.sprinting() {
            return on;
        }
        let start = on && (!self.holds_back || self.legs.current > EXHAUSTED) && !prone && self.settings.overweight < 1.0 && self.health_lets_sprint;
        if start {
            self.legs.run(Drain::Sprint);
            self.oxygen.run(Drain::Sprint);
            // A start within two seconds of the last costs half a point
            // more each time, up to five.
            self.spam = if self.spam_until < now { 0.0 } else { (self.spam + 0.5).min(5.0) };
            self.spam_until = now + 2.0;
            if self.legs.charge(self.spam, Some(0.0), now).1 {
                self.legs_ran_dry();
            }
        } else {
            self.legs.stop(Drain::Sprint);
            self.oxygen.stop(Drain::Sprint);
        }
        self.sprinting()
    }

    /// The legs at nothing: whatever needs them stops (unless spent legs
    /// hold nothing back).
    fn legs_ran_dry(&mut self) {
        if !self.holds_back {
            return;
        }
        self.legs.stop(Drain::Sprint);
        self.oxygen.stop(Drain::Sprint);
    }

    /// A drain run on the arms is run on the legs and the oxygen too once
    /// the arms are spent.
    fn arms_ran_dry(&mut self) {
        if self.arms.runs(Drain::Aim) {
            self.legs.run(Drain::Aim);
            self.oxygen.run(Drain::Aim);
        }
    }

    /// A jump begun (`Physical.OnJump`).
    pub fn jump(&mut self, now: f32) {
        let s = &self.settings;
        let cost = (s.overweight + s.drain_factor()) * s.jump_cost * (1.0 - s.endurance_jump);
        if self.legs.charge(cost, Some(1.0), now).1 {
            self.legs_ran_dry();
        }
        self.oxygen.charge(cost, Some(1.0), now);
    }

    /// The prone flag dropped (`Physical.StandUp`): the legs, the arms and
    /// the oxygen each pay, and the legs pay again whatever the arms
    /// could not.
    pub fn stand_up(&mut self, now: f32) {
        let s = &self.settings;
        let cost = lerp(s.standup[0], s.standup[1], s.overweight);
        // Fully overweight, this one lets the pools refill regardless.
        let downtime = (s.overweight < 1.0).then_some(1.5);
        let mut dry = self.legs.charge(cost, downtime, now).1;
        let (unpaid, spent) = self.arms.charge(cost, downtime, now);
        if unpaid > 0.0 {
            dry |= self.legs.charge(cost, downtime, now).1;
        }
        if spent {
            self.arms_ran_dry();
        }
        if dry {
            self.legs_ran_dry();
        }
        self.oxygen.charge(cost, downtime, now);
    }

    /// The smoothed stance rose by `rise` this frame
    /// (`MovementContext.SmoothPoseLevel`): the rise is tallied, and each
    /// whole three notches of it in the tally is charged as a third of a
    /// notch and takes a third of a notch off the tally.
    pub fn stance_raised(&mut self, rise: f32, now: f32) {
        let s = &self.settings;
        self.pose_rise += rise;
        let thirds = (self.pose_rise / s.delta_level / 3.0) as i32;
        if thirds > 0 {
            let thirds = thirds as f32;
            self.pose_rise -= thirds * s.delta_level / 3.0;
            let cost = lerp(s.pose_notch[0], s.pose_notch[1], s.overweight) * thirds / 3.0;
            let downtime = (s.overweight < 1.0).then_some(0.5);
            if self.legs.charge(cost, downtime, now).1 {
                self.legs_ran_dry();
            }
        }
    }

    /// Whether the movement state is the walking one, and the crawling
    /// one: the walk's drain runs in the first, the crawl's in the second
    /// for a fully overweight character.
    pub fn moving(&mut self, walking: bool, crawling: bool) {
        match walking {
            true => self.legs.run(Drain::Walk),
            false => self.legs.stop(Drain::Walk),
        }
        let pools = [&mut self.legs, &mut self.arms, &mut self.oxygen];
        if crawling && self.settings.overweight >= 1.0 {
            pools.into_iter().for_each(|pool| pool.run(Drain::Crawl));
        } else if !crawling {
            pools.into_iter().for_each(|pool| pool.stop(Drain::Crawl));
        }
    }

    /// Holds the breath or lets it go (`Physical.HoldBreath`). Legs under
    /// the threshold do not hold it, nor does oxygen at or under it; and
    /// asked to on such legs nothing happens at all, a breath already held
    /// stays held. Gives back whether it is held now.
    pub fn hold_breath(&mut self, on: bool) -> bool {
        if !(self.legs_spent() && on) {
            if on && (!self.holds_back || self.oxygen.current > EXHAUSTED) {
                self.oxygen.run(Drain::HoldBreath);
            } else {
                self.oxygen.stop(Drain::HoldBreath);
            }
        }
        self.holding_breath()
    }

    pub fn holding_breath(&self) -> bool {
        self.oxygen.runs(Drain::HoldBreath)
    }

    /// How rested the hands count as for the weapon's breathing, 0.05 to
    /// 1: the arms' level, or the lower of the arms' and the legs' where
    /// the settings have exhaustion shake the hands (`BreathEffector`'s
    /// `StaminaLevel`; a broken arm, which caps it lower, is not here).
    pub fn breath_stamina(&self) -> f32 {
        /// The least the level counts as (`EnergyLowerLimit`).
        const LEAST: f32 = 0.05;
        if !self.holds_back {
            return 1.0;
        }
        let level = if self.settings.legs_shake_hands { self.arms.level().min(self.legs.level()) } else { self.arms.level() };
        level.clamp(LEAST, 1.0)
    }

    /// How much breath the character has for the same, 0 to 1: the
    /// oxygen's level, or the lower of that and the legs' where the
    /// settings have exhaustion deepen the breath (`OxygenLevel`).
    pub fn breath_oxygen(&self) -> f32 {
        if !self.holds_back {
            return 1.0;
        }
        if self.settings.legs_deepen_breath { self.oxygen.level().min(self.legs.level()) } else { self.oxygen.level() }
    }

    /// The sights up or down (`Physical.Aim`). The game calls this when
    /// the sights change and when the weapon is taken stock of again; a
    /// host may call it every frame, and only those two changes have the
    /// cost worked out afresh.
    pub fn aim(&mut self, on: bool) {
        if self.aim_taken != (on, self.aim_mass) {
            self.aim_taken = (on, self.aim_mass);
            self.aim_cost = None;
        }
        if on && self.aim_mass > 0.0 {
            self.arms.run(Drain::Aim);
            if self.arms.current <= 0.0 {
                self.arms_ran_dry();
            }
        } else {
            for pool in [&mut self.legs, &mut self.arms, &mut self.oxygen] {
                pool.stop(Drain::Aim);
            }
        }
    }

    /// One frame of `dt` seconds by the player's clock, with the engine's
    /// clock at `now`: the legs, the arms and the oxygen in that order.
    pub fn update(&mut self, dt: f32, now: f32) {
        // Switched off, nothing is paid for: the pools are full again,
        // whatever the frame's charges took.
        if !self.drains {
            for pool in [&mut self.legs, &mut self.arms, &mut self.oxygen] {
                pool.current = pool.capacity;
            }
            return;
        }
        self.aim_cost = Some(self.cost(Drain::Aim));
        let costs = [Drain::Sprint, Drain::Walk, Drain::Aim, Drain::OverweightIdle, Drain::Crawl, Drain::HoldBreath]
            .map(|d| (d, self.cost(d)));
        let s = &self.settings;
        let refills = [
            s.restoration * s.refill_factor(self.pose),
            s.hands_restoration * s.refill_factor(self.pose),
            s.oxygen_restoration * (1.0 + s.endurance_restoration),
        ];
        if self.legs.work(&costs, refills[0], dt, now) {
            self.legs_ran_dry();
        }
        if self.arms.work(&costs, refills[1], dt, now) {
            self.arms_ran_dry();
        }
        if self.oxygen.work(&costs, refills[2], dt, now) && self.holds_back {
            // Out of oxygen, the breath is let go.
            self.oxygen.stop(Drain::HoldBreath);
        }
        // The tally of spent legs: under the threshold it runs up, and past
        // its figure health is given a fatigue and it starts again;
        // otherwise it falls.
        let [enough, falls] = self.settings.fatigue;
        if self.legs.exhausted() {
            self.fatigue += dt;
            if self.fatigue > enough {
                self.fatigues += 1;
                self.fatigue = 0.0;
            }
        } else if self.fatigue > 0.0 {
            self.fatigue -= falls * dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This install's figures, as far as these tests use them.
    fn settings() -> StaminaSettings {
        let flat = Graph { lower_left: 1.0, lower_right: 1.0, left_plateau: 1.0, right_plateau: 1.0, right_limit: 1.0, zero: 1.0 };
        StaminaSettings {
            capacity: 100.0,
            restoration: 4.5,
            hands_capacity: 80.0,
            hands_restoration: 2.0,
            oxygen_capacity: 350.0,
            oxygen_restoration: 4.0,
            sprint_drain: 4.0,
            jump_cost: 14.0,
            aim_drain: 1.0,
            aim_by_pose: [0.15, 0.75, 1.0],
            restoration_by_pose: [1.45, 1.25, 1.0],
            overweight_by_pose: [-3.0, -2.0, 0.0],
            standup: [10.0, 20.0],
            pose_notch: [1.1, 3.0],
            walk: [0.2, 0.5],
            crouch: [0.17, 0.7],
            prone_cost: 1.0,
            vault_legs: [5.0, 20.0],
            vault_hand: [5.0, 16.0],
            climb_legs: [0.0, 20.0],
            climb_hand: [8.0, 20.0],
            climb_hands: [12.0, 25.0],
            hold_breath: 4.0,
            hold_breath_by_legs: [2.0, 1.0],
            legs_shake_hands: true,
            legs_deepen_breath: false,
            delta_level: 0.166,
            overweight: 0.0,
            walk_overweight: 0.0,
            endurance_capacity: 0.0,
            endurance_restoration: 0.0,
            endurance_jump: 0.0,
            endurance_breath: 0.0,
            strength_aim: 0.0,
            skill_buffs: [0.5, 0.5, 0.3, 1.0, 0.2],
            energy: 1.0,
            hydration: 1.0,
            energy_graph: flat,
            hydration_graph: flat,
            health: 1.0,
            arms_aim: 1.0,
            fatigue: [15.0, 0.42],
        }
    }

    const DT: f32 = 1.0 / 120.0;

    fn run(stamina: &mut Stamina, now: &mut f32, seconds: f32) {
        for _ in 0..(seconds / DT).round() as usize {
            *now += DT;
            stamina.update(DT, *now);
        }
    }

    #[test]
    fn a_sprint_drains_the_legs_and_ends_when_they_are_spent() {
        let mut stamina = Stamina::new(settings());
        let mut now = 10.0;
        assert!(stamina.sprint(true, false, now));
        run(&mut stamina, &mut now, 5.0);
        assert!((stamina.legs.current - 80.0).abs() < 0.05, "{}", stamina.legs.current);
        assert!((stamina.oxygen.current - 330.0).abs() < 0.05, "the oxygen pays as well: {}", stamina.oxygen.current);
        // Empty 25 s in.
        run(&mut stamina, &mut now, 20.1);
        assert_eq!(stamina.legs.current, 0.0);
        assert!(!stamina.sprinting(), "spent legs end the sprint");
        assert!(!stamina.sprint(true, false, now), "and do not start another");
        // The pause is the sprint's second and a second more for an empty
        // pool, from the sprint's last frame; then 4.5 a second.
        run(&mut stamina, &mut now, 1.8);
        assert_eq!(stamina.legs.current, 0.0);
        run(&mut stamina, &mut now, 3.6);
        assert!(stamina.legs.current > EXHAUSTED && stamina.legs.current < 17.0, "{}", stamina.legs.current);
        assert!(stamina.sprint(true, false, now), "more than the threshold starts one again");
    }

    /// The features' two switches (`crate::Systems`): with the stamina off
    /// nothing is paid for; with exhaustion off the pools are paid from as
    /// ever, and spent ones hold nothing back.
    #[test]
    fn the_stamina_and_exhaustion_switched_off() {
        let mut stamina = Stamina::new(settings());
        stamina.drains = false;
        let mut now = 10.0;
        assert!(stamina.sprint(true, false, now));
        stamina.jump(now);
        run(&mut stamina, &mut now, 30.0);
        assert_eq!([stamina.legs.current, stamina.oxygen.current], [stamina.legs.capacity, stamina.oxygen.capacity]);
        assert!(stamina.sprinting());

        let mut stamina = Stamina::new(settings());
        stamina.holds_back = false;
        let mut now = 10.0;
        assert!(stamina.sprint(true, false, now));
        run(&mut stamina, &mut now, 30.0);
        assert_eq!(stamina.legs.current, 0.0, "the legs pay as ever");
        assert!(stamina.sprinting(), "and the sprint goes on");
        assert!(stamina.can_jump() && stamina.can_vault(true) && !stamina.exhausted() && !stamina.legs_spent());
        assert!(stamina.hold_breath(true), "a breath is held on spent legs");
        assert_eq!((stamina.breath_stamina(), stamina.breath_oxygen()), (1.0, 1.0));
    }

    #[test]
    fn sprints_started_in_quick_succession_cost_more_each_time() {
        let mut stamina = Stamina::new(settings());
        let mut now = 10.0;
        let mut costs = Vec::new();
        for _ in 0..4 {
            let before = stamina.legs.current;
            stamina.sprint(true, false, now);
            costs.push(before - stamina.legs.current);
            stamina.sprint(false, false, now);
            now += 0.5;
        }
        assert_eq!(costs, [0.0, 0.5, 1.0, 1.5]);
        now += 2.5;
        let before = stamina.legs.current;
        stamina.sprint(true, false, now);
        assert_eq!(before - stamina.legs.current, 0.0, "after two quiet seconds a start is free again");
    }

    #[test]
    fn a_jump_needs_more_than_the_threshold_and_pauses_the_refill() {
        let mut stamina = Stamina::new(settings());
        let mut now = 10.0;
        stamina.jump(now);
        assert_eq!(stamina.legs.current, 86.0);
        assert_eq!(stamina.oxygen.current, 336.0);
        // A second, and 0.14 more for a pool 14% down.
        run(&mut stamina, &mut now, 1.13);
        assert_eq!(stamina.legs.current, 86.0);
        run(&mut stamina, &mut now, 1.0);
        assert!((stamina.legs.current - 90.4).abs() < 0.2, "{}", stamina.legs.current);
        stamina.legs.current = 15.0;
        assert!(!stamina.can_jump());
        stamina.legs.current = 15.1;
        assert!(stamina.can_jump());
    }

    #[test]
    fn raising_the_stance_is_charged_by_the_third_of_a_notch() {
        let mut stamina = Stamina::new(settings());
        // Nothing until three notches' rise is in the tally, and then a
        // third of a notch's cost for every third of a notch more.
        stamina.stance_raised(0.49, 10.0);
        assert_eq!(stamina.legs.current, 100.0);
        stamina.stance_raised(0.01, 10.0);
        assert!((stamina.legs.current - (100.0 - 1.1 / 3.0)).abs() < 1e-4);
        stamina.stance_raised(0.05, 10.0);
        assert!((stamina.legs.current - (100.0 - 1.1 / 3.0)).abs() < 1e-4);
        stamina.stance_raised(0.01, 10.0);
        assert!((stamina.legs.current - (100.0 - 2.2 / 3.0)).abs() < 1e-4);
    }

    #[test]
    fn aiming_drains_the_arms_and_then_the_legs() {
        let mut stamina = Stamina::new(settings());
        stamina.aim_mass = 4.0;
        let mut now = 10.0;
        stamina.aim(true);
        run(&mut stamina, &mut now, 10.0);
        assert!((stamina.arms.current - 60.0).abs() < 0.05, "two a second standing: {}", stamina.arms.current);
        assert_eq!(stamina.legs.current, 100.0);
        run(&mut stamina, &mut now, 31.0);
        assert_eq!(stamina.arms.current, 0.0);
        assert!(stamina.legs.current < 99.0, "spent arms pass the cost on to the legs: {}", stamina.legs.current);
        stamina.aim(false);
        let legs = stamina.legs.current;
        run(&mut stamina, &mut now, 3.0);
        assert!(stamina.legs.current > legs && stamina.arms.current > 0.0);
    }

    #[test]
    fn thirst_reaches_the_cost_of_aiming_only_when_it_is_worked_out_again() {
        let mut settings = settings();
        // Thirst doubles every drain at nothing drunk, from half down.
        settings.hydration_graph = Graph { lower_left: 2.0, left_plateau: 0.5, ..settings.hydration_graph };
        let mut stamina = Stamina::new(settings);
        stamina.aim_mass = 4.0;
        let mut now = 10.0;
        let spent = |stamina: &mut Stamina, now: &mut f32| {
            let before = stamina.arms.current;
            run(stamina, now, 1.0);
            before - stamina.arms.current
        };
        stamina.aim(true);
        assert!((spent(&mut stamina, &mut now) - 2.0).abs() < 1e-3);
        // Thirstier with the sights up: the cost stays as it was taken.
        stamina.settings.hydration = 0.25;
        stamina.aim(true);
        assert!((spent(&mut stamina, &mut now) - 2.0).abs() < 1e-3, "the sights were up already");
        // Down and up again takes it afresh; so does a change of stance.
        stamina.aim(false);
        stamina.aim(true);
        assert!((spent(&mut stamina, &mut now) - 3.0).abs() < 1e-3);
        stamina.settings.hydration = 1.0;
        assert!((spent(&mut stamina, &mut now) - 3.0).abs() < 1e-3);
        stamina.set_pose(false, 0.0);
        assert!((spent(&mut stamina, &mut now) - 1.5).abs() < 1e-3, "crouched, three quarters of two");
    }

    #[test]
    fn a_held_breath_costs_oxygen_and_is_let_go_when_it_runs_out() {
        let mut stamina = Stamina::new(settings());
        let mut now = 10.0;
        assert!(stamina.hold_breath(true));
        run(&mut stamina, &mut now, 5.0);
        assert!((stamina.oxygen.current - 330.0).abs() < 0.05, "four a second on full legs: {}", stamina.oxygen.current);
        // Emptier legs make it dearer: twice as dear on empty ones.
        stamina.legs.current = 50.0;
        stamina.legs.hold_refill_until(1e9);
        let before = stamina.oxygen.current;
        run(&mut stamina, &mut now, 1.0);
        assert!((before - stamina.oxygen.current - 6.0).abs() < 0.05, "{}", before - stamina.oxygen.current);
        // Legs under the threshold: asking again changes nothing, and a
        // breath let go is not taken again.
        stamina.legs.current = 10.0;
        assert!(stamina.hold_breath(true), "a breath already held stays held");
        assert!(!stamina.hold_breath(false));
        assert!(!stamina.hold_breath(true), "spent legs do not hold one");
        // It ends by itself when the oxygen is gone.
        stamina.legs.current = 100.0;
        stamina.oxygen.current = 16.0;
        assert!(stamina.hold_breath(true));
        run(&mut stamina, &mut now, 5.0);
        assert!(!stamina.holding_breath());
        assert!(!stamina.hold_breath(true), "nor does oxygen at the threshold or under");
        // What the weapon's breathing goes by.
        stamina.arms.current = 40.0;
        stamina.legs.current = 25.0;
        assert!((stamina.breath_stamina() - 0.25).abs() < 1e-6, "the lower of arms and legs here");
        stamina.arms.current = 0.0;
        assert_eq!(stamina.breath_stamina(), 0.05);
    }

    #[test]
    fn getting_up_costs_legs_arms_and_oxygen() {
        let mut stamina = Stamina::new(settings());
        stamina.stand_up(10.0);
        assert_eq!((stamina.legs.current, stamina.arms.current, stamina.oxygen.current), (90.0, 70.0, 340.0));
        // Arms that cannot pay it all leave the legs to pay it again.
        stamina.arms.current = 4.0;
        stamina.stand_up(10.0);
        assert_eq!((stamina.legs.current, stamina.arms.current), (70.0, 0.0));
    }

    #[test]
    fn a_crouched_walk_costs_and_an_upright_one_does_not() {
        let mut stamina = Stamina::new(settings());
        let mut now = 10.0;
        stamina.moving(true, false);
        run(&mut stamina, &mut now, 10.0);
        assert_eq!(stamina.legs.current, 100.0);
        stamina.set_pose(false, 0.0);
        run(&mut stamina, &mut now, 10.0);
        assert!((stamina.legs.current - 98.3).abs() < 0.01, "{}", stamina.legs.current);
        stamina.moving(false, false);
        run(&mut stamina, &mut now, 1.0);
        assert_eq!(stamina.legs.current, 100.0, "and comes back at the crouch's faster rate");
    }
}
