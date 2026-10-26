# The kinds of weapon the first four are not

Tarkov's weapons counted, and one weapon built of each kind the AKM, the M3 Super 90, the Five-seveN and the Model 700 do not cover (the user, 2026-10-13: one of each, to run around with). Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly) on 2026-10-13, from each weapon's own animator (`probe states <id>`) and its item in the database. **No recording has any of these weapons: everything here is from the code and the install, and what is "seen" below is this simulation on the install's animators.** Class and field names are the game's. `firing.md`, `shotgun.md`, `pistol.md` and `bolt-action.md` hold for these too; this is what is added.

## Every weapon, counted

`probe weapons` reads every weapon item of the database: **163 items, 149 with a stock preset** (the StG 44 among them), in 97 different sets of clips and 91 different sets of states. Which logic a weapon runs on goes by three words of its template (`ReloadMode`, `BoltAction`, the item's class) and nothing else: **128 of the install's 148 run on the four logics already built**, and the other twenty are five kinds:

| Kind | How many | The one built | Item |
|---|---|---|---|
| a bolt action whose magazine comes out | 5 | Izhmash SV-98 | `55801eed4bdc2d89578b4588` |
| a self-loader filled through its action | 4 | TOZ SKS | `574d967124597745970e7c94` |
| a revolver | 5 (the MTs-255 shotgun is one) | Chiappa Rhino 50DS | `61a4c8884f95bc3b2c5dc96f` |
| barrels loaded one by one | 5 | MP-43e-1C | `5580223e4bdc2d1c128b457f` |
| a launcher with a drum | 1 | Milkor M32A1 | `6275303a9f372d6ea97f9ec7` |

The barrels' five are the two MP-43s, and three with one barrel: the MP-18, the FN40GL and the RShG-2, **whose reload is another operation, not built** (and the rocket launcher has a firing of its own). Fourteen items have no stock preset and were not read: the signal pistols, the mounted guns and four others.

**The game picks the firing by the weapon** (`FirearmController.GetOperationFactoryDelegates`): a rocket launcher its own; `ReloadMode` `OnlyBarrel` gets `FireOnlyBarrelFireOperation`; an item of the class `Revolver` gets `FireCylinderMagOperation`; `BoltAction` gets `BoltActionFireOperation`; everything else `FireOperation`. The reload goes by the reload key's own test (`FirearmHandsInputTranslator`): a magazine that comes out is changed; `InternalMagazine` is filled with loose rounds, by `ReloadCylinderMagOperation` if the item is a `Revolver`; `OnlyBarrel` with more barrels than one gets `ReloadMultiBarrelOperation`.

| From the template | SV-98 | SKS | Rhino | MP-43 | M32A1 |
|---|---|---|---|---|---|
| class | sniper rifle | carbine | pistol (a `Revolver`) | shotgun | launcher (a `Revolver`) |
| `weapFireType` | single | single | single, doubleaction | single, doublet | single |
| `ReloadMode` | `ExternalMagazine` | `InternalMagazine` | `InternalMagazine` | `OnlyBarrel` | `InternalMagazine` |
| `BoltAction`, `isBoltCatch` | yes, yes | no, yes | no, no | no, no | no, no |
| the weapon's chambers | one | one | none | two (`patron_in_weapon_000`, `_001`) | none |
| rounds | ten in the magazine | ten in a magazine that stays | six in the cylinder | two | six in the drum |

**Every one of the 149 run whole** (2026-10-14, after the user's "ASVAL causes a crash. can we double check all weapons and ensure it doesnt occur!"). `cargo test -p trp-sim --test sim every_weapon::` assembles each weapon the crates hold, finds its skinned parts' bones in its own tree, and runs its logic on its own animator: found, drawn, aimed and fired, reloaded both ways (a magazine, and the quick reload; loose rounds where the magazine stays in), and put away, each time to rest; all 149 pass. In the app, each was the run's weapon, fired, reloaded, aimed and captured (`local\every_weapon_app_s3g.ps1`): all 149 ran to the end with no error. Only the AS VAL and the VSS have a part drawn on bones of their own, their hammer spring (`findings.md`), which had stopped the renderer.

## Two that needed nothing new

**The SV-98** is the Model 700's firing on the AKM's reload: the bolt is worked when the trigger is let go (`bolt-action.md`), and the magazine is changed (`firing.md`). Its template says the bolt has to be opened for a reload (`MustBoltBeOpennedForExternalReload`), which the logic already answered. Seen: found and drawn, `OUT TO IDLE ARM` (2.02 s, nine and one); a shot, `FIRE`, then `FIRE RELOAD` on the release; reloaded with a round chambered, `RELOAD START`, `OUT`, `IN`, `END` (3.06 s, ten and one); from empty, `OPEN CHAMBER` first and `RELOAD CLOSE CHAMBER` last (5.00 s, nine and one).

**The SKS** fires as any self-loader and is filled as the Model 700 is: with the bolt open (`MustBoltBeOpennedForInternalReload`), a round at a time, **the chambered round thrown out first** (`ReloadInternalMagWithOpenBoltOperation.RemoveAmmoFromChamber`). Seen: `RELOAD START INTERNAL`, `RELOAD IN INTERNAL`, `RELOAD END INTERNAL`; eight and one filled to nine and one in 4.61 s (two rounds in, the chambered one on the ground).

## A cylinder

**There is no chamber apart.** The cylinder is the weapon's magazine (`CylinderMagazine`), and each of its chambers is a slot of it (`camora_000` onward) that holds a round, the case of one fired, or nothing. The weapon keeps which chamber is under the hammer (`CurrentCamoraIndex`) and whether the hammer is down (`Weapon.CylinderHammerClosed`).

- **Which chamber fires** (`GetCamoraFireOrLoadStartIndex`): with the hammer cocked, the one under it; with the hammer down, the next round the cylinder, since the trigger turns it before the hammer falls.
- **A shot** (`FireCylinderMagOperation.PrepareShot`): the round is taken out of its chamber and its case stays there; **nothing is thrown out**. The cylinder's index goes on by one, except for the first shot of double action from a cocked hammer. The animator is told the index (`CamoraIndex`, `CamoraFireIndex`), and from a hammer that is down the state `DOUBLE_ACTION_FIRE` is played outright.
- **After the shot** (the mark `AddAmmoInChamber`): the hammer is left down in double action and cocked again in single action (`DoubleActionFireModeFloat`). Single action shows as `FIRE`, `FIRE ARM`, `IDLE`.
- **A press on a chamber with no round** is a dry press: the cylinder turns all the same where the action turns it (double action with the hammer down, single action with it cocked), and the hammer is left as the mode leaves it.
- **Switching to single action** cocks the hammer (`SetFireModeForCylinderMagazine`): the cylinder turns on if it was down, and the animator's `HammerArmed` is set off.

**The reload** (`ReloadCylinderMagOperation`): at its start the free chambers are listed from the one in turn round, and those holding cases; the animator is told how many cases there are (`ShellsInWeapon`, `AmmoCountForRemove`), which comes out first (`CamoraIndexWithShellForRemove`) and which chamber is loaded first (`CamoraIndexForLoadAmmo`). Then by the marks:

| Mark | What it does |
|---|---|
| `RemoveShell` | one case is out, and the next is named; **with no round left in the cylinder every case goes at once** |
| `AddAmmoInChamber` | a round shows in the hand for the next free chamber, if one is in reach |
| `AddAmmoInMag` | it is in; another follows, or `CanReload` is taken away: no chamber left, no round in reach, or the trigger pressed meanwhile |
| `MagHide` | the cylinder is shut: the rounds shown are the weapon's from now, and with the hammer cocked the first loaded chamber comes under it |

Seen on the Rhino: two cases out one by one and two rounds in, `RELOAD START`, `REMOVE SHELLS`, `RELOAD IN`, `RELOAD END` (7.54 s); from empty `RELOAD START ALL`, `RELOAD IN`, `RELOAD END` (10.11 s, six rounds).

**A drum a spring turns** (`SpringDrivenCylinder`, the M32A1's) differs in three things: its index stops at the last chamber where a revolver's goes round; a dry press does not turn it; and a reload begins again from the first chamber. Seen: six shots chamber after chamber, a seventh press dry, and from empty `RELOAD START ALL`, `RELOAD ARM 5`, `RELOAD IN`, `RELOAD END` (13.32 s).

## Barrels loaded one by one

The weapon has a chamber for each barrel and no magazine. A press fires **the first barrel that holds a round**, and in the mode `doublet` every loaded barrel together (`FireOnlyBarrelFireOperation`); the cases stay in the barrels. Seen: `FIRE`, `IDLE` (0.28 s), two shots from the one press in `doublet`.

**The reload** (`ReloadMultiBarrelOperation`) puts a round into every barrel that holds none, as far as the rounds in reach go. The animator is told which barrel (`ChamberIndex`: the barrel's number, or with every barrel loaded at once their count), how many cases there are (`ShellsInWeapon`) and how many rounds (`AmmoInChamber`). **That count is the rounds once loaded**: the game's inventory puts the new rounds into the barrels before the hands begin (`ReloadMultiBarrelResult.Run`), and the weapon is counted after it. It is this that takes an empty weapon out of its resting state at all: the MP-43's animator leaves `IDLE` for a reload with a round "chambered" or a case in a barrel, and its third way out, for a weapon with neither, asks for counts below zero and can never be taken. Then the marks: `RemoveShell` takes every case out, `AddAmmoInChamber` has the rounds in, `AddAmmoInMag` ends it.

Seen: empty and loaded with two, `RELOAD START`, `RELOAD IN`, `RELOAD END` (4.51 s); with cases in the barrels `RELOAD THROW` comes before `RELOAD IN` (5.78 s).

## Found, and what it is filled from

- **Found in a container** (`Firearm::found`; the user, 2026-10-12: no round in the chamber): a magazine, a tube or a cylinder is full and the chamber empty, so the SV-98 and the SKS work their action as they are first drawn. **Barrels are chambers and nothing else: they are empty**, and the MP-43 has to be loaded. A cylinder has no chamber apart, and is found full.
- **The loose rounds in reach** are those the weapon's chamber takes (`ReloadWithAmmo`), the first of its barrels (`ReloadBarrels`), or for a revolver its cylinder (`ReloadRevolverDrum`, by `CylinderMagazine.CheckCompatibility`): `trp_sim::inventory::rounds_in_reach`.

## Built (2026-10-13)

- `crates/trp-sim/src/firearm/chambers.rs`: the cylinder and the barrels, each chamber `Empty`, `Round` or `Shell`; the two firings and the two reloads above. `firearm.rs` hands them the trigger, the reload key, the fire mode and the marks, and has `found` and `carried`. `Firearm::magazine` counts the rounds in all the chambers, and `chamber` is never set.
- `trp_sim::firearm_settings`: `cylinder` (the chambers of the magazine fitted, and whether a spring turns it) and `barrels` from the database; the modes `doubleaction` and `doublet`.
- `crates/trp-sim/tests/kinds.rs`: the five, each on the install's own animator (the figures above).
- The app: the five lie in the crate with two stacks of their rounds each (`WEAPONS`), `--weapon <id or part of a name>` begins with one in hand. Each drawn, fired and reloaded in a capture; with `B` pressed first, the Rhino's second shot plays `DOUBLE_ACTION_FIRE` and one press empties both of the MP-43's barrels.

## One barrel loaded alone, and the rocket launcher (read 2026-10-14, when the user asked that every weapon be selectable)

Read from `ReloadSingleBarrelOperation`, `ReloadSingleBarrelResult.Run`, the idling operation's `ReloadBarrels`, `FirearmHandsInputTranslator.ReloadBarrels`, `RocketLauncherFire`, `FirearmController.IsAiming` and `SetAimingInAnimator`, `FirearmsAnimator.SetAiming`, `SetAimingIn` and `SetAimingOut`, `Idling.ProcessRemoveOneOffWeapon`, and from the install's controllers of the MP-18 (`61f7c9e189e6fb1a5e3ea78d`), the FN40GL (`5e81ebcd8e146c7080625e15`) and the RShG-2 (`676bf44c5539167c3603e869`). With these three, **every weapon of the install that has a stock preset runs** (148, and the StG 44).

**One barrel** (`ReloadMode` `OnlyBarrel` with a single chamber: the MP-18 and the FN40GL) fires as the MP-43 does (`FireOnlyBarrelFireOperation`, "Barrels loaded one by one"). Its reload is another operation, chosen by the barrels' count (`ReloadBarrels`):

- **The key** takes the rounds within reach that the chamber takes, and with none does nothing. Unlike the MP-43's, it is taken **whatever the barrel holds**: empty, a case, or a live round.
- **The exchange is made as it begins** (`ReloadSingleBarrelResult.Run`): a live round in the barrel goes to the first free place in the vest's and the pockets' grids, the smallest grid first, or with none is taken out to be thrown down; the new round goes into the barrel.
- **The animator is told** (`Start`) how many cases there are (`ShellsInWeapon`), whether there was a live round (`AmmoInChamber` 1, else 0), `CanReload`, and `Reload`. The MP-18's `IDLE` leaves for `RELOAD START EMPTY` with neither, else for `RELOAD START`, which goes on to `RELOAD THROW` for a case or `RELOAD OUT` for a round, then `RELOAD IN` and `RELOAD END`; the FN40GL's has a start for each of the three.
- **The marks**: `DelAmmoChamber` (`AmmoInChamber` as the barrel stands); `RemoveShell` (the cases are out); `ShellEject` (`CanReload` again; the live round thrown down where it found no place; `AmmoInChamber` 0); `MagShow` (the round in the hand); `AddAmmoInChamber` (`AmmoInChamber` as the barrel stands, `CanReload` down, **and the reload is over**: the weapon idles from there, where the MP-43's goes on to `AddAmmoInMag`).

**The rocket launcher** (the RShG-2, of the class `RocketLauncher`; `IsOneoff` is false in this install, and it is `OnlyBarrel` with one chamber, its rocket):

- **Its own firing** (`RocketLauncherFire`, chosen for a `RocketLauncher` before every other test): the trigger starts it; the animator's `Fire` follows the trigger; the sights are put down as it starts; at `FiringBullet` the rocket leaves and the chamber is empty; at `FireEnd` the trigger is let go and the weapon idles. Nothing cuts it short.
- **It fires from the sights only.** Its `IDLE` leaves for `FIRE` (and, further down its list, `QUICK FIRE`) only with `IsAimingFloat` over 0.9, the trigger and a rocket in it; `AIM OFF`, taken on `AimingOut`, comes after `FIRE` in that list, so the sights coming down as the firing starts do not stop it.
- **The sights and the animator**: whenever the sights change (`IsAiming`), the game raises `AimingIn` and lowers `AimingOut` with them up, the other way with them down, and sets `PrevIsAimingFloatState` to 0 or 1 the other way about (`SetAimingInAnimator`); for any weapon but a rocket launcher it also sets `IsAimingFloat` to the sights (`FirearmsAnimator.SetAiming`). The launcher's `IsAimingFloat` is the curve its clips `AIM ON` and `AIM OFF` carry. None of the other weapons built has any of these four in its controller's ways or blends.
- **Its controller has no state to reload in**, and nothing in the game's code plays its `FIRE TO OUT`, whose mark `DropWeapon` would throw it away (a used weapon is thrown away by the idling operation only if it is `IsOneOff`, which this one is not). Fired, it stays in the hands empty. The reload key would start the single barrel's operation, which its animator would never answer: the app gives it no rockets to reload with.

### Built (2026-10-14)

- `trp_sim::firearm::chambers`: the single barrel's reload (the barrel replaced whatever it holds; `Event::RoundUnloaded` for a live round taken out, at `ShellEject`); the rocket launcher's firing (`FirearmSettings::rocket_launcher`); the four parameters of the sights told to the animator on every change, for every weapon (`Firearm::step`).
- The app: the round taken out goes to the vest or the pockets where it has room, else to the ground; **the rocket launcher is found with its rocket** (`Firearm::found`, the app's choice as a full magazine is: it cannot be loaded), and its crate holds no rockets; every weapon with a stock preset lies in the crates of weapons.

### Not built, and open

- The rocket and the launcher's grenade as things fired (the M32A1's likewise), and their sights' ranges (the `Levering` layer's `SightingRange`).
- **Compared with nothing**: no recording has any of the three. The figures are the install's controllers run by the simulation.

## A weapon with no chamber (read and built 2026-10-24, 0.114.1)

Nine weapons list no chamber (`Chambers` empty) and take a detachable magazine: the PPSh-41, the RPD and RPDN, the M60E4, the M60E6 and its FDE, the Uzi, the AA-12 Gen 1 and Gen 2. The game's `Weapon.HasChambers` is false for them, and every step that would touch a chamber is skipped (`FirearmSettings::no_chamber`; the five revolvers' and launchers' cylinders and the RSP-30's barrels are other cases). None of the nine's hands animators has an `AmmoInChamber` at all, and all nine have every malfunction switched off.

- **The trigger** fires while the magazine holds a round (`Idling.SetTriggerPressed`: a chamber's round, or with none, the magazine's count above nought); else the dry click. **The round fired comes straight off the magazine** at the shot (`GetAmmoForSingleShot`, `ProcessInventoryOperationsForSingleShot`: popped to nowhere), and nothing waits in a chamber: the weapon holds its magazine's rounds and no more. The animator is told the magazine's count and never a chamber's.
- **Held on**, automatic fire goes on while the magazine holds a round after the shot (`InternalOnFireEndEvent`); with it empty the run ends and the trigger, still held, clicks once (no bolt catch on any of the nine). A burst goes on only while a chamber holds a round, so it would stop after one shot: none of the nine has a burst.
- **A reload** chambers nothing, and so is not over at `MagIn` (that early end is a chambered weapon's): it ends at the animation's rest (`IdleStart`). Putting a magazine in from the inventory likewise chambers nothing, and ends at `MagIn`. The animator chooses its own charging by `Armed` (the M60's `RELOAD START CHARGE` when not armed; `RELOAD CHARGE END` after `RELOAD IN` when the magazine holds rounds and it is not armed).
- **The draw** takes no round. The weapon keeps its own `Armed` (`Weapon.Armed`, set by the clips' `Arm` and `Disarm` marks) and the animator is given it as the weapon is drawn (`SetAnimatorValues`); the M60's draw arms it (`OUT TO IDLE ARM`) where it is not. A weapon carried from the start is armed; one found is not (`PROVISIONAL`: as one with a chamber is found with it empty; the loot's own `Armed` is not read).
- **A round into the chamber from the inventory** is refused (`Idling.Execute`, the input translator's `Rechamber`, `UnloadChamber`, `LoadAmmoToChamber`): the item has no `patron_in_weapon`. Unloading takes the magazine's rounds alone, as for every weapon.
- **The chamber check** is allowed (no chamber test): the M60's plays `CHECK CHAMBER` (its cover opened, the belt looked at, closed: 4.53 s), as the animator has it.
- **The rounds it takes** are its own magazine's cartridges' (`Chambers` gives none): a kit fitted to it keeps rounds its magazine takes.
- **Not built**: the case thrown at the eject's mark (the round moved to the port and thrown there at once; the app puts every case at the port at the shot); a mark whose condition reads a parameter the animator lacks, which the game reads as nought (`RELOAD CHARGE END`'s `AddAmmoInChamber`, `INV TO RELOAD`'s `OnBoltCatch`): such marks are not delivered here (the reload ignores the first in any case; the second would set `BoltCatch` during a magazine moved from the inventory). The recorder writes no `ammo` for these weapons (it reads the first chamber's round): the replay takes the fitted magazine's last stack from the recorded equipment.

**Compared with the running game** (0.114.1): the M60E6 FDE on `courses4`'s seven basics and `courses5`'s course 6 (kept as `r17`, `r18` in the regression). Before: it fired none of the game's 3, 2 and 19 rounds. Now the magazine's count, the hands animator's state, `Armed` and `Fire` the game's on every frame of all ten scripts; the held fire's 19 rounds 0.11 s apart; the recoil's turn within 0.19 degrees rms on three single presses (largest 0.49), 0.08 (0.17) on two aimed, 0.40 (1.60) on the 2 s held aimed run; the camera's 0.04, 0.08 and 0.24. The checks' four seconds of the chamber check, and the selector's look refused meanwhile, the game's state for state. Not compared: a reload (`04-reload` carried no spare box: nothing happened in the game, and the hands' state there is off on 94% of frames, as before), the other eight weapons (on no recording), the draw's arming.

## Compared with the running game

**Course 3 of recorder 0.8.0, by hand** (2026-10-16; `local\traces\courses1\3-*`, set beside the logic with `byhand`, which hands it the recorded operations, the keys as the game's input manager hears them and the player's clock): **the SKS** (filled through its open bolt, fired, filled again, fired out, dry, filled from empty) and **the Rhino 50DS** in both takes (cocked shots, the switch to double action, its shots, the cylinder filled, dry shots) **on the game's states and times on every frame** (4,584, 4,746 and 1,143 frames), each state's time within a ten-thousandth of a cycle; **the MP-43e-1C** the same up to a switch to the other weapon at 32.5 s (`byhand` follows one weapon). What the comparison put right: a cocked revolver's shot plays its fire state from 0.2, as every single shot (`FireOperation.StartFireAnimation`), where it had gone in by the animator's own transition; and the operations on the player's clock (`firing.md`, "A single shot"). Not compared: the M32A1 (no recording).

Before that (2026-10-13), with nothing: the regression was unchanged (`after45` is `after44` in every figure): no other weapon's logic moved.

`PROVISIONAL`: what the animator is told of a cylinder's or the barrels' rounds while idle (`AmmoInChamber` is set to the rounds in all the chambers).

**Not built:** the rounds and the cases as things drawn (the chambers show empty); a round or a case put on the ground; the fistful a character who knows the weapon loads into an empty cylinder (`MasteringLevel`), the quick reload that throws everything out, turning the cylinder by hand (`RollCylinder`), taking a round out of a chamber (`DischargeAmmoFromCamoraOperation`); the round left in a barrel put back into the vest when another is loaded; the launcher's grenade as a thing fired, its sight's ranges (and the rocket's: "One barrel loaded alone, and the rocket launcher"); these weapons' rounds in the inventory screen (`inventory.md`: the weapon in hand keeps its own count); recoil and handling compared with anything.
