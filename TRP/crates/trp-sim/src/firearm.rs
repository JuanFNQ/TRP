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

use crate::marks::Mark;
use crate::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

mod chambers;
mod malfunctions;
pub use chambers::Chamber;
pub use malfunctions::{Wear, WearSwitches};
use crate::malfunction::Malf;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireMode {
    Single,
    FullAuto,
    Burst,
    /// One shot a press, the weapon loading the next by itself where
    /// `Single` on the same weapon is worked by hand (a shotgun's two
    /// modes: `docs/spec/shotgun.md`).
    SemiAuto,
    /// A revolver's trigger cocking the hammer itself, where `Single` on
    /// the same weapon is fired from a hammer cocked by hand.
    DoubleAction,
    /// Every barrel at once, where `Single` on the same weapon fires one.
    Doublet,
}

#[derive(Debug, Clone)]
pub struct FirearmSettings {
    /// Rounds per minute in automatic fire and for single shots.
    pub fire_rate: f32,
    pub single_fire_rate: f32,
    pub modes: Vec<FireMode>,
    pub burst_count: u32,
    pub magazine_capacity: u32,
    pub bolt_catch: bool,
    /// A press during a single shot is remembered and fired after it.
    pub can_queue_second_shot: bool,
    /// 1 plus the cartridge's recoil percentage.
    pub shot_strength: f32,
    /// Full names of the animator's resting state, which the others
    /// return to, of its fire state, and of the state that plays the
    /// switch of fire mode.
    pub rest_state: String,
    pub fire_state: String,
    pub fire_mode_state: String,
    /// The state a shot plays in the self-loading mode (`SA FIRE`).
    pub semi_fire_state: String,
    /// The magazine stays in the weapon and is filled a round at a time
    /// (`ReloadMode` `InternalMagazine`): a shotgun's tube.
    pub internal_magazine: bool,
    /// The fire mode is not changed while the bolt is held open
    /// (`NoFiremodeOnBoltcatch`).
    pub no_fire_mode_on_bolt_catch: bool,
    /// The action is worked by hand after each shot, once the trigger is
    /// let go (`BoltAction`: `docs/spec/bolt-action.md`).
    pub bolt_action: bool,
    /// A magazine that stays in the weapon is filled with the bolt open,
    /// which throws out the chambered round first
    /// (`MustBoltBeOpennedForInternalReload`).
    pub open_bolt_reload: bool,
    /// The bolt has to be opened for some reload of this weapon's, and it
    /// has a catch only the hand lets go (`MustBoltBeOpennedFor...`,
    /// `ManualBoltCatch`): a magazine's reload then leaves the catch alone
    /// as it begins.
    pub must_open_bolt: bool,
    pub manual_bolt_catch: bool,
    /// A round may be put into its chamber by hand (`isChamberLoad`).
    pub chamber_load: bool,
    /// The weapon has a stock that folds, fitted, and nothing on it that
    /// blocks the folding (`ItemManipulator.CanFold`).
    pub foldable: bool,
    /// How fast the animator's states for reloading and for drawing and
    /// putting away play, by the character's skill with this class of
    /// weapon (`weapon_skill_speeds`): 1 for an untrained character. The
    /// animator's own states say which of them go by which.
    pub reload_speed: f32,
    pub draw_speed: f32,
    /// The animation index of the magazine the weapon is fitted with and
    /// reloads with (`magAnimationIndex`), which the animator is told.
    pub magazine_variant: i32,
    /// The character's mastering of this weapon, 0 to 3. From 2 a reload
    /// keeps the sights up.
    pub mastering: u32,
    /// How deep the weapon may stick into an obstacle, in metres, before
    /// the sights come down.
    pub stop_aiming_at: f32,
    /// The weapon is fed from a cylinder (the game's `Revolver`, a
    /// launcher with a drum among them): how many chambers it has, and
    /// whether a spring turns it (`SpringDrivenCylinder`).
    pub cylinder: Option<(u32, bool)>,
    /// How many barrels are loaded one by one (`ReloadMode.OnlyBarrel`);
    /// none for any other weapon.
    pub barrels: u32,
    /// A fitted magazine may be changed the quick way, the old one let
    /// fall (`isFastReload`, `Weapon.CanReloadFast`; firing spec, "The
    /// quick reload").
    pub fast_reload: bool,
    /// A rocket launcher (the game's `RocketLauncher`): fired by its own
    /// operation, from the sights only (more-weapons spec, "One barrel
    /// loaded alone, and the rocket launcher").
    pub rocket_launcher: bool,
    /// A weapon with no chamber (`Chambers` empty with a detachable
    /// magazine: `Weapon.HasChambers` false; the PPSh-41, the RPD and
    /// RPDN, the M60s, the Uzi, the AA-12s): the round fired comes straight
    /// off its magazine, none waits in a chamber, and the animator is not
    /// told of one (more-weapons spec, "A weapon with no chamber").
    pub no_chamber: bool,
    /// Its template is mountable (`IsMountable`), and it has a bipod, which
    /// folds forward or not (`BipodViewController.BipodToggleDirection`):
    /// mounting spec, "Who may ask", "The bipod".
    pub mountable: bool,
    pub bipod: Option<bool>,
}

/// What the weapon did this step, for the host to show and sound.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Shot { strength: f32 },
    /// The shot's sound is to play, as the shot is made (`MakeShot`,
    /// `PlayFireBulletSound`): `trigger` is whether the controller still
    /// holds the trigger then (an automatic weapon's fire plays as a loop
    /// while it does), `doublet` whether two chambers fire together
    /// (`multiShot`). One for each chamber fired (`docs/spec/sound.md`,
    /// "The shots").
    FireSound { trigger: bool, doublet: bool },
    /// The controller's trigger let go (`IsTriggerPressed`'s setter, which
    /// then tells the weapon's sound player to end its loop: `BreakLoop`).
    TriggerReleased,
    /// The sights brought up or down by the aim's call (`SetAim`), which
    /// the game sounds: the kit's rattle either way, the weapon's aiming
    /// sound coming up (`docs/spec/sound.md`, "The aiming's sounds"). What
    /// else lowers them (a sprint, a reload, the body's state) is silent.
    Aimed(bool),
    ShellEject,
    DryFire,
    Sound(String),
    /// A run of automatic fire starts or ends.
    AutoFire(bool),
    MagazineOut,
    MagazineIn,
    /// A magazine's reload has begun, which is when the game makes the
    /// exchange (`ReloadExternalMagResult.Run`): the magazine the host
    /// chose goes into the weapon, and the one fitted leaves it with
    /// `left` rounds (none if none was fitted), to be let fall in a quick
    /// reload and otherwise put where there is room.
    MagazineSwap { left: Option<u32>, quick: bool },
    /// The old magazine has left the hand (the mark `MagHide`,
    /// `OnMagPuttedToRig`): one that had nowhere to go falls now.
    MagazineHidden,
    /// The next magazine has appeared in the hand (the mark `MagShow`,
    /// `OnMagAppeared`): the weapon draws one again.
    MagazineShown,
    /// A magazine moved out of the weapon or into it from the inventory has
    /// gone (the hand has put it away, `OnMagPuttedToRig`) or is in (and a
    /// round chambered from it if one was to be: `InsertMagOperation.
    /// SwitchToIdlingState`): the inventory's move is made now.
    MagazineMoved,
    /// A round put into the chamber from the inventory is in (the mark
    /// `AddAmmoInChamber`, `LoadCartridgeToChamber.OnAddAmmoInChamber`).
    Chambered,
    /// The hands gave up putting a round into the chamber, being put away
    /// meanwhile (`LoadCartridgeToChamber.HideWeapon`): the inventory's move
    /// is undone.
    ChamberGivenUp,
    /// The round taken out of the chamber from the inventory has gone (the
    /// discharge's `MagHide` mark, `DischargeOperation.OnMagPuttedToRig`):
    /// the chamber is empty.
    Discharged,
    /// The weapon's wear and malfunctions (`docs/spec/malfunctions.md`): a
    /// shot's malfunction (`SwitchToMalfunction`; known to the shooter at
    /// once); a shot's wear, for the item; the reminder (`ExamineMalfunction`
    /// when known: the glow and the words); the kind looked at and known
    /// (whether for the first time); the repair begun and over; a misfire
    /// put right by a reload, a draw or the chamber unloaded; the repair
    /// throwing out a failure to feed's round (onto the ground, a thing) or
    /// a misfired one (drawn only); the heat's stray shot.
    Malfunctioned(Malf),
    Worn { durability: f32, max_durability: f32 },
    MalfunctionReminder { malf: Malf, type_known: bool },
    MalfunctionExamined { malf: Malf, first: bool },
    MalfunctionRepairBegun(Malf),
    MalfunctionRepaired(Malf),
    MalfunctionCleared(Malf),
    FedRoundThrown,
    MisfiredRoundThrown,
    Autoshot,
    /// For the left shoulder (left-shoulder spec, "What takes it off"): an
    /// operation has begun (`SetLeftStanceAnimOnStartOperation`: the rest's
    /// start and its `IdleStart` mark, the draw, any other), or the
    /// inventory opened or shut (`FirearmController.SetInventoryOpened`).
    /// The host passes it to `Movement::left_stance_call`.
    LeftStance(crate::left_stance::Call),
    FireMode(FireMode),
    /// The battle screen is to name this way of firing in its ammunition's
    /// panel (`ShowFireMode`: the fire mode key, changed or not; the force
    /// to automatic where it changed; the look at the selector).
    FireModeShown(FireMode),
    /// A look at the magazine has begun (`Idling.CheckAmmo`): the battle
    /// screen names its count (`ShowAmmoDetails`).
    MagazineChecked,
    /// The weapon has appeared in the hands: its draw's last mark.
    Appeared,
    /// It has been put away: its put-away's last mark. Whatever is taken
    /// in hand next is made now.
    Hidden,
    /// So many rounds have left the character's stock for the weapon: one
    /// for an empty chamber as a tube's filling begins, and what went into
    /// the tube as it ends.
    RoundsTaken(u32),
    /// The chambered round has been thrown out of the weapon, unfired: a
    /// magazine filled with the bolt open (the game puts it on the ground
    /// as loot).
    RoundEjected,
    /// The live round in a single barrel being reloaded has come out
    /// (`ReloadSingleBarrelOperation.OnShellEjectEvent`): the game puts it
    /// in the vest or the pockets where it has room, and else on the
    /// ground.
    RoundUnloaded,
    /// The stock has been folded, or unfolded: the weapon is a cell
    /// shorter or longer, its recoil is without or with the stock's share,
    /// and it may be held closer (`docs/spec/shotgun.md`, "The stock").
    Folded(bool),
    /// What the body is to do with the thing in hand, by the game's number
    /// (the mark `ThirdAction`: 1 as it comes to the hands, 2 as it
    /// leaves them): for `Body::set_weapon_action`.
    BodyAction(i32),
    /// An operation that ends blind fire has begun with it on (the stock
    /// folded, the backpack dropped, the weapon put away, a reload): the
    /// host ends it in the movement and the weapon's motion
    /// (`Movement::end_blind_fire`; blind-fire spec, "What ends it").
    BlindFireEnded,
    /// An operation that ends a mount has been asked for (mounting spec,
    /// "What ends it"): a reload, the looks, the fire mode's look, with the
    /// exit (`StartExitingMountedState`); the stock folded and the weapon put
    /// away, at once (`ExitMountedState`). The host tells the movement
    /// (`Movement::leave_mount`), mounted or not.
    LeaveMount { at_once: bool },
    /// The bipod has been put out or in (`ToggleBipodOperation.Start`): the
    /// host asks the body for the gesture (`Interaction::bipod`) and tells the
    /// weapon's motion (`Hands::set_bipod`) and the movement
    /// (`Movement::set_mount_weapon`). The operation ends on the gesture's
    /// mark (`bipod_toggled`).
    Bipod { out: bool, forward: bool },
}

/// The mastering level from which a reload keeps the sights up.
const MASTERING_TO_AIM_IN_A_RELOAD: u32 = 2;
/// The turn-away's value below which sights an obstacle put down come back.
const TURN_AWAY_RUN_DOWN: f32 = 0.2;

/// The game's blend between 0 and 1 that runs at a set speed from a
/// moment that may lie ahead.
#[derive(Debug, Clone, Copy, Default)]
struct Blend {
    target: f32,
    speed: f32,
    start_value: f32,
    start_time: f32,
}

impl Blend {
    fn value(&self, now: f32) -> f32 {
        let direction = if self.target >= 0.5 { 1.0 } else { -1.0 };
        (self.start_value + (now - self.start_time) * self.speed * direction).clamp(0.0, 1.0)
    }

    /// Sends it toward `target` at `speed` a second, starting `delay`
    /// seconds from now. A target it already has changes nothing.
    fn aim_for(&mut self, target: f32, speed: f32, delay: f32, now: f32) {
        if self.target == target {
            return;
        }
        self.start_value = self.value(now);
        self.start_time = now + delay;
        (self.target, self.speed) = (target, speed);
    }
}

/// What a shot in hand is to hand over to when it ends, in place of
/// idling: a reload with a magazine of so many rounds, a tube filled from
/// so many rounds within reach, or the weapon put away (fast or not).
/// Asked for during the shot.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Planned {
    /// A magazine's reload, the quick way or not.
    Reload(u32, bool),
    Shells(u32),
    Hide(bool),
}

/// The operation in hand, as the game has one at a time.
#[derive(Debug, Clone, PartialEq)]
enum Operation {
    /// Not in the hands: before it is first drawn and after it has been
    /// put away.
    Away,
    /// Being drawn, until the draw's last mark. `hide` is a put-away asked
    /// for meanwhile; `arms` is the draw racking a round into an empty
    /// chamber.
    Spawn { hide: Option<bool>, arms: bool },
    Idle,
    /// One shot, then a wait until the weapon may fire again. The
    /// inventory opening is only noted.
    Single { cooldown: f32, fired: bool, ready: bool, second_shot: bool, open_inventory: bool, planned: Option<Planned>, malf: Malf },
    /// Shots on a clock for as long as the trigger is held. The clock
    /// waits for the fire state's own shot mark.
    Automatic { clock: f32, shots: u32, running: bool, open_inventory: bool, planned: Option<Planned> },
    /// A reload, until the new magazine is in and no round is left to
    /// chamber: the rest of the animation plays out with the logic idling.
    /// `catch_off` is that the bolt's catch has been let go in it once: a
    /// second mark for that is not answered; `mag_hidden` that the old
    /// magazine's mark for leaving the hand has been.
    Reload { mag_out: bool, mag_shown: bool, mag_in: bool, catch_off: bool, mag_hidden: bool, hide: Option<bool> },
    /// A magazine that stays in the weapon filled a round at a time
    /// (`ReloadInternalMagOperation`; shotgun spec, "Filling the tube"),
    /// until it is full, the rounds within reach are used up, or it is cut
    /// short. `within_reach` is what is left to load from, `loaded` what
    /// has gone into the tube (the weapon holds them when the operation
    /// ends), `can_abort` whether the trigger or the inventory may cut it
    /// short yet, `aborted` that one has, `chambered` that the mark of the
    /// round going into the chamber has been answered. With the bolt
    /// opened for it (`ReloadInternalMagWithOpenBoltOperation`; bolt-action
    /// spec, "Filling the magazine") `emptied` is that the chamber's mark
    /// for the round thrown out has been answered.
    Shells { within_reach: u32, loaded: u32, can_abort: bool, aborted: bool, chambered: bool, emptied: bool, hide: Option<bool> },
    /// The stock being folded or unfolded (`FoldStockOperation`), until
    /// the state's mark for it: `to` is how it will stand.
    Fold { to: bool, hide: Option<bool> },
    /// The bipod being put out or in (`ToggleBipodOperation`), until the
    /// body's gesture's mark (`OnBipodToggle`).
    Bipod { hide: Option<bool> },
    /// A look at the weapon (at its magazine, into its chamber, at its
    /// selector, or all over), through which the aim key and the trigger
    /// do nothing. It is ready to end once its state's first mark has
    /// come, and ends at the resting state's. `examine` is the look all
    /// over, which the trigger does end.
    Utility { ready: bool, time: f32, in_inventory: bool, examine: bool },
    /// The magazine taken out of the weapon from the inventory
    /// (`PullOutMagOperation`; firing spec, "A magazine taken out or put
    /// in from the inventory"), until the hand has put it away: `pulled`
    /// that its `MagOut` mark has come.
    InventoryMagOut { pulled: bool },
    /// A magazine put into the weapon from the inventory
    /// (`InsertMagOperation`), until it is in, or a round chambered from
    /// it: `shown`, `inserted` that those marks have come.
    InventoryMagIn { shown: bool, inserted: bool },
    /// A round put into the empty chamber from the inventory
    /// (`LoadCartridgeToChamber`), until it is in.
    InventoryChamber,
    /// The round taken out of the chamber from the inventory
    /// (`DischargeOperation`), until the hand has put it away.
    InventoryDischarge,
    /// A shot's malfunction, until its state's mark (`StartMalfunction`).
    Malfunction { malf: Malf },
    /// The look at a malfunction (`ExamineMalfunctionTypeOperation`).
    ExamineMalfunction,
    /// Its repair (`RepairMalfunction`), `ejected` that its round has been
    /// thrown out.
    RepairMalfunction { malf: Malf, ejected: bool },
    /// The backpack being dropped (`DropBackpackOperation`), until the
    /// body's gesture has it off the shoulders: nothing is taken
    /// meanwhile but the sights going down.
    DropBackpack,
    /// Being put away, until the put-away's last mark.
    Remove,
    /// A cylinder's chamber fired, or the hammer fallen on an empty one
    /// (`FireCylinderMagOperation`), until the resting state's mark.
    CylinderFire { fired: bool, dry: bool },
    /// One barrel fired, or all of them (`FireOnlyBarrelFireOperation`).
    BarrelsFire { fired: bool, shots: u32 },
    /// A cylinder or barrels being reloaded (`ReloadCylinderMagOperation`,
    /// `ReloadMultiBarrelOperation`): `added` is that the barrels' rounds
    /// are in.
    ChambersReload { can_abort: bool, aborted: bool, added: bool, hide: Option<bool> },
}

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
    /// The chambered round has gone to the shell port with a shot and the
    /// next is not yet in sight in the chamber (the fire clip's
    /// `AddAmmoInChamber`, an automatic cycle's `AUTO_CHAMBER`): what a host
    /// draws there (`round_drawn_in_chamber`).
    round_out: bool,
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
    /// A shot made by a mark's handler, which the recoil is given in the
    /// next frame.
    shot_made: bool,
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
    /// A loaded weapon that is not in the hands yet: a full magazine and a
    /// round in the chamber. `draw` takes it in hand; `start_drawn` has it
    /// there already.
    pub fn new(settings: FirearmSettings, animator: &WeaponAnimatorData) -> Self {
        let mut weapon = Self::made(settings, animator);
        if weapon.has_chambers() {
            weapon.fill_chambers(true);
        }
        weapon
    }

    /// The weapon as one found in a container (the user, 2026-10-12): no
    /// round in its chamber, its magazine, tube or cylinder full. Barrels
    /// are chambers and nothing else: they are empty.
    pub fn found(&mut self) {
        // A rocket launcher cannot be loaded: it is found with its rocket
        // (the app's choice, as a full magazine is).
        if self.settings.rocket_launcher {
            self.fill_chambers(true);
        } else if self.settings.barrels > 0 {
            self.fill_chambers(false);
        } else if !self.has_chambers() {
            self.chamber = false;
            // `PROVISIONAL`: a weapon with no chamber found unarmed, as one
            // with a chamber is found with it empty (the item's own `Armed`
            // in the game's loot is not read).
            self.armed = false;
        }
    }

    /// The weapon as one the run begins with in hand, drawn before: a
    /// round chambered, or every chamber of its own loaded.
    pub fn carried(&mut self) {
        if self.has_chambers() {
            self.fill_chambers(true);
        } else {
            self.chamber = !self.settings.no_chamber;
            self.armed = true;
        }
    }

    /// The fitted magazine's `magAnimationIndex`, where the host's item has
    /// another magazine than the build the weapon was made from.
    pub fn set_magazine_variant(&mut self, variant: i32) {
        self.mag_variant = variant;
    }

    /// The `magAnimationIndex` of the magazine the next reload or magazine
    /// put in from the inventory brings (`FirearmsAnimator.Reload`'s new
    /// type, `InsertMagOperation`'s): the host's, as it picks the magazine.
    pub fn next_magazine_variant(&mut self, variant: i32) {
        self.incoming_variant = variant;
    }

    /// Whether a reload's old magazine is still in the hand: the reload has
    /// begun and the old one has not yet been put away (`MagHide`).
    pub fn old_magazine_in_hand(&self) -> bool {
        matches!(self.operation, Operation::Reload { mag_hidden: false, .. })
    }

    /// Whether a round lies in the weapon's chamber of its own to be drawn
    /// there (`Firearms.SetRoundIntoWeapon`: the round's model at the
    /// chamber's point, always there while one is chambered, seen as the
    /// action opens): not a weapon with chambers of its own or none, and
    /// not from a shot until the next round is fed in sight
    /// (`MoveAmmoFromChamberToShellPort`, then `OnAddAmmoInChamber`).
    pub fn round_drawn_in_chamber(&self) -> bool {
        self.chamber && !self.round_out && !self.has_chambers() && !self.settings.no_chamber
    }

    /// Whether a round is ready to fire: the chamber's, or on a weapon with
    /// no chamber, the magazine's top one (`Idling.SetTriggerPressed`).
    pub fn loaded(&self) -> bool {
        self.chamber || (self.settings.no_chamber && self.magazine.is_some_and(|rounds| rounds > 0))
    }

    fn made(settings: FirearmSettings, animator: &WeaponAnimatorData) -> Self {
        Self {
            in_chambers: Vec::new(),
            camora: 0,
            hammer_closed: false,
            reload_free: Vec::new(),
            reload_shells: Vec::new(),
            reload_loading: 0,
            reload_reach: 0,
            reload_cached: 0,
            reload_remove: -1,
            reload_unloads: false,
            aim_told: false,
            aim_flags: false,
            chamber: !settings.no_chamber,
            armed: true,
            mag_variant: settings.magazine_variant,
            incoming_variant: settings.magazine_variant,
            round_out: false,
            magazine: Some(settings.magazine_capacity),
            incoming: settings.magazine_capacity,
            chambers: false,
            folded: false,
            mode: 0,
            trigger: false,
            held: false,
            hands_free: true,
            aiming: false,
            overlap: 0.0,
            interrupted: false,
            clock: 0.0,
            tactical: Blend::default(),
            inventory: false,
            reload_flag: false,
            hands_busy: false,
            shot_made: false,
            operation: Operation::Away,
            events: Vec::new(),
            left_stance_seen: None,
            wear: None,
            auto_cycle: 60.0 / settings.fire_rate,
            burst_rate: 0.0,
            animator: WeaponAnimator::new(animator),
            marks: Vec::new(),
            drawn_smooth: false,
            blind_fire: false,
            bipod_out: false,
            mount_tactical: false,
            settings,
        }
    }

    /// Whether the weapon may start blind fire now (`FirearmController.
    /// CanSetBlindFire`): its logic idling, not a rocket launcher, the
    /// inventory shut. So not in a shot, automatic fire, a reload, a look,
    /// a draw or a put-away.
    pub fn can_set_blind_fire(&self) -> bool {
        !self.settings.rocket_launcher && self.is_idling() && !self.inventory
    }

    /// Blind fire's figure as the movement's gate took it
    /// (`Movement::take_blind_fire`), told to the operation in hand
    /// (`FirearmOperation.BlindFire`): the idling and every fire operation
    /// pass it on, every other operation passes 0. Passed on, the flag is
    /// on for a side, and a start puts the sights down without the aim
    /// key's call (no kit's rattle: `BlindFire_Internal` sets `IsAiming`).
    /// Returns what was passed on, for the weapon's motion
    /// (`Hands::start_blind_fire`).
    pub fn set_blind_fire(&mut self, side: i32) -> i32 {
        let passes = matches!(
            self.operation,
            Operation::Idle | Operation::Single { .. } | Operation::Automatic { .. } | Operation::CylinderFire { .. } | Operation::BarrelsFire { .. }
        );
        let passed = if passes { side } else { 0 };
        self.blind_fire = passed != 0;
        if self.blind_fire {
            self.aiming = false;
        }
        passed
    }

    /// Whether the weapon's blind-fire flag is on.
    pub fn blind_fire(&self) -> bool {
        self.blind_fire
    }

    /// An operation that ends blind fire has begun (`SetBlindFire(0)` at
    /// its start): the host ends it in the movement and the weapon's motion
    /// (`Event::BlindFireEnded`). Only told with the flag on.
    fn ends_blind_fire(&mut self) {
        if self.blind_fire {
            self.blind_fire = false;
            self.events.push(Event::BlindFireEnded);
        }
    }

    /// Asks for every animation of the weapon to be drawn on curves
    /// without corners through its clips' samples, or as the engine has
    /// them (`WeaponAnimator::set_drawn_smooth`): the host's to choose,
    /// by the options' switch `SMOOTH FIRST PERSON ANIMATIONS` (the user,
    /// 2026-10-14). **How it is drawn, and no more**: the animator's pose,
    /// its states, its times and its marks are the same either way, and
    /// so is everything here that goes by them. As the engine has them
    /// unless asked.
    pub fn set_drawn_smooth(&mut self, on: bool) {
        self.drawn_smooth = on;
    }

    /// Has the weapon in the hands and at rest, as one that was drawn a
    /// while ago: what its draw and the idling operation's start have told
    /// the animator by then.
    pub fn start_drawn(&mut self) {
        self.fresh();
        self.animator.set_flag("Active", true);
        self.animator.set_layer_weight("LActions", 0.0);
        self.animator_fold();
        self.animator.play(&self.settings.rest_state, 0.0);
        self.animator.update(0.0);
        self.animator.deliver();
        self.to_idle();
    }

    /// Has the weapon in one of its fire modes with no switch played: as
    /// it was left when it was last in the hands. Before `draw` or
    /// `start_drawn`.
    pub fn start_in_mode(&mut self, mode: FireMode) {
        if let Some(index) = self.settings.modes.iter().position(|has| *has == mode) {
            self.mode = index;
        }
    }

    /// The same weapon built anew with other parts (the app's builds:
    /// `inventory::rig_key`) takes over what the build it replaces had in
    /// it and how that was set, before `start_drawn`: the rounds in its
    /// chambers (a cylinder's, barrels'; where it has as many), the turn of
    /// its cylinder, its own chamber, its magazine's count, its fire mode
    /// (where it has it) and its hammer. The app's: the game changes a part
    /// on the weapon's own model and its logic goes on as it was.
    pub fn take_over(&mut self, from: &Firearm) {
        if self.in_chambers.len() == from.in_chambers.len() {
            self.in_chambers = from.in_chambers.clone();
            self.camora = from.camora;
        }
        (self.chamber, self.magazine, self.hammer_closed, self.armed) = (from.chamber, from.magazine, from.hammer_closed, from.armed);
        (self.mag_variant, self.incoming_variant) = (from.mag_variant, from.incoming_variant);
        if let Some(mode) = from.settings.modes.get(from.mode).copied() {
            self.start_in_mode(mode);
        }
    }

    /// The same weapon built anew with another part (a magazine of another
    /// kind put in by a reload) takes over everything the build it replaces
    /// was doing, its operation and its animator's states and times with
    /// it, where its animator runs machines of the same shape: the reload
    /// goes on through the change, on the new build's tree (the app's: the
    /// game changes the part on the weapon's own model). Its own settings
    /// stay. Whether it could.
    pub fn carry_on_from(&mut self, from: &Firearm) -> bool {
        let mut animator = self.animator.clone();
        if !animator.take_state(&from.animator) {
            return false;
        }
        let settings = self.settings.clone();
        *self = from.clone();
        (self.settings, self.animator) = (settings, animator);
        true
    }

    /// Whether no magazine is in sight in the hands now: a reload's old one
    /// has left the hand (`MagHide`) and the new one is not yet shown
    /// (`MagShow`), or one put in from the inventory is not yet shown: when
    /// the weapon's model may change its magazine unseen.
    pub fn magazine_out_of_sight(&self) -> bool {
        matches!(self.operation, Operation::Reload { mag_hidden: true, mag_shown: false, .. } | Operation::InventoryMagIn { shown: false, .. })
    }

    /// Whether a magazine going in is in sight now: a reload's or one put
    /// in from the inventory, from its `MagShow` (where the game makes the
    /// new magazine's model on the weapon) to the operation's end.
    pub fn magazine_shown_going_in(&self) -> bool {
        matches!(self.operation, Operation::Reload { mag_shown: true, .. } | Operation::InventoryMagIn { shown: true, .. })
    }

    /// Has the hammer cocked or down, and the bolt caught open or home,
    /// as something before left them: for a replay that begins where a
    /// recording stood. After `start_drawn`.
    pub fn start_with(&mut self, armed: Option<bool>, bolt_caught: Option<bool>) {
        if let Some(armed) = armed {
            self.armed = armed;
            self.animator.set_flag("Armed", armed);
            self.animator.set_layer_weight("Hammer", if armed { 0.0 } else { 1.0 });
        }
        if let Some(caught) = bolt_caught {
            self.set_bolt_catch(caught);
        }
    }

    /// Has the stock folded, as the weapon was left: before `draw` or
    /// `start_drawn`, which tell the animator.
    pub fn start_folded(&mut self, folded: bool) {
        self.folded = folded && self.settings.foldable;
    }

    /// Whether the stock is folded.
    pub fn stock_folded(&self) -> bool {
        self.folded
    }

    /// The stock as the animator is told it (`FirearmsAnimator.Fold`): the
    /// layer that holds it folded, and the flag its two states go by.
    fn animator_fold(&mut self) {
        self.animator.set_layer_weight("Stock", self.folded as u8 as f32);
        self.animator.set_flag("StockFolded", self.folded);
    }

    /// The class skill's two speeds (`weapon_skill_speeds`, or a level's
    /// buffs: `skills::WeaponBuffs`), which the animator is told as the
    /// weapon is taken in hand (`SyncWithCharacterSkills`): a change shows
    /// from the next draw, as in the game.
    pub fn set_skill_speeds(&mut self, reload_speed: f32, draw_speed: f32) {
        (self.settings.reload_speed, self.settings.draw_speed) = (reload_speed, draw_speed);
        self.animator.set("SpeedReload", reload_speed);
        self.animator.set("SpeedDraw", draw_speed);
    }

    /// The animator's two speeds for what is under way alone
    /// (`FirearmsAnimator.SetSpeedParameters`), as the quick draw sets them;
    /// the weapon's next taking in hand puts its skill's back.
    pub fn set_animator_speeds(&mut self, reload_speed: f32, draw_speed: f32) {
        self.animator.set("SpeedReload", reload_speed);
        self.animator.set("SpeedDraw", draw_speed);
    }

    /// The logic and the animator as the game makes them for a weapon
    /// taken in hand: nothing left over from when it was last there but
    /// what the weapon itself holds (its rounds, its fire mode).
    fn fresh(&mut self) {
        self.animator.reset();
        self.animator.set("WeaponLevel", self.settings.mastering as f32);
        // The skill's two speeds, which the game tells the animator as the
        // weapon is taken in hand (`SyncWithCharacterSkills`).
        self.animator.set("SpeedReload", self.settings.reload_speed);
        self.animator.set("SpeedDraw", self.settings.draw_speed);
        self.trigger_to(false);
        (self.aiming, self.interrupted) = (false, false);
        (self.inventory, self.reload_flag, self.hands_busy, self.shot_made) = (false, false, false, false);
        self.marks.clear();
        // A weapon with a malfunction is made with it held on its animator
        // (`WeaponPrefab`'s spawn of a malfunctioned weapon).
        let malf = self.malfunction();
        if malf != Malf::None {
            self.malfunction_ints(-1, malf.number());
            self.animator.set_layer_weight("Malfunction", 1.0);
        }
    }

    /// Takes the weapon in hand (`SpawnOperation`): the animator leaves
    /// its first state for the draw, and at the draw's last mark the
    /// weapon has appeared (`Event::Appeared`) and idles. A weapon with
    /// rounds in its magazine and none chambered chambers one as it is
    /// drawn. The inventory opening meanwhile is not heard; but **when
    /// the weapon has appeared the game's owner tells the player how the
    /// screen stands afresh** (`GamePlayerOwner.SetHandsController`), so a
    /// host calls `set_inventory` on `Event::Appeared`, and a weapon
    /// drawn under an open screen then goes down for it.
    pub fn draw(&mut self) {
        if self.operation != Operation::Away {
            return;
        }
        self.fresh();
        self.animator_values();
        self.animator.set_flag("Active", true);
        self.animator.set_layer_weight("LActions", 0.0);
        self.animator_fold();
        self.animator_rounds();
        if self.settings.no_chamber {
            self.animator.set_flag("Armed", self.armed);
        }
        self.hands_busy = true;
        // The round goes over now; the animator hears of it at the mark
        // of the bolt going home. (Chambers of their own have no bolt to
        // rack.)
        let arms = !self.has_chambers() && !self.settings.no_chamber && !self.chamber && self.magazine.is_some_and(|rounds| rounds > 0);
        if arms {
            self.take_round();
        }
        self.operation = Operation::Spawn { hide: None, arms };
    }

    /// Has a draw just begun (`draw`, before the animator has stepped) rack
    /// the weapon as it comes up, for the look of it alone (the app's: the
    /// workbench's weapon drawn after a change was built, the user,
    /// 2026-10-21: "Just a nice visual thing"): the animator is told its
    /// chamber empty and a round in its magazine, so that it draws by the
    /// draw that racks, and the rounds as they are at the bolt's mark
    /// (`AddAmmoInChamber`); no round is moved. A draw that racks of
    /// itself, a weapon with chambers of its own and one with no magazine
    /// fitted are left as they are.
    pub fn rack_drawn(&mut self) {
        let has_chambers = self.has_chambers() || self.settings.no_chamber;
        let Operation::Spawn { arms, .. } = &mut self.operation else { return };
        let Some(rounds) = self.magazine.filter(|_| !*arms && !has_chambers) else { return };
        *arms = true;
        self.animator.set("AmmoInChamber", 0.0);
        self.animator.set("AmmoInMag", rounds.max(1) as f32);
    }

    /// Puts the weapon away (`HideWeapon`). At rest, and in a look at it,
    /// the sights go down, the trigger is let go and the animator is told
    /// at once; a shot in hand finishes first, and so do a reload up to
    /// where it hands over and a draw. `Event::Hidden` says when it is
    /// out of the hands.
    pub fn hide(&mut self, fast: bool) {
        // The thing in hand changing leaves a mount at once (`HandsChangingEvent`).
        self.events.push(Event::LeaveMount { at_once: true });
        match &mut self.operation {
            // A malfunction's own operations take no put-away: one asked for
            // meanwhile is lost (the base operation's `ProhibitedCall`;
            // `PROVISIONAL`: their own `HideWeapon` not read).
            Operation::Malfunction { .. } | Operation::ExamineMalfunction | Operation::RepairMalfunction { .. } => {}
            Operation::Idle | Operation::Utility { .. } => {
                if self.is_idling() {
                    self.aiming = false;
                    self.idle_trigger(false);
                }
                self.start_remove(fast);
            }
            Operation::Single { planned, .. } | Operation::Automatic { planned, .. } => {
                *planned = Some(Planned::Hide(fast));
                self.trigger_to(false);
                // A bolt action puts its sights down and works the bolt
                // before it goes (`BoltActionFireOperation.HideWeapon`).
                if self.settings.bolt_action {
                    self.aiming = false;
                    self.animator.set_flag("BoltActionReload", true);
                }
            }
            Operation::Reload { hide, .. } | Operation::Spawn { hide, .. } | Operation::Fold { hide, .. } | Operation::Bipod { hide } => *hide = Some(fast),
            Operation::ChambersReload { hide, aborted, .. } => (*hide, *aborted) = (Some(fast), true),
            // A shot from chambers of their own finishes first; a
            // put-away asked for meanwhile is lost.
            Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } => {}
            // A tube being filled is cut short for it.
            Operation::Shells { hide, aborted, .. } => (*hide, *aborted) = (Some(fast), true),
            // A backpack's drop, and a magazine moved from the inventory,
            // take no put-away at all: one asked for meanwhile is lost
            // (the base operation's `ProhibitedCall`).
            Operation::Remove | Operation::Away | Operation::DropBackpack | Operation::InventoryMagOut { .. } | Operation::InventoryMagIn { .. } | Operation::InventoryDischarge => {}
            // A round being put into the chamber is given up at once, and the
            // weapon goes (`LoadCartridgeToChamber.HideWeapon`).
            Operation::InventoryChamber => {
                self.events.push(Event::ChamberGivenUp);
                self.aiming = false;
                self.trigger_to(false);
                self.animator.set_flag("LoadOne", false);
                self.start_remove(fast);
            }
        }
    }

    fn start_remove(&mut self, fast: bool) {
        // The put-away ends blind fire (`Player.TryProceed`, and the
        // put-away's own start).
        self.ends_blind_fire();
        self.animator.set_flag("Active", false);
        self.animator.set_flag("FastHide", fast);
        self.hands_busy = true;
        self.operation = Operation::Remove;
    }

    /// The view's pitch, in degrees below level, which the game tells the
    /// animator every frame: a weapon whose resting state goes by it has
    /// the parameter, and the stock AKM's has not.
    pub fn set_aim_angle(&mut self, pitch: f32) {
        self.animator.set("Aim_angle", pitch);
    }

    /// The weapon's animator.
    pub fn animator(&self) -> &WeaponAnimator {
        &self.animator
    }

    /// The marks the animator delivered in the last step.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Puts the animator's resting state where a step of `dt` seconds
    /// will bring it to a point in its play, in cycles: a replay starts
    /// where the recording's stood after its first frame.
    pub fn rest_at(&mut self, time: f32, dt: f32) {
        self.animator.play(&self.settings.rest_state, time);
        self.animator.update(0.0);
        self.animator.update(-dt);
        self.animator.deliver();
    }

    /// The rounds as the animator is told them.
    fn animator_rounds(&mut self) {
        if self.has_chambers() {
            return self.animate_chambers();
        }
        if !self.settings.no_chamber {
            self.animator.set("AmmoInChamber", self.chamber as u8 as f32);
        }
        self.animator.set("AmmoInMag", self.magazine.unwrap_or(0) as f32);
    }

    /// What the idling operation tells the animator of the weapon as it
    /// starts (`SetAnimatorValues`).
    fn animator_values(&mut self) {
        let (fitted, variant) = (self.magazine.is_some(), self.mag_variant as f32);
        self.animator_rounds();
        if self.settings.modes.len() > 1 {
            self.animator.set("FireMode", mode_number(self.mode()));
        }
        self.animator.set_flag("MagInWeapon", fitted);
        self.animator.set("MagInWeaponFloat", fitted as u8 as f32);
        self.animator.set("RelTypeOld", if fitted { variant } else { -1.0 });
    }

    pub fn mode(&self) -> FireMode {
        self.settings.modes.get(self.mode).copied().unwrap_or(FireMode::Single)
    }

    /// Whether the weapon is at rest: its logic idling and its animator in
    /// its resting state, on no way out of it.
    pub fn is_idle(&self) -> bool {
        self.is_idling() && self.animator.rests_in(&self.settings.rest_state)
    }

    /// Whether the hands let the player look at a thing to work it
    /// (`FirearmController.CanInteract`): while the logic idles, whatever
    /// the animator is still playing out, or fires; not in a draw, a
    /// reload, a look or a put-away (interactions spec, "What is looked
    /// at").
    pub fn can_interact(&self) -> bool {
        matches!(
            self.operation,
            Operation::Idle | Operation::Single { .. } | Operation::Automatic { .. } | Operation::CylinderFire { .. } | Operation::BarrelsFire { .. }
        )
    }

    /// Whether the idling operation is the one in hand, whatever the
    /// animator is still playing out.
    pub fn is_idling(&self) -> bool {
        self.operation == Operation::Idle
    }

    /// Whether the look all over the weapon is the operation in hand.
    pub fn looks_over(&self) -> bool {
        matches!(self.operation, Operation::Utility { examine: true, .. })
    }

    /// Whether the weapon is in the hands at all: drawn, being drawn or
    /// being put away.
    pub fn in_hand(&self) -> bool {
        self.operation != Operation::Away
    }

    /// Whether a run of automatic fire is under way.
    pub fn auto_fire(&self) -> bool {
        matches!(self.operation, Operation::Automatic { .. })
    }

    /// Tells the weapon where the finger is. Only changes matter.
    /// `allowed` is whether the character's movement state lets the hands
    /// take a press (`Movement::can_press_trigger`): a weapon at rest
    /// drops one made in a sprint or a crawl, and holding the trigger
    /// until the state ends does not fire.
    pub fn set_trigger(&mut self, pressed: bool, allowed: bool) {
        self.hands_free = allowed;
        if pressed == self.held {
            return;
        }
        self.held = pressed;
        self.trigger_changed(pressed);
    }

    fn trigger_changed(&mut self, pressed: bool) {
        match &mut self.operation {
            Operation::Idle => self.idle_trigger(pressed),
            Operation::Single { second_shot, .. } => {
                *second_shot |= pressed && self.settings.can_queue_second_shot;
                self.trigger_to(self.trigger && pressed);
                // A bolt action's bolt is worked once the trigger is no
                // longer held (`BoltActionFireOperation.SetTriggerPressed`).
                if self.settings.bolt_action {
                    self.animator.set_flag("BoltActionReload", !self.trigger);
                }
            }
            Operation::Automatic { .. } => self.trigger_to(pressed),
            // A press cuts the filling of a tube short, once a round has
            // shown in the hand; before that it is lost.
            Operation::Shells { can_abort, aborted, .. } => *aborted |= pressed && *can_abort,
            Operation::ChambersReload { can_abort, aborted, .. } => *aborted |= pressed && *can_abort,
            // A shot from a cylinder or from barrels only lets go of the
            // trigger.
            Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } => {
                self.trigger_to(self.trigger && pressed);
                // A rocket launcher's animator has the trigger
                // (`RocketLauncherFire.PrepareShot`).
                if self.settings.rocket_launcher {
                    self.animator.set_flag("Fire", self.trigger);
                }
            }
            // The look all over is ended by the trigger, pressed or let
            // go, and the idling operation takes it from there
            // (`UtilityOperation.SetTriggerPressed`).
            Operation::Utility { examine: true, .. } => {
                (self.hands_busy, self.reload_flag) = (false, false);
                self.to_idle();
                self.idle_trigger(pressed);
            }
            // A reload, a look at the magazine, a draw and a put-away take
            // no notice of the trigger.
            _ => {}
        }
    }

    /// The trigger as the idling operation takes it. A press is dropped
    /// where the movement state gives the hands no say, and while a
    /// reload's animation has not brought the weapon back to rest.
    ///
    /// A switch of the fire mode does not stop it: the weapon idles
    /// through the switch, the mode is the new one from the key on, and a
    /// press 0.3 s into the switch's animation fires (the fifteenth
    /// recording, `64-fire-in-firemode-switch`; firing spec, "The trigger
    /// in a fire-mode switch").
    fn idle_trigger(&mut self, pressed: bool) {
        if pressed && (!self.hands_free || self.reload_flag) {
            return;
        }
        // A malfunction takes the trigger: no shot (`Idling.SetTriggerPressed`).
        if self.malfunction_trigger(pressed) {
            return;
        }
        self.animator_rounds();
        // Chambers of their own have operations of their own, and so has
        // a rocket launcher.
        if self.settings.rocket_launcher {
            return self.rocket_trigger(pressed);
        }
        if self.settings.cylinder.is_some() {
            return self.cylinder_trigger(pressed);
        }
        if self.settings.barrels > 0 {
            return self.barrels_trigger(pressed);
        }
        self.trigger_to(pressed);
        if pressed && self.loaded() {
            // One shot a press in the two single modes; anything else
            // fires for as long as the trigger is held.
            if matches!(self.mode(), FireMode::Single | FireMode::SemiAuto) {
                self.start_single();
            } else {
                self.start_automatic();
            }
            return;
        }
        // On an empty chamber the animator has the trigger, and a press
        // only clicks.
        self.animator.set_flag("Fire", pressed);
        if pressed {
            self.events.push(Event::DryFire);
        }
    }

    /// The character's inventory opens or shuts (`SetInventoryOpened`;
    /// inventory spec, "The inventory open"). A weapon at rest puts its
    /// sights down, lets the trigger go and tells its animator at once,
    /// whose own states take the hands down and back. A shot in hand only
    /// notes it and tells the animator as it hands back to idling; a
    /// reload tells it once the magazine is out or the new one has shown;
    /// a weapon being drawn or put away does not hear of it at all (one
    /// that has just appeared is told afresh: `draw`).
    pub fn set_inventory(&mut self, open: bool) {
        // The left shoulder first (`FirearmController.SetInventoryOpened`).
        self.events.push(Event::LeftStance(crate::left_stance::Call::Inventory(open)));
        // Opened, blind fire is over (`FirearmController.InventoryOpened`).
        if open {
            self.ends_blind_fire();
        }
        match &mut self.operation {
            // The malfunction's own operations leave the animator be (the
            // base operation's `ProhibitedCall`); the rest after them goes
            // by it.
            Operation::Malfunction { .. } | Operation::ExamineMalfunction | Operation::RepairMalfunction { .. } => self.inventory = open,
            Operation::Idle => {
                self.aiming = false;
                self.idle_trigger(false);
                self.inventory = open;
                self.animator.set_flag("Inventory", open);
            }
            Operation::Utility { in_inventory, time, .. } => {
                *in_inventory = open;
                if !open {
                    *time = 0.0;
                }
                self.inventory = open;
                self.animator.set_flag("Inventory", open);
            }
            Operation::Single { open_inventory, .. } | Operation::Automatic { open_inventory, .. } => {
                *open_inventory = open;
                // The screen does not wait for the trigger: the bolt is
                // worked now.
                if self.settings.bolt_action {
                    self.animator.set_flag("BoltActionReload", true);
                }
            }
            // A stock being folded only notes it, and tells the animator
            // when the fold is over; so does the bipod's toggle.
            Operation::Fold { .. } | Operation::Bipod { .. } => self.inventory = open,
            Operation::Reload { mag_out, mag_shown, .. } => {
                self.inventory = open;
                if *mag_out || *mag_shown {
                    self.animator.set_flag("Inventory", open);
                }
            }
            // A tube being filled is cut short by the screen, opening or
            // shutting; the animator hears of it only once a round has
            // shown (and one opened before that is never told to it by
            // this operation: the game's own setter cannot do what it is
            // written to do. Shotgun spec, "Filling the tube").
            Operation::Shells { can_abort, aborted, .. } => {
                self.inventory = open;
                *aborted = true;
                if *can_abort {
                    self.animator.set_flag("Inventory", open);
                }
            }
            // A backpack being dropped only notes it, and tells the
            // animator when the pack is off.
            Operation::DropBackpack => self.inventory = open,
            // A cylinder or barrels being reloaded is cut short as a
            // tube is; a shot from either only notes the screen
            // (`FireCylinderMagOperation.SetInventoryOpened`).
            Operation::ChambersReload { can_abort, aborted, .. } => {
                self.inventory = open;
                *aborted = true;
                if *can_abort {
                    self.animator.set_flag("Inventory", open);
                }
            }
            Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } => self.inventory = open,
            // A magazine moved from the inventory tells the animator at
            // once (`PullOutMagOperation`, `InsertMagOperation`:
            // `SetInventoryOpened`).
            Operation::InventoryMagOut { .. } | Operation::InventoryMagIn { .. } | Operation::InventoryChamber | Operation::InventoryDischarge => {
                self.inventory = open;
                self.animator.set_flag("Inventory", open);
            }
            Operation::Spawn { .. } | Operation::Remove | Operation::Away => {}
        }
    }

    /// Whether the inventory may move the weapon's magazine now, which the
    /// hands then do (`FirearmController.CanExecute`): only while the
    /// logic idles.
    pub fn can_move_magazine(&self) -> bool {
        matches!(self.operation, Operation::Idle)
    }


    /// The magazine moved out of the weapon from the inventory
    /// (`PullOutMagOperation`; firing spec, "A magazine taken out or put
    /// in from the inventory"): the hands take it out by the reload's first
    /// half from their inventory pose, and `Event::MagazineMoved` says when
    /// it has gone. False, and nothing done, if the logic is not idling or
    /// no magazine is in.
    pub fn pull_out_magazine(&mut self) -> bool {
        if !self.can_move_magazine() || self.magazine.is_none() {
            return false;
        }
        self.trigger_to(false);
        self.animator.set_flag("MagInFromInv", false);
        self.animator.trigger("MagOutFromInv");
        self.animator.set_flag("CanReload", false);
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("IsExternalMag", true);
        // (`PullOutMagOperation.Start`: the fitted one's, nought with none.)
        self.animator.set("RelTypeOld", if self.magazine.is_some() { self.mag_variant as f32 } else { 0.0 });
        let s = &self.settings;
        if s.bolt_catch && self.chamber && !s.manual_bolt_catch && !s.must_open_bolt {
            self.set_bolt_catch(false);
        }
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::InventoryMagOut { pulled: false };
        true
    }

    /// A magazine of `rounds` put into the weapon from the inventory
    /// (`InsertMagOperation`): the hands put it in by the reload's second
    /// half, and chamber a round from it if the chamber is empty;
    /// `Event::MagazineMoved` says when it is done. False, and nothing
    /// done, if the logic is not idling or a magazine is in already.
    pub fn insert_magazine(&mut self, rounds: u32) -> bool {
        if !self.can_move_magazine() || self.magazine.is_some() {
            return false;
        }
        self.trigger_to(false);
        self.incoming = rounds;
        self.chambers = !self.chamber && !self.settings.no_chamber && rounds > 0;
        let variant = self.incoming_variant as f32;
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("IsExternalMag", true);
        self.animator.set_flag("CanReload", true);
        self.animator.set("RelTypeOld", variant);
        self.animator.set("RelTypeNew", variant);
        self.animator.trigger("MagInFromInv");
        let s = &self.settings;
        if s.bolt_catch && self.chamber && !self.chambers && !s.manual_bolt_catch && !s.must_open_bolt {
            self.set_bolt_catch(false);
        }
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::InventoryMagIn { shown: false, inserted: false };
        true
    }


    /// Whether the hands would take a round put into the chamber from the
    /// inventory now (`Idling.Execute`, a round moved to a chamber): the
    /// logic idles, the weapon has one chamber of the weapon's own and it
    /// is empty, a weapon of a detachable magazine has none in, and its
    /// chamber is loaded by hand (`isChamberLoad`).
    pub fn can_load_chamber(&self) -> bool {
        self.can_move_magazine() && !self.has_chambers() && !self.settings.no_chamber && !self.chamber && (self.settings.internal_magazine || self.magazine.is_none()) && self.settings.chamber_load
    }

    /// Whether the weapon has one chamber of its own (not a cylinder's nor
    /// barrels'), which `chamber` says of.
    pub fn single_chamber(&self) -> bool {
        !self.has_chambers()
    }

    /// A round put into the empty chamber from the inventory
    /// (`LoadCartridgeToChamber`; firing spec, "A round into the chamber
    /// from the inventory"): aiming off, the trigger let go, the animator's
    /// load of one (`LoadOne`, chamber 0) from its inventory pose or its
    /// rest; at the state's `AddAmmoInChamber` mark the round is in
    /// (`Event::Chambered`) and the logic idles. False, and nothing done,
    /// where the hands would not take it (`can_load_chamber`).
    pub fn load_chamber(&mut self) -> bool {
        if !self.can_load_chamber() {
            return false;
        }
        self.aiming = false;
        self.trigger_to(false);
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("UseLeftHand", false);
        self.animator.set_flag("LoadOne", true);
        self.animator.set("ChamberIndex", 0.0);
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::InventoryChamber;
        true
    }

    /// Whether the hands would let the chambered round be taken out from
    /// the inventory now (`Idling.Execute`, a round moved out of a chamber):
    /// the logic idles, the weapon has one chamber of its own with a round
    /// in it, and a weapon of a detachable magazine has none in, one of a
    /// magazine of its own has it empty.
    pub fn can_discharge(&self) -> bool {
        let emptied = if self.settings.internal_magazine { self.magazine.unwrap_or(0) == 0 } else { self.magazine.is_none() };
        // (An animator without the state would never give its mark: the
        // round stays, `PROVISIONAL`, the game's weapons all having it.)
        let played = self.animator.has("Discharge") && self.animator.has_state("Hands.DISCHARGE");
        self.can_move_magazine() && !self.has_chambers() && !self.settings.no_chamber && self.chamber && emptied && played
    }

    /// The chambered round taken out from the inventory (`DischargeOperation`;
    /// firing spec, "The chambered round taken out from the inventory"):
    /// aiming off, the trigger let go, blind fire ended, the animator's
    /// `Discharge` (the AKM's `DISCHARGE`, from its inventory pose or its
    /// rest); at the state's `MagHide` mark the chamber is empty
    /// (`Event::Discharged`) and the logic idles. False, and nothing done,
    /// where the hands would not let it out (`can_discharge`).
    pub fn discharge(&mut self) -> bool {
        if !self.can_discharge() {
            return false;
        }
        // It puts a misfire right (`DischargeOperation.Start`).
        self.clear_misfire();
        self.aiming = false;
        self.animator.set_flag("Discharge", true);
        self.trigger_to(false);
        self.animator.set_flag("Fire", false);
        self.ends_blind_fire();
        self.animator.set("ChamberIndex", 0.0);
        // (`SetLoadOneForNextOperation`: a load after it only for a round
        // moved from one chamber to another.)
        self.animator.set_flag("LoadOne", false);
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::InventoryDischarge;
        true
    }

    /// A magazine's move from the inventory done: the inventory makes it,
    /// and the logic idles.
    fn inventory_move_over(&mut self) {
        self.events.push(Event::MagazineMoved);
        self.to_idle();
    }

    /// Whether the animator is in its resting state, on a way out of it or
    /// not (`FirearmsAnimator.IsIdling`): what the game asks before it
    /// begins a gesture of the hands on the world.
    pub fn animator_idling(&self) -> bool {
        self.in_hand() && self.animator.current_is(&self.settings.rest_state)
    }

    /// Whether the hands are down for the open inventory, or on their way
    /// down: the game then tells the body's animator, which is the pose
    /// others see (`Player.WaitWeaponInventory`).
    pub fn in_inventory(&self) -> bool {
        IN_INVENTORY.iter().any(|state| self.animator.current_is(state))
    }

    /// Notes where the finger is in a frame in which a screen has the
    /// keys: the weapon is not told, so a press made under the screen
    /// fires nothing when it shuts with the trigger still held.
    pub fn note_trigger(&mut self, pressed: bool) {
        self.held = pressed;
    }

    /// Whether the sights are up.
    pub fn aiming(&self) -> bool {
        self.aiming
    }

    /// Whether a reload is in hand: from its start until the new magazine
    /// is in, or the round chambered where the chamber was empty. The
    /// animation runs on for a while after. The body's animator is told
    /// this (`movement::Input::reloading`): it is what holds a lying body
    /// still.
    pub fn reloading(&self) -> bool {
        matches!(self.operation, Operation::Reload { .. } | Operation::Shells { .. } | Operation::ChambersReload { .. })
    }

    /// The aim key, pressed or let go. `allowed` is whether the
    /// character's movement state lets the sights come up
    /// (`Movement::can_aim`). Letting go always lowers them, except while
    /// a reload is in hand by a character who does not know the weapon
    /// well, where the key does nothing either way. What else lowers them
    /// is `lower_aim` and `sprint_started`.
    pub fn set_aim(&mut self, want: bool, allowed: bool) {
        // In blind fire the call is refused outright, either way
        // (`FirearmController.SetAim`).
        if self.blind_fire {
            return;
        }
        // Whoever uses the key, an obstacle no longer has the sights to
        // give back.
        self.interrupted = false;
        let was = self.aiming;
        self.aim(want, allowed);
        if self.aiming != was {
            self.events.push(Event::Aimed(self.aiming));
        }
    }

    /// The call the aim key makes, as the operation in hand takes it.
    fn aim(&mut self, want: bool, allowed: bool) {
        let blocked = self.overlap > self.settings.stop_aiming_at;
        match self.operation {
            // A weapon at rest that is pushed back by an obstacle does not
            // come up.
            Operation::Idle => {
                if !want || (allowed && !blocked) {
                    self.aiming = want;
                }
            }
            // In a shot the game does not ask anything.
            Operation::Single { .. } | Operation::Automatic { .. } => self.aiming = want,
            // In a reload only a character who knows the weapon well has
            // the key.
            Operation::Reload { .. } => {
                if self.settings.mastering >= MASTERING_TO_AIM_IN_A_RELOAD && (!want || allowed) && !(want && blocked) {
                    self.aiming = want;
                }
            }
            // Nor through a look at the weapon, a draw or a put-away,
            // whoever the character is.
            _ => {}
        }
    }

    /// The weapon would stick `depth` metres into something, as measured
    /// at the end of a frame (`Movement::weapon_overlap`). Past the limit
    /// the sights go down; once it is clear again, and the turn-away has
    /// run down (`Hands::turn_away_value`), they come back by themselves,
    /// unless the aim key was used in between. `allowed` is whether the
    /// movement state lets the sights be up (`Movement::can_aim`).
    pub fn overlap(&mut self, depth: f32, turn_away: f32, allowed: bool) {
        self.overlap = depth;
        let limit = self.settings.stop_aiming_at;
        if depth > limit && self.aiming {
            self.set_aim(false, allowed);
            self.interrupted = true;
        } else if depth < limit && turn_away < TURN_AWAY_RUN_DOWN && self.interrupted && !self.aiming {
            // Asked once: a raise that is refused is not tried again.
            self.set_aim(true, allowed);
        }
    }

    /// The body has done what puts the sights down: it has entered a
    /// movement state that does not allow aiming
    /// (`Movement::take_aim_lowered`). They do not come back by themselves.
    /// Only an idling weapon obeys: in a shot, a burst or a reload in hand
    /// the game's call does nothing, and the sights stay up until the next
    /// one.
    pub fn lower_aim(&mut self) {
        if self.is_idling() {
            self.aiming = false;
        }
        // A bolt action's shot does answer: the sights go down and the
        // trigger is let go, which works the bolt
        // (`BoltActionFireOperation.OnAimingDisabled`).
        if self.settings.bolt_action && matches!(self.operation, Operation::Single { .. }) {
            self.aiming = false;
            self.trigger_changed(false);
        }
    }

    /// The character's sprint has switched on
    /// (`Movement::take_sprint_started`), which is at the key and before
    /// the sprinting state begins: the sights go down as above, and the
    /// trigger is let go whatever the weapon is doing.
    pub fn sprint_started(&mut self) {
        self.lower_aim();
        self.trigger_changed(false);
    }

    /// The animator told of the sights as they now stand
    /// (`FirearmController.SetAimingInAnimator`; more-weapons spec, "One
    /// barrel loaded alone, and the rocket launcher"): `AimingIn` up and
    /// `AimingOut` down with them up, the other way with them down, and
    /// `PrevIsAimingFloatState` the other way about; and, but for a rocket
    /// launcher, whose own clips' marks set it, `IsAimingFloat`.
    fn tell_aim(&mut self) {
        let on = self.aiming;
        self.aim_told = on;
        if !self.settings.rocket_launcher {
            self.animator.set("IsAimingFloat", if on { 1.0 } else { 0.0 });
        }
        self.animator.set_flag("AimingIn", on);
        self.animator.set_flag("AimingOut", !on);
        self.animator.set("PrevIsAimingFloatState", if on { 0.0 } else { 1.0 });
        self.aim_flags = true;
    }

    /// How far a reload with the sights up has stiffened the weapon, 0 to 1.
    pub fn tactical_reload(&self) -> f32 {
        self.tactical.value(self.clock)
    }

    /// Swaps the magazine for a full one.
    pub fn reload(&mut self) {
        self.reload_with(self.settings.magazine_capacity);
    }

    /// Swaps the magazine for one holding `rounds`. What the player carries
    /// is for the host game to decide. Asked for during a shot, the reload
    /// follows it, if nothing else is to.
    pub fn reload_with(&mut self, rounds: u32) {
        self.reload_magazine(rounds, false);
    }

    /// The quick reload's key (`FirearmHandsInputTranslator.QuickReload`;
    /// firing spec, "The quick reload"): heard only with the animator at
    /// rest. With a magazine fitted, on a weapon that allows it, the old
    /// magazine is let fall and the new one is fitted the quicker way;
    /// otherwise this is the plain reload. `rounds` is what the new
    /// magazine holds. `Event::MagazineSwap` says which it was.
    pub fn quick_reload_with(&mut self, rounds: u32) {
        if !self.animator_idling() {
            return;
        }
        let quick = self.settings.fast_reload && self.magazine.is_some();
        self.reload_magazine(rounds, quick);
    }

    fn reload_magazine(&mut self, rounds: u32, quick: bool) {
        // The reload's key leaves a mount first (`FirearmHandsInputTranslator`).
        self.events.push(Event::LeaveMount { at_once: false });
        // No reload starts in blind fire (`CanStartReload`), nor with a
        // failure to feed, which reminds instead.
        if self.blind_fire {
            return;
        }
        if self.malfunction() == Malf::Feed {
            return self.remind();
        }
        match &mut self.operation {
            // A reload asked for while the last one's animation is still
            // playing out is taken like any other: the operation begins
            // at the key and the animator takes it up when it is back at
            // rest (the fifteenth recording, `69-reload-in-reload-tail`).
            Operation::Idle => {
                // A character who does not know the weapon well lowers
                // the sights to reload.
                if self.settings.mastering < MASTERING_TO_AIM_IN_A_RELOAD {
                    self.aiming = false;
                }
                self.idle_trigger(false);
                self.begin_reload(rounds, quick);
            }
            Operation::Single { planned, .. } | Operation::Automatic { planned, .. } if planned.is_none() => {
                *planned = Some(Planned::Reload(rounds, quick));
                self.trigger_to(false);
                self.work_bolt_for_reload();
            }
            Operation::Single { .. } => self.work_bolt_for_reload(),
            _ => {}
        }
    }

    /// A reload asked for in a bolt action's shot works the bolt, whether
    /// or not the reload is taken (`BoltActionFireOperation.ReloadMag`,
    /// `ReloadWithAmmo`).
    fn work_bolt_for_reload(&mut self) {
        if self.settings.bolt_action {
            self.animator.set_flag("BoltActionReload", true);
        }
    }

    /// The reload operation's start: the animator is told what it is to
    /// do, and does it from its resting state. The host is told of the
    /// exchange, which the game makes now. A quick one raises `ReloadFast`
    /// where the plain one raises `Reload` (`FirearmsAnimator.Reload`).
    fn begin_reload(&mut self, rounds: u32, quick: bool) {
        self.events.push(Event::MagazineSwap { left: self.magazine, quick });
        self.incoming = rounds;
        self.chambers = !self.chamber && !self.settings.no_chamber && rounds > 0;
        let fitted = self.magazine.is_some();
        self.animator.set_flag("Inventory", false);
        self.animator.set_flag("IsExternalMag", true);
        self.animator.set_flag("CanReload", true);
        self.animator.set("RelTypeOld", if fitted { self.mag_variant as f32 } else { -1.0 });
        self.animator.set("RelTypeNew", self.incoming_variant as f32);
        self.animator.trigger(if quick { "ReloadFast" } else { "Reload" });
        // With a round chambered and none to chamber from the new
        // magazine, a bolt held open is let go as the reload begins
        // (`ReloadExternalMagOperation.Start`).
        let s = &self.settings;
        if s.bolt_catch && self.chamber && !self.chambers && !s.manual_bolt_catch && !s.must_open_bolt {
            self.set_bolt_catch(false);
        }
        (self.hands_busy, self.reload_flag) = (true, true);
        self.operation = Operation::Reload { mag_out: false, mag_shown: false, mag_in: false, catch_off: false, mag_hidden: false, hide: None };
    }

    /// Fills a magazine that stays in the weapon, a round at a time
    /// (`FirearmHandsInputTranslator.ReloadWithAmmo`, `Idling.
    /// ReloadWithAmmo`; shotgun spec, "Filling the tube"). `within_reach`
    /// is how many rounds the character has to load from; with none the
    /// key does nothing. A weapon at rest lowers its sights (below
    /// mastering 2) and lets the trigger go, and then **begins only if the
    /// tube has room**: a full tube under an empty chamber is refused.
    /// Asked for during a shot, it follows the shot if nothing else is to.
    /// `Event::RoundsTaken` says what has left the character's stock.
    pub fn reload_shells(&mut self, within_reach: u32) {
        if within_reach == 0 || self.magazine.is_none() || self.blind_fire {
            return;
        }
        match &mut self.operation {
            Operation::Idle => {
                if self.settings.mastering < MASTERING_TO_AIM_IN_A_RELOAD {
                    self.aiming = false;
                }
                self.idle_trigger(false);
                // A single barrel is reloaded whatever it holds
                // (`ReloadSingleBarrelResult.Run`).
                let room = self.magazine.is_some_and(|rounds| rounds < self.settings.magazine_capacity) || self.settings.barrels == 1;
                if room {
                    if self.settings.cylinder.is_some() {
                        self.begin_cylinder_reload(within_reach);
                    } else if self.settings.barrels > 0 {
                        self.begin_barrels_reload(within_reach);
                    } else {
                        self.begin_shells(within_reach);
                    }
                }
            }
            Operation::Single { planned, .. } | Operation::Automatic { planned, .. } if planned.is_none() => {
                *planned = Some(Planned::Shells(within_reach));
                self.trigger_to(false);
                self.work_bolt_for_reload();
            }
            Operation::Single { .. } => self.work_bolt_for_reload(),
            _ => {}
        }
    }

    /// The operation's start (`BaseReloadOperation.Start`,
    /// `ReloadInternalMagBase.Start`, `ReloadInternalMagOperation.
    /// Prepare`): the animator is told, and **an empty chamber is given
    /// its round at once**; the animator hears of that at the mark of the
    /// round going in.
    fn begin_shells(&mut self, mut within_reach: u32) {
        self.animator.set_flag("Inventory", false);
        self.animator.set_flag("CanReload", true);
        self.animator.trigger("Reload");
        self.animator.set_flag("IsExternalMag", false);
        self.hands_busy = true;
        if self.settings.open_bolt_reload {
            // The bolt is opened for it: a chambered round leaves the
            // weapon at once (`ReloadInternalMagWithOpenBoltOperation.
            // Prepare`), and is thrown out at the chamber's mark.
            self.chamber = false;
        } else if !self.chamber && within_reach > 0 {
            self.chamber = true;
            within_reach -= 1;
            self.events.push(Event::RoundsTaken(1));
        }
        self.operation =
            Operation::Shells { within_reach, loaded: 0, can_abort: false, aborted: false, chambered: false, emptied: false, hide: None };
    }

    /// Whether another round is to follow (`ReloadInternalMagBase.
    /// CanReload`): there is one left to load from, and the tube is not
    /// full with those already on their way in.
    fn shells_go_on(&self) -> bool {
        let Operation::Shells { within_reach, loaded, .. } = self.operation else { return false };
        within_reach > loaded && self.magazine.unwrap_or(0) + loaded != self.settings.magazine_capacity
    }

    /// The mark of the round going into the chamber, answered once
    /// (`ReloadInternalMagOperation.OnAddAmmoInChamber`).
    fn shell_chambered(&mut self) {
        let Operation::Shells { can_abort, chambered, .. } = &mut self.operation else { return };
        (*can_abort, *chambered) = (true, true);
        // With the bolt opened for the filling this is the bolt going
        // home, which ends it (`ReloadInternalMagWithOpenBoltOperation.
        // OnAddAmmoInChamber`).
        if self.settings.open_bolt_reload {
            self.shells_over();
            return;
        }
        self.animator.set("AmmoInChamber", self.chamber as u8 as f32);
        let goes_on = self.shells_go_on();
        self.animator.set_flag("CanReload", goes_on);
        if !goes_on || matches!(self.operation, Operation::Shells { aborted: true, .. }) {
            self.shells_over();
        }
    }

    /// The mark of a round going into the tube (`AddAmmoToMag`).
    fn shell_in(&mut self) {
        let Operation::Shells { loaded, can_abort, .. } = &mut self.operation else { return };
        *can_abort = true;
        *loaded += 1;
        let in_tube = self.magazine.unwrap_or(0) + *loaded;
        self.animator.set("AmmoInMag", in_tube as f32);
        let aborted = matches!(self.operation, Operation::Shells { aborted: true, .. });
        // With the bolt open the filling does not end here: the animator
        // is told whether another round follows, and shuts the bolt when
        // none does.
        if self.settings.open_bolt_reload {
            let goes_on = self.shells_go_on() && !aborted;
            self.animator.set_flag("CanReload", goes_on);
            return;
        }
        if !self.shells_go_on() || aborted {
            self.shells_over();
        }
    }

    /// The chamber's mark in a filling with the bolt open, answered once:
    /// the round that was chambered is thrown out
    /// (`ReloadInternalMagWithOpenBoltOperation.RemoveAmmoFromChamber`).
    fn shell_thrown_out(&mut self) {
        let Operation::Shells { emptied, .. } = &mut self.operation else { return };
        if std::mem::replace(emptied, true) {
            return;
        }
        self.animator.set("AmmoInChamber", 0.0);
        self.events.push(Event::RoundEjected);
    }

    /// The filling hands over to idling (`SwitchToIdle`): the animator is
    /// told whether more would have followed, the rounds loaded are the
    /// weapon's from now, and the animation plays out.
    fn shells_over(&mut self) {
        let Operation::Shells { loaded, aborted, hide, .. } = self.operation else { return };
        let goes_on = self.shells_go_on() && !aborted && !self.settings.open_bolt_reload;
        self.animator.set_flag("CanReload", goes_on);
        self.magazine = self.magazine.map(|rounds| rounds + loaded);
        if loaded > 0 {
            self.events.push(Event::RoundsTaken(loaded));
        }
        // The bolt shut on a filled magazine takes its first round into
        // the empty chamber (`CommitReloadWithAmmo`): only if something
        // was loaded.
        if self.settings.open_bolt_reload {
            if loaded > 0 && !self.chamber {
                self.take_round();
            }
            self.animator.set("AmmoInChamber", self.chamber as u8 as f32);
        }
        self.animator.set("AmmoInMag", self.magazine.unwrap_or(0) as f32);
        self.to_idle();
        if let Some(fast) = hide {
            self.hide(fast);
        }
    }

    /// The bolt is caught open or let go (`FirearmsAnimator.SetBoltCatch`):
    /// the animator's flag, the resting pose that goes with it, and the
    /// layer that holds the bolt back.
    fn set_bolt_catch(&mut self, caught: bool) {
        self.animator.set_flag("BoltCatch", caught);
        self.animator.set("IdlePosition", caught as u8 as f32);
        self.animator.set_layer_weight("Catch", caught as u8 as f32);
    }

    /// The operation in hand, by the name of its kind, as a tool reading a
    /// recording beside the logic says it (`byhand`).
    pub fn operation_said(&self) -> String {
        let said = format!("{:?}", self.operation);
        said.split([' ', '(']).next().unwrap_or("").to_owned()
    }

    /// Whether the bolt is held open, as the animator has it.
    pub fn bolt_caught(&self) -> bool {
        self.animator.flag("BoltCatch")
    }

    /// Folds the stock, or unfolds it (`FirearmHandsInputTranslator`, the
    /// fold key; `FoldStockOperation`; shotgun spec, "The stock"). Only a
    /// weapon whose logic idles takes it, and not in a look at it; the
    /// trigger is let go. The animator plays the fold from its resting
    /// state, and at the state's own mark the stock stands the new way
    /// (`Event::Folded`) and the logic idles again while the animation
    /// plays out. Whether it began.
    pub fn fold_stock(&mut self) -> bool {
        if !self.settings.foldable {
            return false;
        }
        // A stock that folds leaves a mount at once (`FoldStock`'s command).
        self.events.push(Event::LeaveMount { at_once: true });
        if !self.is_idling() {
            return false;
        }
        // The fold is not refused in blind fire, and ends it
        // (`FoldStockOperation.Start`).
        self.ends_blind_fire();
        self.trigger_to(false);
        self.animator.set_flag("Inventory", false);
        self.animator.trigger("StockSwitch");
        self.hands_busy = true;
        self.operation = Operation::Fold { to: !self.folded, hide: None };
        true
    }

    /// Whether the thing in hand may mount (`Player.TryMountWeapon`'s part of
    /// the weapon's): a mountable firearm in hand, in no reload, draw,
    /// put-away or interaction with the inventory.
    pub fn may_mount(&self) -> bool {
        self.settings.mountable
            && !self.reloading()
            && !matches!(
                self.operation,
                Operation::Away | Operation::Spawn { .. } | Operation::Remove | Operation::DropBackpack | Operation::InventoryMagOut { .. } | Operation::InventoryMagIn { .. } | Operation::InventoryChamber | Operation::InventoryDischarge
            )
    }

    /// Mounted or not, as the mount told the weapon (`FirearmsAnimator.
    /// SetMounted`: the animator's `Mounted`, where it has one); and whether
    /// mounted forward on the bipod (the tactical reload's blend runs then).
    pub fn set_mounted(&mut self, mounted: bool, on_bipod: bool) {
        if self.animator.has("Mounted") {
            self.animator.set_flag("Mounted", mounted);
        }
        self.mount_tactical = mounted && on_bipod;
    }

    /// Whether the weapon has a bipod, and whether it is out.
    pub fn bipod(&self) -> Option<bool> {
        self.settings.bipod.map(|_| self.bipod_out)
    }

    /// The bipod's key (`FirearmController.ToggleBipod`): refused without a
    /// bipod, mounted, with the weapon stuck into something, in a sprint,
    /// with the body unable to make a gesture (`gesture_free`: its queue
    /// takes one and it may interact), with the animator not at rest, and in
    /// any operation but the idling one. Taken, the bipod is out (or in) at
    /// once, the animator's inventory flag down and its `Bipod` set where it
    /// has one, blind fire ended, and `Event::Bipod` given. Whether taken.
    pub fn toggle_bipod(&mut self, mounted: bool, sprinting: bool, gesture_free: bool) -> bool {
        let Some(forward) = self.settings.bipod else { return false };
        if mounted || self.overlap > 0.0 || sprinting || !gesture_free || !self.animator_idling() || !self.is_idling() {
            return false;
        }
        self.bipod_out = !self.bipod_out;
        self.animator.set_flag("Inventory", false);
        if self.animator.has("Bipod") {
            self.animator.set_flag("Bipod", self.bipod_out);
        }
        self.ends_blind_fire();
        self.operation = Operation::Bipod { hide: None };
        self.events.push(Event::Bipod { out: self.bipod_out, forward });
        true
    }

    /// Whether the bipod's toggle is the operation in hand.
    pub fn toggling_bipod(&self) -> bool {
        matches!(self.operation, Operation::Bipod { .. })
    }

    /// The body's gesture's mark (`OnBipodToggle`; `ToggleBipodOperation.
    /// SwitchToIdle`): the animator is told how the inventory stands, the
    /// weapon idles, and a put-away asked for meanwhile goes.
    pub fn bipod_toggled(&mut self) {
        let Operation::Bipod { hide } = self.operation else { return };
        self.animator.set_flag("Inventory", self.inventory);
        self.to_idle();
        if let Some(fast) = hide {
            self.hide(fast);
        }
    }

    /// The fold's mark (`FoldStockOperation.OnFold`, `SwitchToIdle`).
    fn folded_now(&mut self) {
        let Operation::Fold { to, hide } = self.operation else { return };
        self.folded = to;
        self.animator_fold();
        self.animator.set_flag("Inventory", self.inventory);
        self.to_idle();
        self.events.push(Event::Folded(to));
        if let Some(fast) = hide {
            self.hide(fast);
        }
    }

    /// The next fire mode. The idling operation does it itself: the mode
    /// changes at once and the animator plays the switch when it is next
    /// at rest, so the trigger and the reload key work through it.
    /// The fire mode key (`ChangeWeaponMode`: `GetNextFireMode`, which on a
    /// weapon of one way of firing is that one): the screen names the mode
    /// whether it changed or not.
    pub fn next_fire_mode(&mut self) {
        let count = self.settings.modes.len();
        if count > 0 {
            let next = self.settings.modes[(self.mode + 1) % count];
            self.change_fire_mode_shown(next);
        }
    }

    /// `Idling.ChangeFireMode` from a key: refused as `set_fire_mode` refuses
    /// it, the bolt held open among it; else the screen names the mode
    /// (`ShowFireMode`), changed or not, and it is set. Whether it changed.
    fn change_fire_mode_shown(&mut self, mode: FireMode) -> bool {
        // Refused in blind fire before anything is shown
        // (`FirearmController.ChangeFireMode`).
        if self.blind_fire {
            return false;
        }
        let idling = matches!(self.operation, Operation::Idle | Operation::Utility { .. });
        if !idling || !self.settings.modes.contains(&mode) || (self.settings.bolt_catch && self.settings.no_fire_mode_on_bolt_catch && self.bolt_caught()) {
            return false;
        }
        self.events.push(Event::FireModeShown(mode));
        self.set_fire_mode(mode)
    }

    /// Changes to one of the weapon's fire modes (`Idling.ChangeFireMode`).
    /// Only the idling operation answers (and a look at the weapon, which
    /// is one), and **not while the bolt is held open** on a weapon that
    /// says so (`NoFiremodeOnBoltcatch`). Whether it changed.
    pub fn set_fire_mode(&mut self, mode: FireMode) -> bool {
        if self.blind_fire {
            return false;
        }
        let idling = matches!(self.operation, Operation::Idle | Operation::Utility { .. });
        let Some(index) = self.settings.modes.iter().position(|has| *has == mode).filter(|_| idling) else { return false };
        if self.settings.bolt_catch && self.settings.no_fire_mode_on_bolt_catch && self.bolt_caught() {
            return false;
        }
        if index == self.mode {
            return false;
        }
        self.mode = index;
        self.events.push(Event::FireMode(self.mode()));
        self.animator.set("FireMode", mode_number(self.mode()));
        self.animator.trigger("FiremodeSwitch");
        self.cylinder_mode_changed();
        // A weapon whose animator has a sprinting variant of the switch
        // has the body told the hands are busy.
        if self.animator.has("FiremodeSprint") {
            self.hands_busy = true;
        }
        true
    }

    /// Looks into the chamber (`Idling.CheckChamber`): not with the
    /// trigger pressed. The sights go down for it and the aim key does
    /// nothing until it is over; which state plays is the animator's
    /// choice by the fire mode and the bolt.
    pub fn check_chamber(&mut self) {
        self.events.push(Event::LeaveMount { at_once: false });
        // With a malfunction, its repair where its kind is known, else the
        // reminder (`Idling.CheckChamber`).
        if self.malfunction_check_chamber() {
            return;
        }
        if self.is_idling() && !self.trigger && !self.blind_fire {
            self.aiming = false;
            self.animator.trigger("CheckChamber");
            self.hands_busy = true;
            self.operation = Operation::Utility { ready: false, time: 0.0, in_inventory: false, examine: false };
        }
    }

    /// Looks at the selector (`Idling.CheckFireMode`): only a weapon with
    /// more than one way of firing, and only at rest. The sights go down
    /// for it. Whether the look began.
    pub fn check_fire_mode(&mut self) -> bool {
        self.events.push(Event::LeaveMount { at_once: false });
        if !self.is_idling() || self.blind_fire {
            return false;
        }
        // The screen names the mode first, a weapon of one way of firing too
        // (`Idling.CheckFireMode`'s first line).
        self.events.push(Event::FireModeShown(self.mode()));
        if self.settings.modes.len() <= 1 {
            return false;
        }
        self.aiming = false;
        self.animator.trigger("FiremodeCheck");
        self.hands_busy = true;
        self.operation = Operation::Utility { ready: false, time: 0.0, in_inventory: false, examine: false };
        true
    }

    /// Looks the weapon over (`Idling.ExamineWeapon`): only at rest, and
    /// **not with the sights up**, which it does not put down. The
    /// trigger ends it (`trigger_changed`). Whether the look began.
    pub fn examine(&mut self) -> bool {
        // With a malfunction, the look at it (`Idling.ExamineWeapon`: the
        // shooter knows of it from the shot), which puts the sights down.
        if self.malfunction() != Malf::None {
            return self.examine_malfunction();
        }
        if !self.is_idling() || self.aiming || self.blind_fire {
            return false;
        }
        // Begun, the look leaves a mount (`FirearmController.ExamineWeapon`).
        self.events.push(Event::LeaveMount { at_once: false });
        self.hands_busy = true;
        self.operation = Operation::Utility { ready: false, time: 0.0, in_inventory: false, examine: true };
        self.animator.trigger("Look");
        true
    }

    /// The magnification key's sign to the hands (`FirearmsAnimator.
    /// ModToggleTrigger`, which every operation gives with a change of a
    /// scope's mode: `SetScopeMode`): the animator's `ModSwitch`, where it
    /// has one (optics spec, "Zoom and modes").
    pub fn mod_switch(&mut self) {
        self.animator.trigger("ModSwitch");
    }

    /// Whether the operation in hand lets a tactical device be switched
    /// (`CanChangeLightState`, overridden to refuse only by the draw, the
    /// put-away, a part fitted or taken off, a magazine put in or pulled
    /// out through the inventory, the backpack dropped and the bipod;
    /// tactical-devices spec, "The keys"): at rest, aimed, in any fire, in
    /// a reload, a look or a fold it may. A part changes here only at
    /// rest, by a new build (the app), and there is no bipod.
    pub fn can_change_lights(&self) -> bool {
        !matches!(
            self.operation,
            Operation::Away | Operation::Spawn { .. } | Operation::Remove | Operation::InventoryMagOut { .. } | Operation::InventoryMagIn { .. } | Operation::DropBackpack
        )
    }

    /// Puts the weapon on its fullest way of firing
    /// (`FireModeComponent.GetForceAutoFireMode`): automatic if it has it,
    /// else bursts; a weapon with one way of firing, or with neither of
    /// those, stays as it is. The change itself is `set_fire_mode`'s.
    pub fn force_auto_fire_mode(&mut self) -> bool {
        if self.settings.modes.len() <= 1 {
            return false;
        }
        // (`GetForceAutoFireMode`: automatic, bursts, both barrels, double
        // action, in that order; only where it is not the mode already.)
        let fullest = [FireMode::FullAuto, FireMode::Burst, FireMode::Doublet, FireMode::DoubleAction].into_iter().find(|mode| self.settings.modes.contains(mode));
        fullest.filter(|mode| *mode != self.mode()).is_some_and(|mode| self.change_fire_mode_shown(mode))
    }

    /// The backpack is to be dropped with this in hand (the idling
    /// operation's `Execute` of a thing leaving a slot that is animated,
    /// `DropBackpackOperation.Start`): **only at rest**; in anything else
    /// the game refuses the drop. The trigger's state is taken off the
    /// animator, the hands are told the inventory is shut, the sights go
    /// down. `backpack_dropped` ends it. Whether it began.
    pub fn drop_backpack(&mut self) -> bool {
        if !self.is_idling() {
            return false;
        }
        // It ends blind fire (`DropBackpackOperation.Start`).
        self.ends_blind_fire();
        self.animator.set_flag("Fire", false);
        self.animator.set_flag("Inventory", false);
        self.aiming = false;
        self.operation = Operation::DropBackpack;
        true
    }

    /// The pack is off the shoulders (`OnBackpackDropEvent`): the
    /// animator is told how the inventory stands, and the weapon idles.
    pub fn backpack_dropped(&mut self) {
        if self.operation == Operation::DropBackpack {
            self.animator.set_flag("Inventory", self.inventory);
            self.to_idle();
        }
    }

    /// Whether a backpack's drop is in hand.
    pub fn dropping_backpack(&self) -> bool {
        self.operation == Operation::DropBackpack
    }

    /// Looks at the magazine. The sights go down for it and the aim key
    /// does nothing until it is over. Whether it began (the battle screen
    /// then names the count: `Event::MagazineChecked`).
    pub fn check_magazine(&mut self) -> bool {
        self.events.push(Event::LeaveMount { at_once: false });
        // A failure to feed allows no look at the magazine (`Idling.CheckAmmo`).
        if self.malfunction() == Malf::Feed {
            self.remind();
            return false;
        }
        if self.is_idling() && self.magazine.is_some() && !self.blind_fire {
            self.events.push(Event::MagazineChecked);
            self.aiming = false;
            self.animator.trigger("CheckAmmo");
            self.hands_busy = true;
            self.operation = Operation::Utility { ready: false, time: 0.0, in_inventory: false, examine: false };
            true
        } else {
            false
        }
    }

    /// Whether the hands are busy with the weapon: through a reload, to
    /// the end of its animation, through a look at the weapon, and while
    /// it is drawn and put away. The body's animator is told
    /// (`movement::Input::hands_busy`, the game's `ReloadFloat`): in a
    /// sprint it brings the weapon up into view for it. The game sets the
    /// same for a fire-mode switch on weapons whose animator has a
    /// sprinting variant of it; the AKM's has none.
    pub fn hands_busy(&self) -> bool {
        self.hands_busy
    }

    /// Whether the weapon's logic has in hand something the game does not
    /// count as idling: a shot or automatic fire, a reload up to where it
    /// hands over, a look at the weapon, a draw, a put-away. Switching the
    /// fire mode is the idling operation's own work. The movement is told
    /// (`movement::Input::weapon_occupied`): with the weapon so taken up,
    /// only an obstacle low enough to leave the hands on it is vaulted or
    /// climbed (vaulting spec, "What a try does").
    pub fn occupied(&self) -> bool {
        !matches!(self.operation, Operation::Idle | Operation::Away)
    }

    /// Whether a look at the weapon is in hand.
    pub fn checking(&self) -> bool {
        matches!(self.operation, Operation::Utility { .. })
    }

    /// The fire state asked of the animator outright, part-way in, with
    /// the trigger as the controller has it: the self-loading mode has a
    /// state of its own (`FireOperation.StartFireAnimation`).
    fn animate_fire(&mut self) {
        self.animator.set_flag("Fire", self.trigger);
        let state = if self.mode() == FireMode::SemiAuto { &self.settings.semi_fire_state } else { &self.settings.fire_state };
        self.animator.play(state, FIRE_STATE_START);
    }

    /// A single shot begins: its rounds are settled now, and the shot
    /// itself is made when the fire state's mark reaches the operation.
    fn start_single(&mut self) {
        // The roll, as the operation begins (`FireOperation.PrepareShot`): a
        // misfire is no shot at all; any other malfunction lets the fire
        // state come by the animator's own way in (`StartFireAnimation`
        // plays it only with none), its shot made at its mark.
        let malf = self.roll_shot();
        if malf == Malf::Misfire {
            return self.misfire_single();
        }
        self.take_round_malfunctioned(malf);
        self.round_out = true;
        if malf == Malf::None {
            self.animate_fire();
        } else {
            self.animator.set_flag("Fire", self.trigger);
        }
        // A bolt action holds its bolt for as long as the trigger is held
        // (`BoltActionFireOperation.Start`).
        if self.settings.bolt_action {
            self.animator.set_flag("BoltActionReload", !self.trigger);
        }
        self.operation = Operation::Single {
            cooldown: 60.0 / self.settings.single_fire_rate,
            fired: false,
            ready: false,
            second_shot: false,
            open_inventory: false,
            planned: None,
            malf,
        };
    }

    fn start_automatic(&mut self) {
        self.events.push(Event::AutoFire(true));
        // The heat's factor is applied afresh each burst (`float_6`).
        self.burst_rate = 0.0;
        self.animate_fire();
        // The clock does not start at zero, so that the first step does
        // not count as the start of a second cycle.
        self.operation = Operation::Automatic { clock: 0.0001, shots: 0, running: false, open_inventory: false, planned: None };
        self.fire_round();
    }

    /// The trigger as the controller has it, set; let go, the weapon's
    /// sound player is told (`Event::TriggerReleased`).
    fn trigger_to(&mut self, pressed: bool) {
        if self.trigger && !pressed {
            self.events.push(Event::TriggerReleased);
        }
        self.trigger = pressed;
    }

    /// Takes the chambered round and feeds the next from the magazine; a
    /// weapon with no chamber takes the round off its magazine
    /// (`GetAmmoForSingleShot`: `PopToNowhere`).
    fn take_round(&mut self) {
        if self.settings.no_chamber {
            if let Some(rounds) = self.magazine.as_mut() {
                *rounds = rounds.saturating_sub(1);
            }
            return;
        }
        self.chamber = match &mut self.magazine {
            Some(rounds) if *rounds > 0 => {
                *rounds -= 1;
                true
            }
            _ => false,
        };
    }

    /// Fires the chambered round and feeds the next from the magazine, in
    /// one: an automatic shot.
    fn fire_round(&mut self) {
        // The roll, each shot of the run (`AutomaticFireOperation.InternalOnFireEvent`):
        // a misfire makes no shot; any other malfunction's shot is made and
        // the run is over.
        let malf = self.roll_shot();
        if malf == Malf::Misfire {
            self.take_round_malfunctioned(malf);
            return self.switch_to_malfunction(malf, true);
        }
        self.take_round_malfunctioned(malf);
        self.round_out = true;
        self.events.push(Event::Shot { strength: self.settings.shot_strength });
        // A malfunctioned shot plays its tail at once (`FireBullet`'s
        // `malfunctioned`), as a trigger let go.
        let trigger = self.trigger && malf == Malf::None;
        self.events.push(Event::FireSound { trigger, doublet: false });
        if let Operation::Automatic { shots, .. } = &mut self.operation {
            *shots += 1;
        }
        self.shot_worn();
        if malf != Malf::None {
            return self.switch_to_malfunction(malf, true);
        }
        self.animator.set_flag("Fire", self.trigger);
    }

    /// The idling operation takes over (`Idling.Start`).
    fn to_idle(&mut self) {
        self.operation = Operation::Idle;
        self.round_out = false;
        self.animator_values();
    }

    /// One frame of `dt` seconds, in the game's order: the operation's own
    /// update, the engine's update of the animator, and at the frame's end
    /// the marks that update passed, each to the operation then in hand.
    /// Returns what happened.
    pub fn step(&mut self, dt: f32) -> Vec<Event> {
        self.step_on_clock(dt, dt)
    }

    /// `step`, the operation's own update on the player's clock
    /// (`player_dt`: the game's arms are updated with the player's
    /// whole-millisecond frame time, `Player.ArmsUpdate`) and the animator
    /// on the engine's (`dt`). The single shot's cooldown, the automatic
    /// fire's cycle and a look's wait go by the first (recorded: the
    /// Five-seveN's cooldown of 0.133 s ran out after 16 frames or 17 as
    /// the frames' whole milliseconds came to 133.3 or not, 2026-10-16;
    /// with it the scripted recordings' automatic fire has the game's round
    /// count on every frame, where a shot of each had come a frame off).
    pub fn step_on_clock(&mut self, dt: f32, player_dt: f32) -> Vec<Event> {
        // An operation the host began since the last frame.
        self.note_operation();
        // The animator hears of the sights whenever they have changed.
        if self.aiming != self.aim_told {
            self.tell_aim();
        }
        // A reload made with the sights up stiffens the weapon, and lets
        // it go a moment after either ends.
        self.clock += dt;
        if (self.aiming && self.reloading()) || self.mount_tactical {
            self.tactical.aim_for(1.0, 4.0, 0.0, self.clock);
        } else {
            self.tactical.aim_for(0.0, 2.0, 0.2, self.clock);
        }
        let heat_rate = self.heat_rate().filter(|mult| (mult - self.burst_rate).abs() > f32::EPSILON);
        // The heat's stray shot, at rest.
        if matches!(self.operation, Operation::Idle) {
            self.autoshot();
        }
        match &mut self.operation {
            Operation::Single { cooldown, ready, .. } => {
                *cooldown -= player_dt;
                if *cooldown <= 0.0 && *ready {
                    self.end_single();
                }
            }
            Operation::Automatic { running: false, .. } => {}
            Operation::Automatic { clock, .. } => {
                // The heat's factor, where a new one is drawn: the interval
                // changed with the phase kept (`AutomaticFireOperation.Update`).
                if let Some(mult) = heat_rate {
                    let phase = *clock % self.auto_cycle / self.auto_cycle;
                    self.auto_cycle = 60.0 / (self.settings.fire_rate * mult);
                    *clock = self.auto_cycle * (phase % 1.0);
                    self.burst_rate = mult;
                }
                let cycle = self.auto_cycle;
                let start = *clock % cycle / cycle;
                let end = start + player_dt / cycle;
                *clock += player_dt;
                let mut spans = vec![(start, end.min(1.0))];
                let mut rest = end - 1.0;
                while rest > 0.0 {
                    spans.push((0.0, rest.min(1.0)));
                    rest -= 1.0;
                }
                for (from, to) in spans {
                    self.automatic_span(from, to);
                }
            }
            // A look whose state never gives its first mark is given up.
            Operation::Utility { ready: false, time, in_inventory: false, examine } => {
                if *time > MAX_UTILITY_TIME {
                    self.operation = Operation::Utility { ready: true, time: 0.0, in_inventory: false, examine: *examine };
                    self.idle_started();
                } else {
                    *time += player_dt;
                }
            }
            _ => {}
        }
        // What is drawn is on curves without corners where the host asks
        // (`set_drawn_smooth`), every frame: an animator made afresh for a
        // draw starts as the engine has it.
        self.animator.set_drawn_smooth(self.drawn_smooth);
        self.animator.update(dt);
        // The frame's end: the recoil is given a shot the last frame's
        // marks made, and then this frame's marks are delivered.
        if std::mem::take(&mut self.shot_made) {
            self.events.push(Event::Shot { strength: self.settings.shot_strength });
        }
        // One at a time, each tested against the parameters as the one
        // before it left them (`AnimationEventsEmitter.EmitEvents`).
        let mut marks = Vec::new();
        while let Some(mark) = self.animator.deliver_next() {
            self.on_mark(&mark);
            marks.push(mark);
        }
        self.marks = marks;
        self.note_operation();
        std::mem::take(&mut self.events)
    }

    /// What the operation in hand does to the left shoulder as it begins
    /// (`SetLeftStanceAnimOnStartOperation`): the rest and the draw put its
    /// animation back, the fire (`FireOperationBase`'s) leaves it, every
    /// other takes it off (`FirearmOperation`'s). No weapon of the install
    /// refuses it (`BlockLeftStance`), and none is stationary.
    fn left_stance_start(&self) -> Option<crate::left_stance::HandsStart> {
        use crate::left_stance::HandsStart;
        match self.operation {
            Operation::Idle => Some(HandsStart::Idle),
            Operation::Spawn { .. } => Some(HandsStart::Draw { blocked: false }),
            Operation::Single { .. } | Operation::Automatic { .. } | Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } => None,
            // `MalfunctionOperation`'s own start leaves it too.
            Operation::Malfunction { .. } => None,
            _ => Some(HandsStart::Other),
        }
    }

    /// An operation begun since the left shoulder was last told: its start
    /// told now. (The game calls it from each operation's `Start`; a change
    /// of kind is that here, the operations' own changes of their fields
    /// not being starts. `PROVISIONAL`: looked at as a frame begins and
    /// ends, so one begun and over between the two is not told.)
    fn note_operation(&mut self) {
        let kind = std::mem::discriminant(&self.operation);
        if self.left_stance_seen != Some(kind) {
            self.left_stance_seen = Some(kind);
            if let Some(start) = self.left_stance_start() {
                self.events.push(Event::LeftStance(crate::left_stance::Call::Start(start)));
            }
        }
    }

    /// Whether the operation in hand passes the left shoulder's key on
    /// (`ToggleLeftStance`: `Idling`'s, and so the looks', which are built
    /// on it; every other drops it). Lying down is the movement's to refuse.
    pub fn may_toggle_left_stance(&self) -> bool {
        matches!(self.operation, Operation::Idle | Operation::Utility { .. })
    }

    /// One of the animator's marks reaches the operation in hand.
    fn on_mark(&mut self, mark: &Mark) {
        // A malfunction's repair answers its own marks (`RepairMalfunction`).
        if self.repair_mark(&mark.name) {
            return;
        }
        // Chambers of their own answer their operations' marks first.
        if matches!(self.operation, Operation::CylinderFire { .. } | Operation::BarrelsFire { .. } | Operation::ChambersReload { .. }) {
            let answered = if self.settings.cylinder.is_some() { self.cylinder_mark(&mark.name) } else { self.barrels_mark(&mark.name) };
            if answered {
                return;
            }
        }
        let misfired = self.malfunction() == Malf::Misfire;
        match (mark.name.as_str(), &mut self.operation) {
            ("Arm", _) => {
                self.armed = true;
                self.animator.set_flag("Armed", true);
                self.animator.set_layer_weight("Hammer", 0.0);
            }
            ("Disarm", _) => {
                self.armed = false;
                self.animator.set_flag("Armed", false);
                self.animator.set_layer_weight("Hammer", 1.0);
            }
            // A rocket launcher's sights up and down as its own clips have
            // them (`FirearmController.OnAimReady`, `OnIdleReady`): what
            // its resting state's way to firing waits for.
            ("AimReady", _) if self.settings.rocket_launcher => self.animator.set("IsAimingFloat", 1.0),
            ("IdleReady", _) if self.settings.rocket_launcher => self.animator.set("IsAimingFloat", 0.0),
            ("Sound", _) => self.events.push(Event::Sound(mark.text.clone())),
            ("ThirdAction", _) => self.events.push(Event::BodyAction(mark.int)),
            ("IdleStart", _) => self.idle_started(),
            ("WeapIn", Operation::Spawn { .. }) => self.appeared(),
            ("WeapOut", Operation::Remove) => {
                self.operation = Operation::Away;
                self.events.push(Event::Hidden);
            }
            // The shot itself, a frame after its round was taken.
            ("FiringBullet", Operation::Single { fired, malf, .. }) => {
                *fired = true;
                let malf = *malf;
                // Its sound with the trigger still held, then let go
                // (`FireOperation.OnFireEvent`).
                self.events.push(Event::FireSound { trigger: self.trigger, doublet: false });
                self.shot_made = true;
                self.trigger_to(false);
                self.animator.set_flag("Fire", false);
                self.shot_worn();
                if malf != Malf::None {
                    self.switch_to_malfunction(malf, false);
                }
            }
            // A malfunction's state: its case out (a failure to feed's), the
            // chamber shown empty, and at its mark the weapon at rest with it.
            ("ShellEject", Operation::Malfunction { .. }) => self.events.push(Event::ShellEject),
            ("DelAmmoChamber", Operation::Malfunction { .. }) => self.animator.set("AmmoInChamber", 0.0),
            ("MalfunctionOff", Operation::Malfunction { .. }) => self.malfunction_held(),
            ("MalfunctionOff", Operation::ExamineMalfunction) => self.malfunction_examined(),
            ("FiringBullet", Operation::Automatic { running, .. }) => *running = true,
            ("ShellEject", Operation::Single { .. }) => self.events.push(Event::ShellEject),
            ("DelAmmoChamber", Operation::Single { .. }) => self.animator.set("AmmoInChamber", 0.0),
            ("AddAmmoInChamber", Operation::Single { .. }) => {
                self.round_out = false;
                self.animator_rounds();
                self.animator.set_flag("Fire", self.trigger);
            }
            ("FireEnd", Operation::Single { .. }) => self.trigger_to(false),
            // A draw that racked the bolt.
            ("AddAmmoInChamber", Operation::Spawn { arms: true, .. }) => self.animator_rounds(),
            ("MagOut", Operation::Reload { mag_out, .. }) if !*mag_out => {
                *mag_out = true;
                self.magazine = None;
                self.events.push(Event::MagazineOut);
                self.animator.set_flag("Inventory", self.inventory);
                self.animator.set_flag("MagInWeapon", false);
                self.animator.set("MagInWeaponFloat", 0.0);
            }
            ("MagShow", Operation::Reload { mag_shown, .. }) if !*mag_shown => {
                *mag_shown = true;
                self.events.push(Event::MagazineShown);
                self.animator.set_flag("Inventory", self.inventory);
            }
            ("MagHide", Operation::Reload { mag_hidden, .. }) if !*mag_hidden => {
                *mag_hidden = true;
                self.events.push(Event::MagazineHidden);
            }
            // The bolt's catch in a magazine's reload: caught whenever the
            // mark says so, let go only the first time
            // (`ReloadExternalMagOperation.OnOnOffBoltCatchEvent`).
            ("OnBoltCatch", Operation::Reload { .. }) => self.set_bolt_catch(true),
            ("OffBoltCatch", Operation::Reload { catch_off, .. }) => {
                if !std::mem::replace(catch_off, true) {
                    self.set_bolt_catch(false);
                }
            }
            ("FoldOn" | "FoldOff", Operation::Fold { .. }) => self.folded_now(),
            // A magazine taken out from the inventory: out of the weapon at
            // its mark, put away at the hand's (`PullOutMagOperation.
            // OnMagPulledOutFromWeapon`, `OnMagPuttedToRig`).
            ("MagOut", Operation::InventoryMagOut { pulled }) if !*pulled => {
                *pulled = true;
                self.magazine = None;
                self.events.push(Event::MagazineOut);
                self.animator.set("AmmoInMag", 0.0);
                self.animator.set_flag("MagInWeapon", false);
                self.animator.set("MagInWeaponFloat", 0.0);
            }
            ("MagHide", Operation::InventoryMagOut { pulled: true }) => {
                self.events.push(Event::MagazineHidden);
                self.inventory_move_over();
            }
            // The chambered round taken out from the inventory, put away
            // (`DischargeOperation.OnMagPuttedToRig`).
            ("MagHide", Operation::InventoryDischarge) => {
                self.animator.set_flag("Discharge", false);
                self.chamber = false;
                self.animator_rounds();
                self.animator.set_flag("CanReload", false);
                self.events.push(Event::Discharged);
                self.to_idle();
            }
            // One put in from the inventory (`InsertMagOperation.
            // OnMagAppeared`, `OnMagInsertedToWeapon`, `OnAddAmmoInChamber`).
            ("MagShow", Operation::InventoryMagIn { shown, .. }) if !*shown => {
                *shown = true;
                self.events.push(Event::MagazineShown);
            }
            ("MagIn", Operation::InventoryMagIn { inserted, .. }) if !*inserted => {
                *inserted = true;
                self.magazine = Some(self.incoming);
                self.mag_variant = self.incoming_variant;
                self.events.push(Event::MagazineIn);
                self.animator.set("AmmoInMag", self.incoming as f32);
                self.animator.set_flag("MagInWeapon", true);
                self.animator.set("MagInWeaponFloat", 1.0);
                if !self.chambers {
                    self.inventory_move_over();
                }
            }
            // A round put into the chamber from the inventory
            // (`LoadCartridgeToChamber.OnAddAmmoInChamber`).
            ("AddAmmoInChamber", Operation::InventoryChamber) => {
                self.chamber = true;
                self.animator_rounds();
                self.animator.set_flag("CanReload", false);
                self.animator.set_flag("LoadOne", false);
                self.events.push(Event::Chambered);
                self.to_idle();
            }
            ("AddAmmoInChamber", Operation::InventoryMagIn { inserted: true, .. }) if self.chambers => {
                self.chambers = false;
                self.take_round();
                self.animator_rounds();
                self.inventory_move_over();
            }
            ("MagIn", Operation::Reload { mag_in, .. }) if !*mag_in => {
                *mag_in = true;
                self.magazine = Some(self.incoming);
                self.events.push(Event::MagazineIn);
                self.animator.set_flag("MagInWeapon", true);
                self.animator.set("MagInWeaponFloat", 1.0);
                self.animator.set("AmmoInMag", self.incoming as f32);
                // With nothing to chamber the reload is over here; a weapon
                // with no chamber's goes on to the animation's rest
                // (`ReloadExternalMagResult`: only with chambers).
                if !self.chambers && !self.settings.no_chamber {
                    self.reload_over();
                }
                // (`OnMagInsertedToWeapon`: the new one's.)
                self.mag_variant = self.incoming_variant;
                self.animator.set("RelTypeOld", self.mag_variant as f32);
            }
            ("AddAmmoInChamber", Operation::Reload { .. }) if self.chambers && misfired => {
                // A reload's chambering puts a misfire right (`ReloadExternalMagOperation.OnAddAmmoInChamber`).
                self.clear_misfire();
                self.chambers = false;
                self.take_round();
                self.animator_rounds();
            }
            ("AddAmmoInChamber", Operation::Reload { catch_off, .. }) if self.chambers => {
                let let_go = !std::mem::replace(catch_off, true);
                self.chambers = false;
                self.take_round();
                self.animator_rounds();
                // The bolt goes home on the round: its catch is let go
                // here if no mark has done it.
                if let_go {
                    self.set_bolt_catch(false);
                }
                self.reload_over();
            }
            ("StartUtilityOperation", Operation::Utility { ready, .. }) => *ready = true,
            // A tube being filled. (With the bolt opened for it the round
            // shown does not yet let it be cut short, and the chamber's
            // mark throws the old round out.)
            ("MagShow", Operation::Shells { can_abort, .. }) if !self.settings.open_bolt_reload => *can_abort = true,
            ("DelAmmoChamber", Operation::Shells { .. }) if self.settings.open_bolt_reload => self.shell_thrown_out(),
            ("AddAmmoInChamber", Operation::Shells { chambered: false, .. }) => self.shell_chambered(),
            ("AddAmmoInMag", Operation::Shells { .. }) => self.shell_in(),
            // The bolt caught open on the last round and let go again: a
            // shot answers only on a weapon that has a catch, a weapon at
            // rest, a look at it and a tube being filled always.
            ("OnBoltCatch" | "OffBoltCatch", Operation::Single { .. } | Operation::Automatic { .. }) if self.settings.bolt_catch => {
                self.set_bolt_catch(mark.name == "OnBoltCatch")
            }
            ("OnBoltCatch" | "OffBoltCatch", Operation::Idle | Operation::Utility { .. } | Operation::Shells { .. }) => {
                self.set_bolt_catch(mark.name == "OnBoltCatch")
            }
            _ => {}
        }
    }

    /// The mark at the very start of the animator's resting state.
    fn idle_started(&mut self) {
        // The resting state begun: the sights' news is old
        // (`Idling.OnIdleStartEvent`).
        self.aim_flags = false;
        match self.operation {
            // The weapon is back at rest: the body is told the hands are
            // free, and the trigger is heard again after a reload; and the
            // left shoulder's animation put back (`Idling.OnIdleStartEvent`).
            Operation::Idle => {
                (self.hands_busy, self.reload_flag) = (false, false);
                self.events.push(Event::LeftStance(crate::left_stance::Call::Start(crate::left_stance::HandsStart::Idle)));
            }
            Operation::Single { fired: true, ref mut ready, .. } => *ready = true,
            Operation::Reload { mag_shown: true, .. } => {
                self.reload_flag = false;
                self.reload_over();
            }
            Operation::Utility { ready: true, .. } => {
                (self.hands_busy, self.reload_flag) = (false, false);
                self.to_idle();
            }
            Operation::Spawn { .. } => self.appeared(),
            _ => {}
        }
    }

    /// The draw is over: the weapon idles, and is put away again at once
    /// if that was asked for meanwhile.
    fn appeared(&mut self) {
        let Operation::Spawn { hide, .. } = self.operation else { return };
        // A misfire is put right by the draw (`SpawnOperation.WeaponAppeared`).
        self.clear_misfire();
        self.to_idle();
        self.events.push(Event::Appeared);
        if let Some(fast) = hide {
            self.hide(fast);
        }
        self.hands_busy = false;
    }

    /// The game's reload hands over to its idling: the animation plays on.
    fn reload_over(&mut self) {
        let Operation::Reload { hide, .. } = self.operation else { return };
        self.to_idle();
        if let Some(fast) = hide {
            self.hide(fast);
        }
    }

    /// A single shot's wait is out and the weapon back at rest.
    fn end_single(&mut self) {
        let Operation::Single { second_shot, open_inventory, planned, .. } = self.operation else { return };
        self.animator.set_flag("Fire", self.trigger);
        self.animator_rounds();
        match planned {
            Some(planned) => {
                self.aiming = false;
                self.run(planned);
            }
            None => {
                self.animator.set_flag("Inventory", open_inventory);
                self.to_idle();
                // A queued shot is a press and a release.
                if second_shot && !open_inventory {
                    self.idle_trigger(true);
                    self.trigger_changed(false);
                }
            }
        }
    }

    /// What a shot was asked to hand over to.
    fn run(&mut self, planned: Planned) {
        match planned {
            Planned::Reload(rounds, quick) => self.begin_reload(rounds, quick),
            Planned::Shells(within_reach) => self.begin_shells(within_reach),
            Planned::Hide(fast) => self.start_remove(fast),
        }
    }

    /// The part of one automatic cycle between two fractions of it.
    fn automatic_span(&mut self, from: f32, to: f32) {
        let crosses = |mark: f32| from <= mark && mark < to;
        if !self.auto_fire() {
            return;
        }
        if crosses(0.0) {
            self.fire_round();
        }
        if crosses(AUTO_SHELL_EJECT) {
            self.events.push(Event::ShellEject);
        }
        // The round fed at the shot shows in the chamber at AUTO_CHAMBER;
        // nothing in the logic waits for it, and the animator is told. A
        // weapon with a catch that has fired its last round has its bolt
        // caught there.
        if crosses(AUTO_CHAMBER) {
            let s = &self.settings;
            if s.bolt_catch && !s.manual_bolt_catch && !self.chamber && self.magazine == Some(0) {
                self.set_bolt_catch(true);
            }
            self.round_out = false;
            self.animator_rounds();
            self.animator.set_flag("Fire", self.trigger);
        }
        if crosses(AUTO_CYCLE_END) {
            self.automatic_cycle_end();
        }
    }

    fn automatic_cycle_end(&mut self) {
        let Operation::Automatic { shots, open_inventory, planned, .. } = self.operation else { return };
        self.animator_rounds();
        if self.mode() == FireMode::Burst {
            if shots < self.settings.burst_count && self.chamber {
                self.trigger_to(true);
                return;
            }
            self.trigger_to(false);
        }
        if let Some(planned) = planned {
            self.trigger_to(false);
            self.aiming = false;
            self.animator.set_flag("Fire", false);
            self.animator.play(&self.settings.rest_state, REST_AFTER_AUTO);
            self.events.push(Event::AutoFire(false));
            self.run(planned);
            return;
        }
        // An open inventory ends the run at the cycle's end, trigger or no.
        if self.trigger && !open_inventory {
            // Held on while a round is left: in the chamber, or for a
            // weapon with no chamber in its magazine.
            if self.loaded() {
                return;
            }
            self.automatic_to_idle(open_inventory);
            // Still holding on an empty weapon with no bolt catch: the
            // hammer falls once more.
            if !self.settings.bolt_catch {
                self.trigger_changed(true);
            }
            return;
        }
        self.automatic_to_idle(open_inventory);
    }

    /// Handing back to idling, the animator is asked for its resting
    /// state outright, a tenth in.
    fn automatic_to_idle(&mut self, open_inventory: bool) {
        self.trigger_to(false);
        self.animator.set_flag("Inventory", open_inventory);
        self.animator.set_flag("Fire", false);
        self.animator.play(&self.settings.rest_state, REST_AFTER_AUTO);
        self.events.push(Event::AutoFire(false));
        self.to_idle();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::animator::{Condition, Controller, LayerDef, MachineDef, StateDef, Target, TransitionDef};
    use crate::marks::MarkTable;
    use trp_data::states::{Parameter, ParameterKind};

    const PARAMETERS: [(&str, ParameterKind, f32); 16] = [
        ("ReloadFast", ParameterKind::Trigger, 0.0),
        ("Active", ParameterKind::Bool, 0.0),
        ("Inventory", ParameterKind::Bool, 0.0),
        ("Fire", ParameterKind::Bool, 0.0),
        ("AmmoInChamber", ParameterKind::Float, 0.0),
        ("AmmoInMag", ParameterKind::Float, 0.0),
        ("MagInWeapon", ParameterKind::Bool, 1.0),
        ("CanReload", ParameterKind::Bool, 0.0),
        ("Reload", ParameterKind::Trigger, 0.0),
        ("FireMode", ParameterKind::Float, 0.0),
        ("FiremodeSwitch", ParameterKind::Trigger, 0.0),
        ("CheckAmmo", ParameterKind::Trigger, 0.0),
        ("Armed", ParameterKind::Bool, 1.0),
        ("FastHide", ParameterKind::Bool, 0.0),
        ("RelTypeOld", ParameterKind::Float, 0.0),
        ("RelTypeNew", ParameterKind::Float, 0.0),
    ];

    /// The states of a rifle's animator, cut down to what the logic goes
    /// by, with round numbers: each with its length in seconds, its marks
    /// (as fractions of it) and its ways out. A way out is written as the
    /// state it leads to, the point it waits for (a fraction of the state,
    /// below zero for none), the blend (a fraction of the state) and its
    /// conditions, each a parameter with `!` for "not set", `>0` or `<1`.
    type Way = (&'static str, f32, f32, &'static [&'static str]);
    const STATES: [(&str, f32, &[(&str, f32)], &[Way]); 19] = [
        ("SPAWN", 0.33, &[], &[("OUT TO IDLE", -1.0, 0.0, &["Active"])]),
        ("OUT TO IDLE", 0.967, &[("WeapIn", 0.99)], &[("IDLE", 0.98, 0.02, &["!Inventory"]), ("OPEN INVENTORY", 0.78, 0.22, &["Inventory"])]),
        (
            "IDLE",
            0.6,
            &[("IdleStart", 0.0)],
            &[
                ("OPEN INVENTORY", -1.0, 0.05, &["Inventory"]),
                ("RELOAD OUT ALL", -1.0, 0.1, &["ReloadFast", "MagInWeapon", "CanReload"]),
                ("IDLE TO OUT", -1.0, 0.05, &["!Active"]),
                ("RELOAD START", -1.0, 0.0, &["CanReload", "Reload", "MagInWeapon"]),
                ("DRY FIRE", -1.0, 0.0, &["Armed", "AmmoInChamber<1", "Fire"]),
                ("FIREMODE", -1.0, 0.0, &["FiremodeSwitch"]),
                ("FIRE", -1.0, 0.0, &["Fire", "AmmoInChamber>0"]),
                ("CHECK", -1.0, 0.0, &["CheckAmmo"]),
                ("DRY FIRE DISARMED", -1.0, 0.0, &["!Armed", "AmmoInChamber<1", "Fire"]),
            ],
        ),
        (
            "FIRE",
            0.0967,
            &[("FiringBullet", 0.0), ("ShellEject", 0.31), ("DelAmmoChamber", 0.665), ("AddAmmoInChamber", 0.776), ("FireEnd", 0.873)],
            &[("IDLE", 1.0, 0.0, &[]), ("DRY FIRE", 0.95, 0.0, &["FireMode<1", "AmmoInChamber<1", "AmmoInMag<1"])],
        ),
        ("DRY FIRE", 0.267, &[("Disarm", 0.0)], &[("IDLE", 1.0, 0.0, &["!Fire"])]),
        ("DRY FIRE DISARMED", 0.267, &[], &[("IDLE", 1.0, 0.0, &["!Fire"])]),
        ("RELOAD START", 0.444, &[], &[("RELOAD OUT", 0.3, 0.75, &[])]),
        ("RELOAD OUT", 1.0, &[("MagOut", 0.462), ("MagHide", 0.85)], &[("RELOAD IN", 0.78, 0.22, &["CanReload"])]),
        ("RELOAD OUT ALL", 1.0, &[("MagOut", 0.486), ("MagHide", 0.602)], &[("RELOAD IN", 0.9, 0.1, &[])]),
        (
            "RELOAD IN",
            1.2,
            &[("MagShow", 0.092), ("MagIn", 0.707)],
            &[
                ("RELOAD CHARGE END", 0.95, 0.05, &["MagInWeapon", "AmmoInMag>0", "AmmoInChamber<1"]),
                ("RELOAD END", 0.9, 0.1, &["AmmoInChamber>0", "!Inventory"]),
                ("RELOAD INVENTORY END", 0.94, 0.16, &["Inventory", "AmmoInChamber>0"]),
                ("RELOAD INVENTORY END", 0.94, 0.16, &["Inventory", "AmmoInMag<1"]),
                ("RELOAD END", 0.9, 0.1, &["!Inventory", "AmmoInMag<1"]),
            ],
        ),
        ("RELOAD END", 0.457, &[], &[("IDLE", 0.98, 0.03, &[])]),
        (
            "RELOAD CHARGE END",
            1.3,
            &[("Arm", 0.476), ("AddAmmoInChamber", 0.584)],
            &[("RELOAD INVENTORY END", 0.98, 0.1, &["Inventory"]), ("IDLE", 0.92, 0.19, &["!Inventory"])],
        ),
        ("RELOAD INVENTORY END", 0.333, &[], &[("IN INVENTORY", 0.25, 0.75, &[])]),
        ("OPEN INVENTORY", 0.667, &[], &[("IN INVENTORY", 0.5, 0.375, &[])]),
        ("IN INVENTORY", 0.333, &[], &[("CLOSE INVENTORY", -1.0, 0.042, &["Active", "!Inventory"]), ("IDLE TO OUT", -1.0, 0.5, &["!Active"])]),
        ("CLOSE INVENTORY", 0.667, &[], &[("IDLE", 0.96, 0.045, &["Active"])]),
        ("FIREMODE", 1.0, &[], &[("IDLE", 1.0, 0.0, &[])]),
        ("CHECK", 2.0, &[("StartUtilityOperation", 0.0)], &[("IDLE", 1.0, 0.0, &[])]),
        ("IDLE TO OUT", 0.833, &[("WeapOut", 0.968)], &[("SPAWN", 1.0, 0.0, &[])]),
    ];

    /// The animator of that rifle, written out by hand.
    fn animator() -> WeaponAnimatorData {
        let parameters: Vec<Parameter> =
            PARAMETERS.iter().map(|(name, kind, default)| Parameter { name: (*name).to_owned(), kind: *kind, default: *default }).collect();
        let parameter = |name: &str| parameters.iter().position(|p| p.name == name);
        let condition = |text: &str| {
            let (name, mode, threshold) = match text {
                t if t.starts_with('!') => (&t[1..], 2, 0.0),
                t if t.ends_with(">0") => (&t[..t.len() - 2], 3, 0.0),
                t if t.ends_with("<1") => (&t[..t.len() - 2], 4, 1.0),
                t => (t, 1, 0.0),
            };
            Condition { parameter: parameter(name), mode, threshold }
        };
        let place = |name: &str| STATES.iter().position(|state| state.0 == name).expect("a state of the rifle");
        let states = STATES
            .iter()
            .map(|(name, length, _, ways)| StateDef {
                name: format!("Hands.{name}"),
                speed: 1.0 / length,
                speed_parameter: None,
                looping: false,
                cycle_offset: 0.0,
                trees: vec![None],
                transitions: ways
                    .iter()
                    .map(|(to, exit, blend, conditions)| TransitionDef {
                        target: Target::State(place(to)),
                        duration: *blend,
                        fixed: false,
                        offset: 0.0,
                        exit_time: (*exit >= 0.0).then_some(*exit),
                        conditions: conditions.iter().map(|text| condition(text)).collect(),
                        interruption: 0,
                    })
                    .collect(),
                movement: None,
                loaded: true,
            })
            .collect();
        let machine = MachineDef { states, any_state: Vec::new(), selectors: Vec::new(), default_state: 0 };
        let layer = LayerDef { name: "Hands".to_owned(), machine: 0, slot: 0, default_weight: 1.0, additive: false, moves: Vec::new() };
        let marks: Vec<Vec<(&str, f32)>> = STATES.iter().map(|state| state.2.to_vec()).collect();
        let controller = Controller::by_hand(parameters, vec![machine], vec![layer]);
        WeaponAnimatorData::by_hand(controller, MarkTable::by_hand(&[marks])).expect("a layer for the hands")
    }

    /// A rifle with round numbers: 600 rounds a minute, 450 for single shots.
    pub(crate) fn settings() -> FirearmSettings {
        FirearmSettings {
            fire_rate: 600.0,
            single_fire_rate: 450.0,
            modes: vec![FireMode::Single, FireMode::FullAuto],
            burst_count: 3,
            magazine_capacity: 30,
            bolt_catch: false,
            can_queue_second_shot: true,
            shot_strength: 1.0,
            rest_state: "Hands.IDLE".to_owned(),
            fire_state: "Hands.FIRE".to_owned(),
            fire_mode_state: "Hands.FIREMODE".to_owned(),
            semi_fire_state: "Hands.SA FIRE".to_owned(),
            internal_magazine: false,
            no_fire_mode_on_bolt_catch: false,
            bolt_action: false,
            open_bolt_reload: false,
            must_open_bolt: false,
            manual_bolt_catch: false,
            chamber_load: true,
            foldable: false,
            reload_speed: 1.0,
            draw_speed: 1.0,
            cylinder: None,
            barrels: 0,
            fast_reload: true,
            rocket_launcher: false,
            no_chamber: false,
            magazine_variant: 0,
            mastering: 0,
            stop_aiming_at: 0.1,
            mountable: true,
            bipod: None,
        }
    }

    /// That rifle in the hands and at rest.
    pub(crate) fn rifle() -> Firearm {
        let mut weapon = Firearm::new(settings(), &animator());
        weapon.start_drawn();
        weapon
    }

    const DT: f32 = 1.0 / 60.0;

    fn shots(events: &[Event]) -> usize {
        events.iter().filter(|e| matches!(e, Event::Shot { .. })).count()
    }

    fn run(weapon: &mut Firearm, seconds: f32) -> Vec<Event> {
        let mut all = Vec::new();
        for _ in 0..(seconds / DT).round() as usize {
            all.extend(weapon.step(DT));
        }
        all
    }

    /// The animator's states as they were entered over `seconds`, each
    /// with the time of its step, counted from `from`.
    fn states(weapon: &mut Firearm, seconds: f32, from: f32) -> Vec<(String, f32)> {
        let name = |weapon: &Firearm| {
            let (state, _, next) = weapon.animator().state();
            next.unwrap_or(state).trim_start_matches("Hands.").to_owned()
        };
        let mut last = name(weapon);
        let mut at = Vec::new();
        for step in 0..(seconds / DT).round() as usize {
            weapon.step(DT);
            let now = name(weapon);
            if now != last {
                at.push((now.clone(), from + step as f32 * DT));
                last = now;
            }
        }
        at
    }

    fn names(entered: &[(String, f32)]) -> Vec<&str> {
        entered.iter().map(|entry| entry.0.as_str()).collect()
    }

    /// Steps until `done`, and says how long that took.
    fn run_until(weapon: &mut Firearm, done: impl Fn(&Firearm) -> bool) -> f32 {
        let mut steps = 0;
        while !done(weapon) {
            weapon.step(DT);
            steps += 1;
            assert!(steps < 1200, "never happened");
        }
        steps as f32 * DT
    }

    /// Steps until the weapon is at rest, a step at the least: what a
    /// command asked of the animator shows only after its next update.
    fn settle(weapon: &mut Firearm) -> f32 {
        weapon.step(DT);
        DT + run_until(weapon, |w| w.is_idle())
    }

    /// The inventory opening takes the hands down by the animator's own
    /// states and holds them there; shutting brings them back. The sights
    /// are down, and the weapon's logic idles throughout.
    #[test]
    fn the_hands_go_down_for_an_open_inventory_and_come_back() {
        let mut w = rifle();
        w.set_aim(true, true);
        w.set_inventory(true);
        assert!(!w.aiming());
        let down = states(&mut w, 2.0, 0.0);
        assert_eq!(names(&down), ["OPEN INVENTORY", "IN INVENTORY"]);
        // The held state takes over halfway through the way down.
        assert!((down[1].1 - 0.333).abs() < 3.0 * DT, "{}", down[1].1);
        assert!(w.in_inventory() && !w.is_idle() && !w.occupied());
        w.set_inventory(false);
        let up = states(&mut w, 2.0, 0.0);
        assert_eq!(names(&up), ["CLOSE INVENTORY", "IDLE"]);
        assert!((up[1].1 - up[0].1 - 0.64).abs() < 3.0 * DT, "{:?}", up);
        assert!(w.is_idle() && !w.in_inventory());
    }

    /// Shut again at once, the hands still go down first: the way down has
    /// one way out, into the held state, and the way back opens only when
    /// the blend into that is over.
    #[test]
    fn an_inventory_shut_at_once_still_takes_the_hands_down() {
        let mut w = rifle();
        w.set_inventory(true);
        w.step(DT);
        w.set_inventory(false);
        let all = states(&mut w, 3.0, DT);
        assert_eq!(names(&all), ["IN INVENTORY", "CLOSE INVENTORY", "IDLE"]);
        // Halfway down (0.333 s), then the blend into the held state (0.25 s).
        assert!((all[1].1 - 0.583).abs() < 3.0 * DT, "{:?}", all);
        assert!(w.is_idle());
    }

    /// Opened in a reload, the reload does not bring the hands back to
    /// the weapon first: its last state gives way to the animator's own
    /// way from a reload into the held state.
    #[test]
    fn a_reload_ends_in_the_inventory_s_hold_when_it_is_open() {
        let mut w = rifle();
        w.reload();
        run(&mut w, 0.1);
        w.set_inventory(true);
        let all = states(&mut w, 6.0, 0.1);
        assert_eq!(names(&all), ["RELOAD OUT", "RELOAD IN", "RELOAD INVENTORY END", "IN INVENTORY"]);
        assert_eq!(w.magazine, Some(30));
        assert!(w.in_inventory() && !w.reloading());
        // And from an empty chamber, through the chambering.
        let mut w = rifle();
        (w.chamber, w.magazine) = (false, Some(0));
        w.start_drawn();
        w.reload();
        run(&mut w, 0.1);
        w.set_inventory(true);
        let all = states(&mut w, 8.0, 0.1);
        assert_eq!(names(&all), ["RELOAD OUT", "RELOAD IN", "RELOAD CHARGE END", "RELOAD INVENTORY END", "IN INVENTORY"]);
        assert!(w.chamber && w.magazine == Some(29));
    }

    /// The inventory opened before the magazine is out is only noted: the
    /// animator hears of it when the magazine comes out.
    #[test]
    fn a_reload_tells_the_animator_of_the_inventory_once_the_magazine_is_out() {
        let mut w = rifle();
        w.reload();
        w.step(DT);
        w.set_inventory(true);
        assert!(!w.animator().flag("Inventory"));
        run_until(&mut w, |w| w.magazine.is_none());
        assert!(w.animator().flag("Inventory"));
    }

    /// A weapon is drawn by its animator's own states, appears at the
    /// draw's last mark, and is put away the same way.
    #[test]
    fn a_weapon_is_drawn_and_put_away_by_its_animator() {
        let mut w = Firearm::new(settings(), &animator());
        assert!(!w.in_hand());
        // Out of the hands its keys do nothing.
        w.set_trigger(true, true);
        w.reload();
        assert_eq!(shots(&run(&mut w, 0.5)), 0);
        w.set_trigger(false, true);

        w.draw();
        assert!(w.in_hand() && w.hands_busy() && w.occupied());
        // The inventory opening in a draw is not heard by the draw.
        w.set_inventory(true);
        let mut events = Vec::new();
        let appeared = run_until(&mut w, |w| !w.occupied());
        events.extend(run(&mut w, 0.2));
        assert!((appeared - 0.95).abs() < 4.0 * DT, "{appeared}");
        assert!(w.is_idle() && !w.hands_busy() && !w.in_inventory());
        // What has appeared is told of the screen afresh, and then the
        // hands go down for it like any other weapon's.
        w.set_inventory(true);
        run(&mut w, 1.0);
        assert!(w.in_inventory());
        w.set_inventory(false);
        settle(&mut w);
        assert!(w.is_idle());

        w.hide(false);
        assert!(w.occupied() && w.hands_busy());
        for _ in 0..120 {
            events.extend(w.step(DT));
        }
        assert!(events.contains(&Event::Hidden), "{events:?}");
        assert!(!w.in_hand());
    }

    /// Drawn with rounds in the magazine and none chambered, a weapon
    /// chambers one.
    #[test]
    fn a_draw_chambers_a_round() {
        let mut w = Firearm::new(settings(), &animator());
        (w.chamber, w.magazine) = (false, Some(12));
        w.draw();
        assert!(w.chamber && w.magazine == Some(11));
    }

    /// A draw made to rack for the look of it tells the animator an empty
    /// chamber and a round to feed, and moves no round; a draw that racks
    /// of itself is left as it is.
    #[test]
    fn a_racked_draw_moves_no_round() {
        let mut w = Firearm::new(settings(), &animator());
        (w.chamber, w.magazine) = (true, Some(12));
        w.draw();
        w.rack_drawn();
        assert_eq!((w.animator.get("AmmoInChamber"), w.animator.get("AmmoInMag")), (0.0, 12.0));
        run_until(&mut w, |w| !w.occupied());
        run(&mut w, 0.2);
        assert!(w.is_idle(), "{:?}", w.operation);
        assert_eq!((w.chamber, w.magazine), (true, Some(12)));
        // An empty magazine is drawn racking all the same, and still holds
        // nothing.
        let mut w = Firearm::new(settings(), &animator());
        (w.chamber, w.magazine) = (true, Some(0));
        w.draw();
        w.rack_drawn();
        assert_eq!(w.animator.get("AmmoInMag"), 1.0);
        run_until(&mut w, |w| !w.occupied());
        assert!(w.chamber && w.magazine == Some(0));
        // Drawn with none chambered, it chambers one of itself, once.
        let mut w = Firearm::new(settings(), &animator());
        (w.chamber, w.magazine) = (false, Some(12));
        w.draw();
        w.rack_drawn();
        assert!(w.chamber && w.magazine == Some(11));
    }

    /// Put away while the hands are down for the inventory, the weapon
    /// goes from there.
    #[test]
    fn a_weapon_is_put_away_from_the_inventory_s_hold() {
        let mut w = rifle();
        w.set_inventory(true);
        run(&mut w, 1.5);
        w.hide(false);
        let all = states(&mut w, 2.0, 0.0);
        assert_eq!(names(&all), ["IDLE TO OUT", "SPAWN"]);
        assert!(!w.in_hand());
    }

    #[test]
    fn a_single_shot_fires_once_however_long_the_trigger_is_held() {
        let mut w = rifle();
        w.set_trigger(true, true);
        // The round goes at the press; the shot is made a frame on.
        assert_eq!(w.magazine, Some(29));
        let (first, second) = (w.step(DT), w.step(DT));
        assert_eq!((shots(&first), shots(&second)), (0, 1));
        let mut events = run(&mut w, 1.0);
        assert_eq!(shots(&events), 0);
        events.extend(second);
        assert!(w.chamber && w.is_idle());
        assert!(events.contains(&Event::ShellEject));
    }

    #[test]
    fn single_shots_are_limited_by_their_own_rate() {
        let mut w = rifle();
        // Tapping every other step, far faster than the weapon allows.
        let mut fired = 0;
        for step in 0..120 {
            w.set_trigger(step % 2 == 0, true);
            fired += shots(&w.step(DT));
        }
        // Two seconds at 450 a minute is 15 shots.
        assert!((14..=16).contains(&fired), "{fired}");
    }

    #[test]
    fn automatic_fire_keeps_the_rate_and_stops_on_release() {
        let mut w = rifle();
        w.next_fire_mode();
        settle(&mut w);
        assert_eq!(w.mode(), FireMode::FullAuto);
        w.set_trigger(true, true);
        let events = run(&mut w, 1.0);
        // The first shot at once, then one every tenth of a second.
        assert!((10..=11).contains(&shots(&events)), "{}", shots(&events));
        assert!(w.auto_fire());
        w.set_trigger(false, true);
        let after = run(&mut w, 0.5);
        assert!(shots(&after) <= 1);
        assert!(w.is_idle());
        assert!(after.contains(&Event::AutoFire(false)));
    }

    #[test]
    fn a_tap_in_automatic_still_fires_one_round() {
        let mut w = rifle();
        w.next_fire_mode();
        settle(&mut w);
        w.set_trigger(true, true);
        w.set_trigger(false, true);
        assert_eq!(shots(&run(&mut w, 0.5)), 1);
    }

    #[test]
    fn the_weapon_runs_dry_and_a_reload_fills_it() {
        let mut w = rifle();
        w.next_fire_mode();
        settle(&mut w);
        w.set_trigger(true, true);
        let events = run(&mut w, 5.0);
        // Thirty in the magazine and one in the chamber.
        assert_eq!(shots(&events), 31);
        assert!(!w.chamber && w.magazine == Some(0));
        assert!(events.contains(&Event::DryFire));
        w.set_trigger(false, true);
        settle(&mut w);

        w.reload();
        let events = run(&mut w, 6.0);
        assert!(events.contains(&Event::MagazineOut) && events.contains(&Event::MagazineIn));
        // The empty chamber took the first round of the new magazine.
        assert!(w.chamber);
        assert_eq!(w.magazine, Some(29));
        assert!(w.is_idle());
    }

    #[test]
    fn reloading_with_a_round_chambered_keeps_it() {
        let mut w = rifle();
        w.set_trigger(true, true);
        run(&mut w, 0.5);
        w.set_trigger(false, true);
        w.reload();
        let all = states(&mut w, 6.0, 0.0);
        assert!(w.chamber);
        assert_eq!(w.magazine, Some(30));
        assert_eq!(names(&all), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
        // Each state gives way where the animator's way to the next one
        // opens: start 0.3 of 0.444 s, out 0.78 of 1 s, in 0.9 of 1.2 s,
        // end 0.98 of 0.457 s.
        let rest = all.last().unwrap().1 - all[0].1;
        assert!((rest - 2.44).abs() < 5.0 * DT, "{rest}");
    }

    #[test]
    fn nothing_fires_while_reloading() {
        let mut w = rifle();
        w.reload();
        w.set_trigger(true, true);
        assert_eq!(shots(&run(&mut w, 1.0)), 0);
    }

    /// A reload asked for in a shot follows it.
    #[test]
    fn a_reload_asked_for_in_a_shot_follows_it() {
        let mut w = rifle();
        w.set_trigger(true, true);
        w.step(DT);
        w.reload();
        assert!(!w.reloading());
        let wait = run_until(&mut w, |w| w.reloading());
        // The wait between single shots, 60 / 450 s.
        assert!((wait - 0.133).abs() < 3.0 * DT, "{wait}");
        run_until(&mut w, |w| w.is_idle());
        assert_eq!(w.magazine, Some(30));
    }

    /// A press made where the movement state gives the hands no say (a
    /// sprint, a crawl) is dropped, and holding the trigger until the state
    /// ends fires nothing. Automatic fire already running carries on.
    #[test]
    fn a_press_in_a_sprint_or_a_crawl_is_dropped() {
        let mut w = rifle();
        w.set_trigger(true, false);
        let events = run(&mut w, 0.5);
        assert!(shots(&events) == 0 && w.is_idle() && w.magazine == Some(30));
        // The state ends with the trigger still held.
        w.set_trigger(true, true);
        assert_eq!(shots(&run(&mut w, 0.5)), 0);
        // Let go and pressed again.
        w.set_trigger(false, true);
        w.set_trigger(true, true);
        assert_eq!(shots(&run(&mut w, 0.5)), 1);
        w.set_trigger(false, true);

        w.next_fire_mode();
        settle(&mut w);
        w.set_trigger(true, true);
        run(&mut w, 0.3);
        // The crawl begins with the trigger held.
        w.set_trigger(true, false);
        assert!(shots(&run(&mut w, 0.5)) >= 4);
        w.set_trigger(false, false);
        run(&mut w, 0.5);
        assert!(w.is_idle());
        // A fresh press in the crawl does nothing.
        w.set_trigger(true, false);
        assert_eq!(shots(&run(&mut w, 0.5)), 0);
    }

    #[test]
    fn a_reload_is_over_when_the_magazine_is_in_and_plays_on() {
        // Start left at 0.133 s, out at 0.78 s, and the magazine in 0.707
        // of the way through the 1.2 s that follow.
        let mut w = rifle();
        w.reload();
        assert!(w.reloading());
        let over = run_until(&mut w, |w| !w.reloading());
        assert!((over - 1.76).abs() < 4.0 * DT, "{over}");
        assert_eq!(w.magazine, Some(30));
        // The hands are still coming back: the weapon is not at rest and
        // a press of the trigger is still dropped.
        assert!(!w.is_idle() && w.hands_busy());
        w.set_trigger(true, true);
        let rest = run_until(&mut w, |w| w.is_idle());
        assert!(rest > 0.5, "{rest}");
        assert_eq!(w.magazine, Some(30));
        assert!(!w.hands_busy());
        // Back at rest the trigger is heard again.
        w.set_trigger(false, true);
        w.set_trigger(true, true);
        assert_eq!(shots(&run(&mut w, 0.5)), 1);
    }

    #[test]
    fn a_reload_from_an_empty_chamber_is_over_when_the_round_is_chambered() {
        let mut w = rifle();
        (w.chamber, w.magazine) = (false, Some(0));
        w.start_drawn();
        w.reload();
        run_until(&mut w, |w| w.magazine == Some(30));
        assert!(w.reloading() && !w.chamber);
        run_until(&mut w, |w| w.chamber);
        assert!(!w.reloading() && !w.is_idle());
        assert_eq!(w.magazine, Some(29));
    }

    #[test]
    fn an_empty_magazine_going_in_ends_the_reload_with_nothing_chambered() {
        let mut w = rifle();
        (w.chamber, w.magazine) = (false, Some(0));
        w.start_drawn();
        w.reload_with(0);
        let all = states(&mut w, 10.0, 0.0);
        assert_eq!(names(&all), ["RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
        assert!(w.is_idle() && !w.chamber);
    }

    /// The quick reload throws the magazine out by a state of its own
    /// (`RELOAD OUT ALL`) and is the plain reload from there; the host is
    /// told of the exchange as it begins and of the old magazine leaving
    /// the hand.
    #[test]
    fn the_quick_reload_throws_the_magazine_out_and_goes_on_as_the_plain_one() {
        let mut w = rifle();
        w.set_trigger(true, true);
        run(&mut w, 0.5);
        w.set_trigger(false, true);
        let mut again = rifle();
        again.magazine = Some(29);
        w.quick_reload_with(30);
        assert!(w.reloading());
        let all = states(&mut w, 6.0, 0.0);
        assert_eq!(names(&all), ["RELOAD OUT ALL", "RELOAD IN", "RELOAD END", "IDLE"]);
        assert!(w.chamber && w.magazine == Some(30));
        // Out all 0.9 of 1 s, in 0.9 of 1.2 s, end 0.98 of 0.457 s.
        let rest = all.last().unwrap().1 - all[0].1;
        assert!((rest - 2.43).abs() < 5.0 * DT, "{rest}");
        again.quick_reload_with(30);
        let swap = again.step(DT);
        assert!(swap.contains(&Event::MagazineSwap { left: Some(29), quick: true }), "{swap:?}");
    }

    #[test]
    fn the_old_magazine_leaves_the_hand_once_in_either_reload() {
        for quick in [false, true] {
            let mut w = rifle();
            if quick { w.quick_reload_with(30) } else { w.reload_with(30) }
            let mut said = w.step(DT);
            while !w.is_idle() {
                said.extend(w.step(DT));
                assert!(said.len() < 10_000);
            }
            let hidden = said.iter().filter(|e| **e == Event::MagazineHidden).count();
            let swaps: Vec<&Event> = said.iter().filter(|e| matches!(e, Event::MagazineSwap { .. })).collect();
            assert_eq!(hidden, 1, "{quick}");
            assert_eq!(swaps, [&Event::MagazineSwap { left: Some(30), quick }]);
            // It leaves after the magazine is out and before the new one is in.
            let at = |wanted: &Event| said.iter().position(|e| e == wanted).unwrap();
            assert!(at(&Event::MagazineOut) < at(&Event::MagazineHidden) && at(&Event::MagazineHidden) < at(&Event::MagazineIn));
        }
    }

    /// The key is heard only with the animator at rest, and is the plain
    /// reload where the weapon does not allow the quick one or has no
    /// magazine fitted.
    #[test]
    fn the_quick_reload_is_refused_away_from_rest_and_plain_where_not_allowed() {
        let swapped = |events: &[Event]| events.iter().any(|e| matches!(e, Event::MagazineSwap { .. }));
        let mut w = rifle();
        w.reload();
        w.step(DT);
        w.quick_reload_with(30);
        assert!(!swapped(&w.step(DT)));
        // Not allowed by the weapon.
        let mut settings = settings();
        settings.fast_reload = false;
        let mut w = Firearm::new(settings, &animator());
        w.start_drawn();
        w.quick_reload_with(30);
        assert!(w.step(DT).contains(&Event::MagazineSwap { left: Some(30), quick: false }));
        assert_eq!(names(&states(&mut w, 1.0, 0.0))[0], "RELOAD OUT");
        // No magazine fitted.
        let mut w = rifle();
        w.magazine = None;
        w.quick_reload_with(30);
        assert!(w.step(DT).contains(&Event::MagazineSwap { left: None, quick: false }));
    }

    #[test]
    fn checking_the_magazine_lowers_the_sights_and_holds_them_down() {
        let mut w = rifle();
        w.set_aim(true, true);
        w.check_magazine();
        assert!(w.checking() && !w.aiming() && w.hands_busy());
        w.set_aim(true, true);
        assert!(!w.aiming(), "the key is dead through the check");
        let took = settle(&mut w);
        assert!((took - 2.0).abs() < 4.0 * DT, "{took}");
        assert!(!w.aiming(), "and they do not come back by themselves");
        assert!(!w.hands_busy() && !w.checking());
        w.set_aim(true, true);
        assert!(w.aiming());
        // Switching the fire mode leaves them be: the weapon's logic
        // idles through it, and the key works.
        w.next_fire_mode();
        w.step(DT);
        assert!(!w.is_idle() && !w.occupied() && !w.checking() && w.aiming());
        w.set_aim(false, true);
        w.set_aim(true, true);
        assert!(w.aiming());
    }

    /// A press of the trigger while the fire mode is being switched fires,
    /// in the mode just switched to (the fifteenth recording, scenario
    /// 64). The reload key is taken too, and the reload begins when the
    /// switch has played out.
    #[test]
    fn a_press_in_a_fire_mode_switch_fires() {
        let mut w = rifle();
        w.next_fire_mode();
        w.step(DT);
        assert_eq!(w.animator().state().0, "Hands.FIREMODE");
        run(&mut w, 0.3);
        assert_eq!(w.animator().state().0, "Hands.FIREMODE");
        w.set_trigger(true, true);
        let burst = run(&mut w, 0.2);
        w.set_trigger(false, true);
        assert!(shots(&burst) >= 2, "automatic from the key on: {}", shots(&burst));
        settle(&mut w);

        w.next_fire_mode();
        w.step(DT);
        w.reload();
        assert!(w.reloading());
        let all = states(&mut w, 6.0, 0.0);
        assert_eq!(names(&all), ["IDLE", "RELOAD START", "RELOAD OUT", "RELOAD IN", "RELOAD END", "IDLE"]);
    }

    #[test]
    fn the_aim_key_waits_for_the_magazine_not_for_the_animation() {
        let mut w = rifle();
        w.set_aim(true, true);
        w.reload();
        // Below mastering 2 the reload lowers the sights and the key does
        // nothing while it is in hand.
        assert!(!w.aiming());
        w.set_aim(true, true);
        assert!(!w.aiming());
        run_until(&mut w, |w| !w.reloading());
        // With the magazine in the key works, and so does what lowers the
        // sights from outside, though the hands are still coming back.
        assert!(!w.is_idle());
        w.set_aim(true, true);
        assert!(w.aiming());
        w.lower_aim();
        assert!(!w.aiming());
    }

    /// The aim's call that moves the sights says so, up or down, for the
    /// sounds (`SetAim`); a call that leaves them as they are, or what
    /// lowers them otherwise, says nothing.
    #[test]
    fn the_sights_moved_by_the_aim_s_call_are_told() {
        let mut w = rifle();
        let aimed = |w: &mut Firearm| -> Vec<bool> {
            w.step(0.0).into_iter().filter_map(|e| if let Event::Aimed(up) = e { Some(up) } else { None }).collect()
        };
        aimed(&mut w);
        w.set_aim(true, true);
        assert_eq!(aimed(&mut w), [true]);
        w.set_aim(true, true);
        assert_eq!(aimed(&mut w), Vec::<bool>::new());
        w.set_aim(false, true);
        assert_eq!(aimed(&mut w), [false]);
        w.set_aim(true, true);
        w.lower_aim();
        assert_eq!(aimed(&mut w), [true]);
        assert!(!w.aiming());
    }
}
