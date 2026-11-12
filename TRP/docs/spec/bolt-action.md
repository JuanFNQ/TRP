# A bolt action: the Remington Model 700

What a rifle whose bolt is worked by hand does that the AKM and the M3 do not: the bolt is worked after each shot, and only once the trigger is let go; and its magazine, which stays in the rifle, is filled a round at a time with the bolt open, which throws the chambered round out first. Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly) on 2026-10-12 and from the weapon's own animator (`probe states 5bfea6e90db834001b7347f3`; kept as `local\m700_states.txt`) and its item in the database. **No recording has this weapon: everything here is from the code and the install.** Class and field names are the game's. What `firing.md` says of the operations, the marks and the order of a frame, and what `shotgun.md` says of a magazine filled a round at a time, hold here too; this is what is added.

## The weapon as the install has it

Item `5bfea6e90db834001b7347f3`, a `SniperRifle`. From its template:

| Figure | Value | What it does |
|---|---|---|
| `BoltAction` | true | the shot is the bolt action's own operation |
| `ReloadMode` | `InternalMagazine` | the template's; with a magazine that comes out fitted, the weapon's is `ExternalMagazineWithInternalReloadSupport` (below) |
| `MustBoltBeOpennedForInternalReload` | true | the filling is the one made with the bolt open |
| `weapFireType` | `single` | one mode |
| `SingleFireRate` | 450 | a shot's wait: 0.133 s |
| `CanQueueSecondShot` | false | a press during a shot is not remembered |
| `isBoltCatch` | true | the animator's catch is set by the filling's own marks |
| the stock preset's magazine | Wyatt's five-round, `ReloadMagType` `ExternalMagazine` | five, and one in the chamber; it comes out (as every magazine the rifle takes) |

The stock preset has no sight at all. The rifle also takes magazines that come out (the animator has `IsExternalMag` and a second family of reload states for them): a weapon's reload mode is its template's unless the magazine fitted says otherwise (`Weapon.ReloadMode`), and then the key changes the magazine first and fills only if there is none to change. **So the rifle as the game has it is always of the mixed mode** (every magazine of the install that fits it is `ExternalMagazine`; read 2026-10-31, where it was taken until then for one that stays in): the key changes the magazine where one is within reach, and fills the fitted one a round at a time with the bolt open (below) only where none is. **Built**, below, "The magazine changed, or filled".

## The shot (`BoltActionFireOperation`)

It is the plain single shot (`firing.md`) with one parameter of the animator added, `BoltActionReload`, which says the bolt may be worked:

- As the shot begins it is set to whether the trigger is **not** pressed; the trigger is, so it is down.
- Whenever the trigger changes it is set again from the trigger as the controller has it. The shot's own mark lets the controller's trigger go (as for any single shot) but does not touch the parameter: **so the bolt waits until the finger comes off the trigger.** A press made again meanwhile leaves the controller's trigger off, and the bolt is worked.
- The inventory opening, a reload asked for (taken or not), and the weapon being put away each set it at once. Put away, and when the movement state takes the sights down, the sights go down and the trigger is let go.

The animator does the rest. `FIRE` (0.333 s played twice as fast) has the shot's mark and lets the hammer down; it has one way on, into `FIRE RELOAD`, taken once the state has run out **and** `BoltActionReload` is up. `FIRE RELOAD` (1.767 s, three variants by mastering) is the bolt worked: the case out (`ShellEject` 0.324, `DelAmmoChamber` 0.337), the next round in (`AddAmmoInChamber` 0.349, if the magazine has one), the hammer cocked (`Arm` 0.520), `FireEnd` 0.901, and back to rest. The operation ends as any single shot does, when its 0.133 s are out and the animator has been back at rest; so for as long as the trigger is held after a shot the weapon stays in its shot, and a reload asked for then follows it (and works the bolt at once).

The round that follows is the weapon's from the press, as for every shot (`firing.md`): the animator hears of it at the bolt's mark.

## Filling the magazine (`ReloadInternalMagWithOpenBoltOperation`)

The key and what the idling weapon makes of it are the M3's (`shotgun.md`, "Filling the tube"): rounds from the pockets and the vest, the pockets' first, and **nothing at all if the magazine is full**, whatever the chamber. What differs is the operation:

1. **As it begins** the animator is told as for any filling. **A chambered round is taken out of the weapon at once**; it is thrown out, as a thing lying on the ground, at the chamber's mark (`RemoveAmmoFromChamber`: the animator's `AmmoInChamber` to nothing). No round is put into the chamber here.
2. **The animator's way in** is `RELOAD START` (1.167 s): the bolt opened (`MagShow` 0.232, `ShellEject` 0.492, `DelAmmoChamber` 0.502 if a round was chambered, `OnBoltCatch` 0.546), then the loop `RELOAD IN` (1.533 s a round; `AddAmmoInMag` 0.534 of the way through, delivered only while `CanReload` is up).
3. **`AddAmmoInMag`**: it may now be cut short; one more is on its way in, and `AmmoInMag` says so; `CanReload` is whether another is to follow and it has not been cut short. **It does not end the filling.**
4. With `CanReload` down the loop gives way at its end to `RELOAD CLOSE CHAMBER` (1.167 s): `OffBoltCatch` 0.209, `AddAmmoInChamber` 0.247, the hammer cocked.
5. **`AddAmmoInChamber`**, answered once: the filling ends. The rounds loaded are the magazine's now; **if any were loaded and the chamber is empty, the first of them goes into the chamber**; the animator is told the two counts; the weapon idles.

**Cut short** by the trigger, only once a round has gone in (this operation does not answer the mark of the round shown, where the tube's does); the inventory opening or shutting, and the weapon put away, cut it short at once. The round on its way in still goes in, the bolt is shut, and nothing fires.

So a rifle with four in its magazine and one chambered, filled, throws the chambered round on the ground, takes one round into the magazine, shuts the bolt on it and is at four and one again. Kept as read.

## The magazine changed, or filled (`FirearmHandsInputTranslator.Reload`, `QuickReload`, `ReloadExternalMagazine`; `Idling.Execute`; `ItemContextInteractionsSwitcher`; read and built 2026-10-31)

The reload key, by the weapon's mode as it reads with its magazine:

- **Its magazine comes out** (`ExternalMagazine`, or this mixed mode), **or, by the plain key, none is fitted** (`InternalMagazine` with no magazine): the fullest magazine within reach that the slot takes and that has rounds is looked for first. Found, it is changed for it (`ReloadExternalMagOperation`: the quick way by the quick key, or the plain key stunned, where a magazine is in and the weapon allows it; else the plain way). With a failure to feed and a magazine in, nothing is looked for: it is examined.
- **Not found** (`ReloadMagNotFound`: only the character's "need ammo" phrase, not built), a weapon of a magazine of its own fills the fitted one a round at a time with the bolt open (above; with no magazine, nothing); one of magazines that come out only would put a round into the chamber by hand (`LoadAmmoToChamber`, not built).
- **The quick key** looks for a magazine only where the magazine comes out; with none fitted it goes straight to the filling (which does nothing).

So the rifle with a full magazine in the vest, at two and one, changes its magazine (as the game's recording does at 12.828 s); with no magazine with rounds within reach, it fills. With a magazine in, the inventory may not put a round into its chamber nor take the chambered one out (`Idling.Execute`: "while mag in the weapon"); with none in, it may (`InternalMagazine`, its count nought). The menu's Reload, Load and Unload take it as a magazine weapon while one is in (`InternalMag` is the weapon's mode, not its template's); with a malfunction the shooter knows of they are refused (`InventoryErrors/MalfunctionError`); and any malfunction keeps a magazine from going into or out of its slot (`ItemExtensions`, `IncompatibleByMalfunction`). The same holds for every weapon of this mode: of the install's stock builds, the Model 700, the AXMC, the TRG M10, the SVT-40 and the AVT-40 (`Database::changes_magazines`).

**Built**: `Firearm::magazine_comes_out`, `reload_looks_for_magazine` (by the fitted magazine's `ReloadMagType`, `set_magazine_external`), `can_load_chamber` and `can_discharge` by them; `inventory::reload_mode` and `MIXED_RELOAD` (the menu, the malfunction's refusal); the app's key looks for a magazine first where the game does; `replay`'s scripted reload changes the magazine where the recording carried one. **The app's**: such a weapon starts with spare magazines and its round stacks (`armed`, `Loadout::fit_to`: no longer turned into rounds), and the range's tables have its magazines (`table_loads`).

## Being drawn, and the looks

As `firing.md` and `shotgun.md` have them, by this animator's states: `OUT TO IDLE` (1.0 s), or `OUT TO IDLE ARM` (2.2 s) with rounds in the magazine and none chambered, which works the bolt: **what a rifle found in a container does when it is first taken in hand**. The look into the chamber is `CHECK_CHAMBER` (2.533 s). **The look at the magazine** has two states, by `IsExternalMag`, which the look sets as it begins (`Idling.CheckAmmo`: up for `ExternalMagazine`, the mixed mode, or `InternalMagazine` with no magazine): `CHECK MAG` (3.0 s, the clip by `RelTypeOld`) takes the magazine out and puts it back, its sounds HandOff 0.024, MagRelease 0.167, MagOut 0.219, MagIn 0.713, HandOn 0.929; `CHECK` (3.0 s), with it down, works the bolt, its marks HandOff, BoltOut 0.243, BoltIn 0.662, HandOn, of which the rifle's sound player has no BoltOut or BoltIn. With its magazines this rifle always takes `CHECK MAG`. Built 2026-10-31 (the M700's magazine check had made no sound): the firearm knows whether the fitted and the incoming magazine come out (`FirearmSettings::magazine_external`, `set_magazine_external`, `next_magazine_external`), and the look raises the flag as the game does.

## Built (2026-10-12)

- `trp-sim/src/firearm.rs`: `FirearmSettings::bolt_action` and `open_bolt_reload` (from the item, `trp_sim::firearm_settings`); the bolt's parameter set at each of the moments above; the filling with the bolt open as a second way of `Operation::Shells` (the round taken out as it begins, `Event::RoundEjected` at the chamber's mark, no end at a round's mark, the first round chambered as the bolt shuts).
- The app builds the rifle (`--weapon 5bfea6e90db834001b7347f3`, and it lies in the crate): `R` fills its magazine from the rounds in the pockets and the vest; the round thrown out leaves the shell port by the rifle's own throw of a round (physics spec, "Loose things"; before, it lay on the ground before the character).

## Compared with the running game

**Courses 3 and 4, by hand** (2026-10-16; `local\traces\courses1\3-sv-98`, `4-model-700`, `byhand`): **the SV-98 and the Model 700 on the game's states and times on every frame** (3,896 and 5,176 frames), the shots, the bolt worked, the Model 700's magazine changed. (Written until 2026-10-31 as "filled": the game's reloads in that recording are `ReloadExternalMagOperation`, which `byhand` replays as such, so the filling a round at a time has never been set beside the game.) **The magazine changed**: `byhand` over the three recordings, every reload a change, **0 of 5,176 frames apart** in `4-model-700` (from two and one with a full one in the vest, and from empty), 0 of 3,798 in the `courses2` blind fire, and in `m700-bipod` the change into an empty rifle on the game's states throughout (`RELOAD START EMPTY MAG` to `RELOAD CHARGE END MAG`; `byhand` had begun it with an empty magazine where the game's rifle had none: 679 frames apart before its fix, 71 after, those the scope's magnification key, which `byhand` does not drive). The install test runs both changes from rest on the rifle's animator, each state entered within a frame of the recording's (`the_model_700_s_magazine_is_changed_where_one_is_within_reach`). In the app (`--equip`, `R`): the change from the vest, the states and seven sounds in order. **Not compared**: the filling, the quick change, the menu's lines, the chamber's refusals (no recording has them); the other four weapons are run through a change and a filling (`every_weapon`), never set beside the game. **The look at the magazine**: on no recording; the install test runs it on the rifle's own animator (`CHECK MAG` and its five sounds, after a filling too), and the app's sound log has the five in order at the marks' times.

Before that, with nothing: tried against the install's own controller (`cargo test -p trp-sim --test sim the_model_700 -- --nocapture`), which is what the timings below are:

| What | The simulation |
|---|---|
| the trigger held after a shot | the animator stays in `FIRE`, the logic in its shot, for as long as it is held |
| let go | `FIRE RELOAD` on the next frame, at rest 1.76 s later, four and one |
| a tap | the bolt follows 0.13 s after the press |
| a press while the bolt is worked | lost |
| filled from two and one, twenty rounds within reach | the chambered round thrown out 0.6 s after the key; three loops; five into the magazine and the bolt takes one: four and one, at rest 6.9 s after the key |
| a full magazine | the key does nothing; the chambered round is kept |
| the trigger 1 s into a filling from empty, and again at 2.5 s | the first is lost; after the second the round on its way goes in, the bolt shuts: one and one, nothing fires |
| drawn with five in the magazine and none chambered | `OUT TO IDLE ARM`, appeared 2.2 s after it was asked for |

**Not built:** the magazines that come out, and their reload; a round put into the chamber or taken out of it in the inventory screen (`LOAD ONE`, `DISCHARGE`, `RECHAMBER`); malfunctions; a sight on it (the stock preset has none, so the eye goes to the rifle's own aim bone, 7.8 mm left of the bore and 5.8 cm above it: the game's rule for a build with no sight, `weapon-handling.md`, "The sights"; not compared with the game); what a mounted bipod does to the reload (`TacticalReload` in this operation's start).
