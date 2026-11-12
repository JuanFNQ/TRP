//! Tests of the test range, with their shared helpers.
use super::*;

mod walking;
mod doors_containers;
mod layout;


/// How far a body's sphere goes from a start along a yaw before it
/// meets anything of the range (the game's axes; yaw 0 along +z).
fn clear_ahead(range: &Range, at: [f32; 3], yaw: f32) -> f32 {
    use trp_sim::locomotion::movement::World as _;
    let (s, c) = yaw.to_radians().sin_cos();
    range.sphere_cast([at[0], at[1] + 1.0, at[2]], 0.4, [s, 0.0, c], 400.0).unwrap_or(400.0)
}
use trp_data::formats::curve::{Curve, Key};
use trp_sim::held::things::ThingState;

/// The install's `DoorCurve` and `DelayToOpenContainer` (`probe
/// hardsettings`), for the things to turn by without an install.
fn thing_settings() -> ThingSettings {
    let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
    let curve = Curve { keys: vec![key(0.0, 0.0, 1.11), key(0.568, 0.546, 0.61), key(1.718, 0.976, 0.10), key(2.5, 1.0, 0.01)] };
    ThingSettings { curve, answers_after: 0.55 }
}
