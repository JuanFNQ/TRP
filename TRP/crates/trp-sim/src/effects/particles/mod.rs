//! Unity's particle system run: particles born, stepped by the system's modules and drawn.
//! After the particles spec: a system played (its
//! start delay, its bursts and rate, each particle born on its shape with
//! the main module's start values) or particles emitted with the values the
//! caller gives (`ParticleSystem.Emit(EmitParams)`), each stepped by the
//! modules the system has on, and what each is drawn as. In the game's
//! axes; no engine types.

use std::sync::Arc;

use trp_data::drawing::particles::{ParticleSystemData, Shape};

use crate::weapons::recoil::Rng;

mod cubes;
pub use cubes::Cubes;

mod particle;
mod emission;
mod stepping;
mod collision;
mod shapes;

pub use particle::*;
pub use stepping::*;
pub use collision::*;
use particle::{draw, engine_draw};
use shapes::{add, conjugate, dot, length, on_shape, rotate, scale, sheet_of, size_of, sub};
#[cfg(test)]
use emission::burst_count;

/// The world's gravity, metres a second a second (the install's physics
/// settings: physics spec).
const GRAVITY: f32 = -9.81;

/// The modules' constants in the engine (`engine_draw`).
const SIZE_ID: u32 = 0x8d2c_8431;
const ROTATION_ID: u32 = 0x6aed_452e;
const COLOR_ID: u32 = 0x591b_c05c;
const LIMIT_ID: u32 = 0x1337_1337;
const DRAG_ID: u32 = 0xa679_0296;
const FLIP_ID: u32 = 0xff2b_b1a4;
const FORCE_ID: u32 = 0x1246_0f3b;
const BY_SPEED_ID: u32 = 0xdec4_aea1;
const VELOCITY_ID: u32 = 0xe0fb_d834;

const GRAVITY_DRAW: u32 = 6;
const FORCE: u32 = 8;
const SHEET: u32 = 11;
const ROW: u32 = 12;
const START_FRAME: u32 = 13;

const DAMPEN: u32 = 15;
const BOUNCE: u32 = 16;
const LIFETIME_LOSS: u32 = 17;
const LIGHT_RANGE: u32 = 18;
const LIGHT_INTENSITY: u32 = 19;
const TRAIL_COLOR: u32 = 20;
const TRAIL_WIDTH: u32 = 21;
const TRAIL_LIFE: u32 = 22;


/// `Mathf.SmoothDamp` as Unity publishes it (`UnityCsReference`): `current`
/// toward `target` over about `smooth_time`, its velocity carried, with no
/// most speed; the target taken where it would pass it.
pub fn smooth_damp(current: f32, target: f32, velocity: &mut f32, smooth_time: f32, dt: f32) -> f32 {
    let smooth_time = smooth_time.max(0.0001);
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*velocity + omega * change) * dt;
    *velocity = (*velocity - omega * temp) * exp;
    let mut output = target + (change + temp) * exp;
    if (target - current > 0.0) == (output > target) {
        output = target;
        *velocity = if dt > 0.0 { (output - target) / dt } else { 0.0 };
    }
    output
}

/// A system's playing (`Play`): the time since it was played, its start
/// delay, each burst's cycles done, the rate's share of a particle carried,
/// and whether it has done emitting.
#[derive(Debug, Clone)]
struct Playing {
    time: f32,
    delay: f32,
    cycles_done: Vec<i64>,
    carried: f32,
    done: bool,
}

/// A system running: its figures, where it is, its particles (oldest
/// first) and its own draws.
#[derive(Debug, Clone)]
pub struct ParticleSystem {
    pub data: Arc<ParticleSystemData>,
    pub particles: Vec<Particle>,
    pub placement: Placement,
    playing: Option<Playing>,
    /// Played with nothing to draw by a system that pauses while its
    /// renderer is not seen: its next step is not taken (`play`).
    unseen: bool,
    rng: Rng,
    /// What it keeps for meeting the world at medium or low quality: its
    /// planes met by cube, the next particle's ray, the frame's rays.
    pub cubes: Cubes,
}

impl ParticleSystem {
}

#[cfg(test)]
mod tests;
