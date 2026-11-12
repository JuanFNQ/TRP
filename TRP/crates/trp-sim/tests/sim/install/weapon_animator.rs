//! The weapon's animator as a state machine, and head gestures.

use super::*;

/// The weapon's controller on the engine, by the game's own calls for a
/// weapon being drawn (firing spec, "The weapon's animator as a state
/// machine"): `SPAWN`, the draw, rest; then the inventory's flag, by which
/// the hands go down, are held and come back.
#[test]
fn the_weapons_animator_runs_as_a_state_machine() {
    use trp_sim::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    println!("{} clips; FIRE's marks: {:?}", data.clip_count(), data.marks_of("Hands.FIRE"));
    let mut a = WeaponAnimator::new(&data);
    assert_eq!(a.state().0, "Hands.SPAWN");
    // As `SpawnOperation` and `SetAnimatorValues` leave it for a loaded rifle.
    a.set_flag("Active", true);
    a.set("AmmoInChamber", 1.0);
    a.set("AmmoInMag", 30.0);
    a.set_flag("MagInWeapon", true);
    const FRAME: f32 = 1.0 / 120.0;
    let mut log: Vec<(f32, String)> = Vec::new();
    let mut last = String::new();
    let mut t = 0.0;
    let mut run = |a: &mut WeaponAnimator, seconds: f32, log: &mut Vec<(f32, String)>| {
        for _ in 0..(seconds / FRAME).round() as usize {
            a.update(FRAME);
            t += FRAME;
            let (state, _, next) = a.state();
            let now = format!("{state}{}", next.map_or(String::new(), |next| format!(" -> {next}")));
            if now != last {
                log.push((t, now.clone()));
                last = now;
            }
            for mark in a.deliver() {
                log.push((t, format!("mark {} {}", mark.name, mark.text)));
            }
        }
    };
    run(&mut a, 1.5, &mut log);
    assert_eq!(a.state().0, "Hands.IDLE");
    a.set_flag("Inventory", true);
    run(&mut a, 1.5, &mut log);
    assert_eq!(a.state().0, "Hands.IN INVENTORY");
    a.set_flag("Inventory", false);
    run(&mut a, 1.5, &mut log);
    assert_eq!(a.state().0, "Hands.IDLE");
    for (at, what) in &log {
        println!("{at:6.3} s  {what}");
    }
    let at = |what: &str| log.iter().find(|(_, entry)| entry == what).map(|(at, _)| *at);
    // The draw is 0.967 s long and hands over to rest at 0.98 of it.
    let rest = at("Hands.OUT TO IDLE -> Hands.IDLE").expect("the draw blends into rest");
    assert!((rest - 0.967 * 0.98).abs() < 0.02, "{rest}");
    assert!(at("mark WeapIn ").is_some() && at("mark IdleStart ").is_some());
    // The way down: 0.333 s of `OPEN INVENTORY`, then the held state.
    let down = at("Hands.IDLE -> Hands.OPEN INVENTORY").expect("the hands go down");
    let held = at("Hands.OPEN INVENTORY -> Hands.IN INVENTORY").expect("and are held");
    assert!((held - down - 0.333).abs() < 0.03, "{}", held - down);
}

/// The weapon's logic going by the weapon's own animator, with the
/// install's AKM (firing spec, "The weapon goes by its animator"): the
/// draw and the put-away as the animator's states, a reload that ends in
/// the hold of an open inventory without the hands going back to the
/// weapon, from a chambered round and from an empty chamber, a reload
/// asked for in a shot, a weapon drawn with its chamber empty; and the
/// empty hands on their own animator.
#[test]
fn the_weapon_goes_by_its_animator() {
    use trp_sim::weapons::weapon_animator::WeaponAnimatorData;
    use trp_sim::{EmptyHands, Firearm, FirearmEvent};
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM");
    let data = WeaponAnimatorData::load(&assets, &weapon).expect("its controller");
    let settings = trp_sim::firearm_settings(&assets, &db, &hard, &weapon).expect("its settings");
    const FRAME: f32 = 1.0 / 120.0;
    // Every state the animator enters over some seconds, with when, and
    // whatever the weapon says of itself.
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
    let when = |said: &[(FirearmEvent, f32)], what: &FirearmEvent| said.iter().find(|(event, _)| event == what).map(|(_, at)| *at);

    // Drawn, at rest, put away.
    let mut w = Firearm::new(settings.clone(), &data);
    w.draw();
    assert!(w.hands_busy() && w.occupied());
    let (entered, said) = run(&mut w, 1.5);
    let appeared = when(&said, &FirearmEvent::Appeared).expect("the weapon appears");
    println!("drawn: {entered:?}; appeared {appeared:.3} s after the draw was asked for");
    assert_eq!(names(&entered), ["OUT TO IDLE", "IDLE"]);
    // The draw's state is 0.967 s long and gives way at 0.98 of it.
    assert!((appeared - 0.967 * 0.98).abs() < 0.03, "{appeared}");
    assert!(w.is_idle() && !w.hands_busy());
    w.hide(false);
    let (entered, said) = run(&mut w, 1.5);
    let hidden = when(&said, &FirearmEvent::Hidden).expect("the weapon goes");
    println!("put away: {entered:?}; hidden {hidden:.3} s after it was asked for");
    assert_eq!(names(&entered), ["IDLE TO OUT", "SPAWN"]);
    // Its mark sits 0.968 of the way through a state of 0.833 s.
    assert!((hidden - 0.833 * 0.968).abs() < 0.05, "{hidden}");
    assert!(!w.in_hand());

    // A reload with a round chambered, the inventory opened 0.3 s into it:
    // noted until the magazine is out, and then the reload ends in the
    // hold, by the animator's own way there.
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    w.reload();
    let (mut entered, _) = run(&mut w, 0.3);
    w.set_inventory(true);
    assert!(!w.animator().flag("Inventory"));
    let (more, said) = run(&mut w, 5.0);
    entered.extend(more.into_iter().map(|(name, at)| (name, at + 0.3)));
    println!("a reload with the inventory open: {entered:?}");
    assert_eq!(names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD INVENTORY END", "IN INVENTORY"]);
    assert!(when(&said, &FirearmEvent::MagazineIn).is_some() && w.in_inventory() && !w.reloading());
    assert_eq!((w.magazine, w.chamber), (Some(30), true));
    // Shut, the hands come back.
    w.set_inventory(false);
    let (entered, _) = run(&mut w, 1.5);
    assert_eq!(names(&entered), ["CLOSE INVENTORY", "IDLE"]);
    assert!(w.is_idle());

    // The same from an empty chamber: through the chambering, and from
    // there to the hold.
    let mut w = Firearm::new(settings.clone(), &data);
    (w.chamber, w.magazine) = (false, Some(0));
    w.start_drawn();
    w.reload();
    let (mut entered, _) = run(&mut w, 0.3);
    w.set_inventory(true);
    let (more, _) = run(&mut w, 7.0);
    entered.extend(more.into_iter().map(|(name, at)| (name, at + 0.3)));
    println!("a reload from empty with the inventory open: {entered:?}");
    assert_eq!(names(&entered), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD CHARGE END", "RELOAD INVENTORY END", "IN INVENTORY"]);
    assert_eq!((w.magazine, w.chamber), (Some(29), true));

    // A reload asked for in a single shot follows the shot's own wait.
    let mut w = Firearm::new(settings.clone(), &data);
    w.start_drawn();
    w.set_trigger(true, true);
    w.step(FRAME);
    w.reload();
    let mut waited = FRAME;
    while !w.reloading() && waited < 1.0 {
        w.step(FRAME);
        waited += FRAME;
    }
    println!("a reload asked for in a shot begins {waited:.3} s after the press (the wait between single shots is {:.3} s)", 60.0 / settings.single_fire_rate);
    assert!((waited - 60.0 / settings.single_fire_rate).abs() < 0.03, "{waited}");

    // Drawn with rounds in the magazine and none chambered, the weapon
    // racks its bolt: the animator's other draw.
    let mut w = Firearm::new(settings.clone(), &data);
    (w.chamber, w.magazine) = (false, Some(7));
    w.draw();
    let (entered, said) = run(&mut w, 2.5);
    let appeared = when(&said, &FirearmEvent::Appeared).expect("the weapon appears");
    println!("drawn with an empty chamber: {entered:?}; appeared at {appeared:.3} s");
    assert_eq!(names(&entered), ["OUT TO IDLE ARM", "IDLE"]);
    assert_eq!((w.magazine, w.chamber), (Some(6), true));
    assert_eq!(w.animator().get("AmmoInChamber"), 1.0);

    // The empty hands: there 0.034 s into their state for appearing.
    let empty = trp_data::weapons::weapon::assemble_prefab(&mut assets, trp_data::weapons::weapon::EMPTY_HANDS).expect("the empty hands");
    let data = WeaponAnimatorData::load(&assets, &empty).expect("their controller");
    let mut hands = EmptyHands::new(&data);
    hands.draw();
    assert!(hands.hands_busy());
    let mut appeared = None;
    for frame in 0..120 {
        if hands.step(FRAME, 0.0).contains(&FirearmEvent::Appeared) {
            appeared.get_or_insert((frame + 1) as f32 * FRAME);
        }
    }
    let appeared = appeared.expect("the hands appear");
    println!("empty hands: there {appeared:.3} s after they were asked for; at rest in {}", hands.animator().state().0);
    assert!(appeared < 0.06 && !hands.hands_busy());
    assert_eq!(hands.animator().state().0, "Hands.IDLE");
    // Looking down, the resting hands are the other of their two clips.
    let level = hands.animator().pose().to_vec();
    for _ in 0..30 {
        hands.step(FRAME, 90.0);
    }
    let down = hands.animator().pose();
    let moved = level.iter().zip(down).filter(|(a, b)| (0..3).any(|k| (a.0[k] - b.0[k]).abs() > 1e-4)).count();
    println!("{moved} of their {} bones are elsewhere with the view straight down", level.len());
    assert!(moved > 0);
    hands.hide();
    assert!(!hands.in_hand() && hands.hands_busy());
}

/// A switch on the head flipped by the key (inventory spec, "A switch on the
/// head"): its gesture on the body's own animator, set beside course 3 of
/// `courses2`, and refused with the thing in hand not at rest.
#[test]
fn a_switch_on_the_head_is_flipped_by_a_gesture() {
    use trp_sim::Interaction;
    let Some(mut m) = movement(0.0) else { return };
    const FRAME: f32 = 1.0 / 120.0;
    let still = Input { hands_idle: true, ..Input::default() };
    for _ in 0..120 {
        m.step(FRAME, &still, &Flat);
    }
    // How long the gesture's flag stays up, as the recording counts it.
    let lasts = |m: &mut Movement| -> f32 {
        let mut frames = 1;
        while m.interaction().is_some() && frames < 600 {
            m.step(FRAME, &still, &Flat);
            frames += 1;
        }
        (frames - 1) as f32 * FRAME
    };
    // Recorded (`courses2`, `3-menu-worn`, the key): the face shield's
    // gesture up 1.342 s, down 1.342 s.
    for gesture in [Interaction::FaceshieldOnGear, Interaction::FaceshieldOffGear, Interaction::NightVisionOnGear, Interaction::NightVisionOffGear] {
        assert!(m.set_interaction(gesture));
        assert_eq!(m.interaction(), Some(gesture));
        let took = lasts(&mut m);
        println!("{gesture:?}: {took:.3} s");
        if matches!(gesture, Interaction::FaceshieldOnGear | Interaction::FaceshieldOffGear) {
            assert!((took - 1.342).abs() <= 2.0 * FRAME, "{gesture:?}: {took}");
        }
        for _ in 0..30 {
            m.step(FRAME, &still, &Flat);
        }
    }
    // With the thing in hand not at rest (as under the open inventory),
    // refused.
    let busy = Input { hands_idle: false, ..Input::default() };
    m.step(FRAME, &busy, &Flat);
    assert!(!m.set_interaction(Interaction::FaceshieldOnGear));
}
