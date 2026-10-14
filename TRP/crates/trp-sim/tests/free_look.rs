//! The head's free look, measured with the real install's data and the
//! stock AKM (`docs/spec/free-look.md`). These pass trivially, with a
//! note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test free_look -- --nocapture` for
//! the numbers.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::unity::Assets;
use trp_data::{Database, Install};
use trp_sim::math::quat_rotate;
use trp_sim::{Flat, Hands, HandsInput, Input, Movement, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;
const SPINE: &str = "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3";

/// The hands and a character whose body is asked for its neck and head.
fn rig() -> Option<(Hands, Movement)> {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set EFT_DIR or local/eft_dir.txt)");
        return None;
    };
    let db = Database::load(&install).expect("database");
    let hard = HardSettings::load(&install).expect("settings object");
    let engine = EngineSettings::load(&install).expect("engine settings");
    let mut assets = Assets::new(install).expect("manifest");
    let weapon = trp_data::weapon::assemble(&mut assets, &db, AKM).expect("weapon");
    let data = HandsData::load(&mut assets).expect("player prefab");
    let hands = Hands::new(
        trp_sim::hands_settings(data, &db, &hard, &engine, &weapon, ViewSettings::default()).expect("hands settings"),
    );
    let drawn = [format!("{SPINE}/Base HumanNeck"), format!("{SPINE}/Base HumanNeck/Base HumanHead")];
    let body = trp_sim::body::BodyData::load_with(&mut assets, &drawn).expect("body animator");
    let movement = Movement::new(
        trp_sim::load_movement_settings(&mut assets, &db, &hard, &engine, 0.0).expect("movement"),
        trp_sim::Body::new(&body, weapon.cell_width as f32),
    );
    Some((hands, movement))
}

fn angle(q: [f32; 4]) -> f32 {
    2.0 * q[3].abs().min(1.0).acos().to_degrees()
}

/// The turn from one rotation to another, in degrees.
fn between(a: [f32; 4], b: [f32; 4]) -> f32 {
    let back = [-a[0], -a[1], -a[2], a[3]];
    angle(trp_sim::math::quat_mul(back, b))
}

fn still() -> HandsInput {
    HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..HandsInput::default() }
}

/// The install's limits, and the head following the look's axes: turned,
/// held while the key is, and back when it comes up, the neck and the
/// head each by half of it.
#[test]
fn the_head_turns_and_comes_back() {
    let Some((_, mut m)) = rig() else { return };
    let limits = m.settings.head_look;
    println!("the head turns {:?} across and {:?} up and down, {} of the first with the sights up", limits.across, limits.down, limits.aiming);
    assert_eq!((limits.across, limits.down), ([-40.0, 40.0], [-50.0, 20.0]));
    assert!((limits.aiming - 0.7).abs() < 1e-6);

    for _ in 0..30 {
        m.step(DT, &Input::default(), &Flat);
    }
    let rested = m.body.drawn_bones();
    assert_eq!(rested.len(), 2, "the neck and the head were asked for");
    // Thirty degrees to the right and ten up, over twenty frames.
    for _ in 0..20 {
        m.step(DT, &Input { head_look: [1.5, -0.5], ..Input::default() }, &Flat);
    }
    assert!(m.head_look.control() && m.head_look.looking());
    assert_eq!(m.head_look.rotation(), [-10.0, -30.0, 0.0]);
    for _ in 0..60 {
        m.step(DT, &Input::default(), &Flat);
    }
    assert_eq!(m.head_look.rotation(), [-10.0, -30.0, 0.0], "held while the key is");
    let turned = m.body.drawn_bones();
    let (neck, head) = (between(rested[0].1, turned[0].1), between(rested[1].1, turned[1].1));
    println!("the head turned 30 degrees across and 10 up: the neck's bone turns {neck:.2} degrees and the head's {head:.2}");
    // Half of the turn each; the idle animation moves them a little too.
    let half = (15.0f32 * 15.0 + 5.0 * 5.0).sqrt();
    assert!((neck - half).abs() < 1.5 && (head - half).abs() < 1.5, "{neck} {head} for {half}");

    // The key comes up.
    m.step(DT, &Input { head_reset: true, ..Input::default() }, &Flat);
    assert!(m.head_look.resetting() && !m.head_look.control());
    let mut frames = 1;
    while m.head_look.rotation() != [0.0; 3] {
        m.step(DT, &Input::default(), &Flat);
        frames += 1;
        assert!(frames < 300);
    }
    println!("back to nothing {:.2} s after the key came up", frames as f32 * DT);
    let back = m.body.drawn_bones();
    assert!(between(rested[0].1, back[0].1) < 1.5 && between(rested[1].1, back[1].1) < 1.5);

    // With a screen open the head is handed nothing, and stays.
    for _ in 0..10 {
        m.step(DT, &Input { head_look: [1.0, 1.0], ..Input::default() }, &Flat);
    }
    let held = m.head_look.rotation();
    for _ in 0..30 {
        m.step(DT, &Input { screen: true, head_look: [1.0, 1.0], head_reset: true, ..Input::default() }, &Flat);
    }
    assert_eq!(m.head_look.rotation(), held);
}

/// The camera takes the head's turn on top of everything else, and the
/// aimed field of view gives way to the hip's while the mouse has the head.
#[test]
fn the_camera_takes_the_head_s_turn() {
    let Some((mut hands, _)) = rig() else { return };
    let animated = trp_sim::math::quat_euler([-90.0, 0.0, 0.0]);
    for _ in 0..60 {
        hands.advance(DT, &still());
        hands.frame(DT, animated, None);
    }
    let ahead = |hands: &mut Hands| quat_rotate(hands.frame(DT, animated, None).rotation, [0.0, 0.0, 1.0]);
    let rest = ahead(&mut hands);
    // Thirty degrees to the right: the head's turn across is negative.
    hands.set_head_rotation([0.0, -30.0, 0.0], true, false);
    hands.advance(DT, &still());
    let right = ahead(&mut hands);
    let turned = (rest[0] * right[2] - rest[2] * right[0]).atan2(rest[0] * right[0] + rest[2] * right[2]).to_degrees();
    println!("the head 30 degrees to the right: the camera looks {:.2} degrees to the right of where it did, {:.2} up", -turned, (right[1] - rest[1]).asin().to_degrees());
    assert!((turned + 30.0).abs() < 1.0, "{turned}");
    // Ten degrees down.
    hands.set_head_rotation([10.0, 0.0, 0.0], true, false);
    hands.advance(DT, &still());
    let down = ahead(&mut hands);
    println!("the head 10 degrees down: the camera's line goes from {:.3} to {:.3} in height", rest[1], down[1]);
    assert!(((rest[1] - down[1]).asin().to_degrees() - 10.0).abs() < 1.0, "{down:?}");
    hands.set_head_rotation([0.0; 3], false, false);

    // The sights up: the field of view goes to the aimed one; the mouse
    // taking the head puts the hip's back, and its letting go the aimed.
    let hip = hands.settings.field_of_view;
    let aiming = HandsInput { aiming: true, ..still() };
    let settle = |hands: &mut Hands, input: &HandsInput| {
        let mut fov = 0.0;
        for _ in 0..180 {
            hands.advance(DT, input);
            fov = hands.frame(DT, animated, None).field_of_view;
        }
        fov
    };
    let aimed = settle(&mut hands, &aiming);
    hands.set_head_rotation([0.0, -10.0, 0.0], true, true);
    let looking = settle(&mut hands, &aiming);
    hands.set_head_rotation([0.0; 3], false, true);
    let again = settle(&mut hands, &aiming);
    println!("field of view: {hip} from the hip, {aimed:.1} aimed, {looking:.1} aimed with the mouse on the head, {again:.1} once it lets go");
    assert!((aimed - (hip - 15.0)).abs() < 0.05 && (looking - hip).abs() < 0.05 && (again - aimed).abs() < 0.05);
    // And the sights going down while the mouse has the head leave it.
    hands.set_head_rotation([0.0, -10.0, 0.0], true, true);
    settle(&mut hands, &aiming);
    assert!((settle(&mut hands, &still()) - hip).abs() < 0.05);
}
