# A pistol: the FN Five-seveN

What a pistol does that the AKM does not: its slide stays back on the last round and is let go by a reload; a press during a shot is remembered; it hangs in the holster; and the body and the weapon's motion are told it is a pistol. Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly) on 2026-10-12 and from the weapon's own animator (`probe states 5d3eb3b0a4b93615055e84d2`; kept as `local\57_states.txt`) and its item in the database. **No recording has this weapon: everything here is from the code and the install.** Class and field names are the game's. Everything `firing.md` says of a shot and of a magazine's reload holds here; this is what is added.

## The weapon as the install has it

Item `5d3eb3b0a4b93615055e84d2`, a `Pistol`, two cells by one. From its template:

| Figure | Value | What it does |
|---|---|---|
| `weapFireType` | `single` | one shot a press |
| `SingleFireRate` | 450 | a shot's wait: 0.133 s |
| `CanQueueSecondShot` | true | a press during a shot is fired after it |
| `ReloadMode` | `ExternalMagazine` | the reload changes the magazine |
| `isBoltCatch` | true | the slide is held back on the last round |
| `weapUseType` | `secondary` | it goes into the holster |
| the stock preset's magazine | twenty rounds | |

It gets the plain `FireOperation` and `ReloadExternalMagOperation` (`FirearmController.GetOperationFactoryDelegates`): the same two classes as the AKM.

## The slide held back

`FirearmsAnimator.SetBoltCatch` sets the animator's `BoltCatch`, `IdlePosition` and the weight of its `Catch` layer (`shotgun.md`, "The bolt caught open"). For a weapon with a magazine that comes out:

- **Caught** by the shot that empties the weapon: the fire state's own mark (`OnBoltCatch`, 0.604 of the way through `FIRE`), which the animator only delivers with a magazine fitted and none left in it. (A weapon in automatic fire has no marks; the logic sets the catch itself three quarters of the way through the cycle that fired the last round: `AutomaticFireOperation`.)
- **A press on it** plays `DRY FIRE CATCH` in place of the dry press.
- **A reload begun with a round chambered** lets the catch go as it begins, unless the weapon's catch is the hand's alone (`ManualBoltCatch`) or its bolt has to be opened for a reload.
- **In a reload** the catch's two marks are answered: caught whenever the mark says so, **let go only the first time**. And the mark of the round chambered lets it go if none has.
- **A weapon that is drawn has its catch off**, whatever it was when it was put away: the controller clears it as the weapon is made, and a draw answers neither mark. So an empty pistol put away with its slide back is drawn with the slide forward.

The animator chooses the reload's last state by the catch: with a round chambered `RELOAD END`; with the slide back and none chambered `RELOAD CATCH END TO IDLE` (0.9 s: `OffBoltCatch` 0.266, `AddAmmoInChamber` 0.375); with the slide forward and none chambered `RELOAD CHARGE END` (1.1 s, the slide racked).

## What tells the body and the weapon's motion that it is a pistol

- **The body's animator** is told the kind of thing in hand (`Player.GetWeaponAnimationType`: a `Pistol` is 1, where a rifle is 0): `body-animator.md`, "The kind of thing in hand".
- **The turn-away from an obstacle** uses the pistol's own shift of the weapon and the elbows (`TurnAwayEffector.IsPistol`: `_pistolShift` in place of `_rifleShift`), which the player prefab has beside the rifle's.
- **It hangs in the holster**, whose key is the install's `SecondaryWeapon` (`1`).

## Built (2026-10-12)

- `trp-sim/src/firearm.rs`: the catch in a magazine's reload (`begin_reload`, the two marks with "off" once, the chambered round's mark) and in automatic fire; the reload tells the animator `IsExternalMag`.
- `trp_sim::hands_settings`: `pistol` from the item's class, which picks the turn-away's shift.
- The app builds the pistol (`--weapon 5d3eb3b0a4b93615055e84d2`, and it lies in the crate); Alt and a click puts it into the holster, `1` takes it in hand, and the body's animator is told a pistol is in hand.

## Compared with the running game

**Course 4's Five-seveN, by hand** (2026-10-16; `local\traces\courses1\4-five-seven`, `byhand`): single shots, a fast series of them with the second press of each pair queued in the first shot, the magazine shot empty and changed, **on the game's states and times on every frame** up to a switch to the other weapon at 20.2 s, after the operations were put on the player's clock (`firing.md`, "A single shot": a queued shot had come a frame early).

Before that, with nothing: tried against the install's own controller (`cargo test -p trp-sim --test install the_five_seven -- --nocapture`):

| What | The simulation |
|---|---|
| the trigger held | one shot |
| a press 0.05 s into a shot | fired when the shot is over |
| the last round | `BoltCatch` up, and at rest 0.08 s after the press; a press on it plays `DRY FIRE CATCH` |
| reloaded with the slide back | `RELOAD START`, `RELOAD OUT`, `RELOAD IN`, `RELOAD CATCH END TO IDLE`: the catch off, nineteen and one, at rest 3.91 s after the key |
| reloaded with a round chambered | `RELOAD END` in its place: twenty and one, at rest 3.41 s after the key |
| put away with the slide back and drawn | `OUT TO IDLE`, the catch off |
| drawn with twenty in the magazine and none chambered | `OUT TO IDLE ARM`, appeared 1.63 s after it was asked for |

**The sights and the skill** (2026-10-12, later the same day). The eye goes to the rear sight's aim bone, on the line through the rear and the front sight: 12.0 mm from the pistol's own aim bone, where it had been put before (`weapon-handling.md`, "The sights"). The draw and the reload go by the level of the Pistols skill (`firing.md`, "The skill's two speeds": drawn in 0.97 s untrained and 0.69 s at level 50, reloaded in 3.41 s and 2.84 s). Both are the game's code as read; neither is compared with the game.

**Not built:** the fast draw of the holster's weapon on a double press of its key, and its faster draw speed (`WeaponPistolFastSwitch...`); spare magazines from the inventory (a reload fits a full magazine, as for the AKM); a round into the chamber by hand when no magazine is found (`LoadAmmoToChamber`); the tactical device's slot; malfunctions; the body's own pose for a pistol in the third-person arms (the body's animator is told the kind, and what it then plays is its own).
