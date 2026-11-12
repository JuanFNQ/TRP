//! A recording's particle systems as the recorder writes them in a world file.
//! The format is recorder 0.11.0's (`<trace>.world.jsonl`: `effects`, `systems`), read for the
//! tools that set them beside `trp_sim::particles` (`bin/particles` here,
//! `bin/meetings` of `trp-physics`), with the few turns and placements
//! they share.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;

use anyhow::{Context, Result};
use serde_json::Value;
use trp_data::drawing::particles::{EffectSystem, ParticleSystemData};
use trp_data::unity::Assets;

use crate::effects::particles::{Particle, Placement};


/// A particle as the recorder wrote it.
#[derive(Debug, Clone)]
pub struct RecParticle {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub animated: [f32; 3],
    pub size: [f32; 3],
    pub color: [u8; 4],
    pub turn: [f32; 3],
    pub spin: [f32; 3],
    pub left: f32,
    pub life: f32,
    pub start_size: f32,
    pub seed: u32,
}

/// A system on one recorded frame.
#[derive(Debug, Clone)]
pub struct RecFrame {
    pub frame: i64,
    pub t: f64,
    pub playing: bool,
    pub emitting: bool,
    pub time: f32,
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    pub scale: [f32; 3],
    pub particles: Vec<RecParticle>,
}

/// A followed system, every frame it was written on.
#[derive(Debug, Default)]
pub struct RecSystem {
    pub path: String,
    pub space: String,
    pub frames: Vec<RecFrame>,
    pub done: Option<i64>,
}

/// The effect played: on which frame, where, which copy, which systems.
#[derive(Debug, Clone)]
pub struct Played {
    pub frame: i64,
    pub t: f64,
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub copy: i64,
    pub ids: Vec<i64>,
}

pub fn v3(v: &Value) -> Option<[f32; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}

pub fn v4(v: &Value) -> Option<[f32; 4]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32, a.get(3)?.as_f64()? as f32])
}

pub fn particle_of(v: &Value) -> Option<RecParticle> {
    let a = v.as_array()?;
    let f = |i: usize| a.get(i).and_then(Value::as_f64).map(|x| x as f32);
    let b = |i: usize| a.get(i).and_then(Value::as_u64).map(|x| x.min(255) as u8);
    Some(RecParticle {
        pos: [f(0)?, f(1)?, f(2)?],
        vel: [f(3)?, f(4)?, f(5)?],
        animated: [f(6)?, f(7)?, f(8)?],
        size: [f(9)?, f(10)?, f(11)?],
        color: [b(12)?, b(13)?, b(14)?, b(15)?],
        turn: [f(16)?, f(17)?, f(18)?],
        spin: [f(19)?, f(20)?, f(21)?],
        left: f(22)?,
        life: f(23)?,
        start_size: f(24)?,
        seed: a.get(25).and_then(Value::as_u64)? as u32,
    })
}

/// Every play of the effect in the world file and every system it followed.
pub fn read_world(path: &str, effect: &str) -> Result<(Vec<Played>, HashMap<i64, RecSystem>)> {
    let file = std::fs::File::open(path).with_context(|| format!("open {path}"))?;
    let mut played = Vec::new();
    let mut wanted: HashSet<i64> = HashSet::new();
    let mut systems: HashMap<i64, RecSystem> = HashMap::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        // Only lines that name the effect or carry systems are parsed.
        if !line.contains("\"systems\"") && !line.contains(effect) {
            continue;
        }
        let v: Value = serde_json::from_str(&line).with_context(|| format!("a line of {path}"))?;
        let frame = v.get("frame").and_then(Value::as_i64).unwrap_or(0);
        let t = v.get("t").and_then(Value::as_f64).unwrap_or(0.0);
        for e in v.get("effects").and_then(Value::as_array).into_iter().flatten() {
            if e.get("name").and_then(Value::as_str) != Some(effect) {
                continue;
            }
            let ids: Vec<i64> = e.get("systems").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_i64).collect();
            wanted.extend(ids.iter().copied());
            played.push(Played {
                frame,
                t,
                pos: e.get("pos").and_then(v3).unwrap_or_default(),
                normal: e.get("normal").and_then(v3).unwrap_or([0.0, 1.0, 0.0]),
                copy: e.get("copy").and_then(Value::as_i64).unwrap_or(0),
                ids,
            });
        }
        for s in v.get("systems").and_then(Value::as_array).into_iter().flatten() {
            let Some(id) = s.get("id").and_then(Value::as_i64) else { continue };
            if !wanted.contains(&id) {
                continue;
            }
            let rec = systems.entry(id).or_default();
            if s.get("done").and_then(Value::as_bool) == Some(true) {
                rec.done = Some(frame);
                continue;
            }
            if let Some(p) = s.get("path").and_then(Value::as_str) {
                rec.path = p.to_owned();
            }
            if let Some(p) = s.get("space").and_then(Value::as_str) {
                rec.space = p.to_owned();
            }
            rec.frames.push(RecFrame {
                frame,
                t,
                playing: s.get("playing").and_then(Value::as_bool).unwrap_or(false),
                emitting: s.get("emitting").and_then(Value::as_bool).unwrap_or(false),
                time: s.get("time").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                pos: s.get("pos").and_then(v3).unwrap_or_default(),
                rot: s.get("rot").and_then(v4).unwrap_or([0.0, 0.0, 0.0, 1.0]),
                scale: s.get("scale").and_then(v3).unwrap_or([1.0; 3]),
                particles: s.get("p").and_then(Value::as_array).into_iter().flatten().filter_map(particle_of).collect(),
            });
        }
    }
    Ok((played, systems))
}

/// Every shared system of the effects (`GameWorld/Effects/...`: the hits'
/// older emitters emit into them, and the muzzles' fumes and haze) the
/// world file follows, by name: each time it was followed taken together
/// (the recorder gives it a new number each time it plays again).
pub fn read_shared(path: &str) -> Result<HashMap<String, RecSystem>> {
    let file = std::fs::File::open(path).with_context(|| format!("open {path}"))?;
    let mut names: HashMap<i64, String> = HashMap::new();
    let mut systems: HashMap<String, RecSystem> = HashMap::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        if !line.contains("\"systems\"") {
            continue;
        }
        let v: Value = serde_json::from_str(&line).with_context(|| format!("a line of {path}"))?;
        let frame = v.get("frame").and_then(Value::as_i64).unwrap_or(0);
        let t = v.get("t").and_then(Value::as_f64).unwrap_or(0.0);
        for s in v.get("systems").and_then(Value::as_array).into_iter().flatten() {
            let Some(id) = s.get("id").and_then(Value::as_i64) else { continue };
            // The shared systems are the effects root's own children: an
            // effect's system below it (a grenade's `Drop Sparks`) is not one.
            if let Some(p) = s.get("path").and_then(Value::as_str).filter(|p| p.strip_prefix("GameWorld/Effects/").is_some_and(|rest| !rest.contains('/'))) {
                names.insert(id, bare(p).to_owned());
                let rec = systems.entry(bare(p).to_owned()).or_default();
                rec.path = p.to_owned();
                if let Some(space) = s.get("space").and_then(Value::as_str) {
                    rec.space = space.to_owned();
                }
            }
            let Some(name) = names.get(&id) else { continue };
            if s.get("done").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let rec = systems.get_mut(name).expect("named above");
            if rec.frames.last().is_some_and(|f| f.frame == frame) {
                continue;
            }
            rec.frames.push(RecFrame {
                frame,
                t,
                playing: s.get("playing").and_then(Value::as_bool).unwrap_or(false),
                emitting: s.get("emitting").and_then(Value::as_bool).unwrap_or(false),
                time: s.get("time").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                pos: s.get("pos").and_then(v3).unwrap_or_default(),
                rot: s.get("rot").and_then(v4).unwrap_or([0.0, 0.0, 0.0, 1.0]),
                scale: s.get("scale").and_then(v3).unwrap_or([1.0; 3]),
                particles: s.get("p").and_then(Value::as_array).into_iter().flatten().filter_map(particle_of).collect(),
            });
        }
    }
    for rec in systems.values_mut() {
        rec.frames.sort_by_key(|f| f.frame);
    }
    Ok(systems)
}

/// The shared systems as the install has them, by name: the older emitters'
/// systems of every entry of the effects' list (the materials' and those
/// that name none: the flashbang's shock wave is its alone).
pub fn shared_systems(assets: &mut Assets) -> Result<HashMap<String, EffectSystem>> {
    let mut out = HashMap::new();
    let mut add = |data: ParticleSystemData, system: trp_data::unity::ObjRef| {
        out.entry(bare(&data.name).to_owned()).or_insert(EffectSystem { data, position: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3], depth: 0, system, sub_emitter: false });
    };
    for name in trp_data::drawing::particles::entry_names(assets)? {
        let Ok(full) = trp_data::drawing::particles::effect_entry(assets, &name) else { continue };
        for e in full.emitters {
            if let trp_data::drawing::particles::EmitterKind::System(a) = e.kind {
                add(a.data, a.system);
            }
        }
    }
    Ok(out)
}

pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

pub fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let p = quat_mul(quat_mul(q, [v[0], v[1], v[2], 0.0]), [-q[0], -q[1], -q[2], q[3]]);
    [p[0], p[1], p[2]]
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// The angle between two turns, degrees.
pub fn angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs().min(1.0);
    2.0 * dot.acos().to_degrees()
}

/// A placement under another (the app's `explosion_view::compose`).
pub fn compose(parent: Placement, local: Placement) -> Placement {
    let scaled = std::array::from_fn(|k| local.position[k] * parent.scale[k]);
    let at = rotate(parent.rotation, scaled);
    Placement {
        position: std::array::from_fn(|k| parent.position[k] + at[k]),
        rotation: quat_mul(parent.rotation, local.rotation),
        scale: std::array::from_fn(|k| parent.scale[k] * local.scale[k]),
    }
}

/// `Quaternion.LookRotation(normal)` for the normals an effect is played
/// with: up as the app plays a grenade's (a quarter turn about x, taking z
/// to y); any other by the engine's rule with the world's up.
pub fn look_rotation(normal: [f32; 3]) -> [f32; 4] {
    let n = { let l = length(normal).max(1e-9); normal.map(|c| c / l) };
    if n[1] > 0.9999 {
        return [-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2];
    }
    let z = n;
    let x = { let c = [z[2], 0.0, -z[0]]; let l = length(c).max(1e-9); c.map(|v| v / l) };
    let y = [z[1] * x[2] - z[2] * x[1], z[2] * x[0] - z[0] * x[2], z[0] * x[1] - z[1] * x[0]];
    let (m00, m11, m22) = (x[0], y[1], z[2]);
    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(y[2] - z[1]) / s, (z[0] - x[2]) / s, (x[1] - y[0]) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (y[0] + x[1]) / s, (z[0] + x[2]) / s, (y[2] - z[1]) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(y[0] + x[1]) / s, 0.25 * s, (z[1] + y[2]) / s, (z[0] - x[2]) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(z[0] + x[2]) / s, (z[1] + y[2]) / s, 0.25 * s, (x[1] - y[0]) / s]
    }
}

/// A name without the copy's number (`Effect Grenade Expl New (1)`).
pub fn bare(name: &str) -> &str {
    let name = name.rsplit('/').next().unwrap_or(name);
    match name.rfind(" (") {
        Some(i) if name.ends_with(')') && name[i + 2..name.len() - 1].chars().all(|c| c.is_ascii_digit()) => &name[..i],
        _ => name,
    }
}

/// What the simulation's own rules make of a particle as recorded: its
/// tint, size and turn as drawn now (its colour taken as white, so that the
/// colour drawn is the tint alone).
pub fn as_ours(rec: &RecParticle, white: bool) -> Particle {
    let color = if white { [1.0; 4] } else { rec.color.map(|c| c as f32 / 255.0) };
    Particle {
        position: rec.pos,
        velocity: rec.vel,
        size: rec.start_size,
        color,
        rotation: rec.turn[2],
        lifetime: rec.life,
        age: (rec.life - rec.left).max(0.0),
        seed: rec.seed,
        trail: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hits' shared systems are the effects root's own children: a
    /// grenade's system of the same name below it (`courses3`'s
    /// `far-233833`, on Customs) is not taken into them.
    #[test]
    fn a_shared_system_is_the_effects_root_s_own() {
        let path = std::env::temp_dir().join(format!("trp_shared_{}.world.jsonl", std::process::id()));
        let lines = [
            r#"{"frame":1,"t":0.0,"systems":[{"id":1,"path":"GameWorld/Effects/Drop Sparks","space":"World","p":[]},{"id":2,"path":"GameWorld/Effects/Effect Grenade Expl New Mediator/Effect Grenade Expl New (1)/Drop Sparks","space":"World","p":[]}]}"#,
            r#"{"frame":2,"t":0.01,"systems":[{"id":2,"p":[]}]}"#,
        ];
        std::fs::write(&path, lines.join("\n")).expect("a scratch file");
        let shared = read_shared(path.to_str().expect("a path"));
        let _ = std::fs::remove_file(&path);
        let shared = shared.expect("read");
        let sparks = &shared["Drop Sparks"];
        assert_eq!(sparks.path, "GameWorld/Effects/Drop Sparks");
        assert_eq!(sparks.frames.iter().map(|f| f.frame).collect::<Vec<_>>(), [1]);
    }
}
