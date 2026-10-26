//! Every recording of recorder 0.8.0's medicine course (course 1, kept in
//! `local/traces/courses1/1-*`) replayed (`trp_sim::health_replay`): each
//! effect's states come at the game's times within two frames (those at
//! work before the recording began left out), each part's points within
//! 0.01 and its maximum too (but a mended part's, a random share), energy
//! and hydration within 0.01; and each item's hands (`MedsHands` on the
//! item's own rig): their animator's states and `Active` on the game's
//! frames, how far each state has played within a ten-thousandth, and the
//! item gone from the hands at its put-away's mark (the game's next hands
//! made from then to three frames on). Skipped where the recordings or the
//! install are not there.

use trp_data::unity::Assets;
use trp_sim::health::HealthSkills;
use trp_sim::health_replay::{read, run};

/// The recorded character's skills as the recordings were made
/// (2026-10-15, the user's profile): level 40 in each that health uses.
fn recorded_skills() -> HealthSkills {
    HealthSkills::of_levels(|name| match name {
        "Health" | "Metabolism" | "StressResistance" | "Immunity" | "Surgery" | "Vitality" => 40,
        _ => 0,
    })
}

#[test]
fn the_medicine_course_is_followed() {
    let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses1");
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let Ok(steps) = std::fs::read_dir(folder) else { return };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install.clone()).expect("the assets");
    let mut uses = 0;
    let mut traces: Vec<std::path::PathBuf> = steps
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("1-"))
        .flat_map(|step| std::fs::read_dir(step.path()).into_iter().flatten().filter_map(|e| e.ok()).map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    traces.sort();
    assert!(traces.len() >= 9, "{} medicine recordings", traces.len());
    for path in traces {
        let path = path.to_string_lossy().into_owned();
        let (scenario, frames) = read(&path).expect("the trace");
        let replay = run(&db, Some(&mut assets), recorded_skills(), &scenario, &frames, 1, None).expect("the replay");
        for (key, game) in &replay.recorded {
            if replay.taken_up.contains(key) {
                continue;
            }
            let ours = replay.simulated.get(key).cloned().unwrap_or_default();
            let states = |log: &[(String, f32)]| log.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
            assert_eq!(states(game), states(&ours), "{path}: {} on {}: the game's {game:?}, ours {ours:?}", key.0, key.1);
            for ((state, t_game), (_, t_ours)) in game.iter().zip(&ours) {
                assert!((t_game - t_ours).abs() <= 0.02, "{path}: {} on {} {state}: the game's at {t_game}, ours at {t_ours}", key.0, key.1);
            }
        }
        for (i, (gap, t)) in replay.worst.iter().enumerate() {
            assert!(*gap <= 0.01, "{path}: part {i} {gap} off at {t}");
            let (max_gap, max_t, changed) = replay.worst_maximum[i];
            assert!(changed || max_gap <= 0.01, "{path}: part {i}'s maximum {max_gap} off at {max_t}");
        }
        assert!(replay.worst_energy.0 <= 0.01, "{path}: energy {:?}", replay.worst_energy);
        assert!(replay.worst_hydration.0 <= 0.01, "{path}: hydration {:?}", replay.worst_hydration);
        for hands in &replay.hands {
            let near = |a: f32, b: f32| (a - b).abs() <= 0.005;
            let same = |game: &[(String, f32)], ours: &[(String, f32)]| game.len() == ours.len() && game.iter().zip(ours).all(|(g, o)| g.0 == o.0 && near(g.1, o.1));
            assert!(same(&hands.game, &hands.ours), "{path}: {}'s states: the game's {:?}, ours {:?}", hands.template, hands.game, hands.ours);
            let flags = |list: &[(bool, f32)]| list.iter().map(|(a, t)| (a.to_string(), *t)).collect::<Vec<_>>();
            assert!(same(&flags(&hands.game_active), &flags(&hands.ours_active)), "{path}: {}'s Active: the game's {:?}, ours {:?}", hands.template, hands.game_active, hands.ours_active);
            assert!(hands.worst.0 <= 1e-4 && hands.compared > 300, "{path}: {}'s times {:?} over {}", hands.template, hands.worst, hands.compared);
            let (Some(game), Some(ours)) = (hands.gone[0], hands.gone[1]) else { panic!("{path}: {} gone {:?}", hands.template, hands.gone) };
            assert!(game >= ours - 0.001 && game <= ours + 0.03, "{path}: {} gone from the hands, the game's at {game}, ours at {ours}", hands.template);
            uses += 1;
        }
        println!("{path}: followed");
    }
    assert_eq!(uses, 12, "the hands of twelve uses");
}

/// Recorder 0.8.0's course 2 (falls and barbed wire, `2-*`): each fall
/// worked out from the recording's heights as the game's ground check reads
/// them, before the frame's move, the chances drawn as the game's came;
/// the wire's cuts as the game made them. Every part within 0.01, every
/// effect's states on the game's frames; and each cut of the wire a tally of
/// 1 or more done whole (the fraction over a whole point is in it), not a
/// whole point.
#[test]
fn the_falls_and_the_wire_are_followed() {
    let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses1");
    let Ok(_install) = crate::fixture::install().ok_or(()) else { return };
    let Ok(steps) = std::fs::read_dir(folder) else { return };
    let db = crate::fixture::database().expect("database");
    let mut traces: Vec<std::path::PathBuf> = steps
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("2-"))
        .flat_map(|step| std::fs::read_dir(step.path()).into_iter().flatten().filter_map(|e| e.ok()).map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    traces.sort();
    assert_eq!(traces.len(), 6, "the course's six recordings");
    let mut falls = 0;
    for path in traces {
        let path = path.to_string_lossy().into_owned();
        let (scenario, frames) = read(&path).expect("the trace");
        let replay = run(&db, None, recorded_skills(), &scenario, &frames, 1, None).expect("the replay");
        for (key, game) in &replay.recorded {
            if replay.taken_up.contains(key) {
                continue;
            }
            let ours = replay.simulated.get(key).cloned().unwrap_or_default();
            assert_eq!(game.len(), ours.len(), "{path}: {} on {}: the game's {game:?}, ours {ours:?}", key.0, key.1);
            for ((state, t_game), (ours_state, t_ours)) in game.iter().zip(&ours) {
                assert_eq!(state, ours_state, "{path}: {} on {}", key.0, key.1);
                assert!((t_game - t_ours).abs() <= 0.02, "{path}: {} on {} {state}: the game's at {t_game}, ours at {t_ours}", key.0, key.1);
            }
        }
        assert!(replay.simulated.keys().all(|key| replay.recorded.contains_key(key)), "{path}: an effect the game had not");
        for (i, (gap, t)) in replay.worst.iter().enumerate() {
            assert!(*gap <= 0.01, "{path}: part {i} {gap} off at {t}");
        }
        falls += replay.notes.iter().filter(|note| note.contains("a fall of")).count();
        if scenario == "barbed-wire" {
            assert!(replay.cuts.len() > 40, "{} cuts", replay.cuts.len());
            assert!(replay.cuts.iter().all(|(_, _, tally)| *tally >= 0.999), "{:?}", replay.cuts);
            let over = replay.cuts.iter().filter(|(_, _, tally)| *tally > 1.001).count();
            assert!(over * 2 > replay.cuts.len(), "a cut is the tally over 1, not a whole point: {over} of {}", replay.cuts.len());
        }
        println!("{path}: followed");
    }
    assert_eq!(falls, 3, "the three falls over 3 m (the high drop's second take, the ledge's two)");
}
