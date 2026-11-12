//! A level's time of day as the game runs it.
//! After `docs/spec/lighting.md`, "Customs by its time of day": the Time of Day sky's rules (`TOD_Sky`:
//! the scattering's constants, the sun's and the moon's places by the date,
//! the hour and the world's place, the sky's colours by the time of day, the
//! light's intensity, and the sky's colour in a direction as its shader
//! works it out, sampled on the processor) and the game's time-of-day
//! controller's (`ToDController`: the atmosphere's figures by the sun's
//! height, the sun's colour, the night's colour multiplier, and the ambient
//! harmonics from five samples of the sky). A fixed moment converges in a
//! few frames, as the game's two late updates feed each other.

use trp_data::drawing::particles::Gradient;
use trp_data::level::tod::{Fields, TimeOfDayLevel, WeightedCurve};

use crate::lighting::ambient::Harmonics;

mod sky;
mod celestials;
mod atmosphere;
mod globals;

pub use sky::*;
pub use celestials::*;
pub use globals::*;

/// A colour's three channels.
pub type Rgb = [f32; 3];

fn rgb(c: [f32; 4]) -> Rgb {
    [c[0], c[1], c[2]]
}

fn scale(c: Rgb, k: f32) -> Rgb {
    c.map(|v| v * k)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn lerp_unclamped(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    if a != b {
        ((v - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    if length > 1e-5 {
        v.map(|c| c / length)
    } else {
        [0.0; 3]
    }
}

/// A vector turned by a quaternion (x, y, z, w), and by its inverse.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    trp_data::level::level_light::rotate(q, v)
}

fn rotate_back(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    rotate([-q[0], -q[1], -q[2], q[3]], v)
}

/// `OrbitalToLocal`: a unit vector from the zenith angle and the azimuth.
fn orbital_to_local(theta: f32, phi: f32) -> [f32; 3] {
    let (s, c) = theta.sin_cos();
    let (sp, cp) = phi.sin_cos();
    [s * sp, c, s * cp]
}
