//! The sounds of moving (`docs/spec/footsteps.md`) asked for by the game's
//! rules as the real body animator walks, sprints, stops, jumps and
//! crawls. These pass trivially, with a note, where no install is
//! configured.
//!
//! Run with `cargo test -p trp-sim --test steps -- --nocapture` for the numbers.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::steps::{BankOf, Play, CONCRETE, GEAR, GEAR_FAST, JUMP, LANDING, PRONE, RUN, SPRINT, STOP};
use trp_sim::{Flat, Input, Movement, State};

const DT: f32 = 1.0 / 120.0;

fn movement() -> Option<Movement> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    let settings = trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement");
    // The client's multipliers are read (SPT's settings.json): the run's is 0.85.
    assert!((settings.steps.multipliers[trp_sim::steps::Kind::Run as usize] - 0.85).abs() < 1e-6);
    Some(Movement::new(settings, trp_sim::Body::new(&body, 2.0)))
}

/// Runs the movement for so long with the input, `press` given on the
/// first step only, and gathers what the sounds' rules asked for with
/// each one's time.
fn run(movement: &mut Movement, clock: &mut f32, seconds: f32, input: Input, press: Input) -> Vec<(f32, Play)> {
    let mut asked = Vec::new();
    for step in 0..(seconds / DT) as usize {
        movement.step(DT, if step == 0 { &press } else { &input }, &Flat);
        *clock += DT;
        asked.extend(movement.sounds.take().into_iter().map(|play| (*clock, play)));
    }
    asked
}

fn of(asked: &[(f32, Play)], bank: BankOf) -> Vec<(f32, Play)> {
    asked.iter().filter(|(_, play)| play.bank == bank).copied().collect()
}

#[test]
fn walking_sprinting_jumping_and_crawling_are_heard() {
    let Some(mut movement) = movement() else { return };
    let mut clock = 0.0;
    let still = Input::default();
    run(&mut movement, &mut clock, 1.0, still, still);

    // A walk: a footstep at each foot down, each with the kit 0.1 s after.
    let walk = Input { move_dir: [0.0, 1.0], ..Input::default() };
    let asked = run(&mut movement, &mut clock, 6.0, walk, walk);
    let steps = of(&asked, BankOf::Surface(CONCRETE, RUN));
    let kit = of(&asked, BankOf::Kit(GEAR));
    let gaps: Vec<f32> = steps.windows(2).map(|w| w[1].0 - w[0].0).collect();
    println!("the walk: {} footsteps, {} of the kit; gaps {:.3?}", steps.len(), kit.len(), gaps);
    println!("  volumes {:.3?}, blend {:.3}", steps.iter().map(|(_, p)| p.volume).collect::<Vec<_>>(), steps[0].1.blend);
    assert!(steps.len() >= 10, "a footstep about every half second");
    assert!(gaps.iter().skip(2).all(|g| (0.25..0.8).contains(g)), "{gaps:?}");
    assert_eq!(kit.len(), steps.len());
    for ((at, _), (kit_at, kit)) in steps.iter().zip(&kit) {
        assert!((kit_at - at - 0.1).abs() < 0.01, "the kit 0.1 s after: {at} {kit_at}");
        assert!(kit.kit && kit.volume > 0.0);
    }
    // At the top speed unburdened: the run's 0.85 at most, times the bank's
    // 0.68 at most (its figures not given here: 1, so 0.85).
    assert!(steps.iter().all(|(_, p)| p.volume > 0.5 && p.volume <= 0.85 + 1e-4));

    // A sprint, then let go: the sprint's footsteps with the kit at once,
    // and the stop.
    let sprint = Input { move_dir: [0.0, 1.0], sprint: true, ..Input::default() };
    let asked = run(&mut movement, &mut clock, 3.0, sprint, sprint);
    let sprints = of(&asked, BankOf::Surface(CONCRETE, SPRINT));
    println!("the sprint: {} footsteps, gaps {:.3?}", sprints.len(), sprints.windows(2).map(|w| w[1].0 - w[0].0).collect::<Vec<_>>());
    assert!(sprints.len() >= 5);
    assert!(movement.state == State::Sprint);
    let asked = run(&mut movement, &mut clock, 2.0, still, still);
    let stops = of(&asked, BankOf::Surface(CONCRETE, STOP));
    println!("let go: {:?}", asked.iter().map(|(t, p)| (*t, p.bank)).collect::<Vec<_>>());
    assert_eq!(stops.len(), 1, "one stop");

    // A jump from standing: the jump's sound and the kit's fast rattle,
    // then the landing.
    let jump = Input { jump: true, ..Input::default() };
    let asked = run(&mut movement, &mut clock, 2.0, still, jump);
    println!("the jump: {:?}", asked.iter().map(|(t, p)| (*t, p.bank, p.volume)).collect::<Vec<_>>());
    assert_eq!(of(&asked, BankOf::Surface(CONCRETE, JUMP)).len(), 1);
    assert_eq!(of(&asked, BankOf::Kit(GEAR_FAST)).len(), 1);
    assert_eq!(of(&asked, BankOf::Surface(CONCRETE, LANDING)).len(), 1, "the drop from the jump's top is over 0.3 m");

    // Down to prone, then crawl: the crawl's marks at least half a second
    // apart.
    let prone = Input { prone: true, ..Input::default() };
    let asked = run(&mut movement, &mut clock, 2.0, still, prone);
    assert_eq!(of(&asked, BankOf::Kit(GEAR_FAST)).len(), 1, "going prone rattles the kit");
    let asked = run(&mut movement, &mut clock, 6.0, walk, walk);
    let crawls = of(&asked, BankOf::Surface(CONCRETE, PRONE));
    let gaps: Vec<f32> = crawls.windows(2).map(|w| w[1].0 - w[0].0).collect();
    println!("the crawl: {} marks heard, gaps {gaps:.3?}, volumes {:.3?}", crawls.len(), crawls.iter().map(|(_, p)| p.volume).collect::<Vec<_>>());
    assert!(movement.state == State::ProneMove);
    assert!(crawls.len() >= 4, "two marks a cycle of the crawl");
    assert!(gaps.iter().all(|g| *g >= 0.5 - 1e-3));
}
