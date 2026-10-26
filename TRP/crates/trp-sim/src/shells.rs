//! A spent case's flight (firing spec, "Spent cases"; `BouncingObject`,
//! `Shell`): no rigid body, but jumps worked ahead in steps, each cast
//! against the world, ending at what it meets in an arc, the next jump off
//! it at a share of the speed, until the casts are spent. The host tells
//! what a cast meets and gives the random draws.

/// The hard settings' figures for a case's flight (`EFTHardSettings.Shells`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellSettings {
    pub radius: f32,
    pub play_mult: f32,
    pub max_casts: i32,
    pub step: f32,
    pub spread: f32,
    pub velocity_mult: f32,
    pub velocity_rotation: f32,
    pub bounce: f32,
    pub rebound_x: [f32; 2],
    pub rebound_y: [f32; 2],
}

/// What a cast met: how far along it, where, the surface's normal and the
/// surface's kind (`ESurfaceSound`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub distance: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub surface: u8,
}

/// What a cast is asked: from a place, along a way (not of length one), how
/// far.
pub type Cast<'a> = &'a dyn Fn([f32; 3], [f32; 3], f32) -> Option<Hit>;

/// The engine's gravity (`Physics.gravity`).
const GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn length(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
fn normalized(a: [f32; 3]) -> [f32; 3] {
    let l = length(a);
    if l > 1e-5 { scale(a, 1.0 / l) } else { [0.0; 3] }
}

/// One jump (`BouncingObject.Jump`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Jump {
    begin: [f32; 3],
    velocity: [f32; 3],
    end_time: f32,
    time: f32,
    point: [f32; 3],
    current_velocity: [f32; 3],
    next: [f32; 3],
    /// Where its last step is drawn as an arc: from when, from, over, to.
    arc: Option<(f32, [f32; 3], [f32; 3], [f32; 3])>,
    /// The surface it ends on, if it meets one.
    surface: Option<u8>,
}

impl Jump {
    fn new(begin: [f32; 3], velocity: [f32; 3]) -> Self {
        Self { begin, velocity, end_time: 0.0, time: 0.0, point: [0.0; 3], current_velocity: [0.0; 3], next: [0.0; 3], arc: None, surface: None }
    }

    fn velocity_at(&self, t: f32) -> [f32; 3] {
        add(self.velocity, scale(GRAVITY, t))
    }

    /// `CalculatePoint`: from its start, the velocity at that time times
    /// the time, and half the gravity times the time squared (as the game
    /// has it, the gravity counts one and a half times).
    fn point_at(&self, t: f32, velocity: [f32; 3]) -> [f32; 3] {
        add(add(self.begin, scale(velocity, t)), scale(GRAVITY, 0.5 * t * t))
    }

    /// `Jump.Update`: the time moved on, what is left over past its end
    /// given back (none if it is not over), and where it is.
    fn update(&mut self, dt: &mut f32) -> bool {
        self.time += *dt;
        let over = self.time - self.end_time;
        let ended = if over > 0.0 {
            self.time = self.end_time;
            *dt = over;
            true
        } else {
            *dt = 0.0;
            false
        };
        self.current_velocity = self.velocity_at(self.time);
        self.point = match self.arc {
            Some((from_time, from, over_point, to)) if self.time >= from_time => {
                if self.end_time > 0.0 {
                    let t = self.time / self.end_time;
                    let u = 1.0 - t;
                    add(add(scale(from, u * u), scale(over_point, 2.0 * u * t)), scale(to, t * t))
                } else {
                    to
                }
            }
            _ => self.point_at(self.time, self.current_velocity),
        };
        ended
    }
}

/// A case in flight.
#[derive(Debug, Clone, PartialEq)]
pub struct Flight {
    settings: ShellSettings,
    jump: Jump,
    casts_left: i32,
    frames_since_sound: u32,
    /// Its turn each frame, in degrees about its own axes, a hundredth of
    /// a second's worth (`Shell._rotationVector`).
    pub turn: [f32; 3],
    /// Its casts are spent and it lies still (`Finished`).
    pub done: bool,
}

impl Flight {
    /// A case let go at `begin` with `velocity` and `turn` (`Shell.
    /// ActivatePhysics`): the velocity turned about the weapon's forward by
    /// the settings' turn (none here) and times their multiplier.
    /// `random()` gives a draw from 0 to 1.
    pub fn new(begin: [f32; 3], velocity: [f32; 3], turn: [f32; 3], weapon_forward: [f32; 3], settings: ShellSettings, cast: Cast, random: &mut dyn FnMut() -> f32) -> Self {
        let mut velocity = velocity;
        if settings.velocity_rotation > 0.0 {
            velocity = rotate_about(velocity, normalized(weapon_forward), settings.velocity_rotation.to_radians());
        }
        let velocity = scale(velocity, settings.velocity_mult);
        let mut flight = Self { settings, jump: Jump::new(begin, velocity), casts_left: settings.max_casts, frames_since_sound: 3, turn, done: false };
        flight.jump = flight.make_jump(begin, velocity, cast, random);
        flight.jump.point = begin;
        flight
    }

    /// Where it is.
    pub fn place(&self) -> [f32; 3] {
        self.jump.point
    }

    /// `CreateJump`: the jump from a place with a velocity, cast ahead step
    /// by step until it meets something or the casts are spent.
    fn make_jump(&mut self, begin: [f32; 3], velocity: [f32; 3], cast: Cast, random: &mut dyn FnMut() -> f32) -> Jump {
        let mut jump = Jump::new(begin, velocity);
        let mut t = 0.0f32;
        let mut from = begin;
        while self.casts_left > 0 {
            let before = t;
            t += self.settings.step;
            self.casts_left -= 1;
            let to = jump.point_at(t, jump.velocity_at(t));
            let way = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
            let reach = length(way);
            let Some(hit) = cast(from, way, reach) else {
                from = to;
                continue;
            };
            jump.surface = Some(hit.surface);
            let along = if reach > 0.0 {
                t -= (1.0 - hit.distance / reach) * self.settings.step;
                scale(way, 1.0 / reach)
            } else {
                [0.0, -1.0, 0.0]
            };
            let spread = scale(inside_sphere(random), self.settings.spread);
            jump.next = normalized(add(add(along, scale(hit.normal, 2.0)), spread));
            let end = add(hit.point, scale(hit.normal, self.settings.radius));
            let middle = scale(add(end, from), 0.5);
            jump.arc = Some((before, from, add(middle, [0.0, hit.distance * 0.3, 0.0]), end));
            break;
        }
        jump.end_time = t;
        jump
    }

    /// A frame of the engine's (`BouncingObject.Update`, `Shell.Update`):
    /// moved on by its time; the surface it struck where its sound plays
    /// (`OnBounce`, no more than every third frame), its turn then made
    /// anew.
    pub fn frame(&mut self, dt: f32, cast: Cast, random: &mut dyn FnMut() -> f32) -> Option<u8> {
        if self.done {
            return None;
        }
        let mut time = dt * self.settings.play_mult;
        let mut struck = None;
        let mut guard = 100;
        while time > 0.0 {
            if self.jump.update(&mut time) {
                if self.casts_left <= 0 {
                    self.done = true;
                    break;
                }
                struck = self.jump.surface;
                let speed = (length(self.jump.current_velocity) * self.settings.bounce).max(0.0);
                let next = scale(self.jump.next, speed);
                let point = self.jump.point;
                self.jump = self.make_jump(point, next, cast, random);
            }
            guard -= 1;
            if guard <= 0 {
                self.done = true;
                break;
            }
        }
        let sounded = match struck {
            Some(surface) if self.frames_since_sound >= 3 => {
                self.frames_since_sound = 0;
                let speed2 = {
                    let v = self.jump.current_velocity;
                    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
                };
                let mut drawn = |range: [f32; 2]| {
                    let value = range[0] + (range[1] - range[0]) * random();
                    if random() < 0.5 { -value } else { value }
                };
                self.turn = [speed2 * drawn(self.settings.rebound_x), speed2 * drawn(self.settings.rebound_y), 0.0];
                Some(surface)
            }
            _ => None,
        };
        self.frames_since_sound += 1;
        sounded
    }
}

/// A draw inside a sphere of radius one (`Random.insideUnitSphere`).
fn inside_sphere(random: &mut dyn FnMut() -> f32) -> [f32; 3] {
    loop {
        let p = [random() * 2.0 - 1.0, random() * 2.0 - 1.0, random() * 2.0 - 1.0];
        if length(p) <= 1.0 {
            return p;
        }
    }
}

/// Which of a case's sounds a surface (`ESurfaceSound`) plays, as a place in
/// `trp_data::sound::SHELL_SURFACES` (`Firearms.PlayShellSound`): concrete
/// and asphalt the concrete's, metal, wood and plastic their own, soil,
/// gravel and grass the soil's; any other surface none.
pub fn sound_surface(surface: u8) -> Option<usize> {
    match surface {
        0 | 5 => Some(0),
        1 => Some(1),
        3 | 4 | 7 => Some(2),
        2 => Some(3),
        16 => Some(4),
        _ => None,
    }
}

/// A way turned about a unit axis by an angle (radians).
fn rotate_about(v: [f32; 3], axis: [f32; 3], angle: f32) -> [f32; 3] {
    let (s, c) = angle.sin_cos();
    let dot = v[0] * axis[0] + v[1] * axis[1] + v[2] * axis[2];
    let cross = [axis[1] * v[2] - axis[2] * v[1], axis[2] * v[0] - axis[0] * v[2], axis[0] * v[1] - axis[1] * v[0]];
    std::array::from_fn(|k| v[k] * c + cross[k] * s + axis[k] * dot * (1.0 - c))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The install's hard settings' figures.
    const SETTINGS: ShellSettings = ShellSettings {
        radius: 0.001,
        play_mult: 1.0,
        max_casts: 20,
        step: 0.15,
        spread: 0.15,
        velocity_mult: 3.0,
        velocity_rotation: 0.0,
        bounce: 0.8,
        rebound_x: [1.0, 2.0],
        rebound_y: [1.0, 2.0],
    };

    /// A floor at nought, of metal.
    fn floor(from: [f32; 3], way: [f32; 3], reach: f32) -> Option<Hit> {
        let to = add(from, way);
        (from[1] >= 0.0 && to[1] < 0.0).then(|| {
            let share = from[1] / (from[1] - to[1]);
            Hit { distance: reach * share, point: add(from, scale(way, share)), normal: [0.0, 1.0, 0.0], surface: 1 }
        })
    }

    fn rng() -> impl FnMut() -> f32 {
        let mut seed = 0x1234_5678u32;
        move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 8) as f32 / (1u32 << 24) as f32
        }
    }

    #[test]
    fn a_case_flies_falls_bounces_and_comes_to_rest_on_the_floor() {
        let mut random = rng();
        let mut flight = Flight::new([0.0, 1.5, 0.0], [1.0, 0.5, 0.0], [5.0, 0.0, 0.0], [0.0, 0.0, 1.0], SETTINGS, &floor, &mut random);
        assert_eq!(flight.place(), [0.0, 1.5, 0.0]);
        let mut struck = Vec::new();
        for _ in 0..240 {
            if let Some(surface) = flight.frame(1.0 / 120.0, &floor, &mut random) {
                struck.push(surface);
            }
        }
        // Thrown out (three times its velocity), down to the floor, at
        // least once struck it (metal), and still over it.
        assert!(!struck.is_empty() && struck.iter().all(|&s| s == 1));
        let at = flight.place();
        assert!(at[0] > 1.0, "{at:?}");
        assert!(at[1] >= -1e-3 && at[1] < 0.5, "{at:?}");
    }

    #[test]
    fn a_surface_plays_its_kind_s_sound_or_none() {
        // Concrete and asphalt; metal; soil, grass, gravel; wood; plastic.
        let kinds: Vec<Option<usize>> = [0, 5, 1, 3, 4, 7, 2, 16].into_iter().map(sound_surface).collect();
        assert_eq!(kinds, [Some(0), Some(0), Some(1), Some(2), Some(2), Some(2), Some(3), Some(4)]);
        // Glass, a puddle, tile, thick wood and snow have none.
        assert!([6, 9, 11, 12, 19].into_iter().all(|s| sound_surface(s).is_none()));
    }

    #[test]
    fn its_casts_spent_it_lies_still() {
        let mut random = rng();
        let never = |_: [f32; 3], _: [f32; 3], _: f32| None;
        let mut flight = Flight::new([0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [0.0; 3], [0.0, 0.0, 1.0], SETTINGS, &never, &mut random);
        // Twenty steps of 0.15 s: three seconds of fall, then done.
        for _ in 0..400 {
            flight.frame(1.0 / 120.0, &never, &mut random);
        }
        assert!(flight.done);
        let still = flight.place();
        flight.frame(1.0 / 120.0, &never, &mut random);
        assert_eq!(flight.place(), still);
    }
}
