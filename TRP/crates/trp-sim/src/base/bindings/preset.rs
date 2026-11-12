//! The preset's state machine types and making a preset.

use super::*;

/// A key or a way of the wheel among the manager's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyId {
    Key(Code),
    Wheel(bool),
}

/// A key as the combinations share it: what it is on this frame, which
/// combination has it, and whether that one lets go of it next frame.
#[derive(Debug, Clone)]
pub(super) struct KeyState {
    pub(super) id: KeyId,
    pub(super) press: KeyPress,
    pub(super) in_use_by: Option<usize>,
    pub(super) release_next_frame: bool,
}

/// Where a combination of keys is in being pressed (`EKeyState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
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
pub(super) enum Kind {
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
pub(super) struct Combination {
    pub(super) keys: Vec<usize>,
    /// How many keys it counts as having.
    pub(super) count: usize,
    /// Nothing takes a key from it (`IsImportant`).
    pub(super) important: bool,
    pub(super) kind: Kind,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Progress {
    pub(super) state: State,
    pub(super) time_left: f32,
    pub(super) value: f32,
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
pub(super) struct LookAxes {
    pub(super) group: usize,
    pub(super) modifiers: Vec<usize>,
    pub(super) count: usize,
    pub(super) has: [bool; 2],
}

/// The manager's combinations for a set of bindings (`InputPreset`).
#[derive(Debug, Clone)]
pub struct Preset {
    pub(super) keys: Vec<KeyState>,
    pub(super) combinations: Vec<Combination>,
    pub(super) progress: Vec<Progress>,
    /// The head's free look, a pair of axes for each variant of its
    /// binding that has keys; none where it has none.
    pub(super) look: Vec<LookAxes>,
    /// The axes' combinations in the order their pushes are taken.
    pub(super) axis_order: Vec<usize>,
    pub(super) axes: usize,
    pub(super) double_click: f32,
}

/// What stands in for the engine's hash of a way of the wheel in the sums
/// combinations are told apart by: any two figures no sum of keys reaches.
const WHEEL_HASH: [i64; 2] = [1_000_003, 2_000_003];

impl Preset {
    /// The combinations of a set of bindings (`InputPreset.UpdateBindings`).
    pub fn new(controls: &Controls) -> Self {
        let mut keys: Vec<KeyState> = Vec::new();
        // A key the manager has for it, or with `own` one of its own
        // (`ignorePool`).
        let mut key_of = |id: KeyId, own: bool| -> usize {
            match keys.iter().position(|key| key.id == id).filter(|_| !own) {
                Some(at) => at,
                None => {
                    keys.push(KeyState { id, press: KeyPress::None, in_use_by: None, release_next_frame: false });
                    keys.len() - 1
                }
            }
        };
        // A combination for every variant that has keys, with the sum of
        // its keys' numbers (a way of the wheel counts only when alone).
        let mut made: Vec<(Combination, i64, i64)> = Vec::new();
        let mut look = Vec::new();
        for (index, group) in controls.keys.iter().enumerate() {
            for variant in group.variants.iter().filter(|variant| !variant.is_empty()) {
                // A generated binding's keys are its own.
                let own = group.own_keys();
                let mut held: Vec<(usize, i64, bool)> = variant.keys.iter().map(|code| (key_of(KeyId::Key(*code), own), *code as i64, false)).collect();
                if let Some(up) = variant.wheel {
                    held.push((key_of(KeyId::Wheel(up), own), WHEEL_HASH[up as usize], true));
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
                // be held, nor from the throws' (held), nor from the
                // tactical device's release.
                let important = (matches!(name, "Aim" | "HighThrow" | "LowThrow") && group.press == PressType::Continuous) || name == "ReleaseTactical";
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
                let mut up: Vec<usize> = ids(positive).into_iter().map(|id| key_of(id, false)).collect();
                let (mut down, mut common) = (Vec::new(), Vec::new());
                for key in ids(negative).into_iter().map(|id| key_of(id, false)) {
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
}
