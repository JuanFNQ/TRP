# A character seen from outside

How the game poses and moves a character for eyes other than its own: another player's, a bot's, a camera's. Written from a reading of the game's code (0.16.9.5, SPT 4.1.6 assembly), of the body's controller and of the player prefab in the install; nothing here is copied from any of them. Class and field names are the game's. Read and built on 2026-10-13.

## One class, and a switch on it

There is no second character controller. Every character of a raid is the class `Player`, and `Player.PointOfView` says whose eyes it is made for: `FirstPerson` for the one playing, `ThirdPerson` for everyone else. In this install every other character is a bot, and `LocalGame` makes each of them as a `LocalPlayer` with `EPointOfView.ThirdPerson`. So what another player sees of a character is that character's own simulation with the switch thrown: the same movement, the same body animator, the same weapon animator and logic, the same arm solver.

Three things looked for and set aside:

- **`EFT.NextObservedPlayer`** (`ObservedPlayerView` and some two hundred classes) is how the live game shows a player whose state arrives over the network: nothing in an offline raid makes one. It is not what is described here.
- **The key `ChangePointOfView`** (`Home` in this install's `Control.ini`) is a binding with no handler in this build: `PlayerInputTranslator` has the method that flips the switch, and nothing calls it. Only a debugging owner for several players (`MultilePlayerOwner`) answers the command. The switch itself is a public property and works when set.
- **A third-person camera** for the one playing does not exist either: `ThirdPersonCameraState` is abstract and its one child is the locked camera of `EPointOfView.FreeCamera`. With the switch thrown the camera stays where the eyes were.

## What the switch does (`Player.PointOfView`, `InitVisuals`)

Thrown to third person, in this order:

1. The squeeze of a wide field of view goes: the ribcage's scale is put to 1, and the third-person strategy keeps it there (`ThirdPersonStrategy.ApplyFovAdjustments`).
2. The body's animator is told (`MovementContext.PlayerAnimatorPointOfView`): the flag `ThirdPerson` and the figure `ThirdPersonFloat` go to 1, and **layer 1 (`Weapon`) is given weight 1**. In first person all three are 0.
3. The body's renderers change over (`PlayerBody.UpdatePlayerRenders`): the first-person hands and the watch off, every other part drawn in full and casting its shadow.
4. The weapon's motion takes its third-person strategy and the turn-away its second set of figures (`ProceduralWeaponAnimation.PointOfView`).

And from then on, each frame, `Player.VisualPass` takes its other branches (below).

## The body's animator

The controller's own answer to the flag and the figure, all of it data of the install:

| Layer | For its own eyes | For another's |
|---|---|---|
| `Additive_FullBody` | rests in `INACTIVE_FP` | `Breath`: an additive clip of the chest and shoulders breathing, blended by `Level` (`idle_breathe_low_additive`, `idle_breathe_additive`), at one and a half times its speed; off lying down and in a vault |
| `Additive_BodyTurningOnMove` | rests in `Idle_No_Offset` | `Move_Offset` while moving and `Sprint_Offset` in a sprint: the body leans into its turn, by `DeltaRotation` (and by speed, stance and the way it moves) |
| `Additive_FP_Offsets` | may turn the torso in a sprint and shift the ribcage in blind fire | `INACTIVE` |
| `Sync_SprintHands` | a weapon two cells wide sprints by `sprint_slow_Hands_1st`, a launcher by `Sprint_Hands_Launcher_1st_angled` | by `sprint_slow_0` and `Sprint_Hands_Launcher_3rd` (the tree's blend by `ThirdPersonFloat`) |

Two layers are weighed by the game's code and not by the controller:

- **Layer 1, `Weapon`** (additive, the upper body's bones): the body's own part in what the hands do with the thing they hold. It is driven by one integer, `FirstAction`, which the animator of the thing in hand sets through a mark in its clips, `ThirdAction <number>` (`IEventsConsumerOnThirdAction`, `TranslateAnimatorParameter`): **2 as a weapon begins to leave the hands** (`Weapon.idle_to_out`: the body reaches to put it away, `idle_low_weapon_out` and `idle_weapon_out` by `Level`), **1 as one comes to them** (`Weapon.out_to_idle`, then back to `INACTIVE` at the clip's end); 3 to 15 are a grenade's throw and a blade's strokes. Both states play at `SpeedDraw`, the weapon skill's figure. The stock AKM has the mark 0.008 to 0.010 of the way into `OUT TO IDLE`, `OUT TO IDLE ARM`, `IDLE TO OUT` and `FAST IDLE TO OUT`; the empty hands have it too. **The integer is set whoever is looking**; with the layer's weight at 0 its own eyes see none of it.
- **Layer 2, `UtilityOperation`** (additive): the reach for the vest. Its machine goes to `Idle_Reload_Utility` (or `Prone_Reload_Utility`) while `ReloadFloat` is above 0 and back when it is under 1, at half speed. Its weight is set every frame, **for a character seen from outside only** and within 70 m of the camera (`Player.AdjustUtilityLayerWeight`): while a reload is in hand or the inventory is open, and the character is not sprinting, `inverseLerp(0.5, 0.2, distance)` of the distance between the left hand's marker (`weapon_L_IK_marker`) and the weapon's marker for the vest (`weapon_vest_IK_marker`, a point the weapon's rig carries: on the stock AKM 32 cm to the left of the weapon's bone, 12 cm behind and 29 cm below); otherwise `lerp(weight, 0, 3 * dt)`. A rig with no such marker has the layer at 0. So the torso turns to the pouch as the weapon's own clip takes the hand there. The two distances and the 3 are fields of `Player` with those values in the code (`UtilityLayerRange`, `UtilityLayerLerpSpeed`); the player component is added in code, so the prefab has no say.

Nothing else in the controller asks who is looking. The legs, the stance, the lean, the layer that bends the body with the view (`Additive_Aiming`) and the one that leans it into the sights (`Additive_ISaim`) are the same animation for both.

## Whose arms, whose weapon (`Player.VisualPass`)

In first person every curve by which a body's clip takes an arm or the weapon from the first-person rig is multiplied by one more, `First_Person_Curve_Weight`, which the clips hold at 0 except where the first-person picture is to show the body's own motion (a sprint, a crawl, a vault). **For any other eyes that factor is 1**: the code reads `FirstPersonPointOfView ? GetCurveValue(FIRST_PERSON_CURVE_WEIGHT) : 1`. So (`hands-on-weapon.md`, "Whose arm it is", has the first-person forms):

- **The hands.** `1 - Hand_Left` and `1 - Hand_Right` are the weights of the two arms' solvers and of the left hand's finger pose. Standing, walking and aiming both curves are 0: both hands are solved onto the weapon's markers, as in first person.
- **The shoulders.** `PlayerBones.SetShoulders`, which puts the body's upper arms on the weapon rig's, is called in the first-person branch alone. **Seen from outside the upper arm is where the body's own clip has it**, and the solver works from there.
- **The elbows.** `AdjustElbows(1)`: each elbow's goal is the weapon's own goal blended toward the body's goal bone by `Elbow_Left` or `Elbow_Right` alone.
- **The weapon.** `ThirdPersonWeaponRootAuthority = Weapon_3rd` (0 while mounted), and `PlayerBones.ShiftWeaponRoot` takes its third-person branch: the bone the weapon's motion moves (`Weapon_root_anim`) goes to `lerp(where the first-person rig has it, Weapon_root_3rd_anim's place + its turn * Offset, authority)` and `lerp(its turn, Weapon_root_3rd_anim's turn * DeltaRotation, authority)`, where `Offset` and `DeltaRotation` are what the weapon's motion made of the bone in its own parent this frame. `Weapon_3rd` is 1 in the body's standing, walking and sprinting clips (a tenth less for as much of a side clip of `Additive_Aiming` as is blended in: `body-animator.md`, "An additive layer's curves add"), so **seen from outside the body's own weapon bone carries the weapon all the time**, where the first-person picture has it there only in a sprint. A stationary weapon has the authority at 0.
- **The weapon in the hand.** As in first person: while the right hand's weight is under 1 the weapon is moved to the body's right palm (`PlayerBones.Kinematics`).
- **The rig itself** hangs where it does in first person: at the ribcage's place, turned to the view's yaw and pitch (`MovementContext.DefaultRotationFunction` makes no difference between the two).
- **The ribcage is not turned to the view.** That turn is part of the first-person squeeze (`FirstPersonStrategy.ApplyFovAdjustments`, which also keeps the collarbones in place and scales the ribcage). Seen from outside the ribcage, the collarbones, the neck and the head are where the animation has them.
- **The head** (`PlayerBones.RotateHead(0, GetHeadRotation(), ...)`): the neck and the head are each turned by half of what the free look has the head turned (`NeckChain`, `HeadChain`, both 0.5 on the prefab). The first argument, which would blend in a turn of the neck toward the sights (`R_Neck`, `R_Head`), is always 0. With no free look the head is the animation's. Built the same day with the free look (`free-look.md`).
- **The feet's solver** runs with fewer passes the further off the camera is (`FBBIKUpdate`: `clamp(15 / distance, 0, 2)`).

## The weapon's motion (`ThirdPersonStrategy`)

Each physics step, in place of everything `weapon-handling.md` describes:

1. The turn-away's depth is put to 0 if a first-person pose is held or the mask has no turn-away, then the turn-away runs.
2. If the mask has aiming, the aim pose runs (below).
3. The position spring, the rotation spring and the recoil step.

**Not run:** the reaction to the view's turning and to movement, the force reaction, the breath, the walk's bob, the hand shake, the sway spring, and both of the camera's springs. A shot is felt only if the mask has shooting, and the breath is not told of it (`Shoot`: `Breath.OnShot` is first person's).

Each frame the bone is then placed as in first person (the springs' rests from the lean, the held-closer shift and the poses; `ApplyPosition`, which adds the recoil's kick back) but **turned by `ApplySimpleRotation`**: by the rotation spring alone, about the point the recoil turns the weapon about (`RecoilPivot`) and not the point it swings about, with no sway and **no turn from the recoil at all**. The levelling of the iron sights' line and the turn-away follow as in first person. The camera is not touched (`ApplyCameraTransformations` is empty): it stays where it last stood. A first-person pose of a body state is not assigned (`MovementContext.AssignAnimatorPose` asks for first person).

**The mask** (`EProceduralAnimationMask`). The prefab's is 125: breathing, the two reactions, shooting, the turn-away and aiming; walking is switched at run time. **A bot's is the turn-away alone** (`BotOwner.Create` sets it): a bot's weapon neither kicks nor takes the aim pose. A player's own, seen from outside, keeps the prefab's.

**The aim pose** (`CustomEffector`, on the prefab; its preset is `AimPose`). A figure goes from 0 to 1 at 4 a second while the sights are up and no first-person pose is held, and back when they are down. While it is not 0, each physics step reads two curves at it and adds each curve's figures to a spring as accelerations, divided by the spring's own input scale (so that the scale, which aiming lowers, does not thin them):

- a bump (0 at 0, 1 at 0.35, 0 from 0.685 on): the position spring's y by -0.0066 and the rotation spring's x by -0.33, that second one times its own size again, with the sign of the way the sights are going (`_initAimRotation`): +0.109 as they come up, -0.109 as they go down;
- a rise to 1 at 0.76, held: the position spring's x by -0.002 and its z by +0.002.

So the weapon is held up and to the left of where the body's clip has it for as long as the sights are up, and tips for a moment as they move. Measured in the simulation with the stock AKM: up to 27 mm and 0.8 degrees as the sights come up, and held 28 mm off with them up.

**The turn-away** reads the second pair of figures the prefab has for every component (`AltIntensity`, `AltIntensity1`), has no squeeze in it (the scale is taken as 1) and moves no camera. A rifle is drawn back 0.27 m at most where its own eyes see 0.40; its turn about the barrel's axis is -43 degrees where they see -25.

## What is drawn

- The parts: above. The legs are drawn for both.
- No squeeze, anywhere.
- Levels of detail and culling by distance are the game's economy and change nothing nearer than the first of them: the arms' solver skips the fingers and the forearm's twist beyond 40 m, the right arm beyond 70 m and both beyond 300 m (`IkApply`); the animators stop posing beyond `AnimatorCullDistance` (10 m in this install's settings, and only while out of the picture: the engine's culling mode); the height's lag and the feet's solver are dropped off screen.

## Built

In the simulation (`trp_sim::PointOfView`):

- `Body::set_point_of_view`: the flag, the figure and layer 1's weight; `carried`, `arm_pose` and `held` take the first-person factor as 1, and the shoulder as all the body's. `Body::set_weapon_action` (the mark's number, from `FirearmEvent::BodyAction`, which the firearm and the empty hands now give), `Body::set_draw_speed`, `Body::weigh_reach_for_vest`. The states read for it: `Weapon.INACTIVE`, `INACTIVE_PRONE`, `idle_to_out`, `out_to_idle`; all of `UtilityOperation` and `Additive_FullBody`.
- `Hands::set_point_of_view`: the third-person physics step, `ApplySimpleRotation`, the turn-away's second figures, the aim pose, the mask (`Hands::mask`; a host sets a bot's), `Hands::frame_outside` in place of the camera's frame.
- `trp_data::player::HandsData`: the mask, the aim pose and its speed, each turn-away figure's second pair.

In the app:

- **A view from outside shows the character as others see it**: the free camera (`F4`), the captures `--view side|body|map`, and a new view, **`F5`: playing, watched from outside** (the keys and the mouse stay the character's; the camera stands three metres off, or where the free camera was left, and keeps its place from the character's feet). `--view watch` is that view for a capture, so that `--tap` reaches the character. `--pov first|third` overrides which pose a view gets (`--pov first --view side` is the first-person rig looked at from the side, as every outside capture was before this).
- The drawn ribcage is an entity of its own under the weapon rig's root: left on that root for the character's own eyes (turned to the view and squeezed, as before) and put where the animated ribcage is for another's (`body_view::see_collarbones`). The arms are the same solver's, given the body's arm with the shoulder all the body's.
- What the character looks at goes by where its eyes are (`weapon::Eye`), not by the camera, so a watched character opens doors and searches containers.

In the recorder (0.7.0, built, not run): a key (`\`, the setting `PointOfViewKey`) that throws the game's switch on the player and back, between recordings or in one made by hand; `third_person` on every frame; the animator's `ThirdPerson` and `FirstAction`; the neck and the head.

## Compared with what

**With the game: nothing yet.** No recording has a character seen from outside. Everything above is the game's code and the install's data as read, tried in `crates/trp-sim/tests/third_person.rs` against the install's own controller and prefab:

- at rest and in a walk the body's weapon bone has the weapon whole (authority 1.000) where its own eyes see the rig's; both hands stay the weapon's, both shoulders the body's;
- standing still for four seconds the right upper arm travels 47 mm (the breath), and none for its own eyes; the body's lean into a walk is on, and gone again for its own eyes with the arm back where it rested to 0.00 mm;
- the stock AKM put away says 2 and drawn says 1, and layer 1 goes through `idle_to_out`, `out_to_idle` and back to rest; told the same, a character's own eyes see the left palm move by nothing;
- the view swung a quarter turn a second moves the weapon 9.7 mm and 2.8 degrees for its own eyes and by nothing for another's; a shot kicks it back 26 mm for both and turns it 2.8 degrees for its own eyes only.

The first-person results are untouched: the regression over the sixteen kept recordings is `after42`'s (below, "The regression").

**Seen** in captures (`local\shots\tp_*_s43.png`): standing, the view 45 degrees down and 40 up, a reload with the torso turned to the vest. Standing still the two poses nearly coincide, which is as it should be: the body's weapon bone rests where the first-person rig holds the rifle.

**What a recording would settle**, in the order of what it is worth: recorder 0.7.0, the stock AKM in hand, `\` pressed once before `[`, and the six scripts of `scenarios-hands` or a few of the main set (a walk, a sprint, a reload standing and lying, an aimed shot, a crouch). The trace has the animator's layers with their weights, the curves, the hands' two shares, the weapon's bone and the arms' bones in the player's axes: every figure above can be set beside it. The replay does not yet take a trace's point of view from its header.

## Not built, and open

- **A second character.** The app has one character and shows it from outside. Two at once (a bot, another player) needs the app's rig, body and controller to be made per character; the simulation's types already are.
- **The replay for a third-person trace**: the header's `point_of_view` is not read.
- The grenade's and the blades' states of layer 1; the left-stance layer; a stationary weapon's and a mounted weapon's branches.
- The feet's solver, every level of detail and all culling by distance.
- The weapon's own switch (`Firearms.SetPointOfView`: muzzle effects, tactical devices) and the sounds' (a character seen from outside is heard in the world; its own steps and voice are not placed).
- `PROVISIONAL`: the aim pose's figure runs on the simulation's physics clock, where the game reads it off the frame's time at each of a frame's physics steps; and the turn of the upper body between two of the simulation's frames is drawn as the first-person picture's is (the rig's frame trails the view by a frame, the skeleton is drawn from the frames either side of the moment), which has not been looked at for a view from outside.
