//! The left shoulder (`docs/spec/left-shoulder.md`) with the install's body
//! animator: the key, the body's `LS` layer and its curve in and out, the
//! sprint's taking it off and its end's putting it back at once, lying
//! down's refusal, a hands operation's taking it off, blind fire's refusal
//! and the mount's ending it. These pass trivially, with a note, where no
//! install is configured.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::unity::Assets;
use trp_sim::left_stance::{Call, HandsStart};
use trp_sim::{Flat, Input, Movement};

const DT: f32 = 1.0 / 60.0;

fn movement() -> Option<Movement> {
    let install = crate::fixture::install()?;
    let db = crate::fixture::database().expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let hard = HardSettings::load(&assets.install).expect("settings object");
    let engine = EngineSettings::load(&assets.install).expect("engine settings");
    let settings = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement settings");
    let body = trp_sim::body::BodyData::load(&mut assets).expect("body animator");
    let mut m = Movement::new(settings, trp_sim::Body::new(&body, 5.0));
    // A firearm at rest in hand: its idle's start put the animation back.
    m.step(DT, &armed(), &Flat);
    m.left_stance_call(Call::Start(HandsStart::Idle));
    Some(m)
}

/// A frame's input with a firearm at rest in hand.
fn armed() -> Input {
    Input { blind_firearm: true, blind_may_start: true, left_stance_may: true, ..Input::default() }
}

fn run(m: &mut Movement, seconds: f32, input: Input) {
    for _ in 0..(seconds / DT).round() as usize {
        m.step(DT, &input, &Flat);
    }
}

fn press(m: &mut Movement) {
    m.step(DT, &Input { left_stance: true, ..armed() }, &Flat);
}

/// The key flips it; the body's `LS` layer plays the stance in over half a
/// second, its curve rising with the state's time to 1, and out again as
/// its exit clip plays backwards (the recording: 1 minus the state's time).
#[test]
fn the_body_plays_the_left_shoulder_in_and_out() {
    let Some(mut m) = movement() else {
        eprintln!("no install: skipped");
        return;
    };
    press(&mut m);
    assert!(m.left_stance().wanted && m.left_stance().animator);
    let mut curve = Vec::new();
    for _ in 0..45 {
        m.step(DT, &armed(), &Flat);
        curve.push(m.body.left_stance_curve());
    }
    eprintln!("in: {:?}; the layer {:?}; LeftStance {:?} Force {:?} curve id {:?}", &curve[..12], m.body.layer_state("LS"), m.body.parameter("LeftStance"), m.body.parameter("ForceLeftStanceInOneFrame"), m.body.parameter("Leftstance_curve"));
    assert!(curve[44] > 0.999, "{curve:?}");
    assert!(curve[14] > 0.3 && curve[14] < 0.7, "half way after a quarter second: {}", curve[14]);
    press(&mut m);
    assert!(!m.left_stance().wanted && !m.left_stance().animator);
    let mut out = Vec::new();
    for _ in 0..60 {
        m.step(DT, &armed(), &Flat);
        out.push(m.body.left_stance_curve());
    }
    eprintln!("out: {:?}; the layer {:?}", &out[..12], m.body.layer_state("LS"));
    assert!(out[59] < 0.001, "{out:?}");
    assert!(out[14] > 0.3 && out[14] < 0.85, "going out: {}", out[14]);
}

/// A sprint takes the animation off and keeps the stance wanted; its end
/// puts it back at once (`ForceLeftStanceInOneFrame`). Lying down refuses
/// the key and takes the animation off; a hands operation takes it off
/// until the weapon rests again.
#[test]
fn the_body_and_the_hands_take_it_off_and_put_it_back() {
    let Some(mut m) = movement() else {
        eprintln!("no install: skipped");
        return;
    };
    press(&mut m);
    run(&mut m, 0.6, armed());
    // (A sprint key held before moving is not heard: walk first.)
    run(&mut m, 0.5, Input { move_dir: [0.0, 1.0], ..armed() });
    run(&mut m, 1.5, Input { move_dir: [0.0, 1.0], sprint: true, ..armed() });
    assert!(m.sprinting, "sprinting");
    assert!(m.left_stance().wanted && !m.left_stance().animator, "off in the sprint");
    run(&mut m, 1.5, armed());
    assert!(!m.sprinting);
    assert!(m.left_stance().animator && m.left_stance().force_one_frame, "back at once after it");
    // A reload takes it off; the rest puts it back.
    m.left_stance_call(Call::Start(HandsStart::Other));
    assert!(!m.left_stance().animator);
    m.left_stance_call(Call::Start(HandsStart::Idle));
    assert!(m.left_stance().animator);
    // The inventory opened takes it off; shut, it stays off once a hands
    // operation has ever taken it off (the game's flag is never cleared).
    m.left_stance_call(Call::Inventory(true));
    assert!(!m.left_stance().animator);
    m.left_stance_call(Call::Inventory(false));
    assert!(!m.left_stance().animator);
    m.left_stance_call(Call::Start(HandsStart::Idle));
    assert!(m.left_stance().animator);
    // Lying down: off, and the key refused.
    m.step(DT, &Input { prone: true, ..armed() }, &Flat);
    run(&mut m, 2.0, armed());
    assert!(m.prone && !m.left_stance().animator);
    press(&mut m);
    assert!(m.left_stance().wanted, "refused lying down");
    // A start of blind fire with the stance wanted is dropped.
    m.step(DT, &Input { prone: true, ..armed() }, &Flat);
    run(&mut m, 2.0, armed());
    assert!(!m.prone);
    m.step(DT, &Input { blind_toggle: 1, ..armed() }, &Flat);
    assert_eq!(m.blind_fire(), 0, "blind fire refused with the left shoulder");
}
