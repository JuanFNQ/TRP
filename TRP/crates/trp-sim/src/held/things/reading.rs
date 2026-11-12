//! Making doors, containers and trunks, and reading their state and menu.

use super::*;

impl Thing {
    pub(super) fn new(kind: Kind, shut_angle: f32, open_angle: f32) -> Self {
        Self {
            kind,
            state: ThingState::Shut,
            fallback: ThingState::Shut,
            angle: shut_angle,
            shut_angle,
            open_angle,
            out: 0.0,
            open_gesture: None,
            close_gesture: None,
            hinge: None,
            can_be_breached: true,
            offers_breach: true,
            operatable: true,
            broken: false,
            key: None,
            view_target: [0.5, 1.0],
            snaps_shut: true,
            snaps_locked: false,
            breaching: false,
            failed_kick: None,
            result: None,
            record: [0.0, 0.0],
            previous: None,
            clock: 0.0,
            timers: Vec::new(),
            swings: Vec::new(),
            shut_shift: 0.0,
            sounds: Vec::new(),
        }
    }

    /// What it has sounded since this was last asked, in order.
    pub fn take_sounds(&mut self) -> Vec<ThingSound> {
        std::mem::take(&mut self.sounds)
    }

    /// A door, shut, that turns about `hinge` from `shut_angle` to
    /// `open_angle` degrees.
    pub fn door(hinge: Hinge, shut_angle: f32, open_angle: f32) -> Self {
        Self { hinge: Some(hinge), ..Self::new(Kind::Door, shut_angle, open_angle) }
    }

    /// A container, shut, with the gestures it names for its opening and
    /// its shutting (none for the default two), and the angle its moving
    /// part turns through.
    pub fn container(open: Option<Interaction>, close: Option<Interaction>, shut_angle: f32, open_angle: f32) -> Self {
        Self { open_gesture: open, close_gesture: close, ..Self::new(Kind::Container, shut_angle, open_angle) }
    }

    /// A car's trunk, shut.
    pub fn trunk(open: Option<Interaction>, close: Option<Interaction>, shut_angle: f32, open_angle: f32) -> Self {
        Self { open_gesture: open, close_gesture: close, ..Self::new(Kind::Trunk, shut_angle, open_angle) }
    }

    /// Puts it shut or open and at rest, at that state's angle.
    pub fn at_rest(&mut self, open: bool) {
        self.state = if open { ThingState::Open } else { ThingState::Shut };
        self.fallback = self.state;
        self.angle = if open { self.open_angle } else { self.shut_angle };
        self.out = if open { 1.0 } else { 0.0 };
    }

    /// Locks it: shut, at rest, and opened by `key` if it names one.
    pub fn lock(&mut self, key: Option<String>) {
        self.at_rest(false);
        (self.state, self.fallback, self.key) = (ThingState::Locked, ThingState::Locked, key);
    }

    pub fn state(&self) -> ThingState {
        self.state
    }

    /// What it was before it was held for the work.
    pub fn fallback(&self) -> ThingState {
        self.fallback
    }

    /// Whether a kick has thrown it open at some time.
    pub fn broken(&self) -> bool {
        self.broken
    }

    /// The angle it stands at about its axis, in degrees.
    pub fn angle(&self) -> f32 {
        self.angle
    }

    /// A drawer's share of the way out, 0 to 1.
    pub fn out(&self) -> f32 {
        self.out
    }

    /// What the game offers to do to it, if anything: nothing while it is
    /// being worked.
    pub fn action(&self) -> Option<Action> {
        match (self.state, self.kind) {
            (ThingState::Interacting | ThingState::Locked, _) => None,
            (ThingState::Shut, Kind::Container) => Some(Action::Search),
            (ThingState::Shut, _) => Some(Action::Open),
            (ThingState::Open, _) => Some(Action::Close),
        }
    }

    /// The menu the game shows for it (`GetAvailableActions`), for a
    /// character whose feet are at `from`: `has_key` is whether it carries
    /// the key this thing names, `lying` whether it lies or is getting up
    /// (a kick is then not to be chosen), `threshold` the settings' limit
    /// for a kick's angle (`DOOR_BREACH_THRESHOLD`). None while it is being
    /// worked. A door shut or locked has the kick and three lines that are
    /// never to be chosen; with nothing to choose the interact key does
    /// nothing.
    pub fn menu(&self, from: [f32; 3], has_key: bool, lying: bool, threshold: f32) -> Option<Menu> {
        let mut lines = Vec::new();
        match (self.state, self.kind) {
            (ThingState::Interacting, _) => return None,
            (ThingState::Locked, _) => {
                if has_key && self.key.is_some() {
                    lines.push((Action::Unlock, !self.operatable));
                }
            }
            (ThingState::Shut, Kind::Container) => lines.push((Action::Search, false)),
            (ThingState::Shut, Kind::Door) => lines.push((Action::Open, !self.operatable)),
            (ThingState::Shut, Kind::Trunk) => lines.push((Action::Open, !self.operatable)),
            (ThingState::Open, Kind::Container) => lines.push((Action::Close, false)),
            (ThingState::Open, _) => lines.push((Action::Close, !self.operatable)),
        }
        if self.kind == Kind::Door && matches!(self.state, ThingState::Shut | ThingState::Locked) {
            let kick = self.breach_angle(from, threshold) && self.offers_breach && !lying;
            lines.push((Action::Breach, !kick));
            lines.extend([Action::BangAndClear, Action::FlashAndClear, Action::MoveIn].map(|line| (line, true)));
        }
        Some(Menu::of(lines))
    }
}
