//! What a step reports: interactions, moves, actions, kicks and approaches.

use super::*;

/// A thing of the world that is worked with something in the hand, a key
/// or a card, or that is kicked in: the game then holds the character at
/// it (`Movement::unlock`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldInteraction {
    /// The gesture the thing asks for.
    pub interaction: Interaction,
    /// Whether the thing has the character put at a place of its own to
    /// work it, or has a grip for the hand: the state then lasts its full
    /// three seconds.
    pub snap: bool,
    /// How high the thing is reached for, in the world: a low one is
    /// crouched to.
    pub height: f32,
}

/// What the movement did in a frame that the character's health takes
/// (`Movement::take_moved`; health spec, "Falls", "What health does to the
/// character").
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Moved {
    /// On the ground again after falling so far (`HandleFall`).
    Fell(f32),
    /// A jump begun: every wound pushed by 1.
    JumpStarted,
    /// A jump over: each damaged leg hurt by 3.
    JumpEnded,
    /// A sprint begun: on a damaged leg it starts the self-damage.
    SprintStarted,
    /// A sprint over on the ground, so far: every wound pushed by a tenth.
    SprintEnded(f32),
    /// A vault begun over something so high: a damaged left leg hurt by 3.
    Vaulted(f32),
}

/// What came of something the character was doing to a thing of the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acted {
    /// The wait after a container was searched is over and the character
    /// is still at it: it is now shown.
    Searched,
    /// That wait is over and the character has gone from the container,
    /// or is in a jump: it is not shown, and shuts again.
    LeftUnsearched,
    /// A thing unlocked or kicked: the hand has been on it long enough,
    /// and the thing now does what was asked of it.
    Worked,
    /// The state for such a thing is over.
    Done,
    /// The thing reached for is taken: it goes into the inventory now.
    PickedUp,
    /// A kick has landed on a door, 0.45 of the way through its state: it
    /// throws the door open, or the door holds.
    Kicked { opens: bool },
    /// The backpack being dropped has left the shoulders: its gesture has
    /// come to the mark its state has for that, and the thing itself now
    /// leaves the inventory (`OnBackpackDrop`).
    BackpackDropped,
    /// The bipod's gesture has come to its toggle's mark, or is gone
    /// without it (`OnBipodToggle`, `FastForward`): the weapon's toggle is
    /// over (`Firearm::bipod_toggled`).
    BipodToggled,
}

/// A door being kicked (`MovementContext.StartDoorBreak`): whether the
/// kick will throw it open, as worked out at the key; whether it has
/// landed; and whether the character counts as standing where it is to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Kick {
    pub(super) opens: bool,
    pub(super) landed: bool,
    pub(super) approached: bool,
    /// Where the door puts the character first and what it has it look
    /// at, if it does; and the approach that takes it there, once the
    /// approach has begun.
    pub(super) snap: Option<([f32; 3], [f32; 3])>,
    pub(super) approach: Option<Approaching>,
}

/// The approach to a thing that puts the character at a place of its own
/// (`ApproachState`): how long it has run and how long it takes, in
/// seconds of the engine's clock, and where the character stood, looked
/// and faced when it began against where it is to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Approaching {
    pub(super) time: f32,
    pub(super) length: f32,
    pub(super) from: [f32; 3],
    pub(super) to: [f32; 3],
    pub(super) look_from: [f32; 2],
    pub(super) look_to: [f32; 2],
    pub(super) body_from: f32,
    pub(super) direction: [f32; 2],
    pub(super) fresh: bool,
}

/// How fast the approach walks the character to its place, in metres a
/// second, unburdened and at the most overweight; how fast it turns the
/// view, in degrees a second; and the longest it takes, in seconds
/// (`ApproachState.ApproachSpeed`, `ApproachAngularSpeed`).
pub(super) const APPROACH_SPEED: [f32; 2] = [2.5, 0.01];

pub(super) const APPROACH_TURN: f32 = 90.0;

pub(super) const APPROACH_LONGEST: f32 = 1.5;

/// How far through the kick's state the body begins to move by its
/// animation's own motion, and how far through it the kick lands
/// (`BreachDoorState`).
pub(super) const KICK_MOVES_FROM: f32 = 0.15;

pub(super) const KICK_LANDS_AT: f32 = 0.45;

/// The game's two movement states for the hands on the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Acting {
    /// Unlocking or kicking a door or a container (`DoorInteractState`):
    /// the seconds in it, whether the thing has been told to do its part,
    /// and whether the state runs its full length.
    Working { time: f32, worked: bool, snap: bool },
    /// Taking a thing up (`PickUpState`): the seconds in it, the stance it
    /// began with, and whether the thing has been taken (the state's
    /// action done); for an examine from the world, that the state waits on
    /// the examine's end after its action, and whether that has come.
    PickingUp { time: f32, level: f32, taken: bool, waits: bool, told: bool },
}

/// How long the hand is laid on a thing before the thing does its part,
/// how long the state for it lasts where it runs its full length, and how
/// long a thing is reached for before it is taken, in seconds. The first
/// two are written in the game's state for a door (it leaves the first
/// sooner when the hands' animator says the left hand has arrived, which
/// a firearm's never does); the third in its state for a pick-up.
pub(super) const HAND_LAID_AFTER: f32 = 1.6;

pub(super) const WORKING_ENDS_AFTER: f32 = 3.0;

pub(super) const PICKED_UP_AFTER: f32 = 0.6;

/// The longest a pick-up's state lasts, its action done or not, in
/// seconds (`PickUpState.ManualAnimatorMoveUpdate`).
pub(super) const PICK_UP_LASTS: f32 = 5.0;

/// How far down the view has to point for a pick-up to crouch the
/// character to a third of its stance, in degrees.
pub(super) const PICK_UP_CROUCH_PITCH: f32 = 60.0;

/// How much further from a searched container than at the key the
/// character may be when the wait is over and still be shown it, in
/// metres (`InteractionContextHelper.OnContainerOpen`).
pub(super) const SEARCH_REACH: f32 = 0.25;

/// How far the carried weight must have moved since it was last worked out
/// for a change the inventory tells of to work it out again, in kilograms
/// (`PhysicalBase.LateUpdate`: more than this from `PreviousWeight`).
pub const WEIGHT_STEP: f32 = 0.01;
