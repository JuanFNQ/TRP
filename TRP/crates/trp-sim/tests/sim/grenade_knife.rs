//! A grenade's hands and a knife's on the install's own animators
//! (`docs/spec/grenades-and-knife.md`): the F-1 drawn, its pin pulled, held
//! and thrown, and put back; the ice pick drawn, a slash and a stab with
//! the blade live between their marks and a hit glancing off, and the quick
//! stroke. These pass trivially, with a note, where no install is set up.

use trp_data::unity::Assets;
use trp_sim::weapon_animator::WeaponAnimatorData;
use trp_sim::{GrenadeEvent, GrenadeHands, KnifeEvent, KnifeHands, KnifeSettings, Stroke};

const FRAME: f32 = 1.0 / 120.0;
const F1: &str = "5710c24ad2720bc3458b45a3";
const ICE_PICK: &str = "5c0126f40db834002a125382";

fn rig(template: &str) -> Option<(WeaponAnimatorData, serde_json::Value)> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let item = db.items.get(template).expect("the item");
    let path = item.props["Prefab"]["path"].as_str().expect("its prefab").to_owned();
    let rig = trp_data::weapon::assemble_prefab(&mut assets, &path).expect("its rig");
    Some((WeaponAnimatorData::load(&assets, &rig).expect("its animator"), item.props.clone()))
}

/// Steps the grenade's hands for so long; every state entered and every
/// event, each with the time it came.
fn run_grenade(hands: &mut GrenadeHands, seconds: f32, clock: &mut f32) -> (Vec<(String, f32)>, Vec<(GrenadeEvent, f32)>) {
    let (mut entered, mut said) = (Vec::new(), Vec::new());
    let mut last = hands.animator().state().0.to_owned();
    for _ in 0..(seconds / FRAME).round() as usize {
        *clock += FRAME;
        for event in hands.step(FRAME) {
            said.push((event, *clock));
        }
        let (state, _, next) = hands.animator().state();
        let now = next.unwrap_or(state).trim_start_matches("Hands.").to_owned();
        if now != last {
            entered.push((now.clone(), *clock));
            last = now;
        }
    }
    (entered, said)
}

fn when<E: PartialEq>(said: &[(E, f32)], wanted: &E) -> Option<f32> {
    said.iter().find(|(event, _)| event == wanted).map(|(_, at)| *at)
}

#[test]
fn a_grenade_is_drawn_held_and_thrown() {
    let Some((data, props)) = rig(F1) else { return };
    let hidden_in_throw = props["CanBeHiddenDuringThrow"].as_bool().unwrap_or(false);
    let mut hands = GrenadeHands::new(&data, hidden_in_throw);
    let mut clock = 0.0;
    hands.draw();
    let (entered, said) = run_grenade(&mut hands, 1.6, &mut clock);
    let appeared = when(&said, &GrenadeEvent::Appeared).expect("drawn");
    println!("drawn: {entered:?}; in hand at {appeared:.3} s");
    // The draw's mark is 0.921 of its 1.333 s.
    assert!((appeared - 0.921 * 1.333).abs() < 0.05, "{appeared}");
    assert!(hands.idling());

    // The left button: the pin, then the hold while it is down.
    hands.press(false);
    assert_eq!(hands.waiting(), Some(false));
    let pressed = clock;
    let (entered, _) = run_grenade(&mut hands, 2.0, &mut clock);
    println!("held: {entered:?}");
    assert!(entered.iter().any(|(state, _)| state == "FIRE START") && entered.iter().any(|(state, _)| state == "FIRE IDLE"));
    // Let go: the throw, the grenade let go at the mark, then over.
    hands.release(false);
    let released = clock;
    let (entered, said) = run_grenade(&mut hands, 1.2, &mut clock);
    let thrown = when(&said, &GrenadeEvent::Thrown { low: false }).expect("thrown");
    let used = when(&said, &GrenadeEvent::Used).expect("used");
    println!("thrown: {entered:?}; let go {:.3} s after the release ({:.3} after the press), used up at {:.3}", thrown - released, thrown - pressed, used - released);
    assert!((thrown - released - 0.499 * 0.9).abs() < 0.03, "{}", thrown - released);
    assert!(used > thrown);

    // Let go during the pin: the throw waits for the pin's animation.
    let mut hands = GrenadeHands::new(&data, hidden_in_throw);
    let mut clock = 0.0;
    hands.draw();
    run_grenade(&mut hands, 1.6, &mut clock);
    hands.press(true);
    let pressed = clock;
    run_grenade(&mut hands, 0.2, &mut clock);
    hands.release(true);
    let (_, said) = run_grenade(&mut hands, 2.5, &mut clock);
    let thrown = when(&said, &GrenadeEvent::Thrown { low: true }).expect("thrown from below");
    println!("from below, let go 0.2 s after the press: thrown {:.3} s after the press", thrown - pressed);
    assert!((thrown - pressed - (1.333 + 0.355 * 0.9)).abs() < 0.06, "{}", thrown - pressed);

    // The inventory opened with the pin out puts it back.
    let mut hands = GrenadeHands::new(&data, hidden_in_throw);
    let mut clock = 0.0;
    hands.draw();
    run_grenade(&mut hands, 1.6, &mut clock);
    hands.press(false);
    run_grenade(&mut hands, 1.5, &mut clock);
    hands.set_inventory(true);
    assert!(hands.idling() && hands.waiting().is_none());
    let (entered, said) = run_grenade(&mut hands, 2.5, &mut clock);
    println!("the pin put back: {entered:?}");
    assert!(entered.iter().any(|(state, _)| state == "FIRE ABORT INV"));
    assert!(!said.iter().any(|(event, _)| matches!(event, GrenadeEvent::Thrown { .. })));

    // Put away at rest.
    hands.set_inventory(false);
    run_grenade(&mut hands, 1.0, &mut clock);
    hands.hide();
    let (_, said) = run_grenade(&mut hands, 1.5, &mut clock);
    assert!(when(&said, &GrenadeEvent::Hidden).is_some() && !hands.in_hand());
}

fn run_knife(hands: &mut KnifeHands, seconds: f32, clock: &mut f32) -> Vec<(KnifeEvent, f32)> {
    let mut said = Vec::new();
    for _ in 0..(seconds / FRAME).round() as usize {
        *clock += FRAME;
        for event in hands.step(FRAME) {
            said.push((event, *clock));
        }
    }
    said
}

#[test]
fn a_knife_strikes_and_glances_off() {
    let Some((data, props)) = rig(ICE_PICK) else { return };
    let settings = KnifeSettings::of(&props);
    println!("{settings:?}");
    assert_eq!((settings.slash_reach, settings.stab_reach, settings.slash_cost, settings.stab_cost), (0.6, 0.65, 10.0, 13.0));
    let mut hands = KnifeHands::new(&data, settings);
    let mut clock = 0.0;
    hands.draw();
    let said = run_knife(&mut hands, 1.2, &mut clock);
    let appeared = when(&said, &KnifeEvent::Appeared).expect("drawn");
    println!("drawn at {appeared:.3} s");
    assert!((appeared - 0.202).abs() < 0.05 && hands.idling());

    // A slash: live from its mark to its end, the first hit counted once.
    assert!(!hands.strike(Stroke::Slash, false, 1.0), "no stamina, no stroke");
    assert!(hands.strike(Stroke::Slash, true, 1.0));
    let struck = clock;
    let mut hit_at = None;
    let mut said = Vec::new();
    for _ in 0..120 {
        clock += FRAME;
        for event in hands.step(FRAME) {
            said.push((event, clock));
        }
        if hands.live() && hit_at.is_none() {
            hit_at = Some((clock, hands.hit()));
        }
    }
    let live = said.iter().find(|(event, _)| matches!(event, KnifeEvent::Live { .. })).expect("live").1;
    let dead = when(&said, &KnifeEvent::Dead).expect("over");
    println!("a slash: live {:.3} s after the button, over at {:.3}; hit {hit_at:?}", live - struck, dead - struck);
    assert!((live - struck - 0.307 * 0.733).abs() < 0.03, "{}", live - struck);
    assert!((dead - struck - 0.682 * 0.733).abs() < 0.03, "{}", dead - struck);
    assert_eq!(hit_at.map(|(_, damage)| damage), Some(Some(29.0)));
    assert!(hands.hit().is_none() && hands.idling());
    assert_eq!(hands.animator().get("Deflected"), 1.0);

    // A stab reaches further and costs more.
    assert!(hands.strike(Stroke::Stab, true, 1.0));
    let said = run_knife(&mut hands, 1.2, &mut clock);
    assert!(said.iter().any(|(event, _)| *event == KnifeEvent::Live { stroke: Stroke::Stab, reach: 0.65, cost: 13.0 }));

    // The quick stroke, and back.
    let mut quick = KnifeHands::new(&data, settings);
    let mut clock = 0.0;
    quick.draw_quick(1.0);
    let said = run_knife(&mut quick, 1.5, &mut clock);
    let live = said.iter().find(|(event, _)| matches!(event, KnifeEvent::Live { .. })).expect("live").1;
    let used = when(&said, &KnifeEvent::Used).expect("over");
    println!("the quick stroke: live at {live:.3} s, over at {used:.3}");
    assert!((live - 0.315).abs() < 0.04 && (used - 0.809).abs() < 0.04, "{live} {used}");
}

/// What the body's animator gives the weapon's bone for each kind of thing
/// in hand, standing still in first person: the game's own recording with
/// the knife has the body's first-person weight at 0 (`courses2`,
/// `2-knife-strokes`: `First_Person_Curve_Weight`), so that the knife's rig
/// is the hands' own and never the body's.
#[test]
fn the_body_leaves_a_knife_or_grenade_to_the_hands() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = trp_data::hard_settings::HardSettings::load(&assets.install).expect("settings object");
    let engine = trp_data::engine::EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement settings");
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    use trp_sim::body::hands_kind;
    for (name, kind) in [("rifle", hands_kind::RIFLE), ("thrown", hands_kind::THROWN), ("knife", hands_kind::KNIFE), ("empty hands", hands_kind::EMPTY_HANDS)] {
        let mut m = trp_sim::Movement::new(settings.clone(), trp_sim::Body::new(&body, 1.0));
        m.body.set_hands_kind(kind);
        for _ in 0..240 {
            m.step(FRAME, &trp_sim::Input::default(), &trp_sim::Flat);
        }
        let carried = m.body.carried([0.0, 0.0, 0.0, 1.0]).map_or(0.0, |c| c.authority);
        let arms = [true, false].map(|right| m.body.arm_pose(right, [0.0, 0.0, 0.0, 1.0])).map(|a| (a.hand, a.shoulder, a.elbow));
        println!("{name}: the arms the body takes (hand, shoulder, elbow), right then left: {arms:?}");
        println!("{name}: the body's hold of the weapon's bone {carried:.3}; {:?}", m.body.describe().iter().take(6).collect::<Vec<_>>());
    }
}

/// Where each rig's right-hand marker is, at rest, from its camera bone
/// (`Camera_animated`): the game's palm with the ice pick at rest is 0.22 m
/// under the eye and 0.11 m ahead of it (`courses2`, `2-knife-strokes`).
#[test]
fn a_rig_at_rest_holds_its_marker_under_the_eye() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    for (name, template) in [("bayonet", "54491bb74bdc2d09088b4567"), ("ice pick", ICE_PICK), ("F-1", F1)] {
        let item = db.items.get(template).expect("the item");
        let path = item.props["Prefab"]["path"].as_str().expect("its prefab").to_owned();
        let rig = trp_data::weapon::assemble_prefab(&mut assets, &path).expect("its rig");
        let data = WeaponAnimatorData::load(&assets, &rig).expect("its animator");
        let mut animator = trp_sim::weapon_animator::WeaponAnimator::new(&data);
        animator.set_flag("Active", true);
        for _ in 0..240 {
            animator.update(FRAME);
            animator.deliver();
        }
        let mut frame = trp_sim::rig::WeaponRig::new(&rig.hierarchy);
        let moved = frame.moved_bone();
        frame.pose(animator.pose(), moved);
        let (camera, marker) = (frame.bone("Camera_animated").expect("camera"), frame.bone("Weapon_root/Weapon_root_anim/weapon/weapon_R_IK_marker").expect("marker"));
        let ((camera_at, camera_turn), (marker_at, _)) = (frame.placed(camera), frame.placed(marker));
        let back = [-camera_turn[0], -camera_turn[1], -camera_turn[2], camera_turn[3]];
        let seen = trp_sim::math::quat_rotate(back, std::array::from_fn(|k| marker_at[k] - camera_at[k]));
        println!("{name}: in {}, the right marker from the camera bone {seen:?}", animator.state().0);
    }
}
