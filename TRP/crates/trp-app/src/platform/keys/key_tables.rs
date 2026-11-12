//! The keys, the tables of keys as read and as built, and taps.

use super::*;

/// A key of the keyboard or a button of the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Board(KeyCode),
    Mouse(MouseButton),
}

/// What this install had for the bindings used here when it was read
/// (Tarkov 0.16.9 under SPT 4.1), for an install whose
/// `Control.ini` is missing or lacks one: the name, the keys by the
/// engine's names for them (held together where joined by a plus,
/// `WheelUp` and `WheelDown` for a notch of the wheel), and how it is
/// pressed.
pub(super) const AS_READ: [(&str, &str, &str); 44] = [
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
    ("UnloadMagazine", "R+LeftAlt", "Press"),
    ("ChamberUnload", "R+LeftControl", "Press"),
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
pub(super) const AXES_AS_READ: [(&str, &str, &str); 3] = [("MoveX", "D", "A"), ("MoveY", "W", "S"), ("LeanX", "D+LeftAlt", "A+LeftAlt")];

/// The bindings the app acts on; the rest are the game's, kept and
/// changed by its rules, and do nothing here yet.
pub const BUILT: [&str; 56] = [
    "ThrowGrenade",
    "Knife",
    "QuickKnife",
    "QuickSecondaryWeapon",
    "ToggleGoggles",
    "QuickReloadWeapon",
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
    "UnloadMagazine",
    "ChamberUnload",
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
    "BlindShootAbove",
    "BlindShootRight",
    "WeaponMounting",
    "ToggleBipods",
    "LeftStance",
];

/// This engine's keys by the game engine's names for them (the letters,
/// the digits and the function keys are worked out).
pub(super) const NAMED: [(KeyCode, &str); 46] = [
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
pub(super) fn engine_keys() -> Vec<(Code, Key)> {
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
/// (`--tap`): a letter, a digit, a keypad's digit (`num0` to `num9`,
/// `numenter`), or one of a few words (among them the arrows `left`,
/// `right`, `up` and `down`, and `home`, `end` and `backspace`, which a
/// text field hears).
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
        "mouse3" => "Mouse3",
        "mouse4" => "Mouse4",
        "f1" => "F1",
        "backquote" => "BackQuote",
        "f7" => "F7",
        "f8" => "F8",
        "f9" => "F9",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        "home" => "Home",
        "end" => "End",
        "left" => "LeftArrow",
        "right" => "RightArrow",
        "up" => "UpArrow",
        "down" => "DownArrow",
        "backspace" => "Backspace",
        "numenter" => "KeypadEnter",
        _ => "",
    };
    let name = match word.as_bytes() {
        [letter @ b'a'..=b'z'] => (*letter as char).to_ascii_uppercase().to_string(),
        [digit @ b'0'..=b'9'] => format!("Alpha{}", *digit as char),
        [b'n', b'u', b'm', digit @ b'0'..=b'9'] => format!("Keypad{}", *digit as char),
        _ => named.to_owned(),
    };
    let wanted = code(&name)?;
    engine_keys().into_iter().find(|(code, _)| *code == wanted).map(|(_, key)| key)
}

/// One variant of a binding from what was read of this install, as
/// `AS_READ` writes it.
pub(super) fn as_read(keys: &str) -> Source {
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
