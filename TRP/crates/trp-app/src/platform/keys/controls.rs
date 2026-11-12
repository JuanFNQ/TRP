//! The install's controls, the mouse's scale and the keys' words.

use super::*;

/// The install's bindings (`trp_sim::bindings::install_controls`), and for
/// a name it lacks what was read on the day this was written.
pub fn install_controls(install: &trp_data::Install) -> Controls {
    let mut controls = trp_sim::base::bindings::install_controls(install);
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
    let name = trp_sim::base::bindings::name(key);
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

/// A key's name as the game's notices give it (`KeyTools.GetKeyNameAlias`):
/// Unity's name for it, a few shortened or put as their sign. (The mouse's
/// three buttons are the game's words for them, `Mouse0` and on: left as
/// Unity's names.)
pub(super) fn key_alias(name: &str) -> String {
    let short = match name {
        "Escape" => "Esc",
        "DoubleQuote" => "\"",
        "Ampersand" => "&",
        "LeftParen" => "(",
        "RightParen" => ")",
        "Asterisk" => "*",
        "Plus" => "+",
        "Comma" => ",",
        "Minus" => "-",
        "Slash" | "KeypadDivide" => "/",
        "Semicolon" => ";",
        "Less" => "<",
        "Greater" => ">",
        "Backspace" => "BSpace",
        "None" => "",
        "KeypadMultiply" => "Num *",
        "KeypadMinus" => "Num -",
        "KeypadPlus" => "Num +",
        "KeypadEnter" => "\u{23ce}",
        "KeypadEquals" => "=",
        "UpArrow" => "\u{2191}",
        "DownArrow" => "\u{2193}",
        "RightArrow" => "\u{2192}",
        "LeftArrow" => "\u{2190}",
        "Insert" => "Ins",
        "PageUp" => "PgUp",
        "PageDown" => "PgDn",
        "Numlock" => "NLock",
        "CapsLock" => "Caps",
        "ScrollLock" => "SLock",
        "RightShift" => "RShift",
        "LeftShift" => "LShift",
        "RightControl" => "RCtrl",
        "LeftControl" => "LCtrl",
        "RightAlt" => "RAlt",
        "LeftAlt" => "LAlt",
        "LeftMeta" | "LeftWindows" => "LWin",
        "RightMeta" | "RightWindows" => "RWin",
        "SysReq" => "PrtScn",
        "Delete" => "Del",
        "LeftBracket" => "[",
        "Backslash" => "\\",
        "RightBracket" => "]",
        "BackQuote" => "`",
        _ => {
            if let Some(digit) = name.strip_prefix("Alpha").filter(|rest| rest.len() == 1) {
                return digit.to_owned();
            }
            if let Some(digit) = name.strip_prefix("Keypad").filter(|rest| rest.len() == 1 && rest.chars().all(|c| c.is_ascii_digit())) {
                return format!("Kp {digit}");
            }
            if let Some(number) = name.strip_prefix("JoystickButton") {
                return format!("JB {number}");
            }
            name
        }
    };
    short.to_owned()
}

/// A variant in words: `L ALT + T`, `C + WHEEL UP`; nothing for an empty one.
pub fn words(source: &Source) -> String {
    let wheel = source.wheel.map(|up| if up { "WHEEL UP" } else { "WHEEL DOWN" }.to_owned());
    // The keys that are held first, the key pressed last: modifiers lead.
    let mut keys = source.keys.clone();
    keys.sort_by_key(|key| !(303..=313).contains(key));
    keys.into_iter().map(key_words).chain(wheel).collect::<Vec<_>>().join(" + ")
}
