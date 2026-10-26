//! The desaturation's accumulators beside a recording (health spec,
//! "Desaturation"): each frame's health effects as recorded (`effects`:
//! a painkiller, exhaustion, tunnel vision, and a medical use whose item in
//! hand is a medkit) put through `trp_sim::screen_effects::Desaturation` on
//! the recording's own frame times (`dt`), from its first frame's figures,
//! beside the recorded `DesaturateEffect.HealthDesaturate`, the sharpening's
//! `MaskDesaturate` and `DesaturateEffect.Radius` (what the sharpening's
//! pass takes). Prints the largest gap of each and where it is.
//!
//! `cargo run -p trp-sim --bin desaturation -- <trace.jsonl>...`

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde_json::Value;
use trp_data::{Database, Install};
use trp_sim::health::{MedClass, Medicine};
use trp_sim::screen_effects::{Desaturation, Listed, Source};

fn main() -> Result<()> {
    let db = Database::load(&Install::locate()?)?;
    for path in std::env::args().skip(1) {
        compare(&db, &path).with_context(|| path.clone())?;
    }
    Ok(())
}

fn compare(db: &Database, path: &str) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    // Items by their id, as any frame's equipment lists them.
    let mut templates: HashMap<String, String> = HashMap::new();
    let mut in_hands = String::new();
    // The recorded figures, kept as the last written (only changes are).
    let mut recorded = [0.0f32; 3];
    let keys = ["16:DesaturateEffect.HealthDesaturate", "30:CC_Sharpen.MaskDesaturate", "16:DesaturateEffect.Radius"];
    let mut radii = [0.325, 1.0];
    // Each recorded effect's id, by its type and part, while it is listed.
    let mut ids: HashMap<(String, String), u32> = HashMap::new();
    let mut next_id = 1;
    let mut medkit: HashMap<u32, bool> = HashMap::new();
    let mut sim: Option<Desaturation> = None;
    let mut worst = [(0.0f32, 0u64); 3];
    let mut frames = 0;
    let mut moved = [0usize; 3];
    let mut apart = [false; 3];
    for line in text.lines() {
        let Ok(frame) = serde_json::from_str::<Value>(line) else { continue };
        let Some(number) = frame.get("frame").and_then(Value::as_u64) else { continue };
        if let Some(items) = frame.get("equipment").and_then(Value::as_array) {
            for item in items {
                if let (Some(id), Some(tpl)) = (item.get("id").and_then(Value::as_str), item.get("tpl").and_then(Value::as_str)) {
                    templates.insert(id.to_owned(), tpl.to_owned());
                }
            }
        }
        if let Some(hands) = frame.get("in_hands").and_then(Value::as_str) {
            in_hands = hands.to_owned();
        }
        if let Some(fx) = frame.get("cam_fx") {
            for (k, key) in keys.iter().enumerate() {
                if let Some(v) = fx.get(*key).and_then(Value::as_f64) {
                    if number > 0 && (v as f32 - recorded[k]).abs() > 0.0 {
                        moved[k] += 1;
                    }
                    recorded[k] = v as f32;
                }
            }
            if let Some(r) = fx.get("16:DesaturateEffect.MinMaxRadius").and_then(Value::as_array) {
                radii = [r[0].as_f64().unwrap_or(0.325) as f32, r[1].as_f64().unwrap_or(1.0) as f32];
            }
        }
        let Some(effects) = frame.get("effects").and_then(Value::as_array) else { continue };
        let mut seen = Vec::new();
        let mut listed = Vec::new();
        for e in effects {
            let (Some(kind), Some(part), Some(state)) = (e.get(0).and_then(Value::as_str), e.get(1).and_then(Value::as_str), e.get(2).and_then(Value::as_str)) else { continue };
            let key = (kind.to_owned(), part.to_owned());
            let id = *ids.entry(key.clone()).or_insert_with(|| {
                next_id += 1;
                next_id
            });
            seen.push(key);
            let started = matches!(state, "Started" | "Residued" | "Residual");
            let source = match kind {
                "IPainKiller" => Source::PainKiller,
                "IExhaustion" => Source::Exhaustion,
                "ITunnelVision" => Source::TunnelVision,
                "IMedEffect" => {
                    // The item in hand as it starts, a medkit or not.
                    let is = *medkit.entry(id).or_insert_with(|| templates.get(&in_hands).and_then(|tpl| Medicine::of(db, tpl)).is_some_and(|m| m.class == MedClass::MedKit));
                    if !is {
                        continue;
                    }
                    Source::MedKit
                }
                _ => continue,
            };
            listed.push(Listed { id, source, current: 1.0, strength: 1.0, residual: state.starts_with("Resid"), started });
        }
        ids.retain(|key, _| seen.contains(key));
        let dt = frame.get("dt").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let figures = match sim.as_mut() {
            None => {
                sim = Some(Desaturation::resume(recorded, radii, &listed));
                recorded
            }
            Some(d) => d.frame(dt, &listed),
        };
        frames += 1;
        for k in 0..3 {
            let gap = (figures[k] - recorded[k]).abs();
            if gap > worst[k].0 {
                worst[k] = (gap, number);
            }
            // Where a run of differing frames begins.
            if gap > 1e-5 && !apart[k] {
                println!("  {} parts at frame {number}: {:.6} against {:.6}", ["health", "mask", "radius"][k], figures[k], recorded[k]);
            }
            apart[k] = gap > 1e-5;
        }
    }
    println!("{path}: {frames} frames; recorded changes: health {}, mask {}, radius {}", moved[0], moved[1], moved[2]);
    for (k, name) in ["health", "mask", "radius"].iter().enumerate() {
        println!("  {name}: largest gap {:.6} at frame {}", worst[k].0, worst[k].1);
    }
    Ok(())
}
