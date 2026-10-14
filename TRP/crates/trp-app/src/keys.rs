//! The game's key bindings as the install has them (`Control.ini`, read by
//! `trp_data::Install::key_bindings` and `axis_bindings`), heard as the
//! game's input manager hears them (`trp_sim::bindings`;
//! `docs/spec/settings.md`): every key the app takes in play is one of
//! these, by the game's own name for what it does (`EGameKey`), pressed
//! the way the install has it pressed.
//!
//! The options tab changes them while the app runs (`options_view.rs`);
//! what it changes is kept in the app's own file, never in the install.

use std::collections::HashMap;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use trp_sim::bindings::{code, AxisGroup, Code, Controls, Heard, KeyGroup, MouseHeard, PressType, Preset, Source};

/// A key of the keyboard or a button of the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Board(KeyCode),
    Mouse(MouseButton),
}

/// What this install had for the bindings used here when it was read on
/// 2026-10-12 (Tarkov 0.16.9 under SPT 4.1), for an install whose
/// `Control.ini` is missing or lacks one: the name, the keys by the
/// engine's names for them (held together where joined by a plus,
/// `WheelUp` and `WheelDown` for a notch of the wheel), and how it is
/// pressed.
const AS_READ: [(&str, &str, &str); 42] = [
    ("Nidnod", "Mouse2", "Release"),
    ("LeanLockRight", "E", "Continuous"),
    ("LeanLockLeft", "Q", "Continuous"),
    ("Sprint", "LeftShift", "Press"),
    ("Jump", "Space", "Release"),
    ("Vaulting", "Space", "Continuous"),
    ("Duck", "C", "Release"),
    ("Prone", "X", "Press"),
    ("StepRight", "E+LeftAlt", "Continuous"),
    ("StepLeft", "Q+LeftAlt", "Continuous"),
    ("NextWalkPose", "C+WheelUp", "Press"),
    ("PreviousWalkPose", "C+WheelDown", "Press"),
    ("Next", "WheelUp", "Press"),
    ("Previous", "WheelDown", "Press"),
    ("Shoot", "Mouse0", "Continuous"),
    ("Aim", "Mouse1", "Press"),
    ("ReloadWeapon", "R", "Release"),
    ("QuickReloadWeapon", "R", "DoubleClick"),
    ("ShootingMode", "B", "Press"),
    ("CheckAmmo", "T+LeftAlt", "Press"),
    ("CheckChamber", "T+LeftShift", "Press"),
    ("FoldStock", "L+LeftAlt", "Press"),
    ("PrimaryWeaponFirst", "Alpha2", "Press"),
    ("PrimaryWeaponSecond", "Alpha3", "Press"),
    ("SecondaryWeapon", "Alpha1", "Press"),
    ("Breath", "LeftAlt", "Press"),
    ("Interact", "F", "Continuous"),
    ("ThrowItem", "Delete", "Press"),
    ("Inventory", "Tab", "Press"),
    ("Slot4", "Alpha4", "Release"),
    ("Slot5", "Alpha5", "Release"),
    ("Slot6", "Alpha6", "Release"),
    ("Slot7", "Alpha7", "Release"),
    ("Slot8", "Alpha8", "Release"),
    ("Slot9", "Alpha9", "Release"),
    ("Slot0", "Alpha0", "Release"),
    ("Knife", "U", "Press"),
    ("Walk", "CapsLock", "Press"),
    ("DropBackpack", "Z", "DoubleClick"),
    ("ForceAutoWeaponMode", "B", "DoubleClick"),
    ("CheckFireMode", "B+LeftAlt", "Press"),
    ("ExamineWeapon", "L", "Press"),
];
/// The same for the axes the keys push: the positive way's keys and the
/// negative's.
const AXES_AS_READ: [(&str, &str, &str); 3] = [("MoveX", "D", "A"), ("MoveY", "W", "S"), ("LeanX", "D+LeftAlt", "A+LeftAlt")];

/// The bindings the app acts on; the rest are the game's, kept and
/// changed by its rules, and do nothing here yet.
pub const BUILT: [&str; 43] = [
    "Nidnod",
    "LeanLockRight",
    "LeanLockLeft",
    "Sprint",
    "Jump",
    "Vaulting",
    "Duck",
    "Prone",
    "StepRight",
    "StepLeft",
    "NextWalkPose",
    "PreviousWalkPose",
    "Next",
    "Previous",
    "Shoot",
    "Aim",
    "ReloadWeapon",
    "ShootingMode",
    "CheckAmmo",
    "CheckChamber",
    "FoldStock",
    "PrimaryWeaponFirst",
    "PrimaryWeaponSecond",
    "SecondaryWeapon",
    "Breath",
    "Interact",
    "ThrowItem",
    "Inventory",
    "Slot4",
    "Slot5",
    "Slot6",
    "Slot7",
    "Slot8",
    "Slot9",
    "Slot0",
    "MoveX",
    "MoveY",
    "LeanX",
    "Walk",
    "DropBackpack",
    "ForceAutoWeaponMode",
    "CheckFireMode",
    "ExamineWeapon",
];

/// This engine's keys by the game engine's names for them (the letters,
/// the digits and the function keys are worked out).
const NAMED: [(KeyCode, &str); 46] = [
    (KeyCode::Tab, "Tab"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::Space, "Space"),
    (KeyCode::Enter, "Return"),
    (KeyCode::Escape, "Escape"),
    (KeyCode::Backspace, "Backspace"),
    (KeyCode::ShiftLeft, "LeftShift"),
    (KeyCode::ShiftRight, "RightShift"),
    (KeyCode::ControlLeft, "LeftControl"),
    (KeyCode::ControlRight, "RightControl"),
    (KeyCode::AltLeft, "LeftAlt"),
    (KeyCode::AltRight, "RightAlt"),
    (KeyCode::CapsLock, "CapsLock"),
    (KeyCode::Backquote, "BackQuote"),
    (KeyCode::PageUp, "PageUp"),
    (KeyCode::PageDown, "PageDown"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::ArrowUp, "UpArrow"),
    (KeyCode::ArrowDown, "DownArrow"),
    (KeyCode::ArrowLeft, "LeftArrow"),
    (KeyCode::ArrowRight, "RightArrow"),
    (KeyCode::Minus, "Minus"),
    (KeyCode::Equal, "Equals"),
    (KeyCode::BracketLeft, "LeftBracket"),
    (KeyCode::BracketRight, "RightBracket"),
    (KeyCode::Backslash, "Backslash"),
    (KeyCode::Semicolon, "Semicolon"),
    (KeyCode::Quote, "Quote"),
    (KeyCode::Comma, "Comma"),
    (KeyCode::Period, "Period"),
    (KeyCode::Slash, "Slash"),
    (KeyCode::NumpadDecimal, "KeypadPeriod"),
    (KeyCode::NumpadDivide, "KeypadDivide"),
    (KeyCode::NumpadMultiply, "KeypadMultiply"),
    (KeyCode::NumpadSubtract, "KeypadMinus"),
    (KeyCode::NumpadAdd, "KeypadPlus"),
    (KeyCode::NumpadEnter, "KeypadEnter"),
    (KeyCode::NumLock, "Numlock"),
    (KeyCode::ScrollLock, "ScrollLock"),
    (KeyCode::Pause, "Pause"),
    (KeyCode::PrintScreen, "SysReq"),
    (KeyCode::ContextMenu, "Menu"),
    (KeyCode::SuperLeft, "LeftWindows"),
    (KeyCode::SuperRight, "RightWindows"),
];

/// Every key this engine has a name of the game engine's for, with that
/// engine's number for it, lowest first.
fn engine_keys() -> Vec<(Code, Key)> {
    use KeyCode::*;
    const LETTERS: [KeyCode; 26] =
        [KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ];
    const DIGITS: [KeyCode; 10] = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    const KEYPAD: [KeyCode; 10] = [Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9];
    const FUNCTION: [KeyCode; 12] = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
    const MOUSE: [MouseButton; 5] = [MouseButton::Left, MouseButton::Right, MouseButton::Middle, MouseButton::Back, MouseButton::Forward];
    let run = |keys: &[KeyCode], first: Code| -> Vec<(Code, Key)> { keys.iter().enumerate().map(|(at, key)| (first + at as Code, Key::Board(*key))).collect() };
    let mut all = [run(&LETTERS, 97), run(&DIGITS, 48), run(&KEYPAD, 256), run(&FUNCTION, 282)].concat();
    all.extend(MOUSE.iter().enumerate().map(|(at, button)| (323 + at as Code, Key::Mouse(*button))));
    all.extend(NAMED.iter().filter_map(|(key, name)| Some((code(name)?, Key::Board(*key)))));
    all.sort_by_key(|(code, _)| *code);
    all
}

/// A key by the word a run nobody is at the keys of names it by
/// (`--tap`): a letter, a digit, or one of a few words.
pub fn tapped(word: &str) -> Option<Key> {
    let named = match word {
        "tab" => "Tab",
        "delete" => "Delete",
        "space" => "Space",
        "ctrl" => "LeftControl",
        "alt" => "LeftAlt",
        "shift" => "LeftShift",
        "caps" => "CapsLock",
        "enter" => "Return",
        "escape" => "Escape",
        "lmb" => "Mouse0",
        "rmb" => "Mouse1",
        "mmb" => "Mouse2",
        "f1" => "F1",
        _ => "",
    };
    let name = match word.as_bytes() {
        [letter @ b'a'..=b'z'] => (*letter as char).to_ascii_uppercase().to_string(),
        [digit @ b'0'..=b'9'] => format!("Alpha{}", *digit as char),
        _ => named.to_owned(),
    };
    let wanted = code(&name)?;
    engine_keys().into_iter().find(|(code, _)| *code == wanted).map(|(_, key)| key)
}

/// One variant of a binding from what was read of this install, as
/// `AS_READ` writes it.
fn as_read(keys: &str) -> Source {
    let mut source = Source::default();
    for name in keys.split('+').filter(|name| !name.is_empty()) {
        match name {
            "WheelUp" => source.wheel = Some(true),
            "WheelDown" => source.wheel = Some(false),
            name => source.keys.extend(code(name)),
        }
    }
    source
}

/// The install's bindings (`trp_sim::bindings::install_controls`), and for
/// a name it lacks what was read on the day this was written.
pub fn install_controls(install: &trp_data::Install) -> Controls {
    let mut controls = trp_sim::bindings::install_controls(install);
    for (name, read, press) in AS_READ {
        if !controls.keys.iter().any(|group| group.name == name) {
            controls.keys.push(KeyGroup { name: name.to_owned(), variants: [as_read(read), Source::default()], press: PressType::of(press).unwrap_or(PressType::Press) });
        }
    }
    // Of the axes, those the keys push: the mouse's four are the view's.
    for (name, positive, negative) in AXES_AS_READ {
        if !controls.axes.iter().any(|axis| axis.name == name) {
            controls.axes.push(AxisGroup { name: name.to_owned(), pairs: [[as_read(positive), as_read(negative)], Default::default()] });
        }
    }
    controls
}

/// What the engine itself multiplies the mouse's counts by on Windows
/// before any setting of a build's (the legacy input's own halving of a
/// raw count: not in the install, and not in the game's code; from a post
/// by one who read the engine's, discussions.unity.com/t/812499).
const ENGINE_MOUSE_SCALE: f32 = 0.5;
/// What this install had for the two figures a count is multiplied by
/// after that, for an install where one cannot be read: the engine axis's
/// own (`Mouse X` in the build's input settings) and the binding's
/// (`TurnX` in `Control.ini`).
const MOUSE_AS_READ: [f32; 2] = [0.05, 5.0];

/// Degrees the view turns for one count of the mouse before the player's
/// sensitivity (`docs/spec/settings.md`, "The mouse"): the engine's
/// halving, the build's figure for its mouse axis and the binding's own,
/// the last two read from the install. 0.125 in the install read, which
/// is what the community's converters list for the game.
pub fn degrees_a_count(install: &trp_data::Install) -> f32 {
    let axis = install.engine_axes().ok().and_then(|axes| axes.into_iter().find(|axis| axis.name == "Mouse X").map(|axis| axis.sensitivity));
    ENGINE_MOUSE_SCALE * axis.unwrap_or(MOUSE_AS_READ[0]) * install.axis_sensitivity("TurnX").unwrap_or(MOUSE_AS_READ[1])
}

/// Degrees the head's free look turns for one count of the mouse
/// (`docs/spec/free-look.md`): the same three figures with the look's own
/// binding in place of the turn's (`LookX`, 5 in the install read, so
/// 0.125 again). The player's sensitivity is not in it: the game hands
/// the look's axes to the head as they come.
pub fn look_degrees_a_count(install: &trp_data::Install) -> f32 {
    let axis = install.engine_axes().ok().and_then(|axes| axes.into_iter().find(|axis| axis.name == "Mouse X").map(|axis| axis.sensitivity));
    ENGINE_MOUSE_SCALE * axis.unwrap_or(MOUSE_AS_READ[0]) * install.axis_sensitivity("LookX").unwrap_or(MOUSE_AS_READ[1])
}

/// A key's name for the screen: `Alpha2` is `2`, the mouse's buttons and
/// the keys that come in a left and a right are shortened, the rest in
/// capitals. The app's words: the game's own names for keys are not read.
fn key_words(key: Code) -> String {
    let name = trp_sim::bindings::name(key);
    match name.as_str() {
        "Mouse0" => "LMB".into(),
        "Mouse1" => "RMB".into(),
        "Mouse2" => "MMB".into(),
        "Return" => "ENTER".into(),
        "BackQuote" => "`".into(),
        "SysReq" => "PRTSC".into(),
        name => {
            let short = name.strip_prefix("Alpha").unwrap_or(name);
            let sided = [("Left", "L "), ("Right", "R ")].into_iter().find_map(|(side, word)| {
                short.strip_prefix(side).filter(|rest| ["Shift", "Control", "Alt", "Windows"].contains(rest)).map(|rest| format!("{word}{}", rest.replace("Control", "Ctrl")))
            });
            sided.unwrap_or_else(|| short.to_owned()).to_uppercase()
        }
    }
}

/// A variant in words: `L ALT + T`, `C + WHEEL UP`; nothing for an empty one.
pub fn words(source: &Source) -> String {
    let wheel = source.wheel.map(|up| if up { "WHEEL UP" } else { "WHEEL DOWN" }.to_owned());
    // The keys that are held first, the key pressed last: modifiers lead.
    let mut keys = source.keys.clone();
    keys.sort_by_key(|key| !(303..=313).contains(key));
    keys.into_iter().map(key_words).chain(wheel).collect::<Vec<_>>().join(" + ")
}

/// The bindings, and what the keys came to on this frame.
#[derive(Resource)]
pub struct Bindings {
    /// As they stand.
    pub controls: Controls,
    /// As the install has them, to go back to.
    pub install: Controls,
    preset: Preset,
    keys: HashMap<Code, Key>,
    /// Every key that can be chosen for a binding, lowest number first.
    pub valid: Vec<Code>,
    heard: Heard,
    /// What the mouse's travel on this frame was given to, in its counts:
    /// the view's turn, or the head's free look.
    mouse: MouseHeard,
    index: HashMap<String, usize>,
    captions: HashMap<String, String>,
    /// Nothing is heard: a key is being chosen for a binding.
    pub muted: bool,
    /// How many times the bindings have been changed: what is drawn of
    /// them is drawn afresh when this moves.
    pub version: u32,
}

impl Bindings {
    pub fn of(install: &trp_data::Install) -> Self {
        let controls = install_controls(install);
        let keys = engine_keys();
        let mut bindings = Self {
            install: controls.clone(),
            preset: Preset::new(&controls),
            controls,
            valid: keys.iter().map(|(code, _)| *code).collect(),
            keys: keys.into_iter().collect(),
            heard: Heard::default(),
            mouse: MouseHeard::default(),
            index: HashMap::new(),
            captions: HashMap::new(),
            muted: false,
            version: 0,
        };
        bindings.changed();
        bindings
    }

    /// The bindings have been changed: the combinations are made afresh,
    /// as the game makes them when its settings are saved.
    pub fn changed(&mut self) {
        self.version += 1;
        self.preset = Preset::new(&self.controls);
        self.heard = Heard::default();
        self.index = self.controls.keys.iter().enumerate().map(|(at, group)| (group.name.clone(), at)).collect();
        let first = |group: &KeyGroup| group.variants.iter().find(|variant| !variant.is_empty()).map(words).unwrap_or_default();
        self.captions = self.controls.keys.iter().map(|group| (group.name.clone(), first(group))).collect();
    }

    /// The words for a binding's first key (`2`, `TAB`).
    pub fn caption(&self, name: &str) -> &str {
        self.captions.get(name).map_or("", String::as_str)
    }

    /// Whether the binding is given on this frame: the game's command
    /// for it comes.
    pub fn fired(&self, name: &str) -> bool {
        self.index.get(name).is_some_and(|group| self.heard.given.contains(group))
    }

    /// Whether a binding that holds is taken back on this frame: its keys
    /// came up, where it is bound to be held.
    pub fn ended(&self, name: &str) -> bool {
        self.index.get(name).is_some_and(|group| self.heard.taken_back.contains(group))
    }

    /// The mouse's travel on this frame as the manager gave it out, in
    /// counts across and down: to the turn, or to the head's free look.
    pub fn mouse(&self) -> MouseHeard {
        self.mouse
    }

    /// An axis's push on this frame, -1 to 1.
    pub fn axis(&self, name: &str) -> f32 {
        self.controls.axes.iter().position(|axis| axis.name == name).and_then(|at| self.heard.axes.get(at)).copied().unwrap_or(0.0)
    }

    /// Whether a key is down, by the game engine's number for it.
    pub fn held(&self, code: Code, keys: &ButtonInput<KeyCode>, buttons: &ButtonInput<MouseButton>) -> bool {
        match self.keys.get(&code) {
            Some(Key::Board(key)) => keys.pressed(*key),
            Some(Key::Mouse(button)) => buttons.pressed(*button),
            None => false,
        }
    }

    /// The guide to the keys as they are bound now, for the panel at the
    /// top left of the display (`hud.rs`): sections, each a list of lines
    /// with the keys and what they do. A binding pressed twice says so
    /// after its key, and one of those that hold says whether it is held
    /// or one press on and one press off, as the install has it.
    pub fn guide(&self) -> Vec<(&'static str, Vec<(String, String)>)> {
        let press = |name: &str| self.index.get(name).map(|at| self.controls.keys[*at].press);
        let key = |name: &str| match (self.caption(name), press(name)) {
            ("", _) => "-".to_owned(),
            (words, Some(PressType::DoubleClick)) => format!("{words} x2"),
            (words, _) => words.to_owned(),
        };
        let pair = |first: &str, second: &str| format!("{} / {}", key(first), key(second));
        let axis = |name: &str| {
            let [up, down] = self.controls.axes.iter().find(|axis| axis.name == name).map(|axis| axis.pairs[0].clone()).unwrap_or_default();
            let said = |source: &Source| if source.is_empty() { "-".to_owned() } else { words(source) };
            (said(&up), said(&down))
        };
        // Held, or a press on and a press off: only the bindings that hold
        // have the two ways.
        let how = |name: &str, does: &str| match press(name) {
            Some(PressType::Continuous) => format!("{does} (hold)"),
            Some(PressType::Press) => format!("{does} (press on, press off)"),
            _ => does.to_owned(),
        };
        let line = |keys: String, does: &str| (keys, does.to_owned());
        let ((right, left), (forward, back), (lean_right, lean_left)) = (axis("MoveX"), axis("MoveY"), axis("LeanX"));
        vec![
            (
                "MOVEMENT",
                vec![
                    line(format!("{forward} {left} {back} {right}"), "Move"),
                    (key("Sprint"), how("Sprint", "Sprint")),
                    (key("Walk"), how("Walk", "Walk slowly")),
                    line(key("Jump"), "Jump; held with forward, vault"),
                    line(key("Duck"), "Crouch or stand"),
                    line(key("Prone"), "Lie down or get up"),
                    (pair("LeanLockLeft", "LeanLockRight"), how("LeanLockLeft", "Lean")),
                    line(format!("{lean_left} / {lean_right}"), "Lean slowly; it stays"),
                    (pair("StepLeft", "StepRight"), how("StepLeft", "Step aside")),
                    line(pair("Next", "Previous"), "Walking speed"),
                    line(pair("NextWalkPose", "PreviousWalkPose"), "Stance"),
                    line(format!("{} (hold)", key("Nidnod")), "Look around with the head"),
                    line(key("DropBackpack"), "Drop the backpack"),
                ],
            ),
            (
                "WEAPON",
                vec![
                    line(key("Shoot"), "Fire"),
                    (key("Aim"), how("Aim", "Aim")),
                    line(key("Breath"), "Hold breath, aiming"),
                    line(key("ReloadWeapon"), "Reload"),
                    line(key("CheckAmmo"), "Check the magazine"),
                    line(key("CheckChamber"), "Check the chamber"),
                    line(key("ShootingMode"), "Next fire mode"),
                    line(key("ForceAutoWeaponMode"), "Automatic fire"),
                    line(key("CheckFireMode"), "Check the fire mode"),
                    line(key("ExamineWeapon"), "Look the weapon over"),
                    line(key("FoldStock"), "Fold the stock"),
                    line(format!("{} / {} / {}", key("PrimaryWeaponFirst"), key("PrimaryWeaponSecond"), key("SecondaryWeapon")), "Sling / back / holster"),
                ],
            ),
            (
                "WORLD",
                vec![
                    line(key("Interact"), "Do the menu's chosen line"),
                    line(pair("Next", "Previous"), "Choose a line, with a menu up"),
                    line(key("Inventory"), "Inventory; its last tab, options"),
                    line(key("ThrowItem"), "Put down, in the inventory"),
                ],
            ),
        ]
    }
}

/// Runs the game's input manager for this frame: which bindings the keys
/// and the wheel give, and which they take back. First in the frame, as
/// the game's own runs before what listens to it.
pub fn hear(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    scripted: Res<ScriptedMouse>,
    options: Res<crate::Options>,
    time: Res<Time<Real>>,
    mut bindings: ResMut<Bindings>,
) {
    let bindings = &mut *bindings;
    let map = &bindings.keys;
    // While a key is being chosen every key counts as up, so that what
    // was held is let go of and nothing new is heard.
    let muted = bindings.muted;
    let held = |code: Code| {
        !muted
            && match map.get(&code) {
                Some(Key::Board(key)) => keys.pressed(*key),
                Some(Key::Mouse(button)) => buttons.pressed(*button),
                None => false,
            }
    };
    // The game takes the wheel for pressed on a frame whose axis reads
    // exactly one either way (`InputAxis.Update`), and the install's
    // input settings have that axis read one for a notch (`Mouse Scroll`,
    // one a count): one notch in a frame is a press, and two in the same
    // frame are none.
    let wheel = if muted { 0 } else { notch(scroll.delta.y) };
    bindings.heard = bindings.preset.update(&held, wheel, time.delta_secs());
    // The mouse's travel, which is the turn's unless the free look's keys
    // give it to the head (`Preset::mouse`). A run nobody is at the keys
    // of has the travel it was told (`--mouse`) in place of the mouse's.
    let travel = if options.unattended() { scripted.0.unwrap_or_default() } else { motion.delta };
    bindings.mouse = if muted { MouseHeard::default() } else { bindings.preset.mouse([travel.x, travel.y]) };
}

/// The mouse's travel on this frame in a run nobody is at the keys of:
/// what `--mouse` told, and nothing on the frames it did not name. A run
/// someone plays never looks at it: its mouse is the real one.
#[derive(Resource, Default)]
pub struct ScriptedMouse(pub Option<Vec2>);

/// The wheel as the game's input manager takes it: a notch one way or the
/// other on a frame whose turn is exactly one notch, and nothing on a
/// frame with more or less (settings spec, "How a key becomes a command").
pub fn notch(turned: f32) -> i32 {
    if turned == 1.0 {
        1
    } else if turned == -1.0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_notches_in_a_frame_are_none() {
        assert_eq!([notch(1.0), notch(-1.0), notch(0.0), notch(2.0), notch(-3.0), notch(0.5)], [1, -1, 0, 0, 0, 0]);
    }

    #[test]
    fn every_key_named_here_has_the_game_engine_s_number() {
        for (key, name) in NAMED {
            assert!(code(name).is_some(), "{key:?} is named {name}, which the engine has no number for");
        }
        let all = engine_keys();
        assert!(all.windows(2).all(|pair| pair[0].0 < pair[1].0), "each key once, lowest first");
        // What was read of this install is all in keys known here.
        for (name, read, press) in AS_READ {
            assert!(!as_read(read).is_empty() && PressType::of(press).is_some(), "{name}");
            assert!(read.split('+').all(|key| key.starts_with("Wheel") || code(key).is_some()), "{name}: {read}");
        }
    }

    #[test]
    fn a_variant_reads_with_the_keys_held_first() {
        assert_eq!(words(&as_read("T+LeftAlt")), "L ALT + T");
        assert_eq!(words(&as_read("C+WheelUp")), "C + WHEEL UP");
        assert_eq!(words(&as_read("Alpha2")), "2");
        assert_eq!(words(&as_read("Mouse1+LeftControl")), "L CTRL + RMB");
        assert_eq!(words(&Source::default()), "");
    }
}
