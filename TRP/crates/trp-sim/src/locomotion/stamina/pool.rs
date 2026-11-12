//! The drains and the pools of legs, arms and oxygen.

use super::*;

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
    pub(super) fn downtime(self) -> Option<f32> {
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
    pub(super) drains: Vec<Drain>,
    /// How many times the game's pool would have said it changed
    /// (`Stamina.OnChanged`: a drain started or stopped, a cost taken off
    /// at once), which raises the game's stance panel.
    changes: u32,
    /// Nothing is paid (`Stamina.ForceMode`, a berserk's: the legs' and
    /// the arms'): no cost comes off, its drains run unpaid, every start
    /// it is asked about is let through, and it refills as ever.
    force: bool,
}

impl Pool {
    pub(super) fn new(capacity: f32) -> Self {
        Self { current: capacity, capacity, no_refill_until: 0.0, drains: Vec::new(), changes: 0, force: false }
    }

    /// Whether nothing is paid (`force`).
    pub fn forced(&self) -> bool {
        self.force
    }

    /// Nothing paid from now on, or paid again; either way the refill's
    /// pause is over (the `ForceMode` setter's `DisableRestoration = 0`).
    pub(super) fn set_force(&mut self, on: bool) {
        self.force = on;
        self.no_refill_until = 0.0;
    }

    /// Whether a start that needs more than `threshold` may go
    /// (`Stamina.CanAdd`, before the consumption's own conditions):
    /// always while nothing is paid.
    pub(super) fn can_add(&self, threshold: f32) -> bool {
        self.force || self.current > threshold
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

    pub(super) fn runs(&self, drain: Drain) -> bool {
        self.drains.contains(&drain)
    }

    pub(super) fn run(&mut self, drain: Drain) {
        if !self.runs(drain) {
            self.drains.push(drain);
            self.changes = self.changes.wrapping_add(1);
        }
    }

    pub(super) fn stop(&mut self, drain: Drain) {
        if self.runs(drain) {
            self.drains.retain(|d| *d != drain);
            self.changes = self.changes.wrapping_add(1);
        }
    }

    /// Takes a cost off at once. Gives back what the pool could not pay,
    /// and whether this emptied it. Nothing while nothing is paid.
    pub(super) fn charge(&mut self, cost: f32, downtime: Option<f32>, now: f32) -> (f32, bool) {
        self.changes = self.changes.wrapping_add(1);
        if self.force {
            return (0.0, false);
        }
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

    /// One frame: the running drains come off (none while nothing is
    /// paid), then the refill goes on if its pause is over. Says whether
    /// the pool ran dry.
    pub(super) fn work(&mut self, costs: &[(Drain, f32)], refill: f32, buff: f32, dt: f32, now: f32) -> bool {
        let mut total = 0.0;
        let mut pause: Option<f32> = None;
        for (drain, cost) in costs.iter().filter(|(drain, _)| !self.force && self.runs(*drain)) {
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
        // A stimulator's refill comes first, whether the pool's own is held
        // off or not (`Stamina.Update`).
        if self.current < self.capacity && buff != 0.0 {
            self.current += buff * dt;
        }
        if self.current < self.capacity && now > self.no_refill_until {
            self.current = (self.current + refill * dt).min(self.capacity);
        }
        self.current = self.current.clamp(0.0, self.capacity);
        dry
    }
}
