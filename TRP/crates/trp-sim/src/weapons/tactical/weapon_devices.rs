//! The weapon's tactical devices, their modes and the requests to switch them.

use super::*;

/// How the weapon's key works (`TacticalInputMode`): a press switches
/// each device, or the devices are on while the key is held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Press,
    Hold,
}

impl InputMode {
    /// The setting's word, as the game's settings and its HUD's locale
    /// name it ("Press", "Hold").
    pub fn word(self) -> &'static str {
        match self {
            Self::Press => "Press",
            Self::Hold => "Hold",
        }
    }

    pub fn of_word(word: &str) -> Option<Self> {
        match word {
            "Press" | "0" => Some(Self::Press),
            "Hold" | "1" => Some(Self::Hold),
            _ => None,
        }
    }

    /// The other one (`ToggleTacticalInteraction`, T+B).
    pub fn flipped(self) -> Self {
        match self {
            Self::Press => Self::Hold,
            Self::Hold => Self::Press,
        }
    }
}

/// What a key asks of the weapon's devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKey {
    /// `Tactical` (T) in Press mode: each device switched on its own.
    Toggle,
    /// `Tactical` in Hold mode: all on.
    Down,
    /// `ReleaseTactical` (T let go) in Hold mode, and every event that
    /// lets Hold mode's lights go (the hands changing, a death, a screen
    /// over the battle, the window's focus lost): all off.
    Up,
    /// `NextTacticalDevice` (Ctrl+T), either mode: each device's mode one
    /// up.
    Next,
}

/// A request to the firearm (`SetLightsState`): each device's new state,
/// whether it passes any refusal (`force`), whether the hands play their
/// switch (`ModSwitch`), and whether the player's tactical sound goes
/// with it once taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub states: Vec<(Id, Light)>,
    pub forced: bool,
    pub animated: bool,
    pub sound: bool,
}

/// Every device in a thing's slots, its parts' too, in the order the game
/// walks them (`CompoundItem.AllSlots`: a thing's slots in its kind's
/// order, then each of those in turn the same way).
pub fn devices(item: &Item) -> Vec<&Item> {
    fn below<'a>(item: &'a Item, out: &mut Vec<&'a Item>) {
        let kids: Vec<&Item> = item.slots.iter().flatten().collect();
        out.extend(kids.iter().copied().filter(|kid| kid.kind.light_modes.is_some()));
        for kid in kids {
            below(kid, out);
        }
    }
    let mut out = Vec::new();
    below(item, &mut out);
    out
}

/// A mode one up, wrapped by the device's count where it has one
/// (`LightComponent.SelectedMode`'s setter).
pub fn next_mode(mode: u32, modes: u32) -> u32 {
    if modes > 0 {
        (mode + 1) % modes
    } else {
        mode + 1
    }
}

/// What a key asks of a weapon's devices, or nothing where it has none or
/// the key asks nothing (Hold mode's key down with all on already, let go
/// with none on).
pub fn weapon_request(weapon: &Item, key: WeaponKey) -> Option<Request> {
    let found = devices(weapon);
    if found.is_empty() {
        return None;
    }
    let all = |on: bool| found.iter().map(|d| (d.id, Light { on, mode: d.light.mode })).collect::<Vec<_>>();
    match key {
        WeaponKey::Toggle => Some(Request { states: found.iter().map(|d| (d.id, Light { on: !d.light.on, mode: d.light.mode })).collect(), forced: false, animated: true, sound: false }),
        WeaponKey::Down => (!found.iter().all(|d| d.light.on)).then(|| Request { states: all(true), forced: false, animated: false, sound: true }),
        WeaponKey::Up => found.iter().any(|d| d.light.on).then(|| Request { states: all(false), forced: true, animated: false, sound: true }),
        WeaponKey::Next => Some(Request {
            states: found.iter().map(|d| (d.id, Light { on: d.light.on, mode: next_mode(d.light.mode, d.kind.light_modes.unwrap_or(0)) })).collect(),
            forced: false,
            animated: true,
            sound: false,
        }),
    }
}

/// Writes each device's state into the thing it is in (`LightComponent`'s
/// two figures). Whether every one was found.
pub fn apply(root: &mut Item, states: &[(Id, Light)]) -> bool {
    let mut all = true;
    for &(id, light) in states {
        match root.find_mut(id) {
            Some(item) if item.kind.light_modes.is_some() => {
                let modes = item.kind.light_modes.unwrap_or(0);
                item.light = Light { on: light.on, mode: if modes > 0 { light.mode % modes } else { light.mode } };
            }
            _ => all = false,
        }
    }
    all
}

/// Whether a combo unit on the weapon is on (any mode; a light-only one
/// too; a `Flashlight` is not one).
pub fn combo_on(weapon: &Item) -> bool {
    devices(weapon).iter().any(|d| d.light.on && is(d, "TacticalCombo"))
}

/// The hip wander's scale (`UpdateHipInaccuracy`, written into the breath's
/// `HipPenalty`): by the weapon's total ergonomics E, 0.3 − E/400 with a
/// combo unit on, else 1 − clamp(E/250 − 0.15, 0, 1).
pub fn hip_penalty(ergonomics: f32, combo_on: bool) -> f32 {
    if combo_on {
        0.3 - ergonomics / 400.0
    } else {
        1.0 - (ergonomics / 250.0 - 0.15).clamp(0.0, 1.0)
    }
}
