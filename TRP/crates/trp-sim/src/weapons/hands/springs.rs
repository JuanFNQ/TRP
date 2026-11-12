//! The blender, the spring and the sway the hands are moved by.

use super::*;

/// A value that moves toward a target at a fixed rate, optionally after a
/// delay. The game reads these off the clock instead of stepping them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Blender {
    start_value: f32,
    start_time: f32,
    pub(super) target: f32,
    pub(super) speed: f32,
}

impl Blender {
    pub(crate) fn new(value: f32, speed: f32) -> Self {
        Self { start_value: value, start_time: 0.0, target: value, speed }
    }

    /// Its rate set outright, as the game's blender's `Speed` is (the value
    /// read off the clock from then on goes by it).
    pub(crate) fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub(crate) fn value(&self, now: f32) -> f32 {
        let elapsed = (now - self.start_time).max(0.0);
        let moved = self.start_value + elapsed * self.speed * (self.target - self.start_value).signum();
        if self.start_value > self.target {
            moved.clamp(self.target, self.start_value)
        } else {
            moved.clamp(self.start_value, self.target)
        }
    }

    pub(crate) fn set_target(&mut self, target: f32, now: f32) {
        if self.target != target {
            self.start_value = self.value(now);
            self.start_time = now;
            self.target = target;
        }
    }

    /// Jumps to `value` and holds it for `delay` seconds before moving on.
    pub(crate) fn jump(&mut self, value: f32, delay: f32, now: f32) {
        self.start_time = now + delay;
        self.start_value = value;
    }
}

/// How far a lean reaches the weapon: all the way when slow or low, not at
/// all when walking upright at full speed.
pub(super) fn tilt_by_speed_and_stance(speed: f32, stance: f32) -> f32 {
    ((1.6 - 0.9 * speed - 0.7 * stance) / 0.63).clamp(0.0, 1.0)
}

/// The game's frame-stepped spring.
#[derive(Debug, Clone)]
pub struct Spring {
    pub zero: [f32; 3],
    pub current: [f32; 3],
    pub(super) velocity: [f32; 3],
    pub(super) force: [f32; 3],
    /// Pushes the step takes without its limit (`ForceAccumulatorLimitless`).
    limitless: [f32; 3],
    has_force: bool,
    pub input_intensity: f32,
    pub(super) return_speed: f32,
    pub damping: f32,
    pub(super) acceleration_max: f32,
}

impl Spring {
    /// Takes another's motion (where it is, how fast, the pushes it has
    /// still to take), keeping its own figures.
    pub fn take_state(&mut self, from: &Spring) {
        (self.current, self.velocity, self.force, self.limitless, self.has_force) = (from.current, from.velocity, from.force, from.limitless, from.has_force);
    }

    pub fn new(data: SpringData) -> Self {
        Self {
            zero: [0.0; 3],
            current: [0.0; 3],
            velocity: [0.0; 3],
            force: [0.0; 3],
            limitless: [0.0; 3],
            has_force: false,
            input_intensity: data.input_intensity,
            return_speed: data.return_speed,
            damping: data.damping,
            acceleration_max: data.acceleration_max,
        }
    }

    pub fn add(&mut self, component: usize, value: f32) {
        self.force[component] += value;
        self.has_force = true;
    }

    /// A push the step takes whole, past the spring's limit
    /// (`AddAccelerationLimitless`).
    pub fn add_limitless(&mut self, component: usize, value: f32) {
        self.limitless[component] += value;
        self.has_force = true;
    }

    /// One physics step. The formula has no time step in it.
    pub fn step(&mut self) {
        for i in 0..3 {
            if self.has_force {
                let limited = self.force[i].clamp(-self.acceleration_max, self.acceleration_max);
                self.velocity[i] += (limited + self.limitless[i]) * self.input_intensity;
            }
            // The spring is drawn to its zero, and the zero is added again
            // when the value is read: a held offset counts twice at rest.
            self.velocity[i] -= (self.current[i] - self.zero[i]) * self.return_speed;
            self.velocity[i] *= self.damping;
            self.current[i] += self.velocity[i];
        }
        self.force = [0.0; 3];
        self.limitless = [0.0; 3];
        self.has_force = false;
    }

    pub fn value(&self) -> [f32; 3] {
        [self.zero[0] + self.current[0], self.zero[1] + self.current[1], self.zero[2] + self.current[2]]
    }
}

/// The sway spring: an underdamped oscillator integrated exactly.
#[derive(Debug, Clone)]
pub struct Sway {
    pub position: [f32; 3],
    pub(super) velocity: [f32; 3],
    pub(super) frequency: f32,
    pub(super) damping: f32,
}

impl Sway {
    pub fn new(frequency: f32, damping: f32) -> Self {
        Self { position: [0.0; 3], velocity: [0.0; 3], frequency, damping: damping.min(0.9999) }
    }

    pub fn kick(&mut self, component: usize, velocity: f32) {
        self.velocity[component] += velocity;
    }

    /// New tuning, the spring keeping where it is and how fast it goes.
    pub fn set(&mut self, frequency: f32, damping: f32) {
        (self.frequency, self.damping) = (frequency, damping.min(0.9999));
    }

    pub fn step(&mut self, dt: f32) {
        let decay = self.frequency * self.damping;
        let alpha = self.frequency * (1.0 - self.damping * self.damping).sqrt();
        let e = (-decay * dt).exp();
        let (s, c) = (alpha * dt).sin_cos();
        for i in 0..3 {
            let x = self.position[i];
            let b = (self.velocity[i] + decay * x) / alpha;
            self.position[i] = e * (x * c + b * s);
            self.velocity[i] = -e * ((x * decay - b * alpha) * c + (x * alpha + b * decay) * s);
        }
    }
}
