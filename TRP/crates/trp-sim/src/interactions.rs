//! The gestures of the hands on the world: opening and shutting a
//! container, pushing and pulling a door, taking a thing up
//! (`docs/spec/interactions.md`).
//!
//! In the game these are states of one layer of the body's animator, each
//! switched on by two parameters, and a small model in front of them that
//! plays one at a time and queues the rest (`AnimatedInteractions`). This
//! is that model. What it has the animator do comes out as `Show`, which
//! the movement puts on the body's animator; what the animator's states
//! report back goes in through `finished`.

use std::collections::VecDeque;

/// An interaction the body's animator has a state for, by the game's own
/// names (`EInteraction`). The player's hand signs, the things done to
/// headgear and the bipod's are left out: nothing here starts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    TakeItem,
    DropBackpack,
    PushHingeLeft,
    PullHingeLeft,
    PushHingeRight,
    PullHingeRight,
    DoorPushForward,
    DoorPullBackward,
    DoorKeyOpen,
    DoorCardOpen,
    ContainerOpenUp,
    ContainerCloseDown,
    ContainerOpenPull,
    ContainerClosePush,
    ContainerOpenDefault,
    ContainerCloseDefault,
    ContainerHingeUpOpen,
    ContainerHingeUpClose,
}

/// How the model is to treat one interaction when it is asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Instruction {
    /// It goes with something being done to the inventory, and is neither
    /// thrown out of the queue nor cut short by a forced stop.
    operation: bool,
    /// The animator's layer is put back to rest before it plays.
    forced: bool,
    /// What is waiting to play is thrown out, but for operations.
    reset_queue: bool,
    /// Nothing further is taken until the queue has run empty.
    deny_schedule: bool,
}

impl Interaction {
    /// The number the body's animator knows it by (`InteractionIndex`).
    pub fn index(self) -> u8 {
        use Interaction::*;
        match self {
            TakeItem => 8,
            DropBackpack => 10,
            PushHingeLeft => 16,
            PullHingeLeft => 17,
            PushHingeRight => 18,
            PullHingeRight => 19,
            DoorPushForward => 20,
            DoorPullBackward => 21,
            DoorKeyOpen => 22,
            ContainerOpenUp => 23,
            ContainerCloseDown => 24,
            ContainerOpenPull => 25,
            ContainerClosePush => 26,
            ContainerOpenDefault => 27,
            ContainerCloseDefault => 28,
            ContainerHingeUpOpen => 29,
            ContainerHingeUpClose => 30,
            DoorCardOpen => 35,
        }
    }

    /// The game's own number for it (`EInteraction`), which a thing of the
    /// world names its gestures by and the recorder writes as
    /// `thing_gesture`; the model turns it into the animator's.
    pub fn game_number(self) -> u8 {
        use Interaction::*;
        match self {
            TakeItem => 8,
            DropBackpack => 22,
            DoorCardOpen => 38,
            // The doors' and the containers' run in the same order seven
            // above the animator's (the game's enum against the animator's
            // states, recorded for nine of them).
            other => other.index() + 7,
        }
    }

    /// The interaction of one of the game's own numbers, if it is one of these.
    pub fn of_game_number(number: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|interaction| interaction.game_number() == number)
    }

    const ALL: [Interaction; 18] = [
        Interaction::TakeItem,
        Interaction::DropBackpack,
        Interaction::PushHingeLeft,
        Interaction::PullHingeLeft,
        Interaction::PushHingeRight,
        Interaction::PullHingeRight,
        Interaction::DoorPushForward,
        Interaction::DoorPullBackward,
        Interaction::DoorKeyOpen,
        Interaction::DoorCardOpen,
        Interaction::ContainerOpenUp,
        Interaction::ContainerCloseDown,
        Interaction::ContainerOpenPull,
        Interaction::ContainerClosePush,
        Interaction::ContainerOpenDefault,
        Interaction::ContainerCloseDefault,
        Interaction::ContainerHingeUpOpen,
        Interaction::ContainerHingeUpClose,
    ];

    /// The interaction of an animator index, if it is one of these.
    pub fn of_index(index: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|interaction| interaction.index() == index)
    }

    fn is_door(self) -> bool {
        use Interaction::*;
        matches!(
            self,
            PushHingeLeft | PullHingeLeft | PushHingeRight | PullHingeRight | DoorPushForward | DoorPullBackward | DoorKeyOpen | DoorCardOpen
        )
    }

    /// Whether it goes with something done to the inventory (the state's
    /// own word for it, which the queue and a forced stop go by).
    fn is_operation(self) -> bool {
        matches!(self, Interaction::TakeItem | Interaction::DropBackpack)
    }

    /// The instruction it is asked for with. A door's comes from the
    /// doors' own model, the same for every door; the others are each
    /// state's own.
    fn instruction(self) -> Instruction {
        match self {
            Interaction::TakeItem => Instruction { operation: true, reset_queue: true, deny_schedule: true, ..Instruction::default() },
            Interaction::DropBackpack => Instruction { operation: true, reset_queue: true, forced: true, ..Instruction::default() },
            door if door.is_door() => Instruction { reset_queue: true, deny_schedule: true, ..Instruction::default() },
            _ => Instruction { reset_queue: true, forced: true, ..Instruction::default() },
        }
    }
}

/// What the body's animator is to be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    /// Its flag for an interaction up, and which one.
    Play(u8),
    /// The flag down: the state playing runs out by its own way.
    Stop,
    /// The flag down and the layer put back to rest outright.
    Reset,
}

/// The game's model of the animated interactions: one plays, the rest wait.
#[derive(Debug, Clone)]
pub struct AnimatedInteractions {
    queue: VecDeque<Interaction>,
    can_interact: bool,
    schedule_denied: bool,
    current: Option<Interaction>,
}

impl Default for AnimatedInteractions {
    fn default() -> Self {
        Self { queue: VecDeque::new(), can_interact: true, schedule_denied: false, current: None }
    }
}

impl AnimatedInteractions {
    /// Whether nothing is playing, so that one asked for plays at once.
    pub fn can_interact(&self) -> bool {
        self.can_interact
    }

    /// Whether one is playing or waiting to.
    pub fn is_playing(&self) -> bool {
        !self.queue.is_empty() || self.current.is_some()
    }

    /// The one playing.
    pub fn current(&self) -> Option<Interaction> {
        self.current
    }

    /// Whether that one is playing or waiting to.
    pub fn has(&self, interaction: Interaction) -> bool {
        self.current == Some(interaction) || self.queue.contains(&interaction)
    }

    /// Asks for an interaction (`SetInteraction`).
    pub fn set(&mut self, interaction: Interaction) -> Vec<Show> {
        let mut shows = Vec::new();
        if self.schedule_denied {
            return shows;
        }
        let instruction = interaction.instruction();
        if instruction.reset_queue {
            self.reset_queue();
        }
        if instruction.deny_schedule {
            self.schedule_denied = true;
        }
        if instruction.forced {
            shows.push(Show::Reset);
        }
        self.queue.push_back(interaction);
        if self.can_interact {
            self.proceed(&mut shows);
        }
        shows
    }

    /// A state of the animator's layer has been left: the interaction it
    /// is for is over, and the next one waiting plays.
    pub fn finished(&mut self) -> Vec<Show> {
        let mut shows = vec![Show::Stop];
        self.proceed(&mut shows);
        shows
    }

    /// Everything is stopped (`ForceStopInteractions`), but an operation.
    pub fn force_stop(&mut self) -> Vec<Show> {
        (self.schedule_denied, self.can_interact) = (false, true);
        self.reset_queue();
        match self.current {
            Some(current) if !current.is_operation() => {
                self.current = None;
                vec![Show::Reset]
            }
            _ => Vec::new(),
        }
    }

    /// Throws out what is waiting, but for operations.
    fn reset_queue(&mut self) {
        self.queue.retain(|waiting| waiting.is_operation());
    }

    /// Plays the next one waiting, or has the model at rest.
    fn proceed(&mut self, shows: &mut Vec<Show>) {
        match self.queue.pop_front() {
            None => {
                (self.can_interact, self.schedule_denied) = (true, false);
                self.current = None;
            }
            Some(next) => {
                self.can_interact = false;
                self.current = Some(next);
                shows.push(Show::Play(next.index()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One plays at once; a second asked for meanwhile waits for the
    /// first to be over, unless the first denies the schedule.
    #[test]
    fn one_plays_and_the_rest_wait() {
        let mut model = AnimatedInteractions::default();
        // A container's opening puts the layer to rest and plays.
        assert_eq!(model.set(Interaction::ContainerOpenDefault), [Show::Reset, Show::Play(27)]);
        assert!(model.is_playing() && !model.can_interact());
        // Its shutting, asked for meanwhile, waits; the layer is put to
        // rest all the same, as the game has it.
        assert_eq!(model.set(Interaction::ContainerCloseDefault), [Show::Reset]);
        assert_eq!(model.finished(), [Show::Stop, Show::Play(28)]);
        assert_eq!(model.finished(), [Show::Stop]);
        assert!(!model.is_playing() && model.can_interact());
    }

    /// A door's gesture, and the taking of a thing, let nothing else in
    /// until they are over.
    #[test]
    fn a_door_denies_the_schedule() {
        let mut model = AnimatedInteractions::default();
        assert_eq!(model.set(Interaction::PushHingeLeft), [Show::Play(16)]);
        assert!(model.set(Interaction::ContainerOpenDefault).is_empty());
        assert!(model.set(Interaction::TakeItem).is_empty());
        assert_eq!(model.finished(), [Show::Stop]);
        assert_eq!(model.set(Interaction::TakeItem), [Show::Play(8)]);
    }

    /// A forced stop puts the layer to rest, but lets an operation play out.
    #[test]
    fn a_forced_stop_spares_an_operation() {
        let mut model = AnimatedInteractions::default();
        model.set(Interaction::ContainerOpenDefault);
        assert_eq!(model.force_stop(), [Show::Reset]);
        assert!(!model.is_playing());
        model.set(Interaction::TakeItem);
        assert!(model.force_stop().is_empty());
        assert_eq!(model.current(), Some(Interaction::TakeItem));
    }

    #[test]
    fn every_index_is_its_interaction_s() {
        for index in 0..=41 {
            if let Some(interaction) = Interaction::of_index(index) {
                assert_eq!(interaction.index(), index);
            }
        }
        assert_eq!(Interaction::of_index(27), Some(Interaction::ContainerOpenDefault));
        assert_eq!(Interaction::of_index(9), None);
        // The recorded pairs: what a thing named, and what the animator played.
        for (game, animator) in [(24, 17), (28, 21), (27, 20), (23, 16), (36, 29), (37, 30), (32, 25), (34, 27), (35, 28), (8, 8)] {
            assert_eq!(Interaction::of_game_number(game).map(Interaction::index), Some(animator));
        }
    }
}
