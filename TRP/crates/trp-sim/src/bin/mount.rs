//! Sets a recording of mounts made by hand (recorder 0.10.0 on: the `mount`
//! field while mounted) beside the mount's rules, frame by frame (mounting
//! spec, "Compared with").
//!
//!   mount <trace.jsonl> [<weapon template>]
//!
//! A recording by hand has no script, so the movement cannot be walked
//! through it. The mount's rules can be set on what it recorded:
//!
//! - **Each mount's data** (`trp_sim::mounting::data_for`) from the recorded
//!   point, direction and side and the character as the frame before the
//!   mount had it (place, look, stance), with our pivot for the weapon (its
//!   template's, or the left hand's marker's), on ground at the recorded
//!   target's height: the target place, the look's target yaw, the limits
//!   and the approach, ours beside the game's; and the stance the mount sets
//!   at its start.
//! - **The approach**: on each of its frames the share of the time (on the
//!   player's whole-millisecond clock, `clock_dt`) and the look by our rule
//!   (a lerp of a lerp toward the targets) from the recorded look at the
//!   start, beside the recorded transition and look.
//! - **Held**: on every frame after the approach, the stance by our rule on
//!   the recorded pitch and limits (`UpdatePoseLevel`) and the lean by our
//!   rule on the recorded yaw (`UpdateCurrentTilt`), beside the recorded.
//! - **The exit**: from the frame the weapon's motion is told it is no
//!   longer mounted (`pwa_mounted` down) to the frame the state is off, and
//!   the transition's share on the way, beside 1 less the time over 0.2 s.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::math::{delta_angle, inverse_lerp, lerp, lerp_angle};
use trp_sim::mounting::{data_for, Asking, Hit, Layer, Point, Settings, Side, WeaponMount};
use trp_sim::World;

/// Ground at one height, nothing on it: what the target place's ray meets.
struct Floor(f32);

impl World for Floor {
    fn ground_height(&self, _x: f32, _z: f32) -> f32 {
        self.0
    }

    fn ray(&self, from: [f32; 3], direction: [f32; 3], length: f32, _layer: Layer) -> Option<Hit> {
        if direction[1] >= 0.0 || from[1] < self.0 {
            return None;
        }
        let distance = (from[1] - self.0) / -direction[1];
        (distance <= length).then(|| Hit { distance, point: std::array::from_fn(|k| from[k] + direction[k] * distance), normal: [0.0, 1.0, 0.0] })
    }
}

fn floats(v: Option<&Json>) -> Vec<f32> {
    v.and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).map(|x| x as f32).collect()).unwrap_or_default()
}

fn number(v: &Json, key: &str) -> Option<f32> {
    v.get(key).and_then(Json::as_f64).map(|x| x as f32)
}

fn three(v: Option<&Json>) -> Option<[f32; 3]> {
    let f = floats(v);
    (f.len() >= 3).then(|| [f[0], f[1], f[2]])
}

/// A frame as the comparison needs it.
struct Frame {
    t: f32,
    clock: f32,
    pos: [f32; 3],
    look: [f32; 2],
    pose: f32,
    tilt: f32,
    mount: Option<Json>,
    /// The clips the engine's sources began on the frame.
    sounds: Vec<String>,
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().context("mount <trace.jsonl> [<weapon template>]")?);
    let text = std::fs::read_to_string(&path)?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Json = serde_json::from_str(lines.next().context("empty trace")?)?;
    let weapon_id = args.next().or_else(|| header.get("weapon").and_then(Json::as_str).map(str::to_owned)).context("no weapon")?;
    let frames: Vec<Frame> = lines
        .filter_map(|line| serde_json::from_str::<Json>(line).ok())
        .map(|f| {
            let look = floats(f.get("look"));
            Frame {
                t: number(&f, "t").unwrap_or(0.0),
                clock: number(&f, "clock_dt").or_else(|| number(&f, "dt")).unwrap_or(0.0),
                pos: three(f.get("pos")).unwrap_or([0.0; 3]),
                look: [look.first().copied().unwrap_or(0.0), look.get(1).copied().unwrap_or(0.0)],
                pose: number(&f, "pose").unwrap_or(1.0),
                tilt: number(&f, "tilt").unwrap_or(0.0),
                mount: f.get("mount").filter(|m| m.is_object()).cloned(),
                sounds: f.get("sounds").and_then(Json::as_array).map(|list| list.iter().filter_map(|s| s.get("clip").and_then(Json::as_str).map(str::to_owned)).collect()).unwrap_or_default(),
            }
        })
        .collect();
    let install = Install::locate()?;
    let db = Database::load(&install)?;
    let mut assets = Assets::new(install)?;
    let weapon = trp_data::weapon::assemble(&mut assets, &db, &weapon_id)?;
    let figures = WeaponMount::of_weapon(&db, &weapon)?;
    let settings = Settings::from_config(&db.config)?;
    println!("== {} ({} frames, {}); our pivot {:?}", path.display(), frames.len(), weapon.name, figures.pivot);

    let mounted = |f: &Frame| f.mount.as_ref().and_then(|m| m.get("mounted")).and_then(Json::as_bool).unwrap_or(false);
    let flag = |f: &Frame, key: &str| f.mount.as_ref().and_then(|m| m.get(key)).and_then(Json::as_bool).unwrap_or(false);
    let mut i = 1;
    while i < frames.len() {
        if !(mounted(&frames[i]) && !mounted(&frames[i - 1])) {
            i += 1;
            continue;
        }
        let start = i;
        let end = (start..frames.len()).find(|&k| !mounted(&frames[k])).unwrap_or(frames.len());
        let (before, at) = (&frames[start - 1], &frames[start]);
        let m = at.mount.as_ref().expect("mounted");
        let point = three(m.get("point")).context("no point")?;
        let direction = three(m.get("direction")).context("no direction")?;
        let side = match m.get("side").and_then(Json::as_str) {
            Some("Left") => Side::Left,
            Some("Right") => Side::Right,
            _ => Side::Forward,
        };
        let target = three(m.get("target")).context("no target")?;
        let recorded = |key: &str| floats(m.get(key));
        let approach = number(m, "approach").unwrap_or(0.0);
        let hands_rot = number(m, "hands_rot").unwrap_or(0.0);
        println!("-- mount at {:.3} s ({} frames to {:.3} s), {:?}", at.t, end - start, frames.get(end).map_or(at.t, |f| f.t), side);
        let [height, centre] = [lerp(1.2, 1.6, before.pose), lerp(0.6, 0.8, before.pose)];
        let asking = Asking {
            position: before.pos,
            yaw: before.look[0],
            pitch: before.look[1],
            pose_level: before.pose,
            prone: false,
            grounded: true,
            weapon: figures,
            bipod_out: flag(at, "bipod_used"),
            capsule: [settings.capsule_radius, height, settings.step_offset, centre],
        };
        let ours = data_for(&Floor(target[1]), &settings, Point { at: point, direction, side }, &asking);
        match ours {
            Some(d) => {
                let gap = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                println!("   target    game {:?}  ours {:?}  ({:.4} m apart across the ground)", target, d.target, gap(target, d.target));
                println!("   look yaw  game {hands_rot:.4}  ours {:.4}", d.target_yaw);
                println!("   yaw lim   game {:?}  ours {:?}", recorded("yaw_limit"), d.yaw_limit);
                println!("   pitch lim game {:?}  ours {:?}", recorded("pitch_limit"), d.pitch_limit);
                println!("   pose lim  game {:?}  ours {:?}", recorded("pose_limit"), d.pose_limit);
                println!("   approach  game {approach:.5}  ours {:.5}", d.approach);
                let up = d.pitch_limit[0];
                let set = lerp(d.pose_limit[0], d.pose_limit[1], inverse_lerp(up, d.pitch_limit[1], 0.0));
                println!("   stance at the start  game {:.4}  ours {set:.4}", at.pose);
            }
            None => println!("   our rule refuses it"),
        }

        // The approach: the share, and the look by the lerp of a lerp.
        let (game_pl, game_pitch, game_yaw) = (recorded("pose_limit"), recorded("pitch_limit"), recorded("yaw_limit"));
        let start_look = before.look;
        let mut timer = 0.0f32;
        let (mut worst_share, mut worst_yaw, mut worst_pitch) = (0.0f32, 0.0f32, 0.0f32);
        let mut held_from = end;
        for k in start..end {
            let f = &frames[k];
            let moving = f.mount.as_ref().and_then(|m| m.get("transition")).and_then(Json::as_array).and_then(|a| a.first()).and_then(Json::as_bool).unwrap_or(false);
            if !moving {
                held_from = k;
                break;
            }
            timer += f.clock;
            let t = timer / approach.max(1e-6);
            let share = floats(f.mount.as_ref().and_then(|m| m.get("transition"))).first().copied();
            let recorded_share = f.mount.as_ref().and_then(|m| m.get("transition")).and_then(Json::as_array).and_then(|a| a.get(1)).and_then(Json::as_f64).map(|x| x as f32);
            let _ = share;
            if let Some(r) = recorded_share {
                worst_share = worst_share.max((r - t).abs());
            }
            let x = lerp_angle(start_look[0], hands_rot, t);
            let y = lerp_angle(start_look[1], 0.0, t);
            let look = [lerp(start_look[0], x, t), lerp(start_look[1], y, t)];
            worst_yaw = worst_yaw.max(delta_angle(look[0], f.look[0]).abs());
            worst_pitch = worst_pitch.max((look[1] - f.look[1]).abs());
        }
        println!("   approach: {} frames; share worst {:.4}, look worst yaw {:.4} pitch {:.4} degrees", held_from - start, worst_share, worst_yaw, worst_pitch);

        // Held: the stance by the pitch, the lean by the yaw.
        let half = (game_yaw.get(1).copied().unwrap_or(0.0) - game_yaw.first().copied().unwrap_or(0.0)).abs() / 2.0;
        let (mut worst_pose, mut worst_tilt, mut held) = (0.0f32, 0.0f32, 0);
        let (mut pose_at, mut tilt_at) = (0.0f32, 0.0f32);
        for f in frames.iter().take(end).skip(held_from) {
            if !flag(f, "pwa_mounted") || game_pl.len() < 2 || game_pitch.len() < 2 {
                continue;
            }
            held += 1;
            let pose = lerp(game_pl[0], game_pl[1], inverse_lerp(game_pitch[0], game_pitch[1], f.look[1]));
            if (pose - f.pose).abs() > worst_pose {
                (worst_pose, pose_at) = ((pose - f.pose).abs(), f.t);
            }
            let offset = delta_angle(hands_rot, f.look[0]).clamp(-half, half);
            let sign = if -offset >= 0.0 { 1.0 } else { -1.0 };
            let tilt = sign * lerp(0.0, 5.0, offset.abs() / half);
            if (tilt - f.tilt).abs() > worst_tilt {
                (worst_tilt, tilt_at) = ((tilt - f.tilt).abs(), f.t);
            }
        }
        println!("   held: {held} frames; stance by the pitch worst {worst_pose:.5} (at {pose_at:.3} s), lean by the yaw worst {worst_tilt:.4} (at {tilt_at:.3} s)");

        // The exit.
        if let Some(leaving) = (held_from..end).find(|&k| !flag(&frames[k], "pwa_mounted")) {
            let mut worst = 0.0f32;
            let mut elapsed = 0.0f32;
            for f in frames.iter().take(end).skip(leaving) {
                elapsed += f.clock;
                if let Some(r) = f.mount.as_ref().and_then(|m| m.get("transition")).and_then(Json::as_array).and_then(|a| a.get(1)).and_then(Json::as_f64) {
                    worst = worst.max((r as f32 - (1.0 - elapsed / settings.exit_time).max(0.0)).abs());
                }
            }
            let off = frames.get(end).map_or(f32::NAN, |f| f.t - frames[leaving].t);
            println!("   exit: {:.4} s from the motion told to the state off; share worst {:.4}", off, worst);
        }
        // The sounds: the kit's medium rattle, and the weapon's mount sound
        // beside our timer (the first frame after the mount's on which the
        // frames' time since it has reached the approach's length).
        let mut elapsed = 0.0f32;
        let mut ours = None;
        for f in frames.iter().skip(start + 1) {
            elapsed += f.t - frames[start].t - elapsed;
            if elapsed >= approach - 1e-6 {
                ours = Some(f.t - at.t);
                break;
            }
        }
        let heard: Vec<String> = frames
            .iter()
            .enumerate()
            .skip(start)
            .take(end + 2 - start)
            .flat_map(|(_, f)| f.sounds.iter().filter(|c| c.contains("gear_medium") || c.contains("mount")).map(move |c| format!("{c} at {:+.4} s", f.t - at.t)))
            .collect();
        println!("   sounds: {}; our mount sound at {:+.4} s", heard.join(", "), ours.unwrap_or(f32::NAN));
        i = end;
    }
    Ok(())
}
