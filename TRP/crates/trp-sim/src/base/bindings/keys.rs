//! Key codes and their names, press types and key presses.


/// The engine's number for a key.
pub type Code = u16;

/// The key that drops a key being chosen.
pub const ESCAPE: Code = 27;

/// The engine's number for `F1`; `F2` to `F15` follow it.
pub(super) const FUNCTION_KEYS: Code = 282;

/// The engine's names for the keys that are not one of a run (the letters,
/// the digits, the keypad's digits, the function keys and the mouse's
/// buttons are worked out), each with its number: the engine's published
/// key codes.
const NAMED: [(&str, Code); 49] = [
    ("Backspace", 8),
    ("Tab", 9),
    ("Return", 13),
    ("Pause", 19),
    ("Escape", 27),
    ("Space", 32),
    ("Quote", 39),
    ("Comma", 44),
    ("Minus", 45),
    ("Period", 46),
    ("Slash", 47),
    ("Semicolon", 59),
    ("Equals", 61),
    ("LeftBracket", 91),
    ("Backslash", 92),
    ("RightBracket", 93),
    ("BackQuote", 96),
    ("Delete", 127),
    ("KeypadPeriod", 266),
    ("KeypadDivide", 267),
    ("KeypadMultiply", 268),
    ("KeypadMinus", 269),
    ("KeypadPlus", 270),
    ("KeypadEnter", 271),
    ("KeypadEquals", 272),
    ("UpArrow", 273),
    ("DownArrow", 274),
    ("RightArrow", 275),
    ("LeftArrow", 276),
    ("Insert", 277),
    ("Home", 278),
    ("End", 279),
    ("PageUp", 280),
    ("PageDown", 281),
    ("Numlock", 300),
    ("CapsLock", 301),
    ("ScrollLock", 302),
    ("RightShift", 303),
    ("LeftShift", 304),
    ("RightControl", 305),
    ("LeftControl", 306),
    ("RightAlt", 307),
    ("LeftAlt", 308),
    ("LeftWindows", 311),
    ("RightWindows", 312),
    ("AltGr", 313),
    ("Print", 316),
    ("SysReq", 317),
    ("Menu", 319),
];

/// The number of a key by the engine's name for it.
pub fn code(name: &str) -> Option<Code> {
    let run = |prefix: &str, first: Code, count: Code, from: Code| {
        name.strip_prefix(prefix).and_then(|n| n.parse::<Code>().ok()).filter(|n| (from..from + count).contains(n)).map(|n| first + n - from)
    };
    if let [letter @ b'A'..=b'Z'] = name.as_bytes() {
        return Some(97 + (letter - b'A') as Code);
    }
    run("Alpha", 48, 10, 0)
        .or_else(|| run("Keypad", 256, 10, 0))
        .or_else(|| run("Mouse", 323, 7, 0))
        .or_else(|| run("F", 282, 15, 1))
        .or_else(|| NAMED.iter().find(|(known, _)| *known == name).map(|(_, code)| *code))
}

/// The engine's name for a key by its number.
pub fn name(code: Code) -> String {
    match code {
        97..=122 => ((b'A' + (code - 97) as u8) as char).to_string(),
        48..=57 => format!("Alpha{}", code - 48),
        256..=265 => format!("Keypad{}", code - 256),
        323..=329 => format!("Mouse{}", code - 323),
        282..=296 => format!("F{}", code - 281),
        _ => NAMED.iter().find(|(_, known)| *known == code).map_or_else(|| format!("Key{code}"), |(name, _)| (*name).to_owned()),
    }
}

/// How a binding is pressed (`EPressType`), in the game's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressType {
    /// On the key coming up.
    Release,
    /// Held: a binding that holds is given as it goes down and taken back
    /// as it comes up; any other is given once it has been held for the
    /// double click's time.
    Continuous,
    /// On the key going down.
    Press,
    /// On the second of two presses within the double click's time.
    DoubleClick,
}

impl PressType {
    pub const ALL: [Self; 4] = [Self::Release, Self::Continuous, Self::Press, Self::DoubleClick];

    pub fn name(self) -> &'static str {
        match self {
            Self::Release => "Release",
            Self::Continuous => "Continuous",
            Self::Press => "Press",
            Self::DoubleClick => "DoubleClick",
        }
    }

    pub fn of(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|press| press.name() == name)
    }
}

/// A key as the manager's combinations see it on a frame (`EKeyPress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyPress {
    #[default]
    None,
    Down,
    Hold,
    Up,
}
