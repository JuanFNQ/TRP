# A bolt action: the Remington Model 700

What a rifle whose bolt is worked by hand does that the AKM and the M3 do not: the bolt is worked after each shot, and only once the trigger is let go; and its magazine, which stays in the rifle, is filled a round at a time with the bolt open, which throws the chambered round out first. Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly) on 2026-10-12 and from the weapon's own animator (`probe states 5bfea6e90db834001b7347f3`; kept as `local\m700_states.txt`) and its item in the database. **No recording has this weapon: everything here is from the code and the install.** Class and field names are the game's. What `firing.md` says of the operations, the marks and the order of a frame, and what `shotgun.md` says of a magazine filled a round at a time, hold here too; this is what is added.

## The weapon as the install has it

Item `5bfea6e90db834001b7347f3`, a `SniperRifle`. From its template:

| Figure | Value | What it does |
|---|---|---|
| `BoltAction` | true | the shot is the bolt action's own operation |
| `ReloadMode` | `InternalMagazine` | the reload key fills the magazine; none is changed |
| `MustBoltBeOpennedForInternalReload` | true | the filling is the one made with the bolt open |
| `weapFireType` | `single` | one mode |
| `SingleFireRate` | 450 | a shot's wait: 0.133 s |
| `CanQueueSecondShot` | false | a press during a shot is not remembered |
| `isBoltCatch` | true | the animator's catch is set by the filling's own marks |
| the stock preset's magazine | the five-round one that stays in the rifle | five, and one in the chamber |

The stock preset has no sight at all. The rifle also takes magazines that come out (the animator has `IsExternalMag` and a second family of reload states for them): a weapon's reload mode is its template's unless the magazine fitted says otherwise (`Weapon.ReloadMode`), and then the key changes the magazine first and fills only if there is none to change. Not built: the preset's magazine is the one that stays in.

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

## Being drawn, and the looks

As `firing.md` and `shotgun.md` have them, by this animator's states: `OUT TO IDLE` (1.0 s), or `OUT TO IDLE ARM` (2.2 s) with rounds in the magazine and none chambered, which works the bolt: **what a rifle found in a container does when it is first taken in hand**. The look at the magazine is `CHECK` (3.0 s), the look into the chamber `CHECK_CHAMBER` (2.533 s).

## Built (2026-10-12)

- `trp-sim/src/firearm.rs`: `FirearmSettings::bolt_action` and `open_bolt_reload` (from the item, `trp_sim::firearm_settings`); the bolt's parameter set at each of the moments above; the filling with the bolt open as a second way of `Operation::Shells` (the round taken out as it begins, `Event::RoundEjected` at the chamber's mark, no end at a round's mark, the first round chambered as the bolt shuts).
- The app builds the rifle (`--weapon 5bfea6e90db834001b7347f3`, and it lies in the crate): `R` fills its magazine from the rounds in the pockets and the vest; the round thrown out lies on the ground before the character.

## Compared with the running game

**With nothing.** No recording has a bolt action. Tried against the install's own controller (`cargo test -p trp-sim --test install the_model_700 -- --nocapture`), which is what the timings below are:

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
