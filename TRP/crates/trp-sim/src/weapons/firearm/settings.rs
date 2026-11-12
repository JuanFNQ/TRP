//! Fire modes, the firearm's settings and the events it reports.

use super::*;

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
    /// The fitted magazine comes out (`ReloadMagType` `ExternalMagazine`):
    /// on a weapon whose own mode is `InternalMagazine` (the Model 700)
    /// the game's `Weapon.ReloadMode` is then
    /// `ExternalMagazineWithInternalReloadSupport`, and the look at the
    /// magazine is the animator's for one that comes out (`check_magazine`).
    pub magazine_external: bool,
    /// The largest `UniqueAnimationModID` of the build's parts: the
    /// animator's `UniqueAnimationModId`, by which some weapons' animators
    /// take a part's own states (the PP-19's left-side charging handle,
    /// the BAD lever, a side-folding stock); above 0, aiming is kept from
    /// the animators (`tell_aim`). Firing spec, "A part with its own
    /// animations".
    pub unique_animation: i32,
    /// The character's mastering of this weapon (its family's level,
    /// `CurrentMasteringLevel`), 0 to 2: the animator's `WeaponLevel`; from
    /// 2 a reload keeps the sights up, from 1 a revolver loads a fistful.
    pub mastering: u32,
    /// A belt-fed machine gun (`IsBeltMachineGun`): its reload puts the
    /// sights down whatever the mastering (`DisableAimingOnReload`).
    pub belt_fed: bool,
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
    /// A weapon used once (`IsOneoff`, seven in the install): the reload's
    /// keys do nothing at all with it, a mount not left either.
    pub one_off: bool,
    /// Its rounds may be taken out by the player (`CanUnloadAmmoByPlayer`:
    /// every weapon of the install but the RShG-2): the chamber's key does
    /// nothing with one that may not.
    pub can_unload: bool,
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
    /// A shot is made. `hammer_down`: a cylinder's hammer was down as it
    /// was (`CylinderHammerClosed` read by `Shot`), so its spread takes the
    /// double action's penalty (ballistics spec, "A shot leaves the
    /// weapon"); false for any other weapon.
    Shot { strength: f32, hammer_down: bool },
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
    /// The trigger pressed at rest with a magazine whose top round the
    /// chamber does not take (`Idling.ShowIncompatibleNotification`): the
    /// host tells the shooter (firing spec, "Rounds the chamber does not
    /// take").
    IncompatibleAmmo,
    Sound(String),
    /// A mark of a sound from an underbarrel launcher's animator while it is
    /// the hands': the launcher's own sound player plays it (its prefab's own
    /// handler of its marks; underbarrel-launcher spec, "Two animators").
    LauncherSound(String),
    /// The hands' clip opened (true) or folded (false) the bipod: its
    /// `BipodOpen` and `BipodClose` marks, which the bipod's own sound
    /// player plays (`BipodAudioController.OnBipodOpen`, `OnBipodClose`).
    BipodSound(bool),
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
    /// A skill's action the hands raise where the game's firearm
    /// operations raise it (`Player.Skills.<action>.Complete`; skills spec,
    /// "How a skill is earned"): the host gives it to the class of the
    /// weapon in the hands, or to Mag Drills.
    Earn(Earn),
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
    LeftStance(crate::weapons::left_stance::Call),
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
    /// A quick put-away has begun (`Remove.Start` with
    /// `CheckForFastWeaponSwitch`): the arms pay for it now
    /// (`Physical.OnWeaponSwitchFast`).
    FastSwitch,
    /// The weapon was put away from rest, its sights up or not
    /// (`Idling.HideWeapon`: the player's `PreviousWeaponAimState`). The
    /// host keeps it for the next weapon drawn (`set_appearing`). A
    /// put-away that an operation makes straight from its own end (after a
    /// shot, a rack, a malfunction's, a double barrel's reload) says
    /// nothing, and the host keeps what it had.
    AimKept(bool),
    /// So many rounds have left the character's stock for the weapon: one
    /// for an empty chamber as a tube's filling begins, and what went into
    /// the tube as it ends.
    RoundsTaken(u32),
    /// The chambered round has been thrown out of the weapon, unfired: a
    /// magazine filled with the bolt open (the game puts it on the ground
    /// as loot).
    RoundEjected,
    /// The chambered round's rack has begun (`RechamberOperation.Start`;
    /// firing spec, "The magazine's and the chamber's keys"): `feeds` that
    /// the magazine has a round to take its place, which counts as the
    /// class's chamber action then (`WeaponChamberAction`).
    RackBegun { feeds: bool },
    /// The rack's round has been thrown out of the weapon, unfired
    /// (`RechamberOperation.OnShellEjectEvent`): the inventory throws it away
    /// as a thing (`ThrowPatronAsLoot`), where the chamber held it until now.
    RoundRacked,
    /// The live round in a single barrel being reloaded has come out
    /// (`ReloadSingleBarrelOperation.OnShellEjectEvent`): the game puts it
    /// in the vest or the pockets where it has room, and else on the
    /// ground.
    RoundUnloaded,
    /// A cylinder's quick reload has thrown the live round of this chamber
    /// out (`ReloadCylinderMagOperation.RemoveAllAmmo`: taken out of its
    /// chamber and thrown into the world as a thing, `ThrowPatronAsLoot`).
    RoundDropped { chamber: usize },
    /// A reload of a weapon with chambers of its own shows a round at a
    /// chamber's point (`SetRoundIntoWeapon`: a cylinder's at each
    /// `AddAmmoInChamber` that loads one, barrels' at `MagShow`): the
    /// `nth` of the rounds the reload takes, from the first (`AmmoPack.
    /// GetAmmoToReload`), for the host to draw as that round's kind.
    RoundShown { chamber: usize, nth: usize },
    /// The case at a chamber's shell port is thrown (`StartSpawnShell`,
    /// `StartSpawnAllShells`), with this share of the player's velocity, or
    /// with none, at one metre a second straight down (`Vector3.down`: a
    /// cylinder emptied of every case at once). More-weapons spec, "The
    /// rounds and cases seen in chambers of their own".
    CaseThrown { chamber: usize, share: Option<f32> },
    /// The round fired from a chamber of its own has gone to the chamber's
    /// shell port as its case, in place of any case there (the shot's
    /// `FiringBullet`: `MoveAmmoFromChamberToShellPort`): from now the case
    /// there, and the weapon's (`Weapon.ShellsInChambers`, which the game
    /// sets at this mark for barrels and at the press for a cylinder), is
    /// of that round's kind.
    CaseLeft { chamber: usize },
    /// The stock has been folded, or unfolded: the weapon is a cell
    /// shorter or longer, its recoil is without or with the stock's share,
    /// and it may be held closer (`docs/spec/shotgun.md`, "The stock").
    Folded(bool),
    /// What the body is to do with the thing in hand, by the game's number
    /// (the mark `ThirdAction`: 1 as it comes to the hands, 2 as it
    /// leaves them): for `Body::set_weapon_action`.
    BodyAction(i32),
    /// The skill's draw speed as the weapon's animator was just told it,
    /// which the game tells the body's animator with it
    /// (`PlayerAnimator.SetSpeedParameters`): for `Body::set_draw_speed`.
    BodyDrawSpeed(f32),
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
    /// What happened to the underbarrel launcher (`launcher::LauncherEvent`).
    Launcher(LauncherEvent),
}
