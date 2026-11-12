//! The weapon's own logic: trigger, fire modes, rounds, reloading, being
//! drawn and put away. See `docs/spec/firing.md`.
//!
//! The game runs this as a set of operations, one in hand at a time, that
//! set parameters on the weapon's animator and wait for marks placed in
//! its states. So does this: the weapon's own controller runs on the
//! engine's state machine (`weapon_animator`), the operation in hand tells
//! it what the game's tells its own, and what the logic waits for are the
//! marks the animator delivers at the end of each frame. Which state
//! follows which, how long each lasts and how two are blended are the
//! animator's business alone; a host draws the animator's pose.

use crate::base::marks::Mark;
use crate::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

mod chambers;
mod launcher;
mod malfunctions;
pub use chambers::{Chamber, Shown};
pub use launcher::{LauncherBones, LauncherData, LauncherEvent, RANGES as LAUNCHER_RANGES};
pub use malfunctions::{Wear, WearSwitches};
use crate::weapons::malfunction::Malf;

mod settings;
mod operation;
mod making;
mod starting;
mod drawing;
mod trigger;
mod inventory;
mod chambering;
mod aiming;
mod reload;
mod shells;
mod bipod;
mod checks;
mod firing;
mod stepping;
mod animation_marks;

pub use settings::*;
pub use operation::*;
use operation::{Blend, MASTERING_TO_AIM_IN_A_RELOAD, Operation, Planned, TURN_AWAY_RUN_DOWN};

/// How far into the fire state the game starts it.
const FIRE_STATE_START: f32 = 0.2;
/// How far into the resting state the game puts the animator when a run
/// of automatic fire hands back to idling.
const REST_AFTER_AUTO: f32 = 0.1;
/// Where in one cycle of automatic fire each thing happens, as a fraction.
const AUTO_SHELL_EJECT: f32 = 0.25;
const AUTO_CHAMBER: f32 = 0.75;
const AUTO_CYCLE_END: f32 = 0.99;
/// How long a look at the weapon waits for its own first mark before it
/// gives up, in seconds.
const MAX_UTILITY_TIME: f32 = 2.5;
/// The hands' two states for an open inventory, which the game asks the
/// animator about by name.
const IN_INVENTORY: [&str; 2] = ["Hands.OPEN INVENTORY", "Hands.IN INVENTORY"];

#[derive(Clone)]
pub struct Firearm {
    pub settings: FirearmSettings,
    /// For a weapon whose rounds lie in chambers of their own (a
    /// cylinder's, or barrels): what each holds, which of a cylinder's is
    /// under the hammer, and whether the hammer is down
    /// (`Weapon.CylinderHammerClosed`). `magazine` then counts the rounds
    /// in all of them and `chamber` is never set.
    pub in_chambers: Vec<Chamber>,
    pub camora: usize,
    pub hammer_closed: bool,
    /// What the weapon's model holds at each of those chambers, at the
    /// game's moments (`Shown`), and the chambers the shot in hand fired,
    /// whose rounds go to their ports at its mark.
    shown: Vec<Shown>,
    fired_chambers: Vec<usize>,
    /// A reload of such a weapon: the chambers to load in their order,
    /// those with cases still to take out, how many rounds have shown in
    /// the hand, how many are in reach, how many rounds there were at its
    /// start, and the chamber whose case comes out next.
    reload_free: Vec<usize>,
    reload_shells: Vec<usize>,
    reload_loading: usize,
    reload_reach: u32,
    reload_cached: u32,
    reload_remove: i32,
    /// A single barrel's reload takes a live round out of it.
    reload_unloads: bool,
    /// A round the inventory is moving from one of the weapon's chambers of
    /// its own into another: the load that follows its taking out is the
    /// same move, whose chamber action the taking out raised (`Idling.
    /// ExecuteCylinderMagOperations`, `Idling.Execute`: one operation, its
    /// destination given to the discharge).
    moving_round: bool,
    /// A cylinder's reload is the quick one, which throws its live rounds
    /// out with its cases; and the fistful of a character who knows the
    /// weapon (`_quickReload`, `_highLevelMasteringFullReload`).
    reload_quick: bool,
    reload_fistful: bool,
    /// The sights as the animator was last told of them, and whether it
    /// has been told since the resting state last began (the game's
    /// `bool_0`, by which a rocket launcher's trigger is refused just
    /// after the sights have come down).
    aim_told: bool,
    aim_flags: bool,
    /// A round is chambered.
    pub chamber: bool,
    /// A weapon with no chamber's own `Armed` (`Weapon.Armed`): kept by the
    /// clips' `Arm` and `Disarm` marks and given to the animator as it is
    /// drawn (`SetAnimatorValues`), whose draw arms it where it is not.
    armed: bool,
    /// The fitted magazine's `magAnimationIndex`, and the one the next
    /// reload or magazine put in brings: the animator's `RelTypeOld` and
    /// `RelTypeNew`, by which the reload's and the look's clips are chosen
    /// (firing spec, "Which magazine"). The build's at first.
    mag_variant: i32,
    incoming_variant: i32,
    /// The same two magazines', whether each comes out
    /// (`FirearmSettings::magazine_external`). The build's at first.
    mag_external: bool,
    incoming_external: bool,
    /// The same two magazines' rounds from the foot up to and with the
    /// first, from the top, that the weapon's chamber does not take: the
    /// host's, from its inventory (0 where every round fits, and for a
    /// weapon with no chamber, or chambers of its own). While a magazine
    /// holds no more than that, its top round is one the chamber refuses
    /// and none is fed from it (`Magazine.IsAmmoCompatible`; firing spec,
    /// "Rounds the chamber does not take").
    unfit: u32,
    incoming_unfit: u32,
    /// The chambered round has gone to the shell port with a shot and the
    /// next is not yet in sight in the chamber (the fire clip's
    /// `AddAmmoInChamber`, an automatic cycle's `AUTO_CHAMBER`): what a host
    /// draws there (`round_drawn_in_chamber`).
    round_out: bool,
    /// A round loaded by hand is in the hand, on the chamber's point, from
    /// the clip's `MagShow` until the chamber takes it
    /// (`LoadCartridgeToChamber.OnMagAppeared`).
    round_in_hand: bool,
    /// Rounds in the fitted magazine, if one is fitted.
    pub magazine: Option<u32>,
    /// Rounds in the magazine a reload under way is fitting, and whether
    /// that reload chambers a round from it.
    incoming: u32,
    chambers: bool,
    /// The stock is folded. The weapon keeps it while it is put away.
    folded: bool,
    mode: usize,
    /// The trigger as the logic sees it, which is not always the finger.
    trigger: bool,
    held: bool,
    /// Whether the movement state lets the hands take a press of the
    /// trigger, as last told.
    hands_free: bool,
    /// The sights are up, and the clock and blend of a reload made with
    /// them up.
    aiming: bool,
    /// How deep the weapon stuck into an obstacle when last measured, and
    /// whether it was an obstacle that put the sights down.
    overlap: f32,
    interrupted: bool,
    clock: f32,
    tactical: Blend,
    /// The character's inventory is open, as the operations that take
    /// note of it for the controller last heard (a shot notes it for
    /// itself, a draw not at all).
    inventory: bool,
    /// A reload has begun and the animator has not been back at rest
    /// since: a press of the trigger is dropped.
    reload_flag: bool,
    /// The hands are busy with the weapon, as the body's animator is told.
    hands_busy: bool,
    /// A quick put-away asked for (`hide_quick`): the draw speed its start
    /// gives the animator.
    quick_speed: Option<f32>,
    /// The arms damaged, left and right, as last told (`set_arms_damaged`).
    arms_damaged: [bool; 2],
    /// What the host told of a draw's end before this step
    /// (`set_appearing`): how the screen stands, whether the sights turn
    /// over, whether the movement lets them up.
    appearing: Option<(bool, bool, bool)>,
    /// A shot made by a mark's handler, which the recoil is given in the
    /// next frame; and whether a cylinder's hammer was down as it was made
    /// (the hammer may go down at a later mark of the same frame).
    shot_made: bool,
    shot_hammer_down: bool,
    operation: Operation,
    events: Vec<Event>,
    /// The kind of operation the left shoulder was last told had begun.
    left_stance_seen: Option<std::mem::Discriminant<Operation>>,
    /// The weapon's wear, malfunctions and heat, where the host gave them
    /// (`set_wear`); and the automatic fire's interval (`_shotsTime`: the
    /// game's operation is made once a weapon drawn and keeps the last the
    /// heat set) and the heat's factor applied in this burst (`float_6`).
    wear: Option<Box<Wear>>,
    auto_cycle: f32,
    burst_rate: f32,
    /// The weapon's own animator, and the marks it last delivered.
    animator: WeaponAnimator,
    marks: Vec<Mark>,
    /// Whether what is drawn of the animator is taken on curves without
    /// corners through its clips' samples (`set_drawn_smooth`).
    drawn_smooth: bool,
    /// Whether the animator leaves the bones it need not work out to be
    /// made from its mix when asked (`set_deferred_posing`).
    deferred_posing: bool,
    /// The weapon's blind-fire flag (`FirearmController.Blindfire`;
    /// `docs/spec/blind-fire.md`): while it is on, the sights, their keys,
    /// the fire mode, the looks and every reload are refused.
    blind_fire: bool,
    /// The bipod is out (`FirearmController.BipodState`), and the weapon is
    /// mounted forward on it, which runs the tactical reload's blend
    /// (`UpdateTacticalReload`; mounting spec, "The weapon's motion while
    /// mounted").
    bipod_out: bool,
    mount_tactical: bool,
    /// The build's underbarrel launcher, where one is fitted (`fit_launcher`).
    launcher: Option<Box<launcher::Launcher>>,
}

/// What the character's skill with a class of weapon (Pistols, Assault
/// rifles, Shotguns, Sniper rifles and so on; level 0 to 51) makes of the
/// animator's two speeds, for reloading and for drawing and putting away
/// (`SkillManager.GetWeaponBuffs`, `GetWeaponInfo`; `FirearmController.
/// SetAnimatorAndProceduralValues`): four thousandths a level on the
/// first and eight on the second, both figures written in the game's
/// code. (With an arm broken the game puts both back to 1.)
pub fn weapon_skill_speeds(level: u32) -> (f32, f32) {
    let level = level.min(51) as f32;
    (1.0 + 0.004 * level, 1.0 + 0.008 * level)
}

/// The game's own number for a fire mode (`Weapon.EFireMode`), which the
/// animator is told.
pub fn mode_number(mode: FireMode) -> f32 {
    match mode {
        FireMode::FullAuto => 0.0,
        FireMode::Single => 1.0,
        FireMode::Burst => 3.0,
        FireMode::SemiAuto => 5.0,
        FireMode::Doublet => 2.0,
        FireMode::DoubleAction => 4.0,
    }
}

impl Firearm {
}

#[cfg(test)]
pub(crate) mod tests;
