//! Tests of the recorded things let fall and thrown, each set beside the rule that set it going.
//! Course 1 of recorder 0.10.0 (`local/traces/courses2/1-drop-*`), each thing set
//! beside the rule that set it going, the body the install's model gives
//! (with the rounds it shows: `with_rounds`) and that body's fall, as
//! `bin/loose` does (`docs/spec/physics.md`, "Loose things: compared").
//! Every thrown thing starts where, as fast and turned as the rule says;
//! every set-up is the game's but the first F1's (its model turned a
//! radian inside its object, not known why); on the flat recordings every
//! fall is the game's to a quarter second and within a centimetre to half
//! a second, and the mean gap at a second no more than when last measured
//! (2.8 cm) and a little over. Skipped where the recordings or the install
//! are not there.

use trp_physics::recorded_loose::{compare_setup, compare_start, drift, fall, model_key, read, with_rounds};
use trp_physics::setup::{Line, Report};
use trp_physics::Material;

const TRACES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses2");

/// The recordings on flat ground (the slope's falls meet the level, which
/// a recording of 0.10.0 does not place).
const FLAT: [&str; 3] = ["1-drop-quick-reload/drop-quick-reload.jsonl", "1-drop-heavy/drop-heavy.jsonl", "1-drop-from-inventory/drop-from-inventory.jsonl"];
const SLOPE: &str = "1-drop-on-slope/drop-on-slope.jsonl";

/// The F1 grenade, whose first throw's model the game turned a radian.
const F1: &str = "5710c24ad2720bc3458b45a3";

#[test]
fn the_recorded_falls_are_followed() {
    let Ok(install) = trp_data::Install::locate() else { return };
    let physics = trp_data::setup::engine::PhysicsSettings::load(&install).expect("the physics settings");
    let db = trp_data::Database::load(&install).expect("the database");
    let mut assets = trp_data::unity::Assets::new(install).expect("the install's assets");
    let (mut sum, mut n) = (0.0, 0);
    for file in FLAT {
        let path = format!("{TRACES}/{file}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let trace = read(&path).expect(file);
        assert!(!trace.things.is_empty(), "{file}: nothing set going");
        for thing in &trace.things {
            let mut report = Report::default();
            compare_start(thing, &mut report);
            let differing: Vec<&Line> = report.lines.iter().filter(|l| matches!(l, Line::Differs(_))).collect();
            assert!(differing.is_empty(), "{file} {}: {differing:#?}", thing.id);
            let key = model_key(&trace.header, &db, &thing.template);
            let (colliders, _) = with_rounds(&mut assets, &db, thing, &key).expect(&key);
            let run = fall(thing, colliders.into_iter().map(|(_, c)| c).collect(), &physics, Material::default());
            let (quarter, half, second) = (drift(thing, &run, 0.25).0, drift(thing, &run, 0.5).0, drift(thing, &run, 1.0).0);
            println!("{file} {}: {quarter:.4} m to 0.25 s, {half:.4} to 0.5 s, {second:.4} to 1 s", thing.id);
            assert!(quarter <= 0.001, "{file} {}: {quarter} m to 0.25 s", thing.id);
            assert!(half <= 0.01, "{file} {}: {half} m to 0.5 s", thing.id);
            sum += second;
            n += 1;
        }
    }
    if n > 0 {
        let mean = sum / n as f32;
        println!("{n} things, {mean:.4} m to 1 s on the mean");
        assert!(mean <= 0.032, "{mean} m to 1 s on the mean, more than 0.032");
    }
}

/// Every recorded set-up (24 things, the slope's too) beside the body
/// ours makes: the same in every figure, the rounds a magazine or a weapon
/// shows with their colliders, and our rule showing the points the game
/// showed for the count a magazine's weight gives; but the first F1's
/// turn, the one gap known.
#[test]
fn the_recorded_set_ups_are_ours() {
    let Ok(install) = trp_data::Install::locate() else { return };
    let physics = trp_data::setup::engine::PhysicsSettings::load(&install).expect("the physics settings");
    let db = trp_data::Database::load(&install).expect("the database");
    let mut assets = trp_data::unity::Assets::new(install).expect("the install's assets");
    let (mut things, mut rounds_seen, mut f1_apart) = (0, 0, 0);
    for file in FLAT.iter().chain(std::iter::once(&SLOPE)) {
        let path = format!("{TRACES}/{file}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let trace = read(&path).expect(file);
        for thing in &trace.things {
            let key = model_key(&trace.header, &db, &thing.template);
            let (ours, found) = with_rounds(&mut assets, &db, thing, &key).expect(&key);
            assert!(found.iter().all(|note| !note.contains("NOT") && !note.contains("which our model has not") && !note.contains("no count")), "{file} {}: {found:#?}", thing.id);
            rounds_seen += ours.iter().filter(|(name, _)| name.contains('/')).count();
            let mut report = Report::default();
            compare_setup(thing, &ours, &physics, &mut report);
            let differing: Vec<&Line> = report.lines.iter().filter(|l| matches!(l, Line::Differs(_))).collect();
            if thing.template == F1 && !differing.is_empty() {
                f1_apart += 1;
                continue;
            }
            assert!(differing.is_empty(), "{file} {}: {differing:#?}", thing.id);
            things += 1;
        }
    }
    if things > 0 {
        println!("{things} set-ups the game's, {rounds_seen} rounds' colliders among them; {f1_apart} F1 apart");
        assert!(f1_apart <= 1, "{f1_apart} F1 set-ups apart");
        // The four drops hold 64 rounds shown (the AK magazines' 5 and 6,
        // the AKM's 7, the M700's 4 and its magazine's 3, the MR-133's 1).
        assert_eq!(rounds_seen, 64, "rounds' colliders");
    }
}
