//! The G1 drag table and a round's precomputed trajectory.

use super::*;

/// The smallest float above nought (C#'s `float.Epsilon`), the game's test
/// for a chance that is there at all.
pub(super) const EPSILON: f32 = f32::from_bits(1);

/// The standard G1 drag table (Mach number, drag coefficient), as published
/// and as the game has it from Mach 0 to 5.
const G1: [(f32, f32); 79] = [
    (0.0, 0.2629), (0.05, 0.2558), (0.1, 0.2487), (0.15, 0.2413), (0.2, 0.2344), (0.25, 0.2278), (0.3, 0.2214), (0.35, 0.2155),
    (0.4, 0.2104), (0.45, 0.2061), (0.5, 0.2032), (0.55, 0.202), (0.6, 0.2034), (0.7, 0.2165), (0.725, 0.223), (0.75, 0.2313),
    (0.775, 0.2417), (0.8, 0.2546), (0.825, 0.2706), (0.85, 0.2901), (0.875, 0.3136), (0.9, 0.3415), (0.925, 0.3734), (0.95, 0.4084),
    (0.975, 0.4448), (1.0, 0.4805), (1.025, 0.5136), (1.05, 0.5427), (1.075, 0.5677), (1.1, 0.5883), (1.125, 0.6053), (1.15, 0.6191),
    (1.2, 0.6393), (1.25, 0.6518), (1.3, 0.6589), (1.35, 0.6621), (1.4, 0.6625), (1.45, 0.6607), (1.5, 0.6573), (1.55, 0.6528),
    (1.6, 0.6474), (1.65, 0.6413), (1.7, 0.6347), (1.75, 0.628), (1.8, 0.621), (1.85, 0.6141), (1.9, 0.6072), (1.95, 0.6003),
    (2.0, 0.5934), (2.05, 0.5867), (2.1, 0.5804), (2.15, 0.5743), (2.2, 0.5685), (2.25, 0.563), (2.3, 0.5577), (2.35, 0.5527),
    (2.4, 0.5481), (2.45, 0.5438), (2.5, 0.5397), (2.6, 0.5325), (2.7, 0.5264), (2.8, 0.5211), (2.9, 0.5168), (3.0, 0.5133),
    (3.1, 0.5105), (3.2, 0.5084), (3.3, 0.5067), (3.4, 0.5054), (3.5, 0.504), (3.6, 0.503), (3.7, 0.5022), (3.8, 0.5016),
    (3.9, 0.501), (4.0, 0.5006), (4.2, 0.4998), (4.4, 0.4995), (4.6, 0.4992), (4.8, 0.499), (5.0, 0.4988),
];

/// The speed of sound the game takes, m/s.
const SOUND: f32 = 343.0;

/// The G1 drag coefficient at a speed (`CalculateG1DragCoefficient`): the
/// table's bracket by `floor(speed / 343 / 0.05)` of its places, drawn
/// straight by speed from the place before; nought at the bottom, the last
/// past the end.
pub fn g1_drag(speed: f32) -> f32 {
    let place = ((speed / SOUND / 0.05) as f64).floor() as i64;
    if place <= 0 {
        return 0.0;
    }
    if place as usize > G1.len() - 1 {
        return G1[G1.len() - 1].1;
    }
    let place = place as usize;
    let (low, high) = (G1[place - 1].0 * SOUND, G1[place].0 * SOUND);
    let below = G1[place - 1].1;
    (G1[place].1 - below) / (high - low) * (speed - low) + below
}

/// The step of a flight's table, and how many it may have (13 s).
pub const STEP: f32 = 0.01;

pub const STEPS: usize = 1300;

/// The time past which a flight is thrown on under gravity alone.
const TABLE_TIME: f32 = 13.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct Step {
    pub(super) time: f32,
    pub(super) position: V3,
    pub(super) velocity: V3,
}

/// A bullet's flight (`TrajectoryCalculator`): its table of steps of 0.01 s,
/// made as far as they are asked for.
#[derive(Debug, Clone)]
pub struct Trajectory {
    pub(super) steps: Vec<Step>,
    pub(super) factor: f32,
    slowdown: f32,
    mass_x2: f32,
    pub(super) gravity: V3,
}

impl Trajectory {
    /// From a start and a velocity, a round's mass (g), diameter (mm) and
    /// ballistic coefficient; `gravity` nought for a bot's shot.
    pub fn new(start: V3, velocity: V3, mass_g: f32, diameter_mm: f32, coefficient: f32, gravity: V3) -> Self {
        let mass = mass_g / 1000.0;
        let diameter = diameter_mm / 1000.0;
        let factor = mass * 0.001_422_3 / (diameter * diameter * coefficient);
        let area = diameter * diameter * std::f32::consts::PI / 4.0;
        let mut steps = Vec::with_capacity(64);
        steps.push(Step { time: 0.0, position: start, velocity });
        Self { steps, factor, slowdown: 1.2 * area, mass_x2: mass * 2.0, gravity }
    }

    fn next(&mut self) -> Step {
        let current = *self.steps.last().expect("a flight has its start");
        let speed = magnitude(current.velocity);
        let drag = g1_drag(speed) * self.factor;
        let slowing = scale(normalized(current.velocity), self.slowdown * (0.0 - drag) * speed * speed / self.mass_x2);
        let accel = add(self.gravity, slowing);
        let step = Step {
            time: current.time + STEP,
            position: add(add(current.position, scale(current.velocity, STEP)), scale(accel, 5e-5)),
            velocity: add(current.velocity, scale(accel, STEP)),
        };
        self.steps.push(step);
        step
    }

    pub(super) fn step(&mut self, index: usize) -> Step {
        while self.steps.len() <= index {
            self.next();
        }
        self.steps[index]
    }

    /// Where it is and its velocity at a time since the shot
    /// (`PredictedTrajectoryCalculation`).
    pub fn at(&mut self, time: f32) -> (V3, V3) {
        let index = (((time / STEP) as f64).floor() as i64).clamp(0, STEPS as i64 - 1) as usize;
        if !(time > TABLE_TIME) && index + 1 <= STEPS - 1 {
            let (a, b) = (self.step(index), self.step(index + 1));
            (lerp(a.position, b.position, (time - a.time) / (b.time - a.time)), a.velocity)
        } else {
            let last = *self.steps.last().expect("a flight has its start");
            let t = time - last.time;
            (add(add(last.position, scale(last.velocity, t)), scale(self.gravity, 0.5 * t * t)), add(last.velocity, scale(self.gravity, t)))
        }
    }
}
