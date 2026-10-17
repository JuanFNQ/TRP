//! A recorded death set beside the engine's own (`bin/corpse`, a test of
//! the four kept deaths): a trace of recorder 0.8.0 on, its first frame
//! after the death taken as the corpse's start, the corpse rebuilt as
//! `corpse::build` builds it, stepped as the game steps it, and each
//! body's place set beside the recorded one frame by frame.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use trp_data::ragdoll::RagdollData;

use crate::*;


/// A corpse's bodies are pushed out of what they are in at most
/// `CorpseMaxDepenetrationVelocity` (4 in the install).
pub const DEPENETRATION: f32 = 4.0;

pub struct Frame {
    pub dead_t: f32,
    pub dt: f32,
    pub bodies: HashMap<String, (V3, Q4)>,
}

fn v3(v: &Value) -> Option<V3> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}

fn q4(v: &Value) -> Option<Q4> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32, a.get(3)?.as_f64()? as f32])
}

pub struct Trace {
    pub frames: Vec<Frame>,
    /// The ground under the feet as the last frame before the death had it.
    pub ground: f32,
    pub velocity: V3,
    /// The frames' own time, as their average over the run before the
    /// death: what the game steps its physics by.
    pub step: f32,
    /// The push the game gave the corpse (`ApplyCorpseImpulse`): on the
    /// body of the collider hit, at the hit, along the shot, so strong.
    pub push: Option<(String, V3, V3, f32)>,
    pub physics: trp_data::engine::PhysicsSettings,
}

pub fn read(path: &str) -> Result<Trace> {
    let text = std::fs::read_to_string(path).with_context(|| path.to_owned())?;
    let mut frames = Vec::new();
    let (mut ground, mut velocity, mut dts) = (0.0f32, [0.0f32; 3], Vec::new());
    let mut push = None;
    for line in text.lines().skip(1) {
        let v: Value = serde_json::from_str(line)?;
        let Some(dead_t) = v.get("dead_t").and_then(Value::as_f64) else {
            if let Some(below) = v.get("ground_below").and_then(Value::as_f64) {
                let feet = v.get("pos").and_then(v3).map_or(0.0, |p| p[1]);
                ground = feet - below as f32;
            }
            if let Some(dt) = v.get("dt").and_then(Value::as_f64) {
                dts.push(dt as f32);
            }
            continue;
        };
        if let Some(died) = v.get("died") {
            velocity = died.get("velocity").and_then(v3).unwrap_or([0.0; 3]);
            let collider = died.get("hit_collider").and_then(Value::as_str).unwrap_or("");
            if let (false, Some(point), Some(direction), Some(force)) =
                (collider.is_empty(), died.get("hit_point").and_then(v3), died.get("hit_direction").and_then(v3), died.get("corpse_force").and_then(Value::as_f64))
            {
                push = Some((collider.to_owned(), point, direction, force as f32));
            }
        }
        let mut bodies = HashMap::new();
        for (name, b) in v.get("bodies").and_then(Value::as_object).into_iter().flatten() {
            let a = b.as_array().ok_or_else(|| anyhow!("a body is not a list"))?;
            if let (Some(at), Some(turn)) = (a.first().and_then(v3), a.get(1).and_then(q4)) {
                bodies.insert(name.clone(), (at, turn));
            }
        }
        frames.push(Frame { dead_t: dead_t as f32, dt: v.get("dt").and_then(Value::as_f64).unwrap_or(0.0) as f32, bodies });
    }
    if frames.is_empty() {
        return Err(anyhow!("no frame after a death in {path}"));
    }
    dts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let step = dts.get(dts.len() / 2).copied().unwrap_or(1.0 / 60.0);
    let physics = trp_data::engine::PhysicsSettings::load(&trp_data::Install::locate()?)?;
    Ok(Trace { frames, ground, velocity, step, push, physics })
}

pub struct Run {
    /// Per frame after the first, the place of each body, by name.
    pub places: Vec<HashMap<String, V3>>,
}

/// The corpse rebuilt in the engine as the game builds it (`crate::corpse`),
/// stepped as the game steps it, the push given where the game gives it; each
/// frame's bodies' places. `convention` and `from_rest` let the ways Unity
/// might lay its joint on PhysX's be tried.
pub fn simulate(data: &RagdollData, trace: &Trace, convention: JointConvention, from_rest: bool) -> Run {
    let mut world = World::new(Settings::of(&trace.physics));
    world.convention = convention;
    let (deadbody, terrain) = (trace.physics.layer("Deadbody").unwrap_or(23), trace.physics.layer("Terrain").unwrap_or(11));
    world.add_ground(trace.ground, Material::default(), terrain);
    let mut data = data.clone();
    if !from_rest {
        // As if the skeleton's rest were the pose at death.
        for b in &mut data.bodies {
            if let Some(pose) = trace.frames[0].bodies.get(&b.bone) {
                b.rest = *pose;
            }
        }
    }
    let bodies = crate::corpse::build(&mut world, &data, &trace.frames[0].bodies, trace.velocity, DEPENETRATION, deadbody);
    let mut places = Vec::new();
    for (index, _) in trace.frames.iter().skip(1).enumerate() {
        // The push, the frame after the corpse is made
        // (`CorpseRagdoll.WorkingCycle`): on the body whose own collider was
        // hit.
        if index == 1 {
            if let Some((collider, point, direction, force)) = &trace.push {
                if let Some(body) = bodies.get(collider) {
                    world.impulse_at(*body, [direction[0] * force, direction[1] * force, direction[2] * force], *point);
                }
            }
        }
        world.step(trace.step);
        places.push(bodies.iter().map(|(name, body)| (name.clone(), world.pose(*body).0)).collect());
    }
    Run { places }
}

fn distance(a: V3, b: V3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The mean distance of the bodies from the recorded, frame by frame.
pub fn drift(trace: &Trace, run: &Run, until: f32) -> f32 {
    let mut sum = 0.0;
    let mut n = 0;
    for (frame, places) in trace.frames.iter().skip(1).zip(&run.places) {
        if frame.dead_t > until {
            break;
        }
        for (name, at) in places {
            if let Some((recorded, _)) = frame.bodies.get(name) {
                sum += distance(*at, *recorded);
                n += 1;
            }
        }
    }
    if n == 0 { f32::NAN } else { sum / n as f32 }
}

