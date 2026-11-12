//! The Model 700's bolt and magazine and the Five-seveN's slide lock.

use super::*;

/// The Remington Model 700: the bolt worked when the trigger is let go,
/// and its magazine filled a round at a time with the bolt open
/// (`docs/spec/bolt-action.md`). From the game's code and the install; no
/// recording has the weapon.
#[test]
fn the_model_700_s_bolt_is_worked_by_hand() {
    use trp_sim::{FireMode, Firearm, FirearmEvent};
    let Some((weapon, data, settings, ..)) = weapon_of("5bfea6e90db834001b7347f3") else { return };
    println!(
        "the Model 700: {:.2} kg, {} cells, modes {:?}, a magazine of {}, bolt action {}, filled with the bolt open {}, {} a minute",
        weapon.total_weight, weapon.cell_width, settings.modes, settings.magazine_capacity, settings.bolt_action, settings.open_bolt_reload, settings.single_fire_rate
    );
    assert_eq!(settings.modes, [FireMode::Single]);
    assert_eq!(settings.magazine_capacity, 5);
    assert!(settings.bolt_action && settings.internal_magazine && settings.open_bolt_reload && !settings.foldable);
    const FRAME: f32 = 1.0 / 120.0;
    let at_rest = |magazine: u32, chamber: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.magazine, w.chamber) = (Some(magazine), chamber);
        w.start_drawn();
        w
    };
    let shots = |said: &[(FirearmEvent, f32)]| said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count();

    // The trigger held: the shot, and the bolt left alone for as long as
    // it is held.
    let mut w = at_rest(5, true);
    w.set_trigger(true, true);
    let (entered, said) = run_weapon(&mut w, 2.0);
    println!("held: {entered:?}");
    assert_eq!(shots(&said), 1);
    assert_eq!(w.animator().state().0, "Hands.FIRE");
    assert!(w.occupied() && state_names(&entered) == ["FIRE"]);
    // Let go: the bolt is worked, 1.767 s, and the next round is in.
    w.set_trigger(false, true);
    let (entered, said) = run_weapon(&mut w, 2.5);
    println!("let go: {entered:?} {said:?}");
    assert_eq!(state_names(&entered), ["FIRE RELOAD", "IDLE"]);
    assert!((entered[1].1 - entered[0].1 - 1.767).abs() < 0.03, "{entered:?}");
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::ShellEject));
    assert!(w.is_idle() && w.chamber && w.magazine == Some(4));

    // A tap: the bolt follows the shot at once (the fire state is 0.167 s,
    // entered a fifth in).
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (entered, said) = run_weapon(&mut w, 2.5);
    println!("a tap: {entered:?}");
    assert_eq!((shots(&said), state_names(&entered)), (1, vec!["FIRE RELOAD", "IDLE"]));
    assert!((entered[0].1 - 0.13).abs() < 0.03, "{entered:?}");
    // A second press while the bolt is worked is lost.
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    run_weapon(&mut w, 0.5);
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (_, said) = run_weapon(&mut w, 2.5);
    assert_eq!(shots(&said), 0);
    assert!(w.is_idle() && w.chamber && w.magazine == Some(2));

    // The magazine filled: the bolt opened, which throws the chambered
    // round out, a round a loop, the bolt shut on the first of them.
    let mut w = at_rest(2, true);
    w.reload_shells(20);
    assert!(w.reloading() && !w.chamber, "the chambered round has left the weapon at the key");
    let (entered, said) = run_weapon(&mut w, 9.0);
    println!("filled: {entered:?}\n  {said:?}");
    assert_eq!(state_names(&entered), ["RELOAD START", "RELOAD IN", "RELOAD CLOSE CHAMBER", "IDLE"]);
    assert_eq!(said.iter().filter(|(event, _)| *event == FirearmEvent::RoundEjected).count(), 1);
    let taken: u32 = said.iter().filter_map(|(event, _)| if let FirearmEvent::RoundsTaken(n) = event { Some(*n) } else { None }).sum();
    assert_eq!(taken, 3);
    // Five went into the magazine and the bolt took the first of them.
    assert!(w.is_idle() && w.chamber && w.magazine == Some(4), "{:?} {}", w.magazine, w.chamber);
    assert!(!w.bolt_caught());
    // Three loops of 1.533 s between the opening (1.167 s) and the closing.
    let loops = entered[2].1 - entered[1].1;
    assert!((loops - 3.0 * 1.533).abs() < 0.1, "{loops}");

    // A full magazine is not filled, whatever the chamber; and the round
    // chambered is then kept.
    let mut w = at_rest(5, true);
    w.reload_shells(20);
    assert!(!w.reloading() && w.chamber);

    // Cut short by the trigger: not before the first round is in, and then
    // the round on its way still goes in, the bolt is shut, nothing fires.
    let mut w = at_rest(0, false);
    w.reload_shells(20);
    run_weapon(&mut w, 1.0);
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (_, early) = run_weapon(&mut w, 1.5);
    w.set_trigger(true, true);
    w.step(FRAME);
    w.set_trigger(false, true);
    let (entered, said) = run_weapon(&mut w, 6.0);
    println!("cut short: {entered:?}");
    assert_eq!(shots(&early) + shots(&said), 0);
    assert!(w.is_idle() && w.chamber);
    assert_eq!(w.magazine, Some(1), "two went in and the bolt took one");

    // Found with rounds in the magazine and none chambered, the draw works
    // the bolt.
    let mut w = Firearm::new(settings.clone(), &data);
    (w.magazine, w.chamber) = (Some(5), false);
    w.draw();
    let (entered, said) = run_weapon(&mut w, 3.0);
    println!("first draw: {entered:?}");
    assert_eq!(state_names(&entered), ["OUT TO IDLE ARM", "IDLE"]);
    let appeared = said.iter().find(|(event, _)| *event == FirearmEvent::Appeared).expect("appeared").1;
    assert!((appeared - 2.2).abs() < 0.06, "{appeared}");
    assert!(w.chamber && w.magazine == Some(4));
}

/// The Model 700's stock magazine comes out (`ReloadMagType`
/// `ExternalMagazine`) though the rifle's own mode is `InternalMagazine`:
/// the game's `Weapon.ReloadMode` is `ExternalMagazineWithInternalReloadSupport`,
/// and its look at the magazine tells the animator so (`Idling.CheckAmmo`),
/// which plays `CHECK MAG` (the magazine out and back, with its sounds)
/// rather than `CHECK` (the bolt, whose two sounds the rifle has none of).
/// The M700's magazine check once played no sound.
#[test]
fn the_model_700_s_magazine_is_looked_at_out_of_the_rifle() {
    use trp_sim::{Firearm, FirearmEvent};
    let Some((weapon, data, settings, ..)) = weapon_of("5bfea6e90db834001b7347f3") else { return };
    assert_eq!(weapon.magazine_external, Some(true));
    assert!(settings.internal_magazine && settings.magazine_external);
    const FRAME: f32 = 1.0 / 120.0;
    let look = |w: &mut Firearm| {
        assert!(w.check_magazine());
        let (mut entered, mut sounds) = (Vec::new(), Vec::new());
        for _ in 0..(3.5 / FRAME) as usize {
            for event in w.step(FRAME) {
                if let FirearmEvent::Sound(name) = event {
                    sounds.push(name);
                }
            }
            let state = w.animator().state().0.trim_start_matches("Hands.").to_owned();
            if entered.last() != Some(&state) {
                entered.push(state);
            }
        }
        (entered, sounds)
    };
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    let (entered, sounds) = look(&mut w);
    println!("looked at: {entered:?} {sounds:?}");
    assert_eq!(entered, ["CHECK MAG", "IDLE"]);
    assert_eq!(sounds, ["HandOff", "MagRelease", "MagOut", "MagIn", "HandOn"]);
    assert!(w.is_idle() && w.magazine == Some(5));

    // After a filling, which tells the animator the magazine stays in, the
    // look is still the one out of the rifle.
    w.magazine = Some(2);
    w.reload_shells(10);
    let (filled, _) = run_weapon(&mut w, 12.0);
    assert_eq!(state_names(&filled)[0], "RELOAD START");
    assert!(w.is_idle() && !w.animator().flag("IsExternalMag"));
    let (entered, _) = look(&mut w);
    assert_eq!(entered, ["CHECK MAG", "IDLE"]);

    // A magazine that stays in (none fits this rifle; told so by the host)
    // gets the bolt's look.
    w.set_magazine_external(false);
    let (entered, sounds) = look(&mut w);
    assert_eq!(entered, ["CHECK", "IDLE"]);
    assert_eq!(sounds, ["HandOff", "BoltOut", "BoltIn", "HandOn"]);
}

/// The Model 700 as the game's `ExternalMagazineWithInternalReloadSupport`
/// (bolt-action spec): its magazine is changed where one is within reach,
/// and filled a round at a time only where none is; the menu's Reload, Load
/// and Unload take it as a magazine weapon while one is in; its chamber is
/// not loaded nor emptied from the inventory with a magazine in. The swap's
/// states are the game's of `courses1\4-model-700` (12.828 s from two and
/// one, 36.887 s from empty), where it changed the magazine for the vest's.
#[test]
fn the_model_700_s_magazine_is_changed_where_one_is_within_reach() {
    use trp_sim::items::inventory::{reload_mode, Item, MAGAZINE_SLOT, MIXED_RELOAD};
    use trp_sim::items::item_menu::{lines, Facts, Line};
    use trp_sim::Firearm;
    let Some((_, data, settings, db, ..)) = weapon_of("5bfea6e90db834001b7347f3") else { return };
    // The stock builds: the Model 700 and the AKM change magazines, the M3
    // fills its tube.
    assert!(db.changes_magazines("5bfea6e90db834001b7347f3") && db.changes_magazines("59d6088586f774275f37482f"));
    assert!(!db.changes_magazines("6259b864ebedf17603599e88"));

    // The item's mode, with its magazine and without.
    let (mut rifle, _) = trp_data::character::inventory::Loadout::of_preset(db, "5bfea6e90db834001b7347f3").and_then(|list| Item::of_list(db, &list)).expect("the rifle");
    assert_eq!(reload_mode(&rifle), MIXED_RELOAD);
    let facts = Facts { own: true, ..Facts::default() };
    let can = |item: &Item, line: Line| lines(item, &facts).into_iter().find(|(shown, _)| *shown == line).map(|(_, can)| can);
    assert_eq!(can(&rifle, Line::Unload), Some(Ok(())));
    assert_eq!(can(&rifle, Line::Reload), Some(Ok(())));
    let slot = rifle.slot_index(MAGAZINE_SLOT).expect("its magazine slot");
    rifle.slots[slot] = None;
    assert_eq!(reload_mode(&rifle), "InternalMagazine");
    assert_eq!(can(&rifle, Line::Load), Some(Err("You can't do this to weapon with internal magazine")));

    let at_rest = |magazine: Option<u32>, chamber: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.magazine, w.chamber) = (magazine, chamber);
        w.start_drawn();
        w
    };
    // The states as the recording names them: each once its blend in is
    // over (the animator's current state), from the first's start.
    let run = |w: &mut Firearm, seconds: f32| {
        const FRAME: f32 = 1.0 / 120.0;
        let mut entered: Vec<(String, f32)> = Vec::new();
        for frame in 0..(seconds / FRAME) as usize {
            w.step(FRAME);
            let state = w.animator().state().0.trim_start_matches("Hands.").to_owned();
            if entered.last().map(|(last, _)| last) != Some(&state) {
                entered.push((state, frame as f32 * FRAME));
            }
        }
        entered
    };
    let from = |entered: &[(String, f32)]| entered.iter().map(|(state, at)| (state.clone(), ((at - entered[0].1) * 1000.0).round() / 1000.0)).collect::<Vec<_>>();

    // Two and one: the magazine changed for a full one, as the game's.
    let mut w = at_rest(Some(2), true);
    assert!(w.magazine_comes_out() && w.reload_looks_for_magazine());
    assert!(!w.can_discharge(), "not with the magazine in");
    w.reload_with(5);
    let entered = run(&mut w, 5.0);
    println!("changed: {:?}", from(&entered));
    assert_eq!(state_names(&entered), ["RELOAD START MAG", "RELOAD OUT MAG", "RELOAD IN MAG", "RELOAD END MAG", "IDLE"]);
    for ((_, at), recorded) in from(&entered).iter().zip([0.0, 0.433, 1.997, 3.456, 3.572]) {
        assert!((at - recorded).abs() < 0.02, "{at} {recorded}");
    }
    assert!(w.is_idle() && w.magazine == Some(5) && w.chamber);

    // From empty, the bolt opened for it and closed on the new one's first.
    let mut w = at_rest(Some(0), false);
    assert!(!w.can_load_chamber(), "not with the magazine in");
    w.reload_with(5);
    let entered = run(&mut w, 7.0);
    println!("from empty: {:?}", from(&entered));
    assert_eq!(state_names(&entered), ["OPEN CHAMBER MAG", "RELOAD START MAG", "RELOAD OUT MAG", "RELOAD IN MAG", "RELOAD END MAG", "RELOAD CLOSE CHAMBER MAG", "IDLE"]);
    for ((_, at), recorded) in from(&entered).iter().zip([0.0, 1.167, 1.484, 3.051, 4.502, 4.660, 5.756]) {
        assert!((at - recorded).abs() < 0.02, "{at} {recorded}");
    }
    assert!(w.is_idle() && w.magazine == Some(4) && w.chamber);

    // With no magazine fitted the key still looks for one first, and a
    // round may go into the chamber by hand.
    let w = at_rest(None, false);
    assert!(!w.magazine_comes_out() && w.reload_looks_for_magazine());
    assert!(w.can_load_chamber());
    // A magazine that stays in (none fits this rifle) is filled instead.
    let mut w = at_rest(Some(2), true);
    w.set_magazine_external(false);
    assert!(!w.magazine_comes_out() && !w.reload_looks_for_magazine());
}

/// The FN Five-seveN: a pistol with a magazine and a slide that locks
/// back on the last round (`docs/spec/pistol.md`). From the game's code
/// and the install; no recording has the weapon.
#[test]
fn the_five_seven_s_slide_locks_back() {
    use trp_sim::{FireMode, Firearm, FirearmEvent};
    let Some((weapon, data, settings, db, _)) = weapon_of("5d3eb3b0a4b93615055e84d2") else { return };
    println!(
        "the Five-seveN: {:.2} kg, {} cells, modes {:?}, a magazine of {}, bolt catch {}, a second shot queued {}, {} a minute",
        weapon.total_weight, weapon.cell_width, settings.modes, settings.magazine_capacity, settings.bolt_catch, settings.can_queue_second_shot, settings.single_fire_rate
    );
    assert_eq!(settings.modes, [FireMode::Single]);
    assert_eq!(settings.magazine_capacity, 20);
    assert!(settings.bolt_catch && settings.can_queue_second_shot && !settings.internal_magazine && !settings.bolt_action);
    assert!(db.is_a(db.item(&weapon.template).unwrap(), "Pistol"));
    const FRAME: f32 = 1.0 / 120.0;
    let at_rest = |magazine: u32, chamber: bool| {
        let mut w = Firearm::new(settings.clone(), &data);
        (w.magazine, w.chamber) = (Some(magazine), chamber);
        w.start_drawn();
        w
    };
    let shots = |said: &[(FirearmEvent, f32)]| said.iter().filter(|(event, _)| matches!(event, FirearmEvent::Shot { .. })).count();
    let tap = |w: &mut Firearm| {
        w.set_trigger(true, true);
        w.step(FRAME);
        w.set_trigger(false, true);
    };

    // One shot a press, however long it is held; a press during the shot
    // is remembered and fired after it.
    let mut w = at_rest(20, true);
    w.set_trigger(true, true);
    let (_, said) = run_weapon(&mut w, 1.0);
    w.set_trigger(false, true);
    assert_eq!(shots(&said), 1);
    tap(&mut w);
    let (_, first) = run_weapon(&mut w, 0.05);
    tap(&mut w);
    let (_, said) = run_weapon(&mut w, 1.0);
    assert_eq!((shots(&first), shots(&said)), (1, 1), "the one in hand and the one queued");
    assert!(w.is_idle() && w.magazine == Some(17) && !w.bolt_caught());

    // The last round: the slide stays back, and a press on it plays the
    // dry press of a caught slide.
    let mut w = at_rest(0, true);
    tap(&mut w);
    let (entered, said) = run_weapon(&mut w, 1.0);
    println!("the last round: {entered:?}");
    assert_eq!(shots(&said), 1);
    assert!(w.is_idle() && !w.chamber && w.bolt_caught());
    w.set_trigger(true, true);
    let (entered, _) = run_weapon(&mut w, 0.5);
    w.set_trigger(false, true);
    assert_eq!(state_names(&entered), ["DRY FIRE CATCH"]);
    run_weapon(&mut w, 1.0);

    // Reloaded from there: the magazine changed, the slide let go on the
    // first round of the new one.
    w.reload();
    let (entered, said) = run_weapon(&mut w, 8.0);
    println!("reloaded with the slide back: {entered:?}");
    assert_eq!(state_names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD CATCH END TO IDLE", "IDLE"]);
    assert!(said.iter().any(|(event, _)| *event == FirearmEvent::MagazineIn));
    assert!(w.is_idle() && w.chamber && w.magazine == Some(19) && !w.bolt_caught());
    let whole = entered.last().unwrap().1;
    println!("  at rest {whole:.2} s after the key");

    // With a round chambered the magazine is only changed.
    let mut w = at_rest(3, true);
    w.reload();
    let (entered, _) = run_weapon(&mut w, 8.0);
    println!("reloaded with a round chambered: {entered:?}");
    assert_eq!(state_names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
    assert!(w.is_idle() && w.chamber && w.magazine == Some(20) && !w.bolt_caught());

    // Put away with the slide back and drawn again, the slide is forward.
    let mut w = at_rest(0, true);
    tap(&mut w);
    run_weapon(&mut w, 1.0);
    assert!(w.bolt_caught());
    w.hide(false);
    run_weapon(&mut w, 2.0);
    w.draw();
    let (entered, _) = run_weapon(&mut w, 2.0);
    assert_eq!(state_names(&entered), ["OUT TO IDLE", "IDLE"]);
    assert!(w.is_idle() && !w.bolt_caught());

    // Found with a full magazine and none chambered: the draw racks it.
    let mut w = Firearm::new(settings, &data);
    (w.magazine, w.chamber) = (Some(20), false);
    w.draw();
    let (entered, said) = run_weapon(&mut w, 3.0);
    assert_eq!(state_names(&entered), ["OUT TO IDLE ARM", "IDLE"]);
    let appeared = said.iter().find(|(event, _)| *event == FirearmEvent::Appeared).expect("appeared").1;
    println!("first draw: appeared {appeared:.3} s after it was asked for");
    assert!(w.chamber && w.magazine == Some(19));
}
