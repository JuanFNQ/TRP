//! Tests of the game's key bindings: presses, holds, combinations and reserved keys.
use super::*;

const DT: f32 = 1.0 / 120.0;

fn source(keys: &[&str], wheel: Option<bool>) -> Source {
    Source { keys: keys.iter().map(|key| code(key).unwrap()).collect(), wheel }
}

fn group(name: &str, press: PressType, keys: &[&str]) -> KeyGroup {
    KeyGroup { name: name.to_owned(), variants: [source(keys, None), Source::default()], press }
}

fn controls(keys: Vec<KeyGroup>) -> Controls {
    Controls { keys, axes: Vec::new(), double_click: 0.3 }
}

/// Runs frames with the keys named held, and gives back the names of
/// the bindings given, and of those taken back with a minus before.
fn run(preset: &mut Preset, controls: &Controls, frames: usize, held: &[&str], wheel: i32) -> Vec<String> {
    let down: Vec<Code> = held.iter().map(|key| code(key).unwrap()).collect();
    let mut said = Vec::new();
    for frame in 0..frames {
        let heard = preset.update(&|key| down.contains(&key), if frame == 0 { wheel } else { 0 }, DT);
        said.extend(heard.given.iter().map(|group| controls.keys[*group].name.clone()));
        said.extend(heard.taken_back.iter().map(|group| format!("-{}", controls.keys[*group].name)));
    }
    said
}

#[test]
fn the_mouse_is_the_turn_s_or_the_free_look_s() {
    let controls = controls(vec![group(LOOK_BINDING, PressType::Release, &["Mouse2"]), group("Prone", PressType::Press, &["X"])]);
    let mut preset = Preset::new(&controls);
    let frame = |preset: &mut Preset, held: &[&str], moved: [f32; 2]| {
        let down: Vec<Code> = held.iter().map(|key| code(key).unwrap()).collect();
        let heard = preset.update(&|key| down.contains(&key), 0, DT);
        (preset.mouse(moved), heard.given.iter().map(|group| controls.keys[*group].name.clone()).collect::<Vec<_>>())
    };
    // With the key up the mouse turns the view.
    assert_eq!(frame(&mut preset, &[], [3.0, -1.0]).0, MouseHeard { turn: [3.0, -1.0], look: [0.0; 2] });
    // The key down: the first frame of travel is given to neither,
    // the next ones to the look.
    assert_eq!(frame(&mut preset, &["Mouse2"], [3.0, -1.0]).0, MouseHeard::default());
    assert_eq!(frame(&mut preset, &["Mouse2"], [3.0, -1.0]).0, MouseHeard { turn: [0.0; 2], look: [3.0, -1.0] });
    // Each axis by itself: a frame with no travel up or down lets
    // that axis go, and its next frame of travel is lost again.
    assert_eq!(frame(&mut preset, &["Mouse2"], [2.0, 0.0]).0, MouseHeard { turn: [0.0; 2], look: [2.0, 0.0] });
    assert_eq!(frame(&mut preset, &["Mouse2"], [2.0, 5.0]).0, MouseHeard { turn: [0.0; 2], look: [2.0, 0.0] });
    assert_eq!(frame(&mut preset, &["Mouse2"], [2.0, 5.0]).0, MouseHeard { turn: [0.0; 2], look: [2.0, 5.0] });
    // The key comes up: the binding is given (it is bound to its
    // release), and the mouse is the turn's again on that frame.
    let (mouse, given) = frame(&mut preset, &[], [1.0, 1.0]);
    assert_eq!((mouse, given), (MouseHeard { turn: [1.0, 1.0], look: [0.0; 2] }, vec![LOOK_BINDING.to_owned()]));
    // A binding with no keys has no free look.
    let mut bare = Preset::new(&self::controls(vec![KeyGroup { name: LOOK_BINDING.to_owned(), variants: [Source::default(), Source::default()], press: PressType::Release }]));
    bare.update(&|_| false, 0, DT);
    assert_eq!(bare.mouse([1.0, 1.0]).turn, [1.0, 1.0]);
}

/// The throws' bindings the game makes of the trigger's and the aim's
/// (`ControlSettingsGroup`'s generated keys): held whatever the aim's
/// way, with keys of their own, so the aim bound as a press still lets a
/// grenade go from below as its key comes up; made afresh, after the
/// rest, when the bindings change.
#[test]
fn the_throws_are_made_of_the_trigger_and_the_aim() {
    let mut controls = controls(vec![group("Shoot", PressType::Continuous, &["Mouse0"]), group("Aim", PressType::Press, &["Mouse1"])]);
    controls.generate();
    let names: Vec<&str> = controls.keys.iter().filter(|group| !group.reserved()).map(|group| group.name.as_str()).collect();
    assert_eq!(names, ["Shoot", "Aim", "HighThrow", "LowThrow"]);
    assert!(controls.keys[26..].iter().all(|group| group.press == PressType::Continuous && group.hidden()));
    let mut preset = Preset::new(&controls);
    let mut heard = run(&mut preset, &controls, 1, &["Mouse1"], 0);
    heard.sort();
    assert_eq!(heard, ["Aim", "LowThrow"]);
    assert_eq!(run(&mut preset, &controls, 5, &["Mouse1"], 0), Vec::<String>::new());
    assert_eq!(run(&mut preset, &controls, 1, &[], 0), ["-LowThrow"]);
    let mut heard = run(&mut preset, &controls, 1, &["Mouse0"], 0);
    heard.sort();
    assert_eq!(heard, ["HighThrow", "Shoot"]);
    let mut heard = run(&mut preset, &controls, 1, &[], 0);
    heard.sort();
    assert_eq!(heard, ["-HighThrow", "-Shoot"]);
    // The aim moved to another key: the throw from below with it.
    controls.keys[1].variants[0] = source(&["Mouse3"], None);
    controls.generate();
    assert_eq!(controls.keys.len(), 28, "the two, the twenty-four function keys', the two throws");
    assert_eq!(controls.keys[27].variants[0], source(&["Mouse3"], None));
}

/// The function keys are the game's own bindings, hidden from the
/// settings: a tap gives `F1` once the second press can no longer
/// come, two taps `DoubleF1` alone (the gestures' binds, the second
/// aggressive).
#[test]
fn the_function_keys_are_reserved_and_pressed_once_or_twice() {
    let mut controls = controls(vec![group("Jump", PressType::Release, &["Space"])]);
    controls.generate();
    controls.generate();
    assert_eq!(controls.keys.len(), 25, "made once, however often generated");
    assert!(controls.keys[1..].iter().all(|group| group.reserved() && group.hidden()));
    let mut preset = Preset::new(&controls);
    run(&mut preset, &controls, 6, &["F1"], 0);
    assert!(run(&mut preset, &controls, 24, &[], 0).is_empty());
    assert_eq!(run(&mut preset, &controls, 12, &[], 0), ["F1"]);
    run(&mut preset, &controls, 6, &["F12"], 0);
    run(&mut preset, &controls, 6, &[], 0);
    run(&mut preset, &controls, 6, &["F12"], 0);
    assert_eq!(run(&mut preset, &controls, 60, &[], 0), ["DoubleF12"]);
}

#[test]
fn names_and_numbers_go_both_ways() {
    for key in ["A", "Z", "Alpha0", "Alpha9", "Keypad5", "F1", "F12", "Mouse0", "Mouse4", "LeftAlt", "Tab", "SysReq", "BackQuote"] {
        assert_eq!(name(code(key).unwrap()), key);
    }
    assert_eq!((code("A"), code("Alpha2"), code("F1"), code("Mouse1"), code("LeftShift")), (Some(97), Some(50), Some(282), Some(324), Some(304)));
    assert_eq!(code("Nonsense"), None);
}

#[test]
fn a_press_comes_going_down_and_a_release_coming_up() {
    let controls = controls(vec![
        group("Prone", PressType::Press, &["X"]),
        group("Jump", PressType::Release, &["Space"]),
        group("Vaulting", PressType::Continuous, &["Space"]),
    ]);
    let mut preset = Preset::new(&controls);
    assert_eq!(run(&mut preset, &controls, 10, &["X"], 0), ["Prone"]);
    assert!(run(&mut preset, &controls, 3, &[], 0).is_empty());
    // The jump key held is the vault, given as it goes down; let go, it
    // is the jump and the vault taken back.
    assert_eq!(run(&mut preset, &controls, 10, &["Space"], 0), ["Vaulting"]);
    let mut let_go = run(&mut preset, &controls, 3, &[], 0);
    let_go.sort();
    assert_eq!(let_go, ["-Vaulting", "Jump"]);
}

#[test]
fn a_binding_that_does_not_hold_and_is_bound_to_be_held_waits() {
    let controls = controls(vec![group("ExamineWeapon", PressType::Continuous, &["L"])]);
    let mut preset = Preset::new(&controls);
    // Let go before the double click's time: nothing.
    assert!(run(&mut preset, &controls, 20, &["L"], 0).is_empty());
    assert!(run(&mut preset, &controls, 3, &[], 0).is_empty());
    // Held past it: given once.
    assert_eq!(run(&mut preset, &controls, 60, &["L"], 0), ["ExamineWeapon"]);
}

#[test]
fn the_longer_combination_has_the_key() {
    let mut keys = vec![
        group("Tactical", PressType::Press, &["T"]),
        group("CheckAmmo", PressType::Press, &["T", "LeftAlt"]),
        group("CheckChamber", PressType::Press, &["T", "LeftShift"]),
        group("Breath", PressType::Press, &["LeftAlt"]),
        group("LeanLockRight", PressType::Continuous, &["E"]),
        group("StepRight", PressType::Continuous, &["E", "LeftAlt"]),
    ];
    keys.push(group("Sprint", PressType::Continuous, &["LeftShift"]));
    let controls = controls(keys);
    let mut preset = Preset::new(&controls);
    assert_eq!(run(&mut preset, &controls, 5, &["T"], 0), ["Tactical"]);
    run(&mut preset, &controls, 3, &[], 0);
    // Alt first: the breath. Then T with it: the look at the magazine.
    assert_eq!(run(&mut preset, &controls, 5, &["LeftAlt"], 0), ["Breath"]);
    assert_eq!(run(&mut preset, &controls, 5, &["LeftAlt", "T"], 0), ["CheckAmmo"]);
    run(&mut preset, &controls, 3, &[], 0);
    // Alt and the lean key: the step aside, and no lean.
    run(&mut preset, &controls, 5, &["LeftAlt"], 0);
    assert_eq!(run(&mut preset, &controls, 5, &["LeftAlt", "E"], 0), ["StepRight"]);
    assert_eq!(run(&mut preset, &controls, 3, &["LeftAlt"], 0), ["-StepRight"]);
    run(&mut preset, &controls, 3, &[], 0);
    // The sprint's key is the sprint's whatever else has it.
    assert_eq!(run(&mut preset, &controls, 5, &["LeftShift"], 0), ["Sprint"]);
    assert_eq!(run(&mut preset, &controls, 5, &["LeftShift", "T"], 0), ["CheckChamber"]);
    assert_eq!(run(&mut preset, &controls, 3, &[], 0), ["-Sprint"]);
}

#[test]
fn the_crouch_key_used_with_the_wheel_does_not_crouch() {
    let mut stance_up = group("NextWalkPose", PressType::Press, &["C"]);
    stance_up.variants[0].wheel = Some(true);
    let mut speed_up = group("Next", PressType::Press, &[]);
    speed_up.variants[0].wheel = Some(true);
    let controls = controls(vec![group("Duck", PressType::Release, &["C"]), stance_up, speed_up]);
    let mut preset = Preset::new(&controls);
    // The wheel alone is the speed's.
    assert_eq!(run(&mut preset, &controls, 3, &[], 1), ["Next"]);
    // With the crouch key held it is the stance's, and the key coming
    // up afterwards does nothing.
    run(&mut preset, &controls, 5, &["C"], 0);
    assert_eq!(run(&mut preset, &controls, 5, &["C"], 1), ["NextWalkPose"]);
    assert!(run(&mut preset, &controls, 5, &[], 0).is_empty());
    // Tapped by itself it crouches as it comes up.
    assert!(run(&mut preset, &controls, 5, &["C"], 0).is_empty());
    assert_eq!(run(&mut preset, &controls, 5, &[], 0), ["Duck"]);
}

/// The magazine picker's keys (`NextMagazine`, `PreviousMagazine`: R
/// held and a notch), with R's two others as the install has them (the
/// reload on its coming up, the quick reload on a double click): a
/// notch while the double click is still waited for (0.3 s from R
/// going down) is nothing, the wait holding R; after it, each notch is
/// the picker's, and R coming up is then the reload's (the key the
/// picker takes its choice by).
#[test]
fn the_reload_key_held_with_the_wheel_is_the_magazine_picker_s() {
    let mut next = group("NextMagazine", PressType::Press, &["R"]);
    next.variants[0].wheel = Some(true);
    let mut previous = group("PreviousMagazine", PressType::Press, &["R"]);
    previous.variants[0].wheel = Some(false);
    let controls = controls(vec![group("ReloadWeapon", PressType::Release, &["R"]), group("QuickReloadWeapon", PressType::DoubleClick, &["R"]), next, previous]);
    let mut preset = Preset::new(&controls);
    // 0.1 s in: the double click's wait has R.
    run(&mut preset, &controls, 12, &["R"], 0);
    assert!(run(&mut preset, &controls, 5, &["R"], 1).is_empty());
    // 0.33 s in: the wait is over.
    run(&mut preset, &controls, 23, &["R"], 0);
    assert_eq!(run(&mut preset, &controls, 5, &["R"], 1), ["NextMagazine"]);
    assert_eq!(run(&mut preset, &controls, 5, &["R"], -1), ["PreviousMagazine"]);
    assert_eq!(run(&mut preset, &controls, 60, &[], 0), ["ReloadWeapon"]);
}

#[test]
fn a_double_click_on_the_same_key_makes_the_plain_press_wait() {
    let controls = controls(vec![group("ReloadWeapon", PressType::Release, &["R"]), group("QuickReloadWeapon", PressType::DoubleClick, &["R"])]);
    let mut preset = Preset::new(&controls);
    // A tap: nothing while the second press may still come (0.3 s from
    // the first), then the reload.
    assert!(run(&mut preset, &controls, 6, &["R"], 0).is_empty());
    assert!(run(&mut preset, &controls, 24, &[], 0).is_empty());
    assert_eq!(run(&mut preset, &controls, 12, &[], 0), ["ReloadWeapon"]);
    // Two taps: the fast reload, and no plain one after it.
    run(&mut preset, &controls, 6, &["R"], 0);
    run(&mut preset, &controls, 6, &[], 0);
    run(&mut preset, &controls, 6, &["R"], 0);
    assert_eq!(run(&mut preset, &controls, 60, &[], 0), ["QuickReloadWeapon"]);
    // Without the double click the reload comes as the key does.
    let alone = super::tests::controls(vec![group("ReloadWeapon", PressType::Release, &["R"])]);
    let mut preset = Preset::new(&alone);
    run(&mut preset, &alone, 6, &["R"], 0);
    assert_eq!(run(&mut preset, &alone, 2, &[], 0), ["ReloadWeapon"]);
}

#[test]
fn axes_take_their_keys_and_the_slow_lean_takes_the_walk_s() {
    let pair = |up: &[&str], down: &[&str]| [[source(up, None), source(down, None)], [Source::default(), Source::default()]];
    let controls = Controls {
        keys: vec![group("Breath", PressType::Press, &["LeftAlt"])],
        axes: vec![
            AxisGroup { name: "MoveX".into(), pairs: pair(&["D"], &["A"]) },
            AxisGroup { name: "LeanX".into(), pairs: pair(&["D", "LeftAlt"], &["A", "LeftAlt"]) },
        ],
        double_click: 0.3,
    };
    let mut preset = Preset::new(&controls);
    let axes = |preset: &mut Preset, held: &[&str]| {
        let down: Vec<Code> = held.iter().map(|key| code(key).unwrap()).collect();
        (0..4).map(|_| preset.update(&|key| down.contains(&key), 0, DT).axes).last().unwrap()
    };
    assert_eq!(axes(&mut preset, &["D"]), [1.0, 0.0]);
    assert_eq!(axes(&mut preset, &["A"]), [-1.0, 0.0]);
    // Both ways at once: the positive one is looked at first.
    assert_eq!(axes(&mut preset, &["A", "D"]), [1.0, 0.0]);
    assert_eq!(axes(&mut preset, &[]), [0.0, 0.0]);
    // With Alt the same key leans slowly and does not walk.
    axes(&mut preset, &["LeftAlt"]);
    assert_eq!(axes(&mut preset, &["LeftAlt", "A"]), [0.0, -1.0]);
    assert_eq!(axes(&mut preset, &["A"]), [-1.0, 0.0]);
}

#[test]
fn a_key_is_chosen_by_holding_and_letting_go() {
    let valid: Vec<Code> = ["Escape", "C", "T", "LeftAlt"].iter().map(|key| code(key).unwrap()).collect();
    let frame = |listening: &mut Listening, held: &[&str], wheel: i32| {
        let down: Vec<Code> = held.iter().map(|key| code(key).unwrap()).collect();
        listening.frame(&valid, &|key| down.contains(&key), wheel, true)
    };
    // Alt, then T with it, then T let go: the two, by their numbers.
    let mut listening = Listening::default();
    assert_eq!(frame(&mut listening, &[], 0), Chosen::Going);
    assert_eq!(frame(&mut listening, &["LeftAlt"], 0), Chosen::Going);
    assert_eq!(frame(&mut listening, &["LeftAlt", "T"], 0), Chosen::Going);
    assert_eq!(frame(&mut listening, &["LeftAlt"], 0), Chosen::Done(source(&["T", "LeftAlt"], None)));
    // A key and the wheel.
    let mut listening = Listening::default();
    frame(&mut listening, &["C"], 0);
    assert_eq!(frame(&mut listening, &["C"], -1), Chosen::Done(source(&["C"], Some(false))));
    // Escape drops it.
    let mut listening = Listening::default();
    frame(&mut listening, &["T"], 0);
    assert_eq!(frame(&mut listening, &["T", "Escape"], 0), Chosen::Dropped);
}

#[test]
fn the_same_keys_stay_on_two_bindings_only_pressed_differently() {
    let mut controls = controls(vec![
        group("ReloadWeapon", PressType::Release, &["R"]),
        group("Prone", PressType::Press, &["X"]),
        group("ShootingMode", PressType::Press, &["B"]),
        group("Interact", PressType::Continuous, &["F"]),
    ]);
    let keys = |controls: &Controls, name: &str| controls.keys[controls.group(name).unwrap()].variants[0].clone();
    let (reload, prone, mode, interact) = (0, 1, 2, 3);
    // Prone on R, pressed where the reload is let go: both keep it.
    assert!(!controls.key_chosen(prone, 0, source(&["R"], None)));
    assert_eq!((keys(&controls, "ReloadWeapon"), keys(&controls, "Prone")), (source(&["R"], None), source(&["R"], None)));
    // The fire mode on R too, pressed as prone is: the game asks how,
    // and until it is told prone keeps the key.
    assert!(controls.key_chosen(mode, 0, source(&["R"], None)));
    assert_eq!(keys(&controls, "Prone"), source(&["R"], None));
    // Told the same way: prone loses it.
    controls.press_chosen(mode, PressType::Press);
    assert!(keys(&controls, "Prone").is_empty());
    assert_eq!(keys(&controls, "ReloadWeapon"), source(&["R"], None));
    // A binding whose way is not chosen takes the key outright.
    assert!(!controls.key_chosen(interact, 0, source(&["R"], None)));
    assert!(keys(&controls, "ReloadWeapon").is_empty() && keys(&controls, "ShootingMode").is_empty());
    assert_eq!((keys(&controls, "Interact"), controls.keys[interact].press), (source(&["R"], None), PressType::Continuous));
    let _ = reload;
}
