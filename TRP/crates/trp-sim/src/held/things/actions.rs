//! Thing sounds, kinds, states, actions and the menu a thing offers.


/// How long a door waits after it is told before it begins to turn, in
/// seconds: written in the door's own routine (`Door.SmoothDoorOpenCoroutine`),
/// where a container and a trunk wait the settings' `DelayToOpenContainer`.
pub const DOOR_WAITS: f32 = 0.5;

/// How much faster than the curve's own pace a container or a trunk turns
/// (`SetDoorState` asks for 1.5; a door's routine drops it and runs at 1).
pub(super) const CONTAINER_PACE: f32 = 1.5;

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
    /// A thing lying loose whose kind the character does not know: reached
    /// for as for a take and examined where it lies.
    Examine,
    /// The gun bench on the range (TRP's own): the edit build
    /// screen opened for the weapon in hand. The game's workbench offers no
    /// line of its own; its item menu's is EDIT BUILD (`Editbuild`).
    ModifyWeapon,
    /// The armor bench on the range (TRP's own, working like the edit build
    /// screen for the helmet and armour worn): the
    /// edit build screen opened for the helmet worn, or for the armour worn.
    /// The game's own for either is its item menu's MODDING, out of a raid.
    ModifyHelmet,
    ModifyArmor,
    /// Either bench on the range (TRP's own line, offered only with a
    /// repair kit, much as a key): the inventory opened for repairing, a
    /// repair kit let go on what it mends opening the game's repair window.
    /// The game repairs out of a raid only, by its item menu's line.
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
            Action::Examine => "Examine",
            // (No text of the game's for these: TRP's own words.)
            Action::ModifyWeapon => "Modify weapon",
            Action::ModifyHelmet => "Modify helmet",
            Action::ModifyArmor => "Modify armor",
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
