//! A recording of zeroing set beside the simulation's (optics spec,
//! "Zeroing", "Compared with"): recorder 0.12.0's course 6 (`01-zeroing`),
//! which records every frame the current sight, its zeroing index, the turn
//! of the current mode's optic camera object and of the first collimator in
//! their parents, the weapon motion's local point, the shot's squeeze switch
//! and the lower right's panels.
//!
//!   zeroing <trace.jsonl> [--every]
//!
//! The weapon is built as the recording's equipment has it; its sights are
//! the app's own (`trp_sim::sights::Sights`, with the default round's points),
//! driven frame by frame: the sights up and down as the recording has them,
//! and the script's own presses (`hosts\spt-oracle\programs\10-zeroing`,
//! `zero_up`, `zero_down`, `sight_next`, `magnification`) at the script's
//! clock as the recording gives it (`script_t`); each frame then calibrated
//! at the install's squeeze. Compared on every
//! frame: the zeroing distance, the optic camera object's and the first
//! collimator's turn (the angle between the game's and ours), the local
//! point, the squeeze switch; and the panels: each show of the recorded
//! ammunition's and optic's panels (its words changed or its fade rising
//! again) beside the messages the sights gave.

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::ballistics::Round;
use trp_sim::sights::{SightHud, Sights};

fn quat(v: &Json) -> Option<[f32; 4]> {
    let a = v.as_array()?;
    (a.len() == 4).then(|| std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32))
}

fn vec3(v: &Json) -> Option<[f32; 3]> {
    let a = v.as_array()?;
    (a.len() == 3).then(|| std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32))
}

/// The angle between two turns, radians: of the one turn into the other
/// (`a⁻¹ b`), from its vector part, which keeps its figures where the two
/// are near (a dot product of 32-bit quaternions does not).
fn between(a: [f32; 4], b: [f32; 4]) -> f64 {
    let (a, b) = (a.map(f64::from), b.map(f64::from));
    let (ax, ay, az, aw) = (-a[0], -a[1], -a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    let r = [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz];
    2.0 * (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt().atan2(r[3].abs())
}

#[derive(Default)]
struct Gap {
    worst: f64,
    at: u64,
    sum: f64,
    count: usize,
}

impl Gap {
    fn note(&mut self, frame: u64, gap: f64) {
        self.count += 1;
        self.sum += gap * gap;
        if gap > self.worst {
            (self.worst, self.at) = (gap, frame);
        }
    }
}

/// The recorded weapon as a build (`replay`'s `recorded_build`).
fn recorded_build(header: &Json, weapon: &str) -> Option<trp_data::database::Preset> {
    let items = header.get("equipment")?.as_array()?;
    let text = |item: &Json, key: &str| item.get(key).and_then(Json::as_str).map(str::to_owned);
    let root = items.iter().find(|item| text(item, "tpl").as_deref() == Some(weapon) && matches!(text(item, "at").as_deref(), Some("FirstPrimaryWeapon" | "SecondPrimaryWeapon" | "Holster")))?;
    let root_id = text(root, "id")?;
    let mut build = vec![trp_data::database::PresetItem { id: root_id.clone(), template: weapon.to_owned(), parent: None, slot: None }];
    let mut wanted = vec![root_id.clone()];
    while let Some(parent) = wanted.pop() {
        for item in items.iter().filter(|item| text(item, "in").as_deref() == Some(parent.as_str())) {
            let slot = text(item, "at")?;
            if slot == "cartridges" || slot == "patron_in_weapon" {
                continue;
            }
            let id = text(item, "id")?;
            build.push(trp_data::database::PresetItem { id: id.clone(), template: text(item, "tpl")?, parent: Some(parent.clone()), slot: Some(slot) });
            wanted.push(id);
        }
    }
    Some(trp_data::database::Preset { id: "recorded".into(), name: "recorded".into(), root: root_id, items: build, encyclopedia: None })
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).context("a trace")?;
    let every = args.iter().any(|a| a == "--every");
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let mut lines = text.lines();
    let header: Json = serde_json::from_str(lines.next().context("an empty trace")?)?;
    let weapon_id = header["weapon"].as_str().context("no weapon in the header")?.to_owned();
    let setting = header["fov"].as_f64().unwrap_or(74.0) as f32;

    let install = Install::locate()?;
    let db = Database::load(&install)?;
    let mut assets = Assets::new(install)?;
    let preset = recorded_build(&header, &weapon_id).context("the recording's equipment has no such weapon")?;
    let weapon = trp_data::weapon::assemble_preset(&mut assets, &db, &preset)?;
    for skipped in &weapon.skipped {
        println!("  part not placed: {skipped}");
    }
    let mut sights = Sights::of_weapon(&weapon).context("the weapon has no sights")?;
    let props = &db.items[&weapon_id].props;
    let round = props.get("defAmmo").and_then(Json::as_str).and_then(|id| db.items.get(id)).and_then(Round::of);
    let launcher = props.get("AdjustCollimatorsToTrajectory").and_then(Json::as_bool).unwrap_or(false);
    sights.set_ballistics(round.as_ref(), weapon.speed_factor, [0.0, -9.81, 0.0], launcher);
    // The weapon's squeeze at the install's field of view (`Hands::fov_scale`).
    sights.set_scale(1.0 + (0.65 - 1.0) * ((setting - 50.0) / 25.0).clamp(0.0, 1.0));
    println!("{}: {} ways to aim; the parts {}", weapon.name, sights.list().len(), sights.parts().iter().map(|p| db.items.get(&p.template).map_or("?", |t| t.name.as_str())).collect::<Vec<_>>().join(", "));

    // The script's presses, by the script's own time.
    let scenario = header["scenario"].as_str().unwrap_or_default();
    let script: Json = serde_json::from_str(&std::fs::read_to_string(format!("hosts/spt-oracle/programs/10-zeroing/{scenario}.json")).with_context(|| format!("the script {scenario}"))?)?;
    let mut presses: Vec<(f64, String)> = script["press"].as_array().map(|list| list.iter().filter_map(|p| Some((p["at"].as_f64()?, p["key"].as_str()?.to_owned()))).collect()).unwrap_or_default();
    presses.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut next_press = 0;
    let mut aiming = false;
    // Whether the sights have come up yet (before that the game holds its
    // turns and point from before the recording).
    let mut calibrated = false;
    let mut gaps: std::collections::BTreeMap<&str, Gap> = Default::default();
    let mut differs: std::collections::BTreeMap<&str, (usize, u64)> = Default::default();
    let mut ours_told: Vec<(u64, SightHud)> = Vec::new();
    let mut theirs_shown: Vec<(u64, &'static str, String)> = Vec::new();
    let mut last_panels: [(String, f32); 2] = Default::default();
    let mut frames = 0u64;
    let mut same_frame_index;
    for line in lines {
        let Ok(f) = serde_json::from_str::<Json>(line) else { continue };
        frames += 1;
        let frame = f["frame"].as_u64().unwrap_or(frames);
        let s = &f["sight"];
        let rec_aiming = f["aiming"].as_bool().unwrap_or(false) || f["aiming"].as_f64() == Some(1.0);
        // The recorded changes, as keys: the sights up or down, the next
        // sight, the next mode, a zeroing step.
        if rec_aiming != aiming {
            aiming = rec_aiming;
            sights.aim_changed(aiming);
        }
        let script_t = f["script_t"].as_f64().or_else(|| f["t"].as_f64()).unwrap_or(0.0);
        while next_press < presses.len() && presses[next_press].0 <= script_t + 1e-6 {
            match presses[next_press].1.as_str() {
                "zero_up" => _ = sights.step_zeroing(true, aiming),
                "zero_down" => _ = sights.step_zeroing(false, aiming),
                "sight_next" => _ = sights.change_aim_scope(false),
                "magnification" => _ = sights.change_magnification(),
                _ => {}
            }
            next_press += 1;
        }
        calibrated |= aiming;
        let rec_index = s["index"].as_u64().unwrap_or(0) as usize;
        same_frame_index = rec_index == sights.aim_index();
        sights.calibrate(aiming);
        for told in sights.take_hud() {
            ours_told.push((frame, told));
        }
        // Compared.
        let range = s["range"].as_i64().unwrap_or(0) as i32;
        let same = |name: &'static str, same: bool, differs: &mut std::collections::BTreeMap<&str, (usize, u64)>| {
            let entry = differs.entry(name).or_insert((0, 0));
            if !same {
                if entry.0 == 0 {
                    entry.1 = frame;
                }
                entry.0 += 1;
            }
        };
        same("zeroing distance", sights.zeroing_distance() == Some(range), &mut differs);
        same("the sight's index", same_frame_index, &mut differs);
        if let Some(squeeze) = s["shot_squeeze"].as_bool() {
            same("the shot's squeeze switch", sights.shot_squeeze() == squeeze, &mut differs);
        }
        if !calibrated {
            continue;
        }
        if let Some(point) = vec3(&s["zero_point"]) {
            let ours = sights.local_point();
            let gap = (0..3).map(|k| (f64::from(ours[k]) - f64::from(point[k])).powi(2)).sum::<f64>().sqrt();
            gaps.entry("the local point, m").or_default().note(frame, gap);
        }
        let cache = sights.current().cache;
        let turned: Vec<(usize, [f32; 4])> = sights.turns().collect();
        let turn_of = |node: Option<usize>| node.and_then(|node| turned.iter().find(|(n, _)| *n == node).map(|(_, q)| *q).or_else(|| Some(weapon.hierarchy.nodes.get(node)?.rotation)));
        if let (Some(rec), Some(cache)) = (quat(&s["optic_turn"]), cache) {
            let part = &sights.parts()[cache];
            let node = part.modes.get(sights.state(cache).prefab_mode).and_then(|m| m.optic.as_ref()).and_then(|o| o.camera);
            if let Some(ours) = turn_of(node) {
                gaps.entry("the optic camera object's turn, rad").or_default().note(frame, between(rec, ours));
                if every {
                    println!("{frame:5} optic turn game {rec:?} ours {ours:?}");
                }
            }
        }
        if let (Some(rec), Some(cache)) = (quat(&s["collimator_turn"]), cache) {
            let node = sights.parts()[cache].modes.iter().find_map(|m| m.collimator);
            if let Some(ours) = turn_of(node) {
                gaps.entry("the first collimator's turn, rad").or_default().note(frame, between(rec, ours));
                // Where its dot lies (its -y in its parent: the way to the
                // point) apart from its roll about that way.
                let way = |q: [f32; 4]| trp_sim::math::quat_rotate(q, [0.0, -1.0, 0.0]).map(f64::from);
                let (a, b) = (way(rec), way(ours));
                let cross = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
                let pointing = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt().atan2(a[0] * b[0] + a[1] * b[1] + a[2] * b[2]);
                gaps.entry("the first collimator's way, rad").or_default().note(frame, pointing);
                if every {
                    println!("{frame:5} collimator game {rec:?} ours {ours:?}");
                }
            }
        }
        if let (Some(rec), Some(line)) = (s["line_angle"].as_f64(), sights.launcher_line()) {
            gaps.entry("a launcher's line, degrees").or_default().note(frame, (rec - f64::from(line.angle)).abs());
        }
        // The panels' shows as recorded: words changed, or the fade
        // rising again from where it stood.
        for (k, name) in ["ammo", "optic"].into_iter().enumerate() {
            let p = &f["hud"][name];
            let (Some(words), Some(alpha)) = (p.get(0).and_then(Json::as_str), p.get(3).and_then(Json::as_f64)) else { continue };
            let alpha = alpha as f32;
            let (last_words, last_alpha) = &last_panels[k];
            if (words != last_words && alpha > 0.0) || (alpha > *last_alpha + 1e-4 && *last_alpha < 0.05) {
                theirs_shown.push((frame, name, words.to_owned()));
            }
            last_panels[k] = (words.to_owned(), alpha);
        }
    }
    println!("{frames} frames");
    for (name, gap) in &gaps {
        println!("  {name:40} largest {:.6} at frame {}, rms {:.6} ({} frames)", gap.worst, gap.at, (gap.sum / gap.count.max(1) as f64).sqrt(), gap.count);
    }
    for (name, (count, first)) in &differs {
        if *count == 0 {
            println!("  {name:40} the same on every frame");
        } else {
            println!("  {name:40} DIFFERS on {count} frames, first {first}");
        }
    }
    println!("the panels as recorded ({}):", theirs_shown.len());
    for (frame, name, words) in &theirs_shown {
        println!("  {frame:5} {name} {words:?}");
    }
    println!("the sights' messages ({}):", ours_told.len());
    for (frame, told) in &ours_told {
        println!("  {frame:5} {told:?}");
    }
    Ok(())
}
