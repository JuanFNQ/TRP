# A shotgun: the Benelli M3 Super 90

What a weapon with a magazine that stays in it does that the AKM does not: a shot followed by the pump, two modes with states of their own and a change between them that takes time, a tube filled a shell at a time that can be cut short, a chamber that is loaded apart from the tube, and a bolt held open on the last shell. Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly) on 2026-10-12, from the weapon's own animator (`probe states 6259b864ebedf17603599e88`; kept as `local\m3_states.txt`) and its item in the database, and then **set beside the sixteenth recording frame by frame** (the last section). Class and field names are the game's. Everything `firing.md` says of the operations, the marks and the order of a frame holds here too; this is what is added.

## The weapon as the install has it

Item `6259b864ebedf17603599e88`. What the logic goes by, all from its template:

| Figure | Value | What it does |
|---|---|---|
| `ReloadMode` | `InternalMagazine` | the reload key fills the tube; no magazine is changed |
| `MustBoltBeOpennedForInternalReload` | false | which of the game's two fillings it gets (below) |
| `weapFireType` | `single`, `semiauto` | the game's fire modes 1 and 5: pumped, and self-loading |
| `SingleFireRate` | 300 | a shot's wait: 0.2 s |
| `CanQueueSecondShot` | false | a press during a shot is not remembered |
| `BoltAction` | false | it gets the plain single shot, not the bolt action's |
| `isBoltCatch`, `NoFiremodeOnBoltcatch` | true, true | the bolt is caught open on the last shell, and the mode is not changed while it is |
| the stock preset's magazine | the five-shell cap | the tube holds five; one more in the chamber |

Its animator has the flag `BoltCatch`, the floats `IdlePosition`, `FireMode`, `AmmoInMag`, `AmmoInChamber`, the triggers `CheckChamber` and `FiremodeCheck`, and a layer `Catch`; it has no `BoltActionReload`, no `IsExternalMag` and no `FiremodeSprint` (the calls that set those do nothing on it).

## Which operation it gets

The firearm controller makes its operations by the weapon (`GetOperationFactoryDelegates`). For this one: the plain `FireOperation` for a shot (a weapon with `BoltAction` gets `BoltActionFireOperation`, which is the one that holds the action back while the trigger is held: not this), and `ReloadInternalMagOperation` for the tube (`ReloadInternalMagWithOpenBoltOperation` is for a weapon whose bolt must be opened first: not this). The online raid's player has classes of its own for both (`ClientFirearmController`); the offline raid's does not use them.

## The two modes are two families of states

The animator's `FireMode` float is the game's own number for the mode, and nearly every way out of `IDLE` tests it: between 0 and 2 for the pumped mode (`FIRE`, `PA RELOAD FIRST`, `PA CHECK CHAMBER`, `PA OUT TO IDLE ARM`, `TO PA`), between 4 and 6 for the self-loading one (`SA FIRE`, `SA AMMO IN FIRST CATCH` and `NO CATCH`, `SA CHECK CHAMBER`, `SA OUT TO IDLE ARM`, `TO SA`). The states of the tube's filling (`RELOAD START`, `RELOAD IN`, `RELOAD END`) and the look at the tube (`MAG CHECK`) are shared.

## A shot

Either mode is one shot a press (`Idling.SetTriggerPressed`: `single`, `doublet` and `semiauto` start a single shot; anything else automatic fire). The single shot is `firing.md`'s, with one thing chosen by the mode (`FireOperation.StartFireAnimation`): the state asked for outright, a fifth of the way in, is `SA FIRE` in the self-loading mode and `FIRE` otherwise.

- **Pumped: the pump is the fire state itself.** `FIRE` is one clip of 0.667 s: the shot (`FiringBullet` at 0), the forend back, the shell out (`ShellEject` 0.453, `DelAmmoChamber` 0.465), the forend forward, the next shell in (`AddAmmoInChamber` 0.573, if the tube has one), `FireEnd` 0.989. Nothing in the logic works the pump; the operation waits, as for any single shot, until its 0.2 s are out and the animator has been back at rest. So a pumped shot holds the weapon for 0.54 s (the state entered at 0.2 of 0.667 s, then `IDLE`), and **the trigger held does nothing more**: the shot's own mark lets the trigger go.
- **A press in the middle of it is lost**, since the weapon queues no second shot: shots cannot follow one another faster than the pump.
- **Self-loading:** `SA FIRE` is a clip of 1 s played five times as fast (0.2 s), with the same marks and one more: `OnBoltCatch` at 0.558, if tube and chamber are both empty.
- **The marks are answered one at a time**, each tested against the animator's parameters as the one before it left them (`AnimationEventsEmitter.EmitEvents`). It matters here: in `SA FIRE` the mark that empties the chamber and the mark that catches the bolt, which wants the chamber empty, can fall in one frame.

## The bolt caught open

`SetBoltCatch` sets the animator's `BoltCatch`, puts `IdlePosition` to 1 or 0 and the `Catch` layer's weight with it. Who answers the two marks (`OnBoltCatch`, `OffBoltCatch`): a shot, on a weapon that has a catch; the idling weapon and a look at it; the filling of the tube. A draw and a put-away do not.

- It is caught by the last shell fired in the self-loading mode. (In the pumped mode nothing catches it: `FIRE` has no such mark.)
- It is let go by the first shell of the next filling (`OffBoltCatch` in `SA AMMO IN FIRST CATCH`, and in `PA RELOAD FIRST`).
- **While it is caught the fire mode cannot be changed** (`Idling.ChangeFireMode`: `IsBoltCatch`, `NoFiremodeOnBoltcatch` and the animator's flag).
- With it caught, a look into the chamber plays `JAM LOOK`.

## Changing the mode

`Idling.ChangeFireMode`, as for the AKM: the mode is the new one at once, `FireMode` is set and `FiremodeSwitch` raised, and the animator plays `TO SA` or `TO PA` (1.167 s each, blended in over a quarter of a second) when it is next at rest. Only the idling operation answers, and a look at the weapon, which is one. **Asked for while the tube is filled, it does nothing at all:** the filling has no answer of its own, and the base one only logs.

## Filling the tube

**The key** (`FirearmHandsInputTranslator.Reload`, `ReloadWithAmmo`). The rounds within reach are those lying directly in the pockets and the vest (`Inventory.FastAccessSlots`, the pockets' first) that the chamber takes. With none, nothing. Else as many stacks are taken up as cover what is wanted: the tube's room, and one more for an empty chamber.

**What the weapon at rest makes of it** (`Idling.ReloadWithAmmo`). The sights go down (below mastering 2) and the trigger is let go; and then the filling **begins only if the tube has room**. A full tube under an empty chamber is refused: the key does nothing (from the code; no recording has it). During a shot the filling is planned and follows it. In a look at the weapon, in a draw, a put-away and a filling already in hand, nothing.

**The operation** (`BaseReloadOperation.Start`, `ReloadInternalMagBase.Start`, `ReloadInternalMagOperation`):

1. **As it begins:** the animator's `Inventory` false, `CanReload` true, `Reload` raised; the body told (`ReloadFloat` 1, and `ProneReload` for as long as the operation is in hand). **An empty chamber is given its round at once** (`Prepare`): the weapon has it from the key's frame, and the animator hears of it at the mark.
2. **The animator's way in**, by its own conditions: with a round chambered, `RELOAD START` (0.667 s at 1.2) and then the loop; with none, the first shell goes into the chamber by a state of its own: `PA RELOAD FIRST` (2.333 s, three variants by mastering), or `SA AMMO IN FIRST CATCH` (2.4 s) with the bolt caught, `SA AMMO IN FIRST NO CATCH` (3.233 s) without.
3. **`MagShow`** (in the three first-shell states): the filling may now be cut short.
4. **`AddAmmoInChamber`** (the same three), answered once: it may be cut short; `AmmoInChamber` is what the weapon has; `CanReload` is whether another shell is to follow; if none is, or it has been cut short, it hands over to idling.
5. **`AddAmmoInMag`** (`RELOAD IN`, the loop of 1.167 s at 1.2: a shell every 0.972 s, the mark 0.579 of the way through, and only delivered while `CanReload` is up): it may be cut short; one more is on its way into the tube, and `AmmoInMag` says so; if none is to follow, or it has been cut short, it hands over.
6. **Another is to follow** while there is a round left to load from and the tube is not full with those already on their way in.
7. **Handing over:** `CanReload` is whether another would have followed and it was not cut short; **the shells loaded become the weapon's now** (`Commit`: until then its tube counts what it had); the idling operation takes over and the animation plays out: `RELOAD IN` leaves for `RELOAD END` (0.533 s) from 0.6 of its loop once `CanReload` is down, and a first-shell state leaves for its own way back to rest (`PA RELOAD FIRST TO IDLE`, `SA AMMO IN TO IDLE`).

**Cut short.** A press of the trigger cuts it short if a shell has shown or gone in; before that the press is lost. The shell on its way in still goes in (the handover is at its mark), and nothing fires: the press is used up. The inventory opening or shutting cuts it short too, and the animator is told of the screen only if a shell has shown; **one opened before that is never told to the animator by this operation** (the property that was to pass it on when cutting short becomes possible sets its field before it tests it, so the test never holds: kept as read). A put-away asked for cuts it short and follows it.

**What it does not answer:** the aim key (whatever the mastering: only the magazine's reload has an answer for it), the fire mode, a second reload, a look.

**In the tail** (the animation playing out, the logic idling) the trigger is heard: this filling does not raise the flag by which a magazine's reload refuses it.

## The looks

- **At the tube** (`CheckAmmo`): as the AKM's look at its magazine; the state is `MAG CHECK` (1.667 s).
- **Into the chamber** (`Idling.CheckChamber`): not with the trigger pressed. Sights down, `CheckChamber` raised, and a look's operation until the state's first mark and then the rest's have come. The animator chooses `PA CHECK CHAMBER` or `SA CHECK CHAMBER` (2 s each; each cocks the hammer, `Arm` at 0.222), or `JAM LOOK` with the bolt caught.

## Being drawn

As `firing.md` has it, with the state by the mode: a weapon with shells in its tube and none in its chamber takes one over as the draw starts, and the animator plays `PA OUT TO IDLE ARM` or `SA OUT TO IDLE ARM` (1.5 s, the action worked, `AddAmmoInChamber` giving it the new counts) in place of `OUT TO IDLE` (1.0 s). **This is what a weapon found in a container does when it is first taken in hand** (the user, 2026-10-12: such a weapon has no round in its chamber; and no stock preset in the install's database has one).

## The stock

The stock preset's stock (`6259c3387d6aab70bc23a18d`, the telescoping one) folds: its own template says so (`Foldable`), where the weapon's does not, and names nothing that would have to be fitted for it (the weapon's `FoldedSlot` is the stock's slot). The other stock the M3 takes blocks folding (`BlocksFolding`). Read from the game's code on 2026-10-12; **no recording has a fold.**

**The key** (`FoldStock`, `L` with `Left Alt` in the install; `FirearmHandsInputTranslator`). It asks only if the weapon has something that folds, fitted, and no part that blocks it (`ItemManipulator.CanFold`). The fold is then one of the inventory's own operations on the item in hand, which the hands are asked to carry out (`Player.TrySetInHands`): **only a weapon whose logic idles takes it**, and not one in a look at it (`FirearmController.CanExecute`); anywhere else the key does nothing.

**The operation** (`FoldStockOperation`):

1. As it begins the trigger is let go, the animator's `Inventory` is put down, the body is told the hands are busy, and the animator's trigger `StockSwitch` is raised. From its resting state the animator plays `STOCK FOLD` if the stock is out and `STOCK UNFOLD` if it is folded (1.333 s each).
2. **At the state's own mark** (`FoldOn` 0.694 of the way through the fold, `FoldOff` 0.05 of the way through the unfolding) the stock stands the new way: the animator's `StockFolded` and the weight of its `Stock` layer, which holds the stock's three bones folded; the animator is told how the inventory stands; and the logic idles again while the animation plays out.
3. Through it the trigger, the aim key, a reload and the looks do nothing. The inventory opening is only noted. Put away meanwhile, the weapon goes when the mark has come.

**What a folded stock changes** (`Weapon.RecoilDelta`, `CompoundItem.CalculateExtraSize`, `ProceduralWeaponAnimation.CheckShouldMoveWeaponCloser`):

- **The recoil is without the stock's share.** The parts' sum of `Recoil` leaves the stock's out: for the stock M3, -0.28 becomes -0.07, so the parts' factor on both of the recoil's strengths goes from 0.72 to 0.93.
- **The weapon is a cell shorter** (the stock's `SizeReduceRight`): five cells become four, which is also what the body's animator is told its width is.
- **The weapon is held closer.** A weapon of compact handling (`CompactHandling`: the M3 is one) with nothing on it that reaches the shoulder (`HasShoulderContact`: the stock has it, and folded it does not count) rests 5 cm nearer along its own y and swings about another point (`RotationCenterNoStock`: for the M3 0, -0.25, -0.06 in place of 0, -0.1, -0.03). The settings object has the switch for it on (`OFFSET_FOLDED_WEAPON`). **This is worked out when the fold ends, and only with the sights down at that moment; it is not looked at again when they next come up or go down** (as read: the check asks for the sights and is called from nothing that aiming calls).
- The ergonomics do not change.
- A weapon keeps its fold while it is put away: drawn, the animator is told how it stands.

## Built (2026-10-12)

- **Later the same day, after the user ran it:** the eye is lined up with the ghost ring and the front sight (`weapon-handling.md`, "The sights"); it had gone to the shotgun's own aim bone, 13.3 mm from the sights' line, which with the stock folded put the eye inside the collapsed stock's pad. And the pistol grip, a part of its own that hangs on the stock, is put on its bone by the game's rule (`hands-on-weapon.md`, "A part on its bone"); it had been drawn a quarter turn round, under the receiver ahead of the trigger guard.
- `trp-data/src/weapon.rs`: `Weapon::fold` (the build folded: the parts' recoil, the width, whether it is then held closer) and `held_closer`. `trp-sim`: `Firearm::fold_stock`, `Operation::Fold`, `Event::Folded`, `start_folded`; `Hands::set_folded` (the recoil's strengths, the closer hold with its rest and its swing point). The app: `L` with `Left Alt`, as the install binds it.
- `trp-sim/src/firearm.rs`: `FireMode::SemiAuto`; the fire state by the mode; `Operation::Shells` with `reload_shells` (the key, with how many rounds are within reach), its marks, its cutting short by the trigger, the inventory and a put-away, and `Event::RoundsTaken` for what leaves the character's stock (one for an empty chamber as it begins, the rest as it ends); `set_fire_mode` with the bolt's refusal; `check_chamber`; `set_bolt_catch` and `bolt_caught`; `start_in_mode` and `start_with` for a replay that begins where a recording stood. **Marks are now answered one at a time** (`WeaponAnimator::deliver_next`), for every weapon.
- `FirearmSettings`: `semi_fire_state`, `internal_magazine`, `no_fire_mode_on_bolt_catch`, read from the item (`trp_sim::firearm_settings`).
- `trp-sim/src/inventory.rs`: `rounds_in_reach` and `take_rounds` (the pockets and the vest, the pockets' first).
- The replay takes the weapon a recording was made with from its first line, the fire mode, the hammer and the bolt from each scenario's first frame, and the rounds within reach from the recorded list of what was carried; it sets ten of the weapon animator's parameters and the fire mode beside the recorded ones.
- The app: `--weapon 6259b864ebedf17603599e88`; `R` fills the tube from the shells in the pockets and the vest, which leave them; `B` changes the mode; `T` with `Left Shift` looks into the chamber (the install's binding). The crate's kit for a weapon with a tube has stacks of its shells where the rifle's had magazines.

## Compared with the running game

**The sixteenth recording** (`local\traces\recording16`, recorder 0.6.2, 2026-10-12 15:24 to 15:27, the nineteen scenarios of `hosts\spt-oracle\scenarios-m3`, 18,864 frames; `replay --scenarios hosts\spt-oracle\scenarios-m3 --traces local\traces\recording16`). **In all nineteen, on every frame, these are the game's:** the animator's state, the state it is entering, whether it is between two, how far through its state it is (to four decimals); ten of its parameters (`Active`, `Inventory`, `Fire`, `AmmoInMag`, `AmmoInChamber`, `CanReload`, `BoltCatch`, `Armed`, `FireMode`, `IdlePosition`); the chambered round and the tube's count; the fire mode; the operation in hand; the sights; and the body's two flags for a reload.

| Scenario | What it does | What the game did, and the simulation with it |
|---|---|---|
| `70`, `76` | two pumped shots, hip and aimed | each `FIRE` from 0.2 and `IDLE` 0.533 s later |
| `72` | the trigger held 3 s, pumped | one shot |
| `74` | four presses 0.3 s apart, pumped | two shots: the second and fourth presses fall in a pump and are lost |
| `71`, `73`, `75`, `78` | the tube topped up, pumped (hip, aimed, again) and self-loading | `RELOAD START`, the loop a shell at a time, `RELOAD END`; the tube's count the weapon's at the end; with the aim key held (73) the sights go down at the key and stay down |
| `77`, `83` | the mode changed to self-loading and back, then fired | `TO SA`, `TO PA`; the shot by the new mode's state |
| `79`, `80` | self-loading, held 2 s; three presses 0.12 s apart | one shot; (80) the presses that fall inside a shot's 0.2 s are lost |
| `81` | self-loading until empty, and on | three shots; the bolt caught on the last shell (`BoltCatch`, `IdlePosition`, 0.08 s after the press); the first dry press plays `DRY FIRE` for as long as the trigger is down (0.06 s: the state leaves as it comes up) and lets the hammer down, the four after it `DRY FIRE DISARMED` (0.333 s each) |
| `82` | filled from empty, self-loading, the bolt caught | `SA AMMO IN FIRST CATCH`, the catch let go, the loop, `RELOAD END` |
| `84` | pumped until empty, and on | five shots, no catch; `DRY FIRE DISARMED` on each of the three dry presses (the hammer had been down since the dry presses of 81: nothing in a filling or a pumped shot cocks it but the first-shell state of the pumped mode) |
| `85` | filled from empty, pumped | `PA RELOAD FIRST`, the loop, `RELOAD END` |
| `86` | three shots, a filling, the trigger 2.1 s into it | the filling of three shells ends full on the frame its last shell's mark comes, and nothing fires. **The cut itself is not shown:** the press came with the last shell already on its way, so the filling ended where it would have anyway |
| `87` | the look at the tube, the look into the chamber | `MAG CHECK`; `PA CHECK CHAMBER` |
| `88` | the reload key with the tube full and a shell chambered; then the mode changed, and back | **the key does nothing** (no operation, no state: a full tube is not filled); `TO SA`, then `TO PA`, each begun a quarter of a second after its key (the blend in). The script was written to change the mode in the middle of a filling; with the tube full there was none, so that is still from the code |

**Written from the code and the install, the recording changed nothing in it.** The first replay had every one of those values right in seventeen scenarios; the two fillings from empty (82, 85) did nothing, by a fault of the replay's own (it counted the rounds within reach by the kind of round the trace's first line names, which is none when the weapon is empty; it now goes by the chamber's own filter).

**Also on record** (`local\traces\byhand3`, sixteen recordings made by hand with the M3, 8,084 frames): the screen opened and shut with it in hand (`OPEN INVENTORY`, `IN INVENTORY` 0.667 s later; `CLOSE INVENTORY`, `IDLE` 0.667 s later), put away and drawn by a slot's key, drawn under an open screen, and taken out of a container with Alt and a click: the animator's state is the game's on every frame of all sixteen.

**The weapon's motion** in the same nineteen: the rig the animator poses has the weapon's own bone within 0.06 degrees of the recorded one and both hand markers within 0.08; the arms' solver, given the game's goals, is within 0.08 degrees. The springs and the recoil compare as the AKM's do, in size and not shot by shot (they draw random numbers): the weapon is within 6 mm and 1.3 degrees through a filling, 15 mm and 3.3 degrees through the pumped shots.

**From the code alone:** the stock's fold, whole (`cargo test -p trp-sim --test install the_m3_s_stock -- --nocapture`: the stock stands folded 0.94 s after the key and the weapon is at rest 1.34 s after it; unfolding, it stands out 0.08 s after the key; nothing fires meanwhile; put away in the unfolding, the weapon goes when the animation is over; put away folded, it is drawn folded); a filling cut short by the trigger (86 did not catch it), by the inventory or by a put-away; a full tube refused *under an empty chamber* (88 has it with a shell chambered); one shell within reach; a filling planned in a shot; the fire mode asked for in a filling; the fire mode refused with the bolt caught; `SA AMMO IN FIRST NO CATCH`; `JAM LOOK`; the draw that works the action. Each is tried against the install's own controller (`cargo test -p trp-sim --test install the_m3 -- --nocapture`: cut short 2 s into a filling from an empty tube, two shells are in and nothing fires; `PA OUT TO IDLE ARM` has the weapon appear 1.508 s after the draw is asked for).

**Not built:** several pellets to a shot (`ShotgunDispersion`, `shotgunDispersion`: there are no ballistics here yet); the folded stock in the inventory's picture of the weapon and in its cells there (the model's weapon stays five cells); a shell loaded into the chamber, the chamber emptied and a shell rechambered from the inventory screen (`LOAD ONE`, `PA DISCHARGE`, `SA DISCHARGE`, `PA RECHAMBER`); the fistful and the two and four shells at a time (`AMMO IN FISTFUL`, `TWIN LOADING`, `QUAD LOADING`: no way into them was read); the look at the fire mode (`FIRE MODE CHECK`, `B` with `Left Alt` in the install); malfunctions; the other magazine caps (seven shells and the extensions); the shell as a thing in the hand and in the chamber. In the app the shells in the weapon are not things of the inventory, so the carried weight is under the game's by what the tube and the chamber hold.
