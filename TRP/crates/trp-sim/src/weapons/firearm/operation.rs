//! The operations a firearm runs, the blends and the chambering choices.

use super::*;

/// The mastering level from which a reload keeps the sights up.
pub(super) const MASTERING_TO_AIM_IN_A_RELOAD: u32 = 2;

/// The turn-away's value below which sights an obstacle put down come back.
pub(super) const TURN_AWAY_RUN_DOWN: f32 = 0.2;

/// The game's blend between 0 and 1 that runs at a set speed from a
/// moment that may lie ahead.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Blend {
    pub(super) target: f32,
    pub(super) speed: f32,
    start_value: f32,
    start_time: f32,
}

impl Blend {
    pub(super) fn value(&self, now: f32) -> f32 {
        let direction = if self.target >= 0.5 { 1.0 } else { -1.0 };
        (self.start_value + (now - self.start_time) * self.speed * direction).clamp(0.0, 1.0)
    }

    /// Sends it toward `target` at `speed` a second, starting `delay`
    /// seconds from now. A target it already has changes nothing.
    pub(super) fn aim_for(&mut self, target: f32, speed: f32, delay: f32, now: f32) {
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
pub(super) enum Planned {
    /// A magazine's reload, the quick way or not.
    Reload(u32, bool),
    Shells(u32),
    Hide(bool),
}

/// The operation in hand, as the game has one at a time.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Operation {
    /// Not in the hands: before it is first drawn and after it has been
    /// put away.
    Away,
    /// Being drawn, until the draw's last mark. `hide` is a put-away asked
    /// for meanwhile; `arms` is the draw racking a round into an empty
    /// chamber; `compatible` whether the magazine fed as the draw began
    /// (`SpawnOperation._ammoCompatible`).
    Spawn { hide: Option<bool>, arms: bool, compatible: bool },
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
    /// for the round thrown out has been answered. `fed` is that the empty
    /// chamber was given its round as the filling began.
    Shells { within_reach: u32, loaded: u32, can_abort: bool, aborted: bool, chambered: bool, emptied: bool, fed: bool, hide: Option<bool> },
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
    /// The chambered round thrown out by working the action, and the
    /// magazine's next fed in its place (`RechamberOperation`; firing spec,
    /// "The magazine's and the chamber's keys"): `feeds` that the magazine
    /// had a round as it began; `removed`, `ejected` and `added` that the
    /// marks `DelAmmoChamber`, `ShellEject` and `AddAmmoInChamber` have come;
    /// `hide` a put-away asked for meanwhile, made at its end.
    Rechamber { feeds: bool, removed: bool, ejected: bool, added: bool, hide: Option<bool> },
    /// A cylinder turned on by a chamber (`RollCylinderOperation`), until
    /// its state's `MagHide` mark.
    RollCylinder,
    /// A round the inventory has put into a chamber of the weapon's own
    /// (more-weapons spec, "A round moved into a chamber of its own, or out
    /// of one, from the inventory"): a cylinder's (`LoadCartridgeToCamora`),
    /// until the cylinder is shut; a barrel's (`LoadCartridgeToChamber`),
    /// until the round is in. `chamber` is its number, `loaded` that the
    /// round is in.
    OwnChamberIn { chamber: usize, loaded: bool },
    /// The round in a chamber of the weapon's own taken out from the
    /// inventory: a cylinder's (`DischargeAmmoFromCamoraOperation`), a
    /// barrel's (`DischargeOperation`; of two barrels or more,
    /// `DischargeMultiBarrelWeaponOperation`), until the hand has put it
    /// away or, going on into another chamber, until it is out. `onward`
    /// is that it goes into another chamber; `out` that it has left.
    OwnChamberOut { chamber: usize, onward: bool, out: bool },
    /// A shot's malfunction, until its state's mark (`StartMalfunction`).
    /// `hide` is a put-away asked for meanwhile, made at its end (each of
    /// the three is a `MalfunctionOperation`, whose `HideWeapon` plans it
    /// for `SwitchToIdle`).
    Malfunction { malf: Malf, hide: Option<bool> },
    /// The look at a malfunction (`ExamineMalfunctionTypeOperation`).
    ExamineMalfunction { hide: Option<bool> },
    /// Its repair (`RepairMalfunction`), `ejected` that its round has been
    /// thrown out.
    RepairMalfunction { malf: Malf, ejected: bool, hide: Option<bool> },
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
    /// The switch to the underbarrel launcher or back (`ToggleLauncherOperation`),
    /// until the launcher's `LauncherAppeared` or `LauncherDisappeared`: `on` the
    /// way; `hide` a put-away asked for meanwhile, made at its end.
    ToggleLauncher { on: bool, hide: Option<bool> },
    /// The launcher's rest (`LauncherIdling`).
    LauncherIdle,
    /// The launcher's shot (`LauncherFire`), until its `FireEnd`: `fired` that its
    /// `FiringBullet` has come.
    LauncherFire { fired: bool },
    /// The launcher reloaded by the key (`LauncherReload`), until its
    /// `AddAmmoInChamber`; `hide` a put-away asked for meanwhile, made at its end.
    LauncherReload { hide: Option<bool> },
    /// A round put into the launcher from the inventory
    /// (`LoadAmmoToUnderbarrelWeapon`), until its `AddAmmoInChamber`.
    LauncherLoad,
    /// The launcher's round taken out (`DischargeAmmoFromUnderbarrelWeapon`), until
    /// its `MagHide`.
    LauncherDischarge,
    /// The launcher looked over or into (`LauncherUtilityOperation`), as `Utility`.
    LauncherUtility { ready: bool, time: f32, examine: bool },
}

/// A skill's action the hands raise (`Event::Earn`), each where the game's
/// operation raises it, once each time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Earn {
    /// The class's `WeaponReloadAction`: a reload begun
    /// (`BaseReloadOperation.Start`, for a magazine, a tube, a cylinder or
    /// barrels); each round a tube's filling commits
    /// (`ReloadInternalMagOperation.CommitReloadWithAmmo`, and that of the
    /// filling with the bolt open); a cylinder's reload ended with a round
    /// loaded (`ReloadCylinderMagOperation.EndReload`).
    Reload,
    /// The class's `WeaponChamberAction`: a round the hands put into a
    /// chamber or take out of one for the inventory, as the operation begins
    /// (`Idling.Execute`: `LoadCartridgeToChamber`, `DischargeOperation`, a
    /// cylinder's `LoadCartridgeToCamora`, `DischargeAmmoFromCamoraOperation`);
    /// the round a tube's filling gives the empty chamber, at its mark
    /// (`ReloadInternalMagOperation.OnAddAmmoInChamber`); the bolt shut on a
    /// filling with the bolt open and an empty chamber
    /// (`ReloadInternalMagWithOpenBoltOperation.CommitReloadWithAmmo`). (The
    /// rack's, `RechamberOperation.Start`, is `Event::RackBegun`'s.)
    Chamber,
    /// Mag Drills' `RaidLoadedAmmoAction`, a round loaded by the hands: each
    /// round into a tube at its mark (`AddAmmoToMag`), a round put into a
    /// chamber from the inventory as it goes in
    /// (`LoadCartridgeToChamber.OnAddAmmoInChamber`). A cylinder's rounds
    /// give none (`LoadCartridgeToCamora` and the cylinder's reload raise it
    /// nowhere).
    RoundLoaded,
}

/// What the reload key's last resort leaves the inventory to do
/// (`FirearmHandsInputTranslator.LoadAmmoToChamber`; firing spec, "A round
/// into the chamber by the reload key").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChamberByHand {
    /// Nothing: no chamber of the weapon's own, a round in it, no round
    /// within reach, a malfunction (reminded of), or a magazine with rounds.
    Nothing,
    /// The round found put into the chamber: the inventory's move, which
    /// the hands then play (`load_chamber`) where they take it.
    Load,
    /// The fitted magazine, empty, to go to the vest or the pockets first
    /// (the inventory's move, the hands' `pull_out_magazine`), and the round
    /// put in as the hands are done with it.
    MagazineOutFirst,
}

/// What the chamber's unload key leaves the inventory to do
/// (`FirearmHandsInputTranslator.UnloadChamber`; firing spec, "The
/// magazine's and the chamber's keys").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChamberKey {
    /// Nothing: the key refused, or answered by the hands alone (a
    /// malfunction looked into, the action worked, a cylinder turned).
    Done,
    /// The first loaded chamber's round to go to the vest or the pockets,
    /// the smallest grid with room first: the inventory's move, which the
    /// hands then play (`discharge`, `unload_own_chamber`).
    ToRig,
    /// The underbarrel launcher is in use: its round, where it has one, to
    /// go to the vest or the pockets as `ToRig`'s, which the hands then play
    /// (`UnloadLauncher`, `Firearm::discharge_launcher`).
    Launcher,
}
