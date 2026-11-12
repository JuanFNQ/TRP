//! The figure's events and kinds, the rifle held at idle and the look out.

use super::*;

/// The clips' events and the curves the figure stands by are the
/// install's: the idle's pick 7.305 s into its 7.33 s loop, the weapon
/// check's start and end, four random transitions; the kinds' blend gives
/// the held idle for a two-handed weapon and a pistol and the hands down for
/// nothing, a thrown thing and a knife.
#[test]
fn the_figure_s_events_and_kinds_are_the_install_s() {
    let Some((data, ..)) = data() else { return };
    let time = |clip: &str, function: &str| data.event_time(clip, function).unwrap_or_else(|| panic!("{clip} has no {function}"));
    let pick = time("inv_idle", "OnRandomizeAnimation");
    println!("OnRandomizeAnimation at {pick:.6} s of inv_idle, {:.6} of inv_idle_hands_down", time("inv_idle_hands_down", "OnRandomizeAnimation"));
    assert!((pick - 7.3048).abs() < 1e-3, "{pick}");
    assert_eq!(pick, time("inv_idle_hands_down", "OnRandomizeAnimation"));
    let start = time("inv_idle_weaponcheck_in", "OnWeaponOperationStart");
    let end = time("inv_idle_weaponcheck_out", "OnWeaponOperationEnd");
    println!("OnWeaponOperationStart at {start:.6} s, OnWeaponOperationEnd at {end:.6} s");
    assert!((start - 1.0732).abs() < 1e-3 && (end - 0.257).abs() < 2e-3, "{start} {end}");
    assert_eq!(data.parameter_default("UI_RandomAnimationsCount"), Some(4.0));
    for kind in [kind::NOTHING, kind::TWO_HANDED, kind::PISTOL, kind::THROWN, kind::KNIFE] {
        let figure = Figure::new(&data, kind, None, 0);
        let curve = |name: &str| figure.animator().float(name);
        let curves = [curve("Weapon_3rd"), curve("Hand_Right"), curve("Hand_Left"), curve("Elbow_Right"), curve("Elbow_Left")];
        println!("kind {kind}: {} at {:?}; Weapon_3rd, hands, elbows {curves:?}", layer(&figure, data.layer()).0, layer(&figure, data.layer()).1);
        let held = kind == kind::TWO_HANDED || kind == kind::PISTOL;
        let expected = if held { [1.0, 0.0, 0.0, 0.0, 0.0] } else { [0.0, 1.0, 1.0, 1.0, 1.0] };
        assert_eq!(curves, expected, "kind {kind}");
        assert_eq!(layer(&figure, data.layer()).0, "GUI_IDLE");
    }
}

/// The stock AKM held: after a second the weapon's moved bone is on the
/// body's `Weapon_root_3rd_anim`, each palm on its target, the right on
/// its marker; the feet still through a whole loop of the idle, its first
/// pick at its event, none before.
#[test]
fn the_figure_holds_a_rifle_and_picks_at_the_idle_s_event() {
    let Some((data, assets, db)) = data() else { return };
    let mut assets = assets;
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, db, AKM).expect("the AKM");
    let held = FigureWeaponData::load(&assets, &weapon).expect("the AKM in the figure's hands");
    let loaded = Loaded::of(db, &weapon, 1, false);
    assert_eq!(kind_of(db, AKM), kind::TWO_HANDED);
    assert_eq!((loaded.can_reload, loaded.can_check_chamber, loaded.magazine), (true, true, Some(30)));
    let mut figure = Figure::new(&data, kind_of(db, AKM), Some((&held, loaded)), 0);
    assert_eq!(figure.weapon_moves(), ["Look", "CheckChamber", "CheckAmmo", "Reload"]);
    let feet = figure.bones().feet;
    let mut moved_feet = 0.0f32;
    let mut first_pick = None;
    let mut steps = 0;
    while figure.age() < 7.4 {
        figure.step(DT);
        steps += 1;
        let bones = figure.bones();
        for side in 0..2 {
            moved_feet = moved_feet.max(distance(bones.feet[side].0, feet[side].0));
        }
        if first_pick.is_none() && !figure.picks().is_empty() {
            first_pick = Some(figure.age());
        }
        if steps == 60 {
            let on = bones.weapon_root_anim.expect("a weapon");
            println!("after 1 s: Weapon_root_anim {:?} on {:?}", on.0, bones.weapon_3rd.0);
            assert!(distance(on.0, bones.weapon_3rd.0) < 1e-5 && apart(on.1, bones.weapon_3rd.1) < 0.01);
            for side in 0..2 {
                let (target, marker) = (bones.targets[side].expect("a target"), bones.markers[side].expect("a marker"));
                let gap = distance(bones.palms[side].0, target.0);
                println!("  palm {side}: {:.2} mm from its target, {:.1} mm from its marker, turned {:.3} deg from the target", gap * 1000.0, distance(bones.palms[side].0, marker.0) * 1000.0, apart(bones.palms[side].1, target.1));
                assert!(gap < 2e-3, "palm {side} {gap}");
            }
            let right = bones.markers[0].expect("a marker");
            assert!(distance(bones.palms[0].0, right.0) < 2e-3);
        }
    }
    let first = first_pick.expect("a pick in the first loop");
    println!("first pick at {first:.4} s ({:?}); the feet moved at most {:.3} mm", figure.picks()[0].1, moved_feet * 1000.0);
    assert!(first >= 7.3048 && first < 7.3048 + DT + 1e-4, "{first}");
    assert_eq!(figure.picks().len(), 1);
    assert!(moved_feet < 1e-3, "{moved_feet}");
}

/// A look out steered in at the first pick: the idle goes into it at its
/// loop's end, 7.33 s in, and back to the idle 7.33 s later; with empty
/// hands nothing is ever picked but the idle.
#[test]
fn a_look_out_plays_after_the_pick_and_ends_in_the_idle() {
    let Some((data, assets, db)) = data() else { return };
    let mut assets = assets;
    let weapon = trp_data::weapons::weapon::assemble(&mut assets, db, AKM).expect("the AKM");
    let held = FigureWeaponData::load(&assets, &weapon).expect("the AKM in the figure's hands");
    let mut figure = Figure::new(&data, kind::TWO_HANDED, Some((&held, Loaded::of(db, &weapon, 1, false))), 0);
    figure.steer(Pick::Random(3));
    let gui = data.layer();
    let mut entered = None;
    let mut back = None;
    while figure.age() < 16.0 {
        figure.step(DT);
        let (state, ..) = layer(&figure, gui);
        if entered.is_none() && state == "LookOut_Alert" {
            entered = Some(figure.age());
        }
        if entered.is_some() && back.is_none() && state == "GUI_IDLE" {
            back = Some(figure.age());
        }
    }
    let (entered, back) = (entered.expect("the look out"), back.expect("the idle again"));
    println!("LookOut_Alert from {entered:.4} s to {back:.4} s; picks {:?}", figure.picks());
    assert!((entered - 22.0 / 3.0).abs() < DT + 1e-3, "{entered}");
    assert!((back - entered - 22.0 / 3.0).abs() < DT + 1e-3, "{back}");
    // Empty hands: no weapon animator, nothing drawn, the idle each time.
    let mut empty = Figure::new(&data, kind::NOTHING, None, 0);
    empty.steer(Pick::Random(1));
    while empty.age() < 30.0 {
        empty.step(DT);
        assert_eq!(layer(&empty, gui).0, "GUI_IDLE");
    }
    assert_eq!(empty.picks().len(), 4);
    assert!(empty.picks().iter().all(|(_, pick)| *pick == Pick::Idle));
}
