//! A tracer round's trace: which bullets show one, and the streak each is drawn as.
//! After the ballistics spec, "A tracer" (`TracersLight`): which bullets show one this frame, and the streak each is drawn as, its
//! twelve corners in the game's axes with their picture's places and
//! colours. The figures and curves are the install's renderer's
//! (`trp_data::particles::tracers_light`); drawing them is the host's.

use crate::combat::ballistics::{Bullet, V3};
use trp_data::formats::curve::Curve;

/// A round's `TracerColor` (`TaxonomyColor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracerColor {
    Blue,
    Yellow,
    Green,
    Red,
    Black,
    Grey,
    Violet,
    Orange,
    TracerYellow,
    TracerGreen,
    TracerRed,
    Default,
}

impl TracerColor {
    /// By its name in the item database; one not known is `default`.
    pub fn named(name: &str) -> Self {
        match name {
            "blue" => Self::Blue,
            "yellow" => Self::Yellow,
            "green" => Self::Green,
            "red" => Self::Red,
            "black" => Self::Black,
            "grey" => Self::Grey,
            "violet" => Self::Violet,
            "orange" => Self::Orange,
            "tracerYellow" => Self::TracerYellow,
            "tracerGreen" => Self::TracerGreen,
            "tracerRed" => Self::TracerRed,
            _ => Self::Default,
        }
    }

    /// The game's table of it (`TaxonomyColorExtension.ColorMapping`): the
    /// three tracer colours past 1, the label colours in 255ths.
    pub fn rgba(self) -> [f32; 4] {
        let bytes = |r: u8, g: u8, b: u8| [f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0];
        match self {
            Self::Blue => bytes(28, 65, 86),
            Self::Yellow => bytes(104, 102, 40),
            Self::Green => bytes(21, 45, 0),
            Self::Red => bytes(109, 36, 24),
            Self::Black => bytes(0, 0, 0),
            Self::Grey => bytes(29, 29, 29),
            Self::Violet => bytes(76, 42, 85),
            Self::Orange => bytes(60, 25, 0),
            Self::TracerYellow => [2.0, 1.311, 0.573, 1.0],
            Self::TracerGreen => [0.457, 1.5, 0.507, 1.0],
            Self::TracerRed => [2.0, 0.235, 0.235, 1.0],
            Self::Default => bytes(127, 127, 127),
        }
    }
}

/// The renderer's figures (`TracersLight`'s fields).
#[derive(Debug, Clone)]
pub struct Figures {
    pub max_flying_time: f32,
    pub size_time: Curve,
    pub color_alpha: Curve,
    pub grenade_flying_max_time: f32,
    pub grenade_size: f32,
    pub speed_modifier: f32,
    pub grenade_speed_modifier: f32,
}

impl Figures {
    pub fn of(data: &trp_data::drawing::particles::TracersLightData) -> Self {
        Self {
            max_flying_time: data.max_flying_time,
            size_time: data.size_time.clone(),
            color_alpha: data.color_alpha.clone(),
            grenade_flying_max_time: data.grenade_flying_max_time,
            grenade_size: data.grenade_size,
            speed_modifier: data.speed_modifier,
            grenade_speed_modifier: data.grenade_speed_modifier,
        }
    }
}

/// What a tracer is made from: its bullet's flight as it stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    pub color: TracerColor,
    /// Its newest place and the one before, and how many steps it has made.
    pub position: V3,
    pub previous: V3,
    pub steps: u32,
    /// Time since it was made, and its speed.
    pub age: f32,
    pub speed: f32,
    pub has_hit: bool,
}

impl Flight {
    /// A bullet's, where its round is a tracer.
    pub fn of(bullet: &Bullet) -> Option<Self> {
        let v = bullet.velocity;
        Some(Self {
            color: bullet.round.tracer?,
            position: bullet.position,
            previous: bullet.previous,
            steps: bullet.steps,
            age: bullet.age,
            speed: (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
            has_hit: bullet.has_hit(),
        })
    }
}

/// One tracer to draw (`TracerInfo`): its head, its way (a unit), its
/// colour and its back's, its size and length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tracer {
    pub head: V3,
    pub way: V3,
    pub color: [f32; 4],
    pub back: [f32; 4],
    pub size: f32,
    pub length: f32,
}

/// The tracer a flight shows this frame, if any (`ProcessShots`,
/// `CreateTracer`, `CreateShotTracer`, `CreateGrenadeTracer`).
pub fn tracer(flight: &Flight, figures: &Figures) -> Option<Tracer> {
    if flight.has_hit || flight.age >= figures.max_flying_time || flight.steps == 0 {
        return None;
    }
    let step = sub(flight.position, flight.previous);
    let squared = dot(step, step);
    // A step too short to point with; a first step under 10 m.
    if squared < 0.001 || (flight.steps == 1 && squared < 100.0) {
        return None;
    }
    let way = scale(step, 1.0 / squared.sqrt());
    let mut color = flight.color.rgba();
    if flight.color == TracerColor::TracerYellow {
        if flight.age > figures.grenade_flying_max_time {
            return None;
        }
        let s = flight.age / figures.grenade_flying_max_time;
        color[3] *= 1.0 - s;
        return Some(Tracer {
            head: flight.position,
            way,
            color,
            back: [0.0; 4],
            size: figures.size_time.evaluate(s) * figures.grenade_size,
            length: flight.speed * figures.grenade_speed_modifier,
        });
    }
    let s = flight.age / figures.max_flying_time;
    color[3] *= figures.color_alpha.evaluate(s);
    Some(Tracer { head: flight.position, way, color, back: color, size: figures.size_time.evaluate(s), length: flight.speed * figures.speed_modifier })
}

/// One corner of a tracer's streak: its place (the game's axes), its
/// picture's place and its colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Corner {
    pub position: V3,
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// Each tracer's three quads' triangles, by its corners (`UpdateTracers`).
pub const INDICES: [u32; 18] = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11];

/// A tracer's twelve corners (`UpdateTracers`): the square across its way
/// a little behind its head, then the strip in its up's plane and the strip
/// in its side's, each from the head back its length.
pub fn corners(tracer: &Tracer) -> [Corner; 12] {
    let h = 0.5 * tracer.size;
    let l = tracer.length;
    // `Matrix4x4.LookAt` with the up `Vector3.OrthoNormalize` makes.
    let forward = tracer.way;
    let up = plane_up(forward);
    let side = cross(up, forward);
    let at = |x: f32, y: f32, z: f32| add(tracer.head, add(scale(side, x), add(scale(up, y), scale(forward, z))));
    let back = -0.03 * l;
    let places = [
        (at(-h, -h, back), [0.5, 0.5]),
        (at(h, -h, back), [1.0, 0.5]),
        (at(h, h, back), [1.0, 1.0]),
        (at(-h, h, back), [0.5, 1.0]),
        (at(0.0, -h, 0.0), [0.0, 0.0]),
        (at(0.0, -h, -l), [1.0, 0.0]),
        (at(0.0, h, -l), [1.0, 0.5]),
        (at(0.0, h, 0.0), [0.0, 0.5]),
        (at(-h, 0.0, 0.0), [0.0, 0.0]),
        (at(-h, 0.0, -l), [1.0, 0.0]),
        (at(h, 0.0, -l), [1.0, 0.5]),
        (at(h, 0.0, 0.0), [0.0, 0.5]),
    ];
    std::array::from_fn(|k| Corner { position: places[k].0, uv: places[k].1, color: if k < 4 { tracer.back } else { tracer.color } })
}

/// The unit perpendicular `Vector3.OrthoNormalize` gives a way from a
/// tangent of nought. `PROVISIONAL`: taken as ODE's `dPlaneSpace`, not read
/// from the engine.
fn plane_up(n: V3) -> V3 {
    if n[2].abs() > std::f32::consts::FRAC_1_SQRT_2 {
        let k = 1.0 / (n[1] * n[1] + n[2] * n[2]).sqrt();
        [0.0, -n[2] * k, n[1] * k]
    } else {
        let k = 1.0 / (n[0] * n[0] + n[1] * n[1]).sqrt();
        [-n[1] * k, n[0] * k, 0.0]
    }
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::ballistics::{Asker, Ballistics, BulletWorld, RayHit, Round};
    use trp_data::formats::curve::Key;

    /// The install's figures (`the_tracers_renderer_is_read`).
    fn figures() -> Figures {
        let key = |time: f32, value: f32, slope: f32| Key { time, value, in_slope: slope, out_slope: slope };
        Figures {
            max_flying_time: 4.0,
            size_time: Curve { keys: vec![key(0.0, -0.001_953_125, 17.193_913), key(0.05, 0.5, 0.484_556_2), key(0.7, 0.575, -0.118_562_35), key(0.95, 0.525, -0.755_748_7), key(1.0, 0.0, -20.057_259)] },
            color_alpha: Curve { keys: vec![key(0.0, 1.0, 0.0), key(0.9, 1.0, 0.0), key(1.0, 0.0, -16.043_495)] },
            grenade_flying_max_time: 0.25,
            grenade_size: 0.06,
            speed_modifier: 0.01,
            grenade_speed_modifier: 0.003,
        }
    }

    fn flight(steps: u32, step: V3, age: f32) -> Flight {
        Flight { color: TracerColor::TracerRed, position: add([5.0, 1.0, 2.0], step), previous: [5.0, 1.0, 2.0], steps, age, speed: 900.0, has_hit: false }
    }

    fn near(a: V3, b: V3) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4)
    }

    #[test]
    fn a_tracer_shows_while_it_flies_unhit_under_its_time() {
        let f = figures();
        let t = tracer(&flight(3, [15.0, 0.0, 0.0], 2.0), &f).expect("shown");
        assert_eq!(t.head, [20.0, 1.0, 2.0]);
        assert!(near(t.way, [1.0, 0.0, 0.0]));
        assert!((t.length - 9.0).abs() < 1e-4, "{}", t.length);
        assert_eq!(t.size, f.size_time.evaluate(0.5));
        assert_eq!(t.color, [2.0, 0.235, 0.235, 1.0]);
        assert_eq!(t.back, t.color);
        // Fading out over its last tenth.
        let late = tracer(&flight(3, [15.0, 0.0, 0.0], 3.8), &f).expect("shown");
        assert!(late.color[3] > 0.0 && late.color[3] < 1.0, "{}", late.color[3]);
        // None at its time, after a hit, before its first step, nor where a
        // step is too short to point with.
        assert!(tracer(&flight(3, [15.0, 0.0, 0.0], 4.0), &f).is_none());
        assert!(tracer(&Flight { has_hit: true, ..flight(3, [15.0, 0.0, 0.0], 1.0) }, &f).is_none());
        assert!(tracer(&flight(0, [0.0; 3], 0.0), &f).is_none());
        assert!(tracer(&flight(5, [0.02, 0.0, 0.0], 1.0), &f).is_none());
    }

    #[test]
    fn a_first_step_shows_only_from_10_m() {
        let f = figures();
        assert!(tracer(&flight(1, [9.9, 0.0, 0.0], 0.011), &f).is_none());
        assert!(tracer(&flight(1, [10.1, 0.0, 0.0], 0.011), &f).is_some());
        // The second step at any length past 3.2 cm.
        assert!(tracer(&flight(2, [6.25, 0.0, 0.0], 0.014), &f).is_some());
    }

    #[test]
    fn a_grenade_fragment_s_is_short_and_small() {
        let f = figures();
        let shrapnel = Flight { color: TracerColor::TracerYellow, ..flight(3, [15.0, 0.0, 0.0], 0.1) };
        let t = tracer(&shrapnel, &f).expect("shown");
        assert!((t.color[3] - 0.6).abs() < 1e-6);
        assert_eq!(t.back, [0.0; 4]);
        assert!((t.size - f.size_time.evaluate(0.4) * 0.06).abs() < 1e-7);
        assert!((t.length - 2.7).abs() < 1e-4);
        assert!(tracer(&Flight { age: 0.26, ..shrapnel }, &f).is_none());
    }

    #[test]
    fn the_streak_is_a_square_and_two_crossed_strips() {
        let t = Tracer { head: [0.0, 0.0, 0.0], way: [1.0, 0.0, 0.0], color: [1.0; 4], back: [0.5; 4], size: 0.5, length: 9.0 };
        let c = corners(&t);
        // Flying along x: its up is the world's, its side the up crossed
        // with the way (−z in the game's axes).
        assert!(near(c[4].position, [0.0, -0.25, 0.0]) && near(c[5].position, [-9.0, -0.25, 0.0]) && near(c[6].position, [-9.0, 0.25, 0.0]));
        assert!(near(c[8].position, [0.0, 0.0, 0.25]) && near(c[10].position, [-9.0, 0.0, -0.25]));
        assert!(near(c[0].position, [-0.27, -0.25, 0.25]) && near(c[2].position, [-0.27, 0.25, -0.25]));
        assert_eq!((c[0].uv, c[2].uv, c[4].uv, c[6].uv), ([0.5, 0.5], [1.0, 1.0], [0.0, 0.0], [1.0, 0.5]));
        assert_eq!((c[3].color, c[4].color), ([0.5; 4], [1.0; 4]));
        // Along z, the up the other branch's.
        assert!(near(plane_up([0.0, 0.0, 1.0]), [0.0, -1.0, 0.0]));
        assert!(near(plane_up([0.0, 1.0, 0.0]), [-1.0, 0.0, 0.0]));
    }

    #[test]
    fn the_colours_are_the_game_s() {
        assert_eq!(TracerColor::named("tracerGreen").rgba(), [0.457, 1.5, 0.507, 1.0]);
        let red = TracerColor::named("red").rgba();
        assert!((red[0] - 109.0 / 255.0).abs() < 1e-7 && (red[2] - 24.0 / 255.0).abs() < 1e-7);
        assert_eq!(TracerColor::named("nothing"), TracerColor::Default);
    }

    /// Nothing to meet but a wall 30 m on.
    struct Open;
    impl BulletWorld for Open {
        fn ray(&self, _: Asker, from: V3, to: V3, hits: &mut Vec<RayHit>) {
            if from[2] < 30.0 && to[2] >= 30.0 {
                let way = sub(to, from);
                let t = (30.0 - from[2]) / way[2];
                let surface = crate::combat::ballistics::Surface { material: crate::combat::ballistics::material::CONCRETE, figures: [100.0, 0.0, 0.0, 0.0, 0.0, 0.0] };
                hits.push(RayHit { distance: t * dot(way, way).sqrt(), point: add(from, scale(way, t)), normal: [0.0, 0.0, -1.0], collider: 1, thing: 1, mesh: false, surface, body: None });
            }
        }
    }

    #[test]
    fn a_flying_bullet_keeps_its_last_step_and_a_hit_ends_its_tracer() {
        let round = std::sync::Arc::new(Round {
            initial_speed: 900.0,
            mass_g: 3.4,
            diameter_mm: 5.6,
            ballistic_coefficient: 0.28,
            damage: 40.0,
            penetration_power: 20.0,
            penetration_deviation: 0.0,
            penetration_chance: 0.0,
            ricochet_chance: 0.0,
            fragmentation_chance: 0.0,
            min_fragments: 0,
            max_fragments: 0,
            armor_damage: 0.5,
            life: 2.0,
            accuracy: 0,
            projectiles: 1,
            penetration_damage_mod: 0.0,
            fuse: None,
            stamina_burn: 0.0,
            heavy_bleeding: 0.0,
            light_bleeding: 0.0,
            tracer: Some(TracerColor::TracerRed),
        });
        let mut b = Ballistics::new(0.5, [0.0, -9.81, 0.0]);
        b.create(1, &round, 0, 7, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0);
        let f = figures();
        let shown = |b: &Ballistics| b.bullets().iter().filter_map(Flight::of).filter_map(|flight| tracer(&flight, &f)).count();
        assert_eq!(shown(&b), 0, "not before it moves");
        b.tick(1.0 / 60.0, &mut Open);
        let bullet = &b.bullets()[0];
        assert_eq!((bullet.steps, bullet.previous), (1, [0.0, 0.0, 0.0]));
        assert_eq!(shown(&b), 1, "a first step of 15 m");
        b.tick(1.0 / 60.0, &mut Open);
        let bullet = &b.bullets()[0];
        assert_eq!(bullet.steps, 2);
        assert!(bullet.previous[2] > 14.0 && bullet.previous[2] < 15.0, "{:?}", bullet.previous);
        assert_eq!(shown(&b), 1);
        // Into the wall on the next: its own tracer gone that frame.
        let hits = b.tick(1.0 / 60.0, &mut Open);
        assert_eq!(hits.len(), 1);
        assert!(b.bullets()[0].has_hit());
        assert_eq!(shown(&b), 0);
    }
}
