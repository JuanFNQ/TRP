//! The marks in an animator's states that the game's logic waits for, by
//! the game's own rule for when one fires. See `docs/spec/firing.md`, "The
//! weapon's animator as a state machine".
//!
//! The animator's update only queues the marks it passes; they are
//! delivered later in the frame, each if its conditions on the animator's
//! parameters hold at that moment.

use std::sync::Arc;

use trp_data::states::StateTable;

use crate::animator::{Animator, Controller, Look, LookKind};

/// The end of a state for the marks: a state being left has its marks
/// fired up to here, and one that has played out is held here.
const ALMOST_ONE: f32 = 0.99999;
const A_BIT_OVER_ONE: f32 = 1.00001;

/// A mark as it is delivered: the handler's name and what it is given.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub name: String,
    pub int: i32,
    pub text: String,
}

/// A condition on an animator parameter that a mark needs to be delivered.
#[derive(Debug, Clone)]
struct Test {
    parameter: Option<usize>,
    /// The game's numbering: 0 a whole number, 1 a float, 2 a flag.
    kind: i32,
    /// 0 equal, 1 not equal, 2 greater, 3 less, 4 at least, 5 at most.
    mode: i32,
    float: f32,
    int: i32,
    flag: bool,
}

#[derive(Debug, Clone)]
struct Timed {
    mark: Mark,
    /// When in the state, as a fraction of it.
    time: f32,
    enabled: bool,
    tests: Vec<Test>,
}

/// Every state's marks, by machine and by the state's place in it.
pub struct MarkTable {
    by_state: Vec<Vec<Vec<Timed>>>,
}

impl MarkTable {
    pub fn new(table: &StateTable, controller: &Controller) -> Self {
        let mut by_state: Vec<Vec<Vec<Timed>>> = controller.machines.iter().map(|_| Vec::new()).collect();
        // The controller keeps each machine's states in the table's order.
        for state in &table.states {
            let Some(machine) = by_state.get_mut(state.machine) else { continue };
            let marks = state
                .events
                .iter()
                .map(|event| Timed {
                    mark: Mark { name: event.name.clone(), int: event.int_param, text: event.string_param.clone() },
                    time: event.time,
                    enabled: event.enabled,
                    tests: event
                        .conditions
                        .iter()
                        .map(|c| Test {
                            parameter: controller.parameter(&c.parameter),
                            kind: c.kind,
                            mode: c.mode,
                            float: c.float_value,
                            int: c.int_value,
                            flag: c.bool_value,
                        })
                        .collect(),
                })
                .collect();
            machine.push(marks);
        }
        Self { by_state }
    }

    /// Marks written out by hand: for each machine and each of its states
    /// in order, the marks' names and the fractions of the state they sit
    /// at. None of them has a condition.
    pub fn by_hand(by_state: &[Vec<Vec<(&str, f32)>>]) -> Self {
        let timed = |(name, time): &(&str, f32)| Timed {
            mark: Mark { name: (*name).to_owned(), int: 0, text: String::new() },
            time: *time,
            enabled: true,
            tests: Vec::new(),
        };
        Self { by_state: by_state.iter().map(|states| states.iter().map(|marks| marks.iter().map(timed).collect()).collect()).collect() }
    }

    /// The marks of a state, each with the fraction of the state it sits at.
    pub fn of(&self, machine: usize, state: usize) -> impl Iterator<Item = (&Mark, f32)> {
        self.by_state.get(machine).and_then(|states| states.get(state)).into_iter().flatten().map(|timed| (&timed.mark, timed.time))
    }
}

/// Where each state's marks have been fired up to, and what is waiting to
/// be delivered.
#[derive(Clone)]
pub struct Marks {
    table: Arc<MarkTable>,
    /// By machine and state: the point the state had reached when last
    /// looked at, and whether it looped then.
    points: Vec<Vec<(f32, bool)>>,
    queued: Vec<(usize, usize, usize)>,
}

impl Marks {
    /// Takes another's points and queue where both follow states of the
    /// same shape. Whether it could.
    pub fn take_state(&mut self, from: &Marks) -> bool {
        let same = self.points.len() == from.points.len() && self.points.iter().zip(&from.points).all(|(a, b)| a.len() == b.len());
        if same {
            self.points.clone_from(&from.points);
            self.queued.clone_from(&from.queued);
        }
        same
    }

    pub fn new(table: Arc<MarkTable>) -> Self {
        let points = table.by_state.iter().map(|states| vec![(0.0, false); states.len()]).collect();
        Self { table, points, queued: Vec::new() }
    }

    /// Queues the marks the animator's last update passed.
    pub fn collect(&mut self, animator: &Animator) {
        for look in animator.looks() {
            self.check(look);
        }
    }

    /// One look at a state, as the game's container takes it
    /// (`AnimationEventsContainer.CheckEvents`).
    fn check(&mut self, look: &Look) {
        let Some(&(point, looped)) = self.points.get(look.machine).and_then(|states| states.get(look.state)) else { return };
        let (cycle_before, cycle_now) = (point as i32, look.time as i32);
        let before = if looped || cycle_before <= 0 { point - cycle_before as f32 } else { 1.0 };
        let now = look.time - cycle_now as f32;
        let fire = |from: f32, to: f32, queued: &mut Vec<(usize, usize, usize)>| {
            for (index, timed) in self.table.by_state[look.machine][look.state].iter().enumerate() {
                if timed.time >= from && timed.time < to {
                    queued.push((look.machine, look.state, index));
                }
            }
        };
        let mut queued = std::mem::take(&mut self.queued);
        let point_now;
        if look.leaving {
            // A state being left gives up all it has left, at once.
            fire(before, ALMOST_ONE, &mut queued);
            self.points[look.machine][look.state].0 = ALMOST_ONE;
            self.queued = queued;
            return;
        } else if look.looping {
            let exit = look.kind == LookKind::Exit;
            if exit && cycle_now == 1 && (cycle_now < cycle_before || point > look.time) {
                fire(before, A_BIT_OVER_ONE, &mut queued);
            } else if cycle_before == cycle_now {
                fire(before, now, &mut queued);
            } else if cycle_before < cycle_now {
                fire(before, A_BIT_OVER_ONE, &mut queued);
                if !exit {
                    fire(0.0, now, &mut queued);
                }
            }
            point_now = look.time;
        } else {
            let (from, to) = (point.clamp(0.0, 1.0), look.time.clamp(0.0, 1.0));
            if cycle_before == 0 && cycle_now == 0 {
                if from != to {
                    fire(from, to, &mut queued);
                }
            } else if cycle_before == 0 && cycle_now > 0 && from < 1.0 {
                fire(before, A_BIT_OVER_ONE, &mut queued);
            }
            point_now = if look.kind == LookKind::Exit { 0.0 } else { look.time.clamp(0.0, ALMOST_ONE) };
        }
        self.points[look.machine][look.state] = (point_now, look.looping);
        self.queued = queued;
    }

    /// Delivers what is queued, in the order it was passed: each mark that
    /// is switched on and whose conditions hold with the animator's
    /// parameters as they now are.
    pub fn deliver(&mut self, animator: &Animator) -> Vec<Mark> {
        let queued = std::mem::take(&mut self.queued);
        queued
            .into_iter()
            .filter_map(|(machine, state, index)| {
                let timed = &self.table.by_state[machine][state][index];
                (timed.enabled && timed.tests.iter().all(|test| holds(test, animator))).then(|| timed.mark.clone())
            })
            .collect()
    }

    /// Delivers the next of what is queued, if any is left: the first mark
    /// in the order they were passed that is switched on and whose
    /// conditions hold with the animator's parameters as they are now.
    /// The game hands its queue out this way, one mark at a time
    /// (`AnimationEventsEmitter.EmitEvents`), so what one mark's handler
    /// sets is what the next mark's conditions are tested against.
    pub fn deliver_next(&mut self, animator: &Animator) -> Option<Mark> {
        while !self.queued.is_empty() {
            let (machine, state, index) = self.queued.remove(0);
            let timed = &self.table.by_state[machine][state][index];
            if timed.enabled && timed.tests.iter().all(|test| holds(test, animator)) {
                return Some(timed.mark.clone());
            }
        }
        None
    }

    /// Forgets what is queued and where every state had got to: for an
    /// animator that is started afresh.
    pub fn reset(&mut self) {
        self.queued.clear();
        for point in self.points.iter_mut().flatten() {
            *point = (0.0, false);
        }
    }
}

fn holds(test: &Test, animator: &Animator) -> bool {
    let Some(parameter) = test.parameter else { return false };
    let value = animator.get(parameter);
    match (test.kind, test.mode) {
        (0, 0) => value as i32 == test.int,
        (0, 1) => value as i32 != test.int,
        (0, 2) => value as i32 > test.int,
        (0, 3) => (value as i32) < test.int,
        (0, 4) => value as i32 >= test.int,
        (0, 5) => value as i32 <= test.int,
        (1, 2) => value > test.float,
        (1, 3) => value < test.float,
        (2, _) => (value != 0.0) == test.flag,
        // The game has no test for a float in any other mode, and a mark
        // with one is never delivered.
        _ => false,
    }
}
