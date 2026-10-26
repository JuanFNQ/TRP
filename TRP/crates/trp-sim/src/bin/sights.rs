//! A recording's sights set beside the simulation's (optics spec, "Compared
//! with"): `courses2/4-sights-optic`, an AKM aimed over its iron sights,
//! then with a Burris FullField TAC30 1-4x put on it, aimed through, its
//! magnification key and its zoom's wheel worked by hand.
//!
//!   sights <trace.jsonl> [--every] [--from frame] [--to frame]
//!
//! The weapon's `Hands` and their `Sights` are the app's own, driven frame
//! by frame with the recording's frame times: the sights up and down on the
//! frames the recording has them so; the scope put on where the recording's
//! sight first counts as an optic. The recording holds neither the keys nor
//! the wheel, so each frame's are read off what the zoom did: the
//! magnification key's press where it snapped from end to end, and the
//! wheel's notches (a whole number a frame, -3 to 3) that bring the
//! simulated zoom to the recorded one; the frames that needed any are
//! counted. The mount the recording's scope stood on is not in it: the
//! build here has the Burris on its own 30 mm mount on a railed handguard,
//! which changes nothing compared (the sight's figures, the two fields of
//! view, the zoom). The zeroing (optics spec, "Zeroing"): each frame's
//! zeroing key read off the recorded distance (`range`) the same way, a
//! press up or down that brings the simulated one to it, and the distance
//! compared every frame.

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_data::database::PresetItem;
use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::{Hands, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";
const BURRIS: &str = "5b2388675acfc4771e1be0be";
const RING_MOUNT: &str = "5b2389515acfc4771e1be0c0";
const RAILED_HANDGUARD: &str = "6389f1dfc879ce63f72fc43e";

/// What one recorded frame says of the sights and the camera.
struct Frame {
    frame: u64,
    dt: f32,
    aiming: bool,
    fov: f32,
    index: usize,
    count: usize,
    optic: bool,
    bone: String,
    scope: usize,
    mode: usize,
    zoom: f32,
    zoom_value: f32,
    sensitivity: f32,
    optic_on: bool,
    optic_fov: f32,
    range: i32,
}

fn read(path: &str) -> Result<Vec<Frame>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let mut frames = Vec::new();
    for line in text.lines().skip(1) {
        let Ok(f) = serde_json::from_str::<Json>(line) else { continue };
        let s = &f["sight"];
        let num = |v: &Json| v.as_f64().unwrap_or(0.0) as f32;
        frames.push(Frame {
            frame: f["frame"].as_u64().unwrap_or(0),
            dt: num(&f["dt"]),
            aiming: f["aiming"].as_bool().unwrap_or(false),
            fov: num(&f["fov"]),
            index: s["index"].as_u64().unwrap_or(0) as usize,
            count: s["count"].as_u64().unwrap_or(0) as usize,
            optic: s["optic"].as_bool().unwrap_or(false),
            bone: s["bone"].as_str().unwrap_or("").to_owned(),
            scope: s["scope"].as_u64().unwrap_or(0) as usize,
            mode: s["mode"].as_u64().unwrap_or(0) as usize,
            zoom: num(&s["zoom"]),
            zoom_value: num(&s["zoom_value"]),
            sensitivity: num(&s["sensitivity"]),
            optic_on: s["optic_on"].as_bool().unwrap_or(false),
            optic_fov: num(&s["optic_fov"]),
            range: s["range"].as_i64().unwrap_or(0) as i32,
        });
    }
    Ok(frames)
}

/// The largest gap of a figure, and where.
#[derive(Default)]
struct Gap {
    worst: f32,
    at: u64,
    compared: usize,
}

impl Gap {
    fn note(&mut self, frame: u64, ours: f32, theirs: f32) {
        let gap = (ours - theirs).abs();
        self.compared += 1;
        if gap > self.worst {
            (self.worst, self.at) = (gap, frame);
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).context("a trace")?;
    let every = args.iter().any(|a| a == "--every");
    let bound = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok());
    let (from, to) = (bound("--from").unwrap_or(0), bound("--to").unwrap_or(u64::MAX));
    let frames = read(path)?;
    let header: Json = serde_json::from_str(std::fs::read_to_string(path)?.lines().next().context("no header")?)?;
    let setting = header["fov"].as_f64().unwrap_or(74.0) as f32;

    let install = Install::locate()?;
    let db = Database::load(&install)?;
    let hard = HardSettings::load(&install)?;
    let engine = EngineSettings::load(&install)?;
    let mut assets = Assets::new(install)?;
    let data = HandsData::load(&mut assets)?;
    let view = ViewSettings { field_of_view: setting, ..ViewSettings::default() };

    // The AKM as it is drawn, and with the scope on.
    let stock = db.default_preset(AKM)?.clone();
    let mut scoped = stock.clone();
    let gas_block = scoped.items.iter().find(|i| i.slot.as_deref() == Some("mod_gas_block")).context("the AKM's gas block")?.id.clone();
    let handguard = scoped.items.iter_mut().find(|i| i.parent.as_deref() == Some(gas_block.as_str()) && i.slot.as_deref() == Some("mod_handguard")).context("the AKM's handguard")?;
    handguard.template = RAILED_HANDGUARD.to_owned();
    let handguard = handguard.id.clone();
    scoped.items.push(PresetItem { id: "ring".into(), template: RING_MOUNT.into(), parent: Some(handguard), slot: Some("mod_scope".into()) });
    scoped.items.push(PresetItem { id: "scope".into(), template: BURRIS.into(), parent: Some("ring".into()), slot: Some("mod_scope_000".into()) });
    let mut hands_of = |preset: &trp_data::database::Preset| -> Result<(Hands, Vec<String>)> {
        let weapon = trp_data::weapon::assemble_preset(&mut assets, &db, preset)?;
        for skipped in &weapon.skipped {
            println!("  part not placed: {skipped}");
        }
        let names = weapon.hierarchy.nodes.iter().map(|n| n.name.clone()).collect();
        Ok((Hands::new(trp_sim::hands_settings(data.clone(), &db, &hard, &engine, &weapon, view)?), names))
    };
    let (mut hands, mut names) = hands_of(&stock)?;
    let scoped_hands = hands_of(&scoped)?;
    let describe = |hands: &Hands, name: &str| {
        let sights = hands.sights().expect("sights");
        println!("{name}: {} way(s) to aim", sights.list().len());
        for (i, sight) in sights.list().iter().enumerate() {
            let part = sight.part.map(|p| db.items.get(&sights.parts()[p].template).map_or("?", |t| t.name.as_str())).unwrap_or("(the weapon's own)");
            println!("  {i}: {part}, optic {}, turned {:.3} degrees", sights.is_optic(sight), sight.rotation);
        }
        let eye = sights.eye();
        println!("  the eye's plane {:.4} m, its distance {:.4} m", eye.depth, eye.distance);
    };
    describe(&hands, "the stock AKM");
    describe(&scoped_hands.0, "the AKM with the Burris");
    let mut scoped_hands = Some(scoped_hands);

    let mut gaps: std::collections::BTreeMap<&str, Gap> = std::collections::BTreeMap::new();
    let mut mismatched: std::collections::BTreeMap<&str, (usize, u64)> = std::collections::BTreeMap::new();
    let (mut presses, mut notched_frames, mut notches, mut unfit) = (0, 0, 0i32, Vec::new());
    let (mut zeroing_presses, mut zeroing_unfit) = (0, Vec::new());
    // The frames on which the recorded zoom moved with no key or notch read
    // off them: its motion as the simulation has it on its own.
    let (mut moved, mut moved_alone, mut last_zoom) = (0, 0, None::<f32>);
    let mut aiming = false;
    for f in &frames {
        // The scope is put on where the recording's sight first counts as
        // an optic: a new `Hands` at the field of view, the weapon not
        // aimed then.
        if f.optic && hands.sights().is_some_and(|s| !s.optic()) {
            if let Some(next) = scoped_hands.take() {
                println!("frame {}: the scope is on", f.frame);
                (hands, names) = next;
                aiming = false;
            }
        }
        // The sights up or down, and this frame's easing; else the keys,
        // then the frame's easing.
        let stepped_already = f.aiming != aiming;
        if stepped_already {
            aiming = f.aiming;
            hands.aim_changed_after_frame(aiming, f.dt);
        }
        // The zoom's keys and wheel, read off what the zoom did: the first
        // of no input, the wheel's notches or the magnification key that
        // brings the simulated zoom to the recorded one.
        if let Some(sights) = hands.sights() {
            let fits = |press: bool, notch: f32| {
                let mut trial = sights.clone();
                if press {
                    trial.change_magnification();
                }
                trial.zoom_frame(notch, f.dt, aiming);
                (trial.zoom_value() - f.zoom_value).abs() < 0.001
            };
            let candidates = [(false, 0.0), (false, 1.0), (false, -1.0), (false, 2.0), (false, -2.0), (false, 3.0), (false, -3.0), (true, 0.0)];
            match candidates.iter().find(|&&(press, notch)| fits(press, notch)) {
                Some(&(press, notch)) => {
                    if last_zoom.is_some_and(|last| last != f.zoom_value) {
                        moved += 1;
                        if !press && notch == 0.0 {
                            moved_alone += 1;
                        }
                    }
                    if press {
                        presses += 1;
                        hands.change_magnification();
                    }
                    if notch != 0.0 {
                        notched_frames += 1;
                        notches += notch.abs() as i32;
                    }
                    hands.zoom_frame(notch, f.dt);
                }
                None => {
                    unfit.push(f.frame);
                    hands.zoom_frame(0.0, f.dt);
                }
            }
        }
        last_zoom = Some(f.zoom_value);
        // The zeroing keys, read off the recorded distance: the press, up
        // or down, that brings the simulated one to it (refused unless
        // aiming, as the game refuses it).
        if hands.sights().is_some_and(|s| s.zeroing_distance() != Some(f.range)) {
            let up = hands.sights().and_then(|s| s.zeroing_distance()).is_some_and(|d| f.range > d);
            if hands.step_zeroing(up) {
                zeroing_presses += 1;
            }
            if hands.sights().is_some_and(|s| s.zeroing_distance() != Some(f.range)) {
                zeroing_unfit.push(f.frame);
            }
        }
        if !stepped_already {
            hands.step_field_of_view(f.dt);
        }
        if f.frame < from || f.frame > to {
            continue;
        }
        let sights = hands.sights().expect("sights");
        let ours_on = aiming && sights.optic();
        gaps.entry("main camera's field of view").or_default().note(f.frame, hands.field_of_view_now(), f.fov);
        if f.optic_on && ours_on {
            gaps.entry("optic camera's field of view").or_default().note(f.frame, sights.optic_field_of_view().unwrap_or(0.0), f.optic_fov);
        }
        gaps.entry("zoom value").or_default().note(f.frame, sights.zoom_value(), f.zoom_value);
        gaps.entry("item's sensitivity").or_default().note(f.frame, sights.item_sensitivity().unwrap_or(1.0), f.sensitivity);
        gaps.entry("magnification").or_default().note(f.frame, sights.zoom().unwrap_or(0.0), f.zoom);
        let (scope, mode) = sights.scope_and_mode().unwrap_or((0, 0));
        let bone = &names[sights.current().bone];
        let checks: [(&str, bool); 8] = [
            ("zeroing distance", sights.zeroing_distance() == Some(f.range)),
            ("sight index", sights.aim_index() == f.index),
            ("sights' count", sights.list().len() == f.count),
            ("optic", sights.optic() == f.optic),
            ("scope", scope == f.scope),
            ("mode", mode == f.mode),
            ("optic camera on", ours_on == f.optic_on),
            ("bone name", *bone == f.bone),
        ];
        for (name, same) in checks {
            let entry = mismatched.entry(name).or_insert((0, 0));
            if !same {
                if entry.0 == 0 {
                    entry.1 = f.frame;
                }
                entry.0 += 1;
            }
        }
        if every {
            println!(
                "{:5} aim {} fov {:8.4} / {:8.4}  zoom {:7.4} / {:7.4}  optic fov {:7.4} / {:7.4}{}",
                f.frame,
                aiming as u8,
                hands.field_of_view_now(),
                f.fov,
                sights.zoom_value(),
                f.zoom_value,
                sights.optic_field_of_view().unwrap_or(0.0),
                f.optic_fov,
                if f.optic_on { "" } else { " (off)" }
            );
        }
    }
    println!("{} frames; the magnification key read off {} of them, the wheel off {} ({} notches); {} frames no input fits{}", frames.len(), presses, notched_frames, notches, unfit.len(), if unfit.is_empty() { String::new() } else { format!(" (first {:?})", &unfit[..unfit.len().min(10)]) });
    println!("the recorded zoom moved on {moved} frames, {moved_alone} of them with no key or notch read off them");
    println!("the zeroing keys read off {zeroing_presses} frames; {} frames no single press fits{}", zeroing_unfit.len(), if zeroing_unfit.is_empty() { String::new() } else { format!(" (first {:?})", &zeroing_unfit[..zeroing_unfit.len().min(10)]) });
    for (name, gap) in &gaps {
        println!("  {name:32} largest gap {:.6} at frame {} ({} frames compared)", gap.worst, gap.at, gap.compared);
    }
    for (name, (count, first)) in &mismatched {
        if *count == 0 {
            println!("  {name:32} the same on every frame");
        } else {
            println!("  {name:32} DIFFERS on {count} frames, first {first}");
        }
    }
    Ok(())
}
