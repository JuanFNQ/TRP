//! The stamina graphs and settings read from the install, with their skill factors.

use super::*;

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
    /// What a fast weapon switch costs the arms (`WeaponFastSwitchConsumption`:
    /// 15 in the install; stamina spec, "A drain").
    pub fast_switch_cost: f32,
    /// A melee stroke's speed with the hands exhausted
    /// (`ExhaustedMeleeSpeed`; 1 otherwise).
    pub exhausted_melee_speed: f32,
    /// A blow's damage with the hands exhausted, times this
    /// (`ExhaustedMeleeDamageMultiplier`, 0.33 in the install).
    pub exhausted_melee_damage: f32,
    pub aim_drain: f32,
    /// The aim's rate with the handheld rangefinder in hand, in place of
    /// `aim_drain` (`AimRangeFinderDrainRate`; rangefinder spec).
    pub aim_rangefinder_drain: f32,
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
    /// Endurance's two elites: the arms' capacity raised by it
    /// (`EnduranceHands`, 0.5), and the breath's capacity no longer falling
    /// with the energy (`EnduranceBreathElite`). 0 and off untrained.
    pub endurance_hands: f32,
    pub endurance_breath_elite: bool,
    /// What stimulators' buffs at work add: to the legs' and the arms'
    /// capacity (`MaxStamina`, `Physical.CapacityBuff`), and a refill of
    /// their own a second, the legs and the breath that much and the arms
    /// half (`StaminaRate`, `RestoreRateBuff`, `Stamina.BuffRestoration`;
    /// health spec, "Buffs"). Nothing without.
    pub buff_capacity: f32,
    pub buff_restoration: f32,
    /// The skills' buffs at level 50, in that order, for `set_skills`.
    pub(super) skill_buffs: [f32; 5],

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
        Self::new_limited(config, weight, delta_level, crate::base::settings::WeightLimits::BASE)
    }

    /// As `new`, with the overweight limits as `limits` make them: times
    /// the carrying factor Strength gives (1 untrained), or out of reach
    /// with encumbrance off (`settings::WeightLimits`).
    pub fn new_limited(config: &Json, weight: f32, delta_level: f32, limits: crate::base::settings::WeightLimits) -> Result<Self> {
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
        let within = |name: &str| -> Result<f32> {
            range(name).map(|r| {
                let [low, high] = limits.overweight_limit(r);
                inverse_lerp(low, high, weight)
            })
        };
        Ok(Self {
            capacity: number("Capacity")?,
            restoration: number("BaseRestorationRate")?,
            hands_capacity: number("HandsCapacity")?,
            hands_restoration: number("HandsRestoration")?,
            oxygen_capacity: number("OxygenCapacity")?,
            oxygen_restoration: number("OxygenRestoration")?,
            sprint_drain: number("SprintDrainRate")?,
            jump_cost: number("JumpConsumption")?,
            fast_switch_cost: number("WeaponFastSwitchConsumption")?,
            exhausted_melee_speed: number("ExhaustedMeleeSpeed")?,
            exhausted_melee_damage: number("ExhaustedMeleeDamageMultiplier")?,
            aim_drain: number("AimDrainRate")?,
            aim_rangefinder_drain: number("AimRangeFinderDrainRate")?,
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
            endurance_hands: 0.0,
            endurance_breath_elite: false,
            buff_capacity: 0.0,
            buff_restoration: 0.0,
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

    /// Sets what the Endurance and Strength skills do to stamina from the
    /// skills' buffs (`skills::Buffs`), elites and all: the host's way (the
    /// replay's is `set_skills`, by the levels a recording shows).
    pub fn set_skill_buffs(&mut self, buffs: &crate::condition::skills::Buffs) {
        self.endurance_capacity = buffs.endurance_capacity;
        self.endurance_restoration = buffs.endurance_restoration;
        self.endurance_jump = buffs.endurance_jump;
        self.endurance_breath = buffs.endurance_breath;
        self.strength_aim = buffs.strength_aim;
        self.endurance_hands = buffs.endurance_hands;
        self.endurance_breath_elite = buffs.endurance_breath_elite;
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

    pub(super) fn drain_factor(&self) -> f32 {
        self.hydration_graph.at(self.hydration)
    }

    pub(super) fn refill_factor(&self, pose: usize) -> f32 {
        self.restoration_by_pose[pose] * self.energy_graph.at(self.energy) * (1.0 + self.endurance_restoration)
            / self.drain_factor()
    }

    pub fn legs_capacity(&self) -> f32 {
        (self.capacity * (1.0 + self.endurance_capacity) + self.buff_capacity).max(1.0)
    }

    pub fn arms_capacity(&self) -> f32 {
        (self.hands_capacity * (1.0 + self.endurance_hands) + self.buff_capacity).max(1.0)
    }

    pub fn oxygen_capacity(&self) -> f32 {
        let energy = if self.endurance_breath_elite { 1.0 } else { self.energy.clamp(0.0, 1.0) };
        self.oxygen_capacity * (0.5 + 0.5 * energy * (2.0 * self.endurance_breath + self.health))
    }
}
