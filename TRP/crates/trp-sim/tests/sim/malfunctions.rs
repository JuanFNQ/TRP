//! Malfunctions on the install's AKM (`docs/spec/malfunctions.md`), each
//! given outright on its shot: the jam's shot and its held state, the
//! trigger that only clicks, the look that tells its kind, the repair at the
//! recorded character's speed (Troubleshooting 40, `SpeedFix` 1.2), the
//! misfire's missing shot, the failure to feed's lost round and the reload
//! it refuses. These pass trivially, with a note, where no install is
//! configured.

use trp_data::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::firearm::{Wear, WearSwitches};
use trp_sim::malfunction::{Malf, Settings, Skills, State, WeaponFigures};
use trp_sim::weapon_animator::WeaponAnimatorData;
use trp_sim::{FireMode, Firearm, FirearmEvent};

const AKM: &str = "59d6088586f774275f37482f";
const FRAME: f32 = 1.0 / 120.0;

fn akm() -> Option<(Firearm, WeaponFigures)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    let preset = db.default_preset(AKM).expect("the AKM's preset");
    let parts: Vec<&str> = preset.items.iter().map(|part| part.template.as_str()).filter(|template| *template != AKM).collect();
    let figures = WeaponFigures::of(&db, AKM, &parts);
    let settings_db = Settings::from_config(&db.config).expect("the globals' malfunction settings");
    let mut w = Firearm::new(settings, &data);
    w.start_drawn();
    let mut wear = Wear::new(figures, settings_db, 35.0, 100.0, State::default(), 1);
    wear.skills = Skills { troubleshooting: 40.0, ..Skills::default() };
    w.set_wear(wear);
    w.set_wear_skills(Skills { troubleshooting: 40.0, ..Skills::default() }, 0, WearSwitches::default());
    Some((w, figures))
}

fn run(w: &mut Firearm, seconds: f32) -> Vec<FirearmEvent> {
    let mut events = Vec::new();
    for _ in 0..(seconds / FRAME).round() as usize {
        events.extend(w.step(FRAME));
    }
    events
}

/// Until an event is seen, or the time runs out: the seconds it took.
fn until(w: &mut Firearm, limit: f32, mut wanted: impl FnMut(&FirearmEvent) -> bool) -> Option<f32> {
    let mut at = 0.0;
    while at < limit {
        at += FRAME;
        if w.step(FRAME).iter().any(&mut wanted) {
            return Some(at);
        }
    }
    None
}

#[test]
fn the_akm_s_jam_is_shot_held_looked_at_and_repaired() {
    let Some((mut w, figures)) = akm() else {
        eprintln!("no install: skipped");
        return;
    };
    assert!(figures.allow_jam && (figures.base_chance - 0.1771).abs() < 1e-6, "{figures:?}");
    run(&mut w, 1.0);
    let rounds = w.magazine;
    w.force_malfunction(Malf::Jam);
    w.set_trigger(true, true);
    let events = run(&mut w, 1.0);
    w.set_trigger(false, true);
    assert!(events.iter().any(|e| matches!(e, FirearmEvent::Shot { .. })), "the jam's shot is fired");
    assert!(events.contains(&FirearmEvent::Malfunctioned(Malf::Jam)));
    assert!(events.iter().any(|e| matches!(e, FirearmEvent::Worn { .. })), "and wears");
    assert_eq!(w.malfunction(), Malf::Jam);
    assert!(!w.chamber && w.magazine == rounds, "the chamber left empty, the magazine untouched");
    run(&mut w, 0.5);
    assert!(w.is_idling(), "at rest with the malfunction held");
    // The trigger clicks and reminds.
    w.set_trigger(true, true);
    let events = run(&mut w, 0.3);
    w.set_trigger(false, true);
    assert!(!events.iter().any(|e| matches!(e, FirearmEvent::Shot { .. })));
    assert!(events.iter().any(|e| matches!(e, FirearmEvent::MalfunctionReminder { type_known: false, .. })));
    run(&mut w, 0.5);
    // The chamber's check reminds while its kind is unknown; the look tells it.
    w.check_chamber();
    assert!(w.is_idling() && w.malfunction() == Malf::Jam);
    assert!(w.examine());
    let looked = until(&mut w, 3.0, |e| matches!(e, FirearmEvent::MalfunctionExamined { malf: Malf::Jam, first: true })).expect("the look tells the kind");
    eprintln!("the look: {looked:.3} s");
    assert!((looked - 0.825).abs() < 0.12, "{looked}");
    run(&mut w, 0.3);
    // The repair, at Troubleshooting 40's speed: the clip's mark at 0.852 of
    // 1.8 s over 1.2 (recorded: the operation 1.284 s, the round chambered
    // 1.059 s in).
    w.check_chamber();
    assert_eq!(w.malfunction(), Malf::None, "put right at once");
    let mut at = 0.0;
    let (mut ejected, mut chambered, mut done) = (None, None, None);
    while at < 5.0 && done.is_none() {
        at += FRAME;
        let before = w.chamber;
        for event in w.step(FRAME) {
            match event {
                FirearmEvent::ShellEject => ejected = Some(at),
                FirearmEvent::MalfunctionRepaired(Malf::Jam) => done = Some(at),
                _ => {}
            }
        }
        if !before && w.chamber && chambered.is_none() {
            chambered = Some(at);
        }
    }
    eprintln!("the repair: the case out {ejected:?}, chambered {chambered:?}, over {done:?}");
    let done = done.expect("repaired");
    assert!((done - 1.53 / 1.2).abs() < 0.12, "{done}");
    assert!(chambered.is_some_and(|c| (c - 0.702 * 1.8 / 1.2).abs() < 0.12), "{chambered:?}");
    assert!(w.chamber && w.magazine == rounds.map(|n| n - 1), "a round chambered from the magazine");
    // Then it fires again.
    run(&mut w, 1.0);
    w.set_trigger(true, true);
    let events = run(&mut w, 0.5);
    w.set_trigger(false, true);
    assert!(events.iter().any(|e| matches!(e, FirearmEvent::Shot { .. })));
}

#[test]
fn the_akm_s_misfire_makes_no_shot_and_its_feed_refuses_the_reload() {
    let Some((mut w, _)) = akm() else {
        eprintln!("no install: skipped");
        return;
    };
    run(&mut w, 1.0);
    // A misfire in automatic fire: no shot at all, the round lost.
    while w.mode() != FireMode::FullAuto {
        w.next_fire_mode();
        run(&mut w, 1.5);
    }
    let rounds = w.magazine;
    w.force_malfunction(Malf::Misfire);
    w.set_trigger(true, true);
    let events = run(&mut w, 0.6);
    w.set_trigger(false, true);
    assert!(!events.iter().any(|e| matches!(e, FirearmEvent::Shot { .. })), "no shot");
    assert!(events.contains(&FirearmEvent::Malfunctioned(Malf::Misfire)));
    assert!(!w.chamber && w.magazine == rounds, "the round lost, the magazine untouched");
    assert_eq!(w.malfunction(), Malf::Misfire);
    // A misfire is put right by the chamber's repair once looked at.
    run(&mut w, 0.5);
    assert!(w.examine());
    until(&mut w, 3.0, |e| matches!(e, FirearmEvent::MalfunctionExamined { .. })).expect("looked at");
    run(&mut w, 0.3);
    w.check_chamber();
    let fixed = until(&mut w, 5.0, |e| *e == FirearmEvent::MalfunctionRepaired(Malf::Misfire)).expect("repaired");
    eprintln!("the misfire's repair: {fixed:.3} s");
    assert!((fixed - 1.25 / 1.2).abs() < 0.12, "{fixed}");
    run(&mut w, 1.0);
    // A failure to feed: the shot fires, the magazine's top is lost, the
    // reload is refused, the repair throws the round out.
    let rounds = w.magazine;
    w.force_malfunction(Malf::Feed);
    w.set_trigger(true, true);
    let events = run(&mut w, 0.6);
    w.set_trigger(false, true);
    assert!(events.iter().any(|e| matches!(e, FirearmEvent::Shot { .. })));
    assert_eq!(w.malfunction(), Malf::Feed);
    assert_eq!(w.magazine, rounds.map(|n| n - 1), "the top round out of the weapon");
    run(&mut w, 0.5);
    w.reload();
    let events = run(&mut w, 0.3);
    assert!(w.is_idling() && events.iter().any(|e| matches!(e, FirearmEvent::MalfunctionReminder { .. })), "no reload with a failure to feed");
    assert!(w.examine());
    until(&mut w, 3.0, |e| matches!(e, FirearmEvent::MalfunctionExamined { .. })).expect("looked at");
    run(&mut w, 0.3);
    w.check_chamber();
    let mut thrown = false;
    let fixed = until(&mut w, 6.0, |e| {
        if *e == FirearmEvent::FedRoundThrown {
            thrown = true;
        }
        *e == FirearmEvent::MalfunctionRepaired(Malf::Feed)
    });
    assert!(thrown, "the fed round thrown out");
    eprintln!("the feed's repair: {fixed:?}");
    assert!(fixed.is_some_and(|f| (f - 2.74 / 1.2).abs() < 0.15), "{fixed:?}");
}
