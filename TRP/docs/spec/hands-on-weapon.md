# Hands on the weapon

How the arms come to hold the weapon. Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly) and the weapon and part prefabs. Class and field names are the game's.

## What the clips provide

A weapon's prefab has, beside the weapon itself, two arm chains from the collarbones down to the fingertips. The clips animate them, but the game does not show them: it shows the body's arms and places those by inverse kinematics. From the weapon's rig it takes only:

- Two markers per hand under the `weapon` bone, which the clips move: `weapon_L_IK_marker` and `weapon_R_IK_marker` (where the hand should be) and `weapon_L_hand_marker` and `weapon_R_hand_marker` (a fixed reference for "on the weapon").
- `Bend_Goal_Left` and `Bend_Goal_Right`, also under `weapon`: points the elbows bend toward.
- The finger rotations of the rig's own palms.

Because the markers hang under `weapon`, they follow everything the procedural motion does to it: sway, bob, recoil.

## Grip markers on parts

A part can carry `GripPose` objects: a transform named like a palm, with a hand (left or right), a kind (ordinary, alternative, underbarrel weapon) and a finger pose. The pose is either a stored list of rotations or, when not cached, the rotations of the bones below the object. The stock AKM's wooden handguard has two for the left hand, one ordinary and one alternative; its pistol grip has none.

When a weapon is taken in hand the game looks through everything below the weapon root and picks, per hand, the ordinary grip (or an underbarrel weapon's) that sits deepest in the hierarchy.

## A part on its bone (`ContainerCollectionView.SlotView.InsertItem`, `ModPlacer`; read and built on 2026-10-12, after the user saw a piece of the M3 out of place)

Each part of a build is a prefab of its own, hung on the bone of the weapon, or of another part, that is named for its slot (`mod_stock`, `mod_pistol_grip`, ...). The game puts it there in one of two ways:

- **A part with no word of its own:** at the bone's origin, turned a quarter turn about x from the bone (`Quaternion.Euler(90, 0, 0)`), at its own size. Most prefabs are stored upright with their geometry turned a quarter turn underneath, and the bones live inside that turned space, so the quarter turn stands the part upright again.
- **A part that carries a `ModPlacer`:** at that script's own place, turn (degrees, as `Quaternion.Euler` takes them: about z, then x, then y) and size on the bone. A part made in its bone's own space says so this way.

After that the weapon's animator has the part like any other bone: its clips are bound by path, the part's own name in it, and a clip that has curves for a part's root places it every frame.

**What the install has.** Of the four weapons' twenty-five parts two carry a `ModPlacer`, both the M3's and both with no turn and no shift: its telescoping stock and its forend. The M3's clips have curves for both roots, with the same turn (and the stock's root a centimetre and a half off its bone, which the stock's own `mod_pistol_grip` takes back: the grip hangs at the weapon's `mod_stock`). Its pistol grip is a part of its own that hangs on the stock's `mod_pistol_grip`, has no `ModPlacer` and no curve of any clip: so it is a quarter turn from the stock's bone.

**The rule this replaced** was read off the data before the code was: every part's root was turned to match the weapon's root, which is the same thing for a part on one of the weapon's own bones and wrong for a part hung on a part that is not itself a quarter turn from its bone. It drew the M3's pistol grip a quarter turn round, under the receiver ahead of the trigger guard, with the right hand closed on nothing; the stock and the forend looked right only because the clips put them right every frame.

**Checked against the install** (`a_part_goes_on_its_bone_by_the_game_s_rule`): with the game's rule the M3's grip lies from 2.9 to 11.6 cm behind the trigger's pivot and reaches 11.9 cm below it, and every bone that each weapon's idle clip turns (81, 79, 68 and 74 of them for the AKM, the M3, the Five-seveN and the Model 700) rests within a degree of where the clip first has it, the parts' roots among them; under the old rule the M3's stock and forend rested a quarter turn from their clips. Every other part of the four builds comes out as before to a millionth. **Compared with the game:** not directly (the recorder writes no part's place); the regression over the kept recordings is unchanged in every state and every distance (`after35` against `after34`: only the arms' bone turns move, by hundredths of a degree).

## Each frame

For each hand:

1. `away = distance(IK marker, hand marker)`. `w = inverseLerp(0.1, 0, away)`: 1 while the clip keeps the hand on the weapon, falling to 0 once the marker is 10 cm off, as when the hand leaves for a magazine.
2. If the hand has a grip: the target is the IK marker's position and rotation blended toward the grip's by `w`, and the fingers' grip weight is `w`. Without a grip the target is the IK marker.
3. The arm is solved as a three-joint limb (upper arm, forearm, hand) with the bend goal deciding the elbow's side:
   - the plane's normal is `(bend goal - shoulder) x (target - shoulder)`;
   - the elbow lies in that plane at the two bones' lengths, on the bend goal's side;
   - the upper arm takes the rotation that carries its initial "along the bone, across the plane" frame onto the solved one, so its twist follows the plane;
   - the forearm points at the target and takes its plane normal from its own current rotation, so it keeps the twist the animation gave it;
   - the hand takes the target's rotation outright.
4. Fingers: each finger bone's rotation is the rig's animated one blended toward the grip's stored one by the grip weight.

The left hand has a further blend toward an interaction target (doors, loot) that is not covered here.

## The limb solver, as the game runs it

Step 3 above says where the elbow goes. How each bone is then turned about its own length is the solver's, and it is what the arm's skin shows. Read 2026-10-08 from the solver the player prefab uses (FinalIK's limb solver, one on each collarbone of `playersuperior.bundle`) and the scripts beside it; the figures are the prefab's and are read from the install.

**What it is given.** Three bones: the upper arm, the first of the three forearm bones, and the palm. The palm hangs from the third forearm bone, not the first, so the three are not parent and child in a row, and for such a chain the solver does not keep anything from when it was set up: every update it takes each bone's relation to the bending plane afresh from the pose it finds. Its bend rule is "toward a goal" at full weight (`bendModifier` 4, `bendModifierWeight` 1); it does not hold the palm's turning back (`maintainRotationWeight` 0). Its position and rotation weights are both the hand's figure (`firstPersonHand`, under "Whose arm it is").

**The pose it finds.** The upper arm is where `SetShoulders` just put it: on the weapon rig's upper arm, place and turning, by one less the shoulder's curve (the place by a straight blend, the turning along the arc). Everything below it is the body's own animation: the three forearm bones and the palm each turned in its parent as the body's clips have them, hanging from that upper arm. The weapon rig's own forearm and palm are never looked at.

**One update**, with weight `w`, the shoulder `S` (the upper arm's place, which the solver does not move), the elbow `E` and palm `P` as found, the target's place `T` and turning, and the goal `G`:

1. The plane's normal: `n = (G - S) x (T - S)`, or the last one used if that is nothing.
2. For the upper arm and for the forearm, what to keep: `keep = inverse(look(toward its next joint, n)) * its rotation`, where the next joint is `E` for the upper arm and `P` for the forearm (past the two twist bones), and `look(forward, up)` is the engine's rotation with its z along the first and its y leaning toward the second.
3. Where the palm is to go: `R = lerp(P, T, w)`. The lengths are the ones found: `a = |E - S|`, `b = |P - E|`.
4. The way to the elbow: with `d = |R - S|`, `along = (d^2 + a^2 - b^2) / 2d` and `aside = sqrt(max(0, a^2 - along^2))`, it is `look(R - S, (R - S) x n) * (0, aside, along)`: in the plane, on the goal's side. With `w` below 1 it is blended with the way the upper arm points now, `lerp(E - S, that, w)`.
5. The upper arm is turned to `look(that way, n) * keep`. So it turns about its own length exactly as much as the plane's normal turned about it since the pose was found: the weapon clip's own twist of the upper arm is carried over.
6. The forearm has moved with the upper arm, and so has its normal: `n2 = (its rotation now) * inverse(its rotation as found) * n`. It is turned to `look(R - where the elbow is now, n2) * keep`.
7. The palm's turning goes to the target's by `w` along the arc.

**The twist bones afterwards** (`TwistRelax` on the second and third forearm bones of each arm; run for all four after both arms are solved, the second before the third). Each has a share between the first forearm bone and the palm, 0.2 for the second bone and 0.4 for the third, and two directions stored on the prefab: where its own y axis pointed, at rest, in the first forearm bone's axes and in the palm's. Each update: turn those two back out by the first forearm bone's and the palm's rotations as they now are, blend from the first to the second along the arc by the share, and read that direction in the frame `look(the bone's y, the bone's x)`. The angle `atan2(x, z)` of it, in degrees, is how far the bone is then turned about its own x. The palm is put back where it pointed, since it hangs below. So the forearm's skin turns a fifth of the way toward the palm at the second bone and two fifths at the third, whatever the body's clip had there.

**What follows from it.** The elbow's place depends only on the shoulder, the target, the goal and the two lengths, and is what step 3 of "Each frame" says. The turning of the upper arm about its length comes from the weapon clip's upper arm; of the forearm, from the body's animation of it; of the two twist bones, from neither. A solver that takes the upper arm's twist from the rig's rest pose, or the forearm's from the weapon's clip, puts the elbow in the same place and turns the skin round it differently.

## Whose arm it is

The arms drawn are the body's, and the body's own clips pose them first. What the steps above then do to an arm is weighted, each frame, by curves those clips carry (`Player.VisualPass`, first person):

- **The shoulder.** Each of the body's upper-arm bones is moved, place and turning, toward the bone of the same name in the weapon's rig by `1 - Shoulder_Left` or `1 - Shoulder_Right` (`PlayerBones.SetShoulders`; the rig's bone is as the weapon's clip has it). With the curve at 0, which is the ordinary case, the body's upper arm sits exactly on the rig's. With it at 1 the upper arm stays where the body's clip put it.
- **The hand.** `firstPersonHand = 1 - Hand_Left * First_Person_Curve_Weight`, and the same for the right. That figure is the weight of the arm's limb solver, for position and for turning alike. At 1 the arm is solved onto the weapon as above. At 0 the solver does nothing and the arm, down to the palm, is as the body's clip has it.
- **The fingers.** The left hand's finger pose (step 4 above) is weighted by the left hand's figure too, so with the hand its fingers become the body's clip's. The right hand's finger pose keeps its full weight whatever the curve says: that hand's fingers stay closed on the weapon, which it still carries.
- **The weapon in the hand.** While the right hand's figure is below 1 the weapon is moved to the body's right palm (`weapon-handling.md`, "In the body's hand"), after the arms are solved.
- **The elbow** (`Player.AdjustElbows`, read 2026-10-08). The point each arm's solver bends its elbow toward is not the weapon's bend goal as such. It is a bone of the body's own skeleton, `Bend_Goal_Left` or `Bend_Goal_Right` under the last spine bone, which the body's clips move like any other. After the solver's targets are set and before it runs, that bone is placed, each frame: with `w = First_Person_Curve_Weight * Elbow_Left` (or `Elbow_Right`, each curve taken as 1 at most), if `w` is below 1 the bone goes to `lerp(the weapon's bend goal, where the body's clip has the bone, w)`; at 1 it is left where the clip has it. So with the curve at 0, the ordinary case, the elbow bends toward the weapon's goal, and with it at 1 toward a point the body's animation chooses. (The weapon's goal is first shifted by the turn-away's elbow offsets, turned by the view's yaw and 0.65 of its pitch: up to 0.3 m to the left for both arms as an obstacle pushes the weapon back. Built on 2026-10-09 with the turn-away, `Hands::elbow_shift`; `weapon-handling.md`, "Turn-away". Nothing recorded has them other than zero.)

The crawl's clips have both hand curves and both shoulder curves at 1: both arms are the body's from the shoulder to the palm, and the weapon rides in the right hand. Going down and getting up take the left arm for part of the clip. A sprint with a rifle has the hands' curves at 0 and both shoulders' and both elbows' at 1: the hands stay on the weapon the body carries, while the upper arms sit where the body's clip has them and the elbows bend toward the body's own goals (in the rifle's two sprint clips the left one is 0.22 to 0.34 m, -0.14 to 0.30 m and 0.44 m from the last spine bone along that bone's own axes). The roll lying down has `First_Person_Curve_Weight` at 0 in all three of its clips for either side, and no elbow curve, so by this rule every part of both arms there is the weapon's, the elbows' goals included. Recorded (seventh recording, 38): through the whole roll the body's goal bone is on the weapon's own, in both arms; the two figures differ only by the squeeze along the view, which the weapon's rig is drawn with and the spine's bones are not.

**Compared with the running game** (sixth recording, recorder 0.3.0, which writes the body's upper arms, forearms and palms). On the frames in which the body has an arm whole, the simulation's bones for it (`Body::arm_bone`) are the game's:

| | Upper arm | Forearm | Palm |
|---|---|---|---|
| A crawl forward, back and to both sides (26), both arms | 0.1 mm, 0.06 degrees | 0.1 mm, 0.04 degrees | 0.1 mm, 0.00 degrees |
| A crawl begun and ended with the sights up (34), both arms | 0.4 mm, 0.14 degrees | 1.0 mm, 0.00 degrees | 1.0 mm, 0.06 degrees |
| Going down and getting up with the sights up (35), left arm | 0.0 mm, 0.04 degrees | 0.0 mm, 0.00 degrees | 0.0 mm, 0.00 degrees |

Each figure is the largest gap on any frame. The recorder reads the bones as the frame is drawn, when the game has squeezed everything below the ribcage along the view for a wide field of view; the simulation's are squeezed the same way before they are set beside it. Before the body's aiming layer was fed (`body-animator.md`, "Aiming leans the body into the sights") the arms of 34 and 35 were 4 to 11 degrees and up to 34 mm off.

**The arms on the weapon, compared** (2026-10-09, sixth and seventh recordings; the seventh's recorder also writes the two elbow goals as the game leaves them for its solver). The solver is in the simulation (`crates/trp-sim/src/arms.rs`) and the replay runs it every frame on a rig of its own that plays the weapon's clips (`crates/trp-sim/src/rig.rs`), so every frame of both recordings sets the solved upper arm, forearm and palm beside the game's.

*The solver by itself.* Given the palm and the elbow's goal the game recorded, with the shoulder from the weapon's clip and the forearm from the body's animation as here, on every frame with the hand all the weapon's:

| | Upper arm's turning | Forearm's turning | Elbow's place |
|---|---|---|---|
| The weapon at rest in the hands: 36 scenarios, the roll lying down among them | 0.02 degrees (root mean square), 0.18 at worst | 0.02, 0.10 at worst | 0.0 mm, 0.2 at worst |
| Through reloads, looks at the magazine and switches of fire mode: 13 scenarios | 0.43, 1.9 at worst | 0.13, 0.65 at worst | 0.0 mm |

What is left in the second row is the clip's own timing: the weapon's clips are played by the logic's sequence of states, a frame or so from the game's animator (below).

*The whole chain*, from the simulation's own weapon, shoulder, markers and goals:

| | Upper arm | Forearm (the elbow) | Palm |
|---|---|---|---|
| Standing, leaning, turning, aiming, firing single shots (10, 11, 13, 15, 16, 22) | 0.0 to 0.1 mm; 0.35 to 0.75 degrees, 1.7 at worst | 1.3 to 2.5 mm, 7 at worst; 0.44 to 1.04 degrees | 1.0 to 4.2 mm; 0.45 to 0.85 degrees |
| Walking and jumping (01 to 05, 08, 09, 12, 14, 17, 21) | 0.2 to 0.7 mm; 0.37 to 1.59 degrees, 4.2 at worst | 1.6 to 5.8 mm, 17 at worst; 0.51 to 2.23 degrees | 1.7 to 7.4 mm; 0.55 to 1.49 degrees |
| A sprint, where the body has the shoulders and the elbows' goals (06) | 0.45 and 0.24 degrees, 0.8 at worst | 0.55 and 0.40 degrees, 0.9 at worst | 0.65 degrees, 1.1 at worst |
| The roll lying down, both ways (38) | 0.69 and 0.87 degrees, 2.6 at worst | 2.6 and 2.9 mm, 9 at worst; 0.70 and 0.90 degrees | 4.1 and 3.2 mm; 0.80 and 0.89 degrees, 3.1 at worst |
| Lying, crawling, going down and getting up (25 to 28, 34, 35, 40, 49) | 0.37 to 1.11 degrees, 3.2 at worst | 1.3 to 4.2 mm, 11 at worst | 1.4 to 5.9 mm |
| Reloads, looks, switches (18, 19, 23, 24, 29, 30, 39, 41 to 43, 46 to 48) | 0.31 to 1.88 degrees, 5.8 at worst | 1.2 to 6.9 mm, 26 at worst | 1.2 to 9.0 mm; 0.42 to 2.14 degrees, 7.8 at worst |

Root mean square over a scenario, left arm and right, seventh recording; the sixth gives the same within a few tenths. These gaps are the weapon's own (it breathes and bobs on random numbers, and is 2 to 18 mm and 0.4 to 3.4 degrees from the game's in the same scenarios): the arms follow it. The sprint's positions are left out of the table, and the worst figures of the two sprinting reloads (41, 43) with them, because the recorded ground has a kerb there that lifts everything 4 cm for a few frames. The hands' own shares (`firstPersonHand`) are the game's to four decimal places on every frame of the seventh recording.

*The weapon's clips in the replay.* The two hand markers, read off the replay's rig in the axes of the bone the procedural motion moves, are the game's within 0.1 mm and 0.1 degrees on every frame the weapon is at rest. Through a reload the weapon's own bone is within 3.5 mm, the right hand's marker within 2 mm and the left's, while that hand is off the weapon, within 12 mm, with single frames up to 20 mm where a state changes.

What the comparison turned up:

- **The resting clip was the wrong one** and **the fire-mode switch's two clips were the wrong way round** (`firing.md`, "Which clip a state plays"). The first put the left hand's marker 2.6 cm and the left elbow's goal 6 mm from the game's; the second sent the right hand the wrong way for 1.2 s at every switch.
- **The left elbow in the roll**, which the user reported as sitting wrong on 2026-10-08 and which the rule for the elbow's goal did not explain. The goal was right all along: recorded, the body's goal bone lies on the weapon's own through the whole roll. What differed was the turning of the bones about their length. The app's solver took the upper arm's twist from the rig's rest pose, the forearm's from the weapon's clip, and left the two twist bones as that clip had them, where the game takes the first from the weapon clip's upper arm as it stands, the second from the body's animation, and works the twist bones out afresh. With the weapon turned 70 degrees over, as in the roll, the bending plane is far from where it rests, and that is where the two ways part most. The app now runs the simulation's solver.

## What the app does and does not do

`crates/trp-app/src/arms.rs` reads off its rig what the solver needs (the rig's upper arm as the clip has it, the hand's marker and its fixed reference, the part's grip, the elbow's goal), hands that with the body's arm (`Movement::arm_pose`, carried through `Controller::shown` so that it is blended between the simulation's frames like the rest) to `trp_sim::arms::hold`, and writes the five bones it gets back onto the weapon rig's own arm chains, which is what the arms mesh is skinned to here. The solver, the shoulder's blend, the hand's weight, the elbow's goal and the twist bones are the simulation's, the same code the replay compares with the recordings. The reading and writing in the app are checked by captures only (`local\shots\stand_side_solver.png`, `roll_left_side_solver.png`, `sprint_side_solver.png`, `crawl_side_solver.png`).

- The body's skeleton is not drawn. The arm chains written to are the weapon rig's, whose bones have the body's lengths (0.2965 m of upper arm, three forearm bones of 0.0837 m).
- The left hand's fingers go with the hand: the body's animator is followed for that hand's fifteen finger bones, and each is blended from the rig's (the clip's, closed on the grip) toward the body's by the hand's share. In a crawl the hand lies open on the ground. The right hand's fingers stay the rig's, as in the game, whose finger pose for that hand is not weighted by the curve. Not compared with a recording: the recorder does not write finger bones.
- Not covered: the alternative grip, the left-hand interaction target and the game's holding of the left marker through one (`RestoreIKPos`).
- **The left arm with the sights up in a roll to the left** (reported by the user on 2026-10-09: the left shoulder fills the left third of the view; `local\shots\roll_left_aim.png`). Without the sights the arm is out of view and its bones are the game's (the comparison above, 38). What changes with the sights is the eye, which goes to the sight of a weapon the roll has canted 70 degrees to the left and so comes to sit right over the left upper arm. By the game's code and data that is the game's doing too: the roll's three clips for that side claim no part of either arm (`First_Person_Curve_Weight` is 0 in all of them and none has a shoulder curve), so the upper arm stays on the weapon rig's own, which is fixed in the view's frame, and the aiming eye takes its place from the sight wherever the weapon's pose has put it. **Not recorded:** no scenario so far aims in a roll. Scenario 51 of recorder 0.4.1 does, to both sides; it has the arm's bones and the eye. Nothing has been changed to move the arm: that would be a figure of our own on a point the game can be asked about. The user's decision on it (2026-10-09): do not depart from the game; the arm stays where the game's code puts it, and the recording decides.

  **Recorded** (eighth recording, 51: lying, the sights up, a roll to the left and one to the right with the aim key held). The sights stay up through both rolls, as built, on every frame. The arm is where the simulation has it: the left upper arm within 0.1 mm and 0.4 degrees root mean square (2.8 at worst, as the roll turns over), the solver by itself within 0.06 degrees. What was not where the simulation had it is the eye. The game keeps the camera out of the ground with a box of its own (`weapon-handling.md`, "The camera's collider"), and in an aimed roll the eye, drawn to the sight of a rifle lying canted near the ground, is held up by it. So the shoulder in the view was neither the arm nor a figure to invent: on ground the character stands on, the box lifts the eye 9 cm in the roll to the left, the view goes over the rifle and the shoulder leaves the picture (`local\shots\roll_left_aim_box.png`, beside `roll_left_aim.png` from before). In the recording the lift was only half a centimetre to the left and 3.5 to 6.7 cm to the right, because the recorded character floated 7.6 cm over the ground there.
