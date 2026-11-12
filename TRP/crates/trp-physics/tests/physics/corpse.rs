//! Tests of the recorded deaths, each rebuilt and stepped as `bin/corpse` does.
//! Course 0 of recorder 0.8.0 (`local/traces/courses1`) and of 0.10.0 (`courses2`), each
//! rebuilt and stepped as `bin/corpse` does: the bodies may drift from the
//! recorded no further than they did when last measured
//! (`docs/spec/physics.md`, "Compared with what"), a little over. And
//! `courses2`'s, whose first frame after the death holds the game's own
//! set-up of the corpse, the engine and the weapon let go, set beside ours
//! (`bin/corpse --setup`): every figure the same. Skipped where the
//! recordings or the install are not there.

use trp_data::character::ragdoll::RagdollData;
use trp_physics::recorded::{drift, read, simulate};
use trp_physics::JointConvention;

/// Each death, and the most mean drift allowed to 0.25 s and to 0.5 s, in
/// metres. `courses1`'s: first matched (2.62, 3.45, 1.46, 2.40 cm; 4.65,
/// 6.18, 2.04, 6.24 cm) and half a centimetre, and a centimetre; measured
/// with the present colliders (2.62, 3.45, 1.47, 2.39; 4.65, 6.18, 2.53,
/// 6.21). `courses2`'s: measured (2.20, 3.70, 1.66, 2.16; 3.32,
/// 7.29, 2.64, 5.04) and the same again.
const DEATHS: [(&str, f32, f32); 8] = [
    ("courses1/0-death-standing/01-standing.jsonl", 0.031, 0.057),
    ("courses1/0-death-walking/02-walking.jsonl", 0.040, 0.072),
    ("courses1/0-death-sprinting/03-sprinting.jsonl", 0.020, 0.031),
    ("courses1/0-death-shot/04-shot.jsonl", 0.029, 0.073),
    ("courses2/0-death-standing/01-standing.jsonl", 0.027, 0.044),
    ("courses2/0-death-walking/02-walking.jsonl", 0.042, 0.083),
    ("courses2/0-death-sprinting/03-sprinting.jsonl", 0.022, 0.037),
    ("courses2/0-death-shot/04-shot.jsonl", 0.027, 0.061),
];

const TRACES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces");

#[test]
fn the_recorded_deaths_are_followed() {
    let Ok(install) = trp_data::Install::locate() else { return };
    let mut assets = trp_data::unity::Assets::new(install).expect("the install's assets");
    let data = RagdollData::load(&mut assets).expect("the ragdoll");
    for (file, quarter, half) in DEATHS {
        let path = format!("{TRACES}/{file}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let trace = read(&path).expect(file);
        let run = simulate(&data, &trace, JointConvention::GAME, true);
        let (to_quarter, to_half) = (drift(&trace, &run, 0.25), drift(&trace, &run, 0.5));
        println!("{file}: {to_quarter:.4} m to 0.25 s, {to_half:.4} m to 0.5 s");
        assert!(to_quarter <= quarter, "{file}: {to_quarter} m to 0.25 s, more than {quarter}");
        assert!(to_half <= half, "{file}: {to_half} m to 0.5 s, more than {half}");
    }
}

#[test]
fn the_recorded_set_up_is_ours() {
    let Ok(install) = trp_data::Install::locate() else { return };
    let mut assets = trp_data::unity::Assets::new(install).expect("the install's assets");
    let data = RagdollData::load(&mut assets).expect("the ragdoll");
    for (file, ..) in DEATHS.iter().filter(|(file, ..)| file.starts_with("courses2")) {
        let path = format!("{TRACES}/{file}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let report = trp_physics::setup::compare(&path, &data).expect(file);
        let (checked, differ) = report.counts();
        let differing: Vec<String> = report.lines.iter().filter_map(|l| if let trp_physics::setup::Line::Differs(t) = l { Some(t.clone()) } else { None }).collect();
        println!("{file}: {checked} figures, {differ} differ");
        assert!(checked >= 750, "{file}: only {checked} figures set side by side");
        assert!(differing.is_empty(), "{file}: {differing:#?}");
    }
}
