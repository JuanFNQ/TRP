//! The M3 pumped and filled a shell at a time, and its folding stock.

use super::*;

/// The M3 Super 90 on the install's own controller (`docs/spec/shotgun.md`):
/// a pumped shot and a self-loading one, the tube filled a shell at a time
/// and cut short, what the reload key is refused, the bolt caught open on
/// the last shell and what that does to the fire mode, the first shell
/// into an empty weapon in each mode, the look into the chamber, and the
/// draw that works the action. The sixteenth recording has the shots, the
/// fillings, the mode changes and the looks frame for frame; what is only
/// here is said where it is tried.
#[test]
fn the_m3_is_pumped_and_filled_a_shell_at_a_time() {
    use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
    use trp_sim::{FireMode, Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, "6259b864ebedf17603599e88").expect("the M3 Super 90");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    println!(
        "the M3: modes {:?}, a tube of {}, {} a minute for single shots, bolt catch {}, no mode change on it {}, filled a shell at a time {}",
        settings.modes, settings.magazine_capacity, settings.single_fire_rate, settings.bolt_catch, settings.no_fire_mode_on_bolt_catch, settings.internal_magazine
    );
    assert_eq!(settings.modes, [FireMode::Single, FireMode::SemiAuto]);
    assert_eq!(settings.magazine_capacity, 5);
    assert!(settings.internal_magazine && settings.bolt_catch && settings.no_fire_mode_on_bolt_catch);

    const FRAME: f32 = 1.0 / 120.0;
    let run = |w: &mut Firearm, seconds: f32| -> (Vec<(String, f32)>, Vec<(FirearmEvent, f32)>) {
        let name = |w: &Firearm| {
            let (state, _, next) = w.animator().state();
            next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
        };
        let (mut entered, mut said, mut last) = (Vec::new(), Vec::new(), name(w));
        for frame in 0..(seconds / FRAME).round() as usize {
            let at = (frame + 1) as f32 * FRAME;
            said.extend(w.step(FRAME).into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_))).map(|e| (e, at)));
            let now = name(w);
            if now != last {
                entered.push((now.clone(), at));
                last = now;
            }
        }
        (entered, said)
    };
    let names = |entered: &[(String, f32)]| entered.iter().map(|entry| entry.0.clone()).collect::<Vec<_>>();
    let shots = |said: &[(FirearmEvent, f32)]| said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count();
    let taken = |said: &[(FirearmEvent, f32)]| said.iter().filter_map(|(event, _)| if let FirearmEvent::RoundsTaken(n) = event { Some(*n) } else { None }).sum::<u32>();
    let tap = |w: &mut Firearm| {
        w.set_trigger(true, true);
        w.step(FRAME);
        w.set_trigger(false, true);
    };
    let at_rest = |tube: u32, chamber: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.magazine, w.chamber) = (Some(tube), chamber);
        w.start_drawn();
        w
    };

    // A pumped shot: the fire state from a fifth in, the action worked by
    // the same state, and a press in the middle of it lost.
    let mut w = at_rest(5, true);
    tap(&mut w);
    assert_eq!(w.animator().state().0, "Hands.FIRE");
    let (_, early) = run(&mut w, 0.25);
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (entered, said) = run(&mut w, 1.5);
    println!("a pumped shot, and a press 0.26 s into it: {entered:?}; {} shot(s)", shots(&early) + shots(&said));
    assert_eq!(shots(&early) + shots(&said), 1, "the weapon queues no second shot");
    assert_eq!((w.magazine, w.chamber), (Some(4), true));
    assert!(w.is_idle());

    // The trigger held: one shot, as in the game (recorded: 72).
    let mut w = at_rest(5, true);
    w.set_trigger(true, true);
    let (_, said) = run(&mut w, 2.0);
    assert_eq!(shots(&said), 1);

    // Topped up with a shell chambered: two shells into a tube of three,
    // the count the weapon's only when the filling is over.
    let mut w = at_rest(3, true);
    w.reload_shells(40);
    assert!(w.reloading());
    let (entered, said) = run(&mut w, 1.5);
    assert_eq!(w.magazine, Some(3), "the tube's count is the weapon's when the filling ends");
    let (more, said_more) = run(&mut w, 4.0);
    println!("topped up from 3 + 1: {entered:?} then {more:?}");
    assert_eq!(names(&entered), ["RELOAD START", "RELOAD IN"]);
    assert_eq!(names(&more), ["RELOAD END", "IDLE"]);
    assert_eq!((w.magazine, w.chamber, taken(&said) + taken(&said_more)), (Some(5), true, 2));
    assert!(!w.reloading() && w.is_idle());

    // A full tube under an empty chamber: the key does nothing (from the
    // code alone: no recording has it).
    let mut w = at_rest(5, false);
    w.reload_shells(40);
    assert!(!w.reloading(), "a full tube is not filled, whatever the chamber");
    // And with nothing within reach it does nothing either.
    let mut w = at_rest(2, true);
    w.reload_shells(0);
    assert!(!w.reloading());
    // With one shell within reach, one goes in.
    w.reload_shells(1);
    run(&mut w, 4.0);
    assert_eq!(w.magazine, Some(3));

    // Cut short by the trigger: the shell on its way in still goes in, and
    // nothing fires.
    let mut w = at_rest(0, true);
    w.reload_shells(40);
    run(&mut w, 2.0);
    tap(&mut w);
    let (entered, said) = run(&mut w, 3.0);
    println!("cut short 2 s in: {entered:?}, {:?} in the tube", w.magazine);
    assert_eq!(shots(&said), 0);
    assert!(w.magazine.is_some_and(|tube| tube >= 1 && tube < 5), "{:?}", w.magazine);
    assert!(w.is_idle());

    // From empty, pumped: the first shell goes into the chamber by a state
    // of its own, and the weapon has it from the key on.
    let mut w = at_rest(0, false);
    w.reload_shells(40);
    assert!(w.chamber, "an empty chamber is given its round as the filling begins");
    let (entered, said) = run(&mut w, 10.0);
    println!("from empty, pumped: {entered:?}");
    assert_eq!(names(&entered).first().map(String::as_str), Some("PA RELOAD FIRST"));
    assert_eq!((w.magazine, w.chamber, taken(&said)), (Some(5), true, 6), "one for the chamber as it began, five into the tube");

    // The self-loading mode: a change that takes its time, a shot by its
    // own state, and the bolt caught open on the last shell.
    let mut w = at_rest(1, true);
    assert!(w.set_fire_mode(FireMode::SemiAuto));
    let (entered, _) = run(&mut w, 2.0);
    println!("to the self-loading mode: {entered:?}");
    assert_eq!(names(&entered), ["TO SA", "IDLE"]);
    tap(&mut w);
    assert_eq!(w.animator().state().0, "Hands.SA FIRE", "the self-loading shot has a state of its own");
    let (entered, said) = run(&mut w, 0.6);
    assert_eq!((names(&entered), shots(&said)), (vec!["IDLE".to_owned()], 1));
    assert!(!w.bolt_caught(), "a shell was left to load");
    tap(&mut w);
    let (_, said) = run(&mut w, 0.6);
    assert_eq!(shots(&said), 1);
    assert!(w.bolt_caught() && !w.chamber && w.magazine == Some(0), "the last shell leaves the bolt caught open");
    // The mode is not changed while it is (the item's own flag).
    assert!(!w.set_fire_mode(FireMode::Single));
    assert_eq!(w.mode(), FireMode::SemiAuto);
    // The first shell goes in past the open bolt, which is let go.
    w.reload_shells(40);
    let (entered, _) = run(&mut w, 10.0);
    println!("from empty, self-loading, the bolt caught: {entered:?}");
    assert_eq!(names(&entered).first().map(String::as_str), Some("SA AMMO IN FIRST CATCH"));
    assert!(!w.bolt_caught() && w.chamber && w.magazine == Some(5));
    assert!(w.set_fire_mode(FireMode::Single));
    let (entered, _) = run(&mut w, 2.0);
    assert_eq!(names(&entered), ["TO PA", "IDLE"]);

    // The look into the chamber, by the mode; not with the trigger held.
    let mut w = at_rest(5, true);
    w.check_chamber();
    assert!(w.checking());
    let (entered, _) = run(&mut w, 3.0);
    assert_eq!(names(&entered), ["PA CHECK CHAMBER", "IDLE"]);
    assert!(!w.checking());

    // A weapon with shells in its tube and none chambered works its action
    // as it is drawn (a weapon found in a container: `SpawnOperation`),
    // by the state of its mode.
    let mut w = Firearm::new(settings.clone(), &data);
    (w.magazine, w.chamber) = (Some(5), false);
    w.draw();
    let (entered, said) = run(&mut w, 2.5);
    let appeared = said.iter().find(|(event, _)| *event == FirearmEvent::Appeared).map(|(_, at)| *at).expect("it appears");
    println!("drawn with an empty chamber: {entered:?}; appeared {appeared:.3} s after the draw was asked for");
    assert_eq!(names(&entered), ["PA OUT TO IDLE ARM", "IDLE"]);
    assert_eq!((w.magazine, w.chamber), (Some(4), true));
    let mut w = Firearm::new(settings.clone(), &data);
    (w.magazine, w.chamber) = (Some(5), false);
    w.start_in_mode(FireMode::SemiAuto);
    w.draw();
    let (entered, _) = run(&mut w, 2.5);
    assert_eq!(names(&entered), ["SA OUT TO IDLE ARM", "IDLE"]);
}

/// The M3's stock folds and unfolds by its animator's own two states
/// (`docs/spec/shotgun.md`, "The stock"). From the game's code and the
/// install; no recording has a fold.
#[test]
fn the_m3_s_stock_folds() {
    use trp_sim::{Firearm, FirearmEvent};
    let Some((weapon, data, settings, db, _)) = weapon_of("6259b864ebedf17603599e88") else { return };
    let fold = weapon.fold.expect("the stock preset's stock folds");
    println!(
        "the M3: {} cells wide and {} folded; the parts' recoil {:+.2} and {:+.2} folded; held closer {} and {} folded",
        weapon.cell_width, fold.cell_width, weapon.recoil_delta, fold.recoil_delta, weapon.held_closer, fold.held_closer
    );
    assert!(settings.foldable);
    assert_eq!((weapon.cell_width, fold.cell_width), (5, 4));
    // The stock's own share, 21 hundredths, is what the fold takes away.
    assert!((fold.recoil_delta - weapon.recoil_delta - 0.21).abs() < 1e-4, "{} {}", fold.recoil_delta, weapon.recoil_delta);
    assert!(!weapon.held_closer && fold.held_closer);
    // The AKM's stock does not fold.
    let akm = trp_data::weapons::weapon::assemble(&mut Assets::new(crate::fixture::install().ok_or(()).unwrap()).unwrap(), &db, "59d6088586f774275f37482f").unwrap();
    assert!(akm.fold.is_none() && !akm.held_closer);

    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    assert!(!w.stock_folded());
    // Not in a look at the weapon.
    w.check_magazine();
    assert!(!w.fold_stock());
    run_weapon(&mut w, 3.0);
    assert!(w.is_idle());

    assert!(w.fold_stock());
    assert!(w.occupied() && w.hands_busy());
    // The trigger does nothing meanwhile.
    w.set_trigger(true, true);
    let (entered, said) = run_weapon(&mut w, 2.0);
    w.set_trigger(false, true);
    println!("folding: {entered:?} {said:?}");
    assert_eq!(state_names(&entered), ["STOCK FOLD", "IDLE"]);
    // The stock stands folded 0.694 of the way through the state's 1.333 s.
    let folded = said.iter().find(|(event, _)| *event == FirearmEvent::Folded(true)).expect("folded").1;
    assert!((folded - 0.925).abs() < 0.03, "{folded}");
    assert!((entered[1].1 - 1.34).abs() < 0.03, "{:?}", entered);
    assert!(w.stock_folded() && w.is_idle() && !w.hands_busy());
    assert!(!said.iter().any(|(event, _)| matches!(event, FirearmEvent::Shot { .. })));
    assert!(w.animator().flag("StockFolded"));

    // It shoots folded, and unfolds again; put away in the unfolding, it
    // goes when the stock is out.
    w.set_trigger(true, true);
    let (_, said) = run_weapon(&mut w, 1.0);
    w.set_trigger(false, true);
    assert_eq!(said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count(), 1);
    assert!(w.fold_stock());
    w.hide(false);
    let (entered, said) = run_weapon(&mut w, 3.0);
    println!("unfolding, put away meanwhile: {entered:?} {said:?}");
    let unfolded = said.iter().find(|(event, _)| *event == FirearmEvent::Folded(false)).expect("unfolded").1;
    assert!(unfolded < 0.1, "{unfolded}");
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::Hidden));
    assert!(!w.stock_folded() && !w.in_hand());

    // A weapon put away folded is drawn folded.
    let mut w = Firearm::new(settings, &data);
    w.start_folded(true);
    w.draw();
    run_weapon(&mut w, 2.0);
    assert!(w.is_idle() && w.stock_folded() && w.animator().flag("StockFolded"));
}
