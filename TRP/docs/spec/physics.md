# Physics: the engine, a corpse, things let fall and thrown

What the game hands to its physics engine, and how that engine is set up. The character's own movement is not this: the player's controller is the game's own code (`movement.md`). A corpse's ragdoll and the weapon it lets go are told in `health.md` ("Death"); here is how both, and every loose thing, go through the engine.

Read from the decompiled game (`PhysicsExtensions`, `Corpse`, `CorpseRagdoll`, `GameWorld.ThrowItem`, `LootItem`, `ObjectsFactory.CreateLootPrefab`, `Player.FirearmController.DropMod`), the install (`globalgamemanagers`, the hard settings, the items' prefabs) and, for the corpse, measured against the user's recordings. In this work's own words.

## The engine

The game's Unity is 2022.3.43f1, whose physics is **PhysX 4.1**. This work runs the same engine: the crate `trp-physics` binds PhysX 4.1.1 through `physx-sys` 0.8.2 (the last of those bindings on PhysX 4; built from PhysX's source by the C++ compiler, about five minutes the first time). How Unity drives PhysX is not published; what this does with each Unity setting is written beside it in `trp-physics/src/lib.rs`, and the corpse's results say whether it holds.

**The settings** (the `PhysicsManager` of `globalgamemanagers`, Unity 2022.3's layout; `probe layers` prints them):

| Setting | Figure |
|---|---|
| Gravity | (0, -9.81, 0) |
| Bounce threshold | 2 m/s |
| A new body's most speed out of what it is in | 10 m/s |
| Sleep threshold | 0.005 |
| Contact offset | 0.01 m |
| Solver iterations, for position and velocity | 3 and 1 |
| A body's most spin | 7 rad/s |
| Contacts | persistent manifolds (PCM); no adaptive force |
| Friction | patch; no improved patch friction |
| Solver | PGS; no enhanced determinism |
| Layers | 32, named in the `TagManager`; which meets which by the matrix |

**Layers that matter here**: `Deadbody` (23) meets itself, `Terrain` (11), `HighPolyCollider` (12), `Shells` and `TransparentCollider`. `Loot` (15) meets `Default`, `Terrain`, `HighPolyCollider` and `TransparentCollider`: **not itself, not a corpse, not the character**. So a loose thing falls against the world alone.

**How a Unity body is laid on PhysX here**: the layers are PhysX's groups, meeting as the matrix says; a pair whose layers meet touches, a trigger is told and pushes nothing, and a body that asks for continuous detection is swept against what it meets (Unity's own filter is not published: this is the plain reading of it). Every shape has the contact offset and no rest offset. A body takes its mass and works out its centre of mass and its turning from its colliders (Unity's `Rigidbody` with neither set by hand); triggers count for nothing there (`PROVISIONAL`). Its drag and angular drag are PhysX's damping; its most spin, its most speed out of what it is in, its solver iterations and its sleep threshold are the settings' unless the game sets its own. A capsule no taller than its caps is a sphere (Unity makes it one). A convex mesh collider is the hull of its mesh's points, scaled as its object is, cooked by PhysX's own defaults (`PROVISIONAL`: Unity's cooking options are not read); one that is not convex never moves (Unity will not move a body with one) and is left out of a moving body.

## The step (`PhysicsExtensions.Simulation.SmoothSimulate`)

The game steps its physics by hand, **once a drawn frame**, by the **average of the last 600 frames' times**, at most `MaxSmoothPhysicsDeltaTime` (0.03 s), and only while a rigid body it looks after exists. The recordings bear it out: a corpse moves on every frame of 1/120 s. The ring of 600 is kept by the game from long before a raid; here it is the average of the frames there have been, fed from the run's start (`trp_physics::corpse::Stepper`). Each world here is stepped on one thread, so that the same start gives the same fall.

## The corpse

The bodies and joints are the player prefab's (`health.md`, "The ragdoll"). Unity's `CharacterJoint` is PhysX's six-way joint (`D6`): the three ways along locked, the twist about the joint's axis between its limits, the two swings each within its own limit, a swing of nothing locked. Where Unity's laying of it is not known, the recordings settled it (`bin/corpse --conventions`, `--angles`):

- the corpse's own body is PhysX's first and the one it hangs from the second, the twist not negated; the first swing is about the swing axis;
- the mass scales go to PhysX as the game sets them (the connected body's mass over its own), not inverted;
- **the limits count from the skeleton's rest** (how the two bones stand in the prefab), not from the pose at death: the elbows stop at 25 degrees either side of their rest, the knees within -10 to 80, as the recordings show;
- projection on, at 0.1 m and 180 degrees (`PROVISIONAL`: Unity's defaults, the game not setting them); preprocessing off.

Every body starts with the player's velocity at most 2 m/s, pushed out of what it is in at 4 m/s at most; a shot's push comes on the frame after the corpse is made, on the body whose own collider was hit (`health.md`).

**The weapon let go** has its own colliders (its prefab's, which the game switches on; triggers and those on the `Shells` layer left out), on a spring from the pelvis that is PhysX's distance joint with its spring on and no slack, or a pistol held fast to the right forearm (`health.md`).

## Loose things

**Thrown** (`GameWorld.ThrowItem` with no direction: a thing put down from the inventory, a backpack off the shoulders). From the character's capsule, on its axis at 0.65 of its height over the feet (`PlayerColliderPointOnCenterAxis`), moved on by the character's velocity times the frame's time; at 2 m/s along the look turned up by 45 degrees (`Quaternion.Euler(pitch, yaw, 0) * (0, 1, 1)`, the pitch held between straight up and 45 down), plus half the character's velocity; turned as the weapon's root (`PlayerBones.WeaponRoot`) and then 90 degrees about its x; spun at random up to 3 rad/s about x and y, and 2 about z one way or the other (`Mathf.Sign` of a whole number from -1 to 1, which gives the positive way two times in three). It is not drawn for its first `ThrowLootMakeVisibleDelay` (0.15 s). Thrown down (`ThrowItem` with `downDirection`, the inventory's unloading of a magazine with no room for it) the direction is straight down instead.

**Let fall** (`DropMod`, at `OnMagPuttedToRig`: the quick reload's old magazine). Where the weapon's magazine is at that moment (`Firearms.GetModTransform`: the first object under the weapon's `mod_magazine`, found from its root), carried from the first person's drawn body to the body itself (`BifacialTransform`), falling at 1 cm/s with nothing of the character's velocity, spun at random up to 3 rad/s about each axis. The weapon draws no magazine from then (`Firearms.RemoveMod`) until the next appears in the hand (`OnMagAppeared`, the mark `MagShow`).

**The body** (`LootItem.CreateLootWithRigidbody`). The thing's model as it lies loose (`ObjectsFactory.CreateLootPrefab`: a weapon with its parts, anything else its prefab, its first object's own place, turn and scale the game's to set) with **every collider on it switched on** (a flare's excepted; the prefabs have most of theirs off): boxes on most things, a convex mesh on backpacks and rounds (`probe colliders <template>`). Every object of it on the `Loot` layer. The rigid body's mass is the thing's weight with all in it (`TotalWeight`); its drags a new `Rigidbody`'s (none, and 0.05 for the spin); it is swept if the box round its colliders is no bigger than `LootVolumeForHighQuallityPhysicsClient` (999 m³: every thing is). **Its model is moved back by its centre of mass** (`SetShift`) and its object then set where the thing set off: so what is drawn and what touches sit that much off the place it left, and the thing's place is its centre of mass from then on.

**When it stops** (`LootItem.PhysicsStopWorker`, every frame): once its body sleeps; or, swept and slower than a squared speed of 0.008, once 7.5 s have gone; or after 15 s whatever it does. The game then destroys the rigid body and the thing lies where it is.

**A round** the bolt throws out (`Firearms.ThrowPatronAsLoot`) is thrown from the weapon's shell port with the weapon's own extraction figures: not built.

## Built (0.12.0)

- **`trp-physics`**: the engine set up from the install's settings (one engine for the program, a scene a world; the layer filter with sweeping), bodies of boxes, spheres, capsules and convex hulls, the character joint as above, the spring and the fixed joint, the step; `corpse` (the ragdoll built as the game builds it, the stepper); `loose` (the throw, the let-fall, the body with its shift, the watch, the colliders of a model as it lies loose); `recorded` (a recorded death rebuilt and compared, for `bin/corpse` and the tests).
- **The corpse** (`trp-app/src/death.rs`) on it: the range's ground and boxes, the ragdoll, the weapon with its own colliders on its spring.
- **Loose things** (`trp-app/src/loose.rs`), with the options' `ITEM PHYSICS` on (restart required): what the inventory puts down and the backpack dropped are thrown, the quick reload's magazine let fall, each in a world of its own (it meets nothing that moves) with the range's ground and boxes; its model read on a thread as its picture's is, and its body, made when the model comes, stepped through every step it missed, so that it falls as it would have; drawn where its body is, looked at by its centre of mass, taken up as before. A thing on the ground that was not thrown (the range's own at the start, a round the weapon put down, what a restart puts back) is set down where it lies, its colliders' underside a centimetre over the ground, and let settle: **this work's layout, not the game's** (its things are where the map puts them). A model with no collider (an outfit of the other game's) falls as the box its meshes fill (`PROVISIONAL`). Off, a thing lies as a plain box where it is put, as before.

- **How a loose thing is drawn.** Since 0.13.0 (Bevy 0.18, wgpu 27) with the standard material, as a rig is, its pictures shared with the rigs' where they are the same (`SharedPictures`) and by their levels as every model's (`streaming.md`); a model is let go of once nothing on the ground draws with it. Measured: 48 rifles thrown out of their case one by one, 43 on the ground, and the app goes on. *Before* (0.12.3, 0.12.4, a material of its own, `loose_material.rs`, now gone): On DirectX 12, wgpu 24 (Bevy 0.16's) copies every bind group's samplers into one heap of 2,048 places for all that is drawn (`docs/findings.md`), and the standard material holds six whatever it draws with; a rifle on the ground brings a dozen materials of its own. 22 rifles thrown out of their case stopped the app in 0.12.2 (`create_bind_group` for a `StandardMaterial`: "Not enough memory left"); with a material of one sampler (0.12.3) every rifle of the range thrown down and some taken up again stopped it too (the user: "fragmented", 32 places left). **Since 0.12.4 a loose thing's material holds no sampler**: the standard material's shading (Bevy's own lighting functions, given what the standard material would have been given: the colour and its picture, the normal map, the gloss or surface picture for roughness and metal, the occlusion), every picture read through the sampler the view already holds for its environment map (linear with mipmaps, held at the edge; a place outside a picture taken back into it by hand, as a repeating sampler would; `PROVISIONAL`: the drawing alone, without the anisotropic filtering the pictures' own samplers have, so a thing seen at a slant on the ground is a little softer than in hand). Its pictures are shared with the rigs' where they are the same (`SharedPictures`), and a model is let go of once nothing on the ground draws with it. Measured: the same 22 rifles on the ground hold none of the places (463 of 2,048 in all: the range and the interface), where they held 1,848 with the standard material. **The game has no such limit**: it draws with Direct3D 11 (the user's own `Player.log`: "Direct3D 11.0"), which binds a draw's samplers to it; and it hides loot far off by its level-of-detail groups and culling objects (`LootItem.RegisterInCullingObject`). **wgpu 25 and on** keep one sampler heap for the device with each distinct sampler once, referred to by index (`wgpu-hal` 25's `dx12/sampler.rs`), which ends the limit for every material, the rigs' too: Bevy 0.17 and on, and so the material of one's own went with 0.13.0.

## Compared with what

- **The corpse, frame for frame** against the user's four deaths of 2026-10-15 (recorder 0.8.0, `local\traces\courses1\0-death-*`): rebuilt from each recording's first frame after the death and stepped as the game steps, the mean distance of the twelve bodies from where the game had them (`cargo run -p trp-physics --bin corpse -- <trace>`; the test `trp-physics/tests/corpse.rs` holds them):

  | Death | to 0.25 s | to 0.5 s | to 1 s |
  |---|---|---|---|
  | Standing | 2.6 cm | 4.7 cm | 12.1 cm |
  | Walking | 3.5 cm | 6.2 cm | 18.3 cm |
  | Sprinting | 1.5 cm | 2.0 cm | 10.8 cm |
  | Shot | 2.4 cm | 6.2 cm | 22.1 cm |

  What is left is likely the ground (taken flat, at the feet's height before the death: the map's own is not in the recording) and what of Unity's laying of the joint is still a guess. Each recording stops about a second after the death (the raid ends); recorder 0.9.0 keeps on with the dead player, and dumps the corpse's whole set-up and the engine's settings as the game has them.
- **Loose things: compared with nothing.** From the code and the install. Tested (`trp-physics`'s `loose` tests, `tests/loose.rs`, `tests/shapes.rs`): Unity's turn of three angles; a throw's start; the shift; the AK-74's magazine and a backpack on its convex hull let fall from 1.5 m come to rest within the game's 15 s. Seen in captures: the quick reload's magazine falling from the left hand to lie by the feet, a vest thrown from the inventory landing a step ahead. Recorder 0.9.0's numpad `1` (course 1: "Things let fall and thrown") records each thing set going (its template, weight, place, turn, velocity, spin, the thrower, the ground under it, its whole physical set-up) and then every frame of its fall.

## PROVISIONAL, and not the game's

`PROVISIONAL`: the joints' projection (Unity's defaults); triggers left out of a body's centre of mass; a convex hull's cooking (PhysX's defaults); the weapon let go as the box its meshes fill where it has no collider; a thing with no collider as the box its meshes fill; the first person's drawn body taken as the body for a thing let fall (the rig's squeeze along the view taken off; the game's `BifacialTransform` is not followed further). Not the game's: the dice of a throw (their own sequence; the game's seed is not known); the range's own things set down and let settle (the app's layout); flat ground under the recorded corpses.

## Not built

A round thrown out by the bolt from the shell port; the inventory's down-throw (no unloading of a magazine to the ground yet); a thing that is a weapon thrown from the inventory has no special case in the game and none here; sound; a corpse or a thing pushed by the character (the character is not in the engine: the game's own controller meets the world, not the engine's bodies, and `Loot` does not meet the player); the corpse's sleep.
