# Reactive targets: the hideout range's poppers

The user, 2026-10-26: "can we put the range finder somewhere along with some reactive targets far
away?" The game's own reactive target is the **popper** of the hideout's shooting range: a steel
plate on a hinge that folds back when its face is hit and stands up again. This file is what the
game does with one, read from its code and its install; how the app lays them out is at the end.
The rangefinder: `rangefinder.md` (not built yet).

## What the game has

Read 2026-10-26 from the game's code (`EFT.Hideout.ShootingRange`: `PopperTargets`, `PopperTarget`,
`FoldingPopperTarget`, `HideoutTargetBallisticCollider`, `PaperTarget`, `PopperTargetControl`,
`PopperTargetStandScenario`, `FoldingTargetState`; `EFT.TweenExtensions.AsTask`) and from the hideout
scene `bunker_2.unity` (`level161`), with `probe screen`, `fields`, `raw` and `sceneprops`
(`local\range_*_s138t.txt`). The tween library the poppers move by, DOTween, is not the game's:
its published source was read for the two rules below that are its own (`PureQuaternionPlugin`,
`EaseCurve`). Nothing here was recorded in the game: the recorder has no hideout course.

- **The range's targets.** Three kinds: 18 poppers in three rows of six (level 3's
  `Popper_targets`), paper targets on rails that fold only in the timed training, and paper stands
  that only take holes. Only the poppers answer every hit; they are what is built.
- **One popper** (`Shooting_range_popper_changed (n)`, every one the same but its row): a base
  (`part_01`), a roller (`part_02`) and the plate (`part_03`), each its own object, the roller and
  the plate each turned about its own x axis at its hinge (its place under the popper: the
  plate's 0.400 along the popper's y and 0.089 up its z, the roller's 0.277 and 0.036). The popper's
  y faces the shooter and its z is up (its row is turned −90 degrees about x). Up, both parts stand
  at no turn; down, the plate is turned 86 degrees and the roller 102, the plate falling back away
  from the shooter.
- **What a hit is.** A popper is two ballistic colliders on its plate, both met by bullets as any
  ballistic thing is (the world's shot delegate gives each collider the hit), both `MetalThin` with
  their own figures (penetration level 0, penetration chance 0, ricochet 0.21, fragmentation 0.07,
  deviation chance 0.18, deviation 0.031): a bullet never goes through, and a ricochet off it is a
  hit as much as a stop. **The face**, a flat round disc 0.955 m up the plate from its hinge and
  1 cm before it, whose collider counts a hit by how far it lands from the disc's centre: within
  0.155 m it is a hit on the target (`_sizes` [0.155], segment 0), farther out a miss. **The plate
  and its neck**, a slab 0.31 m across, 1.6 cm deep and 1.11 m long from the hinge, just behind the
  face: every hit there is a miss. A collider reports once a physics step, its nearest segment, if
  it was hit at all (`FixedUpdate`).
- **What the range does with a report** (the default scenario, on as the range is made, off only
  by the player's own switch at the range): a miss **rocks** the popper; a hit **folds** it, then
  after `ResetDelay` (3 s, from the scene) from the end of the fold **unfolds** it. This holds
  whatever the popper is doing: a hit on a fallen plate begins another fold, wait and unfold.
- **A move** (a fold or an unfold) takes each part from where it stands to its goal, over the share
  of the whole move left (its turn still to go over the whole turn, each part its own) of 0.2 s,
  the two parts together; it begins by noting the popper as moving and ends by noting it folded
  or unfolded. Each part's turn is drawn by DOTween: at a time `t` of the move's `d`, the curve of
  the move read at `t / d` times the curve's last key's time, and the part's turn the spherical
  blend from where it began to the goal by that much. The curves (Unity's, every key with the same
  slope in and out, none weighted; time, value, slope):
  - the plate going down: (0, 0, 2), (0.571, 0.856, 0.718), (1, 1, 0); up: (0, 0, 1.132), (0.850,
    0.962, 1.199), (1, 1, 0.251);
  - the roller going down: (0, 0, 2), (1, 1, 0); up: (0, 0, 1), (1, 1, 1).
- **A rock**: both parts to 10 degrees past standing in 0.05 s, then to standing in 0.05 s, each
  straight in time; it does not change the popper's note (folded, unfolded or moving).
- **A new move ends the one under way** where it stands (the sequence killed), and that ends the
  wait of whatever was waiting on it (`AsTask`: a killed tween's task is cancelled, which ends the
  scenario's chain of fold, wait and unfold there). So a hit while the plate falls folds it again
  from where it is, with a fresh wait; a miss while it falls rocks it from there to standing, and
  nothing more comes of the hit; a miss while it lies waiting rocks it up from where it lies, and
  its unfold comes at the end of the wait all the same, from standing (a move of nothing).
- **The sounds** (each popper's own 3D sources under `Shooting_range_popper_audios`; clips in
  `sharedassets161`): a fold plays `shooting_range_popper_fold` or `_fold2` (a draw of 0 or 1:
  `Random.Range(0, 2)`; the third source's `_fold3` is never chosen), an unfold `_unfold`, each at
  the plate's share of its turn left as the move begins, and a rock `_rocking` at full; each stops
  the popper's sound before it. The sources: wholly 3D, the volume falling straight from 1 m to
  nothing at 70 m (the rock's at 65 m), no Doppler. A muted move plays none.
- **Not read**: the order of the face's and the slab's reports when one step holds both; the
  decals the plate's own `DecalSystem` lays (the hit's metal decal is the ballistics'); the
  training's scoring (`ShootTrainingScenario`, `PopperPhaseTraining`), not built.

## Built (0.142.0)

`REACTIVE TARGETS`, a switch of the `SIMPLE` list (on as the app comes; restart required): off,
none is made and nothing of them is read.

- **The model and the sounds from the install** (`trp_data::range_targets`): one popper read whole
  from the scene by the workbench's reader (`scene_props`), its hinges from its parts' places; the
  four clips found by name in the scene's shared file, read by an `AudioClip` tree borrowed from a
  bundle and their data from `sharedassets161.resource`.
- **The behaviour in the simulation** (`trp_sim::popper`): the moves, the rock, the waits and their
  ending as above; the turns by the curves above (`PROVISIONAL` where marked).
- **On the range** (`poppers.rs`), down the lanes (the user's range of 2026-10-26, drawn over the
  map; "Farther, along the lanes"): a row of three poppers 1.6 m apart at each of 25, 50, 100, 200
  and 300 m past the knee-high wall, 6.3 m left of the lanes' line, facing the firing line as the
  game's rows face theirs. The places are the app's (`PROVISIONAL`: the game's rows are 6, 13.6 and
  20 m from its firing line, in a room). The floor drawn 800 m across for them (400 until 0.142.0).
- **Each row's sign** (the user: "Add range markers for each step up, so people can tell how far
  they are"; then "can we make a white sign with black text?" and "just put all the signs on the
  left of each pair of targets"): a white board on a post just left of the row, its distance
  painted on it in black in the app's own dot letters, facing the firing line; 1.2 by 0.6 m at
  25 m and larger as the square root of how much farther (a stronger growth put the far boards
  behind the near ones from the firing line). No solid: bullets pass through it. The app's.
- **The hits**: the face as a disc of 0.155 m met from the front only (Unity's mesh colliders are
  met from their faces' fronts), the slab as a box (both stand-ins for the colliders' meshes:
  `PROVISIONAL`), turned with the plate; a collider's hits counted once a drawn frame, the face's
  before the slab's (`PROVISIONAL`: the game's once a physics step, in an order not read).

## Compared with

Nothing of the game: no recording of the hideout's range. Seen in captures (`workbench.md`, 0.142.0).
Tested: the curves' values at their keys, a fold's and an unfold's length and end, a rock's end,
a second hit mid-fold, a miss mid-fold, a miss while down (`trp_sim::popper`'s tests).
