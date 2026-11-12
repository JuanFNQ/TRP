//! What the figure holds: grenades, knives, the rangefinder and the AKM's moves.

use super::*;

/// A thrown thing in hand: the hands down, the grenade hung by its right
/// marker from the right palm (`PlayerBones.Kinematics` at nought), no arm
/// solved onto it.
#[test]
fn a_grenade_hangs_from_the_right_palm() {
    const F1: &str = "5710c24ad2720bc3458b45a3";
    let Some((data, assets, db)) = data() else { return };
    let mut assets = assets;
    assert_eq!(kind_of(db, F1), kind::THROWN);
    // The item's animated model, its prefab (`CreateItem(.., isAnimated:
    // true)`), as the grenade's hands are made.
    let path = db.items[F1].props["Prefab"]["path"].as_str().expect("its prefab").to_owned();
    let weapon = trp_data::weapons::weapon::assemble_prefab(&mut assets, &path).expect("the F-1's rig");
    let held = FigureWeaponData::load(&assets, &weapon).expect("the F-1 in the figure's hands");
    let mut figure = Figure::new(&data, kind::THROWN, Some((&held, Loaded::of(db, &weapon, 0, false))), 0);
    for _ in 0..60 {
        figure.step(DT);
    }
    let bones = figure.bones();
    let marker = bones.markers[0].expect("a right marker");
    println!("the right marker {:.2} mm and {:.4} deg off the right palm; the left target {:?}", distance(marker.0, bones.palms[0].0) * 1000.0, apart(marker.1, bones.palms[0].1), bones.targets[1]);
    assert!(distance(marker.0, bones.palms[0].0) < 1e-5 && apart(marker.1, bones.palms[0].1) < 0.01);
    assert_eq!(bones.targets, [None, None], "no arm solved");
    assert_eq!(figure.animator().float("Hand_Right"), 1.0);
}

/// What a figure did, frame by frame, until the time given: each change of
/// the `GUI` layer's state (and the state it enters, as `->`), of the
/// weapon's hands' state, and what reached the poser (the sounds left
/// out), each with the figure's time.
fn timeline(figure: &mut Figure, gui: usize, frame: f32, until: f32) -> Vec<(f32, String)> {
    let mut log = Vec::new();
    let (mut body_was, mut weapon_was) = (String::new(), String::new());
    while figure.age() < until {
        figure.step(frame);
        let (state, _, next) = layer(figure, gui);
        let body = next.map_or(state.clone(), |next| format!("{state} -> {next}"));
        if body != body_was {
            log.push((figure.age(), format!("body {body}")));
            body_was = body;
        }
        let own = |name: &str| name.rsplit('.').next().unwrap_or(name).to_owned();
        let weapon = figure.weapon_state().map(|(state, _, next)| next.map_or(own(state), |next| format!("{} -> {}", own(state), own(next)))).unwrap_or_default();
        if weapon != weapon_was {
            log.push((figure.age(), format!("weapon {weapon}")));
            weapon_was = weapon;
        }
        for event in figure.heard().iter().filter(|event| event.name != "Sound") {
            log.push((figure.age(), format!("heard {}", said(event))));
        }
    }
    log
}

/// The first time at or after `from` that the log says `what`.
fn when(log: &[(f32, String)], what: &str, from: f32) -> f32 {
    log.iter().find(|(at, said)| *at >= from - 1e-6 && said == what).map(|(at, _)| *at).unwrap_or_else(|| panic!("no {what:?} from {from:.4} s in {log:#?}"))
}

/// The first time at or after `from` that the weapon's hands begin a state:
/// entered at once, or by a transition into it.
fn enters(log: &[(f32, String)], state: &str, from: f32) -> f32 {
    let (at_once, by) = (format!("weapon {state}"), format!(" -> {state}"));
    log.iter()
        .find(|(at, said)| *at >= from - 1e-6 && (*said == at_once || (said.starts_with("weapon ") && said.ends_with(&by))))
        .map(|(at, _)| *at)
        .unwrap_or_else(|| panic!("the weapon never enters {state:?} from {from:.4} s in {log:#?}"))
}

/// The AKM's four weapon moves, each steered in at the first pick, as the
/// game plays them (`MenuPlayerPoser`, the controllers' states from the
/// install): on the frame after the pick the body into `Hands_Operation_In`
/// and the weapon off patrol (its `PATROL OUT`, then its `IDLE`, whose
/// first `IdleStart` only clears the wait); at the In clip's event, 1.073 s
/// on, the move's trigger, the weapon in the move's state on the next
/// frame; back in its `IDLE`, the second `IdleStart` ends the move: the
/// body leaves its loop for `Hands_Operation_Out` on the next frame, whose
/// event 0.257 s on puts the weapon back on patrol (`PATROL IN` on the
/// next frame), and the idle comes again at Out's end, 1.5 s on. The look
/// plays its whole clip (3.333 s) and so does the magazine's check (4.333
/// s: `probe states 59d6088586f774275f37482f`); a reload with the chamber
/// empty charges the bolt and counts its round (`AddAmmoInChamber`).
#[test]
fn the_akm_s_weapon_moves_as_the_game_plays_them() {
    let Some((data, assets, db)) = data() else { return };
    let mut assets = assets;
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, db, AKM).expect("the AKM");
    let held = FigureWeaponData::load(&assets, &weapon).expect("the AKM in the figure's hands");
    const FRAME: f32 = 1.0 / 120.0;
    let near = |a: f32, b: f32| (a - b).abs() < FRAME * 1.01 + 1e-4;
    let start = data.event_time("inv_idle_weaponcheck_in", "OnWeaponOperationStart").expect("the start's event");
    let end = data.event_time("inv_idle_weaponcheck_out", "OnWeaponOperationEnd").expect("the end's event");
    for (index, (trigger, state, length)) in [("Look", "LOOK", Some(10.0 / 3.0)), ("CheckChamber", "CHECK CHAMBER", None), ("CheckAmmo", "CHECK", Some(13.0 / 3.0)), ("Reload", "RELOAD START", None)].into_iter().enumerate() {
        let chamber = u32::from(trigger != "Reload");
        let mut figure = Figure::new(&data, kind::TWO_HANDED, Some((&held, Loaded::of(db, &weapon, chamber, false))), 0);
        assert_eq!(figure.weapon_moves()[index], trigger);
        figure.steer(Pick::WeaponMove(index));
        let log = timeline(&mut figure, data.layer(), FRAME, 25.0);
        println!("{trigger} ({chamber} chambered):");
        for (at, said) in &log {
            println!("  {at:8.4} {said}");
        }
        let picked = when(&log, "heard body OnRandomizeAnimation", 0.0);
        assert!(picked >= 7.3048 && picked < 7.3048 + FRAME + 1e-4, "{picked}");
        let into = when(&log, "body Hands_Operation_In", picked);
        assert!(near(into, picked + FRAME), "{into}");
        assert!(near(enters(&log, "PATROL OUT", picked), picked + FRAME));
        let idle = enters(&log, "IDLE", picked);
        assert!(near(when(&log, "heard weapon IdleStart", picked), idle), "the first IdleStart as the weapon reaches its idle");
        let began = when(&log, "heard body OnWeaponOperationStart", into);
        assert!(began >= into + start - 1e-4 && began < into + start + FRAME + 1e-4, "{began}");
        let moved = enters(&log, state, began);
        assert!(near(moved, began + FRAME), "{trigger}: {moved}");
        let back = enters(&log, "IDLE", moved + FRAME);
        if let Some(length) = length {
            assert!((back - moved - length).abs() < FRAME + 1e-3, "{trigger}: {:.4} s of {length:.4}", back - moved);
        }
        let done = when(&log, "heard weapon IdleStart", back);
        assert!(near(done, back), "{trigger}: the second IdleStart as the weapon is back in its idle");
        let out = when(&log, "body Hands_Operation_Loop -> Hands_Operation_Out", done);
        assert!(near(out, done + FRAME), "{trigger}: {out}");
        let ended = when(&log, "heard body OnWeaponOperationEnd", out);
        assert!(ended >= out + end - 1e-4 && ended < out + end + FRAME + 1e-4, "{trigger}: {ended}");
        assert!(near(enters(&log, "PATROL IN", ended), ended + FRAME), "{trigger}: back on patrol");
        let again = when(&log, "body GUI_IDLE", out);
        assert!((again - out - 1.5).abs() < FRAME + 1e-3, "{trigger}: {again}");
        enters(&log, "PATROL", ended);
        if trigger == "Reload" {
            when(&log, "heard weapon AddAmmoInChamber", moved);
        }
        // The next pick at the idle's own event, 7.305 s after it came again.
        let next = when(&log, "heard body OnRandomizeAnimation", again);
        assert!(next - again >= 7.3048 - 1e-3 && next - again < 7.3048 + FRAME + 1e-3, "{trigger}: {next}");
    }
}

/// The rangefinder in hand: a weapon's prefab with no `PATROL` (its
/// animator asked for it stays where it is), told kind 0, held by both
/// hands on its markers; its one move the look.
#[test]
fn the_rangefinder_is_held_with_no_patrol() {
    const RANGER: &str = "61605e13ffa6e502ac5e7eef";
    let Some((data, assets, db)) = data() else { return };
    let mut assets = assets;
    assert_eq!(kind_of(db, RANGER), kind::TWO_HANDED);
    let path = db.items[RANGER].props["Prefab"]["path"].as_str().expect("its prefab").to_owned();
    let weapon = trp_data::weapons::weapon::assemble_prefab(&mut assets, &path).expect("its rig");
    let held = FigureWeaponData::load(&assets, &weapon).expect("the rangefinder in the figure's hands");
    let loaded = Loaded::of(db, &weapon, 0, false);
    assert!(!loaded.firearm && !loaded.can_reload && !loaded.can_check_chamber);
    let mut figure = Figure::new(&data, kind::TWO_HANDED, Some((&held, loaded)), 0);
    assert_eq!(figure.weapon_moves(), ["Look"]);
    for _ in 0..60 {
        figure.step(DT);
    }
    let bones = figure.bones();
    println!("the rangefinder after 1 s: its hands' state {:?}", figure.weapon_state().map(|(state, ..)| state.to_owned()));
    for side in 0..2 {
        let target = bones.targets[side].expect("a target");
        let gap = distance(bones.palms[side].0, target.0);
        println!("  palm {side}: {:.2} mm from its target", gap * 1000.0);
        assert!(gap < 2e-3, "palm {side} {gap}");
    }
}

/// A knife or a grenade in hand: the hands down (the idle by the hands
/// down's clip), the thing hung by its right marker from the right palm,
/// no arm solved onto it; a random transition of 4 plays the melee check
/// (`WeaponCheck_Melee`, `inv_idle_handsdown_weaponcheck`, 7.33 s) from the
/// idle's loop's end; and a weapon move picked never takes the body out of
/// its idle (`Hands_Operation_In` wants a kind below 2).
#[test]
fn a_knife_and_a_grenade_hang_from_the_right_palm_and_check_themselves() {
    const F1: &str = "5710c24ad2720bc3458b45a3";
    const BAYONET: &str = "54491bb74bdc2d09088b4567";
    let Some((data, assets, db)) = data() else { return };
    let mut assets = assets;
    for (template, kind) in [(BAYONET, kind::KNIFE), (F1, kind::THROWN)] {
        assert_eq!(kind_of(db, template), kind);
        let path = db.items[template].props["Prefab"]["path"].as_str().expect("its prefab").to_owned();
        let weapon = trp_data::weapons::weapon::assemble_prefab(&mut assets, &path).expect("its rig");
        let held = FigureWeaponData::load(&assets, &weapon).expect("in the figure's hands");
        let loaded = Loaded::of(db, &weapon, 0, false);
        let mut figure = Figure::new(&data, kind, Some((&held, loaded)), 0);
        assert_eq!(figure.weapon_moves(), ["Look"], "{template}: not a firearm");
        figure.steer(Pick::Random(4));
        figure.steer(Pick::WeaponMove(0));
        let log = timeline(&mut figure, data.layer(), DT, 30.0);
        println!("{template} (kind {kind}):");
        for (at, said) in &log {
            println!("  {at:8.4} {said}");
        }
        let bones = figure.bones();
        let marker = bones.markers[0].expect("a right marker");
        assert!(distance(marker.0, bones.palms[0].0) < 1e-5 && apart(marker.1, bones.palms[0].1) < 0.01, "{template}: on the right palm");
        assert_eq!(bones.targets, [None, None], "{template}: no arm solved");
        let checked = when(&log, "body WeaponCheck_Melee", 7.0);
        assert!((checked - 22.0 / 3.0).abs() < DT + 1e-3, "{template}: {checked}");
        let idle = when(&log, "body GUI_IDLE", checked);
        assert!((idle - checked - 22.0 / 3.0).abs() < DT + 1e-3, "{template}: {idle}");
        assert!(log.iter().all(|(_, said)| !said.contains("Hands_Operation")), "{template}: no weapon move's states for a kind of 2 or more");
    }
}
