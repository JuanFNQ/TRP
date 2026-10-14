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

## Compared with the running game

**With nothing.** No recording has any of the five. The regression is unchanged (`after45` is `after44` in every figure): no other weapon's logic moved.

`PROVISIONAL`: what the animator is told of a cylinder's or the barrels' rounds while idle (`AmmoInChamber` is set to the rounds in all the chambers).

**Not built:** the rounds and the cases as things drawn (the chambers show empty); a round or a case put on the ground; the fistful a character who knows the weapon loads into an empty cylinder (`MasteringLevel`), the quick reload that throws everything out, turning the cylinder by hand (`RollCylinder`), taking a round out of a chamber (`DischargeAmmoFromCamoraOperation`); the reload of one barrel (`ReloadSingleBarrelOperation`: the MP-18, the signal pistols); the round left in a barrel put back into the vest when another is loaded; the launcher's grenade as a thing fired, its sight's ranges; these weapons' rounds in the inventory screen (`inventory.md`: the weapon in hand keeps its own count); recoil and handling compared with anything.
