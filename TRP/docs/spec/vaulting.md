# Vaulting and climbing

How the character gets over a low obstacle or up onto one. Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly: the `EFT.Vaulting` namespace, `VaultState`, `ClimbState`, `VaultingMovementState`, `VaultingFallDownState`, `VaultingLandingState`, and the callers in `Player`, `MovementContext`, `Physical` and the firearm controller), of the body's animator controller and of its clips. Class, field and parameter names are the game's. Figures marked "here" are this install's (`globals.json`, `config.VaultingSettings` and `config.Stamina`).

Compared with the running game on 2026-10-10 (four traces: a vault and a climb, each from a walk and from a sprint): see the end. What the comparison goes by is the game's own measurement of the obstacle, fed to the simulation; the measuring itself (the grid of rays) is still from the code alone.

## Two moves, one key

There are two moves. A **vault** takes the character over an obstacle and down the other side (the game's movement state `ClimbOver`, number 24, class `VaultState`). A **climb** puts it on top (`ClimbUp`, 25, class `ClimbState`). Two more states follow a vault: `VaultingFallDown` (26), when the ground beyond is far below, and `VaultingLanding` (27).

Both are asked for with one key, `Vaulting`, which the game treats as held:

- Pressing it switches a per-frame call on, and releasing it switches the call off and clears a timer (`Player.Vaulting`, `EnableVaulting`, `DisableVaulting`). The inventory opening switches it off too, and so does its shutting.
- The press and the release are one and the same call, which turns the switch over from wherever it stands. So a key held while the inventory opens (off) and let go after it has shut turns the call **on** with the key up; a release under the open screen is not heard at all (`inventory.md`, "What an open screen does to the keys"). With the call on and the key up the timer runs, a jump is refused once it has passed its time, and the next press turns it off. Read 2026-10-11 and built (`Movement`'s `vault_on`; `the_inventory_switches_a_held_vault_key_off`); not recorded.
- While it is on, every frame calls the movement state's `Vaulting()` and then adds the frame's time (the engine's) to the timer (`ToggleVaulting`).
- `Vaulting()` does something where the state's class has it: the standing state (`IdlePlayerState`) and the walking state (`MovePlayerState`), and the classes built on those that do not take it back. So it calls `MovementContext.TryVaulting` standing, stepped aside, going down and getting up (all built on standing), walking, crawling and sprinting (built on walking). Lying still and rolled over it is empty by an override. In everything else (a jump, a landing, the stop out of a sprint, a vault under way) it is the base class's empty method.
- Of those, only standing, walking and the sprint lead anywhere: the checks below refuse a character whose prone flag is up, and the animator has no way into a vault from a sidestep or from getting up. But a try made stepped aside or getting up that passes its checks leaves its flag (`DoVault` or `DoClimb`) up, and nothing lowers it but a vault's own end: the animator then takes the move as soon as it is standing again, whether or not the key is still held. Read, not recorded.

**The jump shares the key.** In this install both `Jump` and `Vaulting` are bound to Space (`SPT_Runtime\user\sptSettings\Control.ini`): `Vaulting` with press type `Continuous`, `Jump` with press type `Release`. So a jump is asked for when the key comes up, and `Player.Jump` refuses it when the vault timer has passed `VaultingInputTime` (0.6 s here) or the animator's `IsVaulting` flag is up. A tap jumps on release; a hold tries to vault on every frame it is held, and held longer than 0.6 s it no longer jumps when let go.

In the list the game builds its key handlers from, the jump comes before the vault key, so on the frame of the release the jump is looked at while the timer still holds the hold's length. That order is read, not recorded.

Lying down, the jump key gets the character up (`PronePlayerState.Jump` calls `Prone`).

## Automatic vaulting is dead code

The component has a second path, `DoVaultingTick`, hooked to every frame, which would vault without the key. It runs only when the game setting `AutoVaultingMode` is `Automatic` (it is `Hotkey` here, the default) and the key is not held, and then asks `GetAutoVaultingStrategy`, which needs one of two switches, `IsEnableAutoVault` and `IsEnableAutoClimb`. Those are plain properties that start false, and only the game's vaulting debug tool sets them. So nothing vaults by itself, whatever the setting. The `AutoMoveRestrictions` in the settings are still read for two things: the height limits below (one figure of them) and a slope in the wall check behind the obstacle.

## What a try does

`MovementContext.TryVaulting`, on a frame the key is held in one of the three states:

1. **The gameplay restrictions** (`VaultingGameplayRestrictions.CanVaulting`) have to pass. They work out which move would be chosen from the obstacle *as last measured* (the measurement of an earlier frame, or nothing at all the first time), and then:
   - with the inventory busy (an item being moved), only an obstacle lower than that move's `MaxWithoutHandHeight`;
   - with medicine or food in hand, only lower than `MaxWithoutHandHeight`, and not at all when no move would be chosen;
   - no blind fire;
   - not lying down (`PoseToInt` is not 0);
   - **the weapon.** With a firearm in hand, an idling weapon passes. Any other operation (a reload, a shot, a look at the magazine) passes only for an obstacle lower than `MaxWithoutHandHeight`, or when no move would be chosen;
   - a grenade being cooked or thrown likewise.
2. **Forward has to be held** (`Player.IsForwardInputDirection`: the input direction's forward part above 0).
3. Then `VaultingComponent.TryVaulting`: measure the obstacle (`Tick`, below), choose the move from the fresh measurement (`GetVaultingStrategy`), and if there is one, write the measurement to the animator and raise `DoVault` or `DoClimb`.
4. If a move was chosen, whatever is in the left hand is put away (`RemoveLeftHandItem`).

## Measuring: the grid

The measurement is made with rays cast from a grid that hangs on a transform of the player prefab, `VaultingGridRoot`: a child of the animated `Player` object, at the feet, facing as the **body** faces. Its place is set by each measurement for the next: `(GridOffsetX, GridOffsetY, GridOffsetZ + speed * OffsetFactor)` in the body's axes, where the speed is the length of the character's velocity projected on the **view's** level heading (`PlayerRealForward`), counted only when that projection points along the heading and not against it. Here the offsets are 0 and the factor 0.1: a character walking forward at 2.8 m/s measures from 0.28 m ahead of its feet, one sprinting at 5.2 m/s from 0.52 m. A measurement uses the place the one before left, and the first ever the feet.

The grid, here (`GridSizeX` 0, `GridSizeY` 1.8, `GridSizeZ` 2, steps of 0.1 each way), is a single column, since its width is 0:

- **19 forward rays**, from heights 0, 0.1, ... 1.8 above the grid's root, cast forward along the body's facing, up to 10 m. Each hit is kept as a *forward point*, in order of height.
- **21 downward rays**, from 1.8 m above the root at 0, 0.1, ... 2.0 m ahead of it, cast straight down, up to 10 m. Each hit is kept as a *top point*, in order of distance, with what it hit.

The counts are `ceil(size / step) + 1`. The rays see every layer that collides with the player's own, less the player layer itself (`PlayerStaticCollisionsMask`). A ray that starts inside a solid does not see that solid (the engine's rule), so an obstacle taller than 1.8 m gives top points on whatever is under it.

Points are kept in the grid root's own axes: `y` up from the root, `z` ahead of it.

In one measurement (`VaultingController.Tick`), in this order: the rays; the heaviest point; some averages nothing reads but the debug tool; the root's new place; the obstacle.

### The heaviest point

Among the top points whose height is within the widest limits any move has (0.25 to 1.31 m here: the least `MinHeight` and the greatest `MaxHeight` of the four restriction sets), the one with the greatest `height * e^(-distance)` (`WeightCalculatorModel`). Of equals the nearest wins. A flat top's nearest point therefore wins, and of two obstacles a much higher one further off can beat a low one near by.

With no top point in the limits there is no heaviest point; what the code then holds is a blank point at the root (index 0, height 0, distance 0).

### The obstacle

From the heaviest point (`ObstacleCalculatorModel.CalculateMainObstacle`), with four figures starting at 0:

- Walk the top points **onward** from it, as long as each is within `0.2` m of the heaviest point's height. For each: the length is its distance less the heaviest point's; the height is the greatest of the height so far and the two points' heights; the distance is the heaviest point's `z`; the roof over it is felt for (below); and the drop behind it is measured (below).
- Then walk the top points **back** toward the character the same way. The length found this way is added to the onward length.
- If the length is still 0 (the heaviest point stands alone), the obstacle is taken to be one step long (0.1 m), with the heaviest point's height and distance, and the drop is measured from one step past it.
- The obstacle **is terrain** when the heaviest point is on the terrain layer. Terrain is never vaulted or climbed.

So: `MainObstacleHeight` is how high the top stands over the grid's root (the feet's level), `MainObstacleLength` how deep it is along the body's facing, `DistanceToMainObstacle` how far ahead of the *root* its near edge is (the root, not the feet: the faster the character, the further ahead the root).

**The roof** (`CalculateMinOverObstacleHeight`): a ray 1.6 m straight up from each point walked, and from the heaviest. The least height found is `MinRoofHeight`; none found, there is no roof.

**The drop behind** (`CalculateOverObstacleRatio`), measured again for each point walked, so the figure left is the *last* point's, which is the last one of the walk back if there was one and of the walk onward otherwise. From that point: 0.5 m up and 0.8 m on (in a sprint: 0.5 m up and on to 2.6 m ahead of the root), a ray 10 m down. With `h` the height of what it hits over the root and `t` the point's own height:

`BehindObstacleRatio = h / (t + 0.5 - h) - 1`

and 0 if it hits nothing. Ground behind at the feet's level gives -1. Ground behind at the top's level (a platform) gives `2t - 1`: 1 for a top a metre up. The animator knows this figure as `OverObstacleHeight`.

### Stairs

`StairsCalculatorModel.IsStairsCondition`: six pairs of downward rays from 0.9 m above the root, 0.1 m apart, from 0.366 m behind the root to 0.234 m ahead of it. The rises between neighbours are added up, over the step, and divided by 7 (the count the loop is sized by, `round(0.366 / 0.1 * 1.8)`, one more than the pairs it adds). An average between 0.45 and 1.1917 (slopes of 24 to 50 degrees) is stairs, and on stairs nothing is vaulted or climbed. If any of the rays hits nothing it is not stairs.

## Choosing the move

`GetVaultingStrategy`, on the fresh measurement. No move unless all of:

- vaulting is on in the settings (`IsActive`);
- the animator's `IsVaulting` is down;
- the input direction is not backward;
- the character is on the ground;
- the obstacle is not terrain;
- the body animator's first layer is not in a transition, and its jump flag is not up;
- it is not stairs.

Then a **vault**, if the stamina allows (below) and `VaultMoveModel.CanMove`, and not this: *a sprint with the drop behind above -0.5* (a sprinting character only vaults where the ground beyond is clearly lower than the top). Failing a vault, a **climb**, if the stamina allows and `ClimbMoveModel.CanMove`, and the character is not sprinting. Sprinting is the sprint's flag (`Physical.Sprinting`), not the movement state.

A vault can be made (`CheckVaultCondition`) when all of:

| | Here |
|---|---|
| the height is within `MoveRestrictions` | 0.25 to 1.1 m |
| the length is within them | 0 to 0.41 m |
| the distance is at most `MinDistantToInteract` | 0.5 m |
| the drop behind is under -0.3 | |
| nothing stands behind the obstacle | below |
| the roof, if any, is at least half the player's height up | 0.9 m |
| nothing stands before the obstacle | below |

A climb (`CheckClimbCondition`) when all of:

| | Here |
|---|---|
| the height is within `MoveRestrictions` | 0.25 to 1.31 m |
| the length is within them | 0 to 10 m |
| the distance is at most `MinDistantToInteract` | 0.5 m |
| the roof, if any, is at least 0.95 of the player's height up | 1.71 m |
| nothing stands before the obstacle | below |

The player's height in these is a constant of the code, 1.8 m.

**The drop behind under -0.3** means the ground 0.8 m past the top's last point is lower than `0.41 * (t + 0.5)` over the feet's level: under 0.62 m for a top a metre up, under 0.31 m for one a quarter metre up. A thin wall with level ground beyond passes; a ledge or a box deeper than the reach does not, and is climbed.

**Nothing behind** (`CheckBehindObstacleWall`, vault only): no forward point may be at once higher than 1.1 times the obstacle's height, lower than the obstacle's height plus a gap, and no further than the heaviest point's distance plus the obstacle's length plus 1.6 m. The gap is 0.9 m for an obstacle of 0.41 m or more (the automatic vault's `MaxHeight`) and widens to 1.8 m toward an obstacle of no height. In words: within 1.6 m beyond the obstacle there must be room for a body just above it.

**Nothing before** (`CheckBeforeObstacleWall`, both moves): of the forward points, taken from the lowest ray up, the first that is at least as high as the heaviest point decides: nearer than the heaviest point, no move; otherwise fine. None that high, fine.

`MaxWithoutHandHeight` (0.65 m here for both moves) and, for the climb, `MaxOneHandHeight` (1.2 m) split the moves by how much of the body they take: under the first the hands stay on the weapon; a vault over it takes the left hand; a climb over it takes one hand and over the second both.

## Stamina

Whether the stamina allows (`Physical.CanVault`, `CanClimb`): the legs must hold more than 15; for an obstacle higher than `MaxWithoutHandHeight`, so must the arms.

What it costs, charged all at once when the move's state is entered (`Physical.OnVault`, `OnClimb`; each cost is a pair in the settings, from an unburdened character to a fully overweight one; standing it goes by the walk's overweight, crouched by the general one; neither pool refills for 0.1 s after, and longer the emptier it is, as with every charge):

| | Legs | Arms |
|---|---|---|
| Vault, standing | nothing unless over the walk's weight, then `VaultLegsConsumption` (5 to 20) | over 0.65 m: `VaultOneHandConsumption` (5 to 16) |
| Vault, fully crouched | `VaultLegsConsumption`: 5 unburdened | the same |
| Climb | `ClimbLegsConsumption` (0 to 20) | over 1.2 m: `ClimbTwoHandsConsumption` (12 to 25); over 0.65 m: `ClimbOneHandConsumption` (8 to 20) |

"Standing" and "fully crouched" are stamina's stance classes (`stamina.md`): fully crouched is a stance target under 0.11. What the arms cannot pay the legs pay again, as with getting up.

So for an unburdened character: a standing vault of a low wall is free, one of a high wall costs the arms 5, a climb onto something chest high costs the arms 12, and a fully crouched vault costs the legs 5.

## Into the animator

A chosen move writes four of the animator's parameters and raises a flag (`SetParametersInAnimator`, `DoVaultingByStrategy`):

- `VaultingHeight` and `VaultingLength`: the obstacle's;
- `OverObstacleHeight`: the drop behind;
- `AbsoluteForwardVelocity`: the forward speed the grid's place was set with;
- `DoVault` or `DoClimb`.

The body's controller (`Player_anim_controller`, first layer, sub-machine `Vaulting`) does the rest. Its states, with the movement state each one's script names:

| State | Movement state | Clips | Length | Speed |
|---|---|---|---|---|
| `Vault` | 24 | `Jump_over_20` to `Jump_over_100`, blended by `VaultingHeight` (at 0.2, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0) | 1.75 s | times `Vaulting_speed` |
| `Sprint_vault` | 24 | `Jump_over_sprint_30` to `_69` (at 0.2 to 0.699) and `Sprint_Vault_70` to `_110` (at 0.7 to 1.1), by `VaultingHeight` | 0.96 s | times `Vaulting_speed` |
| `Climb_on` | 25 | `Climb_on_20` to `Climb_on_130`, by `VaultingHeight` (every 0.1 from 0.2 to 1.3) | 1.89 s | times `Vaulting_speed` |
| `Vault_fall` | 26 | the jump's loop clips, by direction | 0.87 s | 1 |
| `Vault_land` | 27 | the jump's landing clips | 0.45 s | 1 |
| `Vault_Land_Idle` | 27 | the standing idle, by stance (loops) | 0.13 s | 1 |

and the ways between them (a blend's length is a share of the state being left unless given in seconds; "after" is the exit point; "from" is where the next state starts):

| From | To | When | Blend | After | From |
|---|---|---|---|---|---|
| `Idle_Aim` | `Vault` | `DoVault`, not `IsVaulting` | 0.2 s | | 0 |
| `Idle_Aim` | `Climb_on` | `DoClimb`, not `IsVaulting` | 0.15 | | 0 |
| `MOVE` | `Vault` | `DoVault`, not `IsVaulting` | 0.1 | | 0.1 |
| `MOVE` | `Climb_on` | `DoClimb`, not `IsVaulting` | 0.15 | | 0 |
| `Sprint` | `Sprint_vault` | `DoVault`, not `IsVaulting` | 0.1 | | 0 |
| `Vault` | `Vault_Land_Idle` | not `Inert`, `OverObstacleHeight` over -1.3 | 0.2 | 0.65 | 0 |
| `Vault` | `Vault_Land_Idle` | `Inert`, `OverObstacleHeight` over -1.3 | 0.15 | 0.6 | 0 |
| `Vault` | `Vault_fall` | `OverObstacleHeight` under -1.3 | 0.159 | 0.661 | 0.004 |
| `Sprint_vault` | `Sprint` | `VaultingHeight` under 0.65, `Sprint` | 0.1 | 0.82 | 0.22 |
| `Sprint_vault` | `Sprint` | `Sprint` | 0.2 | 0.9 | 0.06 |
| `Sprint_vault` | `Vault_Land_Idle` | not `Inert`, `OverObstacleHeight` over -1.3 | 0.25 | 0.9 | 0 |
| `Sprint_vault` | `Vault_Land_Idle` | `Inert`, not `Sprint`, `OverObstacleHeight` over -1.3 | 0.25 | 0.9 | 0 |
| `Sprint_vault` | `Vault_fall` | `OverObstacleHeight` under -1.3 | 0.4 | 0.55 | 0.4 |
| `Vault_fall` | `Vault_land` | `IsGrounded` | 0.1 | | 0 |
| `Vault_land` | `Sprint` | `Sprint` | 0.6 | 0.25 | 0 |
| `Vault_land` | `MOVE` | `Inert` | 0.5 | 0.49 | 0 |
| `Vault_land` | `Idle_Aim` | not `Inert` | 0.5 | 0.5 | 0 |
| `Vault_land` | `Idle_Aim` | | 0.5 | 0.95 | 0 |
| `Vault_Land_Idle` | `Sprint` | `Sprint` | 0.1 | | 0 |
| `Vault_Land_Idle` | `MOVE` | `Inert` | 0.1 | | 0 |
| `Vault_Land_Idle` | `Idle_Aim` | not `Inert` | 0.25 | | 0 |
| `Climb_on` | `Idle_Aim` | not `Inert` (two ways, one for each side of `Level` 0.49) | 0.25 | 0.75 | 0 |
| `Climb_on` | `MOVE` | `Inert` | 0.2 | 0.78 | 0.15 |

There is no way into a vault or a climb from anything but standing, walking and the sprint. A sprinting character can only vault (the sprint has no way into `Climb_on`, and the choice refuses a climb in a sprint).

**A vault out of a sprint does not go straight back into it.** Leaving the sprinting state switches the sprint off (its own flag and the animator's), and the vault's state, entered after, tells the animator the flag as it then is: down. So of `Sprint_vault`'s ways out the two that need `Sprint` are never open, and the move ends through `Vault_Land_Idle` (or the fall). On the way out of the vault the sprint is asked for again (`EnableSprint(true)`, which is the stamina's flag, with its charge for a sprint begun again too soon), and the walk winds up into the sprint from there as after any key press. Read, not recorded.

**A drop behind under -1.3** sends a vault into `Vault_fall`: ground beyond lower than `-0.43 * (t + 0.5)` under the feet's level, which is 0.64 m down for a top a metre up.

One layer more knows of it: `Additive_BodyTurningOnMove` goes to a resting state while `IsVaulting` is up. `Additive_Aiming` and `Additive_FullBody` each have a state named for vaulting with ways out and none in.

## The moves as movement states

All four are plain `MovementState`s: the animator calls them each frame it plays a state that names them, and they move the character by the animator's own root motion. What each adds:

**Entering a vault or a climb** (`VaultingMovementState.Enter`, then the move's own):

- the character controller's capsule is put to its standing size, and remembered;
- the speed cap is set for the state with a factor of 1 in place of the walking limit (`UpdateVaultingSpeedLimit`; the two states have no cap of their own in `SpeedLimiter`, so it is the walking state's by Strength);
- `IsVaulting` goes up, and the state takes the obstacle's height and length back from the animator;
- the controller stops pushing itself up out of things (`SetEnableVerticalDepenetration(false)`);
- for an obstacle of `MaxWithoutHandHeight` or more, the lean is put to 0;
- the stamina is charged, and `Vaulting_speed` set (below);
- for an obstacle over `MaxWithoutHandHeight` (a vault; a climb: or equal to it): the sights go down if they were up, and the view is taken over (below);
- a vault out of a sprint keeps the animator's sprint flag as the sprint's own flag has it and raises `Inert`;
- a vault resizes the capsule at once (below).

**Each frame** (`ManualAnimatorMoveUpdate`): the body follows the view as in any state (drawn after it past the trunk limit), and the animator's root motion is applied as it comes: no gravity, no projection on the ground, no limit, no blending with anything. Only this: while the character counts as on the ground, the motion's downward part is dropped (`ApplyMotion`). Movement keys do nothing but raise or lower `Inert` (raised with the stick past 0.1), which chooses the way out. A climb resizes the capsule after every move.

**Leaving** puts the capsule back, gives the view back, and lets the controller push itself out of things again. A vault lowers `DoVault`; a climb lowers `DoClimb` and `IsVaulting`. A vault that came out of a sprint, if the character can still walk, turns the sprint back on and lowers `IsVaulting` itself.

**`VaultingFallDown`** is an empty state: the base class's motion, which has gravity. **`VaultingLanding`** is the jump's landing state (`JumpLandingState`: motion stops if no movement key is held for half of `MaxTimeWithoutInput`, and the speed setting eases to 0.35 meanwhile), and lowers `IsVaulting` when left.

So `IsVaulting` stays up from the frame the vault's state is entered until the landing state is left, or until the sprint is back. While it is up no jump is taken, no new vault starts, and the key's per-frame call finds a state in which it does nothing.

The stance cannot be changed in a vault or a climb (`SetPoseLevel` refuses in both).

### The view

For an obstacle under `MaxWithoutHandHeight` the view turns as the state's script allows (no factor or limit is written on these states: the defaults).

For a higher one the view does not answer the mouse at all (`Rotate` does nothing), and each frame is placed by a function of the state's own (`RotateAction`): the view as it was when the move began, with its pitch blended toward a limit by the clip's `Mouse_look` curve. The limit: a view pitched down is brought up to at most 50 degrees down; a view pitched up is left. The curve runs 0, 0.5, 1, 0.5, 0 over a clip, so the view is eased to the limit by the middle of the move and back to where it was by the end. The yaw does not change.

### The capsule

The game's character controller is a capsule, and the moves get it past the obstacle by shrinking it from below.

- **A vault**, on entering, with `h` the obstacle's height and the capsule at its standing height `H` (1.6 m): the height becomes `H - 1.1 h` and the centre is put at `h + (H - h) / 1.5` above the feet. The capsule's underside is then `0.267 + 0.883 h` above the feet: 1.15 m for a wall a metre high, clear of it.
- **A climb**, after every frame's move, for a frame that rose by `d`: the height shrinks by `d` and the centre rises by `d / 2`, so the underside rises by `d` against the feet while the feet themselves rise by `d`. Halfway up the underside is already at the top.

The capsule is put back when the move's state is left.

### Speed

`Vaulting_speed`, which multiplies the playback of the three move states, is set on entering (`SetVaultingSpeed`) from the move's `SpeedRange` (a vault 1.25 to 0.9, a climb 1.1 to 0.8 here) at how overweight the character is: standing by the walk's overweight, crouched by the general one. An unburdened character vaults at 1.25 times the clips' pace and climbs at 1.1. With an obstacle over `MaxWithoutHandHeight`, a damaged leg or left arm (and for a climb over `MaxOneHandHeight`, a damaged right arm) counts as fully overweight, or three quarters on painkillers.

How long a move then takes is the blended clips' length over that pace, up to the exit point of the way out taken; measured figures are under "Built".

## The clips

Each clip carries the move as root motion and the body's shape as pose. Read from `character_animations.bundle` (`probe bodyclips <clip> tracks Root_Joint` prints the motion curves at five points):

| Clip | Length | Forward | Up |
|---|---|---|---|
| `Jump_over_100` | 2.00 s | 1.65 m | 0.15 m a quarter in, back to 0 by three quarters |
| `Climb_on_100` | 2.13 s | 0.65 m | 1.00 m, most of it between a quarter and three quarters in |

**A vault hardly lifts the character.** The root travels forward along the ground and the pose carries the body over; the capsule, shrunk from below, passes over the top. (How much of a clip's rise goes out as motion at all is the clip's own import setting, as for every clip: `body-animator.md`.) A climb's root does rise by the obstacle's height, and the blend between clips made for heights 10 cm apart gives a rise for any height between.

**A climb does not carry the character's axis onto the top by itself.** Its clip goes 0.65 m forward, of which the part before the capsule's underside has cleared the top is lost against the obstacle's face, and the way out of the state blends the last of it away. From against the face of a box a metre high the axis ends about 10 cm short of the edge, at the top's height. What holds the character there is the capsule's rounded foot resting on the edge, which is the engine's collision and a host's to answer for ("What it asks of a host").

The clips carry the hand-over curves the sprint and the crawl have (`weapon-handling.md`, "Carried by the body"): `Weapon_3rd` is 1 throughout, and `First_Person_Curve_Weight`, `Hand_Left`, `Elbow_Left`, `Shoulder_Left` and the three for the right arm rise to 1 a quarter in and fall back by the end (the vault's to 0.6 three quarters in; the climb's hands to 0 by then). So for the clips made for high obstacles the body takes the weapon and both arms for the middle of the move, by rules already built. `Aiming_Layer` and `Mouse_look` are there too.

## What else answers

- **The trigger.** `FirearmController.SetTriggerPressed` passes a press on only if `CheckVaulting` allows: in a vault or climb of an obstacle over `MaxWithoutHandHeight`, a press is no press.
- **The sights** go down on entering such a move (above), and nothing puts them back.
- **Harm.** A vault or climb of an obstacle of `MaxWithoutHandHeight` or more with a damaged left leg costs that leg 3 points; a climb of `MaxOneHandHeight` or more with a damaged right leg costs that leg 3.
- **Sound.** Three audio controllers (vault, sprint vault, climb) play from marks of the states: `footsteps.md`, "The vault's sounds" (read and built 2026-10-23).
- **The landing's sound** after a climb is the jump's, with the height taken differently.

## What it asks of a host

Rays into the host's world, a few dozen on each frame the key is held in a state that can vault: 19 forward and 21 down for the grid, up to 21 short ones up for the roof, one down for the drop behind each point walked, and 12 down for the stairs. A host answers each as its own line cast would, with the engine's rule that a line starting inside a solid does not see it. It also has to say whether what the heaviest point hit is terrain.

And the capsule: while a vault or a climb is under way the host's own collision has to let the character through whatever is lower than the capsule's underside, which the simulation gives it.

And footing: a climb ends with the character's axis short of the top's edge (above), so a host whose character is to end on the top has to hold it up by the edge as a rounded foot is held, and say so when asked where the ground holds the feet.

## Built

2026-10-10. Measuring and choosing: `crates/trp-sim/src/vaulting.rs` (`Vaulting::measure`, `choose`; `VaultingSettings` from `globals.json`). The moves: four movement states in `movement.rs` (`State::Vault`, `Climb`, `VaultFall`, `VaultLanding`) over the animator's six states, which the body now loads; the key and its timer (`Input::vault`, and `Input::jump` refused past `VaultingInputTime` or in a vault); the stamina's gate and charges (`Stamina::can_vault`, `vault`, `climb`); the pace (`Vaulting_speed`); the capsule (`Movement::capsule`); the view of a move that takes a hand (`Movement::shown_pitch`, and no turning meanwhile); no trigger in such a move (`Movement::can_press_trigger`) and the sights taken down by it (`Movement::take_aim_taken`); the lean put to 0; the stance not to be changed; the sprint asked for again after a vault out of one.

What a host is asked for it (`World`): `line` as before, for every ray; `terrain`, whether what a line hit is the terrain itself (everything is, unless the host says otherwise, so a world of bare ground vaults nothing); `obstructed` now with the capsule's reach over the feet (`Capsule`), so the host can let the character past what is lower than its underside; and `footing`, where the ground holds the feet of a character at a place, which is the ground's height under its axis unless the host says more. A host tells the movement whether the weapon's logic is busy (`Input::weapon_occupied`, from `Firearm::occupied`).

In the app: Space held vaults or climbs and Space let go jumps, as this install binds them. The range's boxes can all be stood on, the low wall beside the lane is there to vault (0.6 m high, 0.4 m thick) and the ring of blocks to climb (1 m cubes); since 2026-10-14, night, **a window in the door wall** (the user: "add a small vaultable window by the door"; `range::WINDOW`: a sill 0.9 m up in the wall's 0.4 m, an opening 1 m high and 1 m wide), made to the rules above: measured from the lane's side by the simulation's own grid it is an obstacle 0.90 m high and 0.30 m deep and the move chosen is a vault, and before the wall beside it nothing is (`the_window_is_vaulted_through`); a run held on the key walks through it in the vault's state (`--start window --hold w+vault`). The range answers `footing` with a rounded foot of the capsule's radius: `PROVISIONAL`, the engine's own rule for a capsule on an edge not being read.

Measured in the simulation (`cargo test -p trp-sim --test sim vault -- --nocapture`; an unburdened, untrained character at 120 frames a second, against boxes of a test world that keeps the character out of them as the game's own controller does, since 2026-10-10):

| | |
|---|---|
| A wall 1 m high and 0.35 m thick, walked into with the key held | measured as height 1.0, length 0.3, distance 0.5, drop behind -1; vaulted; the arms pay 5, the legs nothing |
| That vault, from its state's first frame to walking again | 0.68 s through the vault's state and one frame of its landing; 1.43 m forward, to 0.73 m beyond the wall (the last of it the push out of the wall); the feet stay at the ground's level |
| The capsule in it | from 1.15 to 1.65 m over the feet |
| The view in it | does not turn; turns again from the landing on |
| A wall 0.4 m high | vaulted in 0.47 s, at no cost, with the view free and the trigger live |
| A box 1 m high and 2 m deep | climbed; the arms pay 8; 1.08 s; 0.39 m forward and 0.99 m up, to 2 cm past the box's face, where the foot rests on the edge; walking on from there it is on the top |
| A sprint at a wall 0.9 m high with the key held | measured from 0.36 m ahead of the feet; vaulted from 0.87 m short of the wall; 0.76 s through the vault, its landing and the walk to the sprint again, 1.38 m past the wall |
| The key tapped for a tenth of a second | jumps when let go |
| The key held three quarters of a second | does not |

Seen in the app (captures `local\shots\vault_body_b.png`, `vault_fp.png`, `climb_body_b.png`, from `--scenario local\scenarios-app\vault-wall.json --yaw 90`): the figure goes over the low wall with its knees drawn up and the rifle held, and comes onto a block with its left hand planted on the top and the rifle in its right.

The medicine's and food's and the grenade's refusals are built since 0.131.0 (`movement.md`, "Room to move": `Input::hands_limit_vault`, from the code; blind fire's since 0.123.0, `blind-fire.md`). Not built: the inventory's (an item being moved); harm to a damaged leg; the speed of a move with a damaged limb; sound; the left hand's item put away. `VaultState.Exit` asks for the sprint again only if the character can walk, which here means not locked against an obstacle (`movement.md`, "Stopped by what stands in the way"); the other thing that stops a walk, legs being healed, does not exist here. The flag a try leaves up when made stepped aside or getting up is as the game's (above).

Taken on trust from the reading and marked where they are used: the capsule's figures in a move are the ones the code sets, with no account of the engine not letting a capsule be shorter than it is wide.

## Compared with the running game

**Four traces, 2026-10-10** (recorder 0.4.8; kept in `local\traces\recording13-vault` and `recording13-vault-low`). The user ran the two vault scripts (`62-vault-walk`, `63-vault-sprint`) at two obstacles: one the game measured as 1.13 to 1.14 m high and 0.3 m deep, which is over a vault's 1.1 m and so was **climbed**, from a walk and from standing after the sprint had been stopped by it; and a low one, 0.49 m high and 0.2 to 0.3 m deep, which was **vaulted**, from a walk and from a sprint.

**How they are replayed.** A trace does not hold the map. It does hold what the game's own grid measured on every frame (`vault_obstacle`: height, length, the drop behind, the forward speed), and how far under the character something solid lay (`ground_below`). So: on the frame the game raised `DoVault` or `DoClimb`, the simulation's try is given the game's measurement as within reach, and on every other frame nothing within reach (`Movement::vault_given`); what the simulation then chooses, and everything after, is its own. And the ground is rebuilt from the path walked (`pos.y - ground_below` at each place: the wall shows as a step up where the character's axis came over it and a step down behind it), with the character kept out of it as the game's own controller keeps it (`movement.md`, "The character's own collision"). The wall so rebuilt is placed to within the distance the character moved between two frames, 2 to 4 cm.

| | Vault from a walk | Vault from a sprint (to 5.18 s) | Climb from a walk |
|---|---|---|---|
| Movement state, on every frame | the game's | the game's | the game's |
| The body animator's state, the state being entered, and how far through | the game's, the time to 0.0000 | the game's, the time to 0.0000 | the game's, the time within 0.003 |
| `DoVault`, `DoClimb`, `IsVaulting`; the four figures given to the animator | the game's on every frame | the game's | the game's |
| The capsule's height and centre | 0.0000 on every frame | 0.0000 | 0.0000 |
| Legs and arms | exact (no cost) | exact (the sprint's drain, off through the vault) | legs exact; the arms pay 8, within 0.02 after |
| Distance from the start | within 6 mm | within 6 mm | within 30 mm |
| The speed over the ground, by the game's own figure | the game's on every frame (0.02 m/s at worst) | the game's bar one frame (0.5 m/s) | the game's but for frames at the wall's face |
| The eye | within 0.2 mm | within 1.9 mm, 0.2 mm rms | within 49 mm, 2.9 mm rms (40 and 4.2 before 2026-10-11, when the body's lag in height was put on the player's clock: nearer on the whole, further on the one frame at the wall's rim, where the wall's place from the trace decides) |
| The weapon as carried | within 6 mm and 1.8 degrees | within 65 mm and 38 degrees for the fifth of a second after the vault (`body-animator.md`, "A transition cut short", left open); otherwise as the walk's | within 8 mm and 2.3 degrees |

The fourth trace, the sprint at the high wall, agrees in its sequence and not frame for frame, for a reason that is the map's: the wall stands some 15 degrees off square to the run, the game's character slid along it for a fifth of a second before it was stopped, and a ground rebuilt along one line has no such wall. There the simulation is stopped dead and goes into the sprint's stop 13 frames sooner; the climb that follows starts on the game's frame (the game's flag says when) and runs as the walk's does. And past 5.2 s the sprint in the second trace ran onto a rock 0.4 m high and round it, which a line cannot hold either.

**What the traces confirmed, each read and nothing more until now:**

- *The choice.* 1.13 m is climbed and not vaulted; 0.49 m with level ground behind is vaulted; a sprinting character vaults the low one and does nothing at the high one (too high to vault, and a sprint never climbs).
- *The costs.* Nothing for a standing vault under 0.65 m; 8 from the arms for a climb between 0.65 and 1.2 m. The legs pay nothing for either.
- *The pace.* `Vaulting_speed` 1.25 for the vaults and 1.1 for the climbs.
- *The capsule.* A vault's is `1.6 - 1.1 h` high with its centre at `h + (1.6 - h) / 1.5`, set at once (1.063 and 1.229 for 0.488 m); a climb's shrinks from the standing one by each frame's rise, to four decimal places on every frame. The standing capsule's centre is 0.8 m up: no skin.
- *A vault's root stays on the ground.* The feet's height does not change through either vault.
- *A vault's length.* From a walk: 56 frames in the vault's state (0.467 s), one frame of the landing state, then the walk. From a sprint: 66 frames (0.554 s), one frame of landing, three of the walk, then the sprint: the sprint is not gone back into straight from the vault, as read.
- *A vault out of a sprint goes at the walk's cap.* The state's hard cap on speed is the walking one (4.27 m/s for the recorded character) and the sprint's vault is held to it all the way.
- *A climb's length.* 148 frames from the walk (1.237 s) and 141 from standing (1.183 s), into the walk through the way out at 0.78.
- *The stop out of a sprint takes no vault key,* and standing takes it at once: the climb began on the standing state's first frame after the stop.

**What they corrected or added** (each built the same day):

- *After a vault the character is pushed out of the wall,* forward, at the state's cap on speed, for nine frames (`movement.md`, "The character's own collision"). The simulation had it walk off at the animation's pace: 14 cm short.
- *The way out of a vault is a transition cut short,* and how the engine holds the blend it cuts was not known (`body-animator.md`, "A transition cut short"). The simulation dropped it: the walk came in over 0.013 s where the game takes 0.12, and the sprint out of a sprint's vault began a frame off.
- *A sprint into a wall is stopped by a lock of the game's own* (`movement.md`, "Stopped by what stands in the way"), which was not read at all.
- *A climb begun on rising ground loses a frame in three at first:* with the upward push off, the capsule sinks into the slope until the controller drops a move to set it back on the ground.
- *The view in a climb that takes a hand:* the state's own figures for turning stay at their defaults (1 and 99) and the view is locked by other means, as read. The recorded view was 1.9 degrees down, so the blend toward the 50-degree limit had nothing to do: still from the code.

**Still from the code alone:** the grid's rays and everything that goes by their points (the heaviest point, the roof, what stands before and behind, stairs, terrain); the distance within which a move is taken (a trace has no figure for it); `Vault_fall` (no drop behind was deep enough); a vault over 0.65 m, which takes a hand, lowers the sights and costs the arms 5; a climb over 1.2 m; any of it crouched or overweight; the trigger and the sights in a move; the jump refused past the key's time.

**Also in these traces, for the first time on record: a fall.** Out of each climb the character walked on over the top, 0.3 m deep, and off its far side, 1.12 m down. Gravity begins on the frame after the one in which the ray under the axis loses the top, the pace over the ground is kept for a quarter of a second and then dies away as read (3.12, 3.12, 3.12, then 1.85, 1.73, 1.63, 1.52, 1.43 m/s by the game's figure, against the simulation's 1.83, 1.72, 1.62, 1.51, 1.42), and the fall takes 0.485 s. The simulation's is one frame ahead, because its wall stands 3 cm further on (`movement.md`, "Falling").
