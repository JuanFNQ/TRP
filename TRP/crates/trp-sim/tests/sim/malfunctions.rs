//! Tests of malfunctions on the install's AKM, each given outright on its shot.
//! After `docs/spec/malfunctions.md`: the jam's shot and its held state, the
//! trigger that only clicks, the look that tells its kind, the repair at the
//! recorded character's speed (Troubleshooting 40, `SpeedFix` 1.2), the
//! misfire's missing shot, the failure to feed's lost round and the reload
//! it refuses. These pass trivially, with a note, where no install is
//! configured.

use trp_data::setup::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::weapons::firearm::{Wear, WearSwitches};
use trp_sim::weapons::malfunction::{Malf, Settings, Skills, State, WeaponFigures};
use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
use trp_sim::{FireMode, Firearm, FirearmEvent};

const AKM: &str = "59d6088586f774275f37482f";
const FRAME: f32 = 1.0 / 120.0;

fn akm() -> Option<(Firearm, WeaponFigures)> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, AKM).expect("the AKM");
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

/// Another of the install's weapons in hand, with its stock parts and the
/// given malfunction already on it (as its item would keep it).
fn weapon_with(template: &str, malf: Malf) -> Option<Firearm> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, template).expect("the weapon");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    let preset = db.default_preset(template).expect("its preset");
    let parts: Vec<&str> = preset.items.iter().map(|part| part.template.as_str()).filter(|part| *part != template).collect();
    let figures = WeaponFigures::of(&db, template, &parts);
    let mut w = Firearm::new(settings, &data);
    w.start_drawn();
    let state = State { malf, known: malf != Malf::None, ..State::default() };
    w.set_wear(Wear::new(figures, Settings::from_config(&db.config).expect("malfunction settings"), 100.0, 100.0, state, 1));
    Some(w)
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
fn the_reload_key_s_last_resort_reminds_of_a_malfunction() {
    let Some((mut w, _)) = akm() else {
        eprintln!("no install: skipped");
        return;
    };
    run(&mut w, 1.0);
    // A failure to feed leaves the chamber empty; with the magazine out and
    // a round within reach, the last resort only reminds of it
    // (`LoadAmmoToChamber`: `MisfireSlideUnknown` off, `ExamineMalfunction`).
    w.force_malfunction(Malf::Feed);
    w.set_trigger(true, true);
    run(&mut w, 0.6);
    w.set_trigger(false, true);
    run(&mut w, 0.5);
    assert!(w.malfunction() == Malf::Feed && !w.chamber);
    w.magazine = None;
    assert_eq!(w.chamber_by_hand(true), trp_sim::ChamberByHand::Nothing);
    let events = run(&mut w, 0.1);
    assert!(events.iter().any(|e| matches!(e, FirearmEvent::MalfunctionReminder { malf: Malf::Feed, .. })), "reminded");
    assert!(w.is_idling() && !w.chamber);
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

/// Only a kind the weapon's template allows is given (the UZI, the PPSh,
/// the M60 and the AA-12 once locked the hands on a malfunction their animators have no state for). The install's figures:
/// those four allow none, the Mosin Infantry no jam.
#[test]
fn only_a_malfunction_the_weapon_allows_is_given() {
    let Some((mut w, figures)) = akm() else {
        eprintln!("no install: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");
    for (name, template) in [("UZI", "66992b349950f5f4cd06029f"), ("PPSh", "5ea03f7400685063ec28bfa8"), ("M60E4", "65fb023261d5829b2d090755"), ("AA-12", "66ffa9b66e19cc902401c5e8")] {
        assert!(!WeaponFigures::of(db, template, &[]).malfunctions(), "{name} allows none");
    }
    let mosin = WeaponFigures::of(db, "5bfd297f0db834001a669119", &[]);
    assert!(!mosin.allows(Malf::Jam) && mosin.allows(Malf::Misfire) && mosin.allows(Malf::Feed) && mosin.allows(Malf::SoftSlide));
    // The AKM made as one that allows no jam: refused, and the next kind
    // given stands.
    let settings = Settings::from_config(&db.config).expect("the globals' malfunction settings");
    w.set_wear(Wear::new(WeaponFigures { allow_jam: false, ..figures }, settings, 100.0, 100.0, State::default(), 1));
    run(&mut w, 1.0);
    assert!(!w.force_malfunction(Malf::Jam), "a jam refused");
    assert!(w.force_malfunction(Malf::Feed), "a failure to feed given");
    w.set_trigger(true, true);
    let events = run(&mut w, 0.6);
    w.set_trigger(false, true);
    assert!(events.contains(&FirearmEvent::Malfunctioned(Malf::Feed)), "{events:?}");
}

/// The weapon put away during a malfunction's state goes once the state is
/// over (`MalfunctionOperation.HideWeapon`, run by `SwitchToIdle`): it was
/// lost, and a weapon dropped meanwhile stayed in hand. (One asked for in
/// the shot, before its mark hands over to the malfunction, is lost, as the
/// game's `FireOperation.SwitchToMalfunction` loses it.)
#[test]
fn a_put_away_asked_for_in_a_malfunction_is_made_at_its_end() {
    let Some((mut w, _)) = akm() else {
        eprintln!("no install: skipped");
        return;
    };
    run(&mut w, 1.0);
    w.force_malfunction(Malf::Jam);
    w.set_trigger(true, true);
    until(&mut w, 1.0, |e| matches!(e, FirearmEvent::Shot { .. })).expect("the jam's shot");
    w.set_trigger(false, true);
    assert!(!w.is_idling(), "in the malfunction's state");
    w.hide(false);
    let hidden = until(&mut w, 4.0, |e| *e == FirearmEvent::Hidden).expect("put away after the state");
    eprintln!("put away {hidden:.3} s after it was asked for");
    assert_eq!(w.malfunction(), Malf::Jam, "the malfunction kept with the weapon");
}

/// The reload a round at a time (`FirearmHandsInputTranslator.ReloadWithAmmo`):
/// any malfunction refuses it and reminds, but a misfire on a weapon whose
/// bolt need not be opened, whose filling chambers a round and puts the
/// misfire right (a Mosin was once reloaded with a malfunction).
#[test]
fn a_malfunction_refuses_the_filling_but_a_misfire_with_the_bolt_shut() {
    const MOSIN: &str = "5bfd297f0db834001a669119";
    const M870: &str = "5a7828548dc32e5a9c28b516";
    let refused = |template: &str, malf: Malf| {
        let mut w = weapon_with(template, malf).expect("the weapon");
        run(&mut w, 1.0);
        w.magazine = Some(1);
        w.reload_shells(10);
        let events = run(&mut w, 0.5);
        let reminded = events.iter().any(|e| matches!(e, FirearmEvent::MalfunctionReminder { .. }));
        let filled = events.iter().any(|e| matches!(e, FirearmEvent::RoundsTaken(_)));
        reminded && !filled && w.is_idling()
    };
    if weapon_with(MOSIN, Malf::None).is_none() {
        eprintln!("no install: skipped");
        return;
    }
    // The Mosin's bolt is opened to fill it: even a misfire refuses.
    assert!(refused(MOSIN, Malf::Feed), "the Mosin with a failure to feed");
    assert!(refused(MOSIN, Malf::Misfire), "the Mosin with a misfire");
    assert!(refused(M870, Malf::Jam), "the M870 with a jam");
    // The M870's is not: a misfire lets the filling go, the misfire put
    // right at the chamber's mark.
    let mut w = weapon_with(M870, Malf::None).expect("the M870");
    run(&mut w, 1.0);
    assert!(w.force_malfunction(Malf::Misfire));
    w.set_trigger(true, true);
    run(&mut w, 0.3);
    w.set_trigger(false, true);
    run(&mut w, 1.0);
    assert!(w.malfunction() == Malf::Misfire && !w.chamber && w.is_idling(), "misfired, at rest");
    // Room in the tube: a full one under an empty chamber is not filled.
    w.magazine = Some(1);
    w.reload_shells(10);
    let cleared = until(&mut w, 4.0, |e| *e == FirearmEvent::MalfunctionCleared(Malf::Misfire)).expect("put right by the chambered round");
    eprintln!("the M870's misfire put right {cleared:.3} s into the filling");
    assert!(w.chamber && w.malfunction() == Malf::None);
    // A barrel weapon's template allows a misfire its shot never rolls.
    let mp18 = weapon_with("61f7c9e189e6fb1a5e3ea78d", Malf::None).expect("the MP-18");
    assert!(!mp18.malfunction_allowed(Malf::Misfire));
}

/// A repair buff in hand (`Weapon.OnShot`'s `TryDisableComponent`, read
/// from code; no buffed weapon recorded): the AKM from 35 at its recorded
/// 0.0185928 a shot, a buff whose threshold is 34.95 holds at 34.981 and
/// 34.963 and is gone on the third shot's wear (34.944); none of the
/// shots after brings it back. With `DURABILITY` off nothing wears and the
/// buff stays.
#[test]
fn a_repair_buff_is_gone_on_the_shot_that_wears_it_under_its_threshold() {
    use trp_sim::items::repair::{BuffKind, Rarity, RepairBuff};
    let buff = RepairBuff { rarity: Rarity::Common, kind: BuffKind::MalfunctionProtections, value: 0.95, threshold: 34.95 };
    for durability_on in [true, false] {
        let Some((mut w, _)) = akm() else {
            eprintln!("no install: skipped");
            return;
        };
        w.set_wear_skills(Skills::default(), 0, WearSwitches { malfunctions: false, durability: durability_on, ..WearSwitches::default() });
        w.set_buff(Some(buff));
        run(&mut w, 1.0);
        while w.mode() != FireMode::FullAuto {
            w.next_fire_mode();
        }
        w.set_trigger(true, true);
        let mut worn = Vec::new();
        for _ in 0..(0.6 / FRAME) as usize {
            for event in w.step(FRAME) {
                if let FirearmEvent::Worn { durability, .. } = event {
                    worn.push((durability, w.wear().and_then(|wear| wear.buff).is_some()));
                }
            }
        }
        w.set_trigger(false, true);
        eprintln!("DURABILITY {durability_on}: {worn:?}");
        assert!(worn.len() >= 4, "{worn:?}");
        if durability_on {
            for (durability, held) in &worn {
                assert_eq!(*held, *durability as f64 >= 34.95, "{worn:?}");
            }
            assert!(worn[1].1 && !worn[2].1, "gone on the third: {worn:?}");
        } else {
            assert!(worn.iter().all(|(durability, held)| *durability == 35.0 && *held), "{worn:?}");
        }
    }
}
