//! The smoke left by shots and its corners.

use super::*;

/// A point of a trail (`MuzzleSmoke.Point`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Point {
    pub(super) skip: bool,
    pub(super) position: [f32; 3],
    pub(super) velocity: [f32; 3],
    pub(super) turbulence: [f32; 3],
    /// Its colour's alpha, a byte.
    pub(super) alpha: u8,
    pub(super) diffusion: f32,
    pub(super) number: i32,
}

/// One corner of a trail's quad as drawn (`MuzzleSmoke.Draw`): its place
/// (the game's axes), its side (−1 or 1), its number along the trail, its
/// diffusion, its direction and its alpha (over 255).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmokeCorner {
    pub position: [f32; 3],
    pub side: f32,
    pub number: f32,
    pub diffusion: f32,
    pub direction: [f32; 3],
    pub alpha: f32,
}

/// A gun smoke's trail (`MuzzleSmoke`).
#[derive(Debug, Clone, Default)]
pub struct Smoke {
    pub(super) on: bool,
    shot_factor: f32,
    pub(super) points: VecDeque<Point>,
    pos: [f32; 3],
    pub(super) velocity: [f32; 3],
    pub(super) speed: f32,
    next_number: i32,
    turbulence_at: f32,
    length_share: f32,
    pub(super) count: i32,
    smoking: f32,
}

impl Smoke {
    /// Whether it is on (its frame worked: `enabled`).
    pub fn on(&self) -> bool {
        self.on
    }

    /// `Clear`: its points let go, off.
    pub fn clear(&mut self) {
        self.points.clear();
        self.smoking = 0.0;
        self.on = false;
    }

    /// A shot (`Shot`): the factor up; at whole, the trail begun again.
    pub fn shot(&mut self, f: &SmokeData, rng: &mut Rng) {
        self.shot_factor = (self.shot_factor + f.increasing_by_shot).min(1.0);
        if self.shot_factor >= 1.0 {
            self.clear();
            self.on = true;
            self.count = 1;
            self.length_share = 1.0 / (f.smoke_length + rng.range(0.0, f.smoke_length_randomness));
        }
    }

    /// `CreatePoint`: at the head, its alpha by the smoking and the speed.
    fn create(&mut self, f: &SmokeData, skip: bool) {
        let alpha = ((self.smoking * f.speed_smoke_strength.evaluate(self.speed)).clamp(0.0, 1.0) * 255.0) as u8;
        let number = self.next_number;
        self.next_number = self.next_number.wrapping_add(1);
        self.points.push_front(Point { skip, position: self.pos, velocity: [0.0; 3], turbulence: [0.0; 3], alpha, diffusion: 0.0, number });
    }

    /// A frame while on (`LateUpdateValues`): the head, the points made,
    /// moved and let go. `place` is the object's (the game's axes),
    /// `forward` the camera's.
    pub fn frame(&mut self, f: &SmokeData, dt: f32, place: [f32; 3], forward: [f32; 3]) {
        if !self.on || dt < f32::EPSILON {
            return;
        }
        self.shot_factor = (self.shot_factor - dt * f.drop_time).max(0.0);
        let last = self.pos;
        self.pos = slerp3(self.pos, place, 0.125);
        self.velocity = slerp3(self.velocity, scale(sub(self.pos, last), 1.0 / dt), 1.0 / 32.0);
        self.speed += ((square(self.velocity).sqrt() * f.speed_multiplier) - self.speed) * 0.25;
        let share = self.count as f32 * self.length_share;
        self.smoking = if share >= 1.0 { 0.0 } else { f.smoke.evaluate(share) * f.smoke_strength };
        self.initialize(f, forward);
        if !self.points.is_empty() {
            self.process(f, dt);
        }
    }

    /// `InitializePoints`.
    fn initialize(&mut self, f: &SmokeData, forward: [f32; 3]) {
        let brake = f.brake_distance * f.brake_distance;
        if self.smoking > 0.001 {
            if self.points.is_empty() {
                self.create(f, false);
                self.create(f, true);
            } else {
                self.points[0].position = self.pos;
            }
            let far = self.points.get(1).is_some_and(|second| square(sub(self.points[0].position, second.position)) > brake);
            if far {
                self.count += 1;
                self.turbulence_at += f.speed_turbulence_density.evaluate(self.speed) * f.turbulence_density;
                let mut v = self.velocity;
                v[1] += 0.1;
                // `Vector3.Cross(v, forward).normalized`.
                let cross = [v[1] * forward[2] - v[2] * forward[1], v[2] * forward[0] - v[0] * forward[2], v[0] * forward[1] - v[1] * forward[0]];
                let length = square(cross).sqrt();
                let unit = if length > 1e-5 { scale(cross, 1.0 / length) } else { [0.0; 3] };
                let turbulence = scale(unit, smoke_draw(self.turbulence_at) * f.turbulence_intensity);
                let strength = f.speed_turbulence_strength.evaluate(self.speed);
                let start = f.speed_start_diffusion.evaluate(self.speed) * f.smoke_end;
                let first = &mut self.points[0];
                first.turbulence = turbulence;
                first.velocity = add(first.velocity, scale(add(self.velocity, scale(turbulence, strength)), f.smoke_velocity));
                first.diffusion = start;
                first.skip = false;
                self.create(f, true);
            }
        } else if self.points.len() >= 2 && self.points[0].skip && square(sub(self.points[0].position, self.points[1].position)) > brake {
            self.points[0].skip = false;
        }
        if self.points.is_empty() {
            self.clear();
        } else if self.points.len() == 2 {
            if self.points[0].diffusion >= f.smoke_end {
                self.points.clear();
            }
        } else if self.points.len() >= 3 && self.points[self.points.len() - 2].diffusion >= f.smoke_end {
            self.points.pop_back();
        }
    }

    /// `ProcessPoints`.
    fn process(&mut self, f: &SmokeData, dt: f32) {
        let fall = f.gravity * dt;
        for p in self.points.iter_mut().filter(|p| !p.skip) {
            p.velocity[1] -= fall;
            p.velocity = scale(p.velocity, f.drag);
            let moved = scale(add(p.velocity, p.turbulence), dt);
            p.position = add(p.position, moved);
            p.diffusion += square(moved) * f.diffusion_by_velocity;
        }
    }

    /// `Draw`: each point's direction, and a quad's four corners between
    /// each two neighbours, newest first.
    pub fn corners(&self) -> Vec<SmokeCorner> {
        let n = self.points.len();
        if n < 2 {
            return Vec::new();
        }
        let direction = |i: usize| -> [f32; 3] {
            let p = &self.points;
            if i == 0 {
                sub(p[n - 1].position, p[0].position)
            } else if i + 1 < n {
                sub(p[i + 1].position, p[i - 1].position)
            } else {
                sub(p[i].position, p[i - 1].position)
            }
        };
        let corner = |i: usize, side: f32| {
            let p = &self.points[i];
            SmokeCorner { position: p.position, side, number: p.number as f32, diffusion: p.diffusion, direction: direction(i), alpha: p.alpha as f32 / 255.0 }
        };
        (1..n).flat_map(|i| [corner(i - 1, -1.0), corner(i, -1.0), corner(i, 1.0), corner(i - 1, 1.0)]).collect()
    }
}
