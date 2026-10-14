//! The game's keys as its input manager hears them, and its rules for
//! changing them (`docs/spec/settings.md`, "How a key becomes a command"
//! and "Changing a key").
//!
//! A binding is the game's name for what a key does (`EGameKey`), up to two
//! variants of keys held together (a variant may end in a notch of the
//! mouse wheel), and a way of being pressed. The input manager makes one
//! combination of each variant and runs them all every frame, the one with
//! the most keys first; a combination that takes its keys keeps them from
//! the shorter ones, which is how `T` with `Left Alt` is not also `T`. What
//! comes out is which bindings were given this frame, and for those that
//! hold (a lean, a sprint bound to be held) which were let go.
//!
//! No engine types: a key is the engine's own number for it
//! (`UnityEngine.KeyCode`, whose names the install's `Control.ini` is
//! written in), and a host says which are down. `install_controls` reads
//! the install's own bindings into these.

/// The engine's number for a key.
pub type Code = u16;

/// The key that drops a key being chosen.
pub const ESCAPE: Code = 27;

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

/// What a key is on a frame, from what it was and whether it is down.
const UPDATE: [[KeyPress; 4]; 2] =
    [[KeyPress::None, KeyPress::Up, KeyPress::Up, KeyPress::None], [KeyPress::Down, KeyPress::Hold, KeyPress::Hold, KeyPress::Down]];

/// What several keys held together are, taken two at a time: none if any
/// is none, coming up if any is, going down if any is, else held.
const MERGE: [[KeyPress; 4]; 4] = [
    [KeyPress::None; 4],
    [KeyPress::None, KeyPress::Down, KeyPress::Down, KeyPress::Up],
    [KeyPress::None, KeyPress::Down, KeyPress::Hold, KeyPress::Up],
    [KeyPress::None, KeyPress::Up, KeyPress::Up, KeyPress::Up],
];

/// One variant of a binding (`InputSource`): the keys held together, in
/// the order they are kept (the game keeps a chosen variant's by their
/// numbers, and a file's as written), and a notch of the wheel with them,
/// up or down, if it ends in one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Source {
    pub keys: Vec<Code>,
    pub wheel: Option<bool>,
}

impl Source {
    pub fn of(keys: &[Code]) -> Self {
        Self { keys: keys.to_vec(), wheel: None }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.wheel.is_none()
    }

    /// One frame of a key being chosen (`InputSource.Listen`): the first
    /// key found down that is not yet among the keys is added (one a
    /// frame, kept by their numbers), and says so by going down; a key of
    /// them that is no longer down ends it, by coming up; and with no key
    /// changed, a notch of the wheel is taken and ends it too, where the
    /// binding may have one. `valid` is every key that can be chosen,
    /// lowest number first.
    pub fn listen(&mut self, valid: &[Code], held: &dyn Fn(Code) -> bool, wheel: i32, takes_wheel: bool) -> KeyPress {
        for &key in valid {
            if held(key) {
                if !self.keys.contains(&key) {
                    let at = self.keys.iter().position(|have| key <= *have).unwrap_or(self.keys.len());
                    self.keys.insert(at, key);
                    return KeyPress::Down;
                }
            } else if self.keys.contains(&key) {
                return KeyPress::Up;
            }
        }
        if takes_wheel && wheel != 0 {
            self.wheel = Some(wheel > 0);
            return KeyPress::Up;
        }
        KeyPress::None
    }
}

/// What a frame of choosing a key came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen {
    /// Still listening.
    Going,
    /// Escape was among the keys: what stood there stays.
    Dropped,
    Done(Source),
}

/// A key being chosen for a variant (`CommandKeyPair.ListenForKey`): keys
/// going down are gathered, and the first of them to come up ends it, as
/// does a third key going down, or a notch of the wheel.
#[derive(Debug, Clone, Default)]
pub struct Listening {
    pub source: Source,
}

impl Listening {
    pub fn frame(&mut self, valid: &[Code], held: &dyn Fn(Code) -> bool, wheel: i32, takes_wheel: bool) -> Chosen {
        let press = self.source.listen(valid, held, wheel, takes_wheel);
        if press != KeyPress::None && self.source.keys.contains(&ESCAPE) {
            return Chosen::Dropped;
        }
        if (press == KeyPress::Up && !self.source.is_empty()) || (press == KeyPress::Down && self.source.keys.len() >= 3) {
            return Chosen::Done(std::mem::take(&mut self.source));
        }
        Chosen::Going
    }
}

/// A binding (`KeyGroup`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGroup {
    pub name: String,
    pub variants: [Source; 2],
    pub press: PressType,
}

/// The bindings whose way of being pressed is not the player's to choose
/// (`ECommandExtension`), and those that have one way only.
const NO_PRESS_CHOICE: [&str; 14] = [
    "Shoot",
    "Nidnod",
    "PushToTalk",
    "Interact",
    "BlindShootAbove",
    "BlindShootRight",
    "MakeScreenshot",
    "ThrowItem",
    "HighThrow",
    "LowThrow",
    "ToggleVoip",
    "LeftStance",
    "Tactical",
    "ThrowGrenade",
];
const FIXED_PRESS: [(&str, PressType); 3] = [("Vaulting", PressType::Continuous), ("Tactical", PressType::Press), ("ThrowGrenade", PressType::Press)];

/// The bindings that hold: given as their keys go down and taken back as
/// they come up, when bound to be held (`ToggleInputKeyCombination`).
const HOLDING: [&str; 19] = [
    "Shoot",
    "Aim",
    "Interact",
    "Sprint",
    "Breath",
    "Walk",
    "Vaulting",
    "Duck",
    "LeanLockLeft",
    "LeanLockRight",
    "BlindShootAbove",
    "BlindShootRight",
    "StepLeft",
    "StepRight",
    "Mumble",
    "MumbleDropdown",
    "PushToTalk",
    "TogglePinMode",
    "ToggleLockMode",
];

/// The bindings the manager gives a rank of their own (`InputManager.Init`):
/// of two ranked ones that want the same key, the lower number has it; the
/// sprint's (-2) never gives way.
const PRIORITIES: [(&str, i32); 9] = [
    ("CheckFireMode", 10),
    ("FoldStock", 11),
    ("CheckAmmo", 12),
    ("CheckChamber", 13),
    ("ChamberUnload", 14),
    ("UnloadMagazine", 15),
    ("Sprint", -2),
    ("BlindShootAbove", 99),
    ("BlindShootRight", 99),
];
const UNRANKED: i32 = -1;
const NEVER_YIELDS: i32 = -2;

impl KeyGroup {
    /// Whether the player may choose how it is pressed.
    pub fn press_choice(&self) -> bool {
        !NO_PRESS_CHOICE.contains(&self.name.as_str())
    }

    /// Whether its way of being pressed is shown to be chosen
    /// (`DropdownAvailable`): not for a binding with a notch of the wheel.
    pub fn press_shown(&self) -> bool {
        self.press_choice() && self.variants.iter().all(|variant| variant.wheel.is_none())
    }

    pub fn is_empty(&self) -> bool {
        self.variants.iter().all(Source::is_empty)
    }

    /// Whether the settings screen leaves it out (`_forbiddenKeys`): the
    /// function keys, Escape and Enter, and the bindings the game makes
    /// out of others.
    pub fn hidden(&self) -> bool {
        let name = self.name.as_str();
        let function = name.strip_prefix("DoubleF").or(name.strip_prefix('F')).is_some_and(|n| n.parse::<u8>().is_ok());
        function
            || name.starts_with("PressSlot")
            || ["Escape", "Enter", "HighThrow", "LowThrow", "PressThrowGrenade", "ReleaseTactical", "ChangePointOfView"].contains(&name)
    }

    fn holds(&self) -> bool {
        HOLDING.contains(&self.name.as_str())
    }
}

/// An axis the keys push one way or the other (`AxisGroup`): for each of
/// its two pairs, the keys for the positive way and for the negative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisGroup {
    pub name: String,
    pub pairs: [[Source; 2]; 2],
}

/// The bindings as they stand.
#[derive(Debug, Clone, PartialEq)]
pub struct Controls {
    pub keys: Vec<KeyGroup>,
    pub axes: Vec<AxisGroup>,
    /// The seconds two presses of a double click may be apart, and a held
    /// key waits (`DoubleClickTimeout`, 0.3 to 1).
    pub double_click: f32,
}

impl Controls {
    pub fn group(&self, name: &str) -> Option<usize> {
        self.keys.iter().position(|group| group.name == name)
    }

    /// The keys chosen for a variant of a binding are in
    /// (`CommandKeyPair.ListenForKey`, after the choice). Says whether the
    /// game now asks how the binding is to be pressed: it does where
    /// another binding has the same keys and is pressed the same way, or
    /// where the binding had no keys before, if its way may be chosen and
    /// its other variant is empty. Until that is answered (`press_chosen`)
    /// the other binding keeps its keys.
    pub fn key_chosen(&mut self, group: usize, variant: usize, source: Source) -> bool {
        let was_empty = self.keys[group].is_empty();
        self.keys[group].variants[variant] = source.clone();
        let free = self.settle(group, &source, false);
        let asks = (!free || was_empty) && self.keys[group].press_shown() && self.keys[group].variants[1 - variant].is_empty();
        if asks {
            if was_empty {
                self.keys[group].press = PressType::ALL[0];
            }
        } else {
            if self.keys[group].variants.iter().any(|variant| variant.wheel.is_some()) {
                // A binding with a notch of the wheel is a press.
                self.keys[group].press = PressType::Press;
            }
            self.settle(group, &source, true);
        }
        // The same keys in its other variant are dropped there.
        if !source.is_empty() && self.keys[group].variants[1 - variant] == source {
            self.keys[group].variants[1 - variant] = Source::default();
        }
        self.fix_press(group);
        asks
    }

    /// How a binding is pressed has been chosen (`DropdownClosed`): any
    /// other binding with the same keys and now the same way loses them.
    pub fn press_chosen(&mut self, group: usize, press: PressType) {
        self.keys[group].press = press;
        for variant in self.keys[group].variants.clone() {
            self.settle(group, &variant, true);
        }
        self.fix_press(group);
    }

    /// The keys chosen for one way of an axis are in
    /// (`ControlSettingsTab.AxisChosen`): the same keys are dropped
    /// wherever else they stand, on an axis or on a binding.
    pub fn axis_chosen(&mut self, axis: usize, pair: usize, way: usize, source: Source) {
        self.axes[axis].pairs[pair][way] = source.clone();
        if source.is_empty() {
            return;
        }
        for (a, group) in self.axes.iter_mut().enumerate() {
            for (p, sources) in group.pairs.iter_mut().enumerate() {
                for (w, other) in sources.iter_mut().enumerate() {
                    if (a, p, w) != (axis, pair, way) && *other == source {
                        *other = Source::default();
                    }
                }
            }
        }
        for group in self.keys.iter_mut().filter(|group| !group.hidden()) {
            for other in group.variants.iter_mut().filter(|other| **other == source) {
                *other = Source::default();
            }
        }
    }

    /// A binding that has one way of being pressed is pressed that way.
    fn fix_press(&mut self, group: usize) {
        if let Some((_, press)) = FIXED_PRESS.iter().find(|(name, _)| *name == self.keys[group].name) {
            self.keys[group].press = *press;
        }
    }

    /// Looks for the same keys on the other bindings shown
    /// (`ControlSettingsTab.KeyChosen`). Two bindings whose way of being
    /// pressed may be chosen share keys as long as they are pressed
    /// differently. Pressed alike, the other loses its keys when `full`;
    /// before that the answer is that the keys are not free, and the game
    /// asks for another way. Where either's way may not be chosen, the
    /// other loses its keys at once. When `full`, the axes lose them too.
    fn settle(&mut self, group: usize, source: &Source, full: bool) -> bool {
        if source.is_empty() {
            return true;
        }
        let (choice, press) = (self.keys[group].press_shown(), self.keys[group].press);
        let mut free = true;
        for other in 0..self.keys.len() {
            if other == group || self.keys[other].hidden() {
                continue;
            }
            let Some(at) = self.keys[other].variants.iter().position(|variant| variant == source) else { continue };
            let both = choice && self.keys[other].press_shown();
            if !both || press == self.keys[other].press {
                free = !both;
                if full || !both {
                    self.keys[other].variants[at] = Source::default();
                }
            }
        }
        if full {
            for axis in &mut self.axes {
                for other in axis.pairs.iter_mut().flatten().filter(|other| **other == *source) {
                    *other = Source::default();
                }
            }
        }
        free
    }
}

/// One variant of a binding as the install keeps it, for the manager: its
/// keys, and a notch of the wheel if it ends in one. Empty if it names a
/// key there is no number for here, or an axis other than the wheel.
pub fn of_install(source: &trp_data::install::KeySource) -> Source {
    let wheel = match &source.axis {
        Some((axis, up)) if axis == "Mouse Scroll" => Some(*up),
        Some(_) => return Source::default(),
        None => None,
    };
    match source.keys.iter().map(|name| code(name)).collect::<Option<Vec<Code>>>() {
        Some(keys) => Source { keys, wheel },
        None => Source::default(),
    }
}

/// The install's own bindings (`Control.ini`): every key binding it has,
/// in its order, and those of its axes that keys push (the mouse's four
/// are the view's). Nothing where the install has no such file.
pub fn install_controls(install: &trp_data::Install) -> Controls {
    let keys = install
        .key_bindings()
        .into_iter()
        .map(|binding| {
            let variant = |at: usize| binding.variants.get(at).map(of_install).unwrap_or_default();
            KeyGroup { variants: [variant(0), variant(1)], press: PressType::of(&binding.press).unwrap_or(PressType::Press), name: binding.name }
        })
        .collect();
    let axes = install
        .axis_bindings()
        .into_iter()
        .filter(|axis| axis.pairs.iter().flatten().all(|way| way.axis.is_none()))
        .map(|axis| {
            let pair = |at: usize| axis.pairs.get(at).map(|[up, down]| [of_install(up), of_install(down)]).unwrap_or_default();
            AxisGroup { pairs: [pair(0), pair(1)], name: axis.name }
        })
        .collect();
    Controls { keys, axes, double_click: install.control_setting("DoubleClickTimeout").unwrap_or(0.3).clamp(0.3, 1.0) }
}

/// A key or a way of the wheel among the manager's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyId {
    Key(Code),
    Wheel(bool),
}

/// A key as the combinations share it: what it is on this frame, which
/// combination has it, and whether that one lets go of it next frame.
#[derive(Debug, Clone)]
struct KeyState {
    id: KeyId,
    press: KeyPress,
    in_use_by: Option<usize>,
    release_next_frame: bool,
}

/// Where a combination of keys is in being pressed (`EKeyState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    ClickIdling,
    ClickWaitForRelease,
    ContinuousIdling,
    ContinuousPressed,
    LongTapPressed,
    LongTapWaitForRelease,
    ReleaseIdling,
    ReleasePressed,
    ReleaseStuck,
    DoubleClickIdling,
    WaitingForFirstRelease,
    WaitingForSecondPress,
    WaitingForSecondRelease,
    /// An axis with nothing but its two ways' keys, and one whose two
    /// ways share a key (`EAxisState`).
    JustAxis,
    AxisIdling,
    AxisPressed,
}

#[derive(Debug, Clone)]
enum Kind {
    Key {
        group: usize,
        holds: bool,
        press: PressType,
        /// The sum the manager tells two combinations of the same keys by.
        check_hash: i64,
        priority: i32,
        blind: bool,
    },
    Axis {
        axis: usize,
        /// The keys of each way, the way looked at first coming first,
        /// with whether it is the positive one.
        ways: [(bool, Vec<usize>); 2],
        common: Vec<usize>,
    },
}

#[derive(Debug, Clone)]
struct Combination {
    keys: Vec<usize>,
    /// How many keys it counts as having.
    count: usize,
    /// Nothing takes a key from it (`IsImportant`).
    important: bool,
    kind: Kind,
}

#[derive(Debug, Clone, Copy)]
struct Progress {
    state: State,
    time_left: f32,
    value: f32,
}

/// What the keys came to on a frame: the bindings given and those taken
/// back, each by its place among the controls' bindings and in the order
/// the manager gives them out; and each axis's push, by its place among
/// the controls' axes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Heard {
    pub given: Vec<usize>,
    pub taken_back: Vec<usize>,
    pub axes: Vec<f32>,
}

/// What the mouse's travel on a frame is given to: the view's turn, or
/// the head's free look, each across and up and down, in whatever the
/// travel was given in (`TurnX`, `TurnY`, `LookX`, `LookY`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MouseHeard {
    pub turn: [f32; 2],
    pub look: [f32; 2],
}

/// The binding whose keys give the mouse to the head's free look
/// (`EGameKey.Nidnod`, the modifier of the look's two axes).
pub const LOOK_BINDING: &str = "Nidnod";

/// The look's two axes for one variant of that binding
/// (`InputAxisCombination` with a modifier): the binding itself, the keys
/// that have to be held, how many inputs the combination counts as having
/// (those keys and the axis), and whether it has each of the mouse's two
/// axes just now.
#[derive(Debug, Clone)]
struct LookAxes {
    group: usize,
    modifiers: Vec<usize>,
    count: usize,
    has: [bool; 2],
}

/// The manager's combinations for a set of bindings (`InputPreset`).
#[derive(Debug, Clone)]
pub struct Preset {
    keys: Vec<KeyState>,
    combinations: Vec<Combination>,
    progress: Vec<Progress>,
    /// The head's free look, a pair of axes for each variant of its
    /// binding that has keys; none where it has none.
    look: Vec<LookAxes>,
    /// The axes' combinations in the order their pushes are taken.
    axis_order: Vec<usize>,
    axes: usize,
    double_click: f32,
}

/// What stands in for the engine's hash of a way of the wheel in the sums
/// combinations are told apart by: any two figures no sum of keys reaches.
const WHEEL_HASH: [i64; 2] = [1_000_003, 2_000_003];

impl Preset {
    /// The combinations of a set of bindings (`InputPreset.UpdateBindings`).
    pub fn new(controls: &Controls) -> Self {
        let mut keys: Vec<KeyState> = Vec::new();
        let mut key_of = |id: KeyId| -> usize {
            keys.iter().position(|key| key.id == id).unwrap_or_else(|| {
                keys.push(KeyState { id, press: KeyPress::None, in_use_by: None, release_next_frame: false });
                keys.len() - 1
            })
        };
        // A combination for every variant that has keys, with the sum of
        // its keys' numbers (a way of the wheel counts only when alone).
        let mut made: Vec<(Combination, i64, i64)> = Vec::new();
        let mut look = Vec::new();
        for (index, group) in controls.keys.iter().enumerate() {
            for variant in group.variants.iter().filter(|variant| !variant.is_empty()) {
                let mut held: Vec<(usize, i64, bool)> = variant.keys.iter().map(|code| (key_of(KeyId::Key(*code)), *code as i64, false)).collect();
                if let Some(up) = variant.wheel {
                    held.push((key_of(KeyId::Wheel(up)), WHEEL_HASH[up as usize], true));
                }
                // The look's axes take this variant's keys as the keys
                // that give them the mouse.
                if group.name == LOOK_BINDING {
                    look.push(LookAxes { group: index, modifiers: held.iter().map(|(key, ..)| *key).collect(), count: held.len() + 1, has: [false; 2] });
                }
                let alone = held.len() == 1;
                let keys_hash: i64 = held.iter().filter(|(_, _, wheel)| alone || !wheel).map(|(_, hash, _)| hash).sum();
                let full_hash: i64 = held.iter().map(|(_, hash, _)| hash).sum();
                let name = group.name.as_str();
                let kind = Kind::Key {
                    group: index,
                    holds: group.holds(),
                    press: group.press,
                    check_hash: keys_hash,
                    priority: PRIORITIES.iter().find(|(ranked, _)| *ranked == name).map_or(UNRANKED, |(_, rank)| *rank),
                    blind: name.starts_with("BlindShoot"),
                };
                // Nothing takes a key from the aim's while it is bound to
                // be held, nor from the tactical device's release.
                let important = (name == "Aim" && group.press == PressType::Continuous) || name == "ReleaseTactical";
                made.push((Combination { keys: held.iter().map(|(key, ..)| *key).collect(), count: held.len(), important, kind }, keys_hash, full_hash));
            }
        }
        // The crouch key bound to its release: every combination with its
        // keys is told apart by all of its keys, the wheel's way included,
        // so that the key coming up after it was used with the wheel does
        // not crouch.
        let ducks: Vec<i64> = made
            .iter()
            .filter(|(made, ..)| matches!(&made.kind, Kind::Key { group, press: PressType::Release, .. } if controls.keys[*group].name == "Duck"))
            .map(|(_, keys_hash, _)| *keys_hash)
            .collect();
        for (combination, keys_hash, full_hash) in &mut made {
            if let (true, Kind::Key { check_hash, .. }) = (ducks.contains(keys_hash), &mut combination.kind) {
                *check_hash = *full_hash;
            }
        }
        let mut list: Vec<Combination> = made.into_iter().map(|(combination, ..)| combination).collect();
        let key_combinations = list.len();
        // An axis: the keys of each way, those on both ways apart.
        for (index, group) in controls.axes.iter().enumerate() {
            for [positive, negative] in group.pairs.iter().filter(|pair| pair.iter().any(|way| !way.is_empty())) {
                let ids = |source: &Source| -> Vec<KeyId> { source.keys.iter().map(|code| KeyId::Key(*code)).chain(source.wheel.map(KeyId::Wheel)).collect() };
                let mut up: Vec<usize> = ids(positive).into_iter().map(&mut key_of).collect();
                let (mut down, mut common) = (Vec::new(), Vec::new());
                for key in ids(negative).into_iter().map(&mut key_of) {
                    match up.iter().position(|have| *have == key) {
                        Some(at) => {
                            up.remove(at);
                            common.push(key);
                        }
                        None => down.push(key),
                    }
                }
                // The way with more keys is looked at first; the positive
                // one where they have as many.
                let ways = if up.len() < down.len() { [(false, down), (true, up)] } else { [(true, up), (false, down)] };
                let count = common.len() + 1;
                list.push(Combination { keys: Vec::new(), count, important: false, kind: Kind::Axis { axis: index, ways, common } });
            }
        }
        // The order they are run in: the most keys first, and among those
        // of as many keys the double clicks first, the last made foremost.
        let most = list.iter().map(|combination| combination.count).max().unwrap_or(0);
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); most + 1];
        for (index, combination) in list.iter().enumerate() {
            if matches!(combination.kind, Kind::Key { press: PressType::DoubleClick, .. }) {
                buckets[combination.count].insert(0, index);
            } else {
                buckets[combination.count].push(index);
            }
        }
        let order: Vec<usize> = buckets.into_iter().rev().flatten().collect();
        let place = |made: usize| order.iter().position(|index| *index == made).unwrap_or(0);
        let axis_order = (key_combinations..list.len()).map(place).collect();
        let combinations: Vec<Combination> = order.iter().map(|index| list[*index].clone()).collect();
        let progress = combinations
            .iter()
            .map(|combination| {
                let state = match &combination.kind {
                    Kind::Key { press: PressType::Release, .. } => State::ReleaseIdling,
                    Kind::Key { press: PressType::Continuous, .. } => State::ContinuousIdling,
                    Kind::Key { press: PressType::Press, .. } => State::ClickIdling,
                    Kind::Key { press: PressType::DoubleClick, .. } => State::DoubleClickIdling,
                    Kind::Axis { common, .. } if common.is_empty() => State::JustAxis,
                    Kind::Axis { .. } => State::AxisIdling,
                };
                Progress { state, time_left: 0.0, value: 0.0 }
            })
            .collect();
        Self { keys, combinations, progress, look, axis_order, axes: controls.axes.len(), double_click: controls.double_click }
    }

    /// What the mouse's travel on this frame is given to, across and up
    /// and down: after `update`, with the keys as that left them.
    ///
    /// The turn's two axes and the look's two are all bound to the mouse,
    /// and the look's have the free look's keys as their modifier
    /// (`InputAxisCombination`): an axis of the look that is idle takes
    /// the mouse's axis on a frame the mouse moves along it with those
    /// keys held, and gives nothing on that frame; from the next it gives
    /// the travel, until a frame on which the mouse does not move along
    /// that axis or the keys are up, which lets the axis go. The turn has
    /// of an axis only what the look has not taken on the frame: so the
    /// first frame of each run of travel is given to neither. Each of the
    /// two axes goes by itself.
    pub fn mouse(&mut self, moved: [f32; 2]) -> MouseHeard {
        let mut heard = MouseHeard::default();
        let mut taken = [false; 2];
        for at in 0..self.look.len() {
            let status = self.look_status(at);
            for axis in 0..2 {
                let has = self.look[at].has[axis];
                let held = match has {
                    true => !matches!(status, KeyPress::Up | KeyPress::None),
                    false => matches!(status, KeyPress::Down | KeyPress::Hold),
                };
                let keeps = moved[axis] != 0.0 && held;
                if keeps && has && !taken[axis] {
                    heard.look[axis] = moved[axis];
                }
                taken[axis] |= keeps;
                self.look[at].has[axis] = keeps;
            }
        }
        for axis in 0..2 {
            if !taken[axis] {
                heard.turn[axis] = moved[axis];
            }
        }
        heard
    }

    /// What the free look's keys are together for one variant, and none
    /// if a combination the look's axes give way to has one of them: any
    /// of as many inputs or more, but never the look's own binding nor
    /// blind fire's (`InputAxisCombination.IsLowerPriority`).
    fn look_status(&self, at: usize) -> KeyPress {
        let look = &self.look[at];
        // The mouse's axis, moving, counts as held.
        let mut status = KeyPress::Hold;
        for &key in &look.modifiers {
            let state = &self.keys[key];
            let gives_way = state.in_use_by.is_some_and(|user| {
                let theirs = &self.combinations[user];
                match &theirs.kind {
                    Kind::Key { group, blind, .. } if *group == look.group || *blind => false,
                    _ => theirs.important || theirs.count >= look.count,
                }
            });
            if gives_way {
                return KeyPress::None;
            }
            let press = match state.id {
                KeyId::Wheel(_) if matches!(state.press, KeyPress::Down | KeyPress::Hold) => KeyPress::Hold,
                KeyId::Wheel(_) => KeyPress::None,
                KeyId::Key(_) => state.press,
            };
            status = MERGE[status as usize][press as usize];
        }
        status
    }

    /// One frame (`InputPreset.UpdateInput`): `held` says which keys are
    /// down, `wheel` which way the wheel turned on this frame if it did,
    /// and `dt` is the frame's length in seconds.
    pub fn update(&mut self, held: &dyn Fn(Code) -> bool, wheel: i32, dt: f32) -> Heard {
        for key in &mut self.keys {
            let down = match key.id {
                KeyId::Key(code) => held(code),
                KeyId::Wheel(up) => wheel != 0 && (wheel > 0) == up,
            };
            key.press = UPDATE[down as usize][key.press as usize];
        }
        let mut heard = Heard { axes: vec![0.0; self.axes], ..Heard::default() };
        for index in 0..self.combinations.len() {
            let Kind::Key { group, holds, .. } = self.combinations[index].kind else {
                self.update_axis(index);
                continue;
            };
            match self.command(index, holds, dt) {
                Some(true) => heard.given.push(group),
                Some(false) => heard.taken_back.push(group),
                None => {}
            }
        }
        // An axis takes the push of the first of its pairs that has one.
        for &index in &self.axis_order {
            if let Kind::Axis { axis, .. } = self.combinations[index].kind {
                if heard.axes[axis].abs() < 0.0001 {
                    heard.axes[axis] = self.progress[index].value;
                }
            }
        }
        heard
    }

    /// Whether a combination gives way to the one that has a key
    /// (`IsLowerPriority`).
    fn lower(&self, this: usize, other: usize) -> bool {
        if this == other {
            return false;
        }
        let (mine, theirs) = (&self.combinations[this], &self.combinations[other]);
        if theirs.important {
            return true;
        }
        match (&mine.kind, &theirs.kind) {
            (Kind::Key { check_hash, priority, .. }, Kind::Key { check_hash: other_hash, priority: other_priority, press, .. }) => {
                if check_hash == other_hash {
                    // The same keys: only a double click is waited for.
                    return *press == PressType::DoubleClick;
                }
                if *priority == NEVER_YIELDS {
                    return false;
                }
                if *priority > UNRANKED && *other_priority > UNRANKED {
                    return priority >= other_priority;
                }
                theirs.count >= mine.count
            }
            // An axis keeps its keys from blind fire's.
            (Kind::Axis { .. }, Kind::Key { blind: true, .. }) => false,
            _ => theirs.count >= mine.count,
        }
    }

    /// What a combination's keys are together, and whether none of them
    /// is in the hands of a combination it gives way to (`GetKeysStatus`).
    fn status(&mut self, index: usize) -> (bool, KeyPress) {
        let (mut free, mut status) = (true, KeyPress::Hold);
        for at in 0..self.combinations[index].keys.len() {
            let key = self.combinations[index].keys[at];
            if self.keys[key].release_next_frame && self.keys[key].in_use_by.is_none_or(|user| user >= index) {
                self.keys[key].in_use_by = None;
                self.keys[key].release_next_frame = false;
            }
            if free && self.keys[key].in_use_by.is_some_and(|user| self.lower(index, user)) {
                free = false;
            }
            status = MERGE[status as usize][self.keys[key].press as usize];
        }
        (free, status)
    }

    /// Takes the combination's keys, for good or for this frame.
    fn handle(&mut self, index: usize, this_frame_only: bool) {
        for at in 0..self.combinations[index].keys.len() {
            let key = self.combinations[index].keys[at];
            self.keys[key].in_use_by = Some(index);
            if this_frame_only {
                self.keys[key].release_next_frame = true;
            }
        }
    }

    /// Lets go of them; a double click that did not come gives them back
    /// as just pressed, so that a plain press of the same key is heard.
    fn unhandle(&mut self, index: usize, restore: Option<KeyPress>) {
        for at in 0..self.combinations[index].keys.len() {
            let key = self.combinations[index].keys[at];
            if self.keys[key].in_use_by == Some(index) {
                self.keys[key].in_use_by = None;
            }
            if let Some(press) = restore {
                self.keys[key].press = press;
            }
        }
    }

    fn change(&mut self, index: usize, state: State) {
        if self.progress[index].state != state {
            self.progress[index].state = state;
            if matches!(state, State::LongTapPressed | State::WaitingForFirstRelease) {
                self.progress[index].time_left = self.double_click;
            }
        }
    }

    fn timed_out(&mut self, index: usize, dt: f32) -> bool {
        self.progress[index].time_left -= dt;
        self.progress[index].time_left <= 0.0
    }

    /// A combination's frame: whether its binding is given (`true`) or
    /// taken back (`false`) on it.
    fn command(&mut self, index: usize, holds: bool, dt: f32) -> Option<bool> {
        let state = self.progress[index].state;
        // A double click that takes too long, or whose key something else
        // has taken, is off.
        if matches!(state, State::WaitingForFirstRelease | State::WaitingForSecondPress | State::WaitingForSecondRelease) {
            if self.timed_out(index, dt) {
                self.unhandle(index, Some(KeyPress::Down));
                self.change(index, State::DoubleClickIdling);
                return None;
            }
            let (free, status) = self.status(index);
            if !free {
                self.unhandle(index, None);
                self.change(index, State::DoubleClickIdling);
                return None;
            }
            if state == State::WaitingForSecondRelease && status == KeyPress::Up {
                self.handle(index, true);
                self.change(index, State::DoubleClickIdling);
                return Some(true);
            }
            self.handle(index, false);
            match (state, status) {
                (State::WaitingForFirstRelease, KeyPress::Up) => self.change(index, State::WaitingForSecondPress),
                (State::WaitingForSecondPress, KeyPress::Down) => self.change(index, State::WaitingForSecondRelease),
                _ => {}
            }
            return None;
        }
        let (free, status) = self.status(index);
        match state {
            State::ClickIdling => {
                if free && status == KeyPress::Down {
                    self.handle(index, false);
                    self.change(index, State::ClickWaitForRelease);
                    return Some(true);
                }
            }
            State::ClickWaitForRelease | State::LongTapWaitForRelease => {
                if status != KeyPress::Hold {
                    if free {
                        self.handle(index, true);
                    } else {
                        self.unhandle(index, None);
                    }
                    self.change(index, if state == State::ClickWaitForRelease { State::ClickIdling } else { State::ContinuousIdling });
                } else if free {
                    self.handle(index, false);
                }
            }
            State::ContinuousIdling => {
                if free && matches!(status, KeyPress::Down | KeyPress::Hold) {
                    self.handle(index, false);
                    // A binding that holds is given at once; any other
                    // only when it has been held long enough.
                    if holds {
                        self.change(index, State::ContinuousPressed);
                        return Some(true);
                    }
                    self.change(index, State::LongTapPressed);
                }
            }
            State::ContinuousPressed => {
                if free && status == KeyPress::Hold {
                    self.handle(index, false);
                } else {
                    self.unhandle(index, None);
                    self.change(index, State::ContinuousIdling);
                    return Some(!holds);
                }
            }
            State::LongTapPressed => {
                if free && status == KeyPress::Hold {
                    self.handle(index, false);
                    if self.timed_out(index, dt) {
                        self.change(index, State::LongTapWaitForRelease);
                        return Some(true);
                    }
                } else {
                    self.unhandle(index, None);
                    self.change(index, State::ContinuousIdling);
                }
            }
            State::DoubleClickIdling => {
                if free && status == KeyPress::Down {
                    self.handle(index, false);
                    self.change(index, State::WaitingForFirstRelease);
                }
            }
            State::ReleaseIdling => {
                if matches!(status, KeyPress::Down | KeyPress::Hold) {
                    if free {
                        self.handle(index, false);
                    }
                    self.change(index, State::ReleasePressed);
                }
            }
            State::ReleasePressed => match status {
                KeyPress::Up => {
                    self.change(index, State::ReleaseIdling);
                    if free {
                        self.handle(index, true);
                        return Some(true);
                    }
                }
                KeyPress::Hold if free => self.handle(index, false),
                KeyPress::Hold => {
                    // A key taken by a combination of other keys: the
                    // release is not heard this time.
                    let mine = match self.combinations[index].kind {
                        Kind::Key { check_hash, .. } => check_hash,
                        Kind::Axis { .. } => 0,
                    };
                    let taken = self.combinations[index].keys.iter().any(|key| {
                        self.keys[*key].in_use_by.is_some_and(|user| !matches!(self.combinations[user].kind, Kind::Key { check_hash, .. } if check_hash == mine))
                    });
                    self.unhandle(index, None);
                    if taken {
                        self.change(index, State::ReleaseStuck);
                    }
                }
                _ => {}
            },
            State::ReleaseStuck => {
                if status == KeyPress::Up {
                    self.change(index, State::ReleaseIdling);
                }
            }
            _ => {}
        }
        None
    }

    /// Which way an axis's keys push, and what its keys are together
    /// (`AxisCombinationState.GetKeysStatus`): the first way whose keys
    /// are all the axis's to use and none of them idle.
    fn axis_status(&self, index: usize, with_common: bool) -> (Option<usize>, KeyPress) {
        let Kind::Axis { ways, common, .. } = &self.combinations[index].kind else { return (None, KeyPress::None) };
        let mut found = (None, KeyPress::Hold);
        for (way, (_, keys)) in ways.iter().enumerate() {
            let mut status = KeyPress::Hold;
            if keys.is_empty() {
                found.1 = status;
                continue;
            }
            for &key in keys {
                let state = &self.keys[key];
                if status == KeyPress::None || state.in_use_by.is_some_and(|user| self.lower(index, user)) {
                    status = KeyPress::None;
                    break;
                }
                let press = match state.id {
                    KeyId::Wheel(_) if matches!(state.press, KeyPress::Down | KeyPress::Hold) => KeyPress::Hold,
                    KeyId::Wheel(_) => KeyPress::None,
                    KeyId::Key(_) => state.press,
                };
                status = MERGE[press as usize][status as usize];
            }
            found.1 = status;
            if status != KeyPress::None {
                found.0 = Some(way);
                break;
            }
        }
        let (Some(way), true) = (found.0, with_common) else { return found };
        let mut status = found.1;
        for &key in common {
            if self.keys[key].in_use_by.is_some_and(|user| self.lower(index, user)) {
                return (None, KeyPress::None);
            }
            status = MERGE[status as usize][self.keys[key].press as usize];
        }
        (Some(way), status)
    }

    /// An axis's push one way: one if every key of that way is down, and
    /// every key the two ways share.
    fn axis_value(&self, index: usize, way: usize) -> f32 {
        let Kind::Axis { ways, common, .. } = &self.combinations[index].kind else { return 0.0 };
        let down = |keys: &[usize]| keys.iter().all(|key| matches!(self.keys[*key].press, KeyPress::Down | KeyPress::Hold));
        let (positive, keys) = &ways[way];
        if !down(keys) || !down(common) {
            return 0.0;
        }
        if *positive { 1.0 } else { -1.0 }
    }

    fn update_axis(&mut self, index: usize) {
        let state = self.progress[index].state;
        let (way, status) = self.axis_status(index, state != State::JustAxis);
        let value = match (state, way) {
            (State::JustAxis, Some(way)) => self.axis_value(index, way),
            (State::AxisIdling, Some(way)) if matches!(status, KeyPress::Down | KeyPress::Hold) => {
                self.take_axis(index, Some(way));
                self.progress[index].state = State::AxisPressed;
                0.0
            }
            (State::AxisPressed, Some(way)) if !matches!(status, KeyPress::Up | KeyPress::None) => {
                self.take_axis(index, Some(way));
                self.axis_value(index, way)
            }
            (State::AxisPressed, _) => {
                self.take_axis(index, None);
                self.progress[index].state = State::AxisIdling;
                0.0
            }
            _ => 0.0,
        };
        self.progress[index].value = value;
    }

    /// An axis whose ways share a key takes that key and the keys of the
    /// way pushed, or lets go of all it has.
    fn take_axis(&mut self, index: usize, way: Option<usize>) {
        let Kind::Axis { ways, common, .. } = &self.combinations[index].kind else { return };
        match way {
            Some(way) => {
                for &key in common.iter().chain(&ways[way].1) {
                    self.keys[key].in_use_by = Some(index);
                }
            }
            None => {
                for &key in common.iter().chain(ways.iter().flat_map(|(_, keys)| keys)) {
                    if self.keys[key].in_use_by == Some(index) {
                        self.keys[key].in_use_by = None;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
}
