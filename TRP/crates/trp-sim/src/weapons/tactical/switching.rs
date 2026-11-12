//! The keys, the events and the devices' frame.

use super::*;

/// The devices' keys on a frame, as the input manager gives them out
/// (tactical-devices spec, "The keys"): T as it goes down, T let go, Ctrl+T
/// and T+B as they go down, H and Ctrl+H as they are let go.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Keys {
    /// `ToggleTacticalInteraction` (T+B): the other activation mode.
    pub flip_mode: bool,
    /// `Tactical` (T).
    pub tactical: bool,
    /// `ReleaseTactical` (T let go).
    pub release: bool,
    /// `NextTacticalDevice` (Ctrl+T).
    pub next: bool,
    /// `ToggleHeadLight` (H).
    pub head_toggle: bool,
    /// `SwitchHeadLight` (Ctrl+H).
    pub head_switch: bool,
}

impl Keys {
    /// The keys given on a frame, by the game's names for its bindings.
    pub fn of(given: impl Fn(&str) -> bool) -> Self {
        Self {
            flip_mode: given("ToggleTacticalInteraction"),
            tactical: given("Tactical"),
            release: given("ReleaseTactical"),
            next: given("NextTacticalDevice"),
            head_toggle: given("ToggleHeadLight"),
            head_switch: given("SwitchHeadLight"),
        }
    }
}

/// What stands around the devices' keys on a frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Around {
    /// The thing in hand.
    pub held: Option<Id>,
    /// What lets Hold mode's lights go (`GamePlayerOwner.ReleaseTactical`'s
    /// callers): a change of the hands begun (`Player.HandsChangingEvent`),
    /// a death, a screen other than the battle's, the window's focus lost.
    /// The thing in hand changing is seen here too.
    pub let_go: bool,
    /// Whether the keys are heard (not under a screen over the battle).
    pub live: bool,
    /// The frame's length, for the head light's coroutine.
    pub dt: f32,
}

/// What the devices did on a frame, for whoever plays and shows it.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The player's tactical sound (Hold mode's switches, the head light's).
    Sound,
    /// Hold mode's lights let go: forced off.
    LetGo,
    /// T+B: the activation mode to keep from now.
    ModeFlipped(InputMode),
    /// A weapon key's request: its devices' states (`(on, mode)`), written,
    /// or refused by the operation in hand, named.
    Weapon { key: WeaponKey, states: Vec<(bool, u32)>, refused_by: Option<String> },
    /// The hip penalty worked out again (`UpdateHipInaccuracy`).
    HipPenalty { penalty: f32, combo: bool },
    /// A head light key taken: its devices' states as written and whether
    /// the gesture plays.
    HeadTaken { states: Vec<(bool, u32)>, gesture: bool },
    /// A head light key refused, and why.
    HeadRefused(&'static str),
    /// The head light's coroutine at 0.8 s: its devices' models switched,
    /// with the sound.
    HeadSwitched,
    /// And at 1.4 s: its lock lifted.
    HeadUnlocked,
}

/// The devices' keys and the head light's coroutine, run each frame by the
/// app and by the replay alike (`FirearmHandsInputTranslator`,
/// `GamePlayerOwner.ReleaseTactical`, `Player.SwitchHeadLights`).
#[derive(Debug, Clone, Default)]
pub struct Devices {
    /// The head light's coroutine and lock.
    pub head: HeadLight,
    /// The thing in hand on the frame before.
    held: Option<Id>,
}

impl Devices {
    /// Begun with that thing in hand.
    pub fn holding(held: Option<Id>) -> Self {
        Self { held, ..Self::default() }
    }

    /// A frame: Hold mode's lights let go, the head light's coroutine moved
    /// on, then, where heard, T+B, the weapon's keys and the head's; and the
    /// hip penalty worked out again whenever a request has changed whether
    /// a combo unit is on. `root` holds what is worn and the thing in hand;
    /// the firearm and its motion are the weapon's in hand, where it is one.
    /// The activation mode is the one in force as the frame began.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        mode: InputMode,
        keys: Keys,
        around: Around,
        root: &mut Item,
        mut firearm: Option<&mut crate::Firearm>,
        hands: Option<&mut crate::Hands>,
        movement: &mut crate::Movement,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        // Hold mode's lights let go, forced past any refusal, with the
        // sound where any was on (`ReleaseTactical`).
        if mode == InputMode::Hold && (around.let_go || around.held != self.held) {
            if let Some(request) = self.held.and_then(|id| root.find(id)).and_then(|weapon| weapon_request(weapon, WeaponKey::Up)) {
                apply(root, &request.states);
                events.extend([Event::LetGo, Event::Sound]);
            }
        }
        self.held = around.held;

        // The head light's coroutine: the models and the sound at 0.8 s,
        // the lock lifted 0.6 s later.
        let step = self.head.step(around.dt);
        if step.switched {
            events.extend([Event::HeadSwitched, Event::Sound]);
        }
        if step.unlocked {
            events.push(Event::HeadUnlocked);
        }

        if around.live {
            if keys.flip_mode {
                events.push(Event::ModeFlipped(mode.flipped()));
            }
            // The weapon's keys, each a request to the firearm
            // (`SetLightsState`), refused by the operation in hand unless
            // forced.
            let mut asked = Vec::new();
            match mode {
                InputMode::Press if keys.tactical => asked.push(WeaponKey::Toggle),
                InputMode::Hold if keys.tactical => asked.push(WeaponKey::Down),
                InputMode::Hold if keys.release => asked.push(WeaponKey::Up),
                _ => {}
            }
            if keys.next {
                asked.push(WeaponKey::Next);
            }
            for key in asked {
                let Some(id) = around.held else { continue };
                let Some(request) = root.find(id).and_then(|weapon| weapon_request(weapon, key)) else { continue };
                let Some(firearm) = firearm.as_deref_mut() else { continue };
                let states = request.states.iter().map(|(_, l)| (l.on, l.mode)).collect();
                if !request.forced && !firearm.can_change_lights() {
                    events.push(Event::Weapon { key, states, refused_by: Some(firearm.operation_said()) });
                    continue;
                }
                apply(root, &request.states);
                if request.animated {
                    firearm.mod_switch();
                }
                if request.sound {
                    events.push(Event::Sound);
                }
                events.push(Event::Weapon { key, states, refused_by: None });
            }

            // The head light's keys (`Player.SwitchHeadLights`): refused
            // while its lock is on, while the hands may not act, or with no
            // device; else written at once and the gesture asked, which may
            // be refused while the switch still comes.
            let (toggle, switch) = if keys.head_switch { (false, true) } else { (keys.head_toggle, false) };
            if toggle || switch {
                let found = head_devices(HEAD_SLOTS.iter().filter_map(|slot| root.in_slot(slot)));
                let refused = if self.head.locked() {
                    Some("its lock is on")
                } else if !movement.state_suits_hands() {
                    Some("the hands may not act")
                } else if found.is_empty() {
                    Some("nothing on the head carries a light")
                } else {
                    None
                };
                match refused {
                    Some(why) => events.push(Event::HeadRefused(why)),
                    None => {
                        let states = head_request(&found, toggle, switch);
                        apply(root, &states);
                        self.head.press();
                        let gesture = movement.set_interaction(crate::Interaction::HelmetRailGear);
                        events.push(Event::HeadTaken { states: states.iter().map(|(_, l)| (l.on, l.mode)).collect(), gesture });
                    }
                }
            }
        }

        // The hip penalty, worked out again as a request is taken, the
        // forced one of a let-go too (`UpdateHipInaccuracy`).
        if let (Some(id), Some(_), Some(hands)) = (around.held, firearm.as_ref(), hands) {
            let combo = root.find(id).is_some_and(combo_on);
            if hands.combo_on() != combo {
                hands.update_hip_inaccuracy(combo);
                events.push(Event::HipPenalty { penalty: hands.hip_penalty(), combo });
            }
        }
        events
    }
}
