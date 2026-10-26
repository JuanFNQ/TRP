//! The things of the world the hands work: a door, a container, a car's
//! trunk (`WorldInteractiveObject` and its kinds `Door`, `LootableContainer`
//! and `Trunk`; interactions spec, "A thing's own part").
//!
//! A thing is shut or open, and "being worked" from the key until it has
//! done what it was told. Told to open or to shut, it waits (a door half a
//! second of its own, a container or a trunk the install's
//! `DelayToOpenContainer`, 0.55 s), then turns from the angle it stands at
//! to its new one along the settings object's `DoorCurve`: at the curve's
//! own pace for a door, so 2.5 s, and half as fast again for a container or
//! a trunk, 1.67 s. A drawer is moved out and back in by the same curve.
//! Each frame the thing stands where the curve has it at the time run so
//! far, and the time is then moved on by the frame; the frame that finds
//! the curve's end passed puts it at its new angle and in its new state.
//!
//! Which gesture the hand makes for it is the thing's to say: a door's by
//! where the character stands to it and which way it will swing, a
//! container's or a trunk's the two it names (or the default two).
//!
//! Set beside fourteen recordings made by hand (2026-10-12): four doors'
//! swings within 0.07 degrees on every frame, six lids' and a trunk's
//! within 0.04, and every new state on the recorded frame.

use trp_data::curve::Curve;
use trp_data::hard_settings::HardSettings;

use crate::interactions::Interaction;

/// How long a door waits after it is told before it begins to turn, in
/// seconds: written in the door's own routine (`Door.SmoothDoorOpenCoroutine`),
/// where a container and a trunk wait the settings' `DelayToOpenContainer`.
pub const DOOR_WAITS: f32 = 0.5;

/// How much faster than the curve's own pace a container or a trunk turns
/// (`SetDoorState` asks for 1.5; a door's routine drops it and runs at 1).
const CONTAINER_PACE: f32 = 1.5;

/// What a thing sounds, each on the frame the game plays it (interactions
/// spec, "A thing's sounds"). The clips, their volumes and where they
/// sound from are the host's: a thing here says only which and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingSound {
    /// A swing's first frame (`SmoothDoorOpenCoroutine`): one of the open
    /// clips if the swing opens (a kick's does not count as opening), and
    /// one of the squeak clips unless the thing was locked, both at one
    /// drawn volume of the opening's range.
    Swing { opens: bool, squeaks: bool },
    /// One of the shut clips (`PlayShut`): the frame a shutting swing's
    /// curve time passes the thing's shut shift before the end, or its
    /// last frame if it never did; not for a thing that was locked.
    Shut,
    /// A kick lands and throws the door open (`Door.KickOpen`): its breach
    /// clip, at full volume, before its swing's own sound.
    Breach,
    /// A kick lands and the door holds (`Door.FailBreach`): its hit clip,
    /// at full volume.
    Hit,
}

/// The kind of thing, which decides how it waits and turns, which gesture
/// it asks for and what the game offers to do to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Swings about its hinge; its gesture goes by where the character
    /// stands to it.
    Door,
    /// Searched, and then shown in the inventory screen beside the
    /// equipment; told to shut when the screen shuts.
    Container,
    /// A car's trunk: opened and shut, nothing shown.
    Trunk,
}

/// Where a thing stands, as the game's `EDoorState` has it (the states built).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingState {
    Shut,
    Open,
    /// From the key until it has done what it was told: the game offers
    /// nothing to do to it meanwhile.
    Interacting,
    /// Shut, and not to be opened by hand until it is unlocked with its
    /// key or kicked in.
    Locked,
}

/// What the game offers to do to a thing looked at, as its menu has it
/// (`InteractionContextHelper.GetAvailableActions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Open,
    Close,
    /// A shut container: opened and, if the character stays by it, shown.
    Search,
    /// A locked thing, by a character that carries its key.
    Unlock,
    /// A shut or a locked door: kicked.
    Breach,
    /// Three lines the game's menu has for a door and never lets be
    /// chosen.
    BangAndClear,
    FlashAndClear,
    MoveIn,
    /// A thing lying loose: taken up.
    Take,
    /// The workbench on the range (the user's, 2026-10-20): the edit build
    /// screen opened for the weapon in hand, by the game's words for its
    /// item menu's line (`Editbuild`). The game's workbench offers no line
    /// of its own.
    EditBuild,
    /// The workbench on the range (the user's, 2026-10-25: "Give the
    /// workbench one more option in the interact menu: Repair Gear - Only
    /// pressable when you have a repair kit, much like a key"): the
    /// inventory opened for repairing, a repair kit let go on what it
    /// mends opening the game's repair window. The game repairs out of a
    /// raid only, by its item menu's line.
    RepairGear,
}

impl Action {
    /// The game's own name for the line, which its texts are looked up by.
    pub fn name(self) -> &'static str {
        match self {
            Action::Open => "OpenDoor",
            Action::Close => "CloseDoor",
            Action::Search => "Search",
            Action::Unlock => "Unlock",
            Action::Breach => "Breach",
            Action::BangAndClear => "Bang & clear",
            Action::FlashAndClear => "Flash & clear",
            Action::MoveIn => "Move in",
            Action::Take => "Take",
            Action::EditBuild => "Editbuild",
            // (No text of the game's: the user's words.)
            Action::RepairGear => "Repair gear",
        }
    }
}

/// The menu for the thing looked at (`AvailableInteractionState`, shown by
/// `ActionPanel`): every line the game offers, those that cannot be chosen
/// among them, and the one the interact key would do. The wheel moves the
/// choice over the lines that can be chosen, round and round.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Menu {
    /// Each line, and whether it is disabled.
    pub lines: Vec<(Action, bool)>,
    pub selected: Option<usize>,
}

impl Menu {
    /// A menu of these lines with the first that can be chosen chosen
    /// (`InitSelected`).
    pub fn of(lines: Vec<(Action, bool)>) -> Self {
        let selected = lines.iter().position(|(_, disabled)| !disabled);
        Self { lines, selected }
    }

    /// The line the interact key would do.
    pub fn chosen(&self) -> Option<Action> {
        self.selected.and_then(|index| self.lines.get(index)).map(|(action, _)| *action)
    }

    /// The wheel, a notch: down the list (`SelectNextAction`) or up it
    /// (`SelectPreviousAction`), past the lines that cannot be chosen and
    /// round from the end. With nothing to choose it does nothing.
    pub fn scroll(&mut self, down: bool) {
        let count = self.lines.len();
        if count == 0 || self.lines.iter().all(|(_, disabled)| *disabled) {
            return;
        }
        // With nothing chosen the game's search for the chosen line comes
        // back with one before the first.
        let mut at = self.selected.map_or(-1, |index| index as i32);
        loop {
            at = (at + if down { 1 } else { -1 }).rem_euclid(count as i32);
            if !self.lines[at as usize].1 {
                break;
            }
        }
        self.selected = Some(at as usize);
    }
}

/// How far off a door's leaf, and how far along it from the hinge, a kick
/// is made from, in metres (`Door.GetBreakInPosition`).
const BREAK_IN_OFF: f32 = 1.3;
const BREAK_IN_ALONG: f32 = 0.5;

/// A kick as it begins: where it is made from, what it is aimed at,
/// whether the door puts the character at that place first, and whether
/// it will throw the door open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breach {
    pub position: [f32; 3],
    pub view_target: [f32; 3],
    pub snaps: bool,
    pub opens: bool,
}

/// How long a kick takes to throw a door open, in seconds, and the slope of
/// its curve at both ends (`Door.KickCurve`, as the class has it: from
/// nothing to all of the turn).
const KICK_LASTS: f32 = 0.5;
const KICK_SLOPE: f32 = 1.0;
/// How long after a kick that fails the door is what it was again, in
/// seconds (`Door.FailBreachRoutine`).
pub const FAILED_KICK_SHOWS: f32 = 0.3;

/// The two figures every thing goes by, from the install.
#[derive(Debug, Clone)]
pub struct ThingSettings {
    /// The way through a swing, its share against seconds (`DoorCurve`).
    pub curve: Curve,
    /// How long a container or a trunk waits after it is told, in seconds
    /// (`DelayToOpenContainer`, 0.55 in the install).
    pub answers_after: f32,
}

impl ThingSettings {
    pub fn new(hard: &HardSettings) -> anyhow::Result<Self> {
        Ok(Self { curve: hard.curve("DoorCurve")?.clone(), answers_after: hard.float("DelayToOpenContainer")? })
    }

    /// How long the curve runs, in seconds (`GetDuration`: its last key's time).
    fn duration(&self) -> f32 {
        self.curve.keys.last().map_or(0.0, |key| key.time)
    }
}

/// How a door stands and turns, in the simulation's axes (the game's: x
/// right, y up, z ahead).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hinge {
    /// A point on the line it turns about (the door's own place).
    pub at: [f32; 3],
    /// The line it turns about, a unit vector. An angle turns the door as
    /// the game's `Quaternion.AngleAxis` does about it.
    pub axis: [f32; 3],
    /// Which way the door points from its hinge at an angle of nothing
    /// (its `DoorForward` axis), a unit vector square to `axis`.
    pub forward: [f32; 3],
}

/// One run of a thing's routine.
#[derive(Debug, Clone, Copy)]
struct Swing {
    /// A door's own wait before it turns: how long it has waited, how long
    /// it waits, and whether it began this frame (that frame does not count).
    wait: Option<(f32, f32, bool)>,
    /// The angle it began from, the turn to make, and whether it opens (a
    /// drawer's way out goes by the latter).
    from: f32,
    turn: f32,
    opening: bool,
    /// Seconds of the curve run so far, and how fast they run.
    progress: f32,
    pace: f32,
    /// What the thing is when the swing is over.
    to: ThingState,
    /// Thrown open by a kick: along the kick's own curve, not the
    /// settings'.
    kicked: bool,
    /// Whether its first frame has run, which sounds its opening, and
    /// whether a shutting has sounded its shut clip (`_shutPlayed`).
    begun: bool,
    shut_played: bool,
}

/// A container's or a trunk's wait before it does what it was told.
#[derive(Debug, Clone, Copy)]
struct Timer {
    waited: f32,
    fresh: bool,
    to: ThingState,
}

/// A door, a container or a trunk.
#[derive(Debug, Clone)]
pub struct Thing {
    pub kind: Kind,
    state: ThingState,
    /// What it was before it was being worked (`FallbackState`).
    fallback: ThingState,
    angle: f32,
    /// Its angles shut and open (`CloseAngle`, `OpenAngle`), in degrees.
    pub shut_angle: f32,
    pub open_angle: f32,
    /// A drawer's share of the way out: 0 in, 1 out.
    out: f32,
    /// The gestures a container or a trunk names for its opening and its
    /// shutting (`_openInteraction`, `_closeInteraction`); none for the
    /// game's default two.
    open_gesture: Option<Interaction>,
    close_gesture: Option<Interaction>,
    /// A door's hinge.
    pub hinge: Option<Hinge>,
    /// Whether a kick can throw it open, and whether the menu offers one
    /// (`CanBeBreached`, `CanInteractWithBreach`); whether it can be worked
    /// at all (`Operatable`); and whether a kick has broken it (`IsBroken`).
    pub can_be_breached: bool,
    pub offers_breach: bool,
    pub operatable: bool,
    broken: bool,
    /// The key that unlocks it, by its item's key id (`KeyId`).
    pub key: Option<String>,
    /// The point of it a kick is aimed at, from the hinge along the leaf
    /// and up, in metres (`viewTarget1`, in the door's own turning frame).
    pub view_target: [f32; 2],
    /// Whether it puts a character that kicks it at a place of its own
    /// first, shut and locked (`Snap`, a set of the door's states: each
    /// door's own).
    pub snaps_shut: bool,
    pub snaps_locked: bool,
    /// The last order taken was a kick (`_interaction.ResultState` is
    /// `Breaching`).
    breaching: bool,
    /// A kick that failed is waiting to put it back as it was.
    failed_kick: Option<(f32, bool)>,
    /// What the last order taken was for (`_interaction.ResultState`),
    /// and how far the game's shared record of a swing has run against
    /// how long it is (`ProgressTime`, `Duration`): a container's routine
    /// keeps its own count and never moves it on.
    result: Option<ThingState>,
    record: [f32; 2],
    /// The last swing of the game's own routine that ran out (a door's,
    /// a trunk's): what it was for and when it ended, on this thing's clock.
    previous: Option<(ThingState, f32)>,
    clock: f32,
    timers: Vec<Timer>,
    swings: Vec<Swing>,
    /// Seconds of the curve before its end at which a shutting sounds its
    /// shut clip (`ShutShift`, the prefab's: 0 unless it says).
    pub shut_shift: f32,
    /// What it has sounded and the host has not yet taken.
    sounds: Vec<ThingSound>,
}

impl Thing {
    fn new(kind: Kind, shut_angle: f32, open_angle: f32) -> Self {
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

    /// The door's line shut and open, from where it stands now, and the
    /// way from the character to the point a kick is aimed at, all flat on
    /// the ground.
    fn breach_lines(&self, from: [f32; 3]) -> Option<([f32; 2], [f32; 2], [f32; 2])> {
        let hinge = self.hinge?;
        let own = turn(hinge.forward, hinge.axis, self.angle);
        let (shut, open) = (turn(own, hinge.axis, self.shut_angle), turn(own, hinge.axis, self.open_angle));
        // The point is the door's own, and turns with it.
        let target: [f32; 3] = std::array::from_fn(|k| hinge.at[k] + own[k] * self.view_target[0] + hinge.axis[k].abs() * self.view_target[1]);
        let way = flat_unit(sub(target, from));
        Some((flat_unit(shut), flat_unit([shut[0] + open[0], 0.0, shut[2] + open[2]]), way))
    }

    /// Whether the character stands where a kick is offered
    /// (`Door.IsBreachAngle`): not too far along the door's own line.
    pub fn breach_angle(&self, from: [f32; 3], threshold: f32) -> bool {
        self.breach_lines(from).is_some_and(|(shut, _, way)| (way[0] * shut[0] + way[1] * shut[1]).abs() < threshold)
    }

    /// Whether a kick from there throws it open (`Door.BreachSuccessRoll`,
    /// which rolls nothing): only one that can be breached, and only from
    /// the side it swings away from.
    pub fn breach_succeeds(&self, from: [f32; 3]) -> bool {
        self.can_be_breached
            && self.operatable
            && self.breach_lines(from).is_some_and(|(_, both, way)| way[0] * both[0] + way[1] * both[1] > 0.0)
    }

    /// Where a door is kicked from and what the kick is aimed at
    /// (`Door.GetBreakInPosition`, `GetBreakInParameters`), for a character
    /// whose feet are at `from`: the place 1.3 m square off the leaf, half
    /// a metre along it from the hinge, on the character's side of it and
    /// at the height of its feet; and the point of the leaf half a metre
    /// along it, at the height of the door's own aiming point.
    pub fn break_in(&self, from: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
        let hinge = self.hinge?;
        let along = turn(hinge.forward, hinge.axis, self.angle);
        let off = cross(along, [0.0, 1.0, 0.0]);
        let place = |side: f32| -> [f32; 3] {
            let at: [f32; 3] = std::array::from_fn(|k| hinge.at[k] + side * BREAK_IN_OFF * off[k] + BREAK_IN_ALONG * along[k]);
            [at[0], from[1], at[2]]
        };
        let (one, other) = (place(1.0), place(-1.0));
        let far = |at: [f32; 3]| dot(sub(at, from), sub(at, from));
        let position = if far(one) < far(other) { one } else { other };
        let target = [hinge.at[0] + BREAK_IN_ALONG * along[0], hinge.at[1] + self.view_target[1], hinge.at[2] + BREAK_IN_ALONG * along[2]];
        Some((position, target))
    }

    /// A kick is begun at it by a character whose feet are at `from`
    /// (`MovementContext.StartDoorBreak`): where it is kicked from and
    /// aimed at, whether the door puts the character there first, and
    /// whether the kick will throw it open, which goes by the place it is
    /// kicked from. The door is held for it from now.
    pub fn start_breach(&mut self, from: [f32; 3]) -> Option<Breach> {
        let (position, view_target) = self.break_in(from)?;
        let snaps = match self.state {
            ThingState::Shut => self.snaps_shut,
            ThingState::Locked => self.snaps_locked,
            _ => false,
        };
        let opens = self.breach_succeeds(position);
        self.hold();
        Some(Breach { position, view_target, snaps, opens })
    }

    /// Held for the work (`LockForInteraction`): being worked, with what
    /// it was noted. A kick holds the door from the key on.
    pub fn hold(&mut self) {
        if self.state != ThingState::Interacting {
            self.fallback = self.state;
            self.state = ThingState::Interacting;
        }
    }

    /// The kick lands and throws the door open (`Door.KickOpen`): from its
    /// shut angle to its open one in half a second, with no wait, and open
    /// when that is over. Not while one kick's swing is already the order
    /// in hand.
    pub fn kick_open(&mut self) {
        if self.breaching {
            return;
        }
        (self.breaching, self.result, self.broken) = (true, None, true);
        self.record = [0.0, KICK_LASTS];
        // The breach clip, and the swing's own opening at once: the
        // routine runs to its first yield as it is started, and a kick's
        // swing has no wait.
        self.sounds.push(ThingSound::Breach);
        self.sounds.push(ThingSound::Swing { opens: false, squeaks: self.fallback != ThingState::Locked });
        self.swings.push(Swing {
            wait: None,
            from: self.shut_angle,
            turn: delta_angle(self.shut_angle, self.open_angle),
            opening: true,
            progress: 0.0,
            pace: 1.0,
            to: ThingState::Open,
            kicked: true,
            begun: true,
            shut_played: false,
        });
    }

    /// The kick lands and the door holds (`Door.FailBreach`): three tenths
    /// of a second on it is what it was again, unless it is swinging.
    pub fn fail_breach(&mut self) {
        self.sounds.push(ThingSound::Hit);
        self.failed_kick = Some((0.0, true));
    }

    /// Unlocked with its key (`Unlock`): shut, at once.
    pub fn unlock(&mut self) {
        (self.state, self.fallback) = (ThingState::Shut, ThingState::Shut);
        self.angle = self.shut_angle;
    }

    /// The gesture it asks of a character whose feet are at `from`
    /// (`GetInteractionParameters`): none for a container or a trunk being
    /// worked, whose number is then nothing.
    pub fn gesture(&self, from: [f32; 3]) -> Option<Interaction> {
        match self.kind {
            Kind::Door => Interaction::of_game_number(self.door_gesture(from)),
            Kind::Container | Kind::Trunk => match self.state {
                ThingState::Shut => Some(self.open_gesture.unwrap_or(Interaction::ContainerOpenDefault)),
                ThingState::Open => Some(self.close_gesture.unwrap_or(Interaction::ContainerCloseDefault)),
                ThingState::Interacting | ThingState::Locked => None,
            },
        }
    }

    /// A door's gesture by the game's own number (`Door.CalculateInteractionIndex`).
    /// Of the line the door lies along now and the line it will lie along
    /// when it has swung, it asks on which side of the first the character
    /// stands against the side the door will swing to (towards the
    /// character or away), and on which hand the hinge is (the upward part
    /// of a cross product). Both lines are the door's own axis turned by its
    /// angles from where the door stands now.
    pub fn door_gesture(&self, from: [f32; 3]) -> u8 {
        let Some(hinge) = self.hinge else { return 0 };
        // The door's own forward axis turns with it.
        let own = turn(hinge.forward, hinge.axis, self.angle);
        let (now, then) = match self.state {
            ThingState::Shut => (turn(own, hinge.axis, self.shut_angle), turn(own, hinge.axis, self.open_angle)),
            ThingState::Open => (turn(own, hinge.axis, -self.shut_angle), turn(own, hinge.axis, -self.open_angle)),
            ThingState::Interacting | ThingState::Locked => ([0.0; 3], [0.0; 3]),
        };
        let to_character = sub(from, hinge.at);
        let foot = sub(project(to_character, now), to_character);
        let side = sub(then, project(then, now));
        let away = dot(foot, side) > 0.0;
        let right = cross(now, foot)[1] > 0.0;
        match (self.state, right, away) {
            (ThingState::Shut, _, true) => 27,
            (ThingState::Shut, true, false) => 26,
            (ThingState::Shut, false, false) => 24,
            (ThingState::Open, true, true) => 25,
            (ThingState::Open, false, true) => 23,
            _ => 28,
        }
    }

    /// The character works it (`MovementState.ExecuteDoorInteraction`):
    /// the gesture for its state and the character's place is worked out,
    /// it is held for the work (`LockForInteraction`) and told what to do
    /// (`Interact`). Returns the gesture to ask of the hand.
    pub fn work(&mut self, open: bool, from: [f32; 3], settings: &ThingSettings) -> Option<Interaction> {
        let gesture = self.gesture(from);
        if self.state != ThingState::Interacting {
            self.fallback = self.state;
            self.state = ThingState::Interacting;
        }
        self.tell(open, settings);
        gesture
    }

    /// Told to open or to shut with no hand (`Interact`), as a container is
    /// when its screen shuts, or when it was searched and the character
    /// went off before it was shown. A door does it at once and waits in
    /// its own routine; a container and a trunk wait first.
    pub fn tell(&mut self, open: bool, settings: &ThingSettings) {
        let to = if open { ThingState::Open } else { ThingState::Shut };
        match self.kind {
            Kind::Door => self.set_state(to, settings),
            Kind::Container | Kind::Trunk => self.timers.push(Timer { waited: 0.0, fresh: true, to }),
        }
    }

    /// Puts it on its way to a new state, if the game takes the order
    /// (`SetDoorState`, `CanStartInteraction`): not to the state it is in,
    /// nor to the one it is already on its way to, nor to the one its last
    /// swing ended in until a swing's length has passed since. Refused, a
    /// thing held for the work and not swinging goes back to what it was.
    fn set_state(&mut self, to: ThingState, settings: &ThingSettings) {
        let duration = settings.duration();
        let recent = self.previous.is_some_and(|(was, ended)| was == to && ended + self.record[1] >= self.clock);
        let swinging = self.record[0] < self.record[1];
        if self.state == to || self.result == Some(to) || recent {
            if !swinging && self.state == ThingState::Interacting {
                self.state = self.fallback;
            }
            return;
        }
        (self.result, self.breaching) = (Some(to), false);
        self.record = [0.0, duration];
        let target = if to == ThingState::Open { self.open_angle } else { self.shut_angle };
        let mut swing = Swing {
            wait: None,
            from: self.angle,
            turn: delta_angle(self.angle, target),
            opening: to == ThingState::Open,
            progress: 0.0,
            pace: CONTAINER_PACE,
            to,
            kicked: false,
            begun: false,
            shut_played: false,
        };
        match self.kind {
            Kind::Door => {
                swing.wait = Some((0.0, DOOR_WAITS, true));
                swing.pace = 1.0;
                self.swings.push(swing);
            }
            // The routine begins at once and runs its first frame now.
            Kind::Container | Kind::Trunk => {
                self.swings.push(swing);
            }
        }
    }

    /// One frame of `dt` seconds. The routines run after the character's
    /// own update in the game's frame, so a host steps a thing after the
    /// character.
    pub fn step(&mut self, dt: f32, settings: &ThingSettings) {
        self.clock += dt;
        let duration = settings.duration();
        let mut index = 0;
        while index < self.swings.len() {
            let mut swing = self.swings[index];
            if let Some((waited, length, fresh)) = swing.wait {
                let waited = if fresh { waited } else { waited + dt };
                if waited + 1e-6 < length {
                    self.swings[index].wait = Some((waited, length, false));
                    index += 1;
                    continue;
                }
                swing.wait = None;
            }
            if swing.progress >= if swing.kicked { KICK_LASTS } else { duration } {
                self.finish(swing);
                self.swings.remove(index);
                continue;
            }
            self.run_frame(&mut swing, dt, settings);
            self.swings[index] = swing;
            index += 1;
        }
        // A kick that failed: the frame it landed on does not count, and
        // the first frame three tenths of a second on puts the door back,
        // unless a swing is under way.
        if let Some((waited, fresh)) = self.failed_kick {
            let waited = if fresh { waited } else { waited + dt };
            if waited + 1e-6 < FAILED_KICK_SHOWS {
                self.failed_kick = Some((waited, false));
            } else {
                self.failed_kick = None;
                if self.record[0] >= self.record[1] {
                    self.state = self.fallback;
                }
            }
        }
        // A container's or a trunk's wait: the frame it was told on does
        // not count, and the frame it runs out on begins the swing, which
        // runs its first frame at once.
        let mut index = 0;
        while index < self.timers.len() {
            let timer = &mut self.timers[index];
            if !std::mem::take(&mut timer.fresh) {
                timer.waited += dt;
            }
            if timer.waited + 1e-6 < settings.answers_after {
                index += 1;
                continue;
            }
            let to = self.timers.remove(index).to;
            let before = self.swings.len();
            self.set_state(to, settings);
            if self.swings.len() > before {
                let mut swing = self.swings[before];
                self.run_frame(&mut swing, dt, settings);
                self.swings[before] = swing;
            }
        }
    }

    /// One frame of a swing past its wait: its first frame sounds the
    /// opening (`PlaySound` as the routine reaches the swing), the thing
    /// stands where the curve has it, the time moves on, and a shutting
    /// sounds its shut clip on the frame the time passes the shift before
    /// the curve's end (`ProgressTime > Duration - ShutShift`, read after
    /// the frame's `UpdateTime`).
    fn run_frame(&mut self, swing: &mut Swing, dt: f32, settings: &ThingSettings) {
        if !swing.begun {
            swing.begun = true;
            self.sounds.push(ThingSound::Swing { opens: swing.opening && !swing.kicked, squeaks: self.fallback != ThingState::Locked });
        }
        self.place(swing, settings);
        swing.progress += dt * swing.pace;
        if self.kind != Kind::Container {
            // The game's own routine keeps the shared record.
            self.record[0] = swing.progress;
        }
        if !swing.opening && !swing.shut_played && swing.progress > settings.duration() - self.shut_shift && self.fallback != ThingState::Locked {
            swing.shut_played = true;
            self.sounds.push(ThingSound::Shut);
        }
    }

    /// Where the curve has it at the swing's time so far.
    fn place(&mut self, swing: &Swing, settings: &ThingSettings) {
        let share = if swing.kicked { kick_share(swing.progress) } else { settings.curve.evaluate(swing.progress) };
        self.angle = swing.from + share * swing.turn;
        self.out = if swing.opening { share } else { 1.0 - share };
    }

    /// The swing is over: the thing is at its new angle and in its new
    /// state (a door's or a trunk's the last order's, a container's its own).
    fn finish(&mut self, swing: Swing) {
        // A shutting that never reached its shift sounds its clip as it ends.
        if !swing.opening && !swing.shut_played && !swing.kicked && self.fallback != ThingState::Locked {
            self.sounds.push(ThingSound::Shut);
        }
        let state = match self.kind {
            Kind::Container => swing.to,
            Kind::Door | Kind::Trunk => self.result.unwrap_or(swing.to),
        };
        self.state = state;
        self.angle = swing.from + swing.turn;
        self.out = if state == ThingState::Open { 1.0 } else { 0.0 };
        if self.kind != Kind::Container {
            // A kick's swing ends as a breach, which is no state an order
            // is ever for: it refuses none after it.
            self.previous = (!swing.kicked).then_some((state, self.clock));
        }
    }

    /// Whether it is waiting or swinging.
    pub fn busy(&self) -> bool {
        !self.timers.is_empty() || !self.swings.is_empty() || self.failed_kick.is_some()
    }
}

/// How far through its turn a kicked door is, so many seconds after the
/// kick: the kick's own curve, from nothing at a slope of one to all of it
/// at a slope of one, half a second long.
fn kick_share(seconds: f32) -> f32 {
    let key = |time, value| trp_data::curve::Key { time, value, in_slope: KICK_SLOPE, out_slope: KICK_SLOPE };
    Curve { keys: vec![key(0.0, 0.0), key(KICK_LASTS, 1.0)] }.evaluate(seconds)
}

/// A vector's part along the ground, at length one (nothing for none).
fn flat_unit(v: [f32; 3]) -> [f32; 2] {
    let size = (v[0] * v[0] + v[2] * v[2]).sqrt();
    if size < f32::EPSILON { [0.0; 2] } else { [v[0] / size, v[2] / size] }
}

/// The shortest turn from one angle to another, in degrees, from -180 to
/// 180 (`Mathf.DeltaAngle`).
pub fn delta_angle(from: f32, to: f32) -> f32 {
    let mut d = (to - from).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    d
}

/// `v` turned by `degrees` about the unit vector `axis`.
pub fn turn(v: [f32; 3], axis: [f32; 3], degrees: f32) -> [f32; 3] {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let along = dot(v, axis);
    let across = cross(axis, v);
    std::array::from_fn(|k| v[k] * cos + across[k] * sin + axis[k] * along * (1.0 - cos))
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| a[k] - b[k])
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `v` along `onto` (`Vector3.Project`: nothing onto nothing).
fn project(v: [f32; 3], onto: [f32; 3]) -> [f32; 3] {
    let size = dot(onto, onto);
    if size < f32::EPSILON {
        return [0.0; 3];
    }
    let share = dot(v, onto) / size;
    onto.map(|o| o * share)
}

#[cfg(test)]
mod tests {
    use super::*;
    use trp_data::curve::Key;

    /// The settings object's `DoorCurve` as the install has it (read with
    /// `probe hardsettings DoorCurve`), and its `DelayToOpenContainer`.
    fn settings() -> ThingSettings {
        let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
        let curve = Curve {
            keys: vec![key(0.0, 0.0, 1.11), key(0.568, 0.546, 0.61), key(1.718, 0.976, 0.10), key(2.5, 1.0, 0.01)],
        };
        ThingSettings { curve, answers_after: 0.55 }
    }

    /// A door upright in a wall that runs along x, hinged at the origin,
    /// pointing along +x when shut and turning about straight up, so that
    /// it opens towards -z.
    fn door() -> Thing {
        Thing::door(Hinge { at: [0.0; 3], axis: [0.0, 1.0, 0.0], forward: [1.0, 0.0, 0.0] }, 0.0, 110.0)
    }

    const DT: f32 = 1.0 / 120.0;

    /// Frames until the thing is no longer being worked, stepping it. The
    /// key's own frame has been stepped before.
    fn frames_until_done(thing: &mut Thing, settings: &ThingSettings) -> usize {
        (1..2000).find(|_| {
            thing.step(DT, settings);
            thing.state() != ThingState::Interacting
        })
        .unwrap()
    }

    /// A door is offered to be opened shut and to be shut open, and nothing
    /// while it is being worked; it waits half a second, swings for the
    /// curve's 2.5 s, and is open on the frame 3 s after the key.
    #[test]
    fn a_door_waits_half_a_second_and_swings_for_the_curve() {
        let settings = settings();
        let mut door = door();
        assert_eq!(door.action(), Some(Action::Open));
        door.work(true, [0.5, 0.0, 1.0], &settings);
        assert_eq!((door.state(), door.action()), (ThingState::Interacting, None));
        // The key's own frame, and 59 more: still at its shut angle.
        for _ in 0..60 {
            door.step(DT, &settings);
        }
        assert_eq!(door.angle(), 0.0);
        // Half a second after the key the swing's first frame puts it
        // where the curve begins, and the next one on by a frame of it.
        door.step(DT, &settings);
        assert_eq!(door.angle(), 0.0);
        door.step(DT, &settings);
        let first = 110.0 * settings.curve.evaluate(DT);
        assert!((door.angle() - first).abs() < 1e-3, "{} against {first}", door.angle());
        // Recorded: open 360 frames after the key at 120 a second.
        let frames = 61 + frames_until_done(&mut door, &settings);
        assert_eq!((door.state(), door.angle(), door.action()), (ThingState::Open, 110.0, Some(Action::Close)));
        assert!(frames.abs_diff(360) <= 1, "{frames} frames");
    }

    /// A container waits the install's 0.55 s and swings half as fast
    /// again: in its new state 2.22 s after the key, as recorded.
    #[test]
    fn a_container_waits_and_swings_faster() {
        let settings = settings();
        let mut crate_ = Thing::container(Some(Interaction::ContainerHingeUpOpen), Some(Interaction::ContainerHingeUpClose), 0.0, -30.0);
        assert_eq!(crate_.action(), Some(Action::Search));
        assert_eq!(crate_.work(true, [0.0; 3], &settings), Some(Interaction::ContainerHingeUpOpen));
        crate_.step(DT, &settings);
        // Recorded: open 266 frames after the key at 120 a second.
        let frames = frames_until_done(&mut crate_, &settings);
        assert!(frames.abs_diff(266) <= 1, "{frames} frames");
        assert_eq!((crate_.state(), crate_.angle()), (ThingState::Open, -30.0));
        assert_eq!(crate_.gesture([0.0; 3]), Some(Interaction::ContainerHingeUpClose));
        // Told to shut, as its screen does: it stays open through the wait
        // and the swing, and is shut 266 frames later.
        crate_.tell(false, &settings);
        crate_.step(DT, &settings);
        let shut = (1..2000usize).find(|_| {
            crate_.step(DT, &settings);
            crate_.state() == ThingState::Shut
        });
        assert!(shut.unwrap().abs_diff(266) <= 1, "{shut:?} frames");
        // A drawer comes out and goes back in by the same curve.
        let mut drawer = Thing::container(None, None, 0.0, 0.0);
        drawer.work(true, [0.0; 3], &settings);
        for _ in 0..150 {
            drawer.step(DT, &settings);
        }
        assert!(drawer.out() > 0.5 && drawer.out() < 1.0, "{}", drawer.out());
    }

    /// A container shut by its screen while the character asks it to shut
    /// as well: the second order is refused and changes nothing; the
    /// container is shut when the first is done (recorded: a drawer and a
    /// dead Scav, both shut 2.22 s after their screens).
    #[test]
    fn a_second_order_to_shut_changes_nothing() {
        let settings = settings();
        let mut drawer = Thing::container(Some(Interaction::ContainerOpenPull), Some(Interaction::DoorPushForward), 0.0, 0.0);
        drawer.work(true, [0.0; 3], &settings);
        drawer.step(DT, &settings);
        frames_until_done(&mut drawer, &settings);
        // Its screen shuts: told to shut, with no hand.
        drawer.tell(false, &settings);
        drawer.step(DT, &settings);
        for _ in 0..83 {
            drawer.step(DT, &settings);
        }
        // Still open, and offered to be shut: the key is pressed on it. It
        // is being worked from then on, and the order is refused.
        assert_eq!(drawer.action(), Some(Action::Close));
        assert_eq!(drawer.work(false, [0.0; 3], &settings), Some(Interaction::DoorPushForward));
        assert_eq!(drawer.state(), ThingState::Interacting);
        let shut = (1..2000usize).find(|_| {
            drawer.step(DT, &settings);
            drawer.state() == ThingState::Shut
        });
        assert!((83 + shut.unwrap()).abs_diff(266) <= 1, "{shut:?} frames");
        assert!(!drawer.busy());
    }

    /// The door's gesture by where the character stands: shut, pushed from
    /// the side it swings away from, pulled from the other with the hinge
    /// to one hand or the other; open, pushed shut or pulled back.
    #[test]
    fn a_door_s_gesture_goes_by_the_side() {
        let settings = settings();
        // It opens towards -z: from +z it swings away.
        let mut door = door();
        assert_eq!(door.door_gesture([0.5, 0.0, 1.0]), 27);
        assert_eq!(door.door_gesture([0.5, 0.0, -1.0]), 24);
        // The same door hinged at its other end, pointing along -x.
        let mirrored = Thing::door(Hinge { at: [1.0, 0.0, 0.0], axis: [0.0, -1.0, 0.0], forward: [-1.0, 0.0, 0.0] }, 0.0, 110.0);
        assert_eq!(mirrored.door_gesture([0.5, 0.0, 1.0]), 27);
        assert_eq!(mirrored.door_gesture([0.5, 0.0, -1.0]), 26);
        // Open, it points along -z from its hinge (+x turned 110 degrees
        // about straight up); the shut line it will swing back to is +x.
        door.work(true, [0.5, 0.0, 1.0], &settings);
        door.step(DT, &settings);
        frames_until_done(&mut door, &settings);
        assert_eq!(door.state(), ThingState::Open);
        // From the side it will swing away from (the far side of the open
        // door from the wall's line): pushed shut. From the side it will
        // swing towards: pulled back.
        let open_direction = turn([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 110.0);
        let beyond = [open_direction[0] * 0.5 - 0.8, 0.0, open_direction[2] * 0.5];
        let between = [open_direction[0] * 0.5 + 0.8, 0.0, open_direction[2] * 0.5];
        let (a, b) = (door.door_gesture(beyond), door.door_gesture(between));
        assert!([a, b].contains(&28) && [a, b].iter().any(|g| *g == 23 || *g == 25), "{a} {b}");
        // Being worked, its number is the game's 28 (recorded so).
        door.work(false, [0.0; 3], &settings);
        assert_eq!(door.door_gesture([0.5, 0.0, 1.0]), 28);
    }

    #[test]
    fn the_shortest_turn() {
        assert_eq!(delta_angle(0.0, 110.0), 110.0);
        assert_eq!(delta_angle(110.0, 0.0), -110.0);
        assert_eq!(delta_angle(0.0, -30.0), -30.0);
        assert_eq!(delta_angle(350.0, 10.0), 20.0);
    }

    /// Steps the thing until it has sounded something, and says what and
    /// on which frame (the key's own frame has been stepped before).
    fn frames_until_sound(thing: &mut Thing, settings: &ThingSettings) -> (usize, Vec<ThingSound>) {
        for frame in 1..2000 {
            thing.step(DT, settings);
            let sounds = thing.take_sounds();
            if !sounds.is_empty() {
                return (frame, sounds);
            }
        }
        panic!("nothing sounded")
    }

    /// A door's opening sounds the open clip and the squeak together on
    /// the first frame of its swing, half a second after the key; its
    /// shutting the squeak then, and the shut clip the frame the curve's
    /// time passes its end (no shift): the frame before it stands still.
    #[test]
    fn a_door_sounds_its_opening_and_its_shutting() {
        let settings = settings();
        let mut door = door();
        door.work(true, [0.5, 0.0, 1.0], &settings);
        door.step(DT, &settings);
        assert!(door.take_sounds().is_empty(), "nothing on the key's frame");
        let (frame, sounds) = frames_until_sound(&mut door, &settings);
        assert_eq!(sounds, vec![ThingSound::Swing { opens: true, squeaks: true }]);
        assert_eq!(frame, (DOOR_WAITS / DT).round() as usize, "the first frame at or past the wait");
        let done = frames_until_done(&mut door, &settings);
        assert!(door.take_sounds().is_empty(), "an opening sounds nothing more");
        assert_eq!(door.state(), ThingState::Open);
        // Shut it: the squeak at the swing's start, the shut clip at its end.
        door.work(false, [0.5, 0.0, 1.0], &settings);
        door.step(DT, &settings);
        let (frame, sounds) = frames_until_sound(&mut door, &settings);
        assert_eq!(sounds, vec![ThingSound::Swing { opens: false, squeaks: true }]);
        assert_eq!(frame, (DOOR_WAITS / DT).round() as usize);
        let (frame, sounds) = frames_until_sound(&mut door, &settings);
        assert_eq!(sounds, vec![ThingSound::Shut]);
        let swing_frames = (2.5 / DT).round() as usize;
        assert!((swing_frames - 1..=swing_frames).contains(&frame), "the frame the time passes 2.5 s, counted from the swing's own first: {frame}");
        frames_until_done(&mut door, &settings);
        assert_eq!(door.state(), ThingState::Shut);
        assert!(door.take_sounds().is_empty());
        let _ = done;
    }

    /// A shift moves the shut clip earlier by that many seconds of the
    /// curve: a door's by the seconds themselves, a container's by two
    /// thirds of them (it runs the curve at 1.5).
    #[test]
    fn the_shut_shift() {
        let settings = settings();
        let mut door = door();
        door.shut_shift = 1.0;
        door.at_rest(true);
        door.work(false, [0.5, 0.0, 1.0], &settings);
        door.step(DT, &settings);
        let (swing, _) = frames_until_sound(&mut door, &settings);
        let (frame, sounds) = frames_until_sound(&mut door, &settings);
        assert_eq!(sounds, vec![ThingSound::Shut]);
        let expected = (DOOR_WAITS / DT).round() as usize + (1.5 / DT).round() as usize;
        assert!((expected - 1..=expected).contains(&(swing + frame)), "{}", swing + frame);
        let mut lid = Thing::container(None, None, 0.0, -30.0);
        lid.shut_shift = 1.0;
        lid.at_rest(true);
        lid.tell(false, &settings);
        lid.step(DT, &settings);
        // The wait runs out and the swing's first frame runs at once.
        let (frame, sounds) = frames_until_sound(&mut lid, &settings);
        assert_eq!(sounds, vec![ThingSound::Swing { opens: false, squeaks: true }]);
        assert_eq!(frame, (0.55 / DT).round() as usize);
        let (frame, sounds) = frames_until_sound(&mut lid, &settings);
        assert_eq!(sounds, vec![ThingSound::Shut]);
        // (2.5 - 1) / 1.5 = 1 s of the swing, the frame the time passes it.
        assert!(((1.0 / DT).round() as usize - 1..=(1.0 / DT).round() as usize).contains(&frame), "{frame}");
    }

    /// A container's opening sounds on the frame its wait runs out, which
    /// is its swing's first; its shutting without a shift sounds the shut
    /// clip as the time passes the end, 1.67 s in.
    #[test]
    fn a_container_sounds_as_its_wait_runs_out() {
        let settings = settings();
        let mut lid = Thing::container(None, None, 0.0, -30.0);
        lid.work(true, [0.0; 3], &settings);
        lid.step(DT, &settings);
        let (frame, sounds) = frames_until_sound(&mut lid, &settings);
        assert_eq!(sounds, vec![ThingSound::Swing { opens: true, squeaks: true }]);
        assert_eq!(frame, (0.55 / DT).round() as usize);
        frames_until_done(&mut lid, &settings);
        assert!(lid.take_sounds().is_empty());
        lid.tell(false, &settings);
        lid.step(DT, &settings);
        let (_, sounds) = frames_until_sound(&mut lid, &settings);
        assert_eq!(sounds, vec![ThingSound::Swing { opens: false, squeaks: true }]);
        let (frame, sounds) = frames_until_sound(&mut lid, &settings);
        assert_eq!(sounds, vec![ThingSound::Shut]);
        let expected = (2.5 / 1.5 / DT).round() as usize;
        assert!((expected - 1..=expected).contains(&frame), "{frame}");
        frames_until_done(&mut lid, &settings);
        assert_eq!(lid.state(), ThingState::Shut);
    }

    /// A kick that opens the door sounds its breach clip and the swing's
    /// squeak (no open clip: the swing is a breach, not an opening) on the
    /// frame it lands; one that fails sounds the hit clip.
    #[test]
    fn a_kick_sounds_its_breach_or_its_hit() {
        let settings = settings();
        let mut kicked = door();
        kicked.start_breach([0.5, 0.0, -1.0]);
        kicked.kick_open();
        assert_eq!(kicked.take_sounds(), vec![ThingSound::Breach, ThingSound::Swing { opens: false, squeaks: true }]);
        frames_until_done(&mut kicked, &settings);
        assert!(kicked.take_sounds().is_empty(), "a kick's swing sounds nothing more");
        let mut held = door();
        held.lock(Some("key".to_owned()));
        held.start_breach([0.5, 0.0, 1.0]);
        held.fail_breach();
        assert_eq!(held.take_sounds(), vec![ThingSound::Hit]);
        // A locked door thrown open squeaks not: its fallback is locked.
        let mut locked = door();
        locked.lock(Some("key".to_owned()));
        locked.start_breach([0.5, 0.0, -1.0]);
        locked.kick_open();
        assert_eq!(locked.take_sounds(), vec![ThingSound::Breach, ThingSound::Swing { opens: false, squeaks: false }]);
    }
}
