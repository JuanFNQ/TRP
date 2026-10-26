# Blind fire: the weapon held over cover or out to the right

Read 2026-10-25 (session 130) from the decompiled game, from the install through `probe` and from recorder
0.13.0's course 8 (`local\traces\courses6\8-blind-mount-*`: the MDR and the M60E6, steps `01-blind-above`,
`02-blind-right`, `03-blind-refusals`, the M60E6's twice) and recorder 0.10.0's `courses2\4-blind-fire`
(the M700, by hand). The reading before it: `local\blind_mount_s127.md`, section 1. Our own words.
Conventions: Unity's (y up, z ahead); a state's time is its normalised time, in clips played.

## The keys

**Two bindings, each held** (`EGameKey.BlindShootAbove`, `BlindShootRight`; `probe keys blind`: the install
binds `W` + `LeftAlt` and `S` + `LeftAlt`, `Continuous`). The press type cannot be changed (settings spec,
"How it is pressed"). Going down, each gives its command once, `ToggleBlindAbove` or `ToggleBlindRight`;
coming up, `BlindShootEnd`. Nothing repeats while the keys are held: a start that was refused needs a new
press. The movement's axis keeps `W` and `S` (settings spec, "What the bindings do"), and the frame's
commands come before its axes.

**The commands** (`MoveInputTranslator`): `ToggleBlindAbove` asks for blind fire at 1 (over the top),
`ToggleBlindRight` at -1 (to the right), `BlindShootEnd` at 0. Releasing either combination ends blind
fire, whichever side it is at. `Alt`+`W`, then `Alt`+`S` with `Alt` still held, asks for the right side
with no end between; the first combination's end comes when its own keys come up, and ends blind fire
whichever side it is at then. (Which of two held combinations the input manager ends first was not
traced; the scripted recordings give the commands themselves, `cmd:`.)

## Who says yes

1. **The left stance**: with it on, a start is dropped (`Player.ToggleBlindFire`); an end is not. Built
   since 0.127.0 (`left-shoulder.md`): the stance wanted drops a start before the state hears it.
2. **The movement state in hand** (`MovementState.BlindFire` and what overrides it):
   - the standing state and those built on it pass the side on: standing, blind fire itself, the step
     aside, going down to lie and getting up (`IdlePlayerState`; the last three do not override it);
   - lying still, rolled over and crawling pass 0 (`PronePlayerState`, `RollPlayerState`,
     `ProneMovePlayerState`);
   - every other state passes 0 too (the base `MovementState.BlindFire`): the walk, the sprint and its
     stop, a jump and its landings, a vault, a climb, a kick. **A press in such a state ends any blind
     fire** rather than being ignored;
   - the approach before a door ignores it (`ApproachState.BlindFire`, empty); a mounted weapon starts
     to leave its mount and passes it on (built 0.125.0: `mounting.md`, "What ends it").
3. **The gate** (`MovementContext.SetBlindFire(int)`): with nothing in the hands, nothing happens. With
   anything but a firearm in the hands, everything is put to 0. A firearm takes the value if it may
   start blind fire, or if the value is 0; a refusal changes nothing.
4. **The firearm may start it** (`FirearmController.CanSetBlindFire`) only with the operation in hand
   idling (the plain idling or the under-barrel launcher's), not a rocket launcher, and the inventory
   shut. So not in a shot, not through automatic fire, not in a reload, a look at the weapon, a draw or a
   put-away.
5. **Taken**: the movement's own figure is the value; the operation in hand is told
   (`FirearmOperation.BlindFire`: the idling and every fire operation pass the value on, every other
   operation passes 0); the body's animator's `BlindFire` is the value at once. A start (a value not 0)
   also takes away a thing held in the left hand (not built).
6. **The operation's part** (`BlindFire_Internal`): the weapon's blind-fire flag is on for a value not 0;
   with the sights up they go down (`IsAiming` set false: no kit rattle, recorded: none at the aimed
   start, 8.70 s of the MDR's `03`, where the sights coming up at 8.00 s had one); the weapon's motion
   is given the value as its blender's target (`ProceduralWeaponAnimation.StartBlindFire`).

## What it holds back

**While the weapon's flag is on** (`FirearmController`): the aim key, both ways (`SetAim`, `ToggleAim`
return at once); the next sight and a sight by its number (`ChangeAimingMode`); the fire mode
(`ChangeFireMode`) and its look (`CheckFireMode`); the look all over (`ExamineWeapon`); the cylinder's roll;
the magazine's look (`CheckAmmo`) and the chamber's (`CheckChamber`); every reload (`CanStartReload`,
`ReloadMag`, `QuickReloadMag`, the launcher's, the cylinder's, with rounds, the barrels'); the left stance
(`ChangeLeftStance`).

**While the movement's figure is not 0**: the movement keys are not handed on at all
(`MoveInputTranslator.TranslateAxes` skips `Move`; the view's turn, the body's turn and the slow lean go
on), so the walk keys held do nothing and the last direction given stands; a vault or a climb
(`VaultingGameplayRestrictions.CheckBlindFireCondition`); the mount (`Player.TryMountWeapon`; built 0.125.0:
`mounting.md`, "Who may ask").

**Not held back**: the trigger (a shot is the weapon's as ever), the stance keys, the jump (below), the
lean (below), the tactical devices, the breath (which needs the sights up, so it cannot be held).

## What ends it

- **The key let go** (`BlindShootEnd`: the state in hand's `BlindFire(0)`).
- **A press in a state that passes 0** (above).
- **`SetBlindFire(0)` straight from**: the start of a reload of any kind (`BaseReloadOperation`, the
  launcher's), a magazine pulled out or put in through the inventory, a round put in a chamber or a
  cylinder, a discharge, a malfunction looked at or cleared, a part fitted or taken off, the stock
  folded, the bipod, the backpack dropped, the launcher switched, the weapon put away; the inventory
  opening (`Player.SetInventoryOpened`, and the controller's `InventoryOpened`); a gesture of the hands
  beginning (`SendHandsInteractionStateChanged`); the approach before a door (`ApproachState.Enter`);
  lying down (`PronePlayerState.Enter`); a change of what is in the hands (`Player.TryProceed`).
- **A start that is refused does not end it**: the gate leaves everything as it was.

**The lean's quirk** (`Player.ToggleLean`): a lean key pressed (not the slow lean) puts only the body's
animator's `BlindFire` to 0, then leans as ever. The body plays its way out and stands; the movement's
figure, the weapon's flag and the blender stay at the side until the key is let go, so the walk keys are
still not heard, the aim is still refused and the camera keeps its offset. A blind-fire key pressed again
then (the state is standing again) takes it up anew. Read from code; not recorded.

**The jump** (`IdlePlayerState.Jump`, which blind fire's state keeps): stance above 0.6 and on the
ground, the jump is asked of the animator as standing asks it; below, the character stands up. Blind
fire's states lead only to their own ways out, so the jump waits until the body is back in `Idle_Aim`
and then goes. Read from code; not recorded (the recording's jump came first, and blind fire asked in the
jump was refused: 20.10 s of `03`).

## The body

**One movement state, four of the animator's states** (`EPlayerState.BlindFire`, 28; one shared
`BlindeFireState`). `probe bodystates BlindFire Player_anim_controller` (`local\blind_curves_s130.txt`):

| State | part, side | length | speed | curve (`SmoothLerpAnimCurve`) |
|---|---|---|---|---|
| `Stand.BlindFire_Top` | in | 1.000 s | 1.3 | 0 until 0.15, then straight to 1 at 0.65, 1 after |
| `Stand.BlindFire_Top_Exit` | out | 1.233 s | 1.3 | 1 until 0.15, then a cubic to 0 at 0.6 (slope -2.222 at 0.15, flat at 0.6), 0 after |
| `Stand.BlindFire_R` | in, right | 1.000 s | 1.3 | as `_Top` |
| `Stand.BlindFire_R_Exit` | out, right | 1.233 s | 1.3 | as `_Top_Exit` |

All four: turning ×1 up to 99 degrees a frame, things may be looked at to be worked (`CanInteract`), no
first-person pose of their own.

**The ways** (the base layer):
- in: only `Stand.Idle_Aim` leads in, to `_Top` while `BlindFire` > 0 and to `_R` while < 0, with no blend;
- out: `_Top` to `_Top_Exit` once `BlindFire` < 1, blended over 0.1 of the state; `_R` to `_R_Exit` once
  `BlindFire` > -1, over 0.2;
- an exit back in, either side, by `BlindFire`, over 0.25, interruptible (so above to right goes `_Top`,
  `_Top_Exit`, `_R`, never through standing);
- an exit to `Idle_Aim` at 0.9, over 0.1; to `MOVE` from 0.5 if `Inert` (a walk key given after the end),
  over 0.5.

Each clip tree goes by `WeaponTypeFloat` (the rifle's clips at 0, the pistol's at 1) and by `Level`, so the
stance is followed inside blind fire (recorded: crouched at 12.00 s, stood up at 13.00 s in the MDR's
`01`). The clips carry the curves that give the weapon and the arms to the body (`Weapon_3rd`,
`First_Person_Curve_Weight`, `Hand_*`, `Shoulder_*`, `Elbow_*`, `Position_cache`): the weapon is where
the body's clip holds it, the procedural motion on top.

**The float** (`BlindFireFloat`, which the clips' trees and two layers go by; `BlindeFireState`):
- entering from outside (`Enter`, from standing): the float is 0 for a way in, the side for a way out;
  the lean is put to 0 and the sprint off, as standing's own `Enter` does;
- between two of the four (`ProcessStateEnter`: the same object, so `ReEnter` only): nothing is reset
  but the side and the curve, which are the newly entered state's;
- each frame the state is the movement's current one, once for each of its animator states playing:
  `curve(time) × side`, the side 1 over the top and -1 to the right. In a blend the state being entered
  is called last and its time wins. Recorded (MDR's `01`): at the re-entry at 7.301 s the float stayed
  0 until the new `_Top` passed 0.15 (0.0033 at 0.1517), not the exit's 0.316 read on any curve;
- leaving to standing (`Exit` of a way out): the float is put to 0.

The game's animator is told the float when it changes; it takes effect in the next update.

**Two further layers**:
- `Additive_FP_Offsets` (machine 13): from `INACTIVE` to `FP_BlindFire_RibcageOffset TOP` while
  `BlindFire` > 0, or `R` while < 0, and only for the character's own eyes (`!ThirdPerson`); back once
  `|BlindFireFloat|` < 0.01, no blend either way. Its trees by `WeaponTypeFloat` and the float shift the
  ribcage (`blindfire_2h_top_offset`, `_R_offset`, the pistol's `1h`). Both ways in and out are true while
  the float is under 0.01 with `BlindFire` set, so the layer goes in and out on alternate frames then
  (recorded, courses2: frames 107 to 121; course 8's `01`: 1.000 to 1.125 s);
- `Additive_Aiming.Aiming` (machine 5): a 2D tree by (`SidestepFloat`, `BlindFireFloat`) whose points
  (0, 1) and (0, -1) are the blind-fire aim grids by `DeltaRotation` and `Aim_angle`.

**The movement in it**: blind fire's state is a standing state for every frame's work (the turn on the
spot, the body's turn, the motion), with the movement keys never handed on.

## The weapon and the camera

**The blender** (`ProceduralWeaponAnimation.BlindfireBlender`, a `BetterValueBlender`): straight to its
target at the prefab's speed, `Speed: 3` a second (`probe script ... ProceduralWeaponAnimation`,
`local\pwa_player_s130.txt`; the code's own default is 5), read off the frame's clock (`Time.time`): 0 to
±1 takes a third of a second, one side to the other two thirds. On the command's frame it reads where it
was; it moves from the next (recorded: 0 on the frame of `ToggleBlindAbove`, 0.025 a frame of 8.33 ms
after; 1.000 on the frame of the end, 0.975 after).

**The strength** (`_blindfireStrength`, worked out while the blender is off 0, kept otherwise): 1 while the
view's pitch is under 45 degrees either way, `(90 − |pitch|) / 45` past it.

**The camera's rest** (`CalculateCameraPosition`, each frame): the camera rotation spring's zero is the
lean's roll, plus `BlindFireCamera` (-15, -5, 0) over the top or `SideFireCamera` (-2, -15, 0) to the right
(the prefab's; the code's default for the side is (-2, -18, 0)) times `|value| / 2` times the strength, plus
the first-person poses held. The spring's steps of the next frame take it; the camera's turn of this frame
takes it at once. The spring holds a rest twice (weapon-handling spec, "Springs"): at full value the camera
settles at (-15, +5, 0) over the top, the yaw turned by the camera's half turn (recorded, courses2 and
course 8: `cam_rot_zero` (-7.5, -2.5, 0) exactly).

**The weapon's own offsets do nothing**: the prefab's `BlindFireOffset` (0.02, 0.07, 0.16),
`BlindFireRotation` (2, -40, -1), `SideFireOffset` (0.08, 0.02, 0) and `SideFireRotation` (0, 10, 1) are
multiplied by a position and a rotation that are only ever set to zero (`ZeroAdjustments`).

**The camera's pass twice** (`ProcessEffectors`, `LateTransformations`): while the blender is off 0 the
camera's pass (the sight's target, the rest above, `LerpCamera`) runs a second time at the end of the
frame, by the frame's own time (`Time.deltaTime`). The camera's place eases toward its rest twice a frame,
the position spring added each time, and its turn's follow of the recoil is blended twice.

**The sights**: the weapon's motion counts as not aiming while its blender's target is ±1
(`ProceduralWeaponAnimation.IsAiming`).

## The overlap

While the blender is off 0 (`FirearmController.WeaponOverlapping`), the weapon's line is first cast from
the blind-fire end point: `(BlindFireOffset or SideFireOffset by the value's sign, × the strength, + the
weapon's held-closer shift) × 1.9`, in the frame the weapon's moved bone hangs in, down the barrel for the
weapon's length as the plain line. If that is clear (under 0.02 m) the depth is its depth and the plain
line is not cast: a wall in front is passed over. Otherwise the plain line is cast as ever. The weapon's
motion's `OverlappingAllowsBlindfire` is only that depth being under 0.02, and nothing refuses blind fire
by it in this build.

Recorded (the M60E6's second runs, made against something at the right): over the top the depth fell
from 0.756 to 0 on the first frame the blender was off 0 (12.015 s of `01-…-233118`); to the right it came
back at 0.537 once the blender was past -0.95 (1.317 s of `02-…-233136`).

## Firing

A shot in blind fire is the weapon's shot as ever: no factor of spread or recoil for it anywhere
(`ShotFactor`, the recoil's), the round leaving the muzzle where the body's clip holds it, with the recoil
as from the hip. **No sound of its own**: none found in the code, none in the recordings (the shots, the
casings, the impacts only).

## Third person

The character's packet carries the side alone (`MovementInfoPacket`); another's view of it works the float
out from the same curves (`ObservedPlayerBlindFireState`). The ribcage layer is the character's own eyes'
only, so from outside the base layer and the aiming layer's blind-fire grids show. Built by the user's
choice of 2026-10-25 ("Yes, from code"); read from code, not recorded.

## The camera's own object

**The first-person camera does not hang from the ribcage** (found 2026-10-25 with course 8): the player
prefab has `CameraContainer` (whose child `Cam` is the camera, `ProceduralWeaponAnimation.CameraContainer`)
beside `Base HumanRibcage` on `Base HumanSpine3`, at the ribcage's own rest place and turn
(`probe tree .../playersuperior.bundle`, `local\player_tree_s130.txt`), never animated, only turned to the
view (`MovementContext`). The weapon's rig hangs from the ribcage. The two are one place but where a clip
shifts the ribcage along its spine: blind fire's layers that go by its float do, and the recorded eye then
stands 9.5 to 12 cm from where the ribcage would put it (the MDR's `01`, 1.508 s on: game eye less ribcage
(-0.055, 0.124, 0.097) against the ribcage's own (0.037, 0.039, 0.053)), growing with the float from its
first frame off 0. Hung from the container, ours is the game's to 0.1 mm (below).

## Built

(2026-10-25, 0.123.0.)
- **The data**: `trp_data::states::MovementBehaviour::curve` (`SmoothLerpAnimCurve`, shown by `probe
  bodystates` as `curve ...`); `trp_data::player::BlindFireData` (the blender's speed, the two camera turns,
  the two end points) on `HandsData::blind_fire`.
- **The body** (`trp_sim::body`): blind fire's four states and `Additive_FP_Offsets`' `INACTIVE` and two
  ribcage states read (`FP_TurnTorsoOnDeltaRotation` left out); `BlindFire` and `BlindFireFloat` set from
  `BodyInput`; `movement_state::BLIND_FIRE` (28); `Body::blind_fire_curve` by part and side;
  `Body::camera_container`, the prefab's `CameraContainer` posed with the other bones.
- **The movement** (`trp_sim::movement`): `State::BlindFire`, a standing state (`stands`); the commands
  (`Input::blind_toggle`, `blind_end`) passed by the state in hand (`takes_blind_fire`: standing, blind
  fire, the step aside, going down, getting up; the approach none; the rest 0) to the gate
  (`set_blind_fire`: `Input::blind_firearm`, `blind_may_start`); the float each frame by the curve of the
  state last entered at the time of the one being entered; entry (the sprint off, the lean to 0, the float
  by part), re-entry (part and side only), exit (the float to 0); the movement keys not handed on while
  the figure is set; the lean's quirk; the jump, the step aside and the stance as standing's; the vault
  refused; the general speed cap; the ends by lying down, the approach, the inventory's opening and a
  gesture shown. `Movement::blind_fire`, `blind_fire_animator`, `take_blind_fire`, `end_blind_fire`,
  `camera_container`, `camera_frame` (the camera's push out of the world goes by it).
- **The weapon** (`trp_sim::firearm`): `Firearm::can_set_blind_fire`, `set_blind_fire` (what the operation
  passes on; the sights down without `Event::Aimed`), `blind_fire`; refused while the flag is on: `set_aim`,
  `set_fire_mode` and the fire mode key, `check_fire_mode`, `examine`, `check_magazine`, `check_chamber`,
  every reload; `Event::BlindFireEnded` at the start of the fold, the backpack's drop, the put-away and the
  inventory's opening.
- **The weapon's motion** (`trp_sim::hands`): the blender on the frames' clock (a target asked during a
  frame taken from that frame's moment), the strength, the camera rotation spring's rest, the second camera
  pass, the camera's target the sight only with the blender at 0, `Hands::blind_fire_line` and
  `measure_overlap` (the blind-fire line first); `frame_outside` takes the frame's time.
- **The hand-over** (`trp_sim::blind_fire`): `tell` (after the movement's frame: the gate's value to the
  weapon, what it passes on to the motion) and `ended` (an operation's end of it).
- **The switch**: `trp_sim::Systems::blind_fire` (off, the keys are not heard); the options' `BLIND FIRE`
  under `SIMPLE`, after `TACTICAL DEVICES` (the user's choice, 2026-10-25, by multiple choice).
- **The app** (`controller.rs`): `BlindShootAbove` and `BlindShootRight` (the install's `Alt`+`W`,
  `Alt`+`S`) given and come up; a script's `cmd:ToggleBlindAbove`, `cmd:ToggleBlindRight`,
  `cmd:BlindShootEnd` (`--scenario`); the next-sight key held back; the camera hung in its container
  (`Shown::camera_apart`, `hands_view`); third person plays the same states (the user's choice: "Yes,
  from code").
- **The replay** (`bin/replay`): the three `cmd:` presses; `blind` (the figure, the blender), `p_BlindFire`
  (from `bp`), `p_BlindFireFloat` and `cam_rot_zero` set beside the recording's; the eye and the weapon
  as the camera sees it from the container; a reload with no magazine within reach by the recorded
  equipment does nothing (`ReloadMagNotFound`), unless the frames show one fitted.
- **Tests**: `tests/sim/blind_fire.rs` (seven), `systems::tests::blind_fire_is_a_system_of_its_own`.

## Compared with

**Course 8 of `courses6`** (recorder 0.13.0; the MDR and the M60E6, `01-blind-above`, `02-blind-right`,
`03-blind-refusals`, the M60E6's second runs made against something at its right), replayed with
`replay --scenarios hosts/spt-oracle/programs/15-blind-mount` (`local\blind_run_s130.sh`; the second runs
copied under the steps' names to `local\traces\courses6_second_s130`), set beside it with
`local\blind_side_s130.pl`, `local\blind_fates_s130.pl` and `local\blind_shots_s130.pl`
(`local\blind_fates_s130.txt`, `local\blind_shots_s130.txt`, the reports `local\blind_replay_*_d_s130.txt`):
- **Each command's fate**: 62 of 63 by the same rule, refused or taken. Recorded: refused walking (2.00 s of
  `03`), in automatic fire (14.50 s), in a reload (the M60E6's first run, 17.50 s), in a jump (20.10 s);
  taken with the sights up (8.70 s, the sights down, no kit rattle), leaning (11.70 s, the lean put to 0),
  on the way out (7.30 s of `01`: the exit back in), over the top to the right and back without standing
  (6.00 s and 8.50 s of `02`); the reload key that found no magazine (the MDR's, the M60E6's second run)
  left it free at 17.50 s. The miss: the M60E6's second `03` at 2.00 s, where a wall the replay's ground has
  not stopped the game's walk.
- **Frame for frame, gap 0 on every frame** of the eight runs on open ground: the movement state, the
  animator's `BlindFire`, `BlindFireFloat`, the hands' animator's state, the magazine's count (the shots'
  frames), the body's weapon bone, ribcage, pelvis and spine.
- **The blender** (`blind`) within 0.0002; **the camera's rest** within 0.0016 degrees, **its spring**
  within 0.0009; **the camera's turn** within 0.17 degrees, the largest just after a shot, from the hip
  recoil's camera kick (-0.74 against our -0.91 at its peak), not blind fire's; **the eye** to 0.0001 m on
  the MDR's `01` and `02` and the M60E6's `02`, within 8 mm on the rest (crouched and stood up in it).
- **The weapon's moved bone**, held over the top or to the right: within 1 cm (2 to 6 mm rms); **on each
  shot fired in blind fire** within 1.6 to 4.1 mm and 0.5 to 1.3 degrees, the size of the gap outside it
  (the burst in `03`, 1.8 to 5.4 mm, 1.0 to 1.5 degrees; the kept `r17`'s turn 0.5 to 1.3 degrees rms).
- **Sound**: none of blind fire's own in the game's (no source at a command in `sounds` or `plain_sounds`),
  none in ours.
- **Captures** beside the game's pictures (`local\shots\blind_top_beside_s130.png`,
  `blind_top2_beside_s130.png`, `blind_right_beside_s130.png`, `blind_right_bottom_s130.png`; the app
  playing the same scripts, `--scenario`, 2560x1440, field of view 74, frames 677 and 1218 at 2.18 and
  5.46 s of the scripts): over the top, the right forearm across the view at the game's angle and place,
  the view tilted up; to the right, the weapon's end low at the bottom right in both (the game's build
  has tan parts where the stock MDR is black). From outside (`blind_top_outside_small_s130.png`): the
  weapon over the head, not recorded.

**Fixed by the comparison**: the blender read the previous frame's length on an uneven frame (0.068 off at
9.75 s); the eye 9.5 to 12 cm off (the camera's own object, above).

**Open**: **the weapon while the body carries it only in part** (blind fire's way in, 1.00 to 1.15 s, and
the change of side, 8.5 to 8.9 s of `02`): up to 5 cm and 7 degrees apart, with the carrying weight, the
body's weapon bone and the ribcage the game's to 0.0002. What the game blends from then is its procedural
weapon, which it places before the ribcage is turned to the view (`ApplyFovAdjustments`, end of frame);
ours is placed in the ribcage turned to the view. Not changed in this step: it touches the whole first-
person rig. **The overlap's blind-fire line** was not compared: the M60E6's second runs have it (over the
top the depth fell from 0.756 to 0 on the first frame off 0; to the right it came back at 0.537 past
-0.95), but the replay's world is ground only.

**Read from code only, not recorded**: the lean's quirk; a jump pressed in blind fire; lying down, the
inventory, a gesture, the fold, the backpack, the put-away and the approach ending it (the weapon's ends
come to the movement a frame after the game's: after the movement's frame, by the weapon's event); the
weapon's keys refused in it (but the aim: held 5.0 to 6.0 s of `01`, refused in both); the second camera
pass (unseen: the camera rests then); looking far up (the strength); a pistol's clips; third person.

## Not built, or not read

- A stationary weapon: its refusals of blind fire and blind fire's of it wait for it. The mount's and the
  bipod's are built since 0.125.0 (`mounting.md`), the left stance's since 0.127.0 (`left-shoulder.md`).
- A thing held in the left hand, the compass, the under-barrel launcher's mismatch (the body's value set,
  the weapon's flag and blender left at 0), a pick-up in blind fire (standing's, which blind fire's state
  keeps), a bot.
