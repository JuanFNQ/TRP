//! A recorded death's set-up beside the corpse as built.
//! For `bin/corpse --setup` (`docs/spec/physics.md`, "Compared with what"). Recorder 0.10.0
//! on writes, on the first frame after the death, the engine's settings and
//! every body, collider and joint of the corpse as the game set them up,
//! and on the next the weapon let go's. Each figure is set beside the
//! install's (the prefab's ragdoll, the engine's and the hard settings) and
//! what `corpse::build` makes of them in PhysX.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use trp_data::character::ragdoll::RagdollData;

use crate::corpse::{PROJECTION, STRAP_BY_PALM, STRAP_DAMPER, STRAP_ON_PELVIS, STRAP_SPRING, UNITY_ANGULAR_DRAG, WEAPON_MASS};
use crate::*;

/// A line of the comparison.
#[derive(Debug, Clone)]
pub enum Line {
    /// A part's heading.
    Head(String),
    /// What the recording says that is not a figure set beside ours.
    Note(String),
    Same(String),
    Differs(String),
}

#[derive(Debug, Default)]
pub struct Report {
    pub lines: Vec<Line>,
}

impl Report {
    /// The figures set side by side, and those that differ.
    pub fn counts(&self) -> (usize, usize) {
        let differ = self.lines.iter().filter(|l| matches!(l, Line::Differs(_))).count();
        (self.lines.iter().filter(|l| matches!(l, Line::Same(_))).count() + differ, differ)
    }

    pub(crate) fn head(&mut self, text: impl Into<String>) {
        self.lines.push(Line::Head(text.into()));
    }

    pub fn note(&mut self, text: impl Into<String>) {
        self.lines.push(Line::Note(text.into()));
    }

    pub(crate) fn put(&mut self, same: bool, text: String) {
        self.lines.push(if same { Line::Same(text) } else { Line::Differs(text) });
    }

    pub(crate) fn number(&mut self, what: &str, recorded: Option<f32>, ours: f32, tolerance: f32) {
        match recorded {
            Some(r) => self.put((r - ours).abs() <= tolerance, format!("{what}: recorded {r}, ours {ours}")),
            None => self.put(false, format!("{what}: not recorded, ours {ours}")),
        }
    }

    pub(crate) fn vector(&mut self, what: &str, recorded: Option<V3>, ours: V3, tolerance: f32) {
        match recorded {
            Some(r) => self.put(distance(r, ours) <= tolerance, format!("{what}: recorded {}, ours {} ({:.5} apart)", show(r), show(ours), distance(r, ours))),
            None => self.put(false, format!("{what}: not recorded, ours {}", show(ours))),
        }
    }

    pub(crate) fn text(&mut self, what: &str, recorded: Option<&str>, ours: &str) {
        let r = recorded.unwrap_or("(not recorded)");
        self.put(r == ours, format!("{what}: recorded {r}, ours {ours}"));
    }

    pub(crate) fn flag(&mut self, what: &str, recorded: Option<bool>, ours: bool) {
        match recorded {
            Some(r) => self.put(r == ours, format!("{what}: recorded {r}, ours {ours}")),
            None => self.put(false, format!("{what}: not recorded, ours {ours}")),
        }
    }
}

pub(crate) fn show(v: V3) -> String {
    format!("({:.4}, {:.4}, {:.4})", v[0], v[1], v[2])
}

pub(crate) fn distance(a: V3, b: V3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

pub(crate) fn f(v: &Value) -> Option<f32> {
    v.as_f64().map(|x| x as f32)
}

pub(crate) fn v3(v: &Value) -> Option<V3> {
    let a = v.as_array()?;
    Some([f(a.first()?)?, f(a.get(1)?)?, f(a.get(2)?)?])
}

pub(crate) fn q4(v: &Value) -> Option<Q4> {
    let a = v.as_array()?;
    Some([f(a.first()?)?, f(a.get(1)?)?, f(a.get(2)?)?, f(a.get(3)?)?])
}

/// An object's own name, the last of its path.
pub(crate) fn last(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A point of the world in the axes of what stands at `at`, turned `turn`.
pub(crate) fn into(at: V3, turn: Q4, point: V3) -> V3 {
    rotate(inverse(turn), sub(point, at))
}

/// The angle between two turns, degrees.
pub(crate) fn angle(a: Q4, b: Q4) -> f32 {
    let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs().min(1.0);
    2.0 * d.acos().to_degrees()
}

/// The inertia as a tensor in the body's axes, from its principal figures
/// and their axes' turn: its six figures (xx, yy, zz, xy, xz, yz).
pub(crate) fn tensor(turn: Q4, principal: V3) -> [f32; 6] {
    let axes = [rotate(turn, [1.0, 0.0, 0.0]), rotate(turn, [0.0, 1.0, 0.0]), rotate(turn, [0.0, 0.0, 1.0])];
    let at = |i: usize, j: usize| (0..3).map(|k| principal[k] * axes[k][i] * axes[k][j]).sum::<f32>();
    [at(0, 0), at(1, 1), at(2, 2), at(0, 1), at(0, 2), at(1, 2)]
}

/// The comparison of the trace at `path` (recorder 0.10.0 on: course 0 of
/// `courses2`) with the corpse as `corpse::build` makes it from `data`.
pub fn compare(path: &str, data: &RagdollData) -> Result<Report> {
    let text = std::fs::read_to_string(path).with_context(|| path.to_owned())?;
    let (mut corpse_frame, mut weapon_frame) = (None, None);
    for line in text.lines().skip(1) {
        if corpse_frame.is_none() && line.contains("\"corpse\":") {
            corpse_frame = Some(serde_json::from_str::<Value>(line)?);
        } else if corpse_frame.is_some() && line.contains("\"weapon_setup\":") {
            weapon_frame = Some(serde_json::from_str::<Value>(line)?);
            break;
        }
    }
    let frame = corpse_frame.ok_or_else(|| anyhow!("no corpse set-up in {path} (recorder 0.10.0 on writes one)"))?;
    let install = trp_data::Install::locate()?;
    let physics = trp_data::setup::engine::PhysicsSettings::load(&install)?;
    let engine = trp_data::setup::engine::EngineSettings::load(&install)?;
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install)?;
    let depenetration = hard.float("CorpseMaxDepenetrationVelocity")?;
    let mut report = Report::default();

    // The engine.
    report.head("The engine (recorded at the death; ours the install's)");
    let e = &frame["engine"];
    report.vector("gravity", v3(&e["gravity"]), physics.gravity, 1e-5);
    report.number("bounce threshold", f(&e["bounce_threshold"]), physics.bounce_threshold, 1e-6);
    report.number("contact offset", f(&e["contact_offset"]), physics.contact_offset, 1e-6);
    report.number("sleep threshold", f(&e["sleep_threshold"]), physics.sleep_threshold, 1e-7);
    report.number("solver iterations", f(&e["solver_iterations"]), physics.solver_iterations as f32, 0.0);
    report.number("solver velocity iterations", f(&e["solver_velocity_iterations"]), physics.solver_velocity_iterations as f32, 0.0);
    report.number("most spin", f(&e["max_angular_speed"]), physics.max_angular_speed, 1e-6);
    report.number("a new body's most speed out of what it is in", f(&e["max_depenetration"]), physics.default_max_depenetration, 1e-6);
    report.number("the fixed step", f(&e["fixed_dt"]), engine.fixed_timestep, 1e-7);
    report.number("the smooth step's most", f(&e["max_smooth_dt"]), hard.float("MaxSmoothPhysicsDeltaTime")?, 1e-7);
    report.number("the smooth step's frames", f(&e["game_samples"]), corpse::Stepper::SAMPLES as f32, 0.0);
    report.text("the game's step", e["game_update_mode"].as_str(), "SmoothSimulate");
    report.text("the engine stepped by", e["simulation_mode"].as_str(), "Script");
    report.note(format!("the smooth step at the death: {} s", f(&e["game_avg_dt"]).unwrap_or(f32::NAN)));

    // The corpse as built from the recorded poses.
    let corpse = &frame["corpse"];
    let died = &frame["died"];
    let velocity = v3(&died["velocity"]).unwrap_or([0.0; 3]);
    let mut poses: HashMap<String, (V3, Q4)> = HashMap::new();
    for b in corpse["bodies"].as_array().into_iter().flatten() {
        if let (Some(at), Some(turn)) = (v3(&b["pos"]), q4(&b["rot"])) {
            poses.insert(last(b["path"].as_str().unwrap_or("")).to_owned(), (at, turn));
        }
    }
    // The bones without a body that colliders hang under (the neck).
    for (name, b) in frame["bones"].as_object().into_iter().flatten() {
        if let (false, Some(at), Some(turn)) = (poses.contains_key(name), b.get(0).and_then(v3), b.get(1).and_then(q4)) {
            poses.insert(name.clone(), (at, turn));
        }
    }
    let mut world = World::new(Settings::of(&physics));
    let deadbody = physics.layer("Deadbody").unwrap_or(23);
    let built = corpse::build(&mut world, data, &poses, velocity, depenetration, deadbody);
    let mass_of: HashMap<&str, f32> = data.bodies.iter().map(|b| (b.bone.as_str(), b.mass)).collect();

    report.head("The corpse's bodies");
    let start = corpse::start_velocity(velocity);
    for b in corpse["bodies"].as_array().into_iter().flatten() {
        let name = last(b["path"].as_str().unwrap_or(""));
        let (Some(ours), Some(body)) = (data.bodies.iter().find(|d| d.bone == name), built.get(name)) else {
            report.note(format!("{name}: not a body of the ragdoll (kinematic {}, mass {})", b["kinematic"], b["mass"]));
            continue;
        };
        let short = name.trim_start_matches("Base Human");
        report.number(&format!("{short} mass"), f(&b["mass"]), ours.mass, 1e-5);
        report.number(&format!("{short} drag"), f(&b["drag"]), ours.drag, 1e-6);
        report.number(&format!("{short} angular drag"), f(&b["angular_drag"]), ours.angular_drag, 1e-6);
        report.number(&format!("{short} most speed out of what it is in"), f(&b["max_depenetration"]), depenetration, 1e-6);
        report.number(&format!("{short} most spin"), f(&b["max_angular"]), physics.max_angular_speed, 1e-6);
        report.number(&format!("{short} sleep threshold"), f(&b["sleep_threshold"]), physics.sleep_threshold, 1e-7);
        let solver = b["solver"].as_array().map(|s| s.iter().filter_map(f).collect::<Vec<_>>()).unwrap_or_default();
        report.number(&format!("{short} solver iterations"), solver.first().copied(), physics.solver_iterations as f32, 0.0);
        report.number(&format!("{short} solver velocity iterations"), solver.get(1).copied(), physics.solver_velocity_iterations as f32, 0.0);
        report.text(&format!("{short} detection"), b["detection"].as_str(), "Discrete");
        report.flag(&format!("{short} kinematic"), b["kinematic"].as_bool(), false);
        report.flag(&format!("{short} gravity"), b["gravity"].as_bool(), true);
        let (v, w) = world.velocity(*body);
        report.vector(&format!("{short} velocity"), v3(&b["vel"]), v, 1e-4);
        report.vector(&format!("{short} spin"), v3(&b["ang"]), w, 1e-4);
        report.vector(&format!("{short} velocity by the rule"), v3(&b["vel"]), start, 1e-4);
        let (centre, turn, principal) = world.mass_frame(*body);
        report.vector(&format!("{short} centre of mass"), v3(&b["centre_of_mass"]), centre, 0.001);
        if let (Some(rp), Some(rt)) = (v3(&b["inertia"]), q4(&b["inertia_rot"])) {
            let (r, o) = (tensor(rt, rp), tensor(turn, principal));
            let most = r.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            let apart = r.iter().zip(&o).fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
            report.put(
                apart <= most * 0.01,
                format!("{short} inertia: recorded {} about axes turned {:?}, ours {} about {:?} (as tensors {:.2}% apart)", show(rp), rt, show(principal), turn, 100.0 * apart / most.max(1e-12)),
            );
        } else {
            report.put(false, format!("{short} inertia: not recorded, ours {}", show(principal)));
        }
    }

    report.head("The corpse's colliders (recorded: those switched on, not triggers, on a body)");
    let mut recorded_on: HashMap<String, Vec<&Value>> = HashMap::new();
    let (mut off, mut triggers) = (0, 0);
    for c in corpse["colliders"].as_array().into_iter().flatten() {
        let on = c["enabled"].as_bool().unwrap_or(false);
        let trigger = c["trigger"].as_bool().unwrap_or(false);
        let body = last(c["body"].as_str().unwrap_or("")).to_owned();
        match (on, trigger, body.is_empty()) {
            (true, false, false) => recorded_on.entry(body).or_default().push(c),
            (_, true, _) => triggers += 1,
            _ => off += 1,
        }
    }
    report.note(format!("recorded besides: {off} switched off (the armour's plates, the gear's loot colliders, the character's own capsules) and {triggers} triggers"));
    for b in &data.bodies {
        let short = b.bone.trim_start_matches("Base Human");
        let recorded = recorded_on.remove(&b.bone).unwrap_or_default();
        let Some((body_at, body_turn)) = poses.get(&b.bone).copied() else { continue };
        let mut matched = vec![false; recorded.len()];
        for c in &b.colliders {
            let ours = corpse::collider_posed(c, (body_at, body_turn), &poses);
            let Some(index) = recorded.iter().enumerate().position(|(i, r)| !matched[i] && last(r["path"].as_str().unwrap_or("")) == c.object) else {
                report.put(false, format!("{short}: ours has {} the game has not switched on", c.object));
                continue;
            };
            matched[index] = true;
            let r = recorded[index];
            let what = format!("{short} / {}", c.object);
            let (Some(pos), Some(rot)) = (v3(&r["pos"]), q4(&r["rot"])) else { continue };
            let turn = mul(inverse(body_turn), rot);
            let place = into(body_at, body_turn, pos);
            let centre = v3(&r["centre"]).unwrap_or([0.0; 3]);
            let at = add(place, rotate(turn, centre));
            let kind = r["kind"].as_str().unwrap_or("");
            match (&ours.shape, kind) {
                (Shape::Box { half }, "BoxCollider") => {
                    let size = v3(&r["size"]).map(|s| [s[0] / 2.0, s[1] / 2.0, s[2] / 2.0]);
                    report.vector(&format!("{what} box's half size"), size, *half, 1e-4);
                }
                (Shape::Sphere { radius }, "SphereCollider") => report.number(&format!("{what} radius"), f(&r["radius"]), *radius, 1e-4),
                (Shape::Capsule { radius, height, direction }, "CapsuleCollider") => {
                    report.number(&format!("{what} radius"), f(&r["radius"]), *radius, 1e-4);
                    report.number(&format!("{what} height"), f(&r["height"]), *height, 1e-4);
                    report.number(&format!("{what} axis"), f(&r["direction"]), *direction as f32, 0.0);
                }
                (shape, kind) => report.put(false, format!("{what}: recorded a {kind}, ours {shape:?}")),
            }
            report.vector(&format!("{what} centre in the body"), Some(at), ours.at, 0.001);
            let apart = angle(turn, ours.turn);
            report.put(apart <= 0.1, format!("{what} turn in the body: {apart:.3} degrees apart"));
            let m = &r["material"];
            let theirs = if m.is_null() {
                Material::default()
            } else {
                let combine = |name: &str| match m[name].as_str() {
                    Some("Minimum") => Combine::Minimum,
                    Some("Multiply") => Combine::Multiply,
                    Some("Maximum") => Combine::Maximum,
                    _ => Combine::Average,
                };
                Material {
                    dynamic_friction: f(&m["dynamic_friction"]).unwrap_or(f32::NAN),
                    static_friction: f(&m["static_friction"]).unwrap_or(f32::NAN),
                    bounciness: f(&m["bounciness"]).unwrap_or(f32::NAN),
                    friction_combine: combine("friction_combine"),
                    bounce_combine: combine("bounce_combine"),
                }
            };
            let name = m["name"].as_str().unwrap_or("none, the engine's default");
            report.put(theirs == ours.material, format!("{what} material: recorded {name} {theirs:?}, ours {:?}", ours.material));
            report.number(&format!("{what} contact offset"), f(&r["contact_offset"]), physics.contact_offset, 1e-6);
            report.text(&format!("{what} layer"), r["layer"].as_str(), "Deadbody");
        }
        for (r, _) in recorded.iter().zip(&matched).filter(|(_, m)| !**m) {
            report.put(false, format!("{short}: the game has {} switched on, ours has not", last(r["path"].as_str().unwrap_or(""))));
        }
    }
    for (body, list) in recorded_on {
        report.put(false, format!("{} colliders switched on in a body ours has not ({body})", list.len()));
    }

    report.head("The corpse's joints");
    for r in corpse["joints"].as_array().into_iter().flatten() {
        let name = last(r["path"].as_str().unwrap_or(""));
        let Some(j) = data.joints.iter().find(|j| j.bone == name) else {
            report.put(false, format!("{name}: a joint ours has not"));
            continue;
        };
        let short = name.trim_start_matches("Base Human");
        report.text(&format!("{short} kind"), r["kind"].as_str(), "CharacterJoint");
        report.text(&format!("{short} hangs from"), Some(last(r["connected"].as_str().unwrap_or(""))), &j.connected);
        report.vector(&format!("{short} anchor"), v3(&r["anchor"]), j.anchor, 1e-6);
        report.vector(&format!("{short} axis"), v3(&r["axis"]), j.axis, 1e-6);
        report.vector(&format!("{short} swing axis"), v3(&r["swing_axis"]), j.swing_axis, 1e-6);
        for (field, limit) in [("low_twist", j.low_twist), ("high_twist", j.high_twist), ("swing1", j.swing1), ("swing2", j.swing2)] {
            let l = r[field].as_array().map(|a| a.iter().filter_map(f).collect::<Vec<_>>()).unwrap_or_default();
            report.number(&format!("{short} {field} limit"), l.first().copied(), limit.limit, 1e-5);
            report.number(&format!("{short} {field} bounciness"), l.get(1).copied(), limit.bounciness, 1e-6);
            report.number(&format!("{short} {field} contact distance"), l.get(2).copied(), limit.contact_distance, 1e-6);
        }
        for (field, spring) in [("twist_spring", j.twist_spring), ("swing_spring", j.swing_spring)] {
            let s = r[field].as_array().map(|a| a.iter().filter_map(f).collect::<Vec<_>>()).unwrap_or_default();
            report.number(&format!("{short} {field} spring"), s.first().copied(), spring[0], 1e-6);
            report.number(&format!("{short} {field} damper"), s.get(1).copied(), spring[1], 1e-6);
        }
        report.flag(&format!("{short} its two bodies meet"), r["collision"].as_bool(), j.enable_collision);
        let projection = r["projection"].as_array().cloned().unwrap_or_default();
        report.flag(&format!("{short} projection"), projection.first().and_then(Value::as_bool), true);
        report.number(&format!("{short} projection's distance"), projection.get(1).and_then(f), PROJECTION[0], 1e-6);
        report.number(&format!("{short} projection's angle"), projection.get(2).and_then(f), PROJECTION[1], 1e-6);
        report.flag(&format!("{short} preprocessing"), r["preprocessing"].as_bool(), false);
        let scale = mass_of.get(j.connected.as_str()).zip(mass_of.get(j.bone.as_str())).map_or(f32::NAN, |(c, o)| c / o);
        report.number(&format!("{short} mass scale"), f(&r["mass_scale"]), scale, 1e-5);
        report.number(&format!("{short} connected mass scale"), f(&r["connected_mass_scale"]), 1.0, 1e-6);
        report.flag(&format!("{short} connected anchor worked out"), r["auto_anchor"].as_bool(), j.auto_anchor);
        let breaks = r["break"].as_array().map(|a| a.iter().all(Value::is_null)).unwrap_or(false);
        report.flag(&format!("{short} never breaks"), Some(breaks), true);
        // Where the connected anchor was worked out: ours is the joint's
        // point in the connected body's axes at the skeleton's rest
        // (`corpse::build`'s `rest`); as the two stood at the death, for
        // the difference.
        if let (Some(&(own_at, own_turn)), Some(&(con_at, con_turn)), Some(own), Some(con)) =
            (poses.get(&j.bone), poses.get(&j.connected), data.bodies.iter().find(|b| b.bone == j.bone), data.bodies.iter().find(|b| b.bone == j.connected))
        {
            let at_death = into(con_at, con_turn, add(own_at, rotate(own_turn, j.anchor)));
            let at_rest = into(con.rest.0, con.rest.1, add(own.rest.0, rotate(own.rest.1, j.anchor)));
            let recorded = v3(&r["connected_anchor"]);
            report.vector(&format!("{short} connected anchor (ours at the skeleton's rest)"), recorded, at_rest, 0.0005);
            report.note(format!("{short} connected anchor as the two stood at the death: {} ({:.4} m from the recorded)", show(at_death), recorded.map_or(f32::NAN, |r| distance(r, at_death))));
        }
    }

    // The weapon let go.
    let Some(wf) = weapon_frame else {
        report.head("No weapon let go in this recording");
        return Ok(report);
    };
    let setup = &wf["weapon_setup"];
    report.head(format!("The weapon let go (recorded {} s after the death)", f(&wf["dead_t"]).unwrap_or(f32::NAN)));
    let Some(b) = setup["bodies"].as_array().and_then(|a| a.first()) else {
        report.note("no body recorded");
        return Ok(report);
    };
    let joint = setup["joints"].as_array().and_then(|a| a.first());
    let pistol = joint.and_then(|j| j["kind"].as_str()) == Some("FixedJoint");
    report.number("mass", f(&b["mass"]), if pistol { corpse::PISTOL_MASS } else { WEAPON_MASS }, 1e-6);
    report.number("drag", f(&b["drag"]), 0.0, 1e-6);
    report.number("angular drag", f(&b["angular_drag"]), UNITY_ANGULAR_DRAG, 1e-6);
    report.number("most speed out of what it is in", f(&b["max_depenetration"]), depenetration, 1e-6);
    report.text("detection", b["detection"].as_str(), "Discrete");
    report.vector("velocity (the player's, not held)", v3(&b["vel"]), velocity, 1e-4);
    report.note(format!(
        "centre of mass {} (the game moves the weapon's objects back by it, `Player.DropItemDead`: the body's place is its centre of mass; ours keeps the weapon's root, the same body); inertia {} about axes turned {}",
        v3(&b["centre_of_mass"]).map_or("-".into(), show),
        v3(&b["inertia"]).map_or("-".into(), show),
        b["inertia_rot"]
    ));
    if let Some(j) = joint {
        if pistol {
            report.note("held fast to the right forearm");
        } else {
            report.text("joint", j["kind"].as_str(), "SpringJoint");
            let s = j["spring"].as_array().map(|a| a.iter().filter_map(f).collect::<Vec<_>>()).unwrap_or_default();
            report.number("spring", s.first().copied(), STRAP_SPRING, 1e-6);
            report.number("damper", s.get(1).copied(), STRAP_DAMPER, 1e-6);
            report.number("least distance", s.get(2).copied(), 0.0, 1e-6);
            report.number("most distance", s.get(3).copied(), 0.0, 1e-6);
            report.number("tolerance (PhysX's own, 0.025)", s.get(4).copied(), 0.025, 1e-6);
            report.vector("its point on the pelvis", v3(&j["connected_anchor"]), STRAP_ON_PELVIS, 1e-6);
            report.flag("connected anchor worked out", j["auto_anchor"].as_bool(), false);
            // Its point on the weapon: the palm of the weapon's own arm
            // (`TransformLinks`' `HumanRPalm`, the first person's, which
            // the app's corpse takes its arms from), 10 cm up and 10 cm
            // back. The recording has only the body's palm after the
            // death: where the hand is on the grip the two are one.
            let palm = wf["bones"]["Base HumanRPalm"].as_array().and_then(|a| a.first()).and_then(v3);
            if let (Some(palm), Some(at), Some(turn), Some(anchor)) = (palm, v3(&b["pos"]), q4(&b["rot"]), v3(&j["anchor"])) {
                let by_body = add(into(at, turn, palm), STRAP_BY_PALM);
                report.note(format!(
                    "its point on the weapon: recorded {}; by the body's palm {} ({:.4} m apart; the game's is the weapon's own arm's palm, not recorded)",
                    show(anchor),
                    show(by_body),
                    distance(anchor, by_body)
                ));
            }
        }
        report.flag("its two bodies meet", j["collision"].as_bool(), true);
        report.flag("preprocessing", j["preprocessing"].as_bool(), false);
        let to = if pistol { "Base HumanRForearm1" } else { "Base HumanPelvis" };
        let mass = if pistol { corpse::PISTOL_MASS } else { WEAPON_MASS };
        report.number("mass scale", f(&j["mass_scale"]), mass_of.get(to).copied().unwrap_or(f32::NAN) / mass, 1e-5);
        report.number("connected mass scale", f(&j["connected_mass_scale"]), 1.0, 1e-6);
    }
    // Its colliders, as `DropItemDead` switches them on.
    let (mut on, mut lods, mut materials, mut kept_off) = (0, HashMap::<String, usize>::new(), HashMap::<String, usize>::new(), Vec::new());
    for c in setup["colliders"].as_array().into_iter().flatten() {
        let name = last(c["path"].as_str().unwrap_or(""));
        if c["enabled"].as_bool() == Some(true) && c["trigger"].as_bool() != Some(true) {
            on += 1;
            let lod = name.rsplit('_').next().filter(|s| s.starts_with("LOD")).unwrap_or("no level").to_owned();
            *lods.entry(lod).or_default() += 1;
            *materials.entry(c["material"]["name"].as_str().unwrap_or("none").to_owned()).or_default() += 1;
            if c["layer"].as_str() != Some("Deadbody") {
                report.put(false, format!("weapon collider {name} switched on, on {}", c["layer"]));
            }
        } else if c["trigger"].as_bool() != Some(true) {
            kept_off.push(format!("{name} ({})", c["layer"].as_str().unwrap_or("?")));
        }
    }
    let mut lods: Vec<_> = lods.into_iter().collect();
    lods.sort();
    report.note(format!(
        "colliders switched on: {on} ({}), by material {:?}; solid ones kept off: {} ({})",
        lods.iter().map(|(l, n)| format!("{n} {l}")).collect::<Vec<_>>().join(", "),
        materials,
        kept_off.len(),
        kept_off.iter().map(|s| s.as_str()).take(4).collect::<Vec<_>>().join(", ")
    ));
    Ok(report)
}
