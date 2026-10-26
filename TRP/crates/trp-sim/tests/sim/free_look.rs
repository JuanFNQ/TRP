//! The head's free look, measured with the real install's data and the
//! stock AKM (`docs/spec/free-look.md`). These pass trivially, with a
//! note, where no install is configured.
//!
//! Run with `cargo test -p trp-sim --test sim free_look:: -- --nocapture` for
//! the numbers.

use trp_data::engine::EngineSettings;
use trp_data::hard_settings::HardSettings;
use trp_data::player::HandsData;
use trp_data::unity::Assets;
use trp_sim::math::quat_rotate;
use trp_sim::{Flat, Hands, HandsInput, Input, Movement, ViewSettings};

const AKM: &str = "59d6088586f774275f37482f";
const DT: f32 = 1.0 / 60.0;
const SPINE: &str = "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3";

/// The hands and a character whose body is asked for its neck and head.
fn rig() -> Option<(Hands, Movement)> {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let db = crate::fixture::database().expect("database");
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

/// The user's recording of the free look (recorder 0.8.0, course 8 of
/// `local/traces/courses1`, 2026-10-15): its keys put through the game's
/// input manager by the install's bindings, its mouse axes given to the
/// look or the turn by them, the look's share to the head. On every frame
/// the head's rotation is the game's within a hundredth of a degree, and
/// the mouse's hold and the looking flag are the game's; on every frame
/// the view turned, the turn's share is what it turned by. Skipped where
/// the recording or the install is not there.
#[test]
fn the_recorded_free_look_is_followed() {
    use trp_sim::bindings::{code, Code, Preset, LOOK_BINDING};
    use trp_sim::head_look::{HeadLook, HeadLookInput, HeadLookSettings};
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses1/8-free-look/free-look.jsonl");
    let (Ok(text), Ok(install)) = (std::fs::read_to_string(path), crate::fixture::install().ok_or(())) else {
        eprintln!("skipped: no recording of course 8 or no install");
        return;
    };
    let controls = trp_sim::bindings::install_controls(&install);
    let axis = |name: &str| install.axis_sensitivity(name).unwrap_or(5.0);
    let (look, turn) = ([axis("LookX"), axis("LookY")], [axis("TurnX"), axis("TurnY")]);
    let sensitivity = install.player_settings().mouse_sensitivity.unwrap_or(1.0);
    let group = controls.keys.iter().position(|binding| binding.name == LOOK_BINDING).expect("the free look's binding");
    let mut preset = Preset::new(&controls);
    let mut head = HeadLook::new(HeadLookSettings::default());
    let mut held: Vec<Code> = Vec::new();
    let mut was: Option<[f32; 2]> = None;
    let (mut frames, mut turned, mut lost) = (0, 0, 0);
    for line in text.lines().skip(1).filter(|line| !line.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line).expect("a frame");
        let floats = |name: &str| -> Vec<f32> { v[name].as_array().map_or_else(Vec::new, |a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()) };
        let words = |name: &str| -> Vec<Code> { v[name].as_array().map_or_else(Vec::new, |a| a.iter().filter_map(|x| code(x.as_str()?)).collect()) };
        let flag = |name: &str| v[name].as_bool().unwrap_or(false);
        let (t, dt) = (v["t"].as_f64().unwrap_or(0.0) as f32, v["dt"].as_f64().unwrap_or(0.0) as f32);
        let (axes, view, game) = (floats("mouse_axes"), floats("look"), floats("head_rot"));
        held.extend(words("keys_down"));
        let up = words("keys_up");
        held.retain(|key| !up.contains(key));
        let heard = preset.update(&|key| held.contains(&key), 0, dt);
        let mouse = preset.mouse([axes[0], axes[1]]);
        lost += (0..2).filter(|&k| axes[k] != 0.0 && mouse.look[k] == 0.0 && mouse.turn[k] == 0.0).count();
        assert!(!flag("inventory_open"), "no screen is opened in this course");
        if heard.given.contains(&group) {
            head.reset();
        }
        head.frame(&HeadLookInput {
            travel: [mouse.look[0] * look[0], -mouse.look[1] * look[1]],
            dt,
            aiming: flag("aiming"),
            camera_pitch: was.map_or(view[1], |before| before[1]),
            sprinting: flag("sprinting"),
        });
        let model = head.rotation();
        assert!((0..2).all(|k| (model[k] - game[k]).abs() < 0.01), "at {t:.3} s the head is {model:?} here and {game:?} in the game");
        assert_eq!(head.control(), flag("mouse_look"), "the mouse's hold at {t:.3} s");
        assert_eq!(head.looking(), flag("head_looking"), "the looking flag at {t:.3} s");
        // The view's turn, with the sights down.
        if let Some(before) = was.filter(|_| !flag("aiming")) {
            let mut by = [view[0] - before[0], view[1] - before[1]];
            by[0] -= 360.0 * (by[0] / 360.0).round();
            let asked = [mouse.turn[0] * turn[0] * sensitivity, -mouse.turn[1] * turn[1] * sensitivity];
            for k in 0..2 {
                assert!((by[k] - asked[k]).abs() < 1e-3, "at {t:.3} s the view turned {by:?} and the model gave the turn {asked:?}");
                turned += usize::from(by[k] != 0.0);
            }
        }
        was = Some([view[0], view[1]]);
        frames += 1;
    }
    println!("{frames} frames followed; the view turned along an axis on {turned}; {lost} frames' travel along an axis went to neither");
    assert!(frames > 4000 && turned > 300 && lost > 100);
}
