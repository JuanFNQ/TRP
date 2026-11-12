//! Tests of the game's pick of the levels of detail against three recordings of it.
//! The pick is `trp_sim::lod::pick`, Unity's rule read in the engine; the recordings are
//! recorder 0.18.0's, `local/traces/lod_s271`, compared in `local/lod_compare_s271.md`.
//! Every group-frame of the held Scav that the
//! main camera had in its view and that was switched on, and every one the
//! optic camera drew, the rule's level against the level the camera drew.
//! TRP draws by its own rule (`trp_sim::lod`, "TRP's own
//! rule"); the game's is kept as the game's, and this says it still is.

use std::collections::HashMap;
use std::io::BufRead;

use serde_json::Value;
use trp_sim::characters::lod::{pick, reach};

#[derive(Default, Debug)]
struct Tally {
    main: usize,
    main_same: usize,
    optic: usize,
    optic_same: usize,
    /// Group-frames whose distance lies within a hundredth of a millimetre
    /// a metre of a level's reach (the record's rounding), left out: the
    /// main camera's, the optic's.
    edge: usize,
    optic_edge: usize,
    wrong: Vec<String>,
}

fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(f64::NAN) as f32
}

fn take(path: &std::path::Path) -> Tally {
    let file = std::fs::File::open(path).expect("the take");
    let mut lines = std::io::BufReader::new(file).lines();
    let _header = lines.next();
    let mut heights: HashMap<i64, Vec<f32>> = HashMap::new();
    let mut points: HashMap<String, [f32; 4]> = HashMap::new();
    let mut tally = Tally::default();
    for line in lines {
        let Ok(line) = line else { break };
        let Ok(o) = serde_json::from_str::<Value>(&line) else { continue };
        if let Some(groups) = o.get("groups").and_then(Value::as_array) {
            for g in groups {
                let levels = g["levels"].as_array().map(|levels| levels.iter().map(|level| f(&level[0])).collect()).unwrap_or_default();
                heights.insert(g["g"].as_i64().unwrap_or(-1), levels);
            }
        }
        let Some(chars) = o.get("chars").and_then(Value::as_array) else { continue };
        if o.get("late").is_some() {
            continue;
        }
        let camera = |name: &str| -> Option<([f32; 3], f32, f32)> {
            let c = o.get(name).filter(|c| !c.is_null())?;
            let p = c["p"].as_array()?;
            Some(([f(&p[0]), f(&p[1]), f(&p[2])], f(&c["fov"]), f(&c["bias"])))
        };
        let (main, optic) = (camera("main"), camera("optic"));
        for c in chars {
            if let Some(p) = c.get("p").and_then(Value::as_object) {
                for (g, v) in p {
                    if let Some(v) = v.as_array() {
                        points.insert(g.clone(), [f(&v[0]), f(&v[1]), f(&v[2]), f(&v[3])]);
                    }
                }
            }
            for e in c.get("g").and_then(Value::as_array).into_iter().flatten() {
                let Some(e) = e.as_array() else { continue };
                let (g, drawn, flags) = (e[0].as_i64().unwrap_or(-1), e[1].as_i64().unwrap_or(-2), e[3].as_i64().unwrap_or(0));
                let (Some(levels), Some(point)) = (heights.get(&g), points.get(&g.to_string())) else { continue };
                let size = point[3];
                let judge = |camera: ([f32; 3], f32, f32), got: i64, count: &mut usize, same: &mut usize, wrong: &mut Vec<String>, edge: &mut usize| {
                    let (eye, fov, bias) = camera;
                    let d = ((point[0] - eye[0]).powi(2) + (point[1] - eye[1]).powi(2) + (point[2] - eye[2]).powi(2)).sqrt();
                    if levels.iter().any(|&h| (d / reach(size, 1.0, h, fov, bias) - 1.0).abs() < 1e-5) {
                        *edge += 1;
                        return;
                    }
                    *count += 1;
                    let rule = pick(size, 1.0, d, fov, bias, levels).map_or(-1, |level| level as i64);
                    if rule == got {
                        *same += 1;
                    } else if wrong.len() < 5 {
                        wrong.push(format!("frame {} group {g}: {d:.3} m, rule {rule}, game {got}", o["frame"]));
                    }
                };
                // The main camera: in its view (flag 1) and switched on (not
                // flag 64).
                if let Some(camera) = main.filter(|_| flags & 1 != 0 && flags & 64 == 0 && drawn != -2) {
                    judge(camera, drawn, &mut tally.main, &mut tally.main_same, &mut tally.wrong, &mut tally.edge);
                }
                // The optic camera, where it drew this frame.
                if let (Some(camera), Some(got)) = (optic, e.get(4).and_then(Value::as_i64)) {
                    if flags & 64 == 0 {
                        judge(camera, got, &mut tally.optic, &mut tally.optic_same, &mut tally.wrong, &mut tally.optic_edge);
                    }
                }
            }
        }
    }
    tally
}

/// The three takes on Shoreline at 2560 by 1440, FOV 74, bias 2: walking
/// back from the held Scav to 201 m, through the 4x scope at 201 to 6 m,
/// and turned away at 5, 21 and 60 m. Every main-camera group-frame in view
/// and every optic one agrees (105,862 and 14,700); the two that lie within
/// the record's rounding of a reach (a hundred-thousandth of it: one in the
/// walk, and the scope take's balaclava at 176.939 m) are left out.
#[test]
fn the_games_pick_as_the_three_recordings_drew_it() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/lod_s271"));
    if !dir.exists() {
        eprintln!("skipped: no recordings of the levels of detail (local/traces/lod_s271)");
        return;
    }
    let mut all = (0, 0);
    for (name, main, optic) in [("0-lod-walk/lod-walk.lod.jsonl", 33_750, 0), ("0-lod-scope/lod-scope.lod.jsonl", 49_236, 14_700), ("0-lod-away/lod-away.lod.jsonl", 22_876, 0)] {
        let path = dir.join(name);
        if !path.exists() {
            eprintln!("skipped: no {name}");
            continue;
        }
        let tally = take(&path);
        eprintln!("{name}: main {} of {} agree, optic {} of {}, at a reach's edge {}; {:?}", tally.main_same, tally.main, tally.optic_same, tally.optic, tally.edge, tally.wrong);
        assert!(tally.edge + tally.optic_edge <= 1, "{name}: {tally:?}");
        assert_eq!((tally.main + tally.edge, tally.optic + tally.optic_edge), (main, optic), "{name}: {tally:?}");
        assert_eq!(tally.main_same, tally.main, "{name}: {:?}", tally.wrong);
        assert_eq!(tally.optic_same, tally.optic, "{name}: {:?}", tally.wrong);
        all.0 += tally.main + tally.edge;
        all.1 += tally.optic;
    }
    eprintln!("main group-frames {}, optic {}", all.0, all.1);
}
