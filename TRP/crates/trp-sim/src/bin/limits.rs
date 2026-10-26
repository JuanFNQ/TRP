//! Sets the armour's speed limit (`trp_sim::armor::speed_penalty`,
//! `Movement::set_armour_penalty`) beside recordings that have the kit and
//! the game's `StateSpeedLimit` (recorder 0.5.0 on: `equipment`; the frames'
//! `state_speed_limit`; the header's `skills` and `max_speed`). Movement
//! spec, "Speed limits", "Compared with".
//!
//!   limits <trace.jsonl or folder>...
//!
//! For each recording: the kit of its first frame as a tree, our penalty
//! from it with the recorded Light and Heavy Vests levels, the limit that
//! makes of the recorded top speed, beside the game's limit on that frame
//! (where neither aiming nor a hit nor a landing nor health holds it).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value as Json;
use trp_data::{Database, Install};
use trp_sim::armor::{speed_penalty, VestSkills, Worn};

/// The kit as the recorder lists it: each thing with its parent's id and
/// the slot it is in, under the one that has no parent.
fn worn_tree(equipment: &[Json]) -> Vec<Worn> {
    let root = equipment.iter().find(|e| e.get("in").is_none()).and_then(|e| e.get("id")).and_then(Json::as_str).unwrap_or("");
    fn under(equipment: &[Json], parent: &str) -> Vec<Worn> {
        equipment
            .iter()
            .filter(|e| e.get("in").and_then(Json::as_str) == Some(parent))
            .map(|e| Worn {
                template: e.get("tpl").and_then(Json::as_str).unwrap_or("").to_owned(),
                slot: e.get("at").and_then(Json::as_str).unwrap_or("").to_owned(),
                children: under(equipment, e.get("id").and_then(Json::as_str).unwrap_or("")),
            })
            .collect()
    }
    under(equipment, root)
}

fn traces(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path).into_iter().flatten().flatten().map(|e| e.path()).collect();
        entries.sort();
        for entry in entries {
            traces(&entry, out);
        }
    } else if path.extension().is_some_and(|e| e == "jsonl") {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".world.jsonl") && !name.ends_with(".colliders.jsonl") {
            out.push(path.to_owned());
        }
    }
}

fn main() -> Result<()> {
    let db = Database::load(&Install::locate()?)?;
    // `--vests <light>,<heavy>`: the vest skills' levels for a recording
    // that does not write the skills (recorder 0.11.0's steps by hand).
    let mut paths = Vec::new();
    let mut fallback: Option<[u32; 2]> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--vests" {
            let given = args.next().unwrap_or_default();
            let levels: Vec<u32> = given.split(',').filter_map(|v| v.parse().ok()).collect();
            fallback = (levels.len() == 2).then(|| [levels[0], levels[1]]);
            continue;
        }
        traces(Path::new(&arg), &mut paths);
    }
    let (mut compared, mut worst, mut kits) = (0usize, 0.0f32, std::collections::BTreeMap::<String, (f32, f32, f32, usize)>::new());
    for path in paths {
        let text = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        let mut lines = text.lines();
        let Some(Ok(header)) = lines.next().map(serde_json::from_str::<Json>) else { continue };
        let Some(frame) = lines.find_map(|l| serde_json::from_str::<Json>(l).ok().filter(|f| f.get("frame").is_some())) else { continue };
        let (Some(top), Some(recorded), Some(equipment)) = (
            header.get("max_speed").and_then(Json::as_f64).map(|x| x as f32),
            frame.get("state_speed_limit").and_then(Json::as_f64).map(|x| x as f32),
            header.get("equipment").and_then(Json::as_array),
        ) else {
            continue;
        };
        // Only where nothing else holds the speed on that frame.
        if frame.get("aiming").and_then(Json::as_bool) == Some(true) {
            continue;
        }
        let level = |name: &str| -> u32 {
            let recorded = header.get("skills").and_then(Json::as_array).map(|all| {
                all.iter().find(|s| s.get(0).and_then(Json::as_str) == Some(name)).and_then(|s| s.get(1)).and_then(Json::as_f64).unwrap_or(0.0) as u32
            });
            recorded.or_else(|| fallback.map(|[light, heavy]| if name == "LightVests" { light } else if name == "HeavyVests" { heavy } else { 0 })).unwrap_or(0)
        };
        let vests = VestSkills::of_levels(level);
        let penalty = speed_penalty(&db, &worn_tree(equipment), &vests);
        let ours = if penalty < 0.0 { (100.0 + penalty) / 100.0 * top } else { 1.0 };
        let gap = (ours - recorded).abs();
        compared += 1;
        worst = worst.max(gap);
        let kit = format!("penalty {penalty:+.2}%");
        let entry = kits.entry(kit).or_insert((ours, recorded, 0.0, 0));
        entry.2 = entry.2.max(gap);
        entry.3 += 1;
        if gap > 1e-4 {
            println!("{}: ours {ours:.6}, the game's {recorded:.6} (penalty {penalty:+.3}%, top {top:.6})", path.display());
            for piece in trp_sim::armor::worn_pieces(&db, &worn_tree(equipment)).iter().filter(|p| p.speed_penalty != 0.0) {
                let name = db.items.get(&piece.template).map(|t| t.name.clone()).unwrap_or_default();
                println!("    {} {name}: {:+.2}%, {:?}", piece.template, piece.speed_penalty, piece.kind);
            }
        }
    }
    for (kit, (ours, recorded, gap, count)) in &kits {
        println!("{kit}: {count} recordings, ours {ours:.6}, the game's {recorded:.6}, largest gap {gap:.6}");
    }
    println!("{compared} recordings compared; largest gap {worst:.6}");
    Ok(())
}
