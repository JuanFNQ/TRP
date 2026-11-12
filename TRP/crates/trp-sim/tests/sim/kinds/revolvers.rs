//! A cylinder's quick reload, the fistful and the cylinder held and turned through shots.

use super::*;

/// A cylinder's quick reload and the fistful of a character who knows the
/// weapon (more-weapons spec, "A cylinder's quick reload, and the
/// fistful"; `ReloadCylinderMagOperation` with `_quickReload` and
/// `_highLevelMasteringFullReload`), on the install's Rhino, RSh-12 and
/// M32A1. The quick key is heard only at rest; it throws the live rounds
/// out with the cases (each as a thing) and loads every chamber, one by one
/// below mastering 1. From mastering 1 a quick reload, or one of an empty
/// cylinder, is a fistful (`RELOAD_INSERT_FISTFUL`): every round shown at
/// once and gone in at the clip's marks, cut short at the rounds in reach
/// or by the trigger (`RELOAD_INSERT_FISTFUL_ABORT`, the rounds not in taken
/// back). A plain reload of a cylinder with live rounds is the round by
/// round one whatever the mastering. Only the install's animators run by
/// the simulation: no recording has one.
#[test]
fn a_cylinder_s_quick_reload_and_the_fistful() {
    if let Some(mut w) = weapon_of(RHINO) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        for _ in 0..2 {
            press(&mut w, 2.0);
        }
        assert_eq!((shown(&w), w.magazine), ("ccRRRR".to_owned(), Some(4)));
        // Not at rest: the quick key is not heard.
        w.set_trigger(true, true);
        w.quick_reload_shells(40);
        w.set_trigger(false, true);
        assert!(!w.reloading());
        run(&mut w, 2.0);
        assert_eq!(w.magazine, Some(3));
        // Mastering 0: every case and live round out at once, then six
        // rounds one by one.
        w.quick_reload_shells(40);
        assert!(w.reloading());
        let (entered, said) = run(&mut w, 14.0);
        println!("  the Rhino's quick reload: {}; said {:?}", told(&entered), events(&said));
        assert_eq!(names(&entered), ["RELOAD START ALL", "RELOAD IN", "RELOAD END", "IDLE"]);
        assert_eq!(dropped(&said), [3, 4, 5]);
        assert_eq!(thrown_and_loaded(&said).0, (0..3).map(|chamber| (chamber, None)).collect::<Vec<_>>());
        assert!(said.iter().any(|(event, _)| *event == FirearmEvent::RoundsTaken(6)));
        assert_eq!((shown(&w), w.magazine), ("RRRRRR".to_owned(), Some(6)));
        // Full, the quick reload still begins and throws them all out.
        w.quick_reload_shells(40);
        let (_, said) = run(&mut w, 14.0);
        assert_eq!((dropped(&said).len(), w.magazine), (6, Some(6)));
    }
    if let Some(mut w) = weapon_of(RHINO) {
        w.settings.mastering = 1;
        w.found();
        w.draw();
        run(&mut w, 3.0);
        assert_eq!(w.animator().get("WeaponLevel"), 1.0);
        // A plain reload with live rounds left: round by round.
        press(&mut w, 2.0);
        w.reload_shells(40);
        let (entered, _) = run(&mut w, 10.0);
        println!("  mastering 1, one fired, reloaded: {}", told(&entered));
        assert!(!names(&entered).contains(&"RELOAD_INSERT_FISTFUL") && w.magazine == Some(6));
        // Emptied: a fistful, every round shown at its point at once.
        for _ in 0..6 {
            press(&mut w, 2.0);
        }
        w.reload_shells(40);
        let (entered, said) = run(&mut w, 12.0);
        println!("  mastering 1, from empty: {}; said {:?}", told(&entered), events(&said));
        assert_eq!(names(&entered), ["RELOAD START ALL", "RELOAD_INSERT_FISTFUL", "RELOAD END", "IDLE"]);
        let shows: Vec<f32> = said.iter().filter(|(event, _)| matches!(event, FirearmEvent::RoundShown { .. })).map(|(_, at)| *at).collect();
        assert_eq!(thrown_and_loaded(&said).1, (0..6).map(|chamber| (chamber, chamber)).collect::<Vec<_>>());
        assert!(shows.iter().all(|at| *at == shows[0]), "all at once: {shows:?}");
        assert_eq!((shown(&w), w.magazine, w.camora), ("RRRRRR".to_owned(), Some(6), 0));
        // Three rounds in reach: the fistful ends at the third.
        for _ in 0..6 {
            press(&mut w, 2.0);
        }
        w.reload_shells(3);
        let (entered, _) = run(&mut w, 12.0);
        println!("  mastering 1, three in reach: {}", told(&entered));
        assert_eq!(names(&entered), ["RELOAD START ALL", "RELOAD_INSERT_FISTFUL", "RELOAD END", "IDLE"]);
        assert_eq!((shown(&w), w.magazine), ("RRR...".to_owned(), Some(3)));
        // The quick key with live rounds: out, and a fistful.
        w.quick_reload_shells(40);
        let (entered, said) = run(&mut w, 12.0);
        println!("  mastering 1, quick: {}", told(&entered));
        assert_eq!(names(&entered), ["RELOAD START ALL", "RELOAD_INSERT_FISTFUL", "RELOAD END", "IDLE"]);
        assert_eq!((dropped(&said), w.magazine), (vec![0, 1, 2], Some(6)));
        // Cut short by the trigger after the second round: the rounds still
        // in the hand are taken back.
        for _ in 0..6 {
            press(&mut w, 2.0);
        }
        w.reload_shells(40);
        let (mut entered, _) = run(&mut w, 0.1);
        while w.animator().get("AmmoInMag") < 2.0 {
            let (more, _) = run(&mut w, FRAME);
            entered.extend(more);
        }
        w.set_trigger(true, true);
        let (more, said) = run(&mut w, 10.0);
        w.set_trigger(false, true);
        entered.extend(more);
        println!("  mastering 1, cut short: {}; said {:?}", told(&entered), events(&said));
        assert!(names(&entered).contains(&"RELOAD_INSERT_FISTFUL_ABORT"), "{}", told(&entered));
        let loaded = w.magazine.unwrap();
        assert!((2..6).contains(&loaded), "{loaded}");
        assert_eq!(shown(&w).matches('R').count(), loaded as usize, "the rounds not in are taken back: {}", shown(&w));
    }
    if let Some(mut w) = weapon_of("633ec7c2a6918cb895019c6c") {
        w.settings.mastering = 1;
        w.found();
        w.draw();
        run(&mut w, 3.0);
        for _ in 0..5 {
            press(&mut w, 2.5);
        }
        w.reload_shells(40);
        let (entered, _) = run(&mut w, 14.0);
        println!("  the RSh-12, mastering 1, from empty: {}", told(&entered));
        assert!(names(&entered).contains(&"RELOAD_INSERT_FISTFUL") && w.magazine == Some(5) && w.is_idle());
    }
    if let Some(mut w) = weapon_of(MSGL) {
        w.found();
        w.draw();
        run(&mut w, 3.0);
        for _ in 0..2 {
            press(&mut w, 2.5);
        }
        w.quick_reload_shells(40);
        let (entered, said) = run(&mut w, 30.0);
        println!("  the M32A1's quick reload: {}", told(&entered));
        assert_eq!((dropped(&said), w.magazine, w.camora), (vec![2, 3, 4, 5], Some(6), 0));
        assert!(w.is_idle());
    }
}

/// The Rhino's cylinder through single-action shots: its `FIRE` has no
/// motion on the `Cylinder` layer and writes no defaults, so the cylinder
/// stays at the chamber it was turned to until `FIRE ARM` turns it on
/// (the cylinder once twitched when shooting: it went back to the first chamber's angle for the
/// state's 110 ms). No frame turns it more than a clip does (25 degrees a
/// frame at most, the RSh-12's), on the pose and on the pose drawn.
#[test]
fn a_revolver_s_cylinder_holds_its_chamber_through_a_shot() {
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, RHINO).expect("the weapon");
    let data = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    let bone = data.bone("Weapon_root/Weapon_root_anim/weapon/weapon_crane/mod_magazine").expect("the cylinder's bone");
    let mut w = Firearm::new(settings, &data);
    w.set_drawn_smooth(true);
    w.found();
    w.draw();
    run(&mut w, 3.0);
    let angle = |a: [f32; 4], b: [f32; 4]| 2.0 * (0..4).map(|k| a[k] * b[k]).sum::<f32>().abs().min(1.0).acos().to_degrees();
    let turns = |w: &Firearm| (w.animator().pose()[bone].1, w.animator().drawn_pose()[bone].1);
    let (mut last, mut worst) = (turns(&w), (0.0f32, 0.0f32));
    for shot in 1..=3 {
        for (pressed, seconds) in [(true, 0.3), (false, 1.7)] {
            w.set_trigger(pressed, true);
            for _ in 0..(seconds / FRAME).round() as usize {
                w.step(FRAME);
                let now = turns(&w);
                worst = (worst.0.max(angle(last.0, now.0)), worst.1.max(angle(last.1, now.1)));
                last = now;
            }
        }
        assert_eq!(w.camora, shot as usize);
    }
    println!("  the cylinder's largest turn in a frame: {:.1} degrees, drawn {:.1}", worst.0, worst.1);
    assert!(worst.0 < 25.0 && worst.1 < 25.0, "{worst:?}");
}

/// Double action, six shots and two dry presses: the cylinder turns from
/// the chamber it stood at to the next, a chamber a press, and never jumps.
/// The double action's own blend on the `Cylinder` layer names a parameter
/// the controller lacks and so plays what `IDLE`'s blend by the new index
/// left in the layer's workspace (`more-weapons.md`, "A cylinder drawn
/// between frames"; recorded in `chiappa1`: 60 degrees over the first 0.6
/// of the state, from the chamber it stood at).
#[test]
fn a_revolver_s_double_action_turns_the_cylinder_on_by_a_chamber() {
    let Some(install) = crate::fixture::install() else { return };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, RHINO).expect("the weapon");
    let data = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    let bone = data.bone("Weapon_root/Weapon_root_anim/weapon/weapon_crane/mod_magazine").expect("the cylinder's bone");
    let mut w = Firearm::new(settings, &data);
    w.set_drawn_smooth(true);
    w.found();
    w.draw();
    run(&mut w, 3.0);
    assert!(w.set_fire_mode(FireMode::DoubleAction));
    run(&mut w, 1.5);
    let angle = |a: [f32; 4], b: [f32; 4]| 2.0 * (0..4).map(|k| a[k] * b[k]).sum::<f32>().abs().min(1.0).acos().to_degrees();
    let turns = |w: &Firearm| (w.animator().pose()[bone].1, w.animator().drawn_pose()[bone].1);
    let (mut last, mut worst) = (turns(&w), (0.0f32, 0.0f32));
    for press in 1..=8 {
        let (camora, at_rest) = (w.camora, last.0);
        for (pressed, seconds) in [(true, 0.3), (false, 1.7)] {
            w.set_trigger(pressed, true);
            for _ in 0..(seconds / FRAME).round() as usize {
                w.step(FRAME);
                let now = turns(&w);
                worst = (worst.0.max(angle(last.0, now.0)), worst.1.max(angle(last.1, now.1)));
                last = now;
            }
        }
        let moved = (w.camora + 6 - camora) % 6;
        let turned = angle(at_rest, last.0);
        println!("  press {press}: chamber {camora} to {}, the cylinder turned {turned:.1} degrees", w.camora);
        assert!((turned - 60.0 * moved as f32).abs() < 1.0, "press {press}: {turned} for {moved} chambers");
    }
    println!("  the cylinder's largest turn in a frame: {:.1} degrees, drawn {:.1}", worst.0, worst.1);
    assert!(worst.0 < 10.0 && worst.1 < 10.0, "{worst:?}");
}
