//! The movement states and the input frame.

use super::*;

/// The movement states. Which one is current follows the body's animator:
/// each of its states names one of these, and entering that animator state
/// enters this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Move,
    Sprint,
    Jump,
    /// Coming to a stop out of a sprint, through the animator's own clip.
    SprintStop,
    /// Landing from a jump made while moving.
    Landing,
    /// Going down to lie prone.
    ToProne,
    /// Lying still.
    ProneIdle,
    /// Crawling.
    ProneMove,
    /// Getting up from prone.
    FromProne,
    /// Stepped out to one side from standing, on the way there or back.
    Sidestep,
    /// Rolled onto one side lying down, on the way there or back.
    Roll,
    /// Over a low obstacle and down the other side (the game's
    /// `ClimbOver`; vaulting spec).
    Vault,
    /// Up onto an obstacle (the game's `ClimbUp`).
    Climb,
    /// Coming down behind a vault where the ground is far below.
    VaultFall,
    /// Landing out of a vault.
    VaultLanding,
    /// Set before a door to kick it (the game's `Approach`; interactions
    /// spec, "A kick").
    Approach,
    /// Kicking a door (the game's `BreachDoor`).
    Breach,
    /// Blind fire, over the top or to the right, on the way in, held, or
    /// on the way out (`docs/spec/blind-fire.md`): a standing state for
    /// all its frames' work.
    BlindFire,
}

impl State {
    const ALL: [State; 19] = [
        State::BlindFire,
        State::Approach,
        State::Breach,
        State::Sidestep,
        State::Roll,
        State::Vault,
        State::Climb,
        State::VaultFall,
        State::VaultLanding,
        State::Idle,
        State::Move,
        State::Sprint,
        State::Jump,
        State::SprintStop,
        State::Landing,
        State::ToProne,
        State::ProneIdle,
        State::ProneMove,
        State::FromProne,
    ];

    /// The game's number for the state, as the body's animator names it.
    pub(super) fn number(self) -> i32 {
        match self {
            State::Idle => movement_state::IDLE,
            State::Move => movement_state::RUN,
            State::Sprint => movement_state::SPRINT,
            State::Jump => movement_state::JUMP,
            State::SprintStop => movement_state::TRANSITION,
            State::Landing => movement_state::JUMP_LANDING,
            State::ToProne => movement_state::TO_PRONE,
            State::ProneIdle => movement_state::PRONE_IDLE,
            State::ProneMove => movement_state::PRONE_MOVE,
            State::FromProne => movement_state::PRONE_TO_STAND,
            State::Sidestep => movement_state::SIDESTEP,
            State::Roll => movement_state::ROLL,
            State::Vault => movement_state::CLIMB_OVER,
            State::Climb => movement_state::CLIMB_UP,
            State::VaultFall => movement_state::VAULTING_FALL_DOWN,
            State::VaultLanding => movement_state::VAULTING_LANDING,
            State::Approach => movement_state::APPROACH,
            State::Breach => movement_state::BREACH_DOOR,
            State::BlindFire => movement_state::BLIND_FIRE,
        }
    }

    pub(super) fn from_number(number: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.number() == number)
    }

    /// The states the game builds on its standing state: they take
    /// movement keys and sprint commands as standing does. (Blind fire's
    /// is one: the keys never reach it while its figure is set.)
    pub(super) fn stands(self) -> bool {
        matches!(
            self,
            State::Idle | State::ToProne | State::ProneIdle | State::FromProne | State::Sidestep | State::Roll | State::BlindFire
        )
    }

    /// The states that pass blind fire's side on to the gate
    /// (`IdlePlayerState.BlindFire`): standing, blind fire's own, the step
    /// aside, going down and getting up. Lying still and rolled over have
    /// their own, which passes 0 (`PronePlayerState`).
    pub(super) fn takes_blind_fire(self) -> bool {
        matches!(self, State::Idle | State::BlindFire | State::Sidestep | State::ToProne | State::FromProne)
    }
}

/// Blind fire's side by the side its animator state names (`Entered::
/// side`, 1 for the game's `Right`): -1 to the right, 1 over the top
/// (`BlindeFireState`: anything but `Right` is 1).
pub(super) fn blind_side(named: f32) -> f32 {
    if named > 0.0 { -1.0 } else { 1.0 }
}

/// The two lean keys as an input has them, the left and the right.
pub(super) fn lean_keys(input: &Input) -> [bool; 2] {
    [input.lean_keys[0] || input.lean < 0.0, input.lean_keys[1] || input.lean > 0.0]
}

/// What the player asks for during one step.
#[derive(Debug, Clone, Copy, Default)]
pub struct Input {
    /// x right, y forward, each -1..1.
    pub move_dir: [f32; 2],
    /// Yaw and pitch change in degrees, sensitivity already applied.
    pub look_delta: [f32; 2],
    /// Sprint key held.
    pub sprint: bool,
    /// Jump pressed this step.
    pub jump: bool,
    /// Lie down, or get up, pressed this step.
    pub prone: bool,
    /// The crouch key pressed this step: between crouched and standing,
    /// or back to the stance last chosen if something else changed it.
    pub crouch: bool,
    /// Stance change this step; one wheel notch is `delta_level`.
    pub pose_delta: f32,
    /// Speed change this step; one wheel notch is `delta_speed`.
    pub speed_delta: f32,
    /// A lean key held: -1 the left one, 0 neither, 1 the right one. The
    /// keys are heard as presses and releases (movement spec, "Lean"), so
    /// a change of this is one key let go and the other pressed.
    pub lean: f32,
    /// Both lean keys as they stand, the left and the right, for a host
    /// that can have the two down at once; `lean` counts as its side's
    /// key being down as well.
    pub lean_keys: [bool; 2],
    /// The step-aside key held: -1 left, 0 neither, 1 right.
    pub step: i32,
    /// Aiming down the sights, which caps the walking speed.
    pub aiming: bool,
    /// The weapon in hand keeps its sights from the body's animator (a
    /// part with its own animations on it: `Firearm::tells_body_of_aim`),
    /// whose `isAiming` then stays down.
    pub aim_kept_from_body: bool,
    /// The weapon's reload is in hand (`Firearm::reloading`). For that
    /// long a lying body is held still.
    pub reloading: bool,
    /// The hands are busy with the weapon (`Firearm::hands_busy`). In a
    /// sprint the body brings the weapon up into view for that long.
    pub hands_busy: bool,
    /// The animator of the thing in hand is in its resting state
    /// (`Firearm::animator_idling`): only then is a gesture of the hands
    /// on the world begun (interactions spec, "What starts one").
    pub hands_idle: bool,
    /// That animator is in its states for an open inventory
    /// (`Firearm::in_inventory`), which is when the body takes its own
    /// pose for one.
    pub hands_in_inventory: bool,
    /// The hold-breath key pressed this step: it holds the breath, or lets
    /// a held one go (this install binds it as a press; the game also has
    /// a command for the key coming up, for a binding that holds). Only a
    /// character standing still (or lying still, or stepped aside) with
    /// the sights up holds one (weapon-handling spec, "Breathing").
    pub hold_breath: bool,
    /// The vault key held (in the game's default binding the jump key,
    /// which then jumps when let go: vaulting spec, "Two moves, one
    /// key"). Every frame it is held tries a vault or a climb.
    pub vault: bool,
    /// The game's own commands for the sprint, the lean, the step aside,
    /// the breath and the crouch, for a host that hears its keys as the
    /// game's input manager gives them out (settings spec, "What the
    /// bindings do"; `trp_sim::bindings`): each is carried out once on
    /// this frame. The keys above are held keys, whose going down and
    /// coming up stand for the same commands; a binding that is pressed
    /// and not held gives the first of each pair here and never the
    /// second, which a held key cannot say. The lean's and the step's go
    /// left then right.
    pub sprint_toggle: bool,
    pub sprint_end: bool,
    pub lean_toggle: [bool; 2],
    pub lean_end: [bool; 2],
    pub step_toggle: [bool; 2],
    pub step_end: [bool; 2],
    /// The hold-breath key coming up, where it is bound to be held: a
    /// held breath is let go.
    pub breath_end: bool,
    /// The crouch key coming up, where it is bound to be held.
    pub restore_pose: bool,
    /// The walk key's two commands (`Player.ToggleMinSpeed`,
    /// `RestoreSpeed`): the speed put to its least, or back to what it
    /// was; and back, where the key is bound to be held and comes up.
    pub walk_toggle: bool,
    pub walk_end: bool,
    /// The slow lean's axis as its keys push it, -1 the left to 1 the
    /// right (`LeanX`; movement spec, "The slow lean"): a tenth of a
    /// degree of lean a frame for as long as it is pushed.
    pub slow_lean: f32,
    /// The weapon's logic has something in hand: a reload, a shot, a look
    /// at the weapon (`Firearm::occupied`). Only an obstacle low enough to
    /// leave the hands on the weapon is then vaulted or climbed.
    pub weapon_occupied: bool,
    /// Medicine or food in hand, or a grenade's throw held or under way
    /// (`CheckMedsAndFoodOperation`, `CheckGrenadeOperation`): only an
    /// obstacle low enough to leave the hands free is then vaulted or
    /// climbed, and nothing with no move measured yet (vaulting spec,
    /// "What a try does").
    pub hands_limit_vault: bool,
    /// Take `look_delta` as it is, without the movement state's own
    /// slowing and limit on the view's turning: for a host that follows a
    /// view recorded from the game, which has been through them already.
    pub free_look: bool,
    /// A screen of the host's has the keys and the mouse, as the game's
    /// inventory screen and its menus have (inventory spec, "What an open
    /// screen does to the keys"): the character is handed no movement, no
    /// turn of the view and none of its keys, and a key pressed or let go
    /// meanwhile is never heard of. What the weapon tells (`aiming` and
    /// its neighbours) and `inventory` are taken all the same.
    pub screen: bool,
    /// The character's inventory is open (`Player.SetInventoryOpened`).
    /// The body's animator is told as it is of a reload, and a change
    /// either way switches a held vault key off.
    pub inventory: bool,
    /// The hands in hand do not pass the screen's opening and shutting on
    /// to the movement state (`OnInventory`), so the body's flag for a
    /// reload stays as it was: an item's hands and a grenade's. The empty
    /// hands, a firearm, a knife and a usable item pass it on.
    pub inventory_unheard: bool,
    /// The head's free look (`docs/spec/free-look.md`): what the mouse's
    /// travel gave the look's two axes on this frame, in degrees (across,
    /// to the right positive, and down), and whether the look's key came
    /// up (`ResetLookDirection`). The mouse gives the look what it does
    /// not give the turn (`bindings::Preset::mouse`).
    pub head_look: [f32; 2],
    pub head_reset: bool,
    /// The camera's pitch below level as last drawn, in degrees: a
    /// sprinting character whose camera is far down looks no further
    /// down. A host with no camera leaves it at nothing.
    pub camera_pitch: f32,
    /// Blind fire's commands (`docs/spec/blind-fire.md`, "The keys"):
    /// `ToggleBlindAbove` (1) or `ToggleBlindRight` (-1) given this frame,
    /// 0 neither; and `BlindShootEnd`, its keys come up. Each is carried
    /// out once, before the movement keys.
    pub blind_toggle: i32,
    pub blind_end: bool,
    /// What the thing in hand says of blind fire (`MovementContext.
    /// SetBlindFire`): whether it is a firearm (anything else puts blind
    /// fire to 0), and whether that firearm may start it now
    /// (`Firearm::can_set_blind_fire`).
    pub blind_firearm: bool,
    pub blind_may_start: bool,
    /// The mount's command (`ECommand.WeaponMounting`, `V`) given this
    /// frame, and whether the thing in hand may mount (`Player.TryMountWeapon`'s
    /// part of the weapon's: a firearm whose template is mountable, no
    /// stationary weapon, no reload, draw, gesture or put-away under way;
    /// `Firearm::may_mount`). Mounting spec, "Who may ask".
    pub mount: bool,
    pub mount_may: bool,
    /// How deep the weapon sticks into something just now, in metres
    /// (`MovementContext.OverlapDepth`): a forward mount's lower pitch limit
    /// goes by it.
    pub overlap_depth: f32,
    /// The left shoulder's command (`ECommand.LeftStanceToggle`, `Mouse4`)
    /// given this frame, and whether the firearm in hand's operation takes
    /// it now (`FirearmController.ChangeLeftStance`, the operation's
    /// `ToggleLeftStance`: at rest or in a look; `Firearm::may_toggle_left_stance`).
    /// Left-shoulder spec, "The key".
    pub left_stance: bool,
    pub left_stance_may: bool,
}
