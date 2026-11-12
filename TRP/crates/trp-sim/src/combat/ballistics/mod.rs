//! A shot after it leaves the weapon: its flight, what it meets, and what each hit does to it.
//! After `docs/spec/ballistics.md`. In Unity's axes; the
//! world it flies through is answered by the host (`BulletWorld`): the app's
//! range, or a recording's hits for a replay.

use crate::base::unity_random::{fast, PrecalculatedRandoms, UnityRandom};

mod trajectory;
mod rounds;
mod bullets;
mod flight;
mod fragmentation;
mod weapon_figures;

pub use trajectory::*;
pub use rounds::*;
pub use bullets::*;
pub use weapon_figures::*;
use trajectory::{EPSILON};
use flight::{Made};

pub type V3 = [f32; 3];

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
fn magnitude(a: V3) -> f32 {
    ((a[0] * a[0] + a[1] * a[1] + a[2] * a[2]) as f64).sqrt() as f32
}
/// Unity's `normalized`: nought for a vector shorter than 1e-5.
fn normalized(a: V3) -> V3 {
    let m = magnitude(a);
    if m > 1e-5 {
        [a[0] / m, a[1] / m, a[2] / m]
    } else {
        [0.0; 3]
    }
}
/// Unity's `Vector3.Lerp`, its share held within 0 and 1.
fn lerp(a: V3, b: V3, t: f32) -> V3 {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
/// Unity's `Vector3.Angle`, in degrees.
fn angle(from: V3, to: V3) -> f32 {
    let denominator = ((dot(from, from) * dot(to, to)) as f64).sqrt() as f32;
    if denominator < 1e-15 {
        return 0.0;
    }
    let cos = (dot(from, to) / denominator).clamp(-1.0, 1.0);
    (cos as f64).acos() as f32 * 57.295_78
}
fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// The materials' numbers (`MaterialType`).
pub mod material;

/// The ballistics calculator (`BallisticsCalculator`): its tables, the
/// bullets in flight, and its settings.
pub struct Ballistics {
    tables: PrecalculatedRandoms,
    /// `Ballistic.GlobalDamageDegradationCoefficient`.
    pub degradation: f32,
    pub gravity: V3,
    bullets: Vec<Bullet>,
    hits_scratch: Vec<RayHit>,
}

impl Ballistics {
}

#[cfg(test)]
mod tests;
