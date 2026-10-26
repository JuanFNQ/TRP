//! Sets the slope's rules (`trp_sim::movement::slope`) beside a recording
//! that has them (recorder 0.10.0 on: `surface_normal`, `normal_avg`,
//! `slope`, `ground_hit`, `motion_before_limit`), frame by frame (movement
//! spec, "Slopes and sliding", "Compared with").
//!
//!   slope <trace.jsonl> [--every]
//!
//! A recording by hand has no script and its ground is not known beyond
//! the path, so the movement cannot be walked through it. Each rule is set
//! on what the game recorded going into it, and its result beside what the
//! game recorded coming out:
//!
//! - **The average**: our five-frame history fed each frame's recorded
//!   surface normal, beside the recorded `normal_avg`.
//! - **The surface**: where the ray found ground no steeper than the
//!   threshold the surface is the hit's; where it found steeper, the
//!   flatter of the hit and the capsule's contact (the contact is not
//!   recorded: the surface is checked to be no steeper than the hit).
//! - **The slide's pull**: ours from the recorded surface and average,
//!   beside the recorded `slope`.
//! - **The motion laid on the ground**: the recorded motion before the
//!   limit less the pull (or gravity's part) is the motion as laid; its
//!   level part is what went in (laying keeps it), taken with no rise of
//!   its own. Ours laid from that beside the recorded rise.

use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_sim::movement::slope::{along_surface, slide, surface_normal, NormalHistory, SLOPE_THRESHOLD};

/// The recorder's names of the movement states whose motion is the base
/// one, laid on the ground where it goes down a slope.
const BASE_STATES: [&str; 7] = ["Idle", "Run", "Sprint", "ProneMove", "Prone2Stand", "Stop", "Landing"];

fn floats(v: Option<&Json>) -> Vec<f32> {
    v.and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).map(|x| x as f32).collect()).unwrap_or_default()
}

fn three(v: Option<&Json>) -> Option<[f32; 3]> {
    let f = floats(v);
    (f.len() >= 3).then(|| [f[0], f[1], f[2]])
}

/// The largest gap seen, and where.
#[derive(Default)]
struct Gap {
    worst: f32,
    at: u64,
    frames: usize,
}

impl Gap {
    fn see(&mut self, gap: f32, frame: u64) {
        self.frames += 1;
        if gap > self.worst {
            self.worst = gap;
            self.at = frame;
        }
    }

    fn line(&self, what: &str) -> String {
        format!("{what}: {} frames, largest gap {:.6} (frame {})", self.frames, self.worst, self.at)
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().context("slope <trace.jsonl> [--every]")?;
    let every = args.iter().any(|a| a == "--every");
    let reader = BufReader::new(std::fs::File::open(path).with_context(|| path.clone())?);
    let mut gravity = -9.81f32;
    let mut history = NormalHistory::default();
    let (mut average, mut surface, mut pull, mut laid, mut level) = (Gap::default(), Gap::default(), Gap::default(), Gap::default(), Gap::default());
    let (mut sliding, mut laid_frames, mut steep_hits, mut no_hit) = (0usize, 0usize, 0usize, 0usize);
    let mut previous_freefall = 0.0f32;
    let mut last_frame: Option<u64> = None;
    for line in reader.lines() {
        let line = line?;
        let v: Json = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(g) = three(v.get("gravity")) {
            gravity = g[1];
            continue;
        }
        let Some(frame) = v.get("frame").and_then(Json::as_u64) else { continue };
        let (Some(normal), Some(recorded_average), Some(recorded_slope), Some(before_limit)) = (
            three(v.get("surface_normal")),
            v.get("normal_avg").and_then(Json::as_f64).map(|x| x as f32),
            three(v.get("slope")),
            three(v.get("motion_before_limit")),
        ) else {
            continue;
        };
        if last_frame.is_some_and(|last| frame != last + 1) {
            println!("frame {frame} follows {}: the history starts again", last_frame.unwrap_or(0));
            history = NormalHistory::default();
        }
        last_frame = Some(frame);
        let grounded = v.get("grounded").and_then(Json::as_bool).unwrap_or(true);
        let freefall = v.get("freefall").and_then(Json::as_f64).map_or(0.0, |x| x as f32);
        let hit = v.get("ground_hit").and_then(Json::as_array).and_then(|h| {
            h.first().and_then(Json::as_bool).filter(|hit| *hit).and_then(|_| three(h.get(1)))
        });

        // The average.
        history.add(normal[1]);
        let ours_average = history.average();
        average.see((ours_average - recorded_average).abs(), frame);

        // The surface, as far as the recording tells it.
        match hit {
            Some(hit) if hit[1] >= SLOPE_THRESHOLD => {
                let ours = surface_normal(Some(hit), [0.0, 1.0, 0.0]);
                surface.see((0..3).map(|k| (ours[k] - normal[k]).abs()).fold(0.0, f32::max), frame);
            }
            Some(hit) => {
                steep_hits += 1;
                // Ours is the flatter of the two: never steeper than the hit.
                surface.see((hit[1] - normal[1]).max(0.0), frame);
            }
            None => no_hit += 1,
        }

        // The slide's pull, from the recorded surface and average.
        let ours_pull = slide(normal, recorded_average);
        if recorded_slope != [0.0; 3] {
            sliding += 1;
        }
        pull.see((0..3).map(|k| (ours_pull[k] - recorded_slope[k]).abs()).fold(0.0, f32::max), frame);

        // The motion laid on the ground, in the states that take the base
        // motion (`MovementState.ApplyMotion`): not lying still, rolling,
        // going down, jumping or in a vault or climb, whose own motion has
        // no laying (and a vault's or a climb's no limit, which leaves the
        // recorded figure as it was before). What gravity or the pull
        // added comes off first. A long fall shrinks the motion as well:
        // left out.
        let state = v.get("state").and_then(Json::as_str).unwrap_or("");
        if !BASE_STATES.contains(&state) {
            previous_freefall = freefall;
            continue;
        }
        // Gravity goes by the fall's time as the frame found it: the frame
        // before's figure (the recorder writes it once the frame has added
        // its own time, or put it to nought on the ground).
        let fallen = std::mem::replace(&mut previous_freefall, freefall);
        if fallen >= 0.25 && !grounded {
            continue;
        }
        let added: [f32; 3] = if recorded_slope != [0.0; 3] {
            recorded_slope.map(|k| k * gravity)
        } else {
            [0.0, gravity * fallen, 0.0]
        };
        let went_in = [before_limit[0] - added[0], 0.0, before_limit[2] - added[2]];
        let ours = along_surface(went_in, normal, recorded_average);
        let recorded_rise = before_limit[1] - added[1];
        if ours != went_in {
            laid_frames += 1;
            laid.see((ours[1] - recorded_rise).abs(), frame);
            if every {
                println!("{frame}: laid rise ours {:.5} game {:.5} (surface {:.4}, average {:.4})", ours[1], recorded_rise, normal[1], recorded_average);
            }
        } else {
            level.see(recorded_rise.abs(), frame);
        }
    }
    println!("{}", average.line("the average of the surface's upward part"));
    println!("{} ({steep_hits} with a hit steeper than the threshold, {no_hit} with no hit, not checked)", surface.line("the surface from the ray's hit"));
    println!("{} ({sliding} sliding in the game)", pull.line("the slide's pull"));
    println!("{} ({laid_frames} laid by ours)", laid.line("the motion's rise where it is laid on the ground"));
    println!("{}", level.line("the motion's rise where it is not (the animation's own, left in)"));
    Ok(())
}
