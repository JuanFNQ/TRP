//! A magazine loaded or unloaded a round at a time (inventory spec,
//! "Loading and unloading a magazine"; `LoadMagazineProcess`,
//! `UnloadMagazineProcess`): each round waits its time, whole milliseconds
//! rounded up as the engine's timer has them, and moves on the first frame
//! after the wait ends; the next wait begins on that frame. What moves is
//! the inventory's (`inventory::load_round`, `unload_round`); this says
//! when. A magazine checked (`CheckMagazineProcess`) is one wait on the
//! engine's frame clock, not rounded (`WaitForSeconds`), after which its
//! count is known.

use crate::inventory::Id;

/// What the process does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Work {
    /// Rounds from a stack into the magazine: how many are still to go.
    Load { source: Id, left: u32 },
    /// The magazine's rounds out, until it is empty or a round finds no
    /// place.
    Unload,
    /// The magazine looked into: one wait, and it is checked.
    Check,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MagazineProcess {
    pub magazine: Id,
    pub work: Work,
    /// One round's time as begun with, in seconds.
    first: f32,
    /// The wait now, in seconds (shorter each round at the skill's elite
    /// level).
    wait: f32,
    /// Whether the skill is at its elite level, and the share of the first
    /// wait taken off each later one then (`LoadTimeSpeedProgress`).
    elite: Option<f32>,
    /// How long the present wait has run.
    waited: f32,
}

impl MagazineProcess {
    /// One round's time: the base (`BaseLoadTime` or `BaseUnloadTime`)
    /// times what is left of a hundred per cent after the skill's buff and
    /// with the magazine's modifier (`PlayerInventoryController.
    /// LoadMagazine`, `UnloadMagazine`).
    pub fn round_time(base: f32, skill_buff: f32, modifier: f32) -> f32 {
        base * (100.0 - skill_buff + modifier) / 100.0
    }

    /// A load of `count` rounds from `source` into `magazine`.
    pub fn load(magazine: Id, source: Id, count: u32, round_time: f32, elite: Option<f32>) -> Self {
        Self { magazine, work: Work::Load { source, left: count }, first: round_time, wait: round_time, elite, waited: 0.0 }
    }

    /// An unload of `magazine`.
    pub fn unload(magazine: Id, round_time: f32, elite: Option<f32>) -> Self {
        Self { magazine, work: Work::Unload, first: round_time, wait: round_time, elite, waited: 0.0 }
    }

    /// A check of `magazine`, taking `time`: `BaseCheckTime` times what is
    /// left of a hundred per cent after Mag Drills' check speed and with the
    /// magazine's `CheckTimeModifier` (`PlayerInventoryController.
    /// InventoryCheckMagazine`; `round_time` is the same sum).
    pub fn check(magazine: Id, time: f32) -> Self {
        Self { magazine, work: Work::Check, first: time, wait: time, elite: None, waited: 0.0 }
    }

    /// The wait as the engine's timer has it: whole milliseconds, rounded
    /// up (`Task.Delay(Mathf.CeilToInt(time * 1000))`); a check's as the
    /// frame clock has it, whole (`WaitForSeconds`).
    fn timer(&self) -> f32 {
        match self.work {
            Work::Check => self.wait,
            _ => (self.wait * 1000.0).ceil() / 1000.0,
        }
    }

    /// Moves the clock on by a frame: whether a round moves on this frame.
    /// The next wait begins with it, and at the skill's elite level is the
    /// shorter by the progress's share of the first, down to 40 per cent of
    /// it.
    pub fn step(&mut self, dt: f32) -> bool {
        self.waited += dt;
        // A hair's allowance for a sum of frame lengths that ends on the
        // wait's own millisecond.
        if self.waited + 1e-6 < self.timer() {
            return false;
        }
        self.waited = 0.0;
        if let Some(progress) = self.elite {
            self.wait = (self.wait - self.first * progress / 100.0).clamp(self.first * 0.4, 10.0);
        }
        true
    }

    /// A round has moved: whether the work is done (a load's last round in;
    /// an unload goes on until the magazine is empty, which its caller
    /// sees).
    pub fn moved(&mut self) -> bool {
        match &mut self.work {
            Work::Load { left, .. } => {
                *left = left.saturating_sub(1);
                *left == 0
            }
            Work::Unload => false,
            Work::Check => true,
        }
    }

    /// A round's wait now, in seconds.
    pub fn round_wait(&self) -> f32 {
        self.wait
    }

    /// The rounds' stack a load takes from.
    pub fn source(&self) -> Option<Id> {
        match self.work {
            Work::Load { source, .. } => Some(source),
            Work::Unload | Work::Check => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frames each round of a process takes, at so many frames a
    /// second.
    fn frames_apart(mut process: MagazineProcess, rate: f32, rounds: usize) -> Vec<u32> {
        let mut apart = Vec::new();
        let mut frames = 0;
        while apart.len() < rounds {
            frames += 1;
            if process.step(1.0 / rate) {
                apart.push(frames);
                frames = 0;
            }
        }
        apart
    }

    /// The recorded course (`courses2`, `3-menu-magazine`, 120 frames a
    /// second, Mag Drills at level 40: a buff of 24, the AKM's magazine's
    /// modifier 0): an unload's round every 28 frames (0.2334 s), a load's
    /// every 78 (0.650 s).
    #[test]
    fn rounds_come_a_wait_apart_rounded_up_to_the_frame() {
        let unload = MagazineProcess::round_time(0.3, 24.0, 0.0);
        let load = MagazineProcess::round_time(0.85, 24.0, 0.0);
        assert!((unload - 0.228).abs() < 1e-6 && (load - 0.646).abs() < 1e-6);
        assert!(frames_apart(MagazineProcess::unload(1, unload, None), 120.0, 30).iter().all(|frames| *frames == 28));
        assert!(frames_apart(MagazineProcess::load(1, 2, 30, load, None), 120.0, 30).iter().all(|frames| *frames == 78));
        // Untrained, the base times themselves: 0.3 and 0.85 s.
        assert_eq!(frames_apart(MagazineProcess::unload(1, 0.3, None), 60.0, 3), vec![18, 18, 18]);
        assert_eq!(frames_apart(MagazineProcess::load(1, 2, 3, 0.85, None), 60.0, 3), vec![51, 51, 51]);
    }

    /// A check waits its time whole, on the frame clock (`WaitForSeconds`):
    /// untrained, the install's 3 s (`BaseCheckTime`, a magazine of
    /// modifier 0) is 360 frames at 120 a second, and a time between two
    /// milliseconds is not rounded up as a round's is. It is done with its
    /// one wait.
    #[test]
    fn a_check_is_one_wait_on_the_frame_clock() {
        assert_eq!(frames_apart(MagazineProcess::check(1, MagazineProcess::round_time(3.0, 0.0, 0.0)), 120.0, 1), vec![360]);
        let steps = |mut process: MagazineProcess| (1..).find(|_| process.step(0.0001)).unwrap();
        assert_eq!(steps(MagazineProcess::check(1, 0.0101)), 101);
        assert_eq!(steps(MagazineProcess::unload(1, 0.0101, None)), 110);
        assert!(MagazineProcess::check(1, 3.0).moved());
    }

    /// At the elite level each wait after the first is shorter by the
    /// progress's share of the first, to no less than 40 per cent of it.
    #[test]
    fn the_elite_level_shortens_each_wait() {
        let mut process = MagazineProcess::load(1, 2, 100, 1.0, Some(10.0));
        let mut waits = Vec::new();
        for _ in 0..8 {
            waits.push(process.timer());
            while !process.step(0.001) {}
        }
        assert_eq!(waits[..3], [1.0, 0.9, 0.8]);
        assert_eq!(waits[7], 0.4);
        let mut done = MagazineProcess::load(1, 2, 2, 1.0, None);
        assert!(!done.moved() && done.moved());
    }
}
