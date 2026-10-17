//! A recording's health set beside the simulation's (`trp_sim::health_replay`;
//! `docs/spec/health.md`, "Compared with what"): each effect's states with
//! their times, the game's beside ours, the parts at the end, and the
//! largest gaps; with the install's assets (unless `--no-hands`), each
//! item's hands: their animator's states and `Active` with their times, the
//! game's beside ours.
//!
//! cargo run -p trp-sim --bin health_replay -- <trace.jsonl> [--seed n] [--every s] [--untrained] [--no-hands]

use anyhow::{Context, Result};
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::health::{HealthSkills, Part};
use trp_sim::health_replay::{read, run, Log};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).context("a trace")?;
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f32>().ok());
    let seed = flag("--seed").unwrap_or(1.0) as u64;
    let every = flag("--every");
    let (scenario, frames) = read(path)?;
    let install = Install::locate()?;
    let db = Database::load(&install)?;
    // The recorded character's skills (the newest profile of the install;
    // `--untrained` leaves them at none, as the app's character is).
    let skills = match install.profile_skill_levels().filter(|_| !args.iter().any(|a| a == "--untrained")) {
        Some(levels) => {
            let shown: Vec<String> = ["Health", "Metabolism", "StressResistance", "Immunity", "Surgery", "Vitality"].iter().map(|n| format!("{n} {}", levels.get(*n).copied().unwrap_or(0))).collect();
            println!("skills of the newest profile: {}", shown.join(", "));
            HealthSkills::of_levels(|name| levels.get(name).copied().unwrap_or(0))
        }
        None => {
            println!("untrained");
            HealthSkills::default()
        }
    };
    let mut assets = if args.iter().any(|a| a == "--no-hands") { None } else { Some(Assets::new(install.clone())?) };
    let replay = run(&db, assets.as_mut(), skills, &scenario, &frames, seed, every)?;
    println!("{scenario}: {} frames, {:.2} s; seed {seed}", frames.len(), frames.last().map_or(0.0, |f| f.t));
    for line in &replay.samples {
        println!("{line}");
    }
    if !replay.cuts.is_empty() {
        println!("\nbarbed wire's cuts, the game's (the tally a part's drop gives over its multiplier):");
        for part in Part::ALL {
            let tallies: Vec<f32> = replay.cuts.iter().filter(|(_, p, _)| *p == part).map(|(_, _, tally)| *tally).collect();
            if let (Some(least), Some(most)) = (tallies.iter().copied().reduce(f32::min), tallies.iter().copied().reduce(f32::max)) {
                println!("  {:9} {:3} cuts, tallies {least:.4} to {most:.4}, {:.3} in all", part.name(), tallies.len(), tallies.iter().sum::<f32>());
            }
        }
    }
    println!("\nwhat was done:");
    for line in &replay.notes {
        println!("  {line}");
    }
    println!("\neffects (state at seconds), the game's and ours:");
    let keys: std::collections::BTreeSet<&(String, String)> = replay.recorded.keys().chain(replay.simulated.keys()).collect();
    for key in keys {
        let show = |log: &Log| log.get(key).map(|l| l.iter().map(|(s, t)| format!("{s} {t:.3}")).collect::<Vec<_>>().join(", ")).unwrap_or_else(|| "-".into());
        let taken = if replay.taken_up.contains(key) { " (as it began before the recording: taken up)" } else { "" };
        println!("  {} on {}{taken}\n    game: {}\n    ours: {}", key.0, key.1, show(&replay.recorded), show(&replay.simulated));
    }
    let show = |parts: &[[f32; 2]; 7]| parts.iter().map(|p| format!("{:.1}/{}", p[0], p[1])).collect::<Vec<_>>().join(" ");
    println!("\nat the end, the game's: {}\n            ours:       {}", show(&replay.end[0]), show(&replay.end[1]));
    println!("\nlargest gaps (current; maximum, * where the game's changed):");
    for part in Part::ALL {
        let (gap, t) = replay.worst[part.index()];
        let (max_gap, max_t, changed) = replay.worst_maximum[part.index()];
        println!("  {:9} {gap:7.3} at {t:.3} s; {max_gap:7.3} at {max_t:.3} s{}", part.name(), if changed { " *" } else { "" });
    }
    println!(
        "  energy    {:7.3} at {:.3} s\n  hydration {:7.3} at {:.3} s",
        replay.worst_energy.0, replay.worst_energy.1, replay.worst_hydration.0, replay.worst_hydration.1
    );
    for hands in &replay.hands {
        let states = |list: &[(String, f32)]| list.iter().map(|(s, t)| format!("{} {t:.3}", s.trim_start_matches("Hands."))).collect::<Vec<_>>().join(", ");
        let active = |list: &[(bool, f32)]| list.iter().map(|(a, t)| format!("{a} {t:.3}")).collect::<Vec<_>>().join(", ");
        let at = |t: Option<f32>| t.map_or("-".to_owned(), |t| format!("{t:.3}"));
        println!("\nthe hands of {} (the variants' count from {}):", db.items.get(&hands.template).map_or("?", |i| i.name.as_str()), hands.first);
        println!("  game: {}\n  ours: {}", states(&hands.game), states(&hands.ours));
        println!("  Active, game: {}\n          ours: {}", active(&hands.game_active), active(&hands.ours_active));
        println!("  gone from the hands: the game's {}, ours {}", at(hands.gone[0]), at(hands.gone[1]));
        println!("  largest gap in a state's time: {:.6} at {:.3} s, over {} frames", hands.worst.0, hands.worst.1, hands.compared);
    }
    Ok(())
}
