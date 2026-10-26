//! The four deaths the user recorded on 2026-10-15 (course 0 of recorder
//! 0.8.0, kept in `local/traces/courses1`), each rebuilt and stepped as
//! `bin/corpse` does: the bodies may drift from the recorded no further
//! than they did when the corpse was first matched (`docs/spec/physics.md`,
//! "Measured"), a little over. Skipped where the recordings or the install
//! are not there.

use trp_data::ragdoll::RagdollData;
use trp_physics::recorded::{drift, read, simulate};
use trp_physics::JointConvention;

/// Each death, and the most mean drift allowed to 0.25 s and to 0.5 s, in
/// metres: the measured (2.62, 3.45, 1.46, 2.40 cm; 4.65, 6.18, 2.04,
/// 6.24 cm) and half a centimetre, and a centimetre.
const DEATHS: [(&str, f32, f32); 4] = [
    ("0-death-standing/01-standing.jsonl", 0.031, 0.057),
    ("0-death-walking/02-walking.jsonl", 0.040, 0.072),
    ("0-death-sprinting/03-sprinting.jsonl", 0.020, 0.031),
    ("0-death-shot/04-shot.jsonl", 0.029, 0.073),
];

#[test]
fn the_recorded_deaths_are_followed() {
    let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses1");
    let Ok(install) = trp_data::Install::locate() else { return };
    if !std::path::Path::new(folder).exists() {
        return;
    }
    let mut assets = trp_data::unity::Assets::new(install).expect("the install's assets");
    let data = RagdollData::load(&mut assets).expect("the ragdoll");
    for (file, quarter, half) in DEATHS {
        let trace = read(&format!("{folder}/{file}")).expect(file);
        let run = simulate(&data, &trace, JointConvention::GAME, true);
        let (to_quarter, to_half) = (drift(&trace, &run, 0.25), drift(&trace, &run, 0.5));
        println!("{file}: {to_quarter:.4} m to 0.25 s, {to_half:.4} m to 0.5 s");
        assert!(to_quarter <= quarter, "{file}: {to_quarter} m to 0.25 s, more than {quarter}");
        assert!(to_half <= half, "{file}: {to_half} m to 0.5 s, more than {half}");
    }
}
