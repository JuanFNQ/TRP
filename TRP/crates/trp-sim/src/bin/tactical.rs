//! Sets course 7's recordings (recorder 0.13.0 on: tactical devices) beside
//! the simulation, frame by frame (tactical-devices spec, "Compared with").
//!
//!   tactical <folder> [part of a name ...]
//!
//! For each trace of the folder:
//!
//! - **Each press** of the recorder's script (`commands`): what our keys
//!   ask of the devices (`trp_sim::tactical`) from the states recorded the
//!   frame before, refused or not by our rule for the operation recorded,
//!   set beside the states recorded after it.
//! - **The hands**: the weapon's own animator (the install's, our
//!   `Firearm`) stepped on the recording's frame times, given `ModSwitch`
//!   where we take an animated request; for each press made with the hands
//!   at rest, when `MOD SWITCH` is entered, when the hands are fully in it,
//!   when they leave it and when they are back at rest, and when its sound
//!   plays (`SndModSwitch`), ours beside the game's (`hands_anim`,
//!   `hands_next`, `sounds`).
//! - **The head light**: our coroutine (`HeadLight`) stepped on the frame
//!   times, its switch and its lock lifted beside the recording's
//!   (`Player.PlayTacticalSound` in `calls`, `head_light_busy`), the body's
//!   gesture's flag (`p_IsInteraction`), and on every frame a head light is
//!   on its place and way by our rule (`head_pose`) beside the recorded
//!   light's, and its figures beside the device's as read.
//! - **The hip penalty** on every frame it changes: the recorded beside our
//!   rule's on the recorded ergonomics and devices.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_data::hard_settings::HardSettings;
use trp_data::tactical::{read_alone, DeviceClass};
use trp_data::unity::anim::path_hash;
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::tactical::{head_pose, hip_penalty, next_mode, turn, HeadLight};
use trp_sim::weapon_animator::WeaponAnimatorData;
use trp_sim::{FireMode, Firearm, FirearmEvent};

/// The hands' states by the recorder's number (`shortNameHash`'s low 24
/// bits).
fn number(name: &str) -> u32 {
    path_hash(name.rsplit('.').next().unwrap_or(name)) & 0x00ff_ffff
}

/// A device as a frame has it: where, its item, template, on, mode, modes.
#[derive(Clone, Debug, PartialEq)]
struct Dev {
    place: String,
    item: String,
    template: String,
    on: bool,
    mode: u32,
    modes: u32,
}

fn devices(frame: &Json) -> Vec<Dev> {
    let list = frame.get("tactical").and_then(Json::as_array).cloned().unwrap_or_default();
    list.iter()
        .filter_map(|d| {
            let d = d.as_array()?;
            Some(Dev {
                place: d.first()?.as_str()?.to_owned(),
                item: d.get(1)?.as_str()?.to_owned(),
                template: d.get(2)?.as_str()?.to_owned(),
                on: d.get(3)?.as_bool()?,
                mode: d.get(4)?.as_u64()? as u32,
                modes: d.get(5)?.as_i64()?.max(0) as u32,
            })
        })
        .collect()
}

fn word<'a>(frame: &'a Json, name: &str) -> &'a str {
    frame.get(name).and_then(Json::as_str).unwrap_or("")
}

fn number_of(frame: &Json, name: &str) -> Option<f32> {
    frame.get(name).and_then(Json::as_f64).map(|v| v as f32)
}

fn floats(frame: &Json, name: &str) -> Vec<f32> {
    frame.get(name).and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).map(|v| v as f32).collect()).unwrap_or_default()
}

/// The script's presses this frame, by the game's command's name.
fn presses(frame: &Json) -> Vec<String> {
    frame.get("commands").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_str).map(|c| c.split(':').next().unwrap_or(c).to_owned()).collect()).unwrap_or_default()
}

/// Our rule (`Firearm::can_change_lights`) by the operation's name as the
/// recorder writes it: refused by the draw, the put-away, a part fitted or
/// taken off, a magazine through the inventory, the backpack dropped.
fn refused_by(operation: &str) -> bool {
    operation.contains("Spawn") || operation == "Remove" || operation.contains("RemoveOperation") || operation.contains("InsertMag") || operation.contains("PullOutMag") || operation.contains("DropBackpack") || operation.contains("ModOperation")
}

/// When the hands go through `MOD SWITCH` after a press: entering it, fully
/// in, leaving, back at rest, each a frame number.
#[derive(Default, Debug, Clone, Copy)]
struct Pass {
    enter: Option<usize>,
    inside: Option<usize>,
    leave: Option<usize>,
    back: Option<usize>,
    sound: Option<usize>,
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let folder = PathBuf::from(args.next().context("tactical <folder> [part of a name ...]")?);
    let wanted: Vec<String> = args.collect();
    let install = Install::locate()?;
    let db = Database::load(&install)?;
    let hard = HardSettings::load(&install)?;
    let mut assets = Assets::new(install)?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(&folder)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl") && !p.to_string_lossy().ends_with(".world.jsonl"))
        .filter(|p| wanted.is_empty() || wanted.iter().any(|w| p.to_string_lossy().contains(w.as_str())))
        .collect();
    files.sort();
    let mut weapons: Vec<(String, WeaponAnimatorData, trp_sim::FirearmSettings)> = Vec::new();
    for file in files {
        trace(&file, &db, &hard, &mut assets, &mut weapons)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn trace(file: &Path, db: &Database, hard: &HardSettings, assets: &mut Assets, weapons: &mut Vec<(String, WeaponAnimatorData, trp_sim::FirearmSettings)>) -> Result<()> {
    let text = std::fs::read_to_string(file)?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Json = serde_json::from_str(lines.next().context("empty trace")?)?;
    let frames: Vec<Json> = lines.map(serde_json::from_str).collect::<Result<_, _>>()?;
    let weapon_id = word(&header, "weapon").to_owned();
    let press_mode = word(&header, "tactical_input") != "Hold";
    println!("== {} ({} frames, {}, {} mode)", file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), frames.len(), db.items.get(&weapon_id).map_or("?", |t| t.name.as_str()), if press_mode { "Press" } else { "Hold" });
    if frames.len() < 2 {
        return Ok(());
    }
    if !weapons.iter().any(|(id, ..)| *id == weapon_id) {
        let weapon = trp_data::weapon::assemble(assets, db, &weapon_id)?;
        let data = WeaponAnimatorData::load(assets, &weapon)?;
        let settings = trp_sim::firearm_settings(assets, db, hard, &weapon)?;
        weapons.push((weapon_id.clone(), data, settings));
    }
    let (_, data, settings) = weapons.iter().find(|(id, ..)| *id == weapon_id).expect("just made");
    let names: Vec<(u32, String)> = data.state_names().iter().map(|n| (number(n), n.rsplit('.').next().unwrap_or(n).to_owned())).collect();
    let name_of = |n: u32| names.iter().find(|(k, _)| *k == n).map_or_else(|| n.to_string(), |(_, s)| s.clone());
    let idle = number("IDLE");
    let mod_switch = number("MOD SWITCH");

    // Each press: ours beside the recorded.
    // Driven as the recording has it: the rounds, the way of firing, the
    // trigger and a reload begun (what the press asks of the hands is ours).
    let mode_of = |w: &str| match w {
        "single" => Some(FireMode::Single),
        "fullauto" => Some(FireMode::FullAuto),
        _ => None,
    };
    let mut firearm = Firearm::new(settings.clone(), data);
    firearm.magazine = frames[0].get("magazine").and_then(Json::as_u64).map(|n| n as u32);
    firearm.chamber = frames[0].get("chamber").and_then(Json::as_u64).unwrap_or(0) != 0;
    if let Some(mode) = mode_of(word(&frames[0], "fire_mode")) {
        firearm.start_in_mode(mode);
    }
    firearm.start_drawn();
    let (mut trigger_was, mut operation_was, mut fire_mode_was) = (false, word(&frames[0], "operation").to_owned(), word(&frames[0], "fire_mode").to_owned());
    let mut ours_states: Vec<(String, Option<String>)> = Vec::with_capacity(frames.len());
    let mut our_sounds: Vec<usize> = Vec::new();
    let mut head = HeadLight::default();
    let mut head_steps: Vec<(usize, &'static str)> = Vec::new();
    let mut presses_at: Vec<(usize, String, bool)> = Vec::new();
    let (mut matched, mut differed) = (0, 0);
    for (i, frame) in frames.iter().enumerate() {
        let dt = number_of(frame, "dt").unwrap_or(1.0 / 120.0);
        if i > 0 {
            let step = head.step(dt);
            if step.switched {
                head_steps.push((i, "switch"));
            }
            if step.unlocked {
                head_steps.push((i, "unlock"));
            }
        }
        let trigger = frame.get("trigger").and_then(Json::as_bool).unwrap_or(trigger_was);
        if trigger != trigger_was {
            firearm.set_trigger(trigger, true);
            trigger_was = trigger;
        }
        let fire_mode = word(frame, "fire_mode");
        if !fire_mode.is_empty() && fire_mode != fire_mode_was {
            if let Some(mode) = mode_of(fire_mode) {
                firearm.set_fire_mode(mode);
            }
            fire_mode_was = fire_mode.to_owned();
        }
        let operation = word(frame, "operation");
        if !operation.is_empty() && operation != operation_was {
            if operation.contains("Reload") && !operation_was.contains("Reload") {
                firearm.reload();
            }
            operation_was = operation.to_owned();
        }
        let before = if i > 0 { devices(&frames[i - 1]) } else { devices(frame) };
        for press in presses(frame) {
            let operation = word(&frames[i.saturating_sub(1)], "operation").to_owned();
            let (hands_key, head_key) = match press.as_str() {
                "ToggleTacticalDevice" => (Some("toggle"), None),
                "NextTacticalDevice" => (Some("next"), None),
                "ToggleHeadLight" => (None, Some((true, false))),
                "SwitchHeadLight" => (None, Some((false, true))),
                _ => (None, None),
            };
            let mut predicted = before.clone();
            let mut accepted = false;
            if let Some(key) = hands_key {
                let theirs: Vec<&mut Dev> = predicted.iter_mut().filter(|d| d.place == "hands").collect();
                accepted = !theirs.is_empty() && !refused_by(&operation);
                if accepted {
                    for d in theirs {
                        match key {
                            "toggle" if press_mode => d.on = !d.on,
                            "toggle" => d.on = true,
                            _ => d.mode = next_mode(d.mode, d.modes),
                        }
                    }
                    if press_mode || key == "next" {
                        firearm.mod_switch();
                    }
                }
            }
            if let Some((toggle, switch)) = head_key {
                let locked = frames.get(i.saturating_sub(1)).and_then(|f| f.get("head_light_busy")).and_then(Json::as_bool).unwrap_or(false);
                let any = predicted.iter().any(|d| d.place == "head");
                accepted = any && !locked && head.press();
                if accepted {
                    for d in predicted.iter_mut().filter(|d| d.place == "head") {
                        if toggle {
                            d.on = !d.on;
                        }
                        if switch {
                            d.mode = next_mode(d.mode, d.modes);
                        }
                    }
                }
            }
            if hands_key.is_none() && head_key.is_none() {
                continue;
            }
            let after = devices(frame);
            let same = predicted == after;
            if same {
                matched += 1;
            } else {
                differed += 1;
            }
            let show = |list: &[Dev]| list.iter().map(|d| format!("{}{}:{}", &d.place[..1], if d.on { "on" } else { "off" }, d.mode)).collect::<Vec<_>>().join(" ");
            println!(
                "  {:7.3} {press:<22} op {operation:<28} ours {} -> {}  recorded {} -> {}  {}",
                number_of(frame, "t").unwrap_or(0.0),
                if accepted { "taken  " } else { "refused" },
                show(&predicted),
                show(&before),
                show(&after),
                if same { "same" } else { "DIFFERS" }
            );
            presses_at.push((i, press.clone(), accepted));
        }
        for event in firearm.step(dt) {
            if matches!(&event, FirearmEvent::Sound(name) if name == "ModSwitch") {
                our_sounds.push(i);
            }
        }
        let (current, _, next) = firearm.animator().state();
        let short = |s: &str| s.rsplit('.').next().unwrap_or(s).to_owned();
        ours_states.push((short(current), next.map(short)));
    }
    println!("  presses: {matched} as recorded, {differed} not");

    // The hands through `MOD SWITCH`, each press made at rest.
    let recorded_state = |i: usize| -> (u32, Option<u32>) {
        let f = &frames[i];
        (f.get("hands_anim").and_then(Json::as_u64).unwrap_or(0) as u32, f.get("hands_next").and_then(Json::as_u64).map(|n| n as u32))
    };
    // `hands_anim` and `hands_next` are written only as they change: carry them.
    let mut carried: Vec<(u32, Option<u32>)> = Vec::with_capacity(frames.len());
    for i in 0..frames.len() {
        let (anim, next) = recorded_state(i);
        let has_anim = frames[i].get("hands_anim").is_some();
        let previous = carried.last().copied().unwrap_or((0, None));
        let anim = if has_anim { anim } else { previous.0 };
        // `hands_next` is absent when there is none, once it changed to none.
        let next = if frames[i].get("hands_next").is_some() { next } else if has_anim || i == 0 { None } else { previous.1 };
        carried.push((anim, next));
    }
    let sound_frames: Vec<usize> = frames
        .iter()
        .enumerate()
        .filter(|(_, f)| f.get("sounds").and_then(Json::as_array).is_some_and(|a| a.iter().any(|s| word(s, "clip").starts_with("weapon_light_switcher"))))
        .map(|(i, _)| i)
        .collect();
    let mut errors: Vec<f32> = Vec::new();
    for &(at, ref press, accepted) in &presses_at {
        if !(press == "ToggleTacticalDevice" || press == "NextTacticalDevice") || !accepted {
            continue;
        }
        let at_rest = carried.get(at.saturating_sub(1)).is_some_and(|&(anim, next)| anim == idle && next.is_none());
        let theirs = {
            let mut p = Pass::default();
            for i in at..frames.len().min(at + 240) {
                let (anim, next) = carried[i];
                if p.enter.is_none() && (next == Some(mod_switch) || anim == mod_switch) {
                    p.enter = Some(i);
                }
                if p.inside.is_none() && anim == mod_switch && next.is_none() {
                    p.inside = Some(i);
                }
                if p.inside.is_some() && p.leave.is_none() && anim == mod_switch && next.is_some_and(|n| n != mod_switch) {
                    p.leave = Some(i);
                }
                if p.inside.is_some() && p.back.is_none() && anim != mod_switch && next.is_none() {
                    p.back = Some(i);
                    break;
                }
            }
            // Its sound, before the next press's.
            let next_press = presses_at.iter().map(|(i, ..)| *i).find(|&i| i > at).unwrap_or(usize::MAX);
            p.sound = sound_frames.iter().copied().find(|&s| s >= at && s < next_press);
            p
        };
        let ours = {
            let mut p = Pass::default();
            for i in at..frames.len().min(at + 240) {
                let (cur, next) = &ours_states[i];
                if p.enter.is_none() && (next.as_deref() == Some("MOD SWITCH") || cur == "MOD SWITCH") {
                    p.enter = Some(i);
                }
                if p.inside.is_none() && cur == "MOD SWITCH" && next.is_none() {
                    p.inside = Some(i);
                }
                if p.inside.is_some() && p.leave.is_none() && cur == "MOD SWITCH" && next.as_deref().is_some_and(|n| n != "MOD SWITCH") {
                    p.leave = Some(i);
                }
                if p.inside.is_some() && p.back.is_none() && cur != "MOD SWITCH" && next.is_none() {
                    p.back = Some(i);
                    break;
                }
            }
            let next_press = presses_at.iter().map(|(i, ..)| *i).find(|&i| i > at).unwrap_or(usize::MAX);
            p.sound = our_sounds.iter().copied().find(|&s| s >= at && s < next_press);
            p
        };
        let t0 = number_of(&frames[at], "t").unwrap_or(0.0);
        let rel = |f: Option<usize>| f.and_then(|i| number_of(&frames[i], "t")).map_or("    -  ".to_owned(), |t| format!("{:7.3}", t - t0));
        println!(
            "  {t0:7.3} {press:<22} {}  enters/in/leaves/back/sound  game {} {} {} {} {}  ours {} {} {} {} {}",
            if at_rest { "at rest " } else { "not at rest" },
            rel(theirs.enter),
            rel(theirs.inside),
            rel(theirs.leave),
            rel(theirs.back),
            rel(theirs.sound),
            rel(ours.enter),
            rel(ours.inside),
            rel(ours.leave),
            rel(ours.back),
            rel(ours.sound)
        );
        if at_rest {
            for (a, b) in [(theirs.enter, ours.enter), (theirs.inside, ours.inside), (theirs.leave, ours.leave), (theirs.back, ours.back), (theirs.sound, ours.sound)] {
                if let (Some(a), Some(b)) = (a, b) {
                    errors.push((number_of(&frames[a], "t").unwrap_or(0.0) - number_of(&frames[b], "t").unwrap_or(0.0)).abs());
                }
            }
        }
    }
    if !errors.is_empty() {
        let worst = errors.iter().copied().fold(0.0, f32::max);
        let frames_off = errors.iter().filter(|e| **e > 1e-4).count();
        println!("  the hands at rest: {} moments, {frames_off} off by a frame or more, the worst {:.4} s", errors.len(), worst);
    }
    let _ = name_of;

    // The head light: our coroutine beside the recorded call and lock.
    let calls_at = |what: &str| -> Vec<usize> {
        frames.iter().enumerate().filter(|(_, f)| f.get("calls").and_then(Json::as_array).is_some_and(|a| a.iter().any(|c| c.get(0).and_then(Json::as_str) == Some(what)))).map(|(i, _)| i).collect()
    };
    let switches = calls_at("Player.PlayTacticalSound");
    let unlocks: Vec<usize> = (1..frames.len())
        .filter(|&i| frames[i].get("head_light_busy").and_then(Json::as_bool) == Some(false) && frames[i - 1].get("head_light_busy").and_then(Json::as_bool) == Some(true))
        .collect();
    let gestures: Vec<(usize, usize)> = {
        let mut out = Vec::new();
        let mut on: Option<usize> = None;
        let mut flag = false;
        for (i, f) in frames.iter().enumerate() {
            if let Some(now) = f.get("p_IsInteraction").and_then(Json::as_bool) {
                flag = now;
            }
            match (on, flag) {
                (None, true) => on = Some(i),
                (Some(start), false) => {
                    out.push((start, i));
                    on = None;
                }
                _ => {}
            }
        }
        out
    };
    let ours_switch: Vec<usize> = head_steps.iter().filter(|(_, w)| *w == "switch").map(|(i, _)| *i).collect();
    let ours_unlock: Vec<usize> = head_steps.iter().filter(|(_, w)| *w == "unlock").map(|(i, _)| *i).collect();
    if !switches.is_empty() || !ours_switch.is_empty() {
        let t = |i: usize| number_of(&frames[i], "t").unwrap_or(0.0);
        let head_presses: Vec<usize> = presses_at.iter().filter(|(_, p, a)| (p == "ToggleHeadLight" || p == "SwitchHeadLight") && *a).map(|(i, ..)| *i).collect();
        for (k, &press) in head_presses.iter().enumerate() {
            let fmt = |list: &[usize]| list.get(k).map_or("    -  ".to_owned(), |&i| format!("{:7.4}", t(i) - t(press)));
            let gesture = gestures.iter().find(|(s, _)| *s >= press).map_or("-".to_owned(), |(s, e)| format!("{:.4} s from {:.4}", t(*e) - t(*s), t(*s) - t(press)));
            println!(
                "  head light at {:7.3}: switch game {} ours {} (frames {:?} {:?}); lock lifted game {} ours {} (frames {:?} {:?}); the gesture's flag {gesture}",
                t(press),
                fmt(&switches),
                fmt(&ours_switch),
                switches.get(k),
                ours_switch.get(k),
                fmt(&unlocks),
                fmt(&ours_unlock),
                unlocks.get(k),
                ours_unlock.get(k)
            );
        }
    }

    // The head lights on: our place and way beside the recorded, and the
    // figures beside the device's.
    let head_templates: Vec<String> = frames.iter().flat_map(devices).filter(|d| d.place == "head").map(|d| d.template).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    for template in head_templates {
        let device = match read_alone(assets, db, &template) {
            Ok(d) => d,
            Err(e) => {
                println!("  the head device {template} could not be read: {e:#}");
                continue;
            }
        };
        // Each light keeps its own turn as last placed: the game moves only
        // those on, each along its own turn of the frame it was last moved
        // (one lit for the first time along its model's, not known here).
        let mut turned_of: std::collections::HashMap<String, [f32; 4]> = std::collections::HashMap::new();
        let mut first_lit = 0;
        let (mut place_err, mut way_err, mut lit_frames) = (Vec::new(), Vec::new(), 0);
        let mut figures_seen: Vec<String> = Vec::new();
        let mut side_right: Option<bool> = None;
        for (i, f) in frames.iter().enumerate() {
            let eye = floats(f, "eye");
            let look = floats(f, "look");
            let rot = floats(f, "camera_world_rot");
            let fov = number_of(f, "fov").unwrap_or(75.0);
            if eye.len() < 3 || look.is_empty() || rot.len() < 4 {
                continue;
            }
            // The eye is kept in the player's axes: turned by its yaw.
            let yaw = look[0].to_radians();
            let camera = [eye[0] * yaw.cos() + eye[2] * yaw.sin(), eye[1], -eye[0] * yaw.sin() + eye[2] * yaw.cos()];
            let camera_turn = [rot[0], rot[1], rot[2], rot[3]];
            let lights = f.get("lights").and_then(Json::as_array).cloned().unwrap_or_default();
            let on: Vec<&Json> = lights.iter().filter(|l| l.get(1).and_then(Json::as_bool) == Some(true) && l.get(0).and_then(Json::as_str).is_some_and(|p| p.starts_with("light_"))).collect();
            // Its side, by where the recorded light sits as first seen on.
            if side_right.is_none() {
                if let Some(l) = on.first() {
                    let p: Vec<f32> = l.get(7).and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).map(|v| v as f32).collect()).unwrap_or_default();
                    let right = turn(camera_turn, [1.0, 0.0, 0.0]);
                    side_right = Some((0..3).map(|k| (p[k] - camera[k]) * right[k]).sum::<f32>() > 0.0);
                }
            }
            for l in on {
                let p: Vec<f32> = l.get(7).and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).map(|v| v as f32).collect()).unwrap_or_default();
                let w: Vec<f32> = l.get(8).and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_f64).map(|v| v as f32).collect()).unwrap_or_default();
                if p.len() < 3 || w.len() < 3 {
                    continue;
                }
                let name = l.get(0).and_then(Json::as_str).unwrap_or("").to_owned();
                let before = turned_of.get(&name).copied();
                let (place, now) = head_pose(camera, camera_turn, fov, side_right.unwrap_or(false), before.unwrap_or([0.0, 0.0, 0.0, 1.0]));
                turned_of.insert(name, now);
                let way = turn(now, [0.0, 0.0, 1.0]);
                if before.is_none() {
                    first_lit += 1;
                    continue;
                }
                lit_frames += 1;
                place_err.push(((0..3).map(|k| (p[k] - place[k]).powi(2)).sum::<f32>()).sqrt());
                way_err.push(((0..3).map(|k| w[k] * way[k]).sum::<f32>()).clamp(-1.0, 1.0).acos().to_degrees());
                let figures = format!(
                    "intensity {} range {} angle {} shadows {} cookie {}",
                    l.get(4).and_then(Json::as_f64).unwrap_or(0.0),
                    l.get(5).and_then(Json::as_f64).unwrap_or(0.0),
                    l.get(6).and_then(Json::as_f64).unwrap_or(0.0),
                    l.get(9).and_then(Json::as_str).unwrap_or(""),
                    l.get(10).and_then(Json::as_str).unwrap_or("")
                );
                let figures = format!("{} {figures}", l.get(0).and_then(Json::as_str).unwrap_or(""));
                let mode = devices(f).iter().find(|d| d.place == "head").map_or(0, |d| d.mode);
                let entry = format!("mode {mode}: {figures}");
                if !figures_seen.contains(&entry) {
                    figures_seen.push(entry);
                }
                let _ = i;
            }
        }
        let name = db.items.get(&template).map_or("?", |t| t.name.as_str());
        println!("  head device {template} {name}: on the {} side", if side_right == Some(true) { "right" } else { "left" });
        for (k, l) in device.lights.iter().enumerate() {
            println!("    ours: light {k} mode {:?}: intensity {} (its own {}) range {} angle {} shadows {} cookie {:?}", l.mode, l.drawn_intensity, l.intensity, l.range, l.spot_angle, l.shadows, l.cookie.as_ref().map(|c| c.name.clone()));
        }
        for seen in &figures_seen {
            println!("    game: {seen}");
        }
        if lit_frames > 0 {
            let worst = |v: &[f32]| v.iter().copied().fold(0.0, f32::max);
            let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
            println!(
                "    {lit_frames} lit frames: place off by {:.4} m at worst, {:.4} m rms; way off by {:.3} degrees at worst, {:.3} rms ({first_lit} frames of a light lit for the first time left out: it moves along its model's turn)",
                worst(&place_err),
                rms(&place_err),
                worst(&way_err),
                rms(&way_err)
            );
        }
    }

    // The hip penalty as it changes.
    let mut last: Option<(f32, f32)> = None;
    let (mut hip_n, mut hip_worst) = (0, 0.0f32);
    for f in &frames {
        let hip = floats(f, "hip_penalty");
        let (Some(&penalty), Some(&ergonomics)) = (hip.first(), hip.get(1)) else { continue };
        if last.is_some_and(|(p, e)| p == penalty && e == ergonomics) {
            continue;
        }
        last = Some((penalty, ergonomics));
        let combo = devices(f).iter().any(|d| d.place == "hands" && d.on && DeviceClass::of(db, &d.template) == Some(DeviceClass::TacticalCombo));
        let ours = hip_penalty(ergonomics, combo);
        let off = (ours - penalty).abs();
        hip_n += 1;
        hip_worst = hip_worst.max(off);
        println!("  {:7.3} hip penalty {penalty:.6} (ergonomics {ergonomics:.2}, a combo unit {}): ours {ours:.6}{}", number_of(f, "t").unwrap_or(0.0), if combo { "on" } else { "off" }, if off > 1e-5 { "  DIFFERS (the game works it out only at a draw or a request)" } else { "" });
    }
    if hip_n > 0 {
        println!("  hip penalty: {hip_n} changes, ours off by {hip_worst:.6} at worst");
    }
    Ok(())
}
