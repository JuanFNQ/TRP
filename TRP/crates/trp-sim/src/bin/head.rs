//! Sets the head's push out of walls (`trp_sim::movement::pom`) beside a
//! recording that has it (recorder 0.10.0 on: `head_capsule`, `head_push`,
//! `motion_before_limit`, `motion`), frame by frame (movement spec, "The
//! head's push out of walls", "Compared with").
//!
//!   head <trace.jsonl>
//!
//! A recording by hand has no script, and the walls its pushes came from are
//! not in it (the game's meshes cannot be read), so the push itself cannot
//! be worked out again here. Two rules can be set on what the game recorded:
//!
//! - **Where the capsule is.** Standing: its object on the camera's
//!   container by the prefab's place (`pom::on_container`), the container
//!   worked back from the recorded camera (`eye` less the camera's own place
//!   in it, turned as the camera's world turn less its own); lying: over
//!   the second spine bone in the body's lay, no higher than 0.2 m
//!   (`pom::over_pivot`), the bone 0.1624 m along the first's recorded turn.
//!   Beside the recorded `head_capsule`, in the player's axes.
//! - **The limit.** The motion before the limit with the recorded push
//!   added by `pom::limited`, beside the recorded motion after it, in the
//!   states that call it (not going down, a vault or a climb).

use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_data::player::HeadCollidersData;
use trp_data::unity::Assets;
use trp_data::Install;
use trp_sim::math::{quat_axis, quat_mul, quat_rotate};
use trp_sim::movement::pom::{limited, on_container, over_pivot};

fn floats(v: Option<&Json>) -> Vec<f32> {
    v.and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).map(|x| x as f32).collect()).unwrap_or_default()
}

fn three(v: Option<&Json>) -> Option<[f32; 3]> {
    let f = floats(v);
    (f.len() >= 3).then(|| [f[0], f[1], f[2]])
}

fn four(v: Option<&Json>) -> Option<[f32; 4]> {
    let f = floats(v);
    (f.len() >= 4).then(|| [f[0], f[1], f[2], f[3]])
}

fn inverse(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
}

/// The largest gap seen, its mean, and where.
#[derive(Default)]
struct Gap {
    worst: f32,
    at: u64,
    sum: f64,
    frames: usize,
}

impl Gap {
    fn see(&mut self, gap: f32, frame: u64) {
        self.frames += 1;
        self.sum += f64::from(gap);
        if gap > self.worst {
            self.worst = gap;
            self.at = frame;
        }
    }

    fn line(&self, what: &str) -> String {
        let mean = if self.frames > 0 { self.sum / self.frames as f64 } else { 0.0 };
        format!("{what}: {} frames, mean {mean:.6} m, largest {:.6} m (frame {})", self.frames, self.worst, self.at)
    }
}

/// The second spine bone's place in its parent, the first (the prefab's).
const SPINE2_IN_SPINE1: [f32; 3] = [-0.162_407_76, 0.0, 0.0];

fn main() -> Result<()> {
    let path = std::env::args().nth(1).context("head <trace.jsonl>")?;
    let head = HeadCollidersData::load(&mut Assets::new(Install::locate()?)?)?;
    println!(
        "standing: radius {}, height {}, its object at {:?} on the container; lying: radius {}, height {}, over {}",
        head.standing.radius, head.standing.height, head.standing.position, head.lying.radius, head.lying.height, head.lying_pivot
    );
    let reader = BufReader::new(std::fs::File::open(&path).with_context(|| path.clone())?);
    let (mut standing, mut lying, mut lying_down, mut limit, mut pushed) = (Gap::default(), Gap::default(), Gap::default(), Gap::default(), Gap::default());
    for line in reader.lines() {
        let Ok(v) = serde_json::from_str::<Json>(&line?) else { continue };
        let Some(frame) = v.get("frame").and_then(Json::as_u64) else { continue };
        let state = v.get("state").and_then(Json::as_str).unwrap_or("");
        let Some(recorded) = three(v.get("head_capsule")) else { continue };
        let lying_shape = matches!(state, "Transit2Prone" | "ProneIdle" | "ProneMove" | "Roll" | "Prone2Stand");

        // Where the capsule is.
        if lying_shape {
            if let (Some(p), Some(q)) = (three(v.get("spine1_p")), four(v.get("spine1_q"))) {
                let along = quat_rotate(q, SPINE2_IN_SPINE1);
                let spine2: [f32; 3] = std::array::from_fn(|k| p[k] + along[k]);
                let tilt = four(v.get("body_tilt")).unwrap_or([0.0, 0.0, 0.0, 1.0]);
                let ours = quat_rotate(tilt, over_pivot(quat_rotate(inverse(tilt), spine2)));
                // Going down the game does not move the capsule (nothing asks it).
                if state == "Transit2Prone" { &mut lying_down } else { &mut lying }.see(distance(ours, recorded), frame);
            }
        } else if let (Some(eye), Some(own_place), Some(own_turn), Some(world_turn)) =
            (three(v.get("eye")), three(v.get("camera_pos")), four(v.get("camera_rot")), four(v.get("camera_world_rot")))
        {
            let body_yaw = v.get("body_yaw").and_then(Json::as_f64).unwrap_or(0.0) as f32;
            let container_turn = quat_mul(inverse(quat_axis(1, body_yaw)), quat_mul(world_turn, inverse(own_turn)));
            let offset = quat_rotate(container_turn, own_place);
            let container: [f32; 3] = std::array::from_fn(|k| eye[k] - offset[k]);
            standing.see(distance(on_container(&head.standing, container, container_turn), recorded), frame);
        }

        // The limit, on the player's clock.
        let limits = !matches!(state, "Transit2Prone" | "ClimbUp" | "ClimbOver" | "VaultLanding");
        let (Some(push), Some(before), Some(after)) = (three(v.get("head_push")), three(v.get("motion_before_limit")), three(v.get("motion"))) else { continue };
        let dt = v.get("clock_dt").and_then(Json::as_f64).unwrap_or(0.0) as f32;
        if !limits || dt <= 0.0 || before == [0.0; 3] && after != [0.0; 3] {
            continue;
        }
        let ours = limited(before.map(|m| m * dt), push, state == "ProneIdle");
        let gap = distance(ours, after.map(|m| m * dt));
        if push[0] * push[0] + push[2] * push[2] > 0.0 { &mut pushed } else { &mut limit }.see(gap, frame);
    }
    println!("{}", standing.line("the standing capsule's place"));
    println!("{}", lying.line("the lying capsule's place, lying still, crawling and getting up"));
    println!("{}", lying_down.line("the lying capsule's place going down (not moved by the game then)"));
    println!("{}", pushed.line("the motion after the limit, with a push"));
    println!("{}", limit.line("the motion after the limit, with none"));
    Ok(())
}
