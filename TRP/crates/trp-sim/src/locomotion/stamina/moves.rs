//! The costs of vaulting, climbing, sprinting, jumping, standing up and moving.

use super::*;

impl Stamina {
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
    pub(super) fn cost(&self, drain: Drain) -> f32 {
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
                let rate = if self.rangefinder { s.aim_rangefinder_drain } else { s.aim_drain };
                self.aim_mass.max(0.0).sqrt() * rate * s.arms_aim * s.aim_by_pose[self.pose] * (1.0 - s.strength_aim) * s.drain_factor() * self.aim_drain_by
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
    /// sprint is now being paid for. While the legs pay nothing (a
    /// berserk) every start is let through, and the cost of coming too
    /// soon is not even worked out (`Stamina.CanAdd`, `Consume`).
    pub fn sprint(&mut self, on: bool, prone: bool, now: f32) -> bool {
        if on == self.sprinting() {
            return on;
        }
        let start = on && (self.legs.forced() || ((!self.holds_back || self.legs.current > EXHAUSTED) && !prone && self.settings.overweight < 1.0 && self.health_lets_sprint));
        if start {
            self.legs.run(Drain::Sprint);
            self.oxygen.run(Drain::Sprint);
            // A start within two seconds of the last costs half a point
            // more each time, up to five.
            if !self.legs.forced() {
                self.spam = if self.spam_until < now { 0.0 } else { (self.spam + 0.5).min(5.0) };
                self.spam_until = now + 2.0;
            }
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
    pub(super) fn legs_ran_dry(&mut self) {
        if !self.holds_back {
            return;
        }
        self.legs.stop(Drain::Sprint);
        self.oxygen.stop(Drain::Sprint);
    }

    /// A drain run on the arms is run on the legs and the oxygen too once
    /// the arms are spent.
    pub(super) fn arms_ran_dry(&mut self) {
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
}
