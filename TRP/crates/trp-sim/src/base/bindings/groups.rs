//! Key groups and axis groups, their press choices and priorities.

use super::*;

/// What a key is on a frame, from what it was and whether it is down.
pub(super) const UPDATE: [[KeyPress; 4]; 2] =
    [[KeyPress::None, KeyPress::Up, KeyPress::Up, KeyPress::None], [KeyPress::Down, KeyPress::Hold, KeyPress::Hold, KeyPress::Down]];

/// What several keys held together are, taken two at a time: none if any
/// is none, coming up if any is, going down if any is, else held.
pub(super) const MERGE: [[KeyPress; 4]; 4] = [
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

pub(super) const FIXED_PRESS: [(&str, PressType); 3] = [("Vaulting", PressType::Continuous), ("Tactical", PressType::Press), ("ThrowGrenade", PressType::Press)];

/// The bindings that hold: given as their keys go down and taken back as
/// they come up, when bound to be held (`ToggleInputKeyCombination`).
const HOLDING: [&str; 21] = [
    "HighThrow",
    "LowThrow",
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
pub(super) const PRIORITIES: [(&str, i32); 9] = [
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

pub(super) const UNRANKED: i32 = -1;

pub(super) const NEVER_YIELDS: i32 = -2;

/// The bindings the game makes of others whenever the bindings are read
/// (`ControlSettingsGroup`'s generated keys): a grenade's throw from above
/// of the trigger's keys, from below of the aim's, each held whatever the
/// binding it is made of (`Continuous`), and each with keys of its own,
/// pressed and taken apart from the same keys of every other binding
/// (`ignorePool`: `InputKey.GetKey`).
pub(super) const GENERATED: [(&str, &str); 2] = [("HighThrow", "Shoot"), ("LowThrow", "Aim")];

impl KeyGroup {
    /// Whether its keys are its own (`KeyGroup.IgnorePool`): the generated
    /// bindings'.
    pub(super) fn own_keys(&self) -> bool {
        GENERATED.iter().any(|(name, _)| *name == self.name)
    }

    /// Whether it is one of the game's reserved function keys' bindings
    /// (`F1` to `F12`, `DoubleF1` to `DoubleF12`), made by `generate`.
    pub fn reserved(&self) -> bool {
        let name = self.name.as_str();
        name.strip_prefix("DoubleF").or(name.strip_prefix('F')).and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (1..=12).contains(&n))
    }

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

    pub(super) fn holds(&self) -> bool {
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
